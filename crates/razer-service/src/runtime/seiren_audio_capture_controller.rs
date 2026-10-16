//! One UI-owned local capture session. COM interfaces stay on its worker;
//! IPC only drains observed PCM and joins the owner during stop/shutdown.
use anyhow::{Context as _, ensure};
use razer_device::seiren_audio_capture::{self as capture, CaptureFrame, CapturePoll};
use razer_device::seiren_microphone::{CaptureOptions, PcmPlaybackFile, PlaybackPoll};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

const MAX_PENDING_SAMPLES: usize = 65_536;

#[derive(Debug)]
pub(super) struct Canceled;
impl std::fmt::Display for Canceled {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Seiren audio capture owner canceled")
    }
}
impl std::error::Error for Canceled {}

#[derive(Default)]
struct Observed {
    metadata: Option<CaptureFrame>,
    pcm: VecDeque<f32>,
    diagnostics: Vec<String>,
    skipped: usize,
    lossless: bool,
    samples_seen: usize,
}
impl Observed {
    fn record(&mut self, mut frame: CaptureFrame) -> anyhow::Result<()> {
        ensure!(
            frame.capture_sample_rate != 0,
            "Actual capture sample rate is zero"
        );
        if let Some(previous) = &self.metadata {
            ensure!(
                previous.endpoint_id == frame.endpoint_id
                    && previous.capture_sample_rate == frame.capture_sample_rate,
                "Audio capture endpoint or format changed within its owned session"
            );
        }
        if self.lossless {
            ensure!(
                self.samples_seen == 0
                    || !frame
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic
                            == "Seiren OS capture reported a PCM discontinuity"),
                "Seiren microphone recording lost PCM during capture"
            );
            ensure!(
                self.pcm.len().saturating_add(frame.mono_pcm.len()) <= MAX_PENDING_SAMPLES,
                "Seiren microphone recording could not retain all unread PCM samples"
            );
        }
        self.samples_seen = self.samples_seen.saturating_add(frame.mono_pcm.len());
        self.pcm.extend(frame.mono_pcm.drain(..));
        if self.pcm.len() > MAX_PENDING_SAMPLES {
            let discarded = self.pcm.len() - MAX_PENDING_SAMPLES;
            self.pcm.drain(..discarded);
            self.skipped = self.skipped.saturating_add(discarded);
        }
        for diagnostic in frame.diagnostics.drain(..) {
            if self.diagnostics.len() < 64 && !self.diagnostics.contains(&diagnostic) {
                self.diagnostics.push(diagnostic);
            }
        }
        self.metadata = Some(frame);
        Ok(())
    }
    fn take(&mut self) -> Option<CaptureFrame> {
        let mut frame = self.metadata.clone()?;
        frame.mono_pcm = self.pcm.drain(..).collect();
        frame.diagnostics = std::mem::take(&mut self.diagnostics);
        if self.skipped != 0 {
            frame.diagnostics.push(format!(
                "Seiren analyser retained the latest PCM; {} older unread samples expired",
                self.skipped
            ));
            self.skipped = 0;
        }
        Some(frame)
    }
}

struct Operation {
    session_id: u64,
    product_id: u32,
    canceled: Arc<AtomicBool>,
    observed: Arc<Mutex<Observed>>,
    worker: JoinHandle<anyhow::Result<()>>,
}
struct PlaybackOperation {
    session_id: u64,
    product_id: u32,
    canceled: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
    worker: JoinHandle<anyhow::Result<()>>,
}

#[derive(Default)]
pub(super) struct Controller {
    active: Option<Operation>,
    playback: Option<PlaybackOperation>,
}
impl Controller {
    pub(super) fn playback_start(
        &mut self,
        session_id: u64,
        product_id: u32,
        clip: PcmPlaybackFile,
    ) -> anyhow::Result<PlaybackPoll> {
        capture::validate_owner(session_id, product_id)?;
        ensure!(
            self.playback.is_none(),
            "Seiren playback already has an owner"
        );
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let started = Arc::new(AtomicBool::new(false));
        let observation = started.clone();
        let worker = thread::Builder::new()
            .name("seiren-microphone-playback".into())
            .spawn(move || {
                let mut publish = || {
                    if signal.load(Ordering::Acquire) {
                        return Err(Canceled.into());
                    }
                    observation.store(true, Ordering::Release);
                    Ok(())
                };
                #[cfg(windows)]
                {
                    super::native::seiren_audio_playback(&signal, &clip, &mut publish)
                }
                #[cfg(not(windows))]
                {
                    let _ = (clip, publish);
                    anyhow::bail!("Local Seiren playback requires a platform PCM render adapter")
                }
            })?;
        self.playback = Some(PlaybackOperation {
            session_id,
            product_id,
            canceled,
            started,
            worker,
        });
        Ok(PlaybackPoll {
            session_id,
            product_id,
            started: false,
            terminal: false,
            canceled: false,
            error: None,
        })
    }
    pub(super) fn playback_poll(&mut self, session_id: u64) -> anyhow::Result<PlaybackPoll> {
        let active = self
            .playback
            .as_ref()
            .context("Seiren playback owner is absent")?;
        ensure!(
            active.session_id == session_id,
            "Seiren playback owner ID differs"
        );
        if active.worker.is_finished() {
            return finish_playback(self.playback.take().unwrap());
        }
        Ok(PlaybackPoll {
            session_id,
            product_id: active.product_id,
            started: active.started.load(Ordering::Acquire),
            terminal: false,
            canceled: active.canceled.load(Ordering::Acquire),
            error: None,
        })
    }
    pub(super) fn playback_stop(&mut self, session_id: u64) -> anyhow::Result<PlaybackPoll> {
        let active = self
            .playback
            .as_ref()
            .context("Seiren playback owner is absent")?;
        ensure!(
            active.session_id == session_id,
            "Seiren playback stop owner ID differs"
        );
        let active = self.playback.take().unwrap();
        active.canceled.store(true, Ordering::Release);
        finish_playback(active)
    }
    pub(super) fn start(
        &mut self,
        session_id: u64,
        product_id: u32,
        options: CaptureOptions,
    ) -> anyhow::Result<CapturePoll> {
        options.validate()?;
        let lossless = matches!(
            options.purpose(),
            razer_device::seiren_microphone::CapturePurpose::Recording
                | razer_device::seiren_microphone::CapturePurpose::Speech
                | razer_device::seiren_microphone::CapturePurpose::Environment
        );
        self.launch_mode(
            session_id,
            product_id,
            lossless,
            move |canceled, publish| {
                #[cfg(windows)]
                {
                    super::native::seiren_audio_capture(canceled, &options, publish)
                }
                #[cfg(not(windows))]
                {
                    let _ = (canceled, publish, options);
                    anyhow::bail!(
                        "Local Seiren audio capture requires a platform PCM input adapter"
                    )
                }
            },
        )
    }
    #[cfg(test)]
    fn launch(
        &mut self,
        session_id: u64,
        product_id: u32,
        run: impl FnOnce(
            &AtomicBool,
            &mut dyn FnMut(CaptureFrame) -> anyhow::Result<()>,
        ) -> anyhow::Result<()>
        + Send
        + 'static,
    ) -> anyhow::Result<CapturePoll> {
        self.launch_mode(session_id, product_id, false, run)
    }
    fn launch_mode(
        &mut self,
        session_id: u64,
        product_id: u32,
        lossless: bool,
        run: impl FnOnce(
            &AtomicBool,
            &mut dyn FnMut(CaptureFrame) -> anyhow::Result<()>,
        ) -> anyhow::Result<()>
        + Send
        + 'static,
    ) -> anyhow::Result<CapturePoll> {
        capture::validate_owner(session_id, product_id)?;
        ensure!(
            self.active.is_none(),
            "Seiren audio capture already has an owner"
        );
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let observed = Arc::new(Mutex::new(Observed {
            lossless,
            ..Default::default()
        }));
        let samples = observed.clone();
        let worker = thread::Builder::new()
            .name("seiren-audio-capture".into())
            .spawn(move || {
                let mut publish = |frame| {
                    samples
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Seiren PCM observation lock is poisoned"))?
                        .record(frame)
                };
                run(&signal, &mut publish)
            })?;
        self.active = Some(Operation {
            session_id,
            product_id,
            canceled,
            observed,
            worker,
        });
        Ok(CapturePoll {
            session_id,
            product_id,
            started: false,
            terminal: false,
            canceled: false,
            error: None,
            frame: None,
        })
    }
    pub(super) fn poll(&mut self, session_id: u64) -> anyhow::Result<CapturePoll> {
        let active = self
            .active
            .as_ref()
            .context("Seiren audio capture owner is absent")?;
        ensure!(
            active.session_id == session_id,
            "Seiren audio capture owner ID differs"
        );
        if !active.worker.is_finished() {
            let frame = active
                .observed
                .lock()
                .map_err(|_| anyhow::anyhow!("Seiren PCM observation lock is poisoned"))?
                .take();
            return Ok(CapturePoll {
                session_id,
                product_id: active.product_id,
                started: frame.is_some(),
                terminal: false,
                canceled: active.canceled.load(Ordering::Acquire),
                error: None,
                frame,
            });
        }
        finish(self.active.take().unwrap())
    }
    pub(super) fn stop(&mut self, session_id: u64) -> anyhow::Result<CapturePoll> {
        let active = self
            .active
            .as_ref()
            .context("Seiren audio capture owner is absent")?;
        ensure!(
            active.session_id == session_id,
            "Seiren audio capture stop owner ID differs"
        );
        let active = self.active.take().unwrap();
        active.canceled.store(true, Ordering::Release);
        finish(active)
    }
}
fn finish_playback(active: PlaybackOperation) -> anyhow::Result<PlaybackPoll> {
    let result = active.worker.join();
    let error = match result {
        Ok(Ok(())) => None,
        Ok(Err(error)) if error.downcast_ref::<Canceled>().is_some() => None,
        Ok(Err(error)) => Some(format!("{error:#}")),
        Err(_) => Some("Seiren playback worker terminated".into()),
    };
    Ok(PlaybackPoll {
        session_id: active.session_id,
        product_id: active.product_id,
        started: active.started.load(Ordering::Acquire),
        terminal: true,
        canceled: active.canceled.load(Ordering::Acquire),
        error,
    })
}
fn finish(active: Operation) -> anyhow::Result<CapturePoll> {
    let result = active.worker.join();
    let error = match result {
        Ok(Ok(())) => None,
        Ok(Err(error)) if error.downcast_ref::<Canceled>().is_some() => None,
        Ok(Err(error)) => Some(format!("{error:#}")),
        Err(_) => Some("Seiren audio capture worker terminated".into()),
    };
    let frame = active
        .observed
        .lock()
        .map_err(|_| anyhow::anyhow!("Seiren PCM observation lock is poisoned"))?
        .take();
    Ok(CapturePoll {
        session_id: active.session_id,
        product_id: active.product_id,
        started: frame.is_some(),
        terminal: true,
        canceled: active.canceled.load(Ordering::Acquire),
        error,
        frame,
    })
}
impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(active) = &self.active {
            active.canceled.store(true, Ordering::Release);
        }
        if let Some(active) = &self.playback {
            active.canceled.store(true, Ordering::Release);
        }
        if let Some(active) = self.active.take() {
            active.canceled.store(true, Ordering::Release);
            match active.worker.join() {
                Ok(Err(error)) if error.downcast_ref::<Canceled>().is_none() => {
                    eprintln!("Seiren audio capture shutdown failed: {error:#}");
                }
                Err(_) => eprintln!("Seiren audio capture worker terminated during shutdown"),
                _ => {}
            }
        }
        if let Some(active) = self.playback.take() {
            if let Ok(reply) = finish_playback(active) {
                if let Some(error) = reply.error {
                    eprintln!("Seiren playback shutdown failed: {error}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observed_frame(pcm: Vec<f32>) -> CaptureFrame {
        CaptureFrame {
            output_sample_rate: Some(96_000),
            capture_sample_rate: 48_000,
            mono_pcm: pcm,
            microphone_levels: None,
            headphone_levels: None,
            endpoint_id: "simulated capture endpoint".into(),
            label: "Simulated Seiren".into(),
            default_fallback: false,
            diagnostics: vec![],
        }
    }
    #[test]
    fn pending_and_terminal_frames_keep_rates_samples_and_errors_separate() {
        let mut owner = Controller::default();
        let pending = owner
            .launch(7, 1422, |_, publish| {
                publish(observed_frame(vec![0.25, -0.5]))?;
                anyhow::bail!("simulated endpoint invalidated")
            })
            .unwrap();
        assert!(!pending.started && pending.frame.is_none());
        let active = owner.active.take().unwrap();
        let reply = finish(active).unwrap();
        assert!(reply.terminal && reply.started && !reply.canceled);
        assert!(reply.error.unwrap().contains("endpoint invalidated"));
        let frame = reply.frame.unwrap();
        assert_eq!(frame.mono_pcm, [0.25, -0.5]);
        assert_eq!(frame.output_sample_rate, Some(96_000));
        assert_eq!(frame.capture_sample_rate, 48_000);
        assert!(frame.microphone_levels.is_none() && frame.headphone_levels.is_none());
    }
    #[test]
    fn a_different_owner_cannot_stop_the_retained_capture_worker() {
        let mut owner = Controller::default();
        owner
            .launch(9, 1446, |canceled, _| {
                while !canceled.load(Ordering::Acquire) {
                    thread::yield_now();
                }
                Err(Canceled.into())
            })
            .unwrap();
        assert!(owner.stop(10).is_err());
        let reply = owner.stop(9).unwrap();
        assert!(reply.terminal && reply.canceled && !reply.started);
        assert!(reply.error.is_none());
        assert!(owner.active.is_none());
    }
    #[test]
    fn recording_overflow_fails_without_replacing_unread_pcm() {
        let mut observed = Observed {
            lossless: true,
            ..Default::default()
        };
        observed
            .record(observed_frame(vec![0.25; MAX_PENDING_SAMPLES]))
            .unwrap();
        assert!(
            observed
                .record(observed_frame(vec![0.5]))
                .unwrap_err()
                .to_string()
                .contains("retain all unread PCM")
        );
        let frame = observed.take().unwrap();
        assert_eq!(frame.mono_pcm, vec![0.25; MAX_PENDING_SAMPLES]);
        assert!(frame.diagnostics.is_empty());
    }
}

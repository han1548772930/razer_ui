//! UI9374 local recorded PCM -> return-switch handshake -> WASAPI playback.
//! Owned files outlive service Stop/Shutdown and are removed after its join.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::{
    seiren_eq::Change,
    seiren_microphone::{PcmPlaybackFile, PlaybackPoll},
};
use razer_discovery::{
    direct,
    discovery::{ObservedDevice, ObservedTransport},
};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::SeirenAudioPlaybackRequested;
use std::{
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) struct Worker {
    cancellation: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    _delivery: Task<()>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.cancellation.store(true, Ordering::Release);
        }
    }
}
enum Update {
    Started,
    Finished(Option<String>),
}

struct RecordingFile(PathBuf);
impl RecordingFile {
    fn write(request: &SeirenAudioPlaybackRequested) -> anyhow::Result<(Self, PcmPlaybackFile)> {
        let directory = std::env::temp_dir().join("razer-ui-microphone");
        std::fs::create_dir_all(&directory)?;
        let path = directory.join(format!(
            "{}-{}.f32",
            std::process::id(),
            request.session_id()
        ));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let owner = Self(path);
        let mut out = std::io::BufWriter::new(file);
        for packet in request.clip().samples().chunks(4096) {
            if request.is_canceled() {
                anyhow::bail!("Microphone playback canceled while preparing actual PCM");
            }
            for sample in packet {
                out.write_all(&sample.to_le_bytes())?;
            }
        }
        out.flush()?;
        drop(out);
        let metadata = PcmPlaybackFile::new(
            owner
                .0
                .to_str()
                .context("Recording path is not UTF-8")?
                .into(),
            request.clip().sample_rate(),
            request.clip().samples().len() as u64,
        )?;
        Ok((owner, metadata))
    }
}
impl Drop for RecordingFile {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.0) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("Microphone recorded PCM cleanup: {error}");
            }
        }
    }
}
fn decode(
    value: serde_json::Value,
    request: &SeirenAudioPlaybackRequested,
) -> anyhow::Result<PlaybackPoll> {
    let poll: PlaybackPoll = serde_json::from_value(value)?;
    ensure!(
        poll.session_id == request.session_id() && poll.product_id == request.product_id(),
        "Microphone playback reply differs from its page owner"
    );
    Ok(poll)
}
fn run(
    request: &SeirenAudioPlaybackRequested,
    route: &ObservedDevice,
    updates: &Sender<Update>,
) -> anyhow::Result<()> {
    if request.is_canceled() {
        return Ok(());
    }
    let (_file, clip) = RecordingFile::write(request)?;
    let mut client = ServiceClient::spawn()?;
    let mut dispatched = false;
    let mut return_attempted = false;
    let mut diagnostics = Vec::new();
    let result = (|| -> anyhow::Result<()> {
        let direct = direct::seiren_eq::resolve(&mut client, route)?;
        if request.is_canceled() {
            return Ok(());
        }
        return_attempted = true;
        // Source postExecute publishes the return state even after a negative
        // firmware ACK. Playback still proceeds after its wait; preserve the
        // actual ACK as a diagnostic, never infer that firmware routing worked.
        match direct.seiren_eq(&mut client, Change::ReturnSwitch(true)) {
            Ok(report) if report.confirmed() => {}
            Ok(report) => diagnostics.push(format!(
                "Microphone return switch was not confirmed: {:?}",
                report.stages()
            )),
            Err(error) => diagnostics.push(format!(
                "Microphone return switch request failed: {error:#}"
            )),
        }
        let wait = Instant::now();
        while wait.elapsed() < Duration::from_millis(500) {
            if request.is_canceled() {
                return Ok(());
            }
            thread::park_timeout(Duration::from_millis(5));
        }
        if request.is_canceled() {
            return Ok(());
        }
        dispatched = true;
        let reply = decode(
            client.request(ServiceRequest::SeirenAudioPlaybackStart {
                session_id: request.session_id(),
                product_id: request.product_id(),
                clip,
            })?,
            request,
        )?;
        if let Some(error) = reply.error {
            anyhow::bail!("Microphone playback initialization failed: {error}");
        }
        ensure!(
            !reply.terminal,
            "Microphone playback ended before its actual start"
        );
        let mut reported = false;
        loop {
            if request.is_canceled() {
                break;
            }
            let reply = decode(
                client.request(ServiceRequest::SeirenAudioPlaybackPoll {
                    session_id: request.session_id(),
                })?,
                request,
            )?;
            if reply.terminal {
                dispatched = false;
            }
            if reply.started && !reported {
                updates
                    .send(Update::Started)
                    .context("Microphone playback page was released")?;
                reported = true;
            }
            if let Some(error) = reply.error {
                anyhow::bail!("Microphone playback failed: {error}");
            }
            if reply.terminal {
                break;
            }
            thread::park_timeout(Duration::from_millis(16));
        }
        Ok(())
    })();
    let stopped = if dispatched {
        (|| -> anyhow::Result<()> {
            let reply = decode(
                client.request(ServiceRequest::SeirenAudioPlaybackStop {
                    session_id: request.session_id(),
                })?,
                request,
            )?;
            ensure!(
                reply.terminal,
                "Microphone playback Stop did not confirm its worker join"
            );
            if let Some(error) = reply.error {
                anyhow::bail!("Microphone playback cleanup failed: {error}");
            }
            Ok(())
        })()
    } else {
        Ok(())
    };
    // Cancellation never prevents return-switch cleanup. Resolve the retained
    // physical identity again; a foreign/reconnected device cannot be written.
    let returned = if return_attempted {
        (|| -> anyhow::Result<()> {
            let direct = direct::seiren_eq::resolve(&mut client, route)?;
            let report = direct.seiren_eq(&mut client, Change::ReturnSwitch(false))?;
            ensure!(
                report.confirmed(),
                "Microphone return-switch reset was not confirmed"
            );
            Ok(())
        })()
    } else {
        Ok(())
    };
    let shutdown = client.request(ServiceRequest::Shutdown).and_then(|value| {
        ensure!(
            value["stopped"] == true,
            "Microphone playback service shutdown was not confirmed"
        );
        Ok(())
    });
    diagnostics.extend(
        [result, stopped, returned, shutdown]
            .into_iter()
            .filter_map(Result::err)
            .map(|error| format!("{error:#}")),
    );
    ensure!(diagnostics.is_empty(), "{}", diagnostics.join("; "));
    Ok(())
}

impl AppShell {
    pub(super) fn install_seiren_audio_playback_cleanup(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            let workers = std::mem::take(&mut this.seiren_audio_playbacks);
            for worker in workers.values() {
                worker.cancellation.store(true, Ordering::Release);
            }
            let cleanup = cx.background_executor().spawn(async move {
                for (_, mut worker) in workers {
                    if let Some(thread) = worker.thread.take() {
                        let _ = thread.join();
                    }
                }
            });
            async move { cleanup.await }
        }));
    }
    pub(super) fn cancel_seiren_audio_playbacks(&mut self) {
        for worker in self.seiren_audio_playbacks.values() {
            worker.cancellation.store(true, Ordering::Release);
        }
    }
    pub(super) fn start_seiren_audio_playback(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: SeirenAudioPlaybackRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !workspace
                .read(cx)
                .seiren_audio_playback_request_matches(&request, cx)
        {
            request.cancellation().store(true, Ordering::Release);
            workspace.update(cx, |workspace, cx| {
                workspace.finish_seiren_audio_playback(&request, None, cx)
            });
            return;
        }
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && route.product_id() == request.product_id()
                    && route.product_id() == route.physical_product_id()
                    && route.peer_product_id().is_none()
                    && matches!(route.transport(), Some(ObservedTransport::Wired))
            })
            .cloned()
            .collect::<Vec<_>>();
        if routes.len() != 1 {
            workspace.update(cx,|workspace,cx|workspace.finish_seiren_audio_playback(&request,Some("Microphone playback requires its uniquely observed local wired device for the firmware return switch".into()),cx));
            return;
        }
        let route = routes[0].clone();
        let id = request.session_id();
        if self.seiren_audio_playbacks.contains_key(&id) {
            return;
        }
        let (updates, received) = mpsc::channel();
        let dispatched = request.clone();
        let thread = match thread::Builder::new()
            .name("razer-seiren-playback".into())
            .spawn(move || {
                let error = run(&dispatched, &route, &updates)
                    .err()
                    .map(|error| format!("{error:#}"));
                let _ = updates.send(Update::Finished(error));
            }) {
            Ok(thread) => thread,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_seiren_audio_playback(&request, Some(error.to_string()), cx)
                });
                return;
            }
        };
        let owner = workspace.downgrade();
        let cancellation = request.cancellation();
        let signal = cancellation.clone();
        let delivery = cx.spawn_in(window, async move |shell, cx| {
            let mut finished = None;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let mut started = false;
                loop {
                    match received.try_recv() {
                        Ok(Update::Started) => started = true,
                        Ok(Update::Finished(error)) => finished = Some(error),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            if finished.is_none() {
                                finished = Some(Some(
                                    "Microphone playback worker ended without a cleanup result"
                                        .into(),
                                ));
                            }
                            break;
                        }
                    }
                }
                let alive = shell.update_in(cx, |shell, _, cx| {
                    if let Some(workspace) = owner.upgrade() {
                        if started {
                            workspace.update(cx, |workspace, cx| {
                                workspace.observe_seiren_audio_playback_started(&request, cx)
                            });
                        }
                    } else {
                        signal.store(true, Ordering::Release);
                    }
                    if finished.is_none()
                        || !shell.seiren_audio_playbacks.get(&id).is_some_and(|worker| {
                            worker.thread.as_ref().is_some_and(JoinHandle::is_finished)
                        })
                    {
                        return true;
                    }
                    let mut error = finished.take().flatten();
                    if let Some(mut worker) = shell.seiren_audio_playbacks.remove(&id) {
                        if worker
                            .thread
                            .take()
                            .is_some_and(|thread| thread.join().is_err())
                        {
                            error = Some("Microphone playback worker panicked".into());
                        }
                    }
                    if let Some(error) = &error {
                        eprintln!("Microphone playback: {error}");
                    }
                    if let Some(workspace) = owner.upgrade() {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_seiren_audio_playback(&request, error, cx)
                        });
                    }
                    if shell.close_requested && shell.save_task.is_none() {
                        shell.continue_save_queue(true, cx);
                    }
                    false
                });
                if !matches!(alive, Ok(true)) {
                    signal.store(true, Ordering::Release);
                    break;
                }
            }
        });
        self.seiren_audio_playbacks.insert(
            id,
            Worker {
                cancellation,
                thread: Some(thread),
                _delivery: delivery,
            },
        );
    }
}

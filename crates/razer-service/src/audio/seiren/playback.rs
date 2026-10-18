//! UI9374 recorded PCM, firmware return switch, WASAPI playback and final release.
use super::{Owner, Session, guarded, shutdown};
use anyhow::{Context as _, ensure};
use razer_device::{
    seiren_eq::Change,
    seiren_microphone::{PcmClip, PcmPlaybackFile, PlaybackPoll},
};
use razer_discovery::{
    direct,
    discovery::{ObservedDevice, ObservedTransport},
};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_storage::audio_recording::RecordingFile;
use std::{
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

pub struct Request {
    owner: Owner,
    clip: Arc<PcmClip>,
    route: ObservedDevice,
}
impl Request {
    pub fn new(
        session_id: u64,
        product_id: u32,
        clip: Arc<PcmClip>,
        route: ObservedDevice,
        canceled: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        ensure!(
            route.product_id() == product_id
                && route.physical_product_id() == product_id
                && route.peer_product_id().is_none()
                && matches!(route.transport(), Some(ObservedTransport::Wired)),
            "Microphone playback requires its observed local wired device"
        );
        Ok(Self {
            owner: Owner::new(session_id, product_id, canceled)?,
            clip,
            route,
        })
    }
}

pub enum Update {
    Started,
    Finished(Option<String>),
}

pub fn spawn(request: Request) -> anyhow::Result<(Session, Receiver<Update>)> {
    let (updates, received) = mpsc::channel();
    let canceled = request.owner.canceled.clone();
    let session = Session::spawn("razer-seiren-playback", canceled, move || {
        let result = guarded(|| run(&request, &updates));
        let _ = updates.send(Update::Finished(
            result.err().map(|error| format!("{error:#}")),
        ));
    })?;
    Ok((session, received))
}

trait Transport {
    fn prepare(&mut self) -> anyhow::Result<()>;
    fn prepare_restore(&mut self) -> anyhow::Result<()>;
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<serde_json::Value>;
    fn return_switch(&mut self, enabled: bool) -> anyhow::Result<()>;
}
struct LocalTransport<'a> {
    client: ServiceClient,
    route: &'a ObservedDevice,
    prepared: Option<direct::DirectRoute>,
}
impl Transport for LocalTransport<'_> {
    fn prepare(&mut self) -> anyhow::Result<()> {
        self.prepared = Some(direct::seiren_eq::resolve(&mut self.client, self.route)?);
        Ok(())
    }
    fn prepare_restore(&mut self) -> anyhow::Result<()> {
        if self.client.is_stopped() {
            // A fatal audio reply closes this IPC client. Confirm its process
            // and pipes released before restoring through a fresh connection.
            self.client
                .finish_termination()
                .context("Microphone playback service termination before route restore")?;
            self.client = ServiceClient::spawn()
                .context("Microphone playback route-restore service connection")?;
        }
        Ok(())
    }
    fn request(&mut self, request: ServiceRequest) -> anyhow::Result<serde_json::Value> {
        self.client.request(request)
    }
    fn return_switch(&mut self, enabled: bool) -> anyhow::Result<()> {
        // Resolve the retained physical identity again for restoration; a
        // missing, ambiguous or different endpoint must not receive this write.
        let direct = if enabled {
            self.prepared
                .take()
                .context("Microphone playback route was not prepared")?
        } else {
            direct::seiren_eq::resolve(&mut self.client, self.route)?
        };
        let report = direct.seiren_eq(&mut self.client, Change::ReturnSwitch(enabled))?;
        ensure!(
            report.confirmed(),
            "Microphone return-switch {enabled} was not confirmed: {:?}",
            report.stages()
        );
        Ok(())
    }
}

fn run(request: &Request, updates: &Sender<Update>) -> anyhow::Result<()> {
    if request.owner.is_canceled() {
        return Ok(());
    }
    let file = RecordingFile::write(request.clip.samples(), || request.owner.is_canceled())?;
    let result = guarded(|| {
        let clip = PcmPlaybackFile::new(
            file.path()
                .to_str()
                .context("Recording path is not UTF-8")?
                .into(),
            request.clip.sample_rate(),
            request.clip.samples().len() as u64,
        )?;
        let mut transport = LocalTransport {
            client: ServiceClient::spawn()?,
            route: &request.route,
            prepared: None,
        };
        run_with_transport(&request.owner, clip, updates, &mut transport, |duration| {
            let start = Instant::now();
            while start.elapsed() < duration && !request.owner.is_canceled() {
                thread::park_timeout(
                    Duration::from_millis(5).min(duration.saturating_sub(start.elapsed())),
                );
            }
        })
    });
    // The client and its Stop/Shutdown acknowledgement release before the file.
    let cleanup = file.cleanup();
    let diagnostics = [result, cleanup]
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    ensure!(diagnostics.is_empty(), "{}", diagnostics.join("; "));
    Ok(())
}

fn decode(value: serde_json::Value, owner: &Owner) -> anyhow::Result<PlaybackPoll> {
    let poll: PlaybackPoll = serde_json::from_value(value)?;
    ensure!(
        poll.session_id == owner.session_id && poll.product_id == owner.product_id,
        "Microphone playback reply differs from its page owner"
    );
    Ok(poll)
}

fn run_with_transport(
    owner: &Owner,
    clip: PcmPlaybackFile,
    updates: &Sender<Update>,
    transport: &mut impl Transport,
    mut wait: impl FnMut(Duration),
) -> anyhow::Result<()> {
    let mut dispatched = false;
    let mut return_attempted = false;
    let mut diagnostics = Vec::new();
    let result = guarded(|| {
        transport.prepare()?;
        if owner.is_canceled() {
            return Ok(());
        }
        return_attempted = true;
        // Source postExecute publishes return state after a negative ACK too.
        // Preserve that error separately while playback follows its real wait.
        if let Err(error) = transport.return_switch(true) {
            diagnostics.push(format!(
                "Microphone return switch request failed: {error:#}"
            ));
        }
        wait(Duration::from_millis(500));
        if owner.is_canceled() {
            return Ok(());
        }
        dispatched = true;
        let reply = decode(
            transport.request(ServiceRequest::SeirenAudioPlaybackStart {
                session_id: owner.session_id,
                product_id: owner.product_id,
                clip,
            })?,
            owner,
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
            if owner.is_canceled() {
                break;
            }
            let reply = decode(
                transport.request(ServiceRequest::SeirenAudioPlaybackPoll {
                    session_id: owner.session_id,
                })?,
                owner,
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
            wait(Duration::from_millis(16));
        }
        Ok(())
    });
    let stopped = if dispatched {
        guarded(|| {
            let reply = decode(
                transport.request(ServiceRequest::SeirenAudioPlaybackStop {
                    session_id: owner.session_id,
                })?,
                owner,
            )?;
            ensure!(
                reply.terminal,
                "Microphone playback Stop did not confirm its worker join"
            );
            if let Some(error) = reply.error {
                anyhow::bail!("Microphone playback cleanup failed: {error}");
            }
            Ok(())
        })
    } else {
        Ok(())
    };
    let returned = if return_attempted {
        guarded(|| {
            transport.prepare_restore()?;
            transport.return_switch(false)
        })
    } else {
        Ok(())
    };
    let shutdown = shutdown(|request| transport.request(request));
    diagnostics.extend(
        [result, stopped, returned, shutdown]
            .into_iter()
            .filter_map(Result::err)
            .map(|error| format!("{error:#}")),
    );
    ensure!(diagnostics.is_empty(), "{}", diagnostics.join("; "));
    Ok(())
}

#[cfg(test)]
mod tests;

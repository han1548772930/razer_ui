//! The page owns activity; this worker owns the local service and its Stop /
//! Shutdown acknowledgements. UI cancellation never drops cleanup intent.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::seiren_audio_capture::{CaptureFrame, CapturePoll};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::SeirenAudioCaptureRequested;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
    time::Duration,
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
    Frame(CaptureFrame),
    Finished(Option<String>),
}

struct CaptureOwner {
    session_id: u64,
    product_id: u32,
    canceled: Arc<AtomicBool>,
    options: razer_device::seiren_microphone::CaptureOptions,
}
impl CaptureOwner {
    fn session_id(&self) -> u64 {
        self.session_id
    }
    fn product_id(&self) -> u32 {
        self.product_id
    }
    fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }
}

fn decode(value: serde_json::Value, request: &CaptureOwner) -> anyhow::Result<CapturePoll> {
    let poll: CapturePoll = serde_json::from_value(value)
        .context("Seiren audio capture service reply has an invalid shape")?;
    ensure!(
        poll.session_id == request.session_id() && poll.product_id == request.product_id(),
        "Seiren audio capture reply differs from its page owner"
    );
    ensure!(
        poll.frame.is_none() || poll.started,
        "Seiren audio samples arrived before capture initialization"
    );
    if let Some(frame) = &poll.frame {
        ensure!(
            frame.capture_sample_rate > 0,
            "Seiren audio capture rate is zero"
        );
        ensure!(
            frame.output_sample_rate.is_none_or(|rate| rate > 0),
            "Seiren default output sample rate is zero"
        );
        ensure!(
            frame.mono_pcm.iter().all(|sample| sample.is_finite())
                && frame
                    .microphone_levels
                    .iter()
                    .flatten()
                    .all(|level| level.is_finite())
                && frame
                    .headphone_levels
                    .iter()
                    .flatten()
                    .all(|level| level.is_finite()),
            "Seiren audio capture reply contains non-finite observations"
        );
    }
    Ok(poll)
}

fn run(request: &SeirenAudioCaptureRequested, updates: &Sender<Update>) -> anyhow::Result<()> {
    if request.is_canceled() {
        return Ok(());
    }
    let mut client = ServiceClient::spawn()?;
    let owner = CaptureOwner {
        session_id: request.session_id(),
        product_id: request.product_id(),
        canceled: request.cancellation(),
        options: request.options().clone(),
    };
    run_with_dispatch(
        &owner,
        updates,
        |request| client.request(request),
        || thread::sleep(Duration::from_millis(16)),
    )
}

fn run_with_dispatch(
    request: &CaptureOwner,
    updates: &Sender<Update>,
    mut dispatch: impl FnMut(ServiceRequest) -> anyhow::Result<serde_json::Value>,
    mut wait: impl FnMut(),
) -> anyhow::Result<()> {
    // Even an invalid initialization reply may follow a created capture.
    // Retain the owner before dispatch, then always ask the service to clean it.
    let mut dispatched = false;
    let result = (|| {
        if request.is_canceled() {
            return Ok(());
        }
        dispatched = true;
        let started = decode(
            dispatch(ServiceRequest::SeirenAudioCaptureStart {
                session_id: request.session_id(),
                product_id: request.product_id(),
                options: request.options.clone(),
            })?,
            request,
        )?;
        if let Some(error) = started.error {
            anyhow::bail!("Seiren audio capture initialization failed: {error}");
        }
        ensure!(
            !started.terminal || request.is_canceled(),
            "Seiren audio capture ended during initialization"
        );
        if let Some(frame) = started.frame {
            if !request.is_canceled() {
                updates
                    .send(Update::Frame(frame))
                    .context("Seiren audio page owner was released")?;
            }
        }
        loop {
            if request.is_canceled() {
                break;
            }
            wait();
            if request.is_canceled() {
                break;
            }
            let reply = decode(
                dispatch(ServiceRequest::SeirenAudioCapturePoll {
                    session_id: request.session_id(),
                })?,
                request,
            )?;
            // A terminal Poll is the service worker's completed join/cleanup
            // acknowledgement. Its controller has already released this ID.
            if reply.terminal {
                dispatched = false;
            }
            if let Some(error) = reply.error {
                anyhow::bail!("Seiren audio capture failed: {error}");
            }
            if !request.is_canceled() {
                if let Some(frame) = reply.frame {
                    updates
                        .send(Update::Frame(frame))
                        .context("Seiren audio page owner was released")?;
                }
            }
            if reply.terminal {
                ensure!(
                    request.is_canceled(),
                    "Seiren audio capture stopped without page cancellation"
                );
                break;
            }
        }
        Ok::<_, anyhow::Error>(())
    })();
    let stopped = if dispatched {
        (|| {
            let reply = decode(
                dispatch(ServiceRequest::SeirenAudioCaptureStop {
                    session_id: request.session_id(),
                })?,
                request,
            )?;
            ensure!(
                reply.terminal,
                "Seiren audio capture Stop did not confirm cleanup"
            );
            // A normal recording stop still owns its final drain until the
            // service has joined. Lifecycle cancellation is rejected by the
            // page's generation guard, rather than silently losing final PCM.
            if let Some(frame) = reply.frame {
                let _ = updates.send(Update::Frame(frame));
            }
            if let Some(error) = reply.error {
                anyhow::bail!("Seiren audio capture cleanup failed: {error}");
            }
            Ok::<_, anyhow::Error>(())
        })()
    } else {
        Ok(())
    };
    let shutdown = dispatch(ServiceRequest::Shutdown).and_then(|reply| {
        ensure!(
            reply["stopped"] == true,
            "Seiren audio service shutdown was not confirmed"
        );
        Ok(())
    });
    // Preserve cleanup failures even if the device or initialization also failed.
    let errors = [result, stopped, shutdown]
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(())
}

impl AppShell {
    pub(super) fn install_seiren_audio_capture_cleanup(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.subscriptions
            .push(cx.observe_window_visibility(window, |this, _, window, cx| {
                let _ = this.observe_seiren_audio_host_visibility(window, cx);
            }));
        let _ = self.observe_seiren_audio_host_visibility(window, cx);
        self.subscriptions.push(cx.on_app_quit(|this, cx| {
            let workers = std::mem::take(&mut this.seiren_audio_captures);
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

    pub(super) fn observe_seiren_audio_host_visibility(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let workspaces = self
            .devices
            .iter()
            .filter(|workspace| matches!(workspace.read(cx).device(cx).product_id, 1422 | 1446))
            .cloned()
            .collect::<Vec<_>>();
        if workspaces.is_empty() {
            return Ok(());
        }
        let status = host_tabs::host_window::status(window)?;
        let visible = status.is_some_and(|status| {
            status.is_visible == Some(true) && status.is_minimized != Some(true)
        });
        for workspace in workspaces {
            workspace.update(cx, |workspace, cx| {
                workspace.observe_seiren_host_visibility(visible, cx)
            });
        }
        Ok(())
    }

    pub(super) fn cancel_seiren_audio_captures(&mut self, cx: &mut Context<Self>) {
        for workspace in self.devices.clone() {
            workspace.update(cx, |workspace, cx| {
                workspace.cancel_seiren_audio_capture(cx)
            });
        }
        for worker in self.seiren_audio_captures.values() {
            worker.cancellation.store(true, Ordering::Release);
        }
    }

    pub(super) fn start_seiren_audio_capture(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: SeirenAudioCaptureRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.observe_seiren_audio_host_visibility(window, cx) {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_seiren_audio_capture(&request, Some(format!("{error:#}")), cx)
            });
            return;
        }
        if self.close_requested
            || !workspace
                .read(cx)
                .seiren_audio_capture_request_matches(&request, cx)
        {
            request.cancellation().store(true, Ordering::Release);
            workspace.update(cx, |workspace, cx| {
                workspace.finish_seiren_audio_capture(&request, None, cx)
            });
            return;
        }
        let id = request.session_id();
        if self.seiren_audio_captures.contains_key(&id) {
            return;
        }
        let (updates, received) = mpsc::channel();
        let dispatched = request.clone();
        let thread = match thread::Builder::new()
            .name("razer-seiren-analyser".into())
            .spawn(move || {
                let error = run(&dispatched, &updates)
                    .err()
                    .map(|error| format!("{error:#}"));
                let _ = updates.send(Update::Finished(error));
            }) {
            Ok(thread) => thread,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_seiren_audio_capture(&request, Some(error.to_string()), cx)
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
                let mut frames = Vec::new();
                loop {
                    match received.try_recv() {
                        Ok(Update::Frame(frame)) => frames.push(frame),
                        Ok(Update::Finished(error)) => finished = Some(error),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            if finished.is_none() {
                                finished = Some(Some(
                                    "Seiren audio capture worker ended without a cleanup result"
                                        .into(),
                                ));
                            }
                            break;
                        }
                    }
                }
                let alive = shell.update_in(cx, |shell, _, cx| {
                    if let Some(workspace) = owner.upgrade() {
                        for frame in frames {
                            if !workspace.update(cx, |workspace, cx| {
                                workspace.observe_seiren_audio_capture(&request, frame, cx)
                            }) {
                                signal.store(true, Ordering::Release);
                            }
                        }
                    } else {
                        signal.store(true, Ordering::Release);
                    }
                    let released = finished.is_some()
                        && shell.seiren_audio_captures.get(&id).is_some_and(|worker| {
                            worker
                                .thread
                                .as_ref()
                                .is_some_and(|thread| thread.is_finished())
                        });
                    if !released {
                        return true;
                    }
                    let mut error = finished.take().flatten();
                    if let Some(mut worker) = shell.seiren_audio_captures.remove(&id) {
                        if worker
                            .thread
                            .take()
                            .is_some_and(|thread| thread.join().is_err())
                        {
                            error = Some("Seiren audio capture worker panicked".into());
                        }
                    }
                    if let Some(error) = &error {
                        eprintln!("Seiren audio capture: {error}");
                    }
                    if let Some(workspace) = owner.upgrade() {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_seiren_audio_capture(&request, error, cx)
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
        self.seiren_audio_captures.insert(
            id,
            Worker {
                cancellation,
                thread: Some(thread),
                _delivery: delivery,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> CaptureOwner {
        CaptureOwner {
            session_id: 23,
            product_id: 1422,
            canceled: Arc::new(AtomicBool::new(false)),
            options: Default::default(),
        }
    }
    fn reply(owner: &CaptureOwner, terminal: bool) -> serde_json::Value {
        serde_json::to_value(CapturePoll {
            session_id: owner.session_id,
            product_id: owner.product_id,
            started: !terminal,
            terminal,
            canceled: terminal,
            error: None,
            frame: None,
        })
        .unwrap()
    }

    #[::core::prelude::v1::test]
    fn invalid_start_reply_still_stops_the_original_session_and_shuts_down() {
        let owner = owner();
        let (updates, _) = mpsc::channel();
        let mut calls = Vec::new();
        let result = run_with_dispatch(
            &owner,
            &updates,
            |request| match request {
                ServiceRequest::SeirenAudioCaptureStart {
                    session_id,
                    product_id,
                    options: _,
                } => {
                    assert_eq!((session_id, product_id), (23, 1422));
                    calls.push("start");
                    Ok(serde_json::json!({"session_id":999,"product_id":1422}))
                }
                ServiceRequest::SeirenAudioCaptureStop { session_id } => {
                    assert_eq!(session_id, 23);
                    calls.push("stop");
                    Ok(reply(&owner, true))
                }
                ServiceRequest::Shutdown => {
                    calls.push("shutdown");
                    Ok(serde_json::json!({"stopped":true}))
                }
                _ => panic!("invalid initialization must not poll"),
            },
            || panic!("invalid initialization must not wait"),
        );
        assert!(result.is_err());
        assert_eq!(calls, ["start", "stop", "shutdown"]);
    }

    #[::core::prelude::v1::test]
    fn cancel_during_initialization_retains_cleanup_errors_and_shutdown() {
        let owner = owner();
        let (updates, _) = mpsc::channel();
        let mut calls = Vec::new();
        let result = run_with_dispatch(
            &owner,
            &updates,
            |request| match request {
                ServiceRequest::SeirenAudioCaptureStart { .. } => {
                    calls.push("start");
                    owner.canceled.store(true, Ordering::Release);
                    Ok(reply(&owner, false))
                }
                ServiceRequest::SeirenAudioCaptureStop { .. } => {
                    calls.push("stop");
                    let mut response = reply(&owner, true);
                    response["error"] = serde_json::json!("COM release failed");
                    Ok(response)
                }
                ServiceRequest::Shutdown => {
                    calls.push("shutdown");
                    Ok(serde_json::json!({"stopped":true}))
                }
                _ => panic!("canceled initialization must not poll"),
            },
            || panic!("canceled initialization must not wait"),
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("COM release failed")
        );
        assert_eq!(calls, ["start", "stop", "shutdown"]);
    }

    #[::core::prelude::v1::test]
    fn terminal_poll_already_acknowledges_worker_cleanup() {
        let owner = owner();
        let (updates, _) = mpsc::channel();
        let mut calls = Vec::new();
        let result = run_with_dispatch(
            &owner,
            &updates,
            |request| match request {
                ServiceRequest::SeirenAudioCaptureStart { .. } => {
                    calls.push("start");
                    Ok(reply(&owner, false))
                }
                ServiceRequest::SeirenAudioCapturePoll { .. } => {
                    calls.push("poll");
                    let mut response = reply(&owner, true);
                    response["error"] = serde_json::json!("Endpoint invalidated");
                    Ok(response)
                }
                ServiceRequest::Shutdown => {
                    calls.push("shutdown");
                    Ok(serde_json::json!({"stopped":true}))
                }
                _ => panic!("terminal Poll has released its controller owner"),
            },
            || {},
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Endpoint invalidated")
        );
        assert_eq!(calls, ["start", "poll", "shutdown"]);
    }

    #[::core::prelude::v1::test]
    fn foreign_scope_or_samples_before_ready_are_rejected() {
        let owner = owner();
        let mut foreign = reply(&owner, false);
        foreign["session_id"] = serde_json::json!(24);
        assert!(decode(foreign, &owner).is_err());
        let invalid = CapturePoll {
            session_id: 23,
            product_id: 1422,
            started: false,
            terminal: false,
            canceled: false,
            error: None,
            frame: Some(CaptureFrame {
                output_sample_rate: Some(48_000),
                capture_sample_rate: 96_000,
                mono_pcm: vec![0.1],
                microphone_levels: None,
                headphone_levels: None,
                endpoint_id: "mock-endpoint".into(),
                label: "Seiren simulated endpoint".into(),
                default_fallback: false,
                diagnostics: Vec::new(),
            }),
        };
        assert!(decode(serde_json::to_value(invalid).unwrap(), &owner).is_err());
    }
}

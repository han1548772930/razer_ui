//! Page activity and service observations stay with their GUI owners.
//! Native capture, IPC and final drain belong to the Seiren service worker.
use super::*;
use razer_pages::features::SeirenAudioCaptureRequested;
use razer_service::seiren_audio::{
    self,
    capture::{self, Update},
};
use std::{
    sync::{atomic::Ordering, mpsc},
    time::Duration,
};

pub(super) struct Worker {
    session: seiren_audio::Session,
    _delivery: Task<()>,
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
        self.register_exit_cleanup(|this, cx| {
            let workers = std::mem::take(&mut this.seiren_audio_captures);
            for worker in workers.values() {
                worker.session.cancel();
            }
            let cleanup = cx.background_executor().spawn(async move {
                for (_, worker) in workers {
                    if let Err(error) = worker.session.join() {
                        eprintln!("Seiren audio capture cleanup: {error:#}");
                    }
                }
            });
            async move { cleanup.await }
        });
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
            worker.session.cancel();
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
        let work = capture::Request::new(
            request.session_id(),
            request.product_id(),
            request.options().clone(),
            request.cancellation(),
        );
        let (session, received) = match work.and_then(capture::spawn) {
            Ok(worker) => worker,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_seiren_audio_capture(&request, Some(format!("{error:#}")), cx)
                });
                return;
            }
        };
        let owner = workspace.downgrade();
        let signal = request.cancellation();
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
                        && shell
                            .seiren_audio_captures
                            .get(&id)
                            .is_some_and(|worker| worker.session.is_finished());
                    if !released {
                        return true;
                    }
                    let mut error = finished.take().flatten();
                    if let Some(worker) = shell.seiren_audio_captures.remove(&id) {
                        if let Err(cleanup) = worker.session.join() {
                            error = Some(match error {
                                Some(error) => format!("{error}; {cleanup:#}"),
                                None => format!("{cleanup:#}"),
                            });
                        }
                    }
                    if let Some(error) = &error {
                        eprintln!("Seiren audio capture: {error}");
                    }
                    let succeeded = error.is_none();
                    if shell.close_requested
                        && let Some(error) = &error
                    {
                        shell.status = format!("麦克风录音清理未完成：{error}");
                    }
                    if let Some(workspace) = owner.upgrade() {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_seiren_audio_capture(&request, error, cx)
                        });
                    }
                    if shell.close_requested {
                        shell.continue_save_queue(succeeded, cx);
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
                session,
                _delivery: delivery,
            },
        );
    }
}

//! The shell connects the retained page to its owned playback service.
//! PCM files, firmware routing and native release stay with that service.
use super::*;
use razer_discovery::discovery::ObservedTransport;
use razer_pages::features::SeirenAudioPlaybackRequested;
use razer_service::seiren_audio::{
    self,
    playback::{self, Update},
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
    pub(super) fn install_seiren_audio_playback_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|this, cx| {
            let workers = std::mem::take(&mut this.seiren_audio_playbacks);
            for worker in workers.values() {
                worker.session.cancel();
            }
            let cleanup = cx.background_executor().spawn(async move {
                for (_, worker) in workers {
                    if let Err(error) = worker.session.join() {
                        eprintln!("Microphone playback cleanup: {error:#}");
                    }
                }
            });
            async move { cleanup.await }
        });
    }
    pub(super) fn cancel_seiren_audio_playbacks(&mut self) {
        for worker in self.seiren_audio_playbacks.values() {
            worker.session.cancel();
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
        let work = playback::Request::new(
            request.session_id(),
            request.product_id(),
            request.clip().clone(),
            route,
            request.cancellation(),
        );
        let (session, received) = match work.and_then(playback::spawn) {
            Ok(worker) => worker,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_seiren_audio_playback(&request, Some(format!("{error:#}")), cx)
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
                        || !shell
                            .seiren_audio_playbacks
                            .get(&id)
                            .is_some_and(|worker| worker.session.is_finished())
                    {
                        return true;
                    }
                    let mut error = finished.take().flatten();
                    if let Some(worker) = shell.seiren_audio_playbacks.remove(&id) {
                        if let Err(cleanup) = worker.session.join() {
                            error = Some(match error {
                                Some(error) => format!("{error}; {cleanup:#}"),
                                None => format!("{cleanup:#}"),
                            });
                        }
                    }
                    if let Some(error) = &error {
                        eprintln!("Microphone playback: {error}");
                    }
                    let succeeded = error.is_none();
                    if shell.close_requested
                        && let Some(error) = &error
                    {
                        shell.status = format!("麦克风回放清理未完成：{error}");
                    }
                    if let Some(workspace) = owner.upgrade() {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_seiren_audio_playback(&request, error, cx)
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
        self.seiren_audio_playbacks.insert(
            id,
            Worker {
                session,
                _delivery: delivery,
            },
        );
    }
}

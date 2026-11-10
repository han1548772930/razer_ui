//! The MIC worker retains the shared radio lease until HID and COM have stopped.
use super::*;
use razer_device::earbuds_microphone::Scope;
use razer_pages::features::EarbudsMicrophoneRequested;
use razer_service::earbuds_microphone::{self as service, Command};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{SyncSender, sync_channel},
};
use uuid::Uuid;

struct Worker {
    active: Arc<AtomicBool>,
    commands: SyncSender<Command>,
    task: Task<()>,
}
#[derive(Default)]
pub(super) struct State {
    registered: bool,
    workers: BTreeMap<Uuid, Worker>,
}
impl Drop for State {
    fn drop(&mut self) {
        for worker in self.workers.values() {
            worker.active.store(false, Ordering::Release);
        }
    }
}
impl AppShell {
    pub(super) fn request_earbuds_microphone(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: EarbudsMicrophoneRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        let scope_id = match &request {
            EarbudsMicrophoneRequested::Start { scope, .. } => scope.id(),
            EarbudsMicrophoneRequested::Edit { scope_id, .. }
            | EarbudsMicrophoneRequested::HardwareMute { scope_id, .. } => *scope_id,
        };
        if !page.read(cx).earbuds_microphone_current(scope_id) {
            return;
        }
        if self.close_requested || !self.devices.contains(&workspace) {
            page.update(cx, |page, cx| {
                page.fail_earbuds_microphone(
                    scope_id,
                    "Earbud microphone workspace is closing".into(),
                    cx,
                )
            });
            return;
        }
        match request {
            EarbudsMicrophoneRequested::Start { scope, active } => {
                if self.earbuds_microphone.workers.contains_key(&scope_id) {
                    return;
                }
                let device = workspace.read(cx).device(cx);
                if scope.product_id() != device.product_id
                    || scope.container_id() != device.device_container_id
                {
                    page.update(cx, |page, cx| {
                        page.fail_earbuds_microphone(
                            scope_id,
                            "Earbud microphone container changed before startup".into(),
                            cx,
                        )
                    });
                    return;
                }
                let window = window.window_handle();
                self.with_earbuds_session(
                    workspace.clone(),
                    active.clone(),
                    None,
                    cx,
                    move |result, shell, cx| match result {
                        Ok(lease) => shell
                            .start_earbuds_microphone(workspace, scope, active, lease, window, cx),
                        Err(error) => page.update(cx, |page, cx| {
                            page.fail_earbuds_microphone(scope_id, error, cx)
                        }),
                    },
                );
            }
            EarbudsMicrophoneRequested::Edit {
                scope_id,
                request_id,
                setting,
            } => {
                self.send_earbuds_microphone(
                    &page,
                    scope_id,
                    Command::Edit {
                        scope_id,
                        request_id,
                        setting,
                    },
                    cx,
                );
            }
            EarbudsMicrophoneRequested::HardwareMute { scope_id, muted } => {
                self.send_earbuds_microphone(
                    &page,
                    scope_id,
                    Command::HardwareMute { scope_id, muted },
                    cx,
                );
            }
        }
    }
    fn send_earbuds_microphone(
        &self,
        page: &Entity<razer_pages::features::AudioProductWorkspace>,
        scope_id: Uuid,
        command: Command,
        cx: &mut Context<Self>,
    ) {
        let result = self
            .earbuds_microphone
            .workers
            .get(&scope_id)
            .ok_or_else(|| "Earbud microphone session is not ready".to_string())
            .and_then(|worker| {
                worker.commands.try_send(command).map_err(|error| {
                    format!("Earbud microphone command was not submitted: {error}")
                })
            });
        if let Err(error) = result {
            page.update(cx, |page, cx| {
                page.fail_earbuds_microphone(scope_id, error, cx)
            });
        }
    }
    fn start_earbuds_microphone(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: Scope,
        active: Arc<AtomicBool>,
        lease: super::earbuds_events::Lease,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let scope_id = scope.id();
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        if !page.read(cx).earbuds_microphone_current(scope_id)
            || self.earbuds_microphone.workers.contains_key(&scope_id)
        {
            return;
        }
        if !self.earbuds_microphone.registered {
            self.earbuds_microphone.registered = true;
            self.register_exit_cleanup(|shell, _| {
                let workers = std::mem::take(&mut shell.earbuds_microphone.workers);
                for worker in workers.values() {
                    worker.active.store(false, Ordering::Release);
                }
                async move {
                    for (_, worker) in workers {
                        worker.task.await;
                    }
                }
            });
        }
        let identity = workspace.read(cx).identity(cx);
        let connection = workspace.read(cx).connection_revision();
        let profile = workspace.read(cx).device(cx).active_profile.clone();
        let (commands, receiver) = sync_channel(32);
        let (events, incoming) = async_channel::unbounded();
        let guard = active.clone();
        let worker_active = active.clone();
        let worker = cx.background_spawn(async move {
            let result = service::run(
                lease.route().clone(),
                scope,
                worker_active,
                receiver,
                events,
            );
            drop(lease);
            result
        });
        let task = cx.spawn(async move |shell, cx| {
            while let Ok(update) = incoming.recv().await {
                let delivered = window.update(cx, |_, window, cx| {
                    shell.update(cx, |shell, cx| {
                        let device = workspace.read(cx).device(cx);
                        let current = shell.devices.contains(&workspace)
                            && workspace.read(cx).identity(cx) == identity
                            && workspace.read(cx).connection_revision() == connection
                            && device.active_profile == profile
                            && shell
                                .device_observations
                                .iter()
                                .any(|route| route.matches(device));
                        if current {
                            page.update(cx, |page, cx| {
                                page.finish_earbuds_microphone(scope_id, update, window, cx)
                            });
                        } else {
                            guard.store(false, Ordering::Release);
                        }
                    })
                });
                if !matches!(delivered, Ok(Ok(()))) {
                    guard.store(false, Ordering::Release);
                    break;
                }
            }
            drop(incoming);
            let result = worker.await;
            let _ = shell.update(cx, |shell, cx| {
                if let Err(error) = result {
                    page.update(cx, |page, cx| {
                        page.fail_earbuds_microphone(scope_id, error, cx)
                    });
                }
                shell.earbuds_microphone.workers.remove(&scope_id);
            });
        });
        self.earbuds_microphone.workers.insert(
            scope_id,
            Worker {
                active,
                commands,
                task,
            },
        );
    }
}

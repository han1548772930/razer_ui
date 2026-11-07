//! Shell ownership and cancellation of the earbud system-volume service session.
use super::*;
use razer_pages::features::EarbudsVolumeRequested;
use razer_service::earbuds_system_volume::{self as service, Command};
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
    pub(super) fn request_earbuds_volume(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: EarbudsVolumeRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        let scope_id = match &request {
            EarbudsVolumeRequested::Start { scope, .. } => scope.id(),
            EarbudsVolumeRequested::Edit { scope_id, .. } => *scope_id,
        };
        if !page.read(cx).earbuds_volume_current(scope_id) {
            return;
        }
        let device = workspace.read(cx).device(cx);
        let route_current = !self.close_requested
            && self.devices.contains(&workspace)
            && razer_device::earbuds_system_volume::supports(device.product_id)
            && self
                .device_observations
                .iter()
                .any(|route| route.matches(device));
        if !route_current {
            page.update(cx, |page, cx| {
                page.fail_earbuds_volume(
                    scope_id,
                    "No current earbud device observation".into(),
                    cx,
                )
            });
            if let Some(worker) = self.earbuds_volume.workers.get(&scope_id) {
                worker.active.store(false, Ordering::Release);
            }
            if let EarbudsVolumeRequested::Start { active, .. } = request {
                active.store(false, Ordering::Release);
            }
            return;
        }
        match request {
            EarbudsVolumeRequested::Edit {
                scope_id,
                request_id,
                setting,
            } => {
                let result = self
                    .earbuds_volume
                    .workers
                    .get(&scope_id)
                    .ok_or_else(|| "Earbud volume session is no longer running".to_string())
                    .and_then(|worker| {
                        worker
                            .commands
                            .try_send(Command::new(scope_id, request_id, setting))
                            .map_err(|error| {
                                format!("Earbud volume edit was not submitted: {error}")
                            })
                    });
                if let Err(error) = result {
                    page.update(cx, |page, cx| page.fail_earbuds_volume(scope_id, error, cx));
                }
            }
            EarbudsVolumeRequested::Start { scope, active } => {
                if self.earbuds_volume.workers.contains_key(&scope_id) {
                    return;
                }
                if scope.product_id() != device.product_id
                    || scope.container_id() != device.device_container_id
                {
                    page.update(cx, |page, cx| {
                        page.fail_earbuds_volume(
                            scope_id,
                            "Earbud container changed before volume session startup".into(),
                            cx,
                        )
                    });
                    active.store(false, Ordering::Release);
                    return;
                }
                if !self.earbuds_volume.registered {
                    self.earbuds_volume.registered = true;
                    self.register_exit_cleanup(|shell, _| {
                        let workers = std::mem::take(&mut shell.earbuds_volume.workers);
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
                let connection = workspace.read(cx).connection_revision();
                let identity = workspace.read(cx).identity(cx);
                let profile = device.active_profile.clone();
                let container = device.device_container_id.clone();
                let (commands, receiver) = sync_channel(32);
                let (events, incoming) = async_channel::unbounded();
                let guard = active.clone();
                let worker_guard = active.clone();
                let worker = cx.background_spawn(async move {
                    service::run(scope, worker_guard, receiver, events)
                });
                let task = cx.spawn_in(window, async move |shell, cx| {
                    while let Ok(update) = incoming.recv().await {
                        let _ = shell.update_in(cx, |shell, window, cx| {
                            let device = workspace.read(cx).device(cx);
                            let current = shell.devices.contains(&workspace)
                                && workspace.read(cx).connection_revision() == connection
                                && workspace.read(cx).identity(cx) == identity
                                && device.active_profile == profile
                                && device.device_container_id == container
                                && shell
                                    .device_observations
                                    .iter()
                                    .any(|route| route.matches(device));
                            if !current {
                                guard.store(false, Ordering::Release);
                            }
                            if current {
                                page.update(cx, |page, cx| {
                                    page.finish_earbuds_volume(scope_id, update, window, cx)
                                });
                            }
                        });
                    }
                    let result = worker.await;
                    let _ = shell.update_in(cx, |shell, _, cx| {
                        if let Err(error) = result {
                            shell.status = format!("耳塞系统音量会话结束：{error}");
                            page.update(cx, |page, cx| {
                                page.fail_earbuds_volume(scope_id, error, cx)
                            });
                        }
                        shell.earbuds_volume.workers.remove(&scope_id);
                    });
                });
                self.earbuds_volume.workers.insert(
                    scope_id,
                    Worker {
                        active,
                        commands,
                        task,
                    },
                );
            }
        }
    }
}

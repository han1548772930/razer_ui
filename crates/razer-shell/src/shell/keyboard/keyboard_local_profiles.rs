//! Read the local library after the current connection returns its serial.
use super::*;
use razer_discovery::discovery::ObservedDevice;
use razer_storage::host_keyboard_profiles;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
struct Scope {
    request_id: uuid::Uuid,
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    connection_revision: u64,
    serial: String,
    route: ObservedDevice,
    valid: Arc<AtomicBool>,
}

impl Scope {
    fn same_owner(
        &self,
        workspace: &Entity<ProductWorkspace>,
        route: &ObservedDevice,
        cx: &App,
    ) -> bool {
        self.valid.load(Ordering::Acquire)
            && self.workspace.upgrade().as_ref() == Some(workspace)
            && self.identity == workspace.read(cx).identity(cx)
            && self.connection_revision == workspace.read(cx).connection_revision()
            && self.serial == workspace.read(cx).device(cx).serial_number
            && device_discovery::same_connection(&self.route, route)
    }

    fn current(&self, shell: &AppShell, cx: &App) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        let routes = shell
            .device_observations
            .iter()
            .filter(|route| route.matches(workspace.read(cx).device(cx)))
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            return false;
        };
        shell.devices.contains(&workspace)
            && shell
                .keyboard_local_profiles
                .scopes
                .get(&self.identity)
                .is_some_and(|scope| scope.request_id == self.request_id)
            && self.same_owner(&workspace, route, cx)
    }
}

#[derive(Default)]
pub(super) struct State {
    scopes: BTreeMap<String, Scope>,
    delivery: Option<Task<()>>,
    pub(super) pending_batches: usize,
}

pub(super) struct LocalSave {
    scope: Scope,
    snapshot: Device,
}

impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.scope.identity, self.scope.workspace.clone())
    }
    pub(super) fn snapshot(&self) -> &Device {
        &self.snapshot
    }
}

impl AppShell {
    pub(super) fn install_keyboard_local_profiles_cleanup(&mut self) {
        self.register_exit_cleanup(|shell, _| {
            for scope in shell.keyboard_local_profiles.scopes.values() {
                scope.valid.store(false, Ordering::Release);
            }
            let delivery = shell.keyboard_local_profiles.delivery.take();
            async move {
                if let Some(delivery) = delivery {
                    delivery.await;
                }
            }
        });
        self.register_exit_resume(|shell, _| {
            shell.keyboard_local_profiles.scopes.clear();
        });
    }

    pub(super) fn retain_keyboard_local_profile_connections(&mut self, cx: &App) {
        for scope in self.keyboard_local_profiles.scopes.values() {
            if !scope.current(self, cx) {
                scope.valid.store(false, Ordering::Release);
            }
        }
    }

    pub(super) fn read_keyboard_local_profiles(
        &mut self,
        candidates: Vec<(Entity<ProductWorkspace>, ObservedDevice)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || self.storage_error.is_some() {
            return;
        }
        let mut requests = Vec::new();
        for (workspace, route) in candidates {
            let device = workspace.read(cx).device(cx);
            if !host_keyboard_profiles::supported(device.product_id)
                || device.serial_number.is_empty()
            {
                continue;
            }
            let identity = workspace.read(cx).identity(cx);
            if self
                .keyboard_local_profiles
                .scopes
                .get(&identity)
                .is_some_and(|scope| scope.same_owner(&workspace, &route, cx))
            {
                continue;
            }
            let baseline = device.clone();
            let saved = workspace.read(cx).saved_snapshot(cx);
            if !host_keyboard_profiles::same_local_profiles(&baseline, &saved) {
                log::warn!(
                    "[keyboard local profiles] pending edits prevent replacing the local library"
                );
                continue;
            }
            let scope = Scope {
                request_id: uuid::Uuid::new_v4(),
                workspace: workspace.downgrade(),
                identity: identity.clone(),
                connection_revision: workspace.read(cx).connection_revision(),
                serial: device.serial_number.clone(),
                route,
                valid: Arc::new(AtomicBool::new(true)),
            };
            if let Some(previous) = self
                .keyboard_local_profiles
                .scopes
                .insert(identity, scope.clone())
            {
                previous.valid.store(false, Ordering::Release);
            }
            requests.push((scope, baseline));
        }
        if requests.is_empty() {
            return;
        }
        let previous = self.keyboard_local_profiles.delivery.take();
        self.keyboard_local_profiles.pending_batches += 1;
        let window = window.window_handle();
        self.keyboard_local_profiles.delivery = Some(cx.spawn(async move |shell, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let results = cx
                .background_spawn(async move {
                    requests
                        .into_iter()
                        .map(|(scope, baseline)| {
                            let result = if scope.valid.load(Ordering::Acquire) {
                                razer_platform::native_paths::host_local_storage_directory()
                                    .and_then(|root| {
                                        host_keyboard_profiles::read(
                                            &root,
                                            baseline.product_id,
                                            &scope.serial,
                                        )
                                    })
                                    .map_err(|error| format!("{error:#}"))
                            } else {
                                Err("Keyboard local profile read canceled".into())
                            };
                            (scope, baseline, result)
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                for (scope, baseline, result) in results {
                    if !scope.current(shell, cx) || shell.close_requested {
                        continue;
                    }
                    match result {
                        Ok(Some(source)) => {
                            let Some(workspace) = scope.workspace.upgrade() else {
                                continue;
                            };
                            let accepted = window.update(cx, |_, window, cx| {
                                workspace.update(cx, |workspace, cx| {
                                    workspace.accept_keyboard_local_profiles(
                                        &baseline, &source, window, cx,
                                    )
                                })
                            });
                            match accepted {
                                Ok(Ok(true)) => {
                                    let snapshot = workspace.read(cx).snapshot(cx);
                                    let file = shell.workspace_file(
                                        shell
                                            .devices
                                            .iter()
                                            .map(|device| device.read(cx).saved_snapshot(cx))
                                            .collect(),
                                        cx,
                                    );
                                    let save = shell.prepare_save(
                                        window,
                                        file,
                                        Some(ProductSave::KeyboardLocalProfiles(LocalSave {
                                            scope,
                                            snapshot,
                                        })),
                                        cx,
                                    );
                                    shell.pending_saves.push_back(save);
                                    shell.start_pending_save(cx);
                                }
                                Ok(Ok(false)) => {}
                                Ok(Err(error)) => {
                                    log::warn!("[keyboard local profiles] {error}");
                                    shell.status = format!("键盘本地配置未载入：{error}");
                                }
                                Err(error) => log::warn!(
                                    "[keyboard local profiles] view owner unavailable: {error:#}"
                                ),
                            }
                        }
                        Ok(None) => {
                            shell.status = "键盘本地配置文档不存在。".into();
                        }
                        Err(error) => {
                            log::warn!("[keyboard local profiles] {error}");
                            shell.status = format!("键盘本地配置读取失败：{error}");
                        }
                    }
                }
                shell.keyboard_local_profiles.pending_batches -= 1;
                if shell.close_requested && shell.save_task.is_none() {
                    shell.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn rebase_keyboard_local_profiles_save(
        &self,
        save: &PreparedSave,
        cx: &App,
    ) -> Result<(), String> {
        if let Some(ProductSave::KeyboardLocalProfiles(local)) = &save.product {
            if !local.scope.current(self, cx) {
                return Err("Keyboard local profile owner changed before persistence".into());
            }
        }
        Ok(())
    }
}


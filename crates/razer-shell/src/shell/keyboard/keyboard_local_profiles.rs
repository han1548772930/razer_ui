//! Read the local library after the current connection returns its serial.
use super::*;
use razer_discovery::discovery::ObservedDevice;
use razer_ipc::{ServiceClient, ServiceRequest};
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

fn mode_operation(
    route: &ObservedDevice,
    operation: razer_device::keyboard_mode::Operation,
    active: Option<Arc<AtomicBool>>,
) -> anyhow::Result<()> {
    if operation == razer_device::keyboard_mode::Operation::Initialize {
        // Ordinary setup waits 500ms before its mode transition. Installer-only
        // setup is not invoked by this local profile owner.
        let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
        while let Some(remaining) = until.checked_duration_since(std::time::Instant::now()) {
            anyhow::ensure!(
                active
                    .as_ref()
                    .is_none_or(|active| active.load(Ordering::Acquire)),
                "Keyboard mode initialization canceled during source settle delay"
            );
            std::thread::sleep(remaining.min(std::time::Duration::from_millis(10)));
        }
    }
    let mut client = ServiceClient::spawn()?;
    if let Some(active) = active {
        client.set_request_guard(active);
    }
    finish_mode_session(|step| match step {
        ModeStep::Execute => {
            razer_discovery::direct::keyboard_metadata::resolve(&mut client, route)
                .and_then(|route| route.set_source_mode(&mut client, operation))
                .map(|_| ())
        }
        ModeStep::Shutdown => client.request(ServiceRequest::Shutdown).map(|_| ()),
        ModeStep::Terminate => client.finish_termination(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModeStep {
    Execute,
    Shutdown,
    Terminate,
}

fn finish_mode_session(mut run: impl FnMut(ModeStep) -> anyhow::Result<()>) -> anyhow::Result<()> {
    let mut errors = Vec::new();
    for step in [ModeStep::Execute, ModeStep::Shutdown, ModeStep::Terminate] {
        if let Err(error) = run(step) {
            errors.push(format!("{step:?}: {error:#}"));
        }
    }
    anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
    Ok(())
}

#[cfg(test)]
mod mode_tests {
    use super::{ModeStep, finish_mode_session};
    #[test]
    fn keyboard_mode_failure_still_closes_and_terminates_its_session() {
        for failed in [
            None,
            Some(ModeStep::Execute),
            Some(ModeStep::Shutdown),
            Some(ModeStep::Terminate),
        ] {
            let mut calls = Vec::new();
            let result = finish_mode_session(|step| {
                calls.push(step);
                anyhow::ensure!(Some(step) != failed, "controlled failure");
                Ok(())
            });
            assert_eq!(
                calls,
                [ModeStep::Execute, ModeStep::Shutdown, ModeStep::Terminate]
            );
            assert_eq!(result.is_err(), failed.is_some());
        }
    }
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
        self.register_exit_cleanup(|shell, cx| {
            let mode_routes = shell
                .keyboard_local_profiles
                .scopes
                .values()
                .filter(|scope| {
                    scope.current(shell, cx)
                        && razer_device::keyboard_mode::supported(scope.route.product_id())
                })
                .map(|scope| scope.route.clone())
                .collect::<Vec<_>>();
            for scope in shell.keyboard_local_profiles.scopes.values() {
                scope.valid.store(false, Ordering::Release);
            }
            let delivery = shell.keyboard_local_profiles.delivery.take();
            let background = cx.background_executor().clone();
            async move {
                if let Some(delivery) = delivery {
                    delivery.await;
                }
                background
                    .spawn(async move {
                        for route in mode_routes {
                            if let Err(error) = mode_operation(
                                &route,
                                razer_device::keyboard_mode::Operation::Release,
                                None,
                            ) {
                                log::warn!("[keyboard mode release] {error:#}");
                            }
                        }
                    })
                    .await;
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
                || device.dashboard.readonly_keyboard_metadata.is_none()
                || !self.devices.contains(&workspace)
                || !device_discovery::retained_connection(
                    std::slice::from_ref(&route),
                    &self.device_observations,
                    device,
                )
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
                                        host_keyboard_profiles::read_or_initialize_local(
                                            &root,
                                            &baseline,
                                            razer_platform::system::computer_name,
                                        )
                                    })
                                    .map_err(|error| format!("{error:#}"))
                            } else {
                                Err("Keyboard local profile read canceled".into())
                            };
                            if matches!(result, Ok(Some(_)))
                                && scope.valid.load(Ordering::Acquire)
                                && razer_device::keyboard_mode::supported(scope.route.product_id())
                            {
                                if let Err(error) = mode_operation(
                                    &scope.route,
                                    razer_device::keyboard_mode::Operation::Initialize,
                                    Some(scope.valid.clone()),
                                ) {
                                    // The actual source catches mode setup failures and continues.
                                    log::warn!("[keyboard mode initialization] {error:#}");
                                }
                            }
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

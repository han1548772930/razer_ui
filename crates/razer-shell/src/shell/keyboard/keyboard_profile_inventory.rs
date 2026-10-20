//! Connection-scoped keyboard OBM reads retain accepted work through cleanup.
use super::*;
use razer_discovery::discovery::{ObservedDevice, ObservedTransport};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[path = "keyboard_profile_inventory_worker.rs"]
mod worker;

#[derive(Clone)]
struct Scope {
    id: uuid::Uuid,
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    connection_revision: u64,
    serial: Option<String>,
    route: ObservedDevice,
    canceled: Arc<AtomicBool>,
}

fn available(route: &ObservedDevice) -> bool {
    razer_device::keyboard_profile_inventory::supports(route.product_id())
        && matches!(
            route.transport(),
            Some(ObservedTransport::Wired | ObservedTransport::Dongle)
        )
}

impl Scope {
    fn same_owner(
        &self,
        workspace: &Entity<ProductWorkspace>,
        route: &ObservedDevice,
        cx: &App,
    ) -> bool {
        !self.canceled.load(Ordering::Acquire)
            && self.workspace.upgrade().as_ref() == Some(workspace)
            && self.identity == workspace.read(cx).identity(cx)
            && self.connection_revision == workspace.read(cx).connection_revision()
            && self
                .serial
                .as_deref()
                .is_none_or(|serial| serial == workspace.read(cx).device(cx).serial_number)
            && device_discovery::same_connection(&self.route, route)
    }

    fn matches(&self, shell: &AppShell, routes: &[ObservedDevice], cx: &App) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        let routes = routes
            .iter()
            .filter(|route| route.matches(workspace.read(cx).device(cx)))
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            return false;
        };
        shell.devices.contains(&workspace)
            && available(route)
            && self.same_owner(&workspace, route, cx)
    }

    fn current(&self, shell: &AppShell, cx: &App) -> bool {
        shell
            .keyboard_profile_inventory
            .connections
            .get(&self.identity)
            .is_some_and(|owner| owner.id == self.id)
            && self.matches(shell, &shell.device_observations, cx)
    }
}

#[derive(Default)]
pub(super) struct State {
    stopped: bool,
    accepted: Arc<AtomicUsize>,
    connections: BTreeMap<String, Scope>,
    operations: BTreeMap<uuid::Uuid, Task<()>>,
}

struct Accepted(Arc<AtomicUsize>);
impl Drop for Accepted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl State {
    fn accept(&self) -> Option<Accepted> {
        if self.stopped {
            return None;
        }
        self.accepted.fetch_add(1, Ordering::AcqRel);
        Some(Accepted(self.accepted.clone()))
    }

    pub(super) fn pending(&self) -> bool {
        !self.operations.is_empty() || self.accepted.load(Ordering::Acquire) > 0
    }

    fn stop(&mut self) -> Vec<Task<()>> {
        self.stopped = true;
        for scope in self.connections.values() {
            scope.canceled.store(true, Ordering::Release);
        }
        self.connections.clear();
        std::mem::take(&mut self.operations).into_values().collect()
    }

    fn resume(&mut self) {
        if !self.pending() {
            self.stopped = false;
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        for scope in self.connections.values() {
            scope.canceled.store(true, Ordering::Release);
        }
    }
}

impl AppShell {
    pub(super) fn begin_shared_keyboard_profile_inventory(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        route: &ObservedDevice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<uuid::Uuid, String> {
        if self.close_requested
            || self.keyboard_profile_inventory.stopped
            || !available(route)
            || route.product_id() != 756
        {
            return Err("Shared keyboard inventory owner is unavailable".into());
        }
        let identity = workspace.read(cx).identity(cx);
        let scope = Scope {
            id: uuid::Uuid::new_v4(),
            workspace: workspace.downgrade(),
            identity: identity.clone(),
            connection_revision: workspace.read(cx).connection_revision(),
            serial: route.keyboard_serial().map(str::to_owned),
            route: route.clone(),
            canceled: Arc::new(AtomicBool::new(false)),
        };
        if !scope.matches(self, &self.device_observations, cx) {
            return Err("Keyboard inventory has no unique current physical connection".into());
        }
        workspace.update(cx, |workspace, cx| {
            workspace.begin_keyboard_profile_inventory(scope.id, scope.canceled.clone(), window, cx)
        })?;
        if let Some(previous) = self
            .keyboard_profile_inventory
            .connections
            .insert(identity, scope.clone())
        {
            previous.canceled.store(true, Ordering::Release);
        }
        Ok(scope.id)
    }

    pub(super) fn publish_shared_keyboard_profile_slots(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        id: uuid::Uuid,
        result: Result<razer_ipc::keyboard_profile_inventory::SlotsObservation, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self
            .keyboard_profile_inventory
            .connections
            .get(&workspace.read(cx).identity(cx))
            .is_some_and(|scope| scope.id == id && scope.current(self, cx));
        if current && !self.close_requested {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_profile_slots(id, result, window, cx)
            });
        }
    }

    pub(super) fn install_keyboard_profile_inventory_cleanup(&mut self) {
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.keyboard_profile_inventory.stop();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
        self.register_exit_resume(|shell, _| shell.keyboard_profile_inventory.resume());
    }

    pub(super) fn retain_keyboard_profile_inventory_connections(
        &mut self,
        routes: &[ObservedDevice],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let stale = self
            .keyboard_profile_inventory
            .connections
            .values()
            .filter(|scope| !scope.matches(self, routes, cx))
            .cloned()
            .collect::<Vec<_>>();
        for scope in stale {
            scope.canceled.store(true, Ordering::Release);
            self.keyboard_profile_inventory
                .connections
                .remove(&scope.identity);
            if let Some(workspace) = scope.workspace.upgrade() {
                let connected = workspace
                    .read(cx)
                    .device(cx)
                    .dashboard
                    .connection_observation
                    .is_some();
                workspace.update(cx, |workspace, cx| {
                    workspace.clear_keyboard_profile_inventory(connected, window, cx)
                });
            }
        }
    }

    pub(super) fn read_keyboard_profile_inventories(
        &mut self,
        candidates: Vec<(Entity<ProductWorkspace>, ObservedDevice)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || self.keyboard_profile_inventory.stopped {
            return;
        }
        for (workspace, route) in candidates {
            if !available(&route) {
                continue;
            }
            // The retained 756 runtime publishes its existing list/metadata
            // read through begin_shared_keyboard_profile_inventory.
            if route.product_id() == 756 {
                continue;
            }
            let identity = workspace.read(cx).identity(cx);
            if self
                .keyboard_profile_inventory
                .connections
                .get(&identity)
                .is_some_and(|scope| scope.same_owner(&workspace, &route, cx))
            {
                continue;
            }
            let scope = Scope {
                id: uuid::Uuid::new_v4(),
                workspace: workspace.downgrade(),
                identity: identity.clone(),
                connection_revision: workspace.read(cx).connection_revision(),
                serial: route.keyboard_serial().map(str::to_owned),
                route,
                canceled: Arc::new(AtomicBool::new(false)),
            };
            if !scope.matches(self, &self.device_observations, cx) {
                continue;
            }
            let options = match workspace.update(cx, |workspace, cx| {
                workspace.begin_keyboard_profile_inventory(
                    scope.id,
                    scope.canceled.clone(),
                    window,
                    cx,
                )
            }) {
                Ok(options) => options,
                Err(error) => {
                    self.status = format!("Keyboard OBM inventory: {error}");
                    continue;
                }
            };
            let Some(accepted) = self.keyboard_profile_inventory.accept() else {
                scope.canceled.store(true, Ordering::Release);
                continue;
            };
            if let Some(previous) = self
                .keyboard_profile_inventory
                .connections
                .insert(identity, scope.clone())
            {
                previous.canceled.store(true, Ordering::Release);
            }
            let full = options.is_some();
            let task_scope = scope.clone();
            let handle = window.window_handle();
            let delivery = cx.spawn(async move |shell, cx| {
                let route = task_scope.route.clone();
                let canceled = task_scope.canceled.clone();
                let completion = cx
                    .background_spawn(async move {
                        let _accepted = accepted;
                        worker::run(route, options, canceled)
                    })
                    .await;
                let _ = shell.update(cx, |shell, cx| {
                    let current = task_scope.current(shell, cx) && !shell.close_requested;
                    shell
                        .keyboard_profile_inventory
                        .operations
                        .remove(&task_scope.id);
                    let cleanup_ok = completion.cleanup_error.is_none();
                    if let Some(error) = &completion.cleanup_error {
                        log::error!("[keyboard inventory cleanup] {error}");
                        shell.status = format!("Keyboard OBM cleanup: {error}");
                    }
                    if current && let Some(workspace) = task_scope.workspace.upgrade() {
                        let result = completion
                            .result
                            .and_then(|reading| completion.cleanup_error.map_or(Ok(reading), Err));
                        let publication = handle.update(cx, |_, window, cx| {
                            workspace.update(cx, |workspace, cx| match result {
                                Ok(worker::Reading::Inventory(observation)) => workspace
                                    .finish_keyboard_profile_inventory(
                                        task_scope.id,
                                        Ok(observation),
                                        window,
                                        cx,
                                    ),
                                Ok(worker::Reading::Complete(observation)) => workspace
                                    .finish_keyboard_onboard_profiles(
                                        task_scope.id,
                                        Ok(observation),
                                        window,
                                        cx,
                                    ),
                                Err(error) if full => workspace.finish_keyboard_onboard_profiles(
                                    task_scope.id,
                                    Err(error),
                                    window,
                                    cx,
                                ),
                                Err(error) => workspace.finish_keyboard_profile_inventory(
                                    task_scope.id,
                                    Err(error),
                                    window,
                                    cx,
                                ),
                            });
                        });
                        if let Err(error) = publication {
                            log::warn!("[keyboard inventory publication] {error:#}");
                        }
                    }
                    if shell.close_requested {
                        shell.continue_save_queue(cleanup_ok, cx);
                    }
                    cx.notify();
                });
            });
            self.keyboard_profile_inventory
                .operations
                .insert(scope.id, delivery);
        }
    }
}

#[cfg(test)]
#[path = "keyboard_profile_inventory_tests.rs"]
mod tests;

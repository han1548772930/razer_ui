//! DPI requests retain their route, delivery and service cleanup independently
//! of window lifetime or unrelated discovery refreshes.
use super::*;
use razer_device::mouse_current_dpi::{DpiReport, ble::initialization::ObservedSlots};
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_pages::features::mouse_polling::MousePollingScope;
use razer_pages::features::mouse_products::MouseCurrentDpiRequested;
use razer_storage::host_local_profile::ble226::Catalog;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[path = "mouse_current_dpi_worker.rs"]
mod worker;

struct Operation {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    route: ObservedDevice,
    request: MouseCurrentDpiRequested,
    local_profile_baseline: Device,
    window: AnyWindowHandle,
    delivery: Task<()>,
    ble_generation: Option<u64>,
}

struct BleConnection {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    route: ObservedDevice,
    scope: MousePollingScope,
    generation: u64,
    slots: Option<ObservedSlots>,
    pending_catalog: Option<PendingBleCatalog<Catalog>>,
}

#[derive(Clone)]
struct PendingBleCatalog<T> {
    serial: String,
    catalog: T,
    baseline: Device,
    scope: MousePollingScope,
}

impl<T> PendingBleCatalog<T> {
    fn matches(&self, device: &Device, scope: MousePollingScope) -> bool {
        self.scope == scope
            && (device.serial_number == self.serial
                || device.serial_number == self.baseline.serial_number)
            && razer_storage::workspace::ensure_local_profiles_unchanged(device, &self.baseline)
                .is_ok()
    }
}

fn discard_stale_ble_catalog<T>(
    pending: &mut Option<PendingBleCatalog<T>>,
    device: &Device,
    scope: MousePollingScope,
) {
    if pending
        .as_ref()
        .is_some_and(|catalog| !catalog.matches(device, scope))
    {
        *pending = None;
    }
}

fn catalog_for_verified_read<'a, T>(
    pending: &'a mut Option<PendingBleCatalog<T>>,
    incoming: Option<PendingBleCatalog<T>>,
    device: &Device,
    scope: MousePollingScope,
    is_read: bool,
    result: &Result<DpiReport, String>,
) -> Option<&'a PendingBleCatalog<T>> {
    discard_stale_ble_catalog(pending, device, scope);
    if pending.is_none()
        && let Some(incoming) = incoming.filter(|catalog| catalog.matches(device, scope))
    {
        *pending = Some(incoming);
    }
    // Projection is deferred into the window and has no acceptance receipt.
    // Retain the original baseline until it changes or the connection retires.
    if is_read && result.as_ref().is_ok_and(|report| report.verified) {
        pending.as_ref()
    } else {
        None
    }
}

fn current_ble_completion(
    current: bool,
    generation: u64,
    scope: MousePollingScope,
    request_generation: Option<u64>,
    request_scope: MousePollingScope,
) -> bool {
    current && Some(generation) == request_generation && scope.same_device_connection(request_scope)
}

impl BleConnection {
    fn matches(&self, shell: &AppShell, observations: &[ObservedDevice], cx: &App) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        shell.devices.contains(&workspace)
            && workspace.read(cx).identity(cx) == self.identity
            && workspace
                .read(cx)
                .mouse_polling_scope(cx)
                .is_some_and(|scope| self.scope.same_device_connection(scope))
            && current_route(&self.route, observations, workspace.read(cx).device(cx))
    }
}

impl Operation {
    fn matches(&self, shell: &AppShell, observations: &[ObservedDevice], cx: &App) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        shell.devices.contains(&workspace)
            && workspace.read(cx).identity(cx) == self.identity
            && workspace
                .read(cx)
                .current_dpi_request_matches(&self.request, cx)
            && current_route(&self.route, observations, workspace.read(cx).device(cx))
    }
}

fn current_route(route: &ObservedDevice, observations: &[ObservedDevice], device: &Device) -> bool {
    super::device_discovery::retained_connection(std::slice::from_ref(route), observations, device)
        && direct::mouse_current_dpi::available(route)
}

#[derive(Default)]
pub(super) struct State {
    stopped: bool,
    next_id: u64,
    accepted: Arc<AtomicUsize>,
    operations: BTreeMap<u64, Operation>,
    next_connection: u64,
    ble_connections: BTreeMap<EntityId, BleConnection>,
}

struct Accepted(Arc<AtomicUsize>);

impl Drop for Accepted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl State {
    fn resume_after_exit(&mut self) {
        if !self.pending() {
            self.stopped = false;
        }
    }

    pub(super) fn pending(&self) -> bool {
        !self.operations.is_empty() || self.accepted.load(Ordering::Acquire) > 0
    }

    fn accept(&mut self) -> Option<(u64, Accepted)> {
        if self.stopped {
            return None;
        }
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("DPI operation identity exhausted");
        self.accepted.fetch_add(1, Ordering::AcqRel);
        Some((self.next_id, Accepted(self.accepted.clone())))
    }

    fn take_for_shutdown(&mut self) -> Vec<Task<()>> {
        self.stopped = true;
        self.ble_connections.clear();
        std::mem::take(&mut self.operations)
            .into_values()
            .map(|operation| {
                operation.request.guard().store(false, Ordering::Release);
                operation.delivery
            })
            .collect()
    }
}

impl Drop for State {
    fn drop(&mut self) {
        for operation in self.operations.values() {
            operation.request.guard().store(false, Ordering::Release);
        }
    }
}

impl AppShell {
    pub(super) fn install_current_dpi_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_resume(|shell, _| shell.mouse_dpi.resume_after_exit());
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.mouse_dpi.take_for_shutdown();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
    }

    pub(super) fn retain_current_dpi_connections(
        &mut self,
        observations: &[ObservedDevice],
        cx: &App,
    ) {
        for operation in self.mouse_dpi.operations.values() {
            if !operation.matches(self, observations, cx) {
                operation.request.guard().store(false, Ordering::Release);
            }
        }
        let removed: Vec<_> = self
            .mouse_dpi
            .ble_connections
            .iter()
            .filter_map(|(owner, connection)| {
                (!connection.matches(self, observations, cx)).then_some(*owner)
            })
            .collect();
        for owner in removed {
            self.mouse_dpi.ble_connections.remove(&owner);
        }
        for connection in self.mouse_dpi.ble_connections.values_mut() {
            if let Some(workspace) = connection.workspace.upgrade()
                && let Some(scope) = workspace.read(cx).mouse_polling_scope(cx)
            {
                discard_stale_ble_catalog(
                    &mut connection.pending_catalog,
                    workspace.read(cx).device(cx),
                    scope,
                );
            }
        }
    }

    pub(super) fn start_current_dpi_operation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        route: ObservedDevice,
        request: MouseCurrentDpiRequested,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some((id, accepted)) = self.mouse_dpi.accept() else {
            let error = "DPI task owner has stopped accepting requests".to_owned();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_current_dpi_result(&request, Err(error.clone()), cx)
            });
            if request.can_finish_during_exit() {
                self.status = error;
                self.cancel_pending_exit(cx);
            }
            return;
        };
        let identity = workspace.read(cx).identity(cx);
        let ble_plan =
            if route.transport() == Some(razer_discovery::discovery::ObservedTransport::Ble) {
                let owner_id = workspace.entity_id();
                let connection_matches =
                    self.mouse_dpi
                        .ble_connections
                        .get(&owner_id)
                        .is_some_and(|connection| {
                            connection.matches(self, &self.device_observations, cx)
                                && connection.scope.same_device_connection(request.scope())
                        });
                if !connection_matches {
                    self.mouse_dpi.next_connection = self
                        .mouse_dpi
                        .next_connection
                        .checked_add(1)
                        .expect("DPI connection generation exhausted");
                    self.mouse_dpi.ble_connections.insert(
                        owner_id,
                        BleConnection {
                            workspace: workspace.downgrade(),
                            identity: identity.clone(),
                            route: route.clone(),
                            scope: request.scope(),
                            generation: self.mouse_dpi.next_connection,
                            slots: None,
                            pending_catalog: None,
                        },
                    );
                }
                let connection = self
                    .mouse_dpi
                    .ble_connections
                    .get_mut(&owner_id)
                    .expect("retained DPI BLE connection");
                discard_stale_ble_catalog(
                    &mut connection.pending_catalog,
                    workspace.read(cx).device(cx),
                    request.scope(),
                );
                Some(worker::BlePlan {
                    connection_owner: format!("{owner_id:?}"),
                    generation: connection.generation,
                    cached: connection.slots.clone(),
                    local: workspace.read(cx).saved_snapshot(cx),
                })
            } else {
                None
            };
        let ble_generation = ble_plan.as_ref().map(|plan| plan.generation);
        let owner = workspace.downgrade();
        let worker_request = request.clone();
        let worker_route = route.clone();
        let serial = workspace.read(cx).device(cx).serial_number.clone();
        let local_profile_baseline = workspace.read(cx).device(cx).clone();
        let delivery = cx.spawn(async move |shell, cx| {
            let completion = cx
                .background_spawn(async move {
                    let _accepted = accepted;
                    worker::run(worker_route, worker_request, serial, ble_plan)
                })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                if let Some(error) = completion.cleanup_error() {
                    eprintln!("[DPI cleanup] {error}");
                }
                let Some(operation) = shell.mouse_dpi.operations.remove(&id) else {
                    return;
                };
                let current = operation.matches(shell, &shell.device_observations, cx);
                let result = if current {
                    completion.result()
                } else {
                    Err("DPI 设备连接或请求范围已改变".to_owned())
                };
                let mut projected_catalog = None;
                if current
                    && let Some(workspace) = operation.workspace.upgrade()
                    && let Some(connection) = shell
                        .mouse_dpi
                        .ble_connections
                        .get_mut(&workspace.entity_id())
                    && current_ble_completion(
                        current,
                        connection.generation,
                        connection.scope,
                        operation.ble_generation,
                        operation.request.scope(),
                    )
                {
                    if let Some(slots) = completion.observed_slots() {
                        connection.slots = Some(slots.clone());
                    } else if completion.initialization_error().is_some() {
                        connection.slots = None;
                    }
                    let incoming = completion.ble_catalog().and_then(|(serial, catalog)| {
                        (serial == catalog.serial()).then(|| PendingBleCatalog {
                            serial: serial.to_owned(),
                            catalog: catalog.clone(),
                            baseline: operation.local_profile_baseline.clone(),
                            scope: operation.request.scope(),
                        })
                    });
                    projected_catalog = catalog_for_verified_read(
                        &mut connection.pending_catalog,
                        incoming,
                        workspace.read(cx).device(cx),
                        operation.request.scope(),
                        operation.request.is_read(),
                        &result,
                    )
                    .cloned();
                }
                let exit_task_failed = operation.request.can_finish_during_exit()
                    && !result.as_ref().is_ok_and(|report| report.verified);
                if let Some(workspace) = operation.workspace.upgrade() {
                    if let Some(catalog) = projected_catalog {
                        let _ = operation.window.update(cx, |_, window, cx| {
                            workspace.update(cx, |workspace, cx| {
                                workspace.accept_current_dpi_ble_catalog(
                                    &operation.request,
                                    &catalog.baseline,
                                    &catalog.serial,
                                    &catalog.catalog,
                                    window,
                                    cx,
                                )
                            });
                        });
                    }
                    workspace.update(cx, |workspace, cx| {
                        if current && let Some(profile) = completion.local_profile() {
                            workspace.accept_current_dpi_local_profile(
                                &operation.request,
                                &operation.local_profile_baseline,
                                result
                                    .as_ref()
                                    .ok()
                                    .and_then(|report| report.serial_number.as_deref()),
                                profile,
                                cx,
                            );
                        }
                        workspace.finish_current_dpi_outcome(
                            &operation.request,
                            current.then(|| completion.actual_report()).flatten(),
                            result.err(),
                            cx,
                        )
                    });
                    if current {
                        let _ = operation.window.update(cx, |_, window, cx| {
                            workspace.update(cx, |workspace, cx| {
                                workspace.sync_current_dpi_controls(window, cx)
                            });
                        });
                    }
                }
                if current {
                    shell.status = completion.message(&operation.request);
                } else if exit_task_failed {
                    shell.status = "DPI 退出任务尚未确认：设备连接或请求范围已改变。".into();
                }
                if exit_task_failed || completion.cleanup_error().is_some() {
                    shell.cancel_pending_exit(cx);
                }
                if shell.close_requested && shell.save_task.is_none() {
                    shell.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        });
        self.mouse_dpi.operations.insert(
            id,
            Operation {
                workspace: owner,
                identity,
                route,
                request,
                local_profile_baseline,
                window,
                delivery,
                ble_generation,
            },
        );
    }
}

#[cfg(test)]
#[path = "mouse_current_dpi_runtime_tests.rs"]
mod tests;

//! Device power operations retain their route, delivery and native cleanup
//! independently of windows and unrelated discovery snapshots.
use super::*;
use razer_discovery::{direct, discovery::ObservedDevice, discovery::ObservedTransport};
use razer_pages::features::{
    mouse_polling::MousePollingScope, mouse_products::MouseCurrentPowerRequested,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[path = "mouse_current_power_worker.rs"]
mod worker;

#[derive(Clone, Copy)]
enum RouteKind {
    Current,
    Idle,
}

#[derive(Clone)]
pub(super) enum Request {
    Current(MouseCurrentPowerRequested),
    Idle {
        scope: MousePollingScope,
        minutes: u8,
        valid: Arc<AtomicBool>,
    },
}

impl Request {
    fn kind(&self) -> RouteKind {
        match self {
            Self::Current(_) => RouteKind::Current,
            Self::Idle { .. } => RouteKind::Idle,
        }
    }

    fn guard(&self) -> Arc<AtomicBool> {
        match self {
            Self::Current(request) => request.guard(),
            Self::Idle { valid, .. } => valid.clone(),
        }
    }

    fn matches(&self, workspace: &ProductWorkspace, cx: &App) -> bool {
        match self {
            Self::Current(request) => workspace.current_power_request_matches(request, cx),
            Self::Idle { scope, minutes, .. } => {
                workspace.mouse_idle_request_matches(*scope, *minutes, cx)
            }
        }
    }

    fn finish(
        &self,
        workspace: &mut ProductWorkspace,
        completion: Option<&worker::Completion>,
        cx: &mut Context<ProductWorkspace>,
    ) {
        match self {
            Self::Current(request) => {
                workspace.finish_current_power(
                    request,
                    completion.and_then(worker::Completion::current_report),
                    cx,
                );
            }
            Self::Idle { scope, minutes, .. } => {
                workspace.finish_mouse_idle(
                    *scope,
                    *minutes,
                    completion.and_then(worker::Completion::idle_seconds),
                    cx,
                );
            }
        }
    }
}

struct Operation {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    route: ObservedDevice,
    request: Request,
    delivery: Task<()>,
}

impl Operation {
    fn matches(&self, shell: &AppShell, observations: &[ObservedDevice], cx: &App) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        self.request.guard().load(Ordering::Acquire)
            && shell.devices.contains(&workspace)
            && workspace.read(cx).identity(cx) == self.identity
            && self.request.matches(workspace.read(cx), cx)
            && current_route(
                &self.route,
                observations,
                workspace.read(cx).device(cx),
                self.request.kind(),
            )
    }
}

fn supports_route(route: &ObservedDevice, kind: RouteKind) -> bool {
    match kind {
        RouteKind::Current => {
            matches!(
                route.transport(),
                Some(ObservedTransport::Wired | ObservedTransport::Dongle | ObservedTransport::Ble)
            ) && razer_device::mouse_current_power::capability(route.product_id()).is_some_and(
                |source| source.accepts(route.physical_product_id(), route.peer_product_id()),
            )
        }
        RouteKind::Idle => {
            route.product_id() == 182
                && ((route.peer_product_id().is_none()
                    && route.transport() == Some(ObservedTransport::Wired))
                    || direct::mouse_receiver_polling::available(route))
        }
    }
}

/// Selection and completion share the capability owner; complete physical
/// identity and ambiguity remain owned by device discovery.
pub(super) fn select_route(
    observations: &[ObservedDevice],
    device: &Device,
    request: &Request,
) -> Option<ObservedDevice> {
    let mut routes = observations.iter().filter(|route| route.matches(device));
    let route = routes.next()?;
    (routes.next().is_none() && supports_route(route, request.kind())).then(|| route.clone())
}

fn current_route(
    route: &ObservedDevice,
    observations: &[ObservedDevice],
    device: &Device,
    kind: RouteKind,
) -> bool {
    super::device_discovery::retained_connection(std::slice::from_ref(route), observations, device)
        && supports_route(route, kind)
}

#[derive(Default)]
pub(super) struct State {
    stopped: bool,
    next_id: u64,
    accepted: Arc<AtomicUsize>,
    operations: BTreeMap<u64, Operation>,
}

/// Admission precedes scheduling so exit also sees an unpolled delivery.
struct Accepted(Arc<AtomicUsize>);

impl Drop for Accepted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl State {
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
            .expect("Mouse power operation identity exhausted");
        self.accepted.fetch_add(1, Ordering::AcqRel);
        Some((self.next_id, Accepted(self.accepted.clone())))
    }

    fn take_for_shutdown(&mut self) -> Vec<Task<()>> {
        self.stopped = true;
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
    pub(super) fn install_current_power_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.mouse_power.take_for_shutdown();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
    }

    pub(super) fn retain_current_power_connections(
        &self,
        observations: &[ObservedDevice],
        cx: &App,
    ) {
        for operation in self.mouse_power.operations.values() {
            if !operation.matches(self, observations, cx) {
                operation.request.guard().store(false, Ordering::Release);
            }
        }
    }

    pub(super) fn start_mouse_power_operation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        route: ObservedDevice,
        request: Request,
        cx: &mut Context<Self>,
    ) {
        let Some((id, accepted)) = self.mouse_power.accept() else {
            request.guard().store(false, Ordering::Release);
            workspace.update(cx, |workspace, cx| request.finish(workspace, None, cx));
            return;
        };
        let identity = workspace.read(cx).identity(cx);
        let owner = workspace.downgrade();
        let worker_request = request.clone();
        let worker_route = route.clone();
        let delivery = cx.spawn(async move |shell, cx| {
            let completion = cx
                .background_spawn(async move {
                    let _accepted = accepted;
                    worker::run(worker_route, worker_request)
                })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                let cleanup_error = completion.cleanup_error();
                if let Some(error) = &cleanup_error {
                    eprintln!("[mouse power cleanup] {error}");
                }
                let Some(operation) = shell.mouse_power.operations.remove(&id) else {
                    return;
                };
                let current = operation.matches(shell, &shell.device_observations, cx);
                if let Some(workspace) = operation.workspace.upgrade() {
                    if current {
                        shell.device_value_owners.remove(&operation.identity);
                        shell.device_read_scopes.remove(&operation.identity);
                        shell.status = completion
                            .message(&operation.request, workspace.read(cx).device(cx).product_id);
                    }
                    // Finish releases the exact page request even after its
                    // original window or connection has disappeared.
                    workspace.update(cx, |workspace, cx| {
                        operation
                            .request
                            .finish(workspace, current.then_some(&completion), cx)
                    });
                }
                if shell.close_requested {
                    if !current && let Some(error) = &cleanup_error {
                        shell.status = format!("电源设置通信清理失败：{error}");
                    }
                    shell.continue_save_queue(cleanup_error.is_none(), cx);
                }
                cx.notify();
            });
        });
        self.mouse_power.operations.insert(
            id,
            Operation {
                workspace: owner,
                identity,
                route,
                request,
                delivery,
            },
        );
    }
}

#[cfg(test)]
#[path = "mouse_current_power_runtime_tests.rs"]
mod tests;

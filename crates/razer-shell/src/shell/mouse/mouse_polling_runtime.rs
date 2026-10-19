//! Polling submissions own their delivery and native cleanup independently of
//! the window and of discovery snapshots for unrelated devices.
use super::*;
use razer_device::mouse_current_polling::Change;
use razer_discovery::{direct, discovery::ObservedDevice, discovery::ObservedTransport};
use razer_pages::features::mouse_polling::{MousePollingScope, PollingField};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[path = "mouse_polling_worker.rs"]
mod worker;

#[derive(Clone)]
pub(super) struct Request {
    scope: MousePollingScope,
    field: PollingField,
    hz: u32,
    device_hz: u32,
    change: Option<Change>,
    valid: Arc<AtomicBool>,
}

impl Request {
    pub(super) fn new(
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
        device_hz: u32,
        change: Option<Change>,
        valid: Arc<AtomicBool>,
    ) -> Self {
        Self {
            scope,
            field,
            hz,
            device_hz,
            change,
            valid,
        }
    }
}

struct Operation {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    profile: String,
    route: ObservedDevice,
    request: Request,
    delivery: Task<()>,
}

impl Operation {
    fn matches(&self, shell: &AppShell, observations: &[ObservedDevice], cx: &App) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        self.request.valid.load(Ordering::Acquire)
            && shell.devices.contains(&workspace)
            && workspace.read(cx).identity(cx) == self.identity
            && workspace.read(cx).device(cx).active_profile == self.profile
            && workspace.read(cx).mouse_polling_request_matches(
                self.request.scope,
                self.request.field,
                self.request.hz,
                cx,
            )
            && current_route(
                &self.route,
                observations,
                workspace.read(cx).device(cx),
                self.request.field,
            )
    }
}

pub(super) fn supports_route(route: &ObservedDevice, field: PollingField) -> bool {
    if route.product_id() == 190 && !direct::mouse_current_polling::available(route) {
        return false;
    }
    match field {
        PollingField::Wired => {
            route.peer_product_id().is_none() && route.transport() == Some(ObservedTransport::Wired)
        }
        PollingField::Wireless if route.product_id() == 190 => {
            route.transport() == Some(ObservedTransport::Dongle)
        }
        PollingField::Wireless => direct::mouse_receiver_polling::available(route),
    }
}

fn current_route(
    route: &ObservedDevice,
    observations: &[ObservedDevice],
    device: &Device,
    field: PollingField,
) -> bool {
    super::device_discovery::retained_connection(std::slice::from_ref(route), observations, device)
        && supports_route(route, field)
}

#[derive(Default)]
pub(super) struct State {
    stopped: bool,
    next_id: u64,
    accepted: Arc<AtomicUsize>,
    operations: BTreeMap<u64, Operation>,
}

/// Acquired before scheduling so exit also sees work in an unpolled executor queue.
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
            .expect("Polling operation identity exhausted");
        self.accepted.fetch_add(1, Ordering::AcqRel);
        Some((self.next_id, Accepted(self.accepted.clone())))
    }

    fn take_for_shutdown(&mut self) -> Vec<Task<()>> {
        self.stopped = true;
        std::mem::take(&mut self.operations)
            .into_values()
            .map(|operation| {
                operation.request.valid.store(false, Ordering::Release);
                operation.delivery
            })
            .collect()
    }
}

impl Drop for State {
    fn drop(&mut self) {
        for operation in self.operations.values() {
            operation.request.valid.store(false, Ordering::Release);
        }
    }
}

impl AppShell {
    pub(super) fn install_mouse_polling_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_resume(|shell, _| shell.mouse_polling.resume_after_exit());
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.mouse_polling.take_for_shutdown();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
    }

    pub(super) fn retain_mouse_polling_connections(
        &self,
        observations: &[ObservedDevice],
        cx: &App,
    ) {
        for operation in self.mouse_polling.operations.values() {
            if !operation.matches(self, observations, cx) {
                operation.request.valid.store(false, Ordering::Release);
            }
        }
    }

    pub(super) fn start_mouse_polling_operation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        route: ObservedDevice,
        request: Request,
        cx: &mut Context<Self>,
    ) {
        let Some((id, accepted)) = self.mouse_polling.accept() else {
            request.valid.store(false, Ordering::Release);
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_polling(request.scope, request.field, request.hz, None, cx)
            });
            return;
        };
        let identity = workspace.read(cx).identity(cx);
        let profile = workspace.read(cx).device(cx).active_profile.clone();
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
                    eprintln!("[polling cleanup] {error}");
                }
                let Some(operation) = shell.mouse_polling.operations.remove(&id) else {
                    return;
                };
                let current = operation.matches(shell, &shell.device_observations, cx);
                if current {
                    shell.device_value_owners.remove(&operation.identity);
                    shell.device_read_scopes.remove(&operation.identity);
                    // Completion may dispatch a queued successor. Publish this
                    // result before that successor owns its loading feedback.
                    shell.status = completion.message();
                }
                if let Some(workspace) = operation.workspace.upgrade()
                    && workspace.read(cx).mouse_polling_in_flight_matches(
                        operation.request.scope,
                        operation.request.field,
                        operation.request.hz,
                        cx,
                    )
                {
                    let observed = current.then(|| completion.observed()).flatten();
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_mouse_polling(
                            operation.request.scope,
                            operation.request.field,
                            operation.request.hz,
                            observed,
                            cx,
                        )
                    });
                }
                if shell.close_requested {
                    if !current && let Some(error) = &cleanup_error {
                        shell.status = format!("轮询率通信清理失败：{error}");
                    }
                    shell.continue_save_queue(cleanup_error.is_none(), cx);
                }
                cx.notify();
            });
        });
        self.mouse_polling.operations.insert(
            id,
            Operation {
                workspace: owner,
                identity,
                profile,
                route,
                request,
                delivery,
            },
        );
    }
}

#[cfg(test)]
#[path = "mouse_polling_runtime_tests.rs"]
mod tests;

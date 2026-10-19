//! PID70's retained setter delivery owns cancellation through transport release.
use super::*;
use razer_discovery::discovery::ObservedDevice;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Operation {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    route: ObservedDevice,
    request: MouseLegacyDpiRequested,
    delivery: Task<()>,
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
                .legacy_dpi_request_matches(&self.request, cx)
            && current_route(&self.route, observations, workspace.read(cx).device(cx))
    }
}

fn current_route(route: &ObservedDevice, observations: &[ObservedDevice], device: &Device) -> bool {
    crate::shell::device_discovery::retained_connection(
        std::slice::from_ref(route),
        observations,
        device,
    ) && route.peer_product_id().is_none()
        && matches!(route.transport(), Some(discovery::ObservedTransport::Wired))
}

#[derive(Default)]
pub(in crate::shell) struct State {
    stopped: bool,
    next_id: u64,
    accepted: Arc<AtomicUsize>,
    operations: BTreeMap<u64, Operation>,
}

struct Accepted(Arc<AtomicUsize>);

impl Drop for Accepted {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl State {
    pub(in crate::shell) fn pending(&self) -> bool {
        !self.operations.is_empty() || self.accepted.load(Ordering::Acquire) > 0
    }

    fn accept(&mut self) -> Option<(u64, Accepted)> {
        if self.stopped {
            return None;
        }
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("legacy DPI operation identity exhausted");
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

    fn resume_after_exit(&mut self) {
        if !self.pending() {
            self.stopped = false;
        }
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
    pub(in crate::shell) fn install_legacy_dpi_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_resume(|shell, _| shell.legacy_dpi.resume_after_exit());
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.legacy_dpi.take_for_shutdown();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
    }

    pub(in crate::shell) fn retain_legacy_dpi_connections(
        &mut self,
        observations: &[ObservedDevice],
        cx: &App,
    ) {
        for operation in self.legacy_dpi.operations.values() {
            if !operation.matches(self, observations, cx) {
                operation.request.guard().store(false, Ordering::Release);
            }
        }
    }

    pub(in crate::shell) fn start_legacy_dpi_operation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        route: ObservedDevice,
        request: MouseLegacyDpiRequested,
        context: &'static str,
        x: u16,
        y: u16,
        cx: &mut Context<Self>,
    ) {
        let Some((id, accepted)) = self.legacy_dpi.accept() else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_legacy_dpi(&request, cx)
            });
            if request.cleanup {
                self.cancel_pending_exit(cx);
            }
            return;
        };
        let identity = workspace.read(cx).identity(cx);
        self.device_value_owners.remove(&identity);
        self.device_read_scopes.remove(&identity);
        self.status = format!("{context}，正在等待设备设置应答。");
        let worker_request = request.clone();
        let worker_route = route.clone();
        let delivery = cx.spawn(async move |shell, cx| {
            let completion = cx
                .background_spawn(async move {
                    let _accepted = accepted;
                    worker::run(worker_route, worker_request, x, y)
                })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                let Some(operation) = shell.legacy_dpi.operations.remove(&id) else {
                    return;
                };
                let current = operation.matches(shell, &shell.device_observations, cx);
                let confirmed = current && completion.acknowledged();
                if let Some(workspace) = operation.workspace.upgrade() {
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_legacy_dpi_result(&operation.request, confirmed, cx)
                    });
                }
                if current {
                    shell.device_value_owners.remove(&operation.identity);
                    shell.device_read_scopes.remove(&operation.identity);
                    shell.status = completion.message(context, true, x, y);
                } else if operation.request.cleanup || completion.cleanup_error().is_some() {
                    shell.status = completion.message(context, false, x, y);
                }
                let succeeded = (!operation.request.cleanup || confirmed)
                    && completion.cleanup_error().is_none();
                if !succeeded {
                    shell.cancel_pending_exit(cx);
                }
                if shell.close_requested && shell.save_task.is_none() {
                    shell.continue_save_queue(succeeded, cx);
                }
                cx.notify();
            });
        });
        self.legacy_dpi.operations.insert(
            id,
            Operation {
                workspace: workspace.downgrade(),
                identity,
                route,
                request,
                delivery,
            },
        );
        cx.notify();
    }
}

#[cfg(test)]
#[path = "mouse_dpi_70_runtime_tests.rs"]
mod tests;

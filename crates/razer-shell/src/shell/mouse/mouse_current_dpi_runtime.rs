//! DPI requests retain their route, delivery and service cleanup independently
//! of window lifetime or unrelated discovery refreshes.
use super::*;
use razer_device::mouse_current_dpi::DpiReport;
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_pages::features::mouse_products::MouseCurrentDpiRequested;
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
        let owner = workspace.downgrade();
        let worker_request = request.clone();
        let worker_route = route.clone();
        let serial = workspace.read(cx).device(cx).serial_number.clone();
        let local_profile_baseline = workspace.read(cx).device(cx).clone();
        let delivery = cx.spawn(async move |shell, cx| {
            let completion = cx
                .background_spawn(async move {
                    let _accepted = accepted;
                    worker::run(worker_route, worker_request, serial)
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
                let exit_task_failed = operation.request.can_finish_during_exit()
                    && (!current
                        || !completion
                            .result()
                            .as_ref()
                            .is_ok_and(|report| report.verified));
                if let Some(workspace) = operation.workspace.upgrade() {
                    let result = if current {
                        completion.result()
                    } else {
                        Err("DPI 设备连接或请求范围已改变".to_owned())
                    };
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
                        workspace.finish_current_dpi_result(&operation.request, result, cx)
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
            },
        );
    }
}

#[cfg(test)]
#[path = "mouse_current_dpi_runtime_tests.rs"]
mod tests;

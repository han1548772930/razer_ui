//! Accepted wheel tasks and their cleanup outlive the presenting window.
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
    request: MouseScrollRequested,
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
                .mouse_scroll_request_matches(&self.request, cx)
            && current_route(&self.route, observations, workspace.read(cx).device(cx))
    }
}

fn current_route(route: &ObservedDevice, observations: &[ObservedDevice], device: &Device) -> bool {
    crate::shell::device_discovery::retained_connection(
        std::slice::from_ref(route),
        observations,
        device,
    ) && direct::mouse_scroll::available(route)
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
    fn resume_after_exit(&mut self) {
        if !self.pending() {
            self.stopped = false;
        }
    }

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
            .expect("Scroll operation identity exhausted");
        self.accepted.fetch_add(1, Ordering::AcqRel);
        Some((self.next_id, Accepted(self.accepted.clone())))
    }

    fn take_for_shutdown(&mut self) -> Vec<Task<()>> {
        self.stopped = true;
        std::mem::take(&mut self.operations)
            .into_values()
            .map(|operation| {
                operation.request.cancel();
                operation.delivery
            })
            .collect()
    }
}

impl Drop for State {
    fn drop(&mut self) {
        for operation in self.operations.values() {
            operation.request.cancel();
        }
    }
}

impl AppShell {
    pub(in crate::shell) fn install_mouse_scroll_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_resume(|shell, _| shell.mouse_scroll.resume_after_exit());
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.mouse_scroll.take_for_shutdown();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
    }

    pub(in crate::shell) fn retain_mouse_scroll_connections(
        &self,
        observations: &[ObservedDevice],
        cx: &App,
    ) {
        for operation in self.mouse_scroll.operations.values() {
            if !operation.matches(self, observations, cx) {
                operation.request.cancel();
            }
        }
    }

    pub(super) fn start_mouse_scroll_operation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        route: ObservedDevice,
        request: MouseScrollRequested,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let Some((id, accepted)) = self.mouse_scroll.accept() else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_scroll(&request, cx)
            });
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
                    razer_service::mouse_scroll::device(
                        worker_route,
                        worker_request.submission().clone(),
                        worker_request.guard(),
                    )
                })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                if let Some(operation) = shell.mouse_scroll.operations.remove(&id) {
                    let current = operation.matches(shell, &shell.device_observations, cx);
                    if let Some(workspace) = operation.workspace.upgrade() {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_mouse_scroll_report(
                                &operation.request,
                                current.then(|| completion.result.as_ref().ok()).flatten(),
                                completion.cleanup_failure.is_none(),
                                cx,
                            )
                        });
                        if current {
                            let _ = operation.window.update(cx, |_, window, cx| {
                                workspace.update(cx, |workspace, cx| {
                                    workspace.sync_mouse_scroll_controls(window, cx)
                                });
                            });
                        }
                    }
                    if current {
                        shell.status = match &completion.result {
                            Ok(report) if operation.request.is_read() => {
                                if report
                                    .current
                                    .as_ref()
                                    .is_some_and(|current| current.is_complete())
                                {
                                    "滚轮设备状态已读取。".into()
                                } else {
                                    "滚轮设备状态未全部读取；失败及未知值已保留。".into()
                                }
                            }
                            Ok(report) if report.verified_for(operation.route.product_id()) => {
                                "滚轮设置已保存，设备写入及读回已确认。".into()
                            }
                            Ok(report) => {
                                let errors = report.errors.join("；");
                                if errors.is_empty() {
                                    "滚轮写入尚未全部确认；实际读回已保留。".into()
                                } else {
                                    format!("滚轮写入尚未全部确认：{errors}；实际读回已保留。")
                                }
                            }
                            Err(error) => {
                                format!("滚轮设备状态尚未确认：{error}；已保留当前设置。")
                            }
                        };
                    }
                }
                if let Some(error) = &completion.cleanup_failure {
                    eprintln!("[mouse scroll cleanup] {error}");
                    shell.status.push_str(&format!(" 通信清理失败：{error}"));
                    shell.cancel_pending_exit(cx);
                }
                if shell.close_requested && shell.save_task.is_none() {
                    shell.continue_save_queue(completion.cleanup_failure.is_none(), cx);
                }
                cx.notify();
            });
        });
        self.mouse_scroll.operations.insert(
            id,
            Operation {
                workspace: owner,
                identity,
                route,
                request,
                window,
                delivery,
            },
        );
    }
}

#[cfg(test)]
#[path = "mouse_scroll_runtime_tests.rs"]
mod tests;

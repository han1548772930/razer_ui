//! Stage-table workers remain owned until their transport has been released.
use super::*;
use razer_discovery::discovery::ObservedDevice;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Clone)]
pub(super) struct Request {
    pub(super) scope: MousePollingScope,
    pub(super) revision: u64,
    pub(super) draft: Option<DpiStagesDraft>,
    pub(super) valid: Arc<AtomicBool>,
}

impl Request {
    pub(super) fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }

    fn token_matches(&self, workspace: &ProductWorkspace, cx: &App) -> bool {
        let current = match &self.draft {
            Some(draft) => workspace.mouse_dpi_write_guard(self.scope, self.revision, draft, cx),
            None => workspace.mouse_dpi_read_guard(self.scope, self.revision, cx),
        };
        current.is_some_and(|current| Arc::ptr_eq(&self.valid, &current))
    }

    fn scope_matches(&self, workspace: &ProductWorkspace, cx: &App) -> bool {
        self.token_matches(workspace, cx)
            && match &self.draft {
                Some(draft) => {
                    workspace.mouse_dpi_request_matches(self.scope, self.revision, draft, cx)
                }
                None => workspace.mouse_dpi_read_matches(self.scope, self.revision, cx),
            }
    }

    fn observation_matches(&self, workspace: &ProductWorkspace, cx: &App) -> bool {
        self.is_valid()
            && self.token_matches(workspace, cx)
            && match &self.draft {
                Some(draft) => {
                    workspace.mouse_dpi_observation_matches(self.scope, self.revision, draft, cx)
                }
                None => workspace.mouse_dpi_read_matches(self.scope, self.revision, cx),
            }
    }

    fn finish(
        &self,
        workspace: &Entity<ProductWorkspace>,
        observed: Option<DpiStagesReading>,
        cx: &mut App,
    ) {
        if !self.token_matches(workspace.read(cx), cx) {
            return;
        }
        workspace.update(cx, |workspace, cx| match &self.draft {
            Some(draft) => {
                workspace.finish_mouse_dpi(self.scope, self.revision, draft, observed, cx)
            }
            None => workspace.finish_mouse_dpi_read(self.scope, self.revision, observed, cx),
        });
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
        shell.devices.contains(&workspace)
            && workspace.read(cx).identity(cx) == self.identity
            && self.request.scope_matches(workspace.read(cx), cx)
            && current_route(&self.route, observations, workspace.read(cx).device(cx))
    }
}

fn current_route(route: &ObservedDevice, observations: &[ObservedDevice], device: &Device) -> bool {
    crate::shell::device_discovery::retained_connection(
        std::slice::from_ref(route),
        observations,
        device,
    ) && direct::mouse_dpi_stages_available(route)
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
            .expect("DPI stage operation identity exhausted");
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

    fn resume_after_exit(&mut self) {
        self.stopped = false;
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
    pub(in crate::shell) fn install_mouse_dpi_stages_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_resume(|shell, _| shell.mouse_dpi_stages.resume_after_exit());
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.mouse_dpi_stages.take_for_shutdown();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
    }

    pub(in crate::shell) fn retain_mouse_dpi_stages_connections(
        &self,
        observations: &[ObservedDevice],
        cx: &App,
    ) {
        for operation in self.mouse_dpi_stages.operations.values() {
            if !operation.matches(self, observations, cx) {
                operation.request.valid.store(false, Ordering::Release);
            }
        }
    }

    pub(super) fn start_mouse_dpi_stages_operation(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        route: ObservedDevice,
        request: Request,
        cx: &mut Context<Self>,
    ) {
        let Some((id, accepted)) = self.mouse_dpi_stages.accept() else {
            request.finish(&workspace, None, cx);
            return;
        };
        let identity = workspace.read(cx).identity(cx);
        self.device_value_owners.remove(&identity);
        self.device_read_scopes.remove(&identity);
        self.status = if request.draft.is_some() {
            "正在写入完整 DPI 阶段表并读取设备确认。"
        } else {
            "正在读取设备 DPI 阶段表。"
        }
        .into();
        let worker_route = route.clone();
        let worker_request = request.clone();
        let delivery = cx.spawn(async move |shell, cx| {
            let completion = cx
                .background_spawn(async move {
                    let _accepted = accepted;
                    worker::run(worker_route, worker_request)
                })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                let Some(operation) = shell.mouse_dpi_stages.operations.remove(&id) else {
                    return;
                };
                let workspace = operation.workspace.upgrade();
                let current = operation.matches(shell, &shell.device_observations, cx)
                    && workspace.as_ref().is_some_and(|workspace| {
                        operation
                            .request
                            .observation_matches(workspace.read(cx), cx)
                    });
                if let Some(workspace) = workspace {
                    let observed = current.then(|| completion.reading().cloned()).flatten();
                    operation.request.finish(&workspace, observed, cx);
                }
                if current {
                    shell.device_value_owners.remove(&operation.identity);
                    shell.device_read_scopes.remove(&operation.identity);
                    shell.status = completion.message(operation.request.draft.is_some(), true);
                } else if completion.cleanup_error().is_some() {
                    shell.status = completion.message(operation.request.draft.is_some(), false);
                }
                let released = completion.cleanup_error().is_none();
                if !released {
                    shell.cancel_pending_exit(cx);
                }
                if shell.close_requested && shell.save_task.is_none() {
                    shell.continue_save_queue(released, cx);
                }
                cx.notify();
            });
        });
        self.mouse_dpi_stages.operations.insert(
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
#[path = "mouse_dpi_stages_runtime_tests.rs"]
mod tests;

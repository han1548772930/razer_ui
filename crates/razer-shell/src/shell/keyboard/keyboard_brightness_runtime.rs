//! Retained brightness requests share the page's cancellation scope. Accepted
//! work stays visible to exit through background execution and service cleanup.
use super::*;
use razer_discovery::{direct, discovery::ObservedDevice};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

#[path = "keyboard_brightness_worker.rs"]
mod worker;
pub(super) use worker::Completion;

#[derive(Clone)]
pub(super) struct Scope {
    workspace: WeakEntity<ProductWorkspace>,
    identity: String,
    profile: String,
    route: ObservedDevice,
}

impl Scope {
    pub(super) fn new(
        workspace: &Entity<ProductWorkspace>,
        route: ObservedDevice,
        cx: &App,
    ) -> Self {
        Self {
            workspace: workspace.downgrade(),
            identity: workspace.read(cx).identity(cx),
            profile: workspace.read(cx).device(cx).active_profile.clone(),
            route,
        }
    }

    fn matches(&self, shell: &AppShell, observations: &[ObservedDevice], cx: &App) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        shell.devices.contains(&workspace)
            && workspace.read(cx).identity(cx) == self.identity
            && workspace.read(cx).device(cx).active_profile == self.profile
            && current_route(&self.route, observations, workspace.read(cx).device(cx))
    }

    pub(super) fn current(&self, shell: &AppShell, canceled: &AtomicBool, cx: &App) -> bool {
        !canceled.load(Ordering::Acquire) && self.matches(shell, &shell.device_observations, cx)
    }
}

fn current_route(route: &ObservedDevice, observations: &[ObservedDevice], device: &Device) -> bool {
    super::device_discovery::retained_connection(std::slice::from_ref(route), observations, device)
        && direct::keyboard_brightness_available(route)
}

struct RetainedOperation {
    scope: Scope,
    canceled: Arc<AtomicBool>,
    delivery: Task<()>,
}

#[derive(Default)]
pub(super) struct State {
    lifecycle: Lifecycle,
    next_id: u64,
    accepted: Arc<AtomicUsize>,
    operations: BTreeMap<u64, RetainedOperation>,
}

#[derive(Default)]
enum Lifecycle {
    #[default]
    Running,
    Stopped,
}

/// Created before scheduling, including the executor's unpolled queue. Its
/// final owner is the background future which also owns ServiceClient cleanup.
struct AcceptedOperation(Arc<AtomicUsize>);

impl AcceptedOperation {
    fn new(accepted: &Arc<AtomicUsize>) -> Self {
        accepted.fetch_add(1, Ordering::AcqRel);
        Self(accepted.clone())
    }
}

impl Drop for AcceptedOperation {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl State {
    pub(super) fn pending(&self) -> bool {
        !self.operations.is_empty() || self.accepted.load(Ordering::Acquire) > 0
    }

    fn finish(&mut self, id: u64, canceled: &Arc<AtomicBool>) {
        if self
            .operations
            .get(&id)
            .is_some_and(|operation| Arc::ptr_eq(&operation.canceled, canceled))
        {
            self.operations.remove(&id);
        }
    }

    fn accept(&mut self) -> Option<(u64, AcceptedOperation)> {
        if matches!(self.lifecycle, Lifecycle::Stopped) {
            return None;
        }
        self.next_id = self.next_id.wrapping_add(1);
        Some((self.next_id, AcceptedOperation::new(&self.accepted)))
    }

    fn take_for_shutdown(&mut self) -> Vec<Task<()>> {
        self.lifecycle = Lifecycle::Stopped;
        std::mem::take(&mut self.operations)
            .into_values()
            .map(|operation| {
                operation.canceled.store(true, Ordering::Release);
                operation.delivery
            })
            .collect()
    }
}

impl Drop for State {
    fn drop(&mut self) {
        for operation in self.operations.values() {
            operation.canceled.store(true, Ordering::Release);
        }
    }
}

impl AppShell {
    pub(super) fn install_keyboard_brightness_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.keyboard_brightness.take_for_shutdown();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
    }

    pub(super) fn retain_keyboard_brightness_connections(
        &mut self,
        observations: &[ObservedDevice],
        cx: &App,
    ) {
        for operation in self.keyboard_brightness.operations.values() {
            if !operation.scope.matches(self, observations, cx) {
                operation.canceled.store(true, Ordering::Release);
            }
        }
    }

    pub(super) fn start_keyboard_brightness_operation(
        &mut self,
        scope: Scope,
        percent: Option<u8>,
        canceled: Arc<AtomicBool>,
        complete: impl FnOnce(&mut Self, Completion, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some((id, accepted)) = self.keyboard_brightness.accept() else {
            canceled.store(true, Ordering::Release);
            complete(
                self,
                Completion::failed("键盘亮度通信所有者正在关闭".into()),
                cx,
            );
            return;
        };
        let signal = canceled.clone();
        let route = scope.route.clone();
        let delivery = cx.spawn(async move |shell, cx| {
            let work_signal = signal.clone();
            let completion = cx
                .background_spawn(async move {
                    let _accepted = accepted;
                    worker::run(route, percent, work_signal)
                })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                shell.keyboard_brightness.finish(id, &signal);
                let cleanup_error = completion.cleanup_error();
                if let Some(error) = &cleanup_error {
                    eprintln!("[keyboard brightness cleanup] {error}");
                }
                complete(shell, completion, cx);
                if shell.close_requested {
                    if let Some(error) = &cleanup_error {
                        shell.status = format!("键盘亮度通信清理失败：{error}");
                    }
                    shell.continue_save_queue(cleanup_error.is_none(), cx);
                }
            });
        });
        self.keyboard_brightness.operations.insert(
            id,
            RetainedOperation {
                scope,
                canceled,
                delivery,
            },
        );
    }
}

#[cfg(test)]
#[path = "keyboard_brightness_runtime_tests.rs"]
mod tests;

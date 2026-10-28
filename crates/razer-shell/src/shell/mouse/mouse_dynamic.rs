//! Dynamic source tasks: durable optimistic profile, serialized setters and a
//! separately retained native input-monitor lifetime.
use super::*;
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_pages::features::mouse_products::{
    MouseDynamicAction, MouseDynamicCompletion, MouseDynamicConnection, MouseDynamicRequested,
};
use razer_service::mouse_dynamic::{self as service, Completion, Outcome};
use std::sync::{Arc, Mutex, atomic::Ordering};

#[path = "mouse_dynamic_save.rs"]
mod save;
pub(super) use save::LocalSave;
#[path = "mouse_dynamic_history.rs"]
mod history;
use history::HistoryOwner;

#[derive(Clone, Copy)]
enum Admission {
    New,
    AcceptedWrite,
}
impl Admission {
    fn queued(request: &MouseDynamicRequested) -> Self {
        if matches!(request.action(), Ok(MouseDynamicAction::Write(_))) {
            Self::AcceptedWrite
        } else {
            Self::New
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    identity: String,
    monitor: bool,
}
struct Queued {
    workspace: Entity<ProductWorkspace>,
    request: MouseDynamicRequested,
    route: ObservedDevice,
    revision: u64,
    window: AnyWindowHandle,
}
struct Operation {
    id: u64,
    workspace: WeakEntity<ProductWorkspace>,
    request: MouseDynamicRequested,
    route: ObservedDevice,
    revision: u64,
    control: Option<service::MonitorControl>,
    delivery: Task<()>,
}
struct PendingStart {
    request: MouseDynamicRequested,
    delivery: Task<()>,
}
#[derive(Default)]
pub(super) struct State {
    stopped: bool,
    next_id: u64,
    operations: BTreeMap<Key, Operation>,
    queued: BTreeMap<Key, Queued>,
    histories: BTreeMap<u64, HistoryOwner>,
    starts: BTreeMap<u64, PendingStart>,
}
impl State {
    fn allocate_id(&mut self) -> u64 {
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("Dynamic operation identity exhausted");
        self.next_id
    }
    fn resume_after_exit(&mut self) {
        if !self.pending() {
            self.stopped = false;
        }
    }

    fn admits(&self, closing: bool, admission: Admission) -> bool {
        !self.stopped && (!closing || matches!(admission, Admission::AcceptedWrite))
    }
    pub(super) fn pending(&self) -> bool {
        self.operations
            .keys()
            .chain(self.queued.keys())
            .any(|key| !key.monitor)
    }
    fn take_for_shutdown(&mut self) -> Vec<Task<()>> {
        self.stopped = true;
        for queued in std::mem::take(&mut self.queued).into_values() {
            queued.request.guard().store(false, Ordering::Release);
        }
        let mut deliveries = std::mem::take(&mut self.operations)
            .into_values()
            .map(|operation| {
                operation.request.guard().store(false, Ordering::Release);
                operation.delivery
            })
            .collect::<Vec<_>>();
        for start in std::mem::take(&mut self.starts).into_values() {
            start.request.guard().store(false, Ordering::Release);
            deliveries.push(start.delivery);
        }
        for history in std::mem::take(&mut self.histories).into_values() {
            history.input.close();
            deliveries.push(history.delivery);
        }
        deliveries
    }
}
impl Drop for State {
    fn drop(&mut self) {
        for operation in self.operations.values() {
            operation.request.guard().store(false, Ordering::Release);
        }
        for history in self.histories.values() {
            history.input.close();
        }
        for start in self.starts.values() {
            start.request.guard().store(false, Ordering::Release);
        }
    }
}

#[derive(Default)]
struct Mailbox {
    started: bool,
    completion: Option<Completion>,
}

impl Operation {
    fn matches(
        &self,
        shell: &AppShell,
        key: &Key,
        observations: &[ObservedDevice],
        cx: &App,
    ) -> bool {
        let Some(workspace) = self.workspace.upgrade() else {
            return false;
        };
        current(
            shell,
            &workspace,
            &self.request,
            key,
            &self.route,
            self.revision,
            observations,
            cx,
        )
    }
}
fn current(
    shell: &AppShell,
    workspace: &Entity<ProductWorkspace>,
    request: &MouseDynamicRequested,
    key: &Key,
    route: &ObservedDevice,
    revision: u64,
    observations: &[ObservedDevice],
    cx: &App,
) -> bool {
    shell.devices.contains(workspace)
        && workspace.read(cx).identity(cx) == key.identity
        && workspace.read(cx).connection_revision() == revision
        && workspace
            .read(cx)
            .mouse_dynamic_scope_is_current(request, cx)
        && (key.monitor
            || workspace
                .read(cx)
                .mouse_dynamic_request_is_current(request, cx))
        && super::device_discovery::retained_connection(
            std::slice::from_ref(route),
            observations,
            workspace.read(cx).device(cx),
        )
        && direct::mouse_dynamic::available(route)
}

impl AppShell {
    fn finish_mouse_dynamic(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &MouseDynamicRequested,
        completion: MouseDynamicCompletion,
        cx: &mut Context<Self>,
    ) {
        let diagnostic = workspace.update(cx, |workspace, cx| {
            workspace.complete_mouse_dynamic(request, completion, cx)
        });
        if let Some(diagnostic) = diagnostic {
            self.status = diagnostic;
            cx.notify();
        }
    }

    pub(super) fn install_mouse_dynamic_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.register_exit_resume(|shell, _| shell.mouse_dynamic.resume_after_exit());
        self.register_exit_cleanup(|shell, _| {
            let deliveries = shell.mouse_dynamic.take_for_shutdown();
            async move {
                for delivery in deliveries {
                    delivery.await;
                }
            }
        });
    }
    pub(super) fn retain_mouse_dynamic_connections(
        &self,
        observations: &[ObservedDevice],
        cx: &App,
    ) {
        for history in self.mouse_dynamic.histories.values() {
            if !history.matches(self, observations, cx) {
                history.input.close();
            }
        }
        for (key, operation) in &self.mouse_dynamic.operations {
            if !operation.matches(self, key, observations, cx) {
                operation.request.guard().store(false, Ordering::Release);
            }
        }
        for (key, queued) in &self.mouse_dynamic.queued {
            if !current(
                self,
                &queued.workspace,
                &queued.request,
                key,
                &queued.route,
                queued.revision,
                observations,
                cx,
            ) {
                queued.request.guard().store(false, Ordering::Release);
            }
        }
    }
    pub(super) fn dispatch_mouse_dynamic(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: &MouseDynamicRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .mouse_dynamic_request_is_current(request, cx)
        {
            return;
        }
        if !self
            .mouse_dynamic
            .admits(self.close_requested, Admission::New)
        {
            self.finish_mouse_dynamic(&workspace, request, MouseDynamicCompletion::Cancelled, cx);
            return;
        }
        match request.action() {
            Ok(MouseDynamicAction::Write(_)) => {
                self.persist_mouse_dynamic(workspace, request.clone(), window, cx)
            }
            Ok(_) => self.submit_mouse_dynamic(
                workspace,
                request.clone(),
                Admission::New,
                window.window_handle(),
                cx,
            ),
            Err(error) => self.finish_mouse_dynamic(
                &workspace,
                request,
                MouseDynamicCompletion::Failed(format!("{error:#}")),
                cx,
            ),
        }
    }
    fn submit_mouse_dynamic(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseDynamicRequested,
        admission: Admission,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !self.mouse_dynamic.admits(self.close_requested, admission)
            || !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .mouse_dynamic_request_is_current(&request, cx)
        {
            self.finish_mouse_dynamic(&workspace, &request, MouseDynamicCompletion::Cancelled, cx);
            return;
        }
        let action = match request.action() {
            Ok(action) => action,
            Err(error) => {
                self.finish_mouse_dynamic(
                    &workspace,
                    &request,
                    MouseDynamicCompletion::Failed(format!("{error:#}")),
                    cx,
                );
                return;
            }
        };
        let key = Key {
            identity: workspace.read(cx).identity(cx),
            monitor: matches!(action, MouseDynamicAction::Monitor(_)),
        };
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && direct::mouse_dynamic::available(route)
            })
            .cloned()
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            self.finish_mouse_dynamic(
                &workspace,
                &request,
                MouseDynamicCompletion::Failed(
                    "Dynamic sensitivity requires one current mouse or receiver route".into(),
                ),
                cx,
            );
            return;
        };
        let queued = Queued {
            revision: workspace.read(cx).connection_revision(),
            workspace,
            request,
            route: route.clone(),
            window,
        };
        if matches!(action, MouseDynamicAction::Monitor(false)) {
            self.unregister_mouse_dynamic_observer(&queued, cx);
        }
        if route.product_id() == 226
            && matches!(action, MouseDynamicAction::Monitor(true))
            && self
                .mouse_dynamic
                .operations
                .get(&key)
                .is_some_and(|operation| {
                    operation.request.guard().load(Ordering::Acquire)
                        && operation.matches(self, &key, &self.device_observations, cx)
                })
        {
            self.repeat_mouse_dynamic_start(key, queued, cx);
            return;
        }
        if let Some(operation) = self.mouse_dynamic.operations.get(&key) {
            if key.monitor {
                operation.request.guard().store(false, Ordering::Release);
            }
            if let Some(skipped) = self.mouse_dynamic.queued.insert(key, queued) {
                self.finish_mouse_dynamic(
                    &skipped.workspace,
                    &skipped.request,
                    MouseDynamicCompletion::Cancelled,
                    cx,
                );
            }
        } else if matches!(action, MouseDynamicAction::Monitor(false)) {
            // This owner never admitted a native START; there is no outstanding
            // monitor obligation to acknowledge or release.
            self.finish_mouse_dynamic(
                &queued.workspace,
                &queued.request,
                MouseDynamicCompletion::Monitoring(false),
                cx,
            );
        } else {
            self.start_mouse_dynamic(key, queued, cx);
        }
    }
    fn repeat_mouse_dynamic_start(&mut self, key: Key, queued: Queued, cx: &mut Context<Self>) {
        // Source j registers F before awaiting the raw START result. Neither a
        // negative acknowledgement nor an RPC failure removes that listener.
        let reply =
            self.register_mouse_dynamic_observer(&queued, cx)
                .and_then(|()| {
                    let operation =
                        self.mouse_dynamic.operations.get(&key).ok_or_else(|| {
                            anyhow::anyhow!("Dynamic capture is no longer retained")
                        })?;
                    let control = operation
                        .control
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Dynamic capture has no START control"))?;
                    Ok((operation.id, control.request_start()?))
                });
        let (capture_id, reply) = match reply {
            Ok(reply) => reply,
            Err(error) => {
                self.finish_mouse_dynamic(
                    &queued.workspace,
                    &queued.request,
                    MouseDynamicCompletion::Failed(format!("{error:#}")),
                    cx,
                );
                return;
            }
        };
        let id = self.mouse_dynamic.allocate_id();
        let request = queued.request.clone();
        let workspace = queued.workspace.downgrade();
        let delivery = cx.spawn(async move |shell, cx| {
            let acknowledgement = reply.receive().await;
            let _ = shell.update(cx, |shell, cx| {
                if shell.mouse_dynamic.starts.remove(&id).is_none() {
                    return;
                }
                let current = shell
                    .mouse_dynamic
                    .operations
                    .get(&key)
                    .is_some_and(|operation| {
                        operation.id == capture_id
                            && operation.matches(shell, &key, &shell.device_observations, cx)
                    });
                let Some(workspace) = workspace.upgrade().filter(|workspace| {
                    current
                        && workspace
                            .read(cx)
                            .mouse_dynamic_request_is_current(&request, cx)
                }) else {
                    return;
                };
                let completion = match acknowledgement {
                    // j ignores the raw result boolean. Retire this registration
                    // without publishing another Monitoring(true) success.
                    Ok(_) => MouseDynamicCompletion::ObserverAttached,
                    Err(error) => MouseDynamicCompletion::Failed(error),
                };
                shell.finish_mouse_dynamic(&workspace, &request, completion, cx);
            });
        });
        self.mouse_dynamic.starts.insert(
            id,
            PendingStart {
                request: queued.request,
                delivery,
            },
        );
    }
    fn start_mouse_dynamic(&mut self, key: Key, queued: Queued, cx: &mut Context<Self>) {
        if !current(
            self,
            &queued.workspace,
            &queued.request,
            &key,
            &queued.route,
            queued.revision,
            &self.device_observations,
            cx,
        ) {
            self.finish_mouse_dynamic(
                &queued.workspace,
                &queued.request,
                MouseDynamicCompletion::Cancelled,
                cx,
            );
            return;
        }
        let action = match queued.request.action() {
            Ok(action) => action,
            Err(error) => {
                self.finish_mouse_dynamic(
                    &queued.workspace,
                    &queued.request,
                    MouseDynamicCompletion::Failed(format!("{error:#}")),
                    cx,
                );
                return;
            }
        };
        let history = if matches!(action, MouseDynamicAction::Monitor(true)) {
            match self.mouse_dynamic_history(&queued, cx) {
                Ok(history) => Some(history),
                Err(error) => {
                    self.finish_mouse_dynamic(
                        &queued.workspace,
                        &queued.request,
                        MouseDynamicCompletion::Failed(format!("{error:#}")),
                        cx,
                    );
                    return;
                }
            }
        } else {
            None
        };
        let id = self.mouse_dynamic.allocate_id();
        let (control, capture) = match history {
            Some(history) => {
                let (control, commands) = service::monitor_requests();
                (Some(control), Some((history, commands)))
            }
            None => (None, None),
        };
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let output = mailbox.clone();
        let route = queued.route.clone();
        let guard = queued.request.guard();
        let worker = cx.background_spawn(async move {
            let completion = if let Some((history, commands)) = capture {
                service::monitor(route, guard, history, commands, || {
                    let mut mailbox = output.lock().unwrap_or_else(|error| error.into_inner());
                    mailbox.started = true;
                })
            } else {
                service::device(
                    route,
                    match action {
                        MouseDynamicAction::Write(setting) => Some(setting),
                        _ => None,
                    },
                    guard,
                )
            };
            output
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .completion = Some(completion);
        });
        let delivery_key = key.clone();
        let delivery_request = queued.request.clone();
        let delivery_owner = queued.workspace.downgrade();
        let delivery = cx.spawn(async move |shell, cx| {
            let completion = loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                let batch = mailbox
                    .try_lock()
                    .ok()
                    .map(|mut mailbox| std::mem::take(&mut *mailbox));
                let Some(batch) = batch else { continue };
                let _ = shell.update(cx, |shell, cx| {
                    let current = shell
                        .mouse_dynamic
                        .operations
                        .get(&delivery_key)
                        .is_some_and(|operation| {
                            operation.id == id
                                && operation.matches(
                                    shell,
                                    &delivery_key,
                                    &shell.device_observations,
                                    cx,
                                )
                        });
                    if !current {
                        delivery_request.guard().store(false, Ordering::Release);
                        return;
                    }
                    let Some(workspace) = delivery_owner.upgrade() else {
                        return;
                    };
                    if batch.started {
                        shell.finish_mouse_dynamic(
                            &workspace,
                            &delivery_request,
                            MouseDynamicCompletion::Monitoring(true),
                            cx,
                        );
                    }
                });
                if let Some(completion) = batch.completion {
                    break completion;
                }
            };
            worker.await;
            let _ = shell.update(cx, |shell, cx| {
                let operation = match shell.mouse_dynamic.operations.remove(&delivery_key) {
                    Some(operation) if operation.id == id => Some(operation),
                    Some(successor) => {
                        shell
                            .mouse_dynamic
                            .operations
                            .insert(delivery_key.clone(), successor);
                        None
                    }
                    None => None,
                };
                let clean = completion.cleanup_failure.is_none();
                if let Some(operation) = operation {
                    let current =
                        operation.matches(shell, &delivery_key, &shell.device_observations, cx);
                    if current {
                        if let Some(workspace) = operation.workspace.upgrade() {
                            let result = match completion.result {
                                Ok(Outcome::Device(report)) => {
                                    MouseDynamicCompletion::Device(report)
                                }
                                Ok(Outcome::MonitoringStopped) => {
                                    MouseDynamicCompletion::Monitoring(false)
                                }
                                Err(error) => MouseDynamicCompletion::Failed(error),
                            };
                            shell.finish_mouse_dynamic(&workspace, &operation.request, result, cx);
                        }
                    }
                    if let Some(next) = shell.mouse_dynamic.queued.remove(&delivery_key) {
                        if !clean
                            || matches!(
                                next.request.action(),
                                Ok(MouseDynamicAction::Monitor(false))
                            )
                        {
                            shell.finish_mouse_dynamic(
                                &next.workspace,
                                &next.request,
                                match &completion.cleanup_failure {
                                    None => MouseDynamicCompletion::Monitoring(false),
                                    Some(error) => MouseDynamicCompletion::Failed(error.clone()),
                                },
                                cx,
                            );
                        } else if !shell
                            .mouse_dynamic
                            .admits(shell.close_requested, Admission::queued(&next.request))
                        {
                            shell.finish_mouse_dynamic(
                                &next.workspace,
                                &next.request,
                                MouseDynamicCompletion::Cancelled,
                                cx,
                            );
                        } else {
                            shell.start_mouse_dynamic(delivery_key, next, cx);
                        }
                    }
                }
                // Final shutdown takes the operation before awaiting delivery.
                // Its cleanup receipt still belongs to the application owner.
                if let Some(error) = &completion.cleanup_failure {
                    eprintln!("[dynamic cleanup] {error}");
                    shell
                        .status
                        .push_str(&format!(" Dynamic 通信清理失败：{error}"));
                    shell.cancel_pending_exit(cx);
                }
                if shell.close_requested && shell.save_task.is_none() {
                    shell.continue_save_queue(clean, cx);
                }
                cx.notify();
            });
        });
        self.mouse_dynamic.operations.insert(
            key,
            Operation {
                id,
                workspace: queued.workspace.downgrade(),
                request: queued.request,
                route: queued.route,
                revision: queued.revision,
                control,
                delivery,
            },
        );
    }
}

#[cfg(test)]
#[path = "mouse_dynamic_lifecycle_tests.rs"]
mod lifecycle_tests;

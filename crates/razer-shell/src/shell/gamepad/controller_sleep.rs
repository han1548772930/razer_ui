//! Current root Power storage -> independent real SET/GET owner.
use super::*;
use razer_device::controller_sleep::{SleepCompletion, SleepResult};
use razer_ipc::{
    ServiceClient, ServiceRequest, ShutdownConfirmation, controller_sleep::SleepResponse,
};
use razer_pages::features::gamepad_products::ControllerSleepRequested;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
pub(super) struct CleanupState {
    pub(super) entries: BTreeMap<u64, CleanupEntry>,
    pub(super) next_id: u64,
    pub(super) last_failure: Option<String>,
}
pub(super) struct CleanupEntry {
    pub(super) owner: CleanupOwner,
    pub(super) progress: Arc<CleanupProgress>,
    pub(super) _operation: Task<()>,
    pub(super) _status: Task<()>,
}
impl CleanupState {
    pub(super) fn pending(&self) -> bool {
        !self.entries.is_empty()
    }
    pub(super) fn blocked(&self, owner: &CleanupOwner) -> bool {
        self.entries
            .values()
            .any(|entry| entry.owner.blocks(owner, &entry.progress))
    }
    fn retry(&self) {
        for entry in self.entries.values() {
            entry.progress.retry();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CleanupOwner {
    workspace: String,
    logical_product_id: u32,
    container: String,
    scope: razer_ipc::device_tasks::Scope,
    physical_product_id: u32,
    retained_node: Option<razer_device::backend::HidNode>,
}
impl CleanupOwner {
    pub(super) fn from_route(
        workspace: String,
        route: &razer_discovery::discovery::ObservedDevice,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            workspace,
            logical_product_id: route.product_id(),
            container: route.container().to_owned(),
            scope: route.device_task_scope()?,
            physical_product_id: route.physical_product_id(),
            retained_node: route.hid_node().cloned(),
        })
    }
    pub(super) fn same_device(&self, other: &Self) -> bool {
        self.workspace.eq_ignore_ascii_case(&other.workspace)
            && self.logical_product_id == other.logical_product_id
            && self.container.eq_ignore_ascii_case(&other.container)
            && self.scope == other.scope
            && self.physical_product_id == other.physical_product_id
            && self.retained_node == other.retained_node
    }
    fn same_resource(&self, other: &Self) -> bool {
        self.workspace.eq_ignore_ascii_case(&other.workspace)
            && self.logical_product_id == other.logical_product_id
            && self.physical_product_id == other.physical_product_id
            && self.container.eq_ignore_ascii_case(&other.container)
            && (self.scope == other.scope
                || (self.retained_node.is_none() || other.retained_node.is_none())
                    && !self.container.is_empty())
    }
    fn blocks(&self, candidate: &Self, progress: &CleanupProgress) -> bool {
        self.same_resource(candidate) && progress.blocked()
    }
}

#[derive(Clone, Default)]
pub(super) struct CleanupGate {
    client_acquired: bool,
    shutdown_request_id: Option<u64>,
    process_released: bool,
    finished: bool,
}
impl CleanupGate {
    fn acquired(&mut self) {
        self.client_acquired = true;
    }
    fn confirmed(&mut self, request_id: u64) {
        self.shutdown_request_id = Some(request_id);
    }
    pub(super) fn safe_to_release(&self) -> bool {
        self.finished
            && (!self.client_acquired
                || self.shutdown_request_id.is_some() && self.process_released)
    }
}

#[derive(Default)]
pub(super) struct CleanupProgress {
    state: Mutex<CleanupSnapshot>,
    wake: Condvar,
}
#[derive(Clone, Default)]
pub(super) struct CleanupSnapshot {
    operation: Option<Result<SleepResult, String>>,
    failures: Vec<String>,
    revision: u64,
    retry: bool,
    pub(super) gate: CleanupGate,
}
impl CleanupSnapshot {
    pub(super) fn finished(&self) -> bool {
        self.gate.finished
    }
    pub(super) fn failure(&self) -> Option<String> {
        (!self.failures.is_empty()).then(|| self.failures.join("; "))
    }
}
impl CleanupProgress {
    fn lock(&self) -> std::sync::MutexGuard<'_, CleanupSnapshot> {
        match self.state.lock() {
            Ok(state) => state,
            Err(error) => {
                let mut state = error.into_inner();
                let diagnostic = "Controller sleep cleanup state lock failed".to_owned();
                if !state.failures.contains(&diagnostic) {
                    state.failures.push(diagnostic);
                }
                state.revision = state.revision.wrapping_add(1);
                state
            }
        }
    }
    pub(super) fn snapshot(&self) -> CleanupSnapshot {
        self.lock().clone()
    }
    fn blocked(&self) -> bool {
        !self.lock().gate.safe_to_release()
    }
    fn retry(&self) {
        let mut state = self.lock();
        if state.gate.process_released || state.gate.finished {
            return;
        }
        state.retry = true;
        self.wake.notify_all();
    }
    pub(super) fn acquired(&self) {
        self.lock().gate.acquired();
    }
    pub(super) fn confirmation(&self, confirmation: Option<ShutdownConfirmation>) {
        if let Some(confirmation) = confirmation {
            self.lock().gate.confirmed(confirmation.request_id());
        }
    }
    fn operation(&self, operation: &anyhow::Result<SleepResult>) {
        self.lock().operation = Some(
            operation
                .as_ref()
                .map(Clone::clone)
                .map_err(|error| format!("{error:#}")),
        );
    }
    pub(super) fn failed(&self, error: String) {
        let mut state = self.lock();
        if !state.failures.contains(&error) {
            state.failures.push(error);
        }
        state.revision = state.revision.wrapping_add(1);
    }
    fn released(&self) {
        let mut state = self.lock();
        state.gate.process_released = true;
        state.revision = state.revision.wrapping_add(1);
        self.wake.notify_all();
    }
    pub(super) fn finish(&self) -> CleanupSnapshot {
        let mut state = self.lock();
        state.gate.finished = true;
        state.revision = state.revision.wrapping_add(1);
        self.wake.notify_all();
        state.clone()
    }
    fn wait_for_retry(&self) {
        let mut state = self.lock();
        if !state.retry {
            state = match self
                .wake
                .wait_timeout(state, std::time::Duration::from_secs(1))
            {
                Ok((state, _)) => state,
                Err(error) => error.into_inner().0,
            };
        }
        state.retry = false;
    }
}

pub(super) fn retain_termination(
    progress: &CleanupProgress,
    mut cleanup_pending: impl FnMut(),
    mut terminate: impl FnMut() -> anyhow::Result<()>,
) -> Vec<String> {
    let mut failures = Vec::new();
    loop {
        match terminate() {
            Ok(()) => {
                progress.released();
                return failures;
            }
            Err(error) => {
                let error = format!("{error:#}");
                if !failures.contains(&error) {
                    failures.push(error.clone());
                }
                cleanup_pending();
                progress.failed(error);
                progress.wait_for_retry();
            }
        }
    }
}

pub(super) struct ControllerSleepSave {
    workspace: Entity<ProductWorkspace>,
    request: ControllerSleepRequested,
    identity: String,
}
impl ControllerSleepSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

impl AppShell {
    pub(super) fn retry_controller_sleep_cleanup(&mut self, _cx: &mut Context<Self>) {
        self.controller_sleep_cleanup.retry();
    }
    pub(super) fn sync_controller_sleep_recording(&mut self, cx: &mut Context<Self>) {
        let recording = self.receiver_macro_exclusive(cx);
        for workspace in &self.devices {
            workspace.update(cx, |workspace, cx| {
                workspace.observe_controller_sleep_recording(recording, cx)
            });
        }
    }
    pub(super) fn persist_controller_sleep(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ControllerSleepRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh_macro_recorder_storage();
        self.sync_controller_sleep_recording(cx);
        let request = if self.receiver_macro_exclusive(cx) {
            request.without_device_access()
        } else {
            request
        };
        if !request.requires_persistence() {
            self.submit_controller_sleep(workspace, request, window.window_handle(), cx);
            return;
        }
        if self.storage_error.is_some()
            || !workspace
                .read(cx)
                .controller_sleep_request_matches(&request, cx)
        {
            let error = self
                .storage_error
                .as_ref()
                .map(|error| format!("休眠设置本地保存失败：{error}"));
            if let Some(error) = &error {
                self.status = error.clone();
            }
            workspace.update(cx, |workspace, cx| {
                workspace.finish_controller_sleep(&request, None, error, cx)
            });
            cx.notify();
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|e| e.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::ControllerSleep(ControllerSleepSave {
                workspace,
                request,
                identity,
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }
    pub(super) fn rebase_controller_sleep_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::ControllerSleep(sleep)) = &save.product else {
            return Ok(());
        };
        let setting = sleep
            .request
            .setting()
            .ok_or_else(|| anyhow::anyhow!("Controller sleep save has no root setting"))?;
        setting.seconds()?;
        anyhow::ensure!(
            self.devices.iter().any(|device| device == &sleep.workspace
                && device.read(cx).identity(cx) == sleep.identity
                && device
                    .read(cx)
                    .controller_sleep_request_matches(&sleep.request, cx)),
            "Controller sleep save owner changed"
        );
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let mut saved = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == sleep.identity {
                    saved.controller_power_saving = Some(setting);
                }
                saved
            })
            .collect();
        Ok(())
    }
    pub(super) fn finish_controller_sleep_save(
        &mut self,
        save: ControllerSleepSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if succeeded {
            self.refresh_macro_recorder_storage();
            let request = if self.close_requested || self.receiver_macro_exclusive(cx) {
                save.request.without_device_access()
            } else {
                save.request
            };
            self.finish_controller_sleep_result(&save.workspace, &request, None, None, window, cx);
        } else {
            let error = (!succeeded).then(|| "休眠设置本地保存失败，设备未写入。".to_owned());
            self.finish_controller_sleep_result(
                &save.workspace,
                &save.request,
                None,
                error,
                window,
                cx,
            );
        }
    }
    fn finish_controller_sleep_result(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &ControllerSleepRequested,
        result: Option<SleepResult>,
        error: Option<String>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if window
            .update(cx, |_, _, cx| {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_controller_sleep(request, result.clone(), error.clone(), cx)
                })
            })
            .is_err()
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_controller_sleep(request, result, error, cx)
            });
        }
    }
    fn submit_controller_sleep(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ControllerSleepRequested,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .controller_sleep_request_matches(&request, cx)
        {
            self.finish_controller_sleep_result(
                &workspace,
                &request,
                None,
                Some("休眠设置操作已取消。".into()),
                window,
                cx,
            );
            return;
        }
        self.refresh_macro_recorder_storage();
        if !request.device_access() || self.receiver_macro_exclusive(cx) {
            let request = request.without_device_access();
            self.finish_controller_sleep_result(&workspace, &request, None, None, window, cx);
            self.status = "休眠设置已保存在本机；宏录制期间不访问设备。".into();
            cx.notify();
            return;
        }
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && razer_device::controller_sleep::capability(route.product_id())
                        .is_some_and(|capability| capability.accepts(route.physical_product_id()))
                    && route.peer_product_id().is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            let error = "当前连接尚不唯一，设备休眠设置未确认。".to_owned();
            self.status = error.clone();
            self.finish_controller_sleep_result(
                &workspace,
                &request,
                None,
                Some(error),
                window,
                cx,
            );
            cx.notify();
            return;
        };
        let route = route.clone();
        let identity = workspace.read(cx).identity(cx);
        let owner = match CleanupOwner::from_route(identity.clone(), &route) {
            Ok(owner) => owner,
            Err(error) => {
                self.finish_controller_sleep_result(
                    &workspace,
                    &request,
                    None,
                    Some(format!("设备休眠连接身份未确认：{error:#}")),
                    window,
                    cx,
                );
                cx.notify();
                return;
            }
        };
        if self.controller_sleep_cleanup.blocked(&owner) {
            self.controller_sleep_cleanup.retry();
            self.finish_controller_sleep_result(
                &workspace,
                &request,
                None,
                Some("同一设备的服务清理尚未确认，当前设备请求未执行。".into()),
                window,
                cx,
            );
            cx.notify();
            return;
        }
        self.controller_sleep_cleanup.next_id =
            self.controller_sleep_cleanup.next_id.wrapping_add(1);
        let cleanup_id = self.controller_sleep_cleanup.next_id;
        let progress = Arc::new(CleanupProgress::default());
        let status_progress = Arc::clone(&progress);
        let status_workspace = workspace.clone();
        let status_request = request.clone();
        let status_identity = identity.clone();
        let status_owner = owner.clone();
        let status_task = cx.spawn(async move |this, cx| {
            let mut revision = 0;
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let snapshot = status_progress.snapshot();
                if snapshot.gate.finished {
                    break;
                }
                if snapshot.revision == revision {
                    continue;
                }
                revision = snapshot.revision;
                let Some(cleanup) = snapshot.failure() else {
                    continue;
                };
                if this
                    .update(cx, |this, cx| {
                        if !this
                            .controller_sleep_cleanup
                            .entries
                            .contains_key(&cleanup_id)
                        {
                            return;
                        }
                        this.controller_sleep_cleanup.last_failure = Some(cleanup.clone());
                        let current = this.devices.contains(&status_workspace)
                            && status_workspace.read(cx).identity(cx) == status_identity
                            && status_workspace
                                .read(cx)
                                .controller_sleep_request_matches(&status_request, cx)
                            && this.device_observations.iter().any(|observed| {
                                observed.matches(status_workspace.read(cx).device(cx))
                                    && CleanupOwner::from_route(status_identity.clone(), observed)
                                        .is_ok_and(|current| status_owner.same_device(&current))
                            });
                        if current {
                            let (observed, error) = match snapshot.operation {
                                Some(Ok(observed)) => (Some(observed), cleanup.clone()),
                                Some(Err(operation)) => (None, format!("{operation}; {cleanup}")),
                                None => (None, cleanup.clone()),
                            };
                            this.finish_controller_sleep_result(
                                &status_workspace,
                                &status_request,
                                observed,
                                Some(error),
                                window,
                                cx,
                            );
                        }
                        this.status = format!(
                            "设备服务清理未完成，同一设备的后续操作及退出保持等待：{cleanup}"
                        );
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        self.controller_sleep_workers += 1;
        let worker_progress = Arc::clone(&progress);
        let completed_progress = Arc::clone(&progress);
        let result_owner = owner.clone();
        let operation_task = cx.spawn(async move |this, cx| {
            let selected = route.clone();
            let guard = request.clone();
            let result = cx
                .background_spawn(async move {
                    anyhow::ensure!(
                        guard.is_valid(),
                        "Controller sleep scope canceled before submission"
                    );
                    let mut client = ServiceClient::spawn()?;
                    worker_progress.acquired();
                    guard.set_cleanup_pending(true);
                    client.set_request_guard(guard.guard());
                    let result = (|| {
                        let node =
                            razer_discovery::controller_triggers::resolve(&mut client, &selected)?;
                        let value = client.request(ServiceRequest::ControllerSleep {
                            node: node.clone(),
                            product_id: selected.product_id(),
                            sleep_operation: guard.operation().clone(),
                        })?;
                        SleepResponse::decode(
                            value,
                            &node,
                            selected.product_id(),
                            guard.operation(),
                        )
                    })();
                    let operation = result.map(|reply| reply.result);
                    worker_progress.operation(&operation);
                    let shutdown = if client.is_stopped() {
                        None
                    } else {
                        client
                            .request(ServiceRequest::Shutdown)
                            .err()
                            .map(|error| format!("{error:#}"))
                    };
                    worker_progress.confirmation(client.shutdown_confirmation());
                    if let Some(error) = &shutdown {
                        worker_progress.failed(error.clone());
                    }
                    // Admission closure is not process-exit proof. Keep this
                    // actual client in the background owner until all handles
                    // are reaped, including canceled and failed IPC requests.
                    let termination = retain_termination(
                        &worker_progress,
                        || guard.set_cleanup_pending(true),
                        || {
                            let termination = client.finish_termination();
                            worker_progress.confirmation(client.shutdown_confirmation());
                            termination
                        },
                    );
                    let mut failures = shutdown.into_iter().chain(termination).collect::<Vec<_>>();
                    if client.shutdown_confirmation().is_none() {
                        let failure = "Service teardown was not confirmed by a matching Shutdown reply; process and pipe resources were released".to_owned();
                        worker_progress.failed(failure.clone());
                        failures.push(failure);
                    }
                    finish_exchange(
                        operation,
                        (!failures.is_empty()).then(|| failures.join("; ")),
                    )
                })
                .await;
            let snapshot = completed_progress.finish();
            let release_owner = snapshot.gate.safe_to_release();
            request.set_cleanup_pending(!release_owner);
            let _ = this.update(cx, |this, cx| {
                if release_owner {
                    let _cleanup_owner = this.controller_sleep_cleanup.entries.remove(&cleanup_id);
                }
                this.controller_sleep_workers -= 1;
                if let Some(diagnostic) = snapshot.failure() {
                    log::warn!("[controller-sleep cleanup] {diagnostic}");
                    this.controller_sleep_cleanup.last_failure = Some(diagnostic);
                }
                let current = this.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == identity
                    && workspace
                        .read(cx)
                        .controller_sleep_request_matches(&request, cx)
                    && this.device_observations.iter().any(|observed| {
                        observed.matches(workspace.read(cx).device(cx))
                            && CleanupOwner::from_route(identity.clone(), observed)
                                .is_ok_and(|current| result_owner.same_device(&current))
                    });
                if current {
                    let (observed, error) = match &result {
                        Ok((observed, cleanup)) => (Some(observed.clone()), cleanup.clone()),
                        Err(error) => (None, Some(format!("{error:#}"))),
                    };
                    this.finish_controller_sleep_result(
                        &workspace, &request, observed, error, window, cx,
                    );
                    this.status = match result {
                        Ok((_, Some(error))) if release_owner => format!(
                            "设备休眠操作已返回，服务清理与进程释放已确认；保留清理诊断：{error}"
                        ),
                        Ok((_, Some(error))) => format!(
                            "设备休眠操作已返回；同一设备的后续操作及退出等待服务清理确认：{error}"
                        ),
                        Ok((observed, None)) => match observed.completion {
                            SleepCompletion::Observed => "已读取设备休眠设置。".into(),
                            SleepCompletion::Confirmed => {
                                "休眠设置已保存，设备秒数读回确认。".into()
                            }
                            SleepCompletion::Mismatch => {
                                "设备休眠读回与请求不一致，本地草稿保留。".into()
                            }
                            SleepCompletion::Failed => "设备休眠读取失败，未确认写入结果。".into(),
                            SleepCompletion::CanceledBeforeOperation
                            | SleepCompletion::CanceledAfterSet => {
                                "休眠操作已取消，已发送的设备写入未撤销。".into()
                            }
                        },
                        Err(error) => format!("设备休眠操作未确认：{error:#}"),
                    };
                } else {
                    this.finish_controller_sleep_result(
                        &workspace,
                        &request,
                        None,
                        Some("设备连接已变化，忽略旧休眠结果。".into()),
                        window,
                        cx,
                    );
                }
                if this.close_requested && this.save_task.is_none() {
                    this.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        });
        self.controller_sleep_cleanup.entries.insert(
            cleanup_id,
            CleanupEntry {
                owner,
                progress,
                _operation: operation_task,
                _status: status_task,
            },
        );
    }
}

pub(super) fn finish_exchange<T>(
    result: anyhow::Result<T>,
    shutdown: Option<String>,
) -> anyhow::Result<(T, Option<String>)> {
    match (result, shutdown) {
        (Ok(result), shutdown) => Ok((result, shutdown)),
        (Err(error), None) => Err(error),
        (Err(error), Some(shutdown)) => Err(error.context(format!(
            "Controller sleep worker cleanup failed: {shutdown}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CleanupOwner, CleanupProgress, SleepCompletion, SleepResult, finish_exchange,
        retain_termination,
    };
    use razer_device::backend::HidNode;
    use razer_device::controller_sleep::{SleepOperation, SleepReadStatus, SleepWriteStatus};
    use std::cell::Cell;
    use std::prelude::v1::test;

    #[test]
    fn operation_and_cleanup_failures_are_both_retained() {
        let error = finish_exchange::<SleepResult>(
            Err(anyhow::anyhow!("real GET transport failure")),
            Some("worker exit failed".into()),
        )
        .unwrap_err();
        let error = format!("{error:#}");
        assert!(error.contains("real GET transport failure"));
        assert!(error.contains("worker exit failed"));
    }
    #[test]
    fn cleanup_failure_preserves_typed_device_failure_without_claiming_confirmation() {
        let failure = SleepResult::new(
            &SleepOperation::Read,
            SleepWriteStatus::NotRequested,
            SleepReadStatus::Failed {
                error: "device rejected GET".into(),
            },
            SleepCompletion::Failed,
        )
        .unwrap();
        let (result, shutdown) = finish_exchange(Ok(failure), Some("exit failure".into())).unwrap();
        assert_eq!(result.completion, SleepCompletion::Failed);
        assert!(result.observation().is_none());
        assert_eq!(shutdown.as_deref(), Some("exit failure"));
    }

    #[test]
    fn real_cleanup_retry_retains_owner_and_error_until_actual_release() {
        let progress = CleanupProgress::default();
        progress.acquired();
        progress.lock().gate.confirmed(42);
        let pending = Cell::new(false);
        let attempts = Cell::new(0);
        let handle_owned = Cell::new(true);
        let failures = retain_termination(
            &progress,
            || {
                pending.set(true);
                progress.retry();
            },
            || {
                attempts.set(attempts.get() + 1);
                assert!(
                    handle_owned.get(),
                    "The same live process handle remains owned"
                );
                if attempts.get() == 1 {
                    Err(anyhow::anyhow!("OS could not establish process exit"))
                } else {
                    assert!(pending.get());
                    assert!(
                        progress.blocked(),
                        "Retry admission does not clear the failed owner"
                    );
                    handle_owned.set(false);
                    Ok(())
                }
            },
        );
        assert_eq!(attempts.get(), 2);
        assert!(!handle_owned.get());
        assert!(
            progress.blocked(),
            "The operation owner must publish its completed cleanup before release"
        );
        progress.confirmation(None);
        let snapshot = progress.finish();
        assert!(snapshot.gate.safe_to_release());
        assert!(snapshot.gate.process_released);
        assert_eq!(snapshot.gate.shutdown_request_id, Some(42));
        assert_eq!(failures, ["OS could not establish process exit"]);
        assert!(
            snapshot.failure().is_some(),
            "The cleanup failure receipt remains recorded"
        );
    }

    #[test]
    fn acquired_client_requires_both_service_confirmation_and_resource_release() {
        for confirmed in [false, true] {
            for released in [false, true] {
                let progress = CleanupProgress::default();
                progress.acquired();
                if confirmed {
                    // This models the private pure gate event. Production can
                    // only deliver it through ShutdownConfirmation.
                    progress.lock().gate.confirmed(42);
                }
                if released {
                    progress.released();
                }
                let snapshot = progress.finish();
                assert_eq!(snapshot.gate.safe_to_release(), confirmed && released);
                assert_eq!(progress.blocked(), !(confirmed && released));
            }
        }
    }

    #[test]
    fn no_client_acquired_does_not_require_an_unrelated_shutdown_receipt() {
        for error in [
            "scope canceled before spawn",
            "service process could not spawn",
        ] {
            let progress = CleanupProgress::default();
            progress.failed(error.into());
            let snapshot = progress.finish();
            assert!(snapshot.gate.safe_to_release());
            assert!(!snapshot.gate.client_acquired);
            assert_eq!(snapshot.gate.shutdown_request_id, None);
            assert_eq!(snapshot.failure().as_deref(), Some(error));
        }
    }

    #[test]
    fn released_process_without_service_receipt_stays_blocked_without_busy_retry() {
        let progress = CleanupProgress::default();
        progress.acquired();
        progress.failed("service rejected native restoration".into());
        let attempts = Cell::new(0);
        let failures = retain_termination(
            &progress,
            || panic!("A successful process release needs no retry"),
            || {
                attempts.set(attempts.get() + 1);
                Ok(())
            },
        );
        assert!(failures.is_empty());
        let snapshot = progress.finish();
        assert!(snapshot.gate.process_released);
        assert!(progress.blocked());
        progress.retry();
        assert!(!progress.snapshot().retry);
        assert_eq!(attempts.get(), 1);
        assert_eq!(
            snapshot.failure().as_deref(),
            Some("service rejected native restoration")
        );
    }

    fn node() -> HidNode {
        HidNode {
            path: b"current-controller-collection".to_vec(),
            vendor_id: 5426,
            product_id: 2676,
            usage_page: 1,
            usage: 5,
            interface_number: 1,
            release_number: 1,
            serial_number: Some("controller-a".into()),
            manufacturer: None,
            product: None,
        }
    }

    #[test]
    fn unconfirmed_cleanup_blocks_only_the_same_complete_device_owner() {
        let owner = CleanupOwner {
            workspace: "2676:current-container".into(),
            logical_product_id: 2676,
            container: "current-container".into(),
            scope: razer_ipc::device_tasks::Scope::collection(2676, &node()),
            physical_product_id: 2676,
            retained_node: Some(node()),
        };
        let progress = CleanupProgress::default();
        progress.acquired();
        progress.released();
        progress.finish();
        assert!(owner.blocks(&owner, &progress));
        let mut same_owner = owner.clone();
        same_owner.workspace = "2676:CURRENT-CONTAINER".into();
        assert!(owner.blocks(&same_owner, &progress));

        let mut another_workspace = owner.clone();
        another_workspace.workspace = "2676:another-container".into();
        let mut another_physical_product = owner.clone();
        another_physical_product.physical_product_id = 2680;
        let mut another_collection = owner.clone();
        let mut replacement = node();
        replacement.path = b"replacement-controller-collection".to_vec();
        another_collection.scope = razer_ipc::device_tasks::Scope::collection(2676, &replacement);
        another_collection.retained_node = Some(replacement);
        let mut same_path_other_collection = owner.clone();
        same_path_other_collection
            .retained_node
            .as_mut()
            .unwrap()
            .usage = 6;
        assert!(owner.blocks(&same_path_other_collection, &progress));
        assert!(!owner.same_device(&same_path_other_collection));
        let mut another_logical_product = owner.clone();
        another_logical_product.logical_product_id = 2636;
        another_logical_product.scope = razer_ipc::device_tasks::Scope::collection(2636, &node());
        for unrelated in [
            another_workspace,
            another_physical_product,
            another_collection,
            another_logical_product,
        ] {
            assert!(!owner.blocks(&unrelated, &progress));
        }
        assert!(
            progress.blocked(),
            "Unrelated owners do not clear the exit barrier"
        );
    }

    #[test]
    fn container_route_without_a_collection_retains_a_conservative_owner_barrier() {
        let owner = CleanupOwner {
            workspace: "2676:current-container".into(),
            logical_product_id: 2676,
            container: "current-container".into(),
            scope: razer_ipc::device_tasks::Scope::container(2676, "CURRENT-CONTAINER").unwrap(),
            physical_product_id: 2676,
            retained_node: None,
        };
        let progress = CleanupProgress::default();
        progress.acquired();
        progress.released();
        progress.finish();
        let mut same_owner = owner.clone();
        same_owner.scope =
            razer_ipc::device_tasks::Scope::container(2676, "current-container").unwrap();
        assert!(owner.blocks(&same_owner, &progress));
        let mut unrelated = owner.clone();
        unrelated.container = "other-container".into();
        unrelated.scope =
            razer_ipc::device_tasks::Scope::container(2676, "other-container").unwrap();
        assert!(!owner.blocks(&unrelated, &progress));
    }

    #[test]
    fn resolving_or_enriching_the_same_endpoint_cannot_bypass_failed_cleanup() {
        let owner = CleanupOwner {
            workspace: "2676:current-container".into(),
            logical_product_id: 2676,
            container: "current-container".into(),
            scope: razer_ipc::device_tasks::Scope::container(2676, "current-container").unwrap(),
            physical_product_id: 2676,
            retained_node: None,
        };
        let progress = CleanupProgress::default();
        progress.acquired();
        progress.released();
        progress.finish();
        let mut resolved = owner.clone();
        resolved.scope = razer_ipc::device_tasks::Scope::collection(2676, &node());
        resolved.retained_node = Some(node());
        assert!(owner.blocks(&resolved, &progress));
        assert!(resolved.blocks(&owner, &progress));
        assert!(!owner.same_device(&resolved));

        let mut enriched = resolved.clone();
        let metadata = enriched.retained_node.as_mut().unwrap();
        metadata.serial_number = Some("newly-reported-serial".into());
        metadata.manufacturer = Some("newly-reported-manufacturer".into());
        metadata.product = Some("newly-reported-product".into());
        assert!(resolved.blocks(&enriched, &progress));
        assert!(enriched.blocks(&resolved, &progress));
        assert!(!resolved.same_device(&enriched));
        let mut unrelated = resolved.clone();
        unrelated.workspace = "2676:other-container".into();
        unrelated.container = "other-container".into();
        assert!(!owner.blocks(&unrelated, &progress));
    }

    #[test]
    fn shutdown_protocol_difference_does_not_discard_proven_device_observation() {
        let observed = SleepResult::new(
            &SleepOperation::Read,
            SleepWriteStatus::NotRequested,
            SleepReadStatus::Observed {
                observation: razer_device::controller_sleep::SleepObservation::from_seconds(300),
            },
            SleepCompletion::Observed,
        )
        .unwrap();
        let (result, shutdown) = finish_exchange(
            Ok(observed),
            Some("shutdown receipt rejected; process exit proven".into()),
        )
        .unwrap();
        assert_eq!(result.observation().unwrap().seconds, 300);
        assert_eq!(result.completion, SleepCompletion::Observed);
        assert!(shutdown.unwrap().contains("process exit proven"));
    }
}

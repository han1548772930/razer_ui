//! Page-independent controller import -> local commit -> source Power restoration.
//! The existing controller cleanup owner retains both service and OS release gates.
use super::controller_sleep::{
    CleanupEntry, CleanupOwner, CleanupProgress, finish_exchange, retain_termination,
};
use super::*;
use razer_ipc::{ServiceClient, ServiceRequest, controller_initialization::InitializationResponse};
use razer_pages::features::ControllerInitializationRequested;
use std::sync::Arc;

pub(super) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    request: ControllerInitializationRequested,
    snapshot: Device,
}
impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
    pub(super) fn snapshot(&self) -> &Device {
        &self.snapshot
    }
    pub(super) fn validate(&self, cx: &App) -> Result<(), String> {
        if self.workspace.read(cx).identity(cx) == self.identity
            && self
                .workspace
                .read(cx)
                .controller_initialization_matches(&self.request, cx)
        {
            Ok(())
        } else {
            Err("Controller import local-save owner changed".into())
        }
    }
}

impl AppShell {
    pub(super) fn initialize_controller_roots(&mut self, cx: &mut Context<Self>) {
        if self.close_requested {
            return;
        }
        let recording = self.receiver_macro_exclusive(cx);
        let mut candidates = Vec::new();
        for workspace in &self.devices {
            let device = workspace.read(cx).device(cx);
            let Some(source) = razer_device::controller_sleep::capability(device.product_id) else {
                continue;
            };
            let routes = self
                .device_observations
                .iter()
                .filter(|route| {
                    route.matches(device)
                        && source.accepts(route.physical_product_id())
                        && route.peer_product_id().is_none()
                })
                .cloned()
                .collect::<Vec<_>>();
            if let [route] = routes.as_slice() {
                candidates.push((workspace.clone(), route.clone()));
            }
        }
        for (workspace, route) in candidates {
            let identity = workspace.read(cx).identity(cx);
            let owner = match CleanupOwner::from_route(identity.clone(), &route) {
                Ok(owner) => owner,
                Err(error) => {
                    self.status = format!("手柄初始化连接身份未确认：{error:#}");
                    continue;
                }
            };
            if self.controller_sleep_cleanup.blocked(&owner) {
                continue;
            }
            let key = format!(
                "{}:{}:{:?}",
                route.container(),
                route.physical_product_id(),
                route.hid_node()
            );
            let Some(request) = workspace.update(cx, |workspace, cx| {
                workspace.begin_controller_initialization(key, recording, cx)
            }) else {
                continue;
            };
            self.controller_sleep_cleanup.next_id =
                self.controller_sleep_cleanup.next_id.wrapping_add(1);
            let cleanup_id = self.controller_sleep_cleanup.next_id;
            let progress = Arc::new(CleanupProgress::default());
            let status_progress = Arc::clone(&progress);
            let status_task = cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(50))
                        .await;
                    let snapshot = status_progress.snapshot();
                    if snapshot.finished() {
                        break;
                    }
                    if let Some(error) = snapshot.failure() {
                        if this
                            .update(cx, |this, cx| {
                                this.controller_sleep_cleanup.last_failure = Some(error.clone());
                                this.status = format!("手柄初始化服务清理仍在等待：{error}");
                                cx.notify();
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            });
            let worker_progress = Arc::clone(&progress);
            let completed_progress = Arc::clone(&progress);
            let result_owner = owner.clone();
            self.controller_sleep_workers += 1;
            let operation_task = cx.spawn(async move |this, cx| {
                let selected = route.clone();
                let guard = request.clone();
                let result = cx.background_spawn(async move {
                    anyhow::ensure!(guard.is_valid(), "Controller import was canceled before submission");
                    let mut client = ServiceClient::spawn()?;
                    worker_progress.acquired();
                    client.set_request_guard(guard.guard());
                    let operation = (|| {
                        let node = razer_discovery::controller_triggers::resolve(&mut client, &selected)?;
                        let value = client.request(ServiceRequest::ControllerInitialization { node: node.clone(), product_id: selected.product_id(), plan: guard.plan().clone() })?;
                        InitializationResponse::decode(value, &node, selected.product_id(), guard.plan()).map(|reply| reply.result)
                    })();
                    let shutdown = if client.is_stopped() { None } else { client.request(ServiceRequest::Shutdown).err().map(|error| format!("{error:#}")) };
                    worker_progress.confirmation(client.shutdown_confirmation());
                    if let Some(error) = &shutdown { worker_progress.failed(error.clone()); }
                    let termination = retain_termination(&worker_progress, || {}, || {
                        let result = client.finish_termination();
                        worker_progress.confirmation(client.shutdown_confirmation());
                        result
                    });
                    let mut failures = shutdown.into_iter().chain(termination).collect::<Vec<_>>();
                    if client.shutdown_confirmation().is_none() {
                        let failure = "Controller import service cleanup has no matching Shutdown receipt".to_owned();
                        worker_progress.failed(failure.clone());
                        failures.push(failure);
                    }
                    finish_exchange(operation, (!failures.is_empty()).then(|| failures.join("; ")))
                }).await;
                let cleanup = completed_progress.finish();
                let released = cleanup.gate.safe_to_release();
                let _ = this.update(cx, |this, cx| {
                    this.controller_sleep_workers -= 1;
                    if released { let _owner = this.controller_sleep_cleanup.entries.remove(&cleanup_id); }
                    if let Some(error) = cleanup.failure() { this.controller_sleep_cleanup.last_failure = Some(error); }
                    let current = this.devices.contains(&workspace) && workspace.read(cx).identity(cx) == identity
                        && workspace.read(cx).controller_initialization_matches(&request, cx)
                        && this.device_observations.iter().any(|observed| observed.matches(workspace.read(cx).device(cx)) && CleanupOwner::from_route(identity.clone(), observed).is_ok_and(|owner| result_owner.same_device(&owner)));
                    if current {
                        match result {
                            Ok((observed, diagnostic)) => {
                                let accepted = this.main_window.update(cx, |_, window, cx| {
                                    workspace.update(cx, |workspace, cx| workspace.finish_controller_initialization(&request, Ok(observed), window, cx))
                                }).unwrap_or(false);
                                if accepted && released {
                                    let snapshot = workspace.read(cx).snapshot(cx);
                                    let file = this.workspace_file(this.devices.iter().map(|owner| owner.read(cx).saved_snapshot(cx)).collect(), cx);
                                    let save = this.prepare_save(this.main_window, file, Some(ProductSave::ControllerInitialization(LocalSave { workspace: workspace.clone(), identity: identity.clone(), request: request.clone(), snapshot })), cx);
                                    this.pending_saves.push_back(save);
                                    this.start_pending_save(cx);
                                    this.status = diagnostic.map(|error| format!("手柄真实目录已读取；保留服务清理诊断：{error}")).unwrap_or_else(|| "手柄根字段与真实配置档已读取，正在保存导入结果。".into());
                                } else if !released {
                                    this.status = "手柄读取已返回，服务清理未确认，恢复写入仍被阻挡。".into();
                                } else {
                                    this.status = "手柄导入结果未发布，读取失败或本地编辑已变化。".into();
                                }
                            }
                            Err(error) => {
                                let error = format!("{error:#}");
                                let _ = this.main_window.update(cx, |_, window, cx| workspace.update(cx, |workspace, cx| workspace.finish_controller_initialization(&request, Err(error.clone()), window, cx)));
                                this.status = format!("手柄根初始化未确认：{error}");
                            }
                        }
                    }
                    if this.close_requested && this.save_task.is_none() { this.continue_save_queue(true, cx); }
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

    pub(super) fn finish_controller_initialization_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        let recording = self.close_requested || self.receiver_macro_exclusive(cx);
        save.workspace.update(cx, |workspace, cx| {
            workspace.complete_controller_initialization_storage(
                &save.request,
                succeeded,
                recording,
                cx,
            )
        });
        if succeeded {
            // The generic DeviceConnected phase starts only after the import
            // document's real save callback; its own save gates runtime restore.
            self.initialize_controller_roots(cx);
        }
        if !succeeded {
            self.status = "手柄导入结果保存失败，未提交缓存恢复写入。".into();
        }
    }
}

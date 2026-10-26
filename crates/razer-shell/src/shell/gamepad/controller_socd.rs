//! Profile SOCD commits precede the independently acknowledged device SET.
use super::controller_sleep::{
    CleanupEntry, CleanupOwner, CleanupProgress, finish_exchange, retain_termination,
};
use super::*;
use razer_device::controller_socd::SocdResult;
use razer_ipc::{ServiceClient, ServiceRequest, controller_socd::SocdResponse};
use razer_pages::features::ControllerSocdRequested;
use std::sync::Arc;

pub(super) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    request: ControllerSocdRequested,
    storage_committed: bool,
}
impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
    pub(super) fn storage_committed(&mut self) {
        self.storage_committed = true;
    }
}

impl AppShell {
    pub(super) fn persist_controller_socd(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ControllerSocdRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !request.requires_persistence() {
            self.submit_controller_socd(workspace, request, cx);
            return;
        }
        if !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .controller_socd_request_matches(&request, cx)
            || self.storage_error.is_some()
        {
            workspace.update(cx, |workspace, cx| {
                workspace.complete_controller_socd_storage(&request, false, false, cx)
            });
            if let Some(error) = &self.storage_error {
                self.status = format!("SOCD 本地保存失败：{error}");
            }
            cx.notify();
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|owner| owner.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        let save = self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::ControllerSocd(LocalSave {
                workspace,
                identity,
                request,
                storage_committed: false,
            })),
            cx,
        );
        self.pending_saves.push_back(save);
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_controller_socd_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::ControllerSocd(socd)) = &save.product else {
            return Ok(());
        };
        anyhow::ensure!(
            self.devices.contains(&socd.workspace)
                && socd.workspace.read(cx).identity(cx) == socd.identity
                && socd
                    .workspace
                    .read(cx)
                    .controller_socd_request_matches(&socd.request, cx),
            "Controller SOCD local-save owner changed"
        );
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let mut saved = entity.read(cx).saved_snapshot(cx);
                if entity == &socd.workspace {
                    socd.request.project_saved(&mut saved)?;
                }
                Ok(saved)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(())
    }

    pub(super) fn finish_controller_socd_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        self.refresh_macro_recorder_storage();
        let permit_device = succeeded
            && !self.close_requested
            && !self.receiver_macro_exclusive(cx)
            && self.devices.contains(&save.workspace)
            && save.workspace.read(cx).identity(cx) == save.identity;
        save.workspace.update(cx, |workspace, cx| {
            workspace.complete_controller_socd_storage(
                &save.request,
                save.storage_committed,
                permit_device,
                cx,
            )
        });
        if !save.storage_committed {
            self.status = "SOCD 本地保存失败，设备未写入。".into();
        } else if !succeeded {
            self.status = "SOCD 已保存到本机；设备连接已变化，未发送旧请求。".into();
        }
    }

    fn defer_controller_socd(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ControllerSocdRequested,
        owner: CleanupOwner,
        cx: &mut Context<Self>,
    ) {
        self.controller_sleep_cleanup.next_id =
            self.controller_sleep_cleanup.next_id.wrapping_add(1);
        let wait_id = self.controller_sleep_cleanup.next_id;
        self.controller_sleep_workers += 1;
        request.set_cleanup_pending(true);
        self.status = "SOCD 已保存，正在等待同一设备的上一项服务清理。".into();
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let completed = this.update(cx, |this, cx| {
                    if !this.controller_socd_waiters.contains_key(&wait_id) {
                        return true;
                    }
                    let current = !this.close_requested
                        && !this.receiver_macro_exclusive(cx)
                        && this.devices.contains(&workspace)
                        && workspace
                            .read(cx)
                            .controller_socd_request_matches(&request, cx)
                        && this.device_observations.iter().any(|observed| {
                            observed.matches(workspace.read(cx).device(cx))
                                && CleanupOwner::from_route(
                                    workspace.read(cx).identity(cx),
                                    observed,
                                )
                                .is_ok_and(|current| owner.same_device(&current))
                        });
                    if current && this.controller_sleep_cleanup.blocked(&owner) {
                        return false;
                    }
                    let _waiter = this.controller_socd_waiters.remove(&wait_id);
                    this.controller_sleep_workers -= 1;
                    request.set_cleanup_pending(false);
                    if current {
                        // Submission repeats full route and request validation;
                        // the cleanup reservation is acquired in this update.
                        this.submit_controller_socd(workspace.clone(), request.clone(), cx);
                    } else {
                        workspace.update(cx, |workspace, cx| {
                            workspace.finish_controller_socd(
                                &request,
                                Err("SOCD 等待期间连接或操作已变化，未发送旧请求。".into()),
                                cx,
                            )
                        });
                    }
                    if this.close_requested && this.save_task.is_none() {
                        this.continue_save_queue(true, cx);
                    }
                    cx.notify();
                    true
                });
                if !matches!(completed, Ok(false)) {
                    break;
                }
            }
        });
        self.controller_socd_waiters.insert(wait_id, task);
        cx.notify();
    }

    fn submit_controller_socd(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ControllerSocdRequested,
        cx: &mut Context<Self>,
    ) {
        self.refresh_macro_recorder_storage();
        let eligible = !self.close_requested
            && !self.receiver_macro_exclusive(cx)
            && self.devices.contains(&workspace)
            && workspace
                .read(cx)
                .controller_socd_request_matches(&request, cx);
        if !eligible {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_controller_socd(&request, Err("SOCD 设备操作已取消。".into()), cx)
            });
            return;
        }
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && route.product_id() == 2676
                    && matches!(route.physical_product_id(), 2676 | 2677 | 2680 | 2681)
                    && route.peer_product_id().is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_controller_socd(
                    &request,
                    Err("SOCD 当前设备连接不唯一，未提交写入。".into()),
                    cx,
                )
            });
            return;
        };
        let route = route.clone();
        let identity = workspace.read(cx).identity(cx);
        let owner = match CleanupOwner::from_route(identity.clone(), &route) {
            Ok(owner) => owner,
            Err(error) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_controller_socd(
                        &request,
                        Err(format!("SOCD 连接身份未确认：{error:#}")),
                        cx,
                    )
                });
                return;
            }
        };
        if self.controller_sleep_cleanup.blocked(&owner) {
            self.retry_controller_sleep_cleanup(cx);
            self.defer_controller_socd(workspace, request, owner, cx);
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
            let mut last_failure = None;
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;
                let snapshot = status_progress.snapshot();
                if snapshot.finished() {
                    break;
                }
                let Some(error) = snapshot.failure() else {
                    continue;
                };
                if last_failure.as_ref() == Some(&error) {
                    continue;
                }
                last_failure = Some(error.clone());
                if this
                    .update(cx, |this, cx| {
                        if !this
                            .controller_sleep_cleanup
                            .entries
                            .contains_key(&cleanup_id)
                        {
                            return;
                        }
                        this.controller_sleep_cleanup.last_failure = Some(error.clone());
                        let current = this.devices.contains(&status_workspace)
                            && status_workspace.read(cx).identity(cx) == status_identity
                            && status_workspace
                                .read(cx)
                                .controller_socd_request_matches(&status_request, cx)
                            && this.device_observations.iter().any(|observed| {
                                observed.matches(status_workspace.read(cx).device(cx))
                                    && CleanupOwner::from_route(status_identity.clone(), observed)
                                        .is_ok_and(|owner| status_owner.same_device(&owner))
                            });
                        if current {
                            status_workspace.update(cx, |workspace, cx| {
                                workspace.finish_controller_socd(
                                    &status_request,
                                    Err(format!("SOCD 服务清理仍在等待：{error}")),
                                    cx,
                                )
                            });
                        }
                        this.status = format!(
                            "设备服务清理未完成，同一设备的后续操作及退出保持等待：{error}"
                        );
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let worker_progress = Arc::clone(&progress);
        let completed_progress = Arc::clone(&progress);
        let result_owner = owner.clone();
        request.set_cleanup_pending(true);
        self.controller_sleep_workers += 1;
        let operation_task = cx.spawn(async move |this, cx| {
            let selected = route.clone();
            let guard = request.clone();
            let result = cx
                .background_spawn(async move {
                    anyhow::ensure!(
                        guard.is_valid(),
                        "Controller SOCD scope canceled before submission"
                    );
                    let mut client = ServiceClient::spawn()?;
                    worker_progress.acquired();
                    client.set_request_guard(guard.guard());
                    let operation = (|| {
                        let node =
                            razer_discovery::controller_triggers::resolve(&mut client, &selected)?;
                        let value = client.request(ServiceRequest::ControllerSocd {
                            node: node.clone(),
                            product_id: selected.product_id(),
                            socd_operation: guard.operation(),
                        })?;
                        SocdResponse::decode(value, &node, selected.product_id(), guard.operation())
                            .map(|reply| reply.result)
                    })();
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
                    let termination = retain_termination(
                        &worker_progress,
                        || guard.set_cleanup_pending(true),
                        || {
                            let result = client.finish_termination();
                            worker_progress.confirmation(client.shutdown_confirmation());
                            result
                        },
                    );
                    let mut failures = shutdown.into_iter().chain(termination).collect::<Vec<_>>();
                    if client.shutdown_confirmation().is_none() {
                        let failure =
                            "SOCD service cleanup has no matching Shutdown receipt".to_owned();
                        worker_progress.failed(failure.clone());
                        failures.push(failure);
                    }
                    finish_exchange(
                        operation,
                        (!failures.is_empty()).then(|| failures.join("; ")),
                    )
                })
                .await;
            let cleanup = completed_progress.finish();
            let released = cleanup.gate.safe_to_release();
            request.set_cleanup_pending(!released);
            let _ = this.update(cx, |this, cx| {
                this.controller_sleep_workers -= 1;
                if released {
                    let _owner = this.controller_sleep_cleanup.entries.remove(&cleanup_id);
                }
                if let Some(error) = cleanup.failure() {
                    this.controller_sleep_cleanup.last_failure = Some(error);
                }
                let current = this.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == identity
                    && workspace
                        .read(cx)
                        .controller_socd_request_matches(&request, cx)
                    && this.device_observations.iter().any(|observed| {
                        observed.matches(workspace.read(cx).device(cx))
                            && CleanupOwner::from_route(identity.clone(), observed)
                                .is_ok_and(|owner| result_owner.same_device(&owner))
                    });
                if current {
                    let published = match &result {
                        Ok((observed, _)) => Ok(observed.clone()),
                        Err(error) => Err(format!("{error:#}")),
                    };
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_controller_socd(&request, published, cx)
                    });
                    this.status = describe_exchange(&result, released);
                } else {
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_controller_socd(
                            &request,
                            Err("设备连接已变化，忽略旧 SOCD 结果。".into()),
                            cx,
                        )
                    });
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

fn describe_exchange(
    result: &anyhow::Result<(SocdResult, Option<String>)>,
    released: bool,
) -> String {
    let mut status = match result {
        Ok((observed, _)) if observed.acknowledged() => {
            "SOCD 已保存，设备 SET 应答已确认。".to_owned()
        }
        Ok((observed, _)) if observed.canceled => "SOCD 设备操作已取消，本地草稿保留。".into(),
        Ok((observed, _)) => match &observed.write {
            razer_device::controller_socd::SocdWriteStatus::Failed { error } => {
                format!("SOCD 设备写入失败：{error}")
            }
            _ => "SOCD 设备写入未确认。".into(),
        },
        Err(error) => format!("SOCD 设备操作未确认：{error:#}"),
    };
    if let Ok((_, Some(error))) = result {
        status.push_str(&format!(" 服务清理诊断：{error}"));
    }
    if !released {
        status.push_str(" 后续设备操作及退出等待服务清理确认。");
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::controller_socd::SocdWriteStatus;
    use std::prelude::v1::test;

    #[test]
    fn controller_socd_acknowledgement_does_not_replace_teardown_confirmation() {
        let observed = SocdResult {
            write: SocdWriteStatus::Acknowledged {
                response_payload: vec![0x91],
            },
            canceled: false,
        };
        let result = finish_exchange(
            Ok(observed.clone()),
            Some("Shutdown receipt missing".into()),
        );
        let (actual, diagnostic) = result.as_ref().unwrap();
        assert_eq!(actual, &observed);
        assert_eq!(diagnostic.as_deref(), Some("Shutdown receipt missing"));
        let status = describe_exchange(&result, false);
        assert!(status.contains("SET 应答已确认"));
        assert!(status.contains("等待服务清理确认"));
        assert!(status.contains("Shutdown receipt missing"));
    }

    #[test]
    fn controller_socd_cancel_and_failed_set_never_report_acknowledged_success() {
        for observed in [
            SocdResult {
                write: SocdWriteStatus::Acknowledged {
                    response_payload: vec![],
                },
                canceled: true,
            },
            SocdResult {
                write: SocdWriteStatus::Failed {
                    error: "device rejection".into(),
                },
                canceled: false,
            },
        ] {
            let status = describe_exchange(&Ok((observed, None)), true);
            assert!(!status.contains("应答已确认"));
            assert!(status.contains("已取消") || status.contains("device rejection"));
        }
        let result = finish_exchange::<SocdResult>(
            Err(anyhow::anyhow!("actual SET failure")),
            Some("actual release failure".into()),
        );
        let status = describe_exchange(&result, false);
        assert!(status.contains("actual SET failure"));
        assert!(status.contains("actual release failure"));
        assert!(!status.contains("应答已确认"));
    }
}

//! Current 182 stage table intents over a uniquely re-observed direct route.
use super::*;
use razer_device::mouse_dpi_stages::{DpiStagesDraft, DpiStagesReading};
use razer_discovery::{direct, discovery};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::mouse_polling::MousePollingScope;

impl AppShell {
    pub(super) fn write_mouse_dpi_stages(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: MousePollingScope,
        revision: u64,
        draft: DpiStagesDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mouse_dpi_request(workspace, scope, revision, Some(draft), window, cx);
    }

    pub(super) fn read_mouse_dpi_stages(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: MousePollingScope,
        revision: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mouse_dpi_request(workspace, scope, revision, None, window, cx);
    }

    fn mouse_dpi_request(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: MousePollingScope,
        revision: u64,
        draft: Option<DpiStagesDraft>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let matches = match &draft {
            Some(draft) => workspace
                .read(cx)
                .mouse_dpi_request_matches(scope, revision, draft, cx),
            None => workspace
                .read(cx)
                .mouse_dpi_read_matches(scope, revision, cx),
        };
        if !matches {
            workspace.update(cx, |owner, cx| match &draft {
                Some(draft) => owner.finish_mouse_dpi(scope, revision, draft, None, cx),
                None => owner.finish_mouse_dpi_read(scope, revision, None, cx),
            });
            return;
        }
        let device = workspace.read(cx).device(cx);
        let valid_scope = match &draft {
            Some(draft) => workspace
                .read(cx)
                .mouse_dpi_write_guard(scope, revision, draft, cx),
            None => workspace.read(cx).mouse_dpi_read_guard(scope, revision, cx),
        };
        let Some(valid_scope) = valid_scope else {
            return;
        };
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(device)
                    && route.peer_product_id().is_none()
                    && matches!(route.transport(), Some(discovery::ObservedTransport::Wired))
            })
            .collect::<Vec<_>>();
        if routes.len() != 1 {
            self.status =
                "DPI 阶段表未发送：需要唯一的有线设备连接，当前无线转发链尚未核实。".into();
            workspace.update(cx, |owner, cx| match &draft {
                Some(draft) => owner.finish_mouse_dpi(scope, revision, draft, None, cx),
                None => owner.finish_mouse_dpi_read(scope, revision, None, cx),
            });
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        let product_id = device.product_id;
        let discovery_revision = self.discovery_revision;
        self.device_value_owners.remove(&identity);
        self.device_read_scopes.remove(&identity);
        self.status = if draft.is_some() {
            "正在写入完整 DPI 阶段表并读取设备确认。"
        } else {
            "正在读取设备 DPI 阶段表。"
        }
        .into();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let selected = route.clone();
            let requested = draft.clone();
            let result = cx
                .background_spawn(async move {
                    use anyhow::Context as _;
                    let settings = razer_device::mouse_dpi_stages::capability(product_id)
                        .context("产品 DPI 阶段表能力尚未核实")?;
                    let mut client = ServiceClient::spawn()?;
                    let result = (|| {
                        anyhow::ensure!(
                            valid_scope.load(std::sync::atomic::Ordering::Acquire),
                            "DPI scope changed before device resolution"
                        );
                        let direct = direct::resolve(&mut client, &selected, &settings.transport)?;
                        anyhow::ensure!(
                            valid_scope.load(std::sync::atomic::Ordering::Acquire),
                            "DPI scope changed before device submission"
                        );
                        match requested {
                            Some(draft) => direct.write_dpi_stages(&mut client, draft),
                            None => direct.read_dpi_stages(&mut client),
                        }
                    })();
                    let shutdown_error = client
                        .request(ServiceRequest::Shutdown)
                        .err()
                        .map(|e| format!("{e:#}"));
                    result.map(|reading| (reading, shutdown_error))
                })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                let token = match &draft {
                    Some(draft) => workspace
                        .read(cx)
                        .mouse_dpi_in_flight_matches(scope, revision, draft, cx),
                    None => workspace
                        .read(cx)
                        .mouse_dpi_reading_token_matches(scope, revision, cx),
                };
                if !token {
                    return;
                }
                let scope_current = match &draft {
                    Some(draft) => workspace
                        .read(cx)
                        .mouse_dpi_observation_matches(scope, revision, draft, cx),
                    None => workspace
                        .read(cx)
                        .mouse_dpi_read_matches(scope, revision, cx),
                };
                let current = this.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == identity
                    && scope_current
                    && this.discovery_revision == discovery_revision
                    && this.device_observations.iter().any(|current| {
                        current.matches(workspace.read(cx).device(cx))
                            && current.container().eq_ignore_ascii_case(route.container())
                            && current.physical_product_id() == route.physical_product_id()
                            && current.hid_node() == route.hid_node()
                            && current.peer_product_id().is_none()
                            && matches!(
                                current.transport(),
                                Some(discovery::ObservedTransport::Wired)
                            )
                    });
                let observed: Option<DpiStagesReading> = if current {
                    result.as_ref().ok().map(|(value, _)| value.clone())
                } else {
                    None
                };
                this.device_value_owners.remove(&identity);
                this.device_read_scopes.remove(&identity);
                workspace.update(cx, |owner, cx| match &draft {
                    Some(draft) => owner.finish_mouse_dpi(scope, revision, draft, observed, cx),
                    None => owner.finish_mouse_dpi_read(scope, revision, observed, cx),
                });
                this.status = if !current {
                    "DPI 操作期间设备或配置已变化，结果未应用到当前页面。".into()
                } else {
                    match result {
                        Ok((_, None)) => if draft.is_some() {
                            "设备已回读确认当前 DPI 阶段表；本地配置与板载配置保存单独处理。"
                        } else {
                            "已读取设备 DPI 阶段表；隐藏阶段和独立 X/Y 设置保留本地配置。"
                        }
                        .into(),
                        Ok((_, Some(error))) => {
                            format!("DPI 阶段表已读取确认，但通信进程退出失败：{error}")
                        }
                        Err(error) => {
                            format!("DPI 阶段表尚未确认：{error:#}。请重新读取设备状态。")
                        }
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }
}

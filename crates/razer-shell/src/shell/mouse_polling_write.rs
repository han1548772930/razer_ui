//! Source 182 polling intent -> shared direct route -> verified current setting.
use super::*;
use razer_discovery::{direct, discovery};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::mouse_polling::{MousePollingScope, PollingField};

impl AppShell {
    pub(super) fn write_mouse_polling(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace
            .read(cx)
            .mouse_polling_request_matches(scope, field, hz, cx)
        {
            return;
        }
        let device = workspace.read(cx).device(cx);
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(device)
                    && route.peer_product_id().is_none()
                    && matches!(route.transport(), Some(discovery::ObservedTransport::Wired))
            })
            .collect::<Vec<_>>();
        if field != PollingField::Wired || routes.len() != 1 {
            self.status = "回报率未发送：需要唯一的有线设备连接，无线分支尚未完成原码接入。".into();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_polling(scope, field, hz, None, cx)
            });
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        let product_id = device.product_id;
        let revision = self.discovery_revision;
        self.device_value_owners.remove(&identity);
        self.device_read_scopes.remove(&identity);
        self.status = "正在设置回报率并读取设备确认。".into();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let selected = route.clone();
            let result = cx
                .background_spawn(async move {
                    use anyhow::Context as _;
                    use razer_device::{
                        device_reads::DeviceReadValue, device_writes::DeviceWriteSetting,
                    };
                    let cap = razer_device::device_reads::capability(product_id)
                        .context("产品回报率读取能力尚未核实")?;
                    let mut client = ServiceClient::spawn()?;
                    let result = (|| match direct::resolve(&mut client, &selected, cap)?
                        .write(&mut client, DeviceWriteSetting::Polling { hz })?
                    {
                        DeviceReadValue::Polling { hz: observed } if observed == hz => Ok(observed),
                        _ => anyhow::bail!("回报率读回与请求不一致"),
                    })();
                    let shutdown_error = client
                        .request(ServiceRequest::Shutdown)
                        .err()
                        .map(|error| format!("{error:#}"));
                    result.map(|observed| (observed, shutdown_error))
                })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if !workspace
                    .read(cx)
                    .mouse_polling_in_flight_matches(scope, field, hz, cx)
                {
                    return;
                }
                if !this.devices.contains(&workspace)
                    || workspace.read(cx).identity(cx) != identity
                    || !workspace
                        .read(cx)
                        .mouse_polling_request_matches(scope, field, hz, cx)
                {
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_mouse_polling(scope, field, hz, None, cx)
                    });
                    return;
                }
                let current = this.discovery_revision == revision
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
                let observed = current
                    .then(|| result.as_ref().ok().map(|(hz, _)| *hz))
                    .flatten();
                this.device_value_owners.remove(&identity);
                this.device_read_scopes.remove(&identity);
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_mouse_polling(scope, field, hz, observed, cx)
                });
                this.status = if !current {
                    "设备连接已变化，回报率尚未确认。".into()
                } else {
                    match result {
                        Ok((_, None)) => {
                            format!("设备已回读确认回报率：{hz} Hz；本地配置单独保存。")
                        }
                        Ok((_, Some(error))) => {
                            format!("设备已确认回报率：{hz} Hz；通信进程退出失败：{error}")
                        }
                        Err(error) => format!("回报率未确认：{error:#}。请重新读取设备状态。"),
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }
}

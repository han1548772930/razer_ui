//! Keyboard brightness intent -> identity-matched direct Feature route -> readback.
use super::*;
use razer_discovery::discovery;
use razer_ipc::{ServiceClient, ServiceRequest};

impl AppShell {
    pub(super) fn write_keyboard_brightness(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        generation: u64,
        percent: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace
            .read(cx)
            .keyboard_brightness_request_current(generation, percent, cx)
        {
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let device = workspace.read(cx).device(cx);
        let product_id = device.product_id;
        let container = device.device_container_id.clone();
        let profile = device.active_profile.clone();
        let revision = self.discovery_revision;
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(device)
                    && route.peer_product_id().is_none()
                    && matches!(route.transport(), Some(discovery::ObservedTransport::Wired))
            })
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            self.status =
                "亮度未发送：没有唯一的有线设备连接。接收器转发写入尚未实现，本地草稿已保留。"
                    .into();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_brightness(generation, percent, None, false, cx)
            });
            cx.notify();
            return;
        };
        let physical_product_id = route.physical_product_id();
        let route_observation = (**route).clone();
        self.status = "正在设置键盘亮度并读取设备确认。".into();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    use anyhow::Context as _;
                    let cap = razer_device::keyboard_settings::capability(product_id)
                        .context("产品亮度写入协议尚未核实")?;
                    let mut client = ServiceClient::spawn()?;
                    let result = (|| {
                        let route = razer_discovery::direct::resolve(
                            &mut client,
                            &route_observation,
                            &cap.transport,
                        )?;
                        route
                            .write_keyboard(&mut client, percent)
                            .map(|observed| observed.percent)
                    })();
                    let shutdown_error = client
                        .request(ServiceRequest::Shutdown)
                        .err()
                        .map(|error| format!("{error:#}"));
                    result.map(|observed| (observed, shutdown_error))
                })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if !this.devices.contains(&workspace) {
                    workspace.update(cx, |workspace, cx| {
                        workspace.cancel_keyboard_brightness_connection(cx);
                        workspace.finish_keyboard_brightness(generation, percent, None, false, cx);
                    });
                    return;
                }
                if !workspace
                    .read(cx)
                    .keyboard_brightness_request_matches(generation, percent, cx)
                {
                    return;
                }
                let route_current = this.discovery_revision == revision
                    && this.device_observations.iter().any(|route| {
                        route.matches(workspace.read(cx).device(cx))
                            && route.container().eq_ignore_ascii_case(&container)
                            && route.physical_product_id() == physical_product_id
                            && route.peer_product_id().is_none()
                            && matches!(
                                route.transport(),
                                Some(discovery::ObservedTransport::Wired)
                            )
                    });
                let scope_current = route_current
                    && workspace.read(cx).identity(cx) == identity
                    && workspace.read(cx).device(cx).active_profile == profile
                    && workspace
                        .read(cx)
                        .keyboard_brightness_request_current(generation, percent, cx);
                let observed = scope_current
                    .then(|| result.as_ref().ok().map(|(observed, _)| *observed))
                    .flatten();
                if scope_current
                    || workspace
                        .read(cx)
                        .keyboard_brightness_request_current(generation, percent, cx)
                {
                    this.status = if !scope_current {
                        "键盘连接已变化，尚未确认当前亮度。本地草稿已保留。".into()
                    } else {
                        match result {
                            Ok((_, None)) => {
                                format!("设备已回读确认键盘亮度：{percent}%。本地保存单独处理。")
                            }
                            Ok((_, Some(error))) => {
                                format!("设备已确认键盘亮度：{percent}%；通信进程退出失败：{error}")
                            }
                            Err(error) => format!(
                                "键盘亮度未确认：{error:#}。本地草稿已保留，请重新读取设备状态。"
                            ),
                        }
                    };
                }
                // Completion may emit the queued newest desired setting. Set
                // status first so its new loading feedback is not overwritten.
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_brightness(
                        generation,
                        percent,
                        observed,
                        scope_current,
                        cx,
                    )
                });
                cx.notify();
            });
        })
        .detach();
    }
}

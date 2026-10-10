//! Actual brightness observation, independent of the local draft and source init setter.
use super::*;
use razer_discovery::{direct, discovery};
use razer_ipc::{ServiceClient, ServiceRequest};

impl AppShell {
    pub(super) fn read_keyboard_brightness(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace
            .read(cx)
            .keyboard_brightness_read_matches(generation, cx)
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
        let [route] = routes.as_slice() else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_brightness_read(
                    generation,
                    None,
                    Some("没有唯一的有线设备观察；接收器与 BLE 读取尚未接通".into()),
                    false,
                    cx,
                )
            });
            return;
        };
        let route = (**route).clone();
        let identity = workspace.read(cx).identity(cx);
        let profile = device.active_profile.clone();
        let product_id = device.product_id;
        let revision = self.discovery_revision;
        if !self.devices.contains(&workspace) {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_keyboard_brightness_read(
                    generation,
                    None,
                    Some("设备页面已关闭".into()),
                    false,
                    cx,
                )
            });
            return;
        }
        cx.spawn_in(window, async move |this, cx| {
            let selected = route.clone();
            let result = cx
                .background_spawn(async move {
                    use anyhow::Context as _;
                    let cap = razer_device::keyboard_settings::capability(product_id)
                        .context("产品亮度读取协议尚未核实")?;
                    let mut client = ServiceClient::spawn()?;
                    let result = (|| {
                        direct::resolve(&mut client, &selected, &cap.transport)?
                            .read_keyboard(&mut client)
                            .map(|reading| reading.percent)
                    })();
                    let shutdown_error = client
                        .request(ServiceRequest::Shutdown)
                        .err()
                        .map(|error| format!("{error:#}"));
                    result.map(|percent| (percent, shutdown_error))
                })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if !workspace
                    .read(cx)
                    .keyboard_brightness_read_matches(generation, cx)
                {
                    return;
                }
                let scope_current = this.devices.contains(&workspace)
                    && this.discovery_revision == revision
                    && workspace.read(cx).identity(cx) == identity
                    && workspace.read(cx).device(cx).active_profile == profile
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
                // Edit revision is owned by the feature. It filters an old read
                // without cancelling the user's queued write from this connection.
                let (observed, error) = match result {
                    Ok((percent, shutdown_error)) => (Some(percent), shutdown_error),
                    Err(error) => (None, Some(format!("{error:#}"))),
                };
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_keyboard_brightness_read(
                        generation,
                        observed,
                        error,
                        scope_current,
                        cx,
                    )
                });
            });
        })
        .detach();
    }
}

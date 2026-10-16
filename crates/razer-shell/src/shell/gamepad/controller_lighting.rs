//! Source profile persistence precedes the mounted controller lighting setter.
//! An acknowledged write, observed read and local draft remain separate results.
use super::*;
use razer_device::{
    backend::HidNode,
    controller_lighting::{LightingOperation, LightingResult, capability},
};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::gamepad_products::ControllerLightingRequested;
use serde::Deserialize;

pub(super) struct ControllerLightingSave {
    workspace: Entity<ProductWorkspace>,
    request: ControllerLightingRequested,
    identity: String,
    profile_id: String,
}

#[derive(Deserialize)]
struct LightingReply {
    node: HidNode,
    product_id: u32,
    operation: LightingOperation,
    device_write_acknowledged: bool,
    device_read_observed: bool,
    lighting_result: LightingResult,
}
impl LightingReply {
    fn result(
        self,
        node: &HidNode,
        product_id: u32,
        operation: &LightingOperation,
    ) -> anyhow::Result<LightingResult> {
        anyhow::ensure!(
            &self.node == node && self.product_id == product_id && &self.operation == operation,
            "Controller lighting response identity differs"
        );
        let observed = self.lighting_result.effect.is_some() || self.lighting_result.brightness.is_some();
        let acknowledged = self.lighting_result.acknowledged_writes > 0;
        anyhow::ensure!(
            self.device_write_acknowledged == acknowledged
                && self.device_read_observed == observed
                && if operation.is_read() { observed && !acknowledged } else { acknowledged },
            "Controller lighting reply does not carry the requested device result"
        );
        Ok(self.lighting_result)
    }
}

impl AppShell {
    pub(super) fn persist_controller_lighting(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ControllerLightingRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if request.operation().is_read() {
            self.submit_controller_lighting(workspace, request, window.window_handle(), cx);
            return;
        }
        if self.storage_error.is_some()
            || !workspace.read(cx).controller_lighting_request_matches(&request, cx)
        {
            let error = self.storage_error.clone().unwrap_or_else(|| "Controller lighting owner changed".into());
            workspace.update(cx, |workspace, cx| {
                workspace.finish_controller_lighting(&request, Err(error), Some(window), cx)
            });
            cx.notify();
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let profile_id = workspace.read(cx).device(cx).active_profile.clone();
        let file = store::WorkspaceFile::new(
            self.devices.iter().map(|entity| entity.read(cx).saved_snapshot(cx)).collect(),
            self.tracking_intro_seen,
        )
        .with_shortcuts(self.shortcuts.read(cx).saved_snapshot())
        .with_preferences(self.settings.read(cx).snapshot())
        .with_custom_colors(cx.global::<CustomColors>().colors())
        .with_host_tab_order(self.host_tabs.order())
        .with_dashboard(self.dashboard_state.read(cx).snapshot())
        .with_module_services(self.module_catalog.read(cx).service_snapshot())
        .with_macros(self.macro_library.read(cx).snapshot());
        self.pending_saves.push_back(PreparedSave {
            leviathan: None,
            window: window.window_handle(),
            file,
            mouse_polling: None,
            mouse_dpi_stages: None,
            mouse_dpi: None,
            mouse_current_dpi: None,
            mouse_current_power: None,
            controller_trigger: None,
            controller_lighting: Some(ControllerLightingSave { workspace, request, identity, profile_id }),
            seiren_eq: None,
            mouse_profile: None,
            keyboard_actuation: None,
        });
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_controller_lighting_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(lighting) = &save.controller_lighting else { return Ok(()); };
        anyhow::ensure!(
            self.devices.contains(&lighting.workspace)
                && lighting.workspace.read(cx).identity(cx) == lighting.identity
                && lighting.workspace.read(cx).controller_lighting_request_matches(&lighting.request, cx),
            "Controller lighting profile owner changed before persistence"
        );
        let mut found = false;
        let mut devices = Vec::with_capacity(self.devices.len());
        for entity in &self.devices {
            let mut device = entity.read(cx).saved_snapshot(cx);
            if entity.read(cx).identity(cx) == lighting.identity {
                let profile = device.profiles.iter_mut().find(|profile| profile.id == lighting.profile_id)
                    .ok_or_else(|| anyhow::anyhow!("Controller lighting profile was removed"))?;
                let settings = profile.source_settings.get_or_insert_with(|| serde_json::json!({}));
                lighting.request.rebase_settings(settings)?;
                found = true;
            }
            devices.push(device);
        }
        anyhow::ensure!(found, "Controller lighting workspace was removed");
        save.file.devices = devices;
        Ok(())
    }

    pub(super) fn finish_controller_lighting_save(
        &mut self,
        save: ControllerLightingSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if succeeded && !self.close_requested {
            self.submit_controller_lighting(save.workspace, save.request, window, cx);
        } else {
            self.finish_controller_lighting_result(
                &save.workspace, &save.request,
                Err(if succeeded { "Controller lighting canceled during shutdown" } else { "Controller lighting local persistence did not complete" }.into()),
                window, cx,
            );
        }
    }

    fn finish_controller_lighting_result(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &ControllerLightingRequested,
        result: Result<LightingResult, String>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if window.update(cx, |_, window, cx| {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_controller_lighting(request, result.clone(), Some(window), cx)
            })
        }).is_err() {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_controller_lighting(request, result, None, cx)
            });
        }
    }

    fn submit_controller_lighting(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ControllerLightingRequested,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested || !workspace.read(cx).controller_lighting_request_matches(&request, cx) {
            self.finish_controller_lighting_result(&workspace, &request, Err("Controller lighting owner changed".into()), window, cx);
            return;
        }
        let routes = self.device_observations.iter().filter(|route| {
            route.matches(workspace.read(cx).device(cx))
                && capability(route.product_id()).is_some_and(|cap| cap.accepts(route.physical_product_id()))
                && route.peer_product_id().is_none()
        }).cloned().collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            self.status = if request.operation().is_read() {
                "当前连接尚不唯一，手柄灯效未读取。"
            } else {
                "手柄灯效已保存到本机；当前连接尚不唯一，设备未写入。"
            }.into();
            self.finish_controller_lighting_result(&workspace, &request, Err("Controller lighting has no unique current physical connection".into()), window, cx);
            cx.notify();
            return;
        };
        let route = route.clone();
        let identity = workspace.read(cx).identity(cx);
        self.controller_lighting_workers += 1;
        cx.spawn(async move |this, cx| {
            let selected = route.clone();
            let guard = request.clone();
            let result = cx.background_spawn(async move {
                anyhow::ensure!(guard.is_valid(), "Controller lighting canceled before submission");
                let mut client = ServiceClient::spawn()?;
                client.set_request_guard(guard.guard());
                let result = (|| {
                    let node = razer_discovery::controller_lighting::resolve(&mut client, &selected)?;
                    let reply: LightingReply = serde_json::from_value(client.request(ServiceRequest::ControllerLighting {
                        node: node.clone(), product_id: selected.product_id(), lighting_operation: guard.operation().clone(),
                    })?)?;
                    reply.result(&node, selected.product_id(), guard.operation())
                })();
                let shutdown = if client.is_stopped() { None } else {
                    client.request(ServiceRequest::Shutdown).err().map(|error| format!("{error:#}"))
                };
                match (result, shutdown) {
                    (Ok(result), shutdown) => Ok((result, shutdown)),
                    (Err(error), Some(shutdown)) => Err(error.context(format!("Controller lighting cleanup also failed: {shutdown}"))),
                    (Err(error), None) => Err(error),
                }
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.controller_lighting_workers = this.controller_lighting_workers.saturating_sub(1);
                let current = this.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == identity
                    && workspace.read(cx).controller_lighting_request_matches(&request, cx)
                    && this.device_observations.iter().any(|observed| {
                        observed.matches(workspace.read(cx).device(cx))
                            && observed.physical_product_id() == route.physical_product_id()
                            && observed.container().eq_ignore_ascii_case(route.container())
                            && observed.hid_node() == route.hid_node()
                    });
                let completion = if current {
                    result.as_ref().map(|(value, _)| value.clone()).map_err(|error| format!("{error:#}"))
                } else {
                    Err("Controller lighting response belongs to a replaced physical owner".into())
                };
                this.finish_controller_lighting_result(&workspace, &request, completion, window, cx);
                if current {
                    this.status = match result {
                        Ok((_, Some(error))) => format!("手柄灯效设备结果已确认；通信进程退出失败：{error}"),
                        Ok((_, None)) => if request.operation().is_read() { "已读取设备手柄灯效。" } else { "手柄灯效已保存到本机，设备写入应答已确认。" }.into(),
                        Err(error) => format!("手柄灯效设备操作未确认：{error:#}；本地设置保留。"),
                    };
                }
                if this.close_requested && this.save_task.is_none() {
                    this.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        }).detach();
    }
}

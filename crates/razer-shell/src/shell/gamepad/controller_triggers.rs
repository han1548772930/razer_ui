//! Local profile writer continuation -> unique current controller -> firmware
//! acknowledgement. The source's optimistic controls are not rolled back.
use super::*;
use razer_device::controller_triggers::TriggerOperation;
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::gamepad_products::ControllerTriggerRequested;

pub(super) struct ControllerTriggerSave {
    workspace: Entity<ProductWorkspace>,
    request: ControllerTriggerRequested,
    identity: String,
    profile_id: String,
}

impl AppShell {
    pub(super) fn persist_controller_trigger(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ControllerTriggerRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if request.operation().is_read() {
            self.submit_controller_trigger(workspace, request, window.window_handle(), cx);
            return;
        }
        if self.storage_error.is_some()
            || !workspace
                .read(cx)
                .controller_trigger_request_matches(&request, cx)
        {
            if let Some(error) = &self.storage_error {
                self.status = format!("手柄设置本地保存失败：{error}");
            }
            workspace.update(cx, |workspace, cx| {
                workspace.finish_controller_trigger(&request, None, Some(window), cx)
            });
            cx.notify();
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let profile_id = workspace.read(cx).device(cx).active_profile.clone();
        let file = store::WorkspaceFile::new(
            self.devices
                .iter()
                .map(|e| e.read(cx).saved_snapshot(cx))
                .collect(),
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
            mouse_profile: None,
            keyboard_actuation: None,
            seiren_eq: None,
            mouse_dpi: None,
            mouse_current_dpi: None,
            mouse_current_power: None,
            controller_lighting: None,
            controller_trigger: Some(ControllerTriggerSave {
                workspace,
                request,
                identity,
                profile_id,
            }),
        });
        self.start_pending_save(cx);
    }
    pub(super) fn rebase_controller_trigger_save(&self, save: &mut PreparedSave, cx: &App) {
        let Some(trigger) = &save.controller_trigger else {
            return;
        };
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let mut saved = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == trigger.identity {
                    if let Some(profile) = saved
                        .profiles
                        .iter_mut()
                        .find(|p| p.id == trigger.profile_id)
                    {
                        let settings = profile.source_settings.get_or_insert_with(
                            || serde_json::json!({"profile":{},"controller":{}}),
                        );
                        if !settings.is_object() {
                            *settings = serde_json::json!({"profile":{},"controller":{}});
                        }
                        if !settings["profile"].is_object() {
                            settings["profile"] = serde_json::json!({});
                        }
                        if !settings["controller"].is_object() {
                            settings["controller"] = serde_json::json!({});
                        }
                        if trigger.request.operation().is_thumbstick() {
                            rebase_thumbstick_settings(
                                settings,
                                trigger.request.profile(),
                                trigger.request.operation(),
                            );
                        } else {
                            for key in ["controllerAnalogActuation", "controllerDigitalActuation"] {
                                settings["profile"][key] = trigger.request.profile()[key].clone();
                            }
                            for (key, array, analog) in [
                                ("controllerAnalogActuation", "analogActuations", true),
                                ("controllerDigitalActuation", "digitalActuations", false),
                            ] {
                                if let Some(entries) =
                                    trigger.request.profile()[key][array].as_array()
                                {
                                    for entry in entries {
                                        let side = match entry["controllerId"].as_u64() {
                                            Some(1) => "leftTrigger",
                                            Some(2) => "rightTrigger",
                                            _ => continue,
                                        };
                                        if !settings["controller"][side].is_object() {
                                            settings["controller"][side] = serde_json::json!({});
                                        }
                                        let target = &mut settings["controller"][side];
                                        let active = entry["isActive"].as_bool().unwrap_or(false);
                                        target["mode"] =
                                            serde_json::json!(if active == analog { 0 } else { 1 });
                                        for field in if analog {
                                            &["startRange", "endRange"][..]
                                        } else {
                                            &["actuationPoint", "isRapidTrigger"][..]
                                        } {
                                            target[*field] = entry[*field].clone();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                saved
            })
            .collect();
    }
    pub(super) fn finish_controller_trigger_save(
        &mut self,
        save: ControllerTriggerSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if succeeded && !self.close_requested {
            self.submit_controller_trigger(save.workspace, save.request, window, cx);
        } else {
            self.finish_controller_trigger_result(&save.workspace, &save.request, None, window, cx);
        }
    }
    fn finish_controller_trigger_result(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &ControllerTriggerRequested,
        observed: Option<serde_json::Value>,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        // A closed window cannot strand the feature queue or its scope guard.
        if window
            .update(cx, |_, window, cx| {
                workspace.update(cx, |w, cx| {
                    w.finish_controller_trigger(request, observed.clone(), Some(window), cx)
                })
            })
            .is_err()
        {
            workspace.update(cx, |w, cx| {
                w.finish_controller_trigger(request, observed, None, cx)
            });
        }
    }
    fn submit_controller_trigger(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: ControllerTriggerRequested,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !workspace
                .read(cx)
                .controller_trigger_request_matches(&request, cx)
        {
            self.finish_controller_trigger_result(&workspace, &request, None, window, cx);
            return;
        }
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && razer_device::controller_triggers::capability(route.product_id())
                        .is_some_and(|c| c.accepts(route.physical_product_id()))
                    && route.peer_product_id().is_none()
            })
            .cloned()
            .collect::<Vec<_>>();
        let [route] = routes.as_slice() else {
            self.status = if request.operation().is_read() {
                "当前连接尚不唯一，手柄设置未读取。"
            } else {
                "手柄设置已保存到本机；当前连接尚不唯一，设备未写入。"
            }
            .into();
            self.finish_controller_trigger_result(&workspace, &request, None, window, cx);
            cx.notify();
            return;
        };
        let route = route.clone();
        let identity = workspace.read(cx).identity(cx);
        self.controller_trigger_workers += 1;
        cx.spawn(async move |this, cx| {
            let selected = route.clone();
            let guard = request.clone();
            let result = cx
                .background_spawn(async move {
                    anyhow::ensure!(
                        guard.is_valid(),
                        "Controller trigger scope canceled before submission"
                    );
                    let mut client = ServiceClient::spawn()?;
                    client.set_request_guard(guard.guard());
                    let result = (|| {
                        let node =
                            razer_discovery::controller_triggers::resolve(&mut client, &selected)?;
                        let report = client.request(ServiceRequest::ControllerTriggers {
                            node: node.clone(),
                            product_id: selected.product_id(),
                            trigger_operation: guard.operation().clone(),
                        })?;
                        anyhow::ensure!(
                            report["node"] == serde_json::to_value(&node)?
                                && report["product_id"] == selected.product_id()
                                && report["operation"] == serde_json::to_value(guard.operation())?,
                            "Controller trigger response identity differs"
                        );
                        anyhow::ensure!(
                            if guard.operation().is_read() {
                                report["device_read_observed"] == true
                                    && report["profile"].is_object()
                            } else {
                                report["device_write_acknowledged"] == true
                                    && report["profile"].is_null()
                            },
                            "Controller trigger reply has no genuine result"
                        );
                        Ok::<_, anyhow::Error>(report)
                    })();
                    let shutdown = if client.is_stopped() {
                        None
                    } else {
                        client
                            .request(ServiceRequest::Shutdown)
                            .err()
                            .map(|e| format!("{e:#}"))
                    };
                    result.map(|value| (value, shutdown))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.controller_trigger_workers = this.controller_trigger_workers.saturating_sub(1);
                let current = this.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == identity
                    && workspace
                        .read(cx)
                        .controller_trigger_request_matches(&request, cx)
                    && this.device_observations.iter().any(|observed| {
                        observed.matches(workspace.read(cx).device(cx))
                            && observed.physical_product_id() == route.physical_product_id()
                            && observed.container().eq_ignore_ascii_case(route.container())
                            && observed.hid_node() == route.hid_node()
                    });
                let observed = current
                    .then(|| result.as_ref().ok().map(|(value, _)| value.clone()))
                    .flatten();
                this.finish_controller_trigger_result(&workspace, &request, observed, window, cx);
                if current {
                    this.status = match result {
                        Ok((_, Some(error))) => {
                            format!("手柄设备操作已确认；通信进程退出失败：{error}")
                        }
                        Ok((_, None)) => if request.operation().is_read() {
                            "已读取设备手柄设置。"
                        } else {
                            "手柄设置已保存到本机，设备写入应答已确认。"
                        }
                        .into(),
                        Err(error) => format!("手柄设备操作未确认：{error:#}；本地设置保留。"),
                    };
                }
                if this.close_requested && this.save_task.is_none() {
                    this.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// Current middleware handler storage updates replace only their own storageKey.
/// Keep unrelated settings from the latest snapshot when an older save starts.
fn rebase_thumbstick_settings(
    settings: &mut serde_json::Value,
    profile: &serde_json::Value,
    operation: &TriggerOperation,
) {
    let fields = match operation {
        TriggerOperation::ThumbstickDeadzone { .. } => {
            Some(("controllerDeadzone", "deadzones", "deadzoneValue"))
        }
        TriggerOperation::ThumbstickSensitivity { .. } => Some((
            "controllerSensitivity",
            "sensitivityValues",
            "sensitivityValue",
        )),
        _ => None,
    };
    if let Some((key, array, field)) = fields {
        let source = &profile[key];
        if let Some(entries) = source[array].as_array() {
            settings["profile"][key] = source.clone();
            for entry in entries {
                let side = match entry["controllerId"].as_u64() {
                    Some(1) => "leftStick",
                    Some(2) => "rightStick",
                    _ => continue,
                };
                if !settings["controller"][side].is_object() {
                    settings["controller"][side] = serde_json::json!({});
                }
                settings["controller"][side][field] = entry["value"].clone();
                if key == "controllerDeadzone" {
                    settings["controller"][side]["preventDoubleDeadzone"] =
                        source["preventDoubleDeadzone"].clone();
                }
            }
        }
    } else if matches!(operation, TriggerOperation::Circularity { .. }) {
        if let Some(mode) = profile["controllerAnalogCircularity"].as_str() {
            settings["profile"]["controllerAnalogCircularity"] = serde_json::json!(mode);
            settings["controller"]["circularity"] =
                serde_json::json!({"mode":if mode == "standard" { 0 } else { 1 }});
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{TriggerOperation, rebase_thumbstick_settings};
    use razer_device::controller_triggers::ThumbstickValue;
    use serde_json::json;

    #[test]
    fn queued_thumbstick_save_preserves_newer_unrelated_settings() {
        let latest = json!({
            "profile": {
                "controllerDeadzone": {"deadzones":[{"controllerId":1,"value":9}],"preventDoubleDeadzone":true},
                "controllerSensitivity": {"sensitivityValues":[{"controllerId":1,"value":20}]},
                "controllerAnalogCircularity":"standard",
                "mappings":[{"inputID":"BUTTON_M1"}]
            },
            "controller": {
                "leftStick":{"deadzoneValue":9,"sensitivityValue":20,"preventDoubleDeadzone":true},
                "circularity":{"mode":0}
            }
        });
        let older = json!({
            "controllerDeadzone": {"deadzones":[{"controllerId":1,"value":5}],"preventDoubleDeadzone":false},
            "controllerSensitivity": {"sensitivityValues":[{"controllerId":1,"value":60}]},
            "controllerAnalogCircularity":"circular"
        });
        let cases = [
            (
                TriggerOperation::ThumbstickDeadzone {
                    profile_slot: 0,
                    settings: vec![ThumbstickValue::new(1, 5).unwrap()],
                    prevent_double_deadzone: false,
                },
                "controllerDeadzone",
            ),
            (
                TriggerOperation::ThumbstickSensitivity {
                    profile_slot: 0,
                    settings: vec![ThumbstickValue::new(1, 60).unwrap()],
                },
                "controllerSensitivity",
            ),
            (
                TriggerOperation::Circularity {
                    profile_slot: 0,
                    circular: true,
                },
                "controllerAnalogCircularity",
            ),
        ];
        for (operation, changed) in cases {
            let mut saved = latest.clone();
            rebase_thumbstick_settings(&mut saved, &older, &operation);
            assert_eq!(saved["profile"][changed], older[changed]);
            for field in [
                "controllerDeadzone",
                "controllerSensitivity",
                "controllerAnalogCircularity",
                "mappings",
            ] {
                if field != changed {
                    assert_eq!(saved["profile"][field], latest["profile"][field]);
                }
            }
            if changed == "controllerDeadzone" {
                assert_eq!(saved["controller"]["leftStick"]["deadzoneValue"], 5);
                assert_eq!(
                    saved["controller"]["leftStick"]["preventDoubleDeadzone"],
                    false
                );
            } else {
                assert_eq!(saved["controller"]["leftStick"]["deadzoneValue"], 9);
                assert_eq!(
                    saved["controller"]["leftStick"]["preventDoubleDeadzone"],
                    true
                );
            }
            assert_eq!(
                saved["controller"]["leftStick"]["sensitivityValue"],
                if changed == "controllerSensitivity" { 60 } else { 20 }
            );
            assert_eq!(
                saved["controller"]["circularity"]["mode"],
                if changed == "controllerAnalogCircularity" {
                    1
                } else {
                    0
                }
            );
        }
    }
}

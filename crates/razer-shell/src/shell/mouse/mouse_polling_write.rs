//! Current mouse polling intents, source profile persistence and device tasks.
use super::mouse_polling_runtime::{self, Request};
use super::*;
use razer_pages::features::mouse_polling::{MousePollingScope, PollingField};

pub(super) struct InGameSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    profile_id: String,
    setting: serde_json::Value,
}
impl InGameSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

enum Accepted {
    Rate { field: PollingField, hz: u32 },
    InGame(serde_json::Value),
}

/// Source Ct and Mt change independent fields and advance the same device
/// version. Queue rebasing preserves later durable changes in other fields.
fn merge_profile(saved: &mut Device, profile_id: &str, accepted: Accepted) -> anyhow::Result<()> {
    let profile = saved
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| anyhow::anyhow!("Polling profile was removed before persistence"))?;
    let mut settings = profile
        .source_settings
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));
    anyhow::ensure!(
        settings.is_object(),
        "Polling local profile is not an object"
    );
    let next_version = if matches!(saved.product_id, 182 | 190) {
        let root = saved.source_device_settings.as_ref();
        anyhow::ensure!(
            root.is_none_or(serde_json::Value::is_object),
            "Polling device settings is not an object"
        );
        let previous = root
            .and_then(|root| root.get("version"))
            .map(|value| {
                value.as_u64().ok_or_else(|| {
                    anyhow::anyhow!("Polling local version is not an unsigned integer")
                })
            })
            .transpose()?;
        Some(razer_device::mapping_submission::next_source_version(
            previous,
        )?)
    } else {
        None
    };
    let mut typed = profile.settings.clone();
    match accepted {
        Accepted::Rate { field, hz } => {
            let allowed = if saved.product_id == 190 {
                &razer_device::mouse_current_polling::UI_RATES[..]
            } else {
                &[125, 500, 1000][..]
            };
            // Receiver task callers may retain a source high-rate request
            // whose actual sender caps it; the source selection persists.
            anyhow::ensure!(
                allowed.contains(&hz)
                    || saved.product_id == 182 && [2000, 4000, 8000].contains(&hz),
                "Polling selection is outside the current product choices"
            );
            let key = if settings.get("PollingRate").is_some() {
                "PollingRate"
            } else {
                field.key()
            };
            settings[key] = serde_json::json!(hz);
            if matches!(saved.product_id, 182 | 190) {
                let markers = settings
                    .as_object_mut()
                    .expect("validated settings")
                    .entry("_pollingLocalFieldsV1")
                    .or_insert_with(|| serde_json::json!([]));
                let markers = markers.as_array_mut().ok_or_else(|| {
                    anyhow::anyhow!("Polling local field markers are not an array")
                })?;
                if !markers.iter().any(|marker| marker == key) {
                    markers.push(serde_json::json!(key));
                }
                if let Some(typed) = &mut typed {
                    match field {
                        PollingField::Wired => typed.polling_wired = Some(hz),
                        PollingField::Wireless => typed.polling_wireless = Some(hz),
                    }
                }
            }
        }
        Accepted::InGame(setting) => {
            anyhow::ensure!(saved.product_id == 190, "In-game polling belongs to PID190");
            razer_pages::features::mouse_products::validate_in_game_polling(&setting)?;
            settings["inGamePollingRate"] = setting;
        }
    }
    let profile = saved
        .profiles
        .iter_mut()
        .find(|profile| profile.id == profile_id)
        .expect("validated profile");
    profile.source_settings = Some(settings);
    profile.settings = typed;
    if let Some(version) = next_version {
        saved
            .source_device_settings
            .get_or_insert_with(|| serde_json::json!({}))["version"] = serde_json::json!(version);
    }
    Ok(())
}

#[cfg(test)]
#[path = "mouse_polling_save_tests.rs"]
mod tests;

impl AppShell {
    pub(super) fn persist_mouse_in_game_polling(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !self.devices.contains(&workspace)
            || workspace.read(cx).device(cx).product_id != 190
        {
            return;
        }
        if let Some(error) = &self.storage_error {
            self.status = format!("游戏内回报率未保存：{error}");
            cx.notify();
            return;
        }
        let device = workspace.read(cx).snapshot(cx);
        let Some(profile) = device.active_profile_obj() else {
            return;
        };
        let Some(setting) = profile
            .source_settings
            .as_ref()
            .and_then(|settings| settings.get("inGamePollingRate"))
            .cloned()
        else {
            return;
        };
        let identity = workspace.read(cx).identity(cx);
        let profile_id = profile.id.clone();
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|entity| entity.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::MouseInGamePolling(InGameSave {
                workspace,
                identity,
                profile_id,
                setting,
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }
    pub(super) fn write_mouse_polling(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(workspace.read(cx).device(cx).product_id, 70 | 182 | 190) {
            self.persist_mouse_polling(workspace, scope, field, hz, window, cx);
        } else {
            self.write_mouse_polling_device(workspace, scope, field, hz, window, cx);
        }
    }

    pub(super) fn write_mouse_polling_device(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !self.devices.contains(&workspace)
            || !workspace
                .read(cx)
                .mouse_polling_request_matches(scope, field, hz, cx)
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_polling(scope, field, hz, None, cx)
            });
            return;
        }
        let device = workspace.read(cx).device(cx);
        let Some(device_hz) = workspace
            .read(cx)
            .mouse_polling_device_hz(scope, field, hz, cx)
        else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_polling(scope, field, hz, None, cx)
            });
            return;
        };
        let change = if device.product_id == 190 {
            let mut change = razer_device::mouse_current_polling::Change::new(device_hz);
            if workspace
                .read(cx)
                .mouse_polling_synchronizes_profile(scope, field, hz, cx)
            {
                let Some(profile) = device
                    .active_profile_obj()
                    .filter(|profile| !profile.guid.is_empty())
                else {
                    self.status = "回报率未写入：当前本地配置缺少实际 profile GUID。".into();
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_mouse_polling(scope, field, hz, None, cx)
                    });
                    cx.notify();
                    return;
                };
                change = change.with_profile_guid(profile.guid.clone());
            }
            Some(change)
        } else {
            None
        };
        let Some(guard) = workspace
            .read(cx)
            .mouse_polling_write_guard(scope, field, hz, cx)
        else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_polling(scope, field, hz, None, cx)
            });
            return;
        };
        let routes = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(device) && mouse_polling_runtime::supports_route(route, field)
            })
            .collect::<Vec<_>>();
        if routes.len() != 1 {
            self.status = "回报率配置保留；没有唯一匹配的当前设备连接，设备未写入。".into();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_polling(scope, field, hz, None, cx)
            });
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        self.device_value_owners.remove(&identity);
        self.device_read_scopes.remove(&identity);
        self.status = "正在设置回报率并读取设备确认。".into();
        cx.notify();
        // Window lifetime does not own admission, result publication or cleanup.
        let _ = window;
        self.start_mouse_polling_operation(
            workspace,
            route,
            Request::new(scope, field, hz, device_hz, change, guard),
            cx,
        );
    }

    pub(super) fn rebase_mouse_polling_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let (identity, profile_id) = match &save.product {
            Some(ProductSave::MousePolling(polling)) => (&polling.identity, &polling.profile.id),
            Some(ProductSave::MouseInGamePolling(setting)) => {
                (&setting.identity, &setting.profile_id)
            }
            _ => return Ok(()),
        };
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let mut saved = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == *identity {
                    let accepted = match &save.product {
                        Some(ProductSave::MousePolling(polling)) => Accepted::Rate {
                            field: polling.field,
                            hz: polling.hz,
                        },
                        Some(ProductSave::MouseInGamePolling(setting)) => {
                            Accepted::InGame(setting.setting.clone())
                        }
                        _ => unreachable!("matched product save"),
                    };
                    merge_profile(&mut saved, profile_id, accepted)?;
                }
                Ok(saved)
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(())
    }
}

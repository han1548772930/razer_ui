use super::*;
use razer_discovery::direct;
use razer_model::model::Profile;
use razer_pages::features::mouse_products::MouseCurrentDpiRequested;

pub(super) struct MouseCurrentDpiSave {
    workspace: Entity<ProductWorkspace>,
    request: MouseCurrentDpiRequested,
    identity: String,
    profile: Profile,
}
impl MouseCurrentDpiSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

/// Rebase only the accepted DPI fields onto the latest durable profile. The
/// current182/190 stage task makers advance the device document's 53-bit version.
fn merge_current_dpi_profile(
    target: &mut Device,
    profile_id: &str,
    source: &serde_json::Value,
) -> anyhow::Result<()> {
    let mut candidate = target.clone();
    let saved = &mut candidate;
    anyhow::ensure!(source.is_object(), "DPI source profile is not an object");
    let profile = saved
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| anyhow::anyhow!("DPI profile was removed before persistence"))?;
    let mut settings = profile
        .source_settings
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));
    anyhow::ensure!(settings.is_object(), "DPI local profile is not an object");
    let next_version = if matches!(saved.product_id, 182 | 190) {
        let root = saved.source_device_settings.as_ref();
        anyhow::ensure!(
            root.is_none_or(serde_json::Value::is_object),
            "DPI local device settings is not an object"
        );
        let previous = root
            .and_then(|root| root.get("version"))
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("DPI local version is not an unsigned integer"))
            })
            .transpose()?;
        Some(razer_device::mapping_submission::next_source_version(
            previous,
        )?)
    } else {
        None
    };
    let keys = if saved.product_id == 182 {
        &["dpiStages"][..]
    } else {
        &["dpiStages", "dpi", "DPIStages", "DPI", "version"][..]
    };
    for key in keys {
        if let Some(value) = source.get(key) {
            settings[*key] = value.clone();
        }
    }
    saved
        .profiles
        .iter_mut()
        .find(|profile| profile.id == profile_id)
        .expect("validated profile identity")
        .source_settings = Some(settings);
    if let Some(version) = next_version {
        saved
            .source_device_settings
            .get_or_insert_with(|| serde_json::json!({}))["version"] = serde_json::json!(version);
    }
    razer_storage::host_local_profile::mark_edited(saved, profile_id, source)?;
    *target = candidate;
    Ok(())
}

#[cfg(test)]
#[path = "mouse_current_dpi_save_tests.rs"]
mod tests;

impl AppShell {
    /// Source profile initialization precedes the connection's DPI observation.
    /// The page retains the read key and serializes refresh behind accepted edits.
    pub(super) fn initialize_current_mouse_profiles(&mut self, cx: &mut Context<Self>) {
        if self.close_requested || self.storage_error.is_some() {
            return;
        }
        let macro_recording = self.receiver_macro_exclusive(cx);
        for workspace in &self.devices {
            let device = workspace.read(cx).device(cx);
            if razer_device::mouse_current_dpi::capability(device.product_id).is_none() {
                continue;
            }
            let routes: Vec<_> = self
                .device_observations
                .iter()
                .filter(|route| {
                    route.matches(device) && direct::mouse_current_dpi::available(route)
                })
                .collect();
            let key = if let [route] = routes.as_slice() {
                Some(format!(
                    "{}:{}:{:?}:{:?}",
                    route.container(),
                    route.physical_product_id(),
                    route.peer_product_id(),
                    route.hid_node()
                ))
            } else {
                None
            };
            workspace.update(cx, |workspace, cx| {
                workspace.initialize_current_mouse_profile(key, macro_recording, cx)
            });
        }
    }

    pub(super) fn persist_current_dpi(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCurrentDpiRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if request.profile().is_none() {
            self.submit_current_dpi_device(workspace, request, window, cx);
            return;
        }
        if self.storage_error.is_some()
            || !workspace.read(cx).current_dpi_request_matches(&request, cx)
        {
            if let Some(error) = &self.storage_error {
                self.status = format!("DPI 本地保存失败：{error}");
            }
            workspace.update(cx, |workspace, cx| {
                workspace.finish_current_dpi(&request, None, cx)
            });
            if self.storage_error.is_some() {
                self.cancel_pending_exit(cx);
            }
            cx.notify();
            return;
        }
        let device = workspace.read(cx).snapshot(cx);
        let Some(profile) = device
            .profiles
            .iter()
            .find(|profile| profile.id == device.active_profile)
            .cloned()
        else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_current_dpi(&request, None, cx)
            });
            return;
        };
        let identity = workspace.read(cx).identity(cx);
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
            Some(ProductSave::MouseCurrentDpi(MouseCurrentDpiSave {
                workspace,
                request,
                identity,
                profile,
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_current_dpi_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::MouseCurrentDpi(dpi)) = &save.product else {
            return Ok(());
        };
        let Some(source) = dpi.request.profile() else {
            return Ok(());
        };
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| -> anyhow::Result<Device> {
                let mut saved = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == dpi.identity {
                    merge_current_dpi_profile(&mut saved, &dpi.profile.id, source)?;
                }
                Ok(saved)
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(())
    }

    pub(super) fn finish_current_dpi_save(
        &mut self,
        save: MouseCurrentDpiSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !succeeded
            || save.request.change().is_none() && !save.request.is_read()
            || self.close_requested
        {
            save.workspace.update(cx, |workspace, cx| {
                workspace.finish_current_dpi(&save.request, None, cx)
            });
            return;
        }
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let workspace = save.workspace.clone();
            let request = save.request.clone();
            let launched = window.update(cx, |_, window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.submit_current_dpi_device(save.workspace, save.request, window, cx)
                });
            });
            if launched.is_err() {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_current_dpi(&request, None, cx)
                });
            }
        });
    }

    pub(super) fn submit_current_dpi_device(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCurrentDpiRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !self.devices.contains(&workspace)
            || !workspace.read(cx).current_dpi_request_matches(&request, cx)
            || request.change().is_none() && !request.is_read()
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_current_dpi(&request, None, cx)
            });
            return;
        }
        let routes: Vec<_> = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && direct::mouse_current_dpi::available(route)
            })
            .collect();
        if routes.len() != 1 {
            self.status = "DPI 设备状态未确认：尚无唯一的当前鼠标或接收器连接。".into();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_current_dpi(&request, None, cx)
            });
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        self.status = "正在核对 DPI 设备应答与读回结果。".into();
        cx.notify();
        self.start_current_dpi_operation(workspace, route, request, window.window_handle(), cx);
    }
}

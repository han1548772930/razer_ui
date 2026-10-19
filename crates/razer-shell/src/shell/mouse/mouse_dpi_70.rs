//! Current PID70 local sensitivity persistence -> active-DPI setter ACK.
use super::*;
use razer_discovery::discovery;
use razer_pages::features::mouse_products::MouseLegacyDpiRequested;

#[path = "mouse_dpi_70_runtime.rs"]
mod runtime;
#[path = "mouse_dpi_70_worker.rs"]
mod worker;
pub(super) use runtime::State;

pub(super) fn prepare_mapping_document(
    device: &mut Device,
    macros: &razer_pages::features::macro_library::MacroLibraryFile,
) -> anyhow::Result<()> {
    let Some(product) =
        razer_device::mapping_document::MappingProduct::for_product_id(device.product_id)
    else {
        return Ok(());
    };
    let guids = (0..256).map(|_| uuid::Uuid::new_v4().to_string());
    if let Some(document) = razer_model::mapping::prepare_document(device, macros, guids)? {
        device
            .source_device_settings
            .get_or_insert_with(|| serde_json::json!({}))[product.storage_field()] = document;
    }
    Ok(())
}

impl AppShell {
    pub(super) fn navigate_legacy_mouse_profile(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        event: &razer_pages::features::mouse_products::MouseLegacyProfileNavigate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if workspace.read(cx).mouse_polling_scope(cx) != Some(event.scope) {
            return;
        }
        let device = workspace.read(cx).snapshot(cx);
        let mut profiles: Vec<_> = device.profiles.iter().collect();
        profiles.sort_by_key(|profile| {
            profile
                .name
                .to_uppercase()
                .encode_utf16()
                .collect::<Vec<_>>()
        });
        let Some(index) = profiles
            .iter()
            .position(|profile| profile.id == device.active_profile)
        else {
            return;
        };
        let target = if event.output["type"] == "switchProfile" {
            profiles.iter().position(|profile| {
                profile.guid == event.output["guid"].as_str().unwrap_or_default()
            })
        } else {
            match event.output["name"].as_str().unwrap_or_default() {
                "NextProfile" => Some((index + 1).min(profiles.len() - 1)),
                "PreviousProfile" => Some(index.saturating_sub(1)),
                "CycleUp" => Some((index + 1) % profiles.len()),
                "CycleDown" => Some((index + profiles.len() - 1) % profiles.len()),
                _ => None,
            }
        };
        if let Some(target) = target {
            let id = profiles[target].id.clone();
            workspace.update(cx, |workspace, cx| {
                workspace.select_profile(&id, window, cx)
            });
        }
    }
    pub(super) fn persist_legacy_mouse_profile(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: razer_pages::features::mouse_polling::MousePollingScope,
        apply: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(&workspace)
            || !matches!(
                workspace.read(cx).device(cx).product_id,
                70 | 158 | 182 | 190
            )
            || workspace.read(cx).mouse_polling_scope(cx) != Some(scope)
        {
            return;
        }
        self.persist_legacy_profile(workspace, Some(scope), apply, None, window, cx);
    }

    pub(super) fn persist_legacy_keyboard_profile(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(&workspace)
            || !razer_device::mapping_document::MappingProduct::for_product_id(
                workspace.read(cx).device(cx).product_id,
            )
            .is_some_and(|product| product.is_keyboard())
        {
            return;
        }
        self.persist_legacy_profile(workspace, None, false, None, window, cx);
    }

    pub(super) fn persist_legacy_profile(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: Option<razer_pages::features::mouse_polling::MousePollingScope>,
        apply: bool,
        scroll: Option<razer_pages::features::mouse_products::MouseScrollRequested>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(error) = &self.storage_error {
            self.status = format!("配置文件未保存：{error}");
            self.cancel_pending_exit(cx);
            cx.notify();
            return;
        }
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|entity| entity.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        let identity = workspace.read(cx).identity(cx);
        let device = workspace.read(cx).snapshot(cx);
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::MouseProfile(MouseLegacyProfileSave {
                workspace,
                scope,
                identity,
                device,
                apply,
                scroll,
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn persist_legacy_dpi(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseLegacyDpiRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let valid = self.storage_error.is_none()
            && workspace.read(cx).device(cx).product_id == 70
            && workspace.read(cx).legacy_dpi_request_matches(&request, cx);
        let device = workspace.read(cx).snapshot(cx);
        let profile = device
            .profiles
            .iter()
            .find(|p| p.id == device.active_profile)
            .cloned();
        if !valid || profile.is_none() {
            if let Some(error) = &self.storage_error {
                eprintln!("Mouse DPI profile cannot be persisted: {error}");
                self.status = format!("DPI 本地保存失败：{error}");
            }
            workspace.update(cx, |workspace, cx| {
                workspace.finish_legacy_dpi(&request, cx)
            });
            if self.storage_error.is_some() || request.cleanup {
                self.cancel_pending_exit(cx);
                cx.notify();
            }
            return;
        }
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|entity| entity.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        let identity = workspace.read(cx).identity(cx);
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::MouseLegacyDpi(MouseLegacyDpiSave {
                workspace,
                request,
                identity,
                profile: profile.expect("checked active profile"),
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn submit_legacy_dpi_device(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseLegacyDpiRequested,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested && !request.cleanup
            || !self.devices.contains(&workspace)
            || !workspace.read(cx).legacy_dpi_request_matches(&request, cx)
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_legacy_dpi(&request, cx)
            });
            if request.cleanup {
                self.status = "DPI 退出恢复尚未确认：当前设备连接或请求范围已改变。".into();
                self.cancel_pending_exit(cx);
            }
            return;
        }
        let context = if request.persist {
            "DPI 已保存到本机"
        } else {
            "临时 DPI 设置"
        };
        let Some((x, y)) = request.dpi else {
            if request.send {
                self.status = format!("{context}；所选阶段没有有效 DPI 值，未发送设备设置。");
            }
            workspace.update(cx, |workspace, cx| {
                workspace.finish_legacy_dpi(&request, cx)
            });
            if request.cleanup {
                self.cancel_pending_exit(cx);
            }
            return;
        };
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
        if routes.len() != 1 {
            self.status = format!("{context}；未发送到设备，需要唯一的有线连接。");
            workspace.update(cx, |workspace, cx| {
                workspace.finish_legacy_dpi(&request, cx)
            });
            if request.cleanup {
                self.cancel_pending_exit(cx);
            }
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        self.start_legacy_dpi_operation(workspace, route, request, context, x, y, cx);
    }
}

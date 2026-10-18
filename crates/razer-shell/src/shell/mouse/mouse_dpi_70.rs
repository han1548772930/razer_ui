//! Current PID70 local sensitivity persistence -> active-DPI setter ACK.
use super::*;
use razer_discovery::{direct, discovery};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::mouse_products::MouseLegacyDpiRequested;

pub(super) fn prepare_mapping_document(
    device: &mut Device,
    macros: &razer_pages::features::macro_library::MacroLibraryFile,
) -> anyhow::Result<()> {
    let Some(product) =
        razer_device::mapping_document::MappingProduct::for_product_id(device.product_id)
    else {
        return Ok(());
    };
    let profiles = device
        .profiles
        .iter()
        .map(|profile| {
            let mut source = profile
                .source_settings
                .clone()
                .unwrap_or_else(|| serde_json::json!({}));
            source["guid"] = serde_json::json!(profile.guid);
            source["name"] = serde_json::json!(profile.name);
            source
        })
        .collect::<Vec<_>>();
    let active = device
        .profiles
        .iter()
        .find(|profile| profile.id == device.active_profile)
        .ok_or_else(|| anyhow::anyhow!("Active mouse profile is missing"))?;
    let key = product.storage_field();
    let previous = device
        .source_device_settings
        .as_ref()
        .and_then(|settings| settings.get(key));
    let guids = (0..256).map(|_| uuid::Uuid::new_v4().to_string());
    let mut document = if device.product_id == 756 {
        razer_device::keyboard_latest_mapping::prepare_document_with_device_settings(
            &profiles,
            &active.guid,
            device.layout_id,
            previous,
            device.source_device_settings.as_ref(),
            guids,
        )?
    } else if razer_device::keyboard_magnetic_mapping::supports(device.product_id) {
        razer_device::keyboard_magnetic_mapping::prepare_document(
            device.product_id,
            &profiles,
            &active.guid,
            device.layout_id,
            previous,
            guids,
        )?
    } else if device.product_id == 679 {
        razer_device::keyboard_current_mapping::prepare_document(
            &profiles,
            &active.guid,
            device.layout_id,
            previous,
            guids,
        )?
    } else if device.product_id == 70 {
        razer_device::mouse_legacy_mapping::prepare_document(
            &profiles,
            &active.guid,
            previous,
            guids,
        )?
    } else {
        razer_device::keyboard_legacy_mapping::prepare_document(
            &profiles,
            &active.guid,
            previous,
            guids,
        )?
    };
    match macros
        .source_macro_documents()
        .map_err(anyhow::Error::msg)
        .and_then(|macros| razer_device::macro_engine::publication_documents(&macros))
    {
        Ok(macros) => {
            document["macros"] = serde_json::json!(macros);
            document["phasedMacros"] = serde_json::json!(macros);
            document
                .as_object_mut()
                .unwrap()
                .remove("_macro_compilation_error");
        }
        Err(error) => {
            // Saving an incomplete legacy local draft remains possible. The
            // native publisher must reject this snapshot before any write.
            document.as_object_mut().unwrap().remove("macros");
            document.as_object_mut().unwrap().remove("phasedMacros");
            document["_macro_compilation_error"] = format!("{error:#}").into();
        }
    }
    device
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}))[key] = document;
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
            || !matches!(workspace.read(cx).device(cx).product_id, 70 | 182)
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
            if self.storage_error.is_some() {
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !self.devices.contains(&workspace)
            || !workspace.read(cx).legacy_dpi_request_matches(&request, cx)
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_legacy_dpi(&request, cx)
            });
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
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        let revision = self.discovery_revision;
        self.device_value_owners.remove(&identity);
        self.device_read_scopes.remove(&identity);
        self.status = format!("{context}，正在等待设备设置应答。");
        self.legacy_dpi_workers += 1;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let selected = route.clone();
            let guard = request.clone();
            let result = cx
                .background_spawn(async move {
                    anyhow::ensure!(
                        guard.is_valid(),
                        "DPI request cancelled before service creation"
                    );
                    let mut client = ServiceClient::spawn()?;
                    let result = (|| {
                        anyhow::ensure!(
                            guard.is_valid(),
                            "DPI request cancelled before route resolution"
                        );
                        let cap = razer_device::mouse_legacy_polling::read_capability();
                        let direct = direct::resolve(&mut client, &selected, cap)?;
                        anyhow::ensure!(
                            guard.is_valid(),
                            "DPI request cancelled before device submission"
                        );
                        let ack = direct.submit_legacy_dpi(&mut client, x, y)?;
                        anyhow::ensure!(
                            guard.is_valid(),
                            "DPI request cancelled after device submission"
                        );
                        Ok::<_, anyhow::Error>(ack)
                    })();
                    let shutdown_error = client
                        .request(ServiceRequest::Shutdown)
                        .err()
                        .map(|e| format!("{e:#}"));
                    result.map(|ack| (ack, shutdown_error))
                })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.legacy_dpi_workers = this.legacy_dpi_workers.saturating_sub(1);
                let current = this.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == identity
                    && workspace.read(cx).legacy_dpi_request_matches(&request, cx)
                    && this.discovery_revision == revision
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
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_legacy_dpi(&request, cx)
                });
                this.device_value_owners.remove(&identity);
                this.device_read_scopes.remove(&identity);
                this.status = if !current {
                    format!("{context}；配置或连接已变化，旧设备应答已丢弃。")
                } else {
                    match result {
                        Ok((_, None)) => {
                            format!("{context}，设备已应答 X {x} / Y {y} 设置。")
                        }
                        Ok((_, Some(error))) => {
                            format!("设备已应答 DPI 设置；通信进程退出失败：{error}")
                        }
                        Err(error) => format!("{context}，设备设置未取得应答：{error:#}"),
                    }
                };
                if this.close_requested && this.save_task.is_none() {
                    this.continue_save_queue(true, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }
}

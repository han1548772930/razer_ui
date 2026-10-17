use super::*;
use razer_discovery::direct;
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_model::model::Profile;
use razer_pages::features::mouse_products::MouseCurrentDpiRequested;

pub(super) struct MouseCurrentDpiSave {
    workspace: Entity<ProductWorkspace>,
    request: MouseCurrentDpiRequested,
    identity: String,
    profile: Profile,
}

impl AppShell {
    /// Source90012 startup/restore owns initialization; GET observations never
    /// replace the local profile. A retained connection/profile key prevents
    /// repeated reads or render notifications from replaying initialization.
    pub(super) fn initialize_mouse182_profiles(&mut self, cx: &mut Context<Self>) {
        if self.close_requested || self.storage_error.is_some() {
            return;
        }
        let macro_recording = self.receiver_macro_exclusive(cx);
        for workspace in &self.devices {
            let device = workspace.read(cx).device(cx);
            if device.product_id != 182 {
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
                workspace.initialize_mouse182_profile(key, macro_recording, cx)
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
        self.pending_saves.push_back(PreparedSave {
            window: window.window_handle(),
            file,
            product: Some(ProductSave::MouseCurrentDpi(MouseCurrentDpiSave {
                workspace,
                request,
                identity,
                profile,
            })),
        });
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
                    let next_version = if saved.product_id == 182 {
                        let root = saved
                            .source_device_settings
                            .get_or_insert_with(|| serde_json::json!({}));
                        anyhow::ensure!(
                            root.is_object(),
                            "PID182 local device settings is not an object"
                        );
                        let previous = root
                            .get("version")
                            .map(|value| {
                                value.as_u64().ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "PID182 local version is not an unsigned integer"
                                    )
                                })
                            })
                            .transpose()?;
                        Some(razer_device::mapping_submission::next_source_version(
                            previous,
                        )?)
                    } else {
                        None
                    };
                    if let Some(profile) = saved
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.id == dpi.profile.id)
                    {
                        let settings = profile
                            .source_settings
                            .get_or_insert_with(|| serde_json::json!({}));
                        anyhow::ensure!(settings.is_object(), "DPI local profile is not an object");
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
                    } else {
                        anyhow::bail!("DPI profile was removed before persistence");
                    }
                    if let Some(version) = next_version {
                        saved
                            .source_device_settings
                            .as_mut()
                            .expect("validated root settings")["version"] =
                            serde_json::json!(version);
                    }
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
            self.status = "DPI 本地配置保留；尚无唯一的当前鼠标或接收器连接，设备未写入。".into();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_current_dpi(&request, None, cx)
            });
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        let revision = self.discovery_revision;
        self.current_dpi_workers += 1;
        self.status = "正在核对 DPI 设备应答与读回结果。".into();
        cx.notify();
        let workspace = workspace.downgrade();
        cx.spawn_in(window, async move |this, cx| {
            let selected = route.clone();
            let guard = request.clone();
            let result = cx
                .background_spawn(async move {
                    anyhow::ensure!(
                        guard.is_valid(),
                        "Current mouse DPI scope canceled before service creation"
                    );
                    let mut client = ServiceClient::spawn()?;
                    client.set_request_guard(guard.guard());
                    let result = (|| {
                        let report = direct::mouse_current_dpi::request(
                            &mut client,
                            &selected,
                            guard.change().cloned(),
                        )?;
                        anyhow::ensure!(
                            guard.is_valid(),
                            "Current mouse DPI scope canceled after submission"
                        );
                        Ok::<_, anyhow::Error>(report)
                    })();
                    let shutdown_error = if client.is_stopped() {
                        None
                    } else {
                        client
                            .request(ServiceRequest::Shutdown)
                            .err()
                            .map(|error| format!("{error:#}"))
                    };
                    result.map(|report| (report, shutdown_error))
                })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.current_dpi_workers = this.current_dpi_workers.saturating_sub(1);
                let Some(workspace) = workspace.upgrade() else {
                    if this.close_requested && this.save_task.is_none() {
                        this.continue_save_queue(true, cx);
                    }
                    return;
                };
                let current = this.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == identity
                    && workspace.read(cx).current_dpi_request_matches(&request, cx)
                    && this.discovery_revision == revision
                    && this.device_observations.iter().any(|current| {
                        current.matches(workspace.read(cx).device(cx))
                            && current.physical_product_id() == route.physical_product_id()
                            && current.container().eq_ignore_ascii_case(route.container())
                            && current.hid_node() == route.hid_node()
                            && current.peer_product_id() == route.peer_product_id()
                            && current.connection() == route.connection()
                            && direct::mouse_current_dpi::available(current)
                    });
                let report = current
                    .then(|| result.as_ref().ok().map(|(report, _)| report.clone()))
                    .flatten();
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_current_dpi(&request, report, cx)
                });
                if current {
                    this.status = match result {
                        Ok((_, None)) => {
                            if request
                                .change()
                                .is_some_and(|change| change.is_identity_read())
                            {
                                "已读取设备序列号，正在载入对应配置。".into()
                            } else if request.is_read() {
                                "已读取当前 DPI。".into()
                            } else {
                                "DPI 已保存到本机，设备写入已应答。".into()
                            }
                        }
                        Ok((_, Some(error))) => {
                            format!("DPI 设备操作已确认；通信进程退出失败：{error}")
                        }
                        Err(error) => format!("DPI 设备状态尚未确认：{error:#}；本地配置保留。"),
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

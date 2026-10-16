use super::*;
use razer_discovery::{direct, discovery::ObservedTransport};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::mouse_products::MouseCurrentPowerRequested;

pub(super) struct MouseCurrentPowerSave {
    workspace: Entity<ProductWorkspace>,
    request: MouseCurrentPowerRequested,
    identity: String,
}

impl AppShell {
    pub(super) fn persist_current_power(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCurrentPowerRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !request.requires_local_persistence(workspace.read(cx).device(cx).product_id) {
            self.submit_current_power_device(workspace, request, window, cx);
            return;
        }
        if self.storage_error.is_some()
            || !workspace
                .read(cx)
                .current_power_request_matches(&request, cx)
        {
            if let Some(error) = &self.storage_error {
                self.status = format!("电源设置本地保存失败：{error}");
            }
            workspace.update(cx, |workspace, cx| {
                workspace.finish_current_power(&request, None, cx)
            });
            cx.notify();
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let file = store::WorkspaceFile::new(
            self.devices
                .iter()
                .map(|entity| entity.read(cx).saved_snapshot(cx))
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
            controller_trigger: None,
            mouse_current_power: Some(MouseCurrentPowerSave {
                workspace,
                request,
                identity,
            }),
        });
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_current_power_save(&self, save: &mut PreparedSave, cx: &App) {
        let Some(power) = &save.mouse_current_power else {
            return;
        };
        // Basic settings are device-owned. A profile switch must not move them
        // into the newly selected profile or overwrite unrelated saved fields.
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let mut saved = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == power.identity {
                    let settings = saved
                        .source_device_settings
                        .get_or_insert_with(|| serde_json::json!({}));
                    for &key in
                        razer_pages::features::mouse_products::power_device_fields(saved.product_id)
                    {
                        let requested_key = power.request.change().map(|change| change.key());
                        if requested_key.is_some_and(|requested| {
                            requested != key
                                && !(requested == "powerSavingValue" && key == "PowerSavingValue")
                        }) {
                            continue;
                        }
                        if let Some(value) = power.request.settings().get(key) {
                            settings[key] = value.clone();
                        }
                    }
                    razer_pages::features::mouse_products::filter_power_persistence(
                        saved.product_id,
                        settings,
                    );
                }
                saved
            })
            .collect();
    }

    pub(super) fn finish_current_power_save(
        &mut self,
        save: MouseCurrentPowerSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !succeeded || save.request.change().is_none() || self.close_requested {
            save.workspace.update(cx, |workspace, cx| {
                workspace.finish_current_power(&save.request, None, cx)
            });
            return;
        }
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let workspace = save.workspace.clone();
            let request = save.request.clone();
            if window
                .update(cx, |_, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.submit_current_power_device(save.workspace, save.request, window, cx)
                    });
                })
                .is_err()
            {
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_current_power(&request, None, cx)
                });
            }
        });
    }

    fn submit_current_power_device(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: MouseCurrentPowerRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || request.change().is_none()
            || !workspace
                .read(cx)
                .current_power_request_matches(&request, cx)
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_current_power(&request, None, cx)
            });
            return;
        }
        let routes: Vec<_> = self
            .device_observations
            .iter()
            .filter(|route| {
                route.matches(workspace.read(cx).device(cx))
                    && matches!(
                        route.transport(),
                        Some(
                            ObservedTransport::Wired
                                | ObservedTransport::Dongle
                                | ObservedTransport::Ble
                        )
                    )
                    && razer_device::mouse_current_power::capability(route.product_id())
                        .is_some_and(|source| {
                            source.accepts(route.physical_product_id(), route.peer_product_id())
                        })
            })
            .collect();
        if routes.len() != 1 {
            self.status = if request.is_read() {
                "当前连接尚不唯一，设备省电设置未读取。".into()
            } else if !request.requires_local_persistence(workspace.read(cx).device(cx).product_id)
            {
                "当前连接尚不唯一，设备省电设置未写入。".into()
            } else {
                "电源设置已保存到本机；当前连接尚不唯一，设备未写入。".into()
            };
            workspace.update(cx, |workspace, cx| {
                workspace.finish_current_power(&request, None, cx)
            });
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        let revision = self.discovery_revision;
        self.current_power_workers += 1;
        self.status = "正在核对电源设置设备应答…".into();
        cx.notify();
        // Completion does not need the original window. It must still release
        // the request and drain the local writer if that window closes.
        let _ = window;
        cx.spawn(async move |this, cx| {
            let selected = route.clone();
            let guard = request.clone();
            let result = cx
                .background_spawn(async move {
                    anyhow::ensure!(
                        guard.is_valid(),
                        "Mouse power scope canceled before service creation"
                    );
                    let mut client = ServiceClient::spawn()?;
                    client.set_request_guard(guard.guard());
                    let result = (|| {
                        let direct = direct::mouse_current_power::resolve(&mut client, &selected)?;
                        anyhow::ensure!(
                            guard.is_valid(),
                            "Mouse power scope canceled before device submission"
                        );
                        let report = direct.mouse_current_power(
                            &mut client,
                            guard.change().cloned().ok_or_else(|| {
                                anyhow::anyhow!("Power write request has no change")
                            })?,
                        )?;
                        anyhow::ensure!(
                            guard.is_valid(),
                            "Mouse power scope canceled after device submission"
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
            let _ = this.update(cx, |this, cx| {
                this.current_power_workers = this.current_power_workers.saturating_sub(1);
                let current = this.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == identity
                    && workspace
                        .read(cx)
                        .current_power_request_matches(&request, cx)
                    && this.discovery_revision == revision
                    && this.device_observations.iter().any(|current| {
                        current.matches(workspace.read(cx).device(cx))
                            && current.physical_product_id() == route.physical_product_id()
                            && current.container().eq_ignore_ascii_case(route.container())
                            && current.hid_node() == route.hid_node()
                            && current.peer_product_id() == route.peer_product_id()
                    });
                let observed = current
                    .then(|| result.as_ref().ok().map(|(report, _)| report.clone()))
                    .flatten();
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_current_power(&request, observed, cx)
                });
                if current {
                    this.status = match result {
                        Ok((_, None)) => {
                            if request.is_read() {
                                "已读取设备省电设置。".into()
                            } else if !request.requires_local_persistence(
                                workspace.read(cx).device(cx).product_id,
                            ) {
                                "设备省电设置已确认。".into()
                            } else {
                                "电源设置已保存到本机，设备设置已确认。".into()
                            }
                        }
                        Ok((_, Some(error))) => {
                            format!("电源设置设备操作已确认；通信进程退出失败：{error}")
                        }
                        Err(error) => {
                            if request.is_read() {
                                format!("设备省电设置读取失败：{error:#}")
                            } else if !request.requires_local_persistence(
                                workspace.read(cx).device(cx).product_id,
                            ) {
                                format!("设备省电设置写入尚未确认：{error:#}；当前编辑值保留。")
                            } else {
                                format!("电源设置设备写入尚未确认：{error:#}；本地设置保留。")
                            }
                        }
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

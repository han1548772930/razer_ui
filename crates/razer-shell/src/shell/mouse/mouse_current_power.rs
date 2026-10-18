use super::*;
use razer_pages::features::mouse_products::MouseCurrentPowerRequested;

pub(super) struct MouseCurrentPowerSave {
    workspace: Entity<ProductWorkspace>,
    request: MouseCurrentPowerRequested,
    identity: String,
}
impl MouseCurrentPowerSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
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
            self.submit_current_power_device(workspace, request, cx);
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
            if self.storage_error.is_some() {
                self.cancel_pending_exit(cx);
            }
            cx.notify();
            return;
        }
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
            Some(ProductSave::MouseCurrentPower(MouseCurrentPowerSave {
                workspace,
                request,
                identity,
            })),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(super) fn rebase_current_power_save(&self, save: &mut PreparedSave, cx: &App) {
        let Some(ProductSave::MouseCurrentPower(power)) = &save.product else {
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
        _window: AnyWindowHandle,
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
            if owner
                .update(cx, |this, cx| {
                    this.submit_current_power_device(save.workspace, save.request, cx)
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
        cx: &mut Context<Self>,
    ) {
        if self.close_requested
            || !self.devices.contains(&workspace)
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
        let operation = mouse_current_power_runtime::Request::Current(request.clone());
        let Some(route) = mouse_current_power_runtime::select_route(
            &self.device_observations,
            workspace.read(cx).device(cx),
            &operation,
        ) else {
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
        };
        let identity = workspace.read(cx).identity(cx);
        self.device_value_owners.remove(&identity);
        self.device_read_scopes.remove(&identity);
        self.status = "正在核对电源设置设备应答…".into();
        self.start_mouse_power_operation(workspace, route, operation, cx);
        cx.notify();
    }
}

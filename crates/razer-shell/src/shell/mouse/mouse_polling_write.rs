//! Current PID70/182 polling intents -> shared direct route -> readback.
use super::*;
use razer_discovery::{direct, discovery};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_pages::features::mouse_polling::{MousePollingScope, PollingField};

impl AppShell {
    pub(super) fn write_mouse_polling(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: MousePollingScope,
        field: PollingField,
        hz: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(workspace.read(cx).device(cx).product_id, 70 | 182) {
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
                route.matches(device)
                    && match field {
                        PollingField::Wired => {
                            route.peer_product_id().is_none()
                                && matches!(
                                    route.transport(),
                                    Some(discovery::ObservedTransport::Wired)
                                )
                        }
                        PollingField::Wireless => direct::mouse_receiver_polling::available(route),
                    }
            })
            .collect::<Vec<_>>();
        if routes.len() != 1 {
            self.status = "回报率本地配置已保存；没有唯一匹配的当前设备连接，设备未写入。".into();
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_polling(scope, field, hz, None, cx)
            });
            cx.notify();
            return;
        }
        let route = routes[0].clone();
        let identity = workspace.read(cx).identity(cx);
        let product_id = device.product_id;
        let revision = self.discovery_revision;
        self.device_value_owners.remove(&identity);
        self.device_read_scopes.remove(&identity);
        self.status = "正在设置回报率并读取设备确认。".into();
        cx.notify();
        let workspace = workspace.downgrade();
        cx.spawn_in(window, async move |this, cx| {
            let selected = route.clone();
            let result = cx
                .background_spawn(async move {
                    use anyhow::Context as _;
                    use razer_device::{
                        device_reads::DeviceReadValue, device_writes::DeviceWriteSetting,
                    };
                    anyhow::ensure!(
                        guard.load(std::sync::atomic::Ordering::Acquire),
                        "Polling scope canceled before service creation"
                    );
                    let mut client = ServiceClient::spawn()?;
                    client.set_request_guard(guard.clone());
                    let result = (|| {
                        let observed = if field == PollingField::Wireless {
                            direct::mouse_receiver_polling::write(&mut client, &selected, hz)?
                                .observed_hz
                        } else {
                            let cap = razer_device::device_reads::capability(product_id)
                                .context("产品回报率读取能力尚未核实")?;
                            match direct::resolve(&mut client, &selected, cap)?
                                .write(&mut client, DeviceWriteSetting::Polling { hz })?
                            {
                                DeviceReadValue::Polling { hz: observed } if observed == hz => {
                                    observed
                                }
                                _ => anyhow::bail!("回报率读回与请求不一致"),
                            }
                        };
                        anyhow::ensure!(
                            guard.load(std::sync::atomic::Ordering::Acquire),
                            "Polling scope canceled after submission"
                        );
                        Ok(observed)
                    })();
                    let shutdown_error = if client.is_stopped() {
                        None
                    } else {
                        client
                            .request(ServiceRequest::Shutdown)
                            .err()
                            .map(|error| format!("{error:#}"))
                    };
                    result.map(|observed| (observed, shutdown_error))
                })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                let Some(workspace) = workspace.upgrade() else {
                    return;
                };
                if !workspace
                    .read(cx)
                    .mouse_polling_in_flight_matches(scope, field, hz, cx)
                {
                    return;
                }
                if !this.devices.contains(&workspace)
                    || workspace.read(cx).identity(cx) != identity
                    || !workspace
                        .read(cx)
                        .mouse_polling_request_matches(scope, field, hz, cx)
                {
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_mouse_polling(scope, field, hz, None, cx)
                    });
                    return;
                }
                let current = this.discovery_revision == revision
                    && this.device_observations.iter().any(|current| {
                        current.matches(workspace.read(cx).device(cx))
                            && current.container().eq_ignore_ascii_case(route.container())
                            && current.physical_product_id() == route.physical_product_id()
                            && current.hid_node() == route.hid_node()
                            && current.peer_product_id() == route.peer_product_id()
                            && current.connection() == route.connection()
                            && current.transport() == route.transport()
                    });
                let observed = current
                    .then(|| result.as_ref().ok().map(|(hz, _)| *hz))
                    .flatten();
                this.device_value_owners.remove(&identity);
                this.device_read_scopes.remove(&identity);
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_mouse_polling(scope, field, hz, observed, cx)
                });
                this.status = if !current {
                    "设备连接已变化，回报率尚未确认。".into()
                } else {
                    match result {
                        Ok((observed, None)) => {
                            format!("设备已回读确认回报率：{observed} Hz。")
                        }
                        Ok((observed, Some(error))) => {
                            format!("设备已确认回报率：{observed} Hz；通信进程退出失败：{error}")
                        }
                        Err(error) => format!("回报率未确认：{error:#}。请重新读取设备状态。"),
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn rebase_mouse_polling_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::MousePolling(polling)) = &save.product else {
            return Ok(());
        };
        save.file.devices = self
            .devices
            .iter()
            .map(|entity| {
                let mut saved = entity.read(cx).saved_snapshot(cx);
                if entity.read(cx).identity(cx) == polling.identity {
                    let profile = saved
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.id == polling.profile.id)
                        .ok_or_else(|| {
                            anyhow::anyhow!("Polling profile was removed before persistence")
                        })?;
                    let settings = profile
                        .source_settings
                        .get_or_insert_with(|| serde_json::json!({}));
                    anyhow::ensure!(
                        settings.is_object(),
                        "Polling local profile is not an object"
                    );
                    let key = if settings.get("PollingRate").is_some() {
                        "PollingRate"
                    } else {
                        polling.field.key()
                    };
                    settings[key] = serde_json::json!(polling.hz);
                    if saved.product_id == 182 {
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
                        if let Some(typed) = &mut profile.settings {
                            match polling.field {
                                PollingField::Wired => typed.polling_wired = Some(polling.hz),
                                PollingField::Wireless => typed.polling_wireless = Some(polling.hz),
                            }
                        }
                        let root = saved
                            .source_device_settings
                            .get_or_insert_with(|| serde_json::json!({}));
                        anyhow::ensure!(
                            root.is_object(),
                            "Polling device settings is not an object"
                        );
                        let previous = root
                            .get("version")
                            .map(|value| {
                                value.as_u64().ok_or_else(|| {
                                    anyhow::anyhow!(
                                        "Polling local version is not an unsigned integer"
                                    )
                                })
                            })
                            .transpose()?;
                        root["version"] = serde_json::json!(
                            razer_device::mapping_submission::next_source_version(previous)?
                        );
                    }
                }
                Ok(saved)
            })
            .collect::<anyhow::Result<_>>()?;
        Ok(())
    }
}

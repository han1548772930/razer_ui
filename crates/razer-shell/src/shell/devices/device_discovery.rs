//! Publish current connection/parameter observations into retained product owners.
use super::*;
use anyhow::Context as _;
use razer_discovery::discovery;
use razer_discovery::receiver::ReceiverRoute;
use razer_ipc::ServiceClient;
use razer_ipc::ServiceRequest;
use razer_pages::features::ReceiverOperation;
use razer_pages::features::ReceiverPairingEvent;
use razer_pages::features::ReceiverPairingIntent;
use razer_pages::features::ReceiverPairingObservation;
use razer_pages::features::ReceiverPeer;
use razer_pages::features::mouse_polling::MousePollingObservation;
use razer_pages::features::mouse_polling::PollingConnection;
use razer_pages::features::mouse_polling::PollingField;

#[cfg(test)]
#[path = "device_discovery_tests.rs"]
mod tests;

#[derive(Clone, Copy)]
pub(super) struct DeviceReadScope {
    polling: razer_pages::features::mouse_polling::MousePollingScope,
    idle: Option<razer_pages::features::mouse_products::MousePowerSavingRead>,
}

fn receiver_scope_matches(observed: &discovery::ObservedDevice, container: &str, pid: u32) -> bool {
    observed.container().eq_ignore_ascii_case(container)
        && observed.physical_product_id() == pid
        && observed.peer_product_id().is_some()
}

fn receiver_owner_matches(device: &Device, container: &str, pid: u32) -> bool {
    device.real_product_id == pid
        && device.device_container_id.eq_ignore_ascii_case(container)
        && ((container.len() == 38
            && container.starts_with('{')
            && container.ends_with('}')
            && uuid::Uuid::parse_str(container).is_ok_and(|id| !id.is_nil()))
            // Only a scheduling hint. The exact retained node and fresh
            // enumeration must pass receiver_route_for_owner before I/O.
            || container.starts_with(&format!("hid-collection:1532:{pid:04x}:")))
        && !device
            .serial_number
            .to_ascii_uppercase()
            .starts_with("PREVIEW-")
        && !device
            .serial_number
            .to_ascii_uppercase()
            .starts_with("DEMO-")
        && matches!(
            device.dashboard.connection_observation,
            Some(
                razer_model::model::DeviceConnectionObservation::UsbPresent
                    | razer_model::model::DeviceConnectionObservation::HidPresent
            )
        )
}

pub(super) fn receiver_route_for_owner(
    observations: &[discovery::ObservedDevice],
    device: &Device,
    container: &str,
    pid: u32,
) -> anyhow::Result<ReceiverRoute> {
    anyhow::ensure!(
        receiver_owner_matches(device, container, pid),
        "接收器物理连接未确认"
    );
    let owners = observations
        .iter()
        .filter(|observed| {
            observed.matches(device)
                && observed.physical_product_id() == pid
                && observed.peer_product_id().is_none()
        })
        .collect::<Vec<_>>();
    let [owner] = owners.as_slice() else {
        anyhow::bail!("接收器缺少唯一的当前接口观察");
    };
    ReceiverRoute::from_observation(owner)
}

pub(super) fn dock_route_for_workspace(
    observations: &[discovery::ObservedDevice],
    device: &Device,
    records: Option<&serde_json::Value>,
) -> anyhow::Result<ReceiverRoute> {
    if device.product_id != 756 {
        return receiver_route_for_owner(
            observations,
            device,
            &device.device_container_id,
            device.real_product_id,
        );
    }
    anyhow::ensure!(
        !device
            .serial_number
            .to_ascii_uppercase()
            .starts_with("PREVIEW-")
            && !device
                .serial_number
                .to_ascii_uppercase()
                .starts_with("DEMO-"),
        "DualLink 接口尚未观察"
    );
    let candidates = observations
        .iter()
        .filter(|observed| observed.matches(device))
        .collect::<Vec<_>>();
    let [observed] = candidates.as_slice() else {
        anyhow::bail!("DualLink owner 缺少唯一当前观察");
    };
    let route = ReceiverRoute::from_observation(observed)
        .or_else(|_| ReceiverRoute::from_connected_peer(observed))?;
    let physical = route.product_id();
    if !matches!(physical, 756 | 757) {
        let records = records.context("DualLink slave 缺少源 factory 主设备缓存")?;
        let rows: Vec<_> = match records {
            serde_json::Value::Object(rows) => rows.values().collect(),
            serde_json::Value::Array(rows) => rows.iter().collect(),
            _ => Vec::new(),
        };
        anyhow::ensure!(
            rows.iter()
                .any(|row| row["dongleId"] == device.real_product_id
                    && row["master"]["dongleId"] == physical
                    && row["deviceContainerId"]
                        .as_str()
                        .is_some_and(|id| id.eq_ignore_ascii_case(&device.device_container_id))),
            "当前物理 owner 与源 DualLink slave factory 不匹配"
        );
    }
    razer_device::receiver_capabilities::for_source(u16::try_from(physical)?, Some(756))?;
    Ok(route)
}

fn replace_receiver_observations(
    observations: &mut Vec<discovery::ObservedDevice>,
    container: &str,
    pid: u32,
    incoming: &[discovery::ObservedDevice],
) -> Vec<discovery::ObservedDevice> {
    let removed = observations
        .iter()
        .filter(|observed| receiver_scope_matches(observed, container, pid))
        .cloned()
        .collect();
    observations.retain(|observed| !receiver_scope_matches(observed, container, pid));
    observations.extend_from_slice(incoming);
    removed
}

fn same_receiver_peer(a: &discovery::ObservedDevice, b: &discovery::ObservedDevice) -> bool {
    a.product_id() == b.product_id()
        && a.container().eq_ignore_ascii_case(b.container())
        && a.physical_product_id() == b.physical_product_id()
        && a.peer_product_id() == b.peer_product_id()
        && a.hid_node() == b.hid_node()
        && a.connection() == b.connection()
}

pub(super) fn same_connection(
    a: &discovery::ObservedDevice,
    b: &discovery::ObservedDevice,
) -> bool {
    same_receiver_peer(a, b) && a.transport() == b.transport()
}

pub(super) fn retained_connection(
    previous: &[discovery::ObservedDevice],
    current: &[discovery::ObservedDevice],
    device: &Device,
) -> bool {
    let mut previous = previous.iter().filter(|observed| observed.matches(device));
    let mut current = current.iter().filter(|observed| observed.matches(device));
    let (Some(old), Some(new)) = (previous.next(), current.next()) else {
        return false;
    };
    previous.next().is_none() && current.next().is_none() && same_connection(old, new)
}

fn observe_polling_connection(
    workspace: &Entity<ProductWorkspace>,
    transport: Option<discovery::ObservedTransport>,
    cx: &mut App,
) {
    let Some(scope) = workspace.read(cx).mouse_polling_scope(cx) else {
        return;
    };
    let connection = transport.map(|transport| match transport {
        discovery::ObservedTransport::Wired => PollingConnection::Wired,
        discovery::ObservedTransport::Dongle => PollingConnection::Dongle,
        discovery::ObservedTransport::Ble => PollingConnection::Ble,
    });
    workspace.update(cx, |workspace, cx| {
        workspace.observe_mouse_polling(scope, MousePollingObservation::Connection(connection), cx)
    });
}

impl AppShell {
    /// Current 182 MM/CM release -> ON_SET_POWER_SAVING_VALUE -> timeToSleep.
    /// The worker owns direct HID route validation, serialization and readback.
    pub(super) fn write_mouse_idle(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: razer_pages::features::mouse_polling::MousePollingScope,
        minutes: u8,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 83789/w grants only updateLocalStorage while the source macro
        // recorder is exclusive. PID182 pn also skips that localStorage write.
        self.refresh_macro_recorder_storage();
        if self.receiver_macro_exclusive(cx) {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_idle(scope, minutes, None, cx)
            });
            return;
        }
        if self.close_requested
            || !workspace
                .read(cx)
                .mouse_idle_request_matches(scope, minutes, cx)
        {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_idle(scope, minutes, None, cx)
            });
            return;
        }
        let Some(guard) = workspace
            .read(cx)
            .mouse_idle_write_guard(scope, minutes, cx)
        else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_idle(scope, minutes, None, cx)
            });
            return;
        };
        let request = mouse_current_power_runtime::Request::Idle {
            scope,
            minutes,
            valid: guard,
        };
        let Some(route) = mouse_current_power_runtime::select_route(
            &self.device_observations,
            workspace.read(cx).device(cx),
            &request,
        ) else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_idle(scope, minutes, None, cx)
            });
            self.status =
                "休眠设置未发送：尚无唯一的设备连接或在线接收器关联。当前选择已保留。".into();
            cx.notify();
            return;
        };
        let identity = workspace.read(cx).identity(cx);
        // An older discovery read must not replace this mutation's readback.
        self.device_value_owners.remove(&identity);
        self.device_read_scopes.remove(&identity);
        self.status = "正在写入休眠时间并读取设备确认。".into();
        self.start_mouse_power_operation(workspace, route, request, cx);
        cx.notify();
    }

    pub(super) fn query_dock_pairing(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        event: &razer_pages::features::DockPairingEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let identity = workspace.read(cx).identity(cx);
        let kind = event.kind().to_owned();
        if kind == "DUALLINK_CANCEL" {
            self.receiver_queries.retain(|key, _| {
                key != &identity && !key.starts_with(&format!("{identity}\u{1f}"))
            });
            let active = self.receiver_pairing_operations.contains_key(&identity);
            self.retire_receiver_pairing(&identity);
            // Active Session cleanup sends scan(4); an idle source cancellation
            // still needs the actual command, rather than a retired worker only.
            if !active {
                self.submit_dock_pairing(workspace, event, window, cx);
            }
            return;
        }
        if !matches!(
            kind.as_str(),
            "DUALLINK_BIND_INFO" | "DUALLINK_CONNECT_TYPE" | "DUALLINK_DEVICE_INFO"
        ) {
            self.submit_dock_pairing(workspace, event, window, cx);
            return;
        }
        if kind == "DUALLINK_BIND_INFO" {
            self.retire_receiver_pairing(&identity);
        }
        let device = workspace.read(cx).device(cx);
        let container = device.device_container_id.clone();
        let source_product = device.product_id;
        let source_physical = device.real_product_id;
        let source = (source_product == 756).then_some(756);
        let records = self
            .local_pairing_records
            .clone()
            .unwrap_or(serde_json::Value::Null);
        let route = dock_route_for_workspace(&self.device_observations, device, Some(&records));
        let retained_route = route.as_ref().ok().cloned();
        let product_id = retained_route
            .as_ref()
            .map_or(source_physical, ReceiverRoute::product_id);
        let session = event.session();
        let key = format!("{identity}\u{1f}{kind}");
        let revision = self.discovery_revision;
        let generation = self.receiver_queries.entry(key.clone()).or_default();
        *generation = (session, generation.1.wrapping_add(1));
        let generation = *generation;
        let runtime_layouts = self.receiver_runtime_layouts(cx);
        cx.spawn_in(window, async move |this, cx| {
            let worker_kind = kind.clone();
            let worker_container = container.clone();
            let result = cx
                .background_spawn(async move {
                    let mut client = ServiceClient::spawn()?;
                    let mut diagnostics = Vec::new();
                    // Keep the parent164/241 mode-probe semantics independent from
                    // source presentation and from binding-query failures.
                    let mode = matches!(source_product, 164 | 241).then(|| {
                        route
                            .as_ref()
                            .map_err(|error| anyhow::anyhow!("{error:#}"))?
                            .dock_mode(&mut client)
                            .map_err(|error| anyhow::anyhow!("{error:#}"))
                    });
                    let cached_slave = match &records {
                        serde_json::Value::Object(rows) => {
                            rows.values().any(|row| row["productId"] == source_product)
                        }
                        serde_json::Value::Array(rows) => {
                            rows.iter().any(|row| row["productId"] == source_product)
                        }
                        _ => false,
                    };
                    let projection = if worker_kind == "DUALLINK_CONNECT_TYPE"
                        || worker_kind == "DUALLINK_DEVICE_INFO" && cached_slave
                    {
                        None
                    } else {
                        Some(
                            route
                                .as_ref()
                                .map_err(|error| anyhow::anyhow!("{error:#}"))
                                .and_then(|route| route.query_for_source(&mut client, source)),
                        )
                    };
                    let physical = client.request(ServiceRequest::HidDevices);
                    let connected = match razer_storage::source_local::get(
                        &razer_storage::source_local::path(),
                        "connectedDeviceInfo",
                    ) {
                        Ok(Some(serde_json::Value::Array(rows))) => rows,
                        Ok(None) => Vec::new(),
                        Ok(Some(_)) => {
                            diagnostics.push("connectedDeviceInfo 不是源数组".into());
                            Vec::new()
                        }
                        Err(error) => {
                            diagnostics.push(format!("connectedDeviceInfo: {error:#}"));
                            Vec::new()
                        }
                    };
                    let bindings = projection
                        .as_ref()
                        .and_then(|result| result.as_ref().ok())
                        .map(|projection| {
                            let physical = physical
                                .as_ref()
                                .map_err(|error| anyhow::anyhow!("{error:#}"))?;
                            razer_discovery::receiver_pairing::project_binding_info_with_runtime(
                                source_product,
                                &worker_container,
                                projection.query_response(),
                                &connected,
                                physical,
                                &runtime_layouts,
                            )
                        });
                    let payload = match worker_kind.as_str() {
                        "DUALLINK_CONNECT_TYPE" => (|| {
                            anyhow::ensure!(source_product == 756, "当前连接类型查询没有源 caller");
                            let physical = physical
                                .as_ref()
                                .map_err(|error| anyhow::anyhow!("{error:#}"))?;
                            anyhow::ensure!(physical["complete"] == true, "物理接口枚举未完成");
                            let present = physical["interfaces"]
                                .as_array()
                                .context("物理接口列表缺失")?
                                .iter()
                                .any(|row| {
                                    row["product_id"] == 757
                                        && row["device_container_id"].as_str().is_some_and(|id| {
                                            id.eq_ignore_ascii_case(&worker_container)
                                        })
                                });
                            Ok(serde_json::json!(if source_physical == 757 {
                                if present { 2 } else { 3 }
                            } else {
                                1
                            }))
                        })(),
                        "DUALLINK_DEVICE_INFO" => {
                            if let Some(Err(error)) = &projection {
                                diagnostics.push(format!("[ge queryBindInfo] {error:#}"));
                            }
                            if let Some(Err(error)) = &bindings {
                                diagnostics.push(format!("[ge binding presentation] {error:#}"));
                            }
                            razer_discovery::receiver_pairing::project_device_info(
                                source_product,
                                &records,
                                bindings.as_ref().and_then(|result| result.as_ref().ok()),
                            )
                        }
                        _ => match bindings {
                            Some(result) => result,
                            None => Err(anyhow::anyhow!(
                                "{}",
                                projection
                                    .as_ref()
                                    .and_then(|p| p.as_ref().err())
                                    .map(|e| format!("{e:#}"))
                                    .unwrap_or_else(|| "绑定查询未提供当前观察".into())
                            )),
                        },
                    };
                    if let Err(error) = client.request(ServiceRequest::Shutdown) {
                        diagnostics.push(format!("[dock query cleanup] {error:#}"));
                    }
                    Ok::<_, anyhow::Error>((projection, mode, payload, diagnostics))
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.receiver_queries.get(&key) != Some(&generation)
                    || this.discovery_revision != revision
                    || !this.devices.contains(&workspace)
                    || workspace.read(cx).identity(cx) != identity
                    || workspace.read(cx).device(cx).real_product_id != source_physical
                    || retained_route.as_ref().is_some_and(|route| {
                        !this.device_observations.iter().any(|observed| {
                            observed.matches(workspace.read(cx).device(cx))
                                && route.matches(observed)
                        })
                    })
                {
                    return;
                }
                this.receiver_queries.remove(&key);
                let (projection, mode, payload) = match result {
                    Ok((projection, mode, payload, diagnostics)) => {
                        for diagnostic in diagnostics {
                            log::warn!("[dock source query] {diagnostic}");
                        }
                        (projection, mode, payload.map_err(|e| format!("{e:#}")))
                    }
                    Err(error) => (None, None, Err(format!("{error:#}"))),
                };
                if let Some(mode) = mode {
                    if let Err(error) = &mode {
                        log::warn!("[dock factory mode] {error:#}");
                    }
                    workspace.update(cx, |workspace, cx| {
                        workspace.observe_pairing_mode(Some(mode.unwrap_or(false)), cx)
                    });
                    this.sync_known_devices(cx);
                }
                let accepted = workspace.update(cx, |workspace, cx| {
                    workspace.observe_dock_pairing(
                        razer_pages::features::DockPairingObservation::result(
                            session, &kind, payload,
                        ),
                        cx,
                    )
                });
                if accepted && kind == "DUALLINK_BIND_INFO" {
                    this.publish_receiver_query(
                        &container,
                        product_id,
                        projection.as_ref().and_then(|p| p.as_ref().ok()),
                        window,
                        cx,
                    );
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn observe_discovery(
        &mut self,
        observation: &Result<discovery::DiscoverySnapshot, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Expire request observations even on failure. An unchanged physical
        // connection remains owned; absence is unknown, not an offline reply.
        self.discovery_revision = self.discovery_revision.wrapping_add(1);
        if let Ok(snapshot) = observation {
            self.retain_keyboard_brightness_connections(snapshot.devices(), cx);
            self.retain_current_dpi_connections(snapshot.devices(), cx);
            self.retain_mouse_polling_connections(snapshot.devices(), cx);
            self.retain_current_power_connections(snapshot.devices(), cx);
            self.retain_product_lighting_connections(snapshot.devices(), cx);
            self.retain_mouse_dynamic_connections(snapshot.devices(), cx);
        }
        let retained = self
            .devices
            .iter()
            .filter(|workspace| {
                let device = workspace.read(cx).device(cx);
                observation.as_ref().is_ok_and(|snapshot| {
                    retained_connection(&self.device_observations, snapshot.devices(), device)
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        self.retain_interdevice_connections(&retained, cx);
        self.cancel_keyboard_wristrest(cx);
        self.cancel_mouse_runtime_artwork(cx);
        self.cancel_mouse_receiver_indicator();
        self.cancel_controller_inputs(cx);
        self.retain_hanbo_cooler_connections(&retained, cx);
        self.cancel_thx_sound_jobs();
        self.thx_progress.clear();
        self.receiver_queries.clear();
        self.cancel_receiver_pairing();
        self.pairing_transport
            .reconnect_owners()
            .retain_physical(|product_id, container| {
                observation.as_ref().is_ok_and(|snapshot| {
                    snapshot.devices().iter().any(|device| {
                        device.physical_product_id() == product_id
                            && device.container().eq_ignore_ascii_case(container)
                    })
                })
            });
        self.device_observations.clear();
        self.device_read_scopes.clear();
        self.device_value_owners.clear();
        self.receiver_devices.clear();
        self.receiver_devices_complete = false;
        for workspace in &self.devices {
            if !retained.contains(workspace) {
                workspace.update(cx, |workspace, cx| workspace.observe_connection(None, cx));
                observe_polling_connection(workspace, None, cx);
            }
        }
        let snapshot = match observation {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.status = format!("设备发现失败：{error}");
                self.sync_thx_render_lifecycle(false, window, cx);
                self.sync_audio_notifications(window, cx);
                self.sync_keyboard_wristrest(window, cx);
                self.sync_mouse_runtime_artwork(window, cx);
                self.sync_controller_inputs(cx);
                self.sync_audio_preset_shortcut_cleanup(cx);
                self.sync_known_devices(cx);
                self.sync_local_mapping_targets(window, cx);
                self.sync_hyperpolling_receivers(window, cx);
                cx.notify();
                return;
            }
        };
        let mut errors = snapshot.errors().to_vec();
        self.device_observations = snapshot.devices().to_vec();
        errors.extend(self.apply_device_observations(
            snapshot.devices(),
            &retained,
            true,
            window,
            cx,
        ));
        self.observe_camera_host_window(window, cx);
        self.rebuild_receiver_devices(cx);
        self.receiver_devices_complete = errors.is_empty();
        self.status = if errors.is_empty() {
            format!(
                "已观察到 {} 项产品接口或接收器关联",
                snapshot.devices().len()
            )
        } else {
            format!("设备发现部分完成：{}", errors.join("；"))
        };
        self.sync_known_devices(cx);
        self.sync_local_mapping_targets(window, cx);
        self.sync_gamer_room(cx);
        self.sync_audio_notifications(window, cx);
        self.sync_keyboard_wristrest(window, cx);
        self.sync_mouse_runtime_artwork(window, cx);
        self.sync_controller_inputs(cx);
        self.sync_controller_lighting_owners(self.receiver_devices_complete, cx);
        self.sync_receiver_lighting_owners(self.receiver_devices_complete, cx);
        self.sync_hanbo_cooler_connections(cx);
        self.sync_hyperpolling_receivers(window, cx);
        self.sync_audio_preset_shortcut_cleanup(cx);
        self.sync_thx_render_lifecycle(self.receiver_devices_complete, window, cx);
        self.refresh_thx_spatial(window, cx);
        let tray_widgets = self.tray_widgets(cx);
        if let Some(tray) = &mut self.tray {
            tray.set_widget_devices(tray_widgets, cx);
        }
        cx.notify();
    }

    fn apply_device_observations(
        &mut self,
        observations: &[discovery::ObservedDevice],
        retained: &[Entity<ProductWorkspace>],
        collect_read_scopes: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<String> {
        let mut errors = Vec::new();
        for observed in observations {
            let matching: Vec<_> = self
                .devices
                .iter()
                .filter(|workspace| observed.matches(workspace.read(cx).device(cx)))
                .cloned()
                .collect();
            match matching.as_slice() {
                [workspace] => workspace.update(cx, |workspace, cx| {
                    workspace.observe_connection(Some(observed.connection()), cx)
                }),
                [] => {
                    if let Some(device) = observed.clone().into_device() {
                        self.add_device(device, window, cx);
                    } else {
                        errors.push(format!(
                            "已观察到产品 {}，但没有对应的本地页面",
                            observed.product_id()
                        ));
                    }
                }
                _ => errors.push("同一产品/容器存在多个本地身份，尚未取得序列号来消除歧义".into()),
            }
            let matches: Vec<_> = self
                .devices
                .iter()
                .filter(|workspace| observed.matches(workspace.read(cx).device(cx)))
                .collect();
            if let [workspace] = matches.as_slice() {
                if collect_read_scopes {
                    workspace.update(cx, |workspace, cx| workspace.begin_dpi_basic_reads(cx));
                }
                // An identical full route is the same connection generation.
                // A real removal already publishes None before a later reappearance.
                if !retained.contains(workspace) {
                    observe_polling_connection(workspace, observed.transport(), cx);
                }
                if collect_read_scopes {
                    self.device_value_owners
                        .insert(workspace.read(cx).identity(cx));
                }
                if collect_read_scopes
                    && let Some(scope) = workspace.read(cx).mouse_polling_scope(cx)
                {
                    let idle = workspace.update(cx, |workspace, cx| {
                        workspace.begin_mouse_idle_read(scope, cx)
                    });
                    self.device_read_scopes.insert(
                        workspace.read(cx).identity(cx),
                        DeviceReadScope {
                            polling: scope,
                            idle,
                        },
                    );
                }
            }
        }
        errors
    }

    fn rebuild_receiver_devices(&mut self, cx: &App) {
        let app: &App = cx;
        self.receiver_devices = self
            .device_observations
            .iter()
            .filter(|observed| match observed.connection() {
                razer_model::model::DeviceConnectionObservation::ReceiverPeer(1) => true,
                razer_model::model::DeviceConnectionObservation::UsbPresent
                | razer_model::model::DeviceConnectionObservation::HidPresent => !matches!(
                    observed.transport(),
                    Some(discovery::ObservedTransport::Dongle)
                ),
                _ => false,
            })
            .filter_map(|observed| {
                let mut matches = self
                    .devices
                    .iter()
                    .filter(|workspace| observed.matches(workspace.read(app).device(app)));
                let workspace = matches.next()?;
                if matches.next().is_some() {
                    return None;
                }
                let device = workspace.read(app).device(app);
                let serial = device.serial_number.to_ascii_uppercase();
                (!serial.starts_with("PREVIEW-") && !serial.starts_with("DEMO-")).then(|| {
                    (
                        workspace.read(app).identity(app),
                        (device.product_id, device.edition_id),
                    )
                })
            })
            .collect();
    }

    /// Replace only peers of the receiver that actually answered. Query errors
    /// expire that scope to unknown; unrelated interfaces retain their evidence.
    pub(super) fn publish_receiver_query(
        &mut self,
        container: &str,
        product_id: u32,
        projection: Option<&discovery::ReceiverQueryProjection>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if projection.is_some_and(|projection| {
            projection.receiver_product_id() != product_id
                || !projection.container().eq_ignore_ascii_case(container)
        }) {
            return;
        }
        let incoming = projection.map_or(&[][..], |projection| projection.devices());
        let removed = replace_receiver_observations(
            &mut self.device_observations,
            container,
            product_id,
            incoming,
        );
        self.retain_mouse_polling_connections(&self.device_observations, cx);
        self.retain_current_power_connections(&self.device_observations, cx);
        self.retain_product_lighting_connections(&self.device_observations, cx);
        self.retain_mouse_dynamic_connections(&self.device_observations, cx);
        let retired = self
            .devices
            .iter()
            .filter(|workspace| {
                let device = workspace.read(cx).device(cx);
                removed
                    .iter()
                    .chain(incoming)
                    .any(|observed| observed.matches(device))
                    && !retained_connection(&removed, incoming, device)
            })
            .cloned()
            .collect::<Vec<_>>();
        let retained = self
            .devices
            .iter()
            .filter(|workspace| !retired.contains(workspace))
            .cloned()
            .collect::<Vec<_>>();
        self.retain_interdevice_connections(&retained, cx);
        for workspace in &retired {
            self.device_read_scopes
                .remove(&workspace.read(cx).identity(cx));
            self.device_value_owners
                .remove(&workspace.read(cx).identity(cx));
            // Discard old parameter observations together with their transport.
            workspace.update(cx, |workspace, cx| {
                workspace.observe_connection(None, cx);
                workspace.observe_read_values(None, cx);
            });
            observe_polling_connection(workspace, None, cx);
        }
        self.receiver_devices_complete &= projection.is_some_and(|p| p.errors().is_empty());
        self.rebuild_receiver_devices(cx);
        // Connection publication advances the polling epoch. A metadata-only
        // requery must not clear rates/firmware or invalidate outstanding reads.
        let changed = incoming
            .iter()
            .filter(|new| !removed.iter().any(|old| same_connection(old, new)))
            .cloned()
            .collect::<Vec<_>>();
        let errors = self.apply_device_observations(&changed, &[], false, window, cx);
        self.receiver_devices_complete &= errors.is_empty();
        self.rebuild_receiver_devices(cx);
        if !errors.is_empty() {
            self.status = format!("接收器设备页面部分更新：{}", errors.join("；"));
        }
        self.sync_known_devices(cx);
        self.sync_local_mapping_targets(window, cx);
        self.sync_gamer_room(cx);
        self.sync_audio_notifications(window, cx);
        self.sync_audio_preset_shortcut_cleanup(cx);
        self.sync_thx_render_lifecycle(self.receiver_devices_complete, window, cx);
        let tray_widgets = self.tray_widgets(cx);
        if let Some(tray) = &mut self.tray {
            tray.set_widget_devices(tray_widgets, cx);
        }
        cx.notify();
    }

    /// Second discovery phase: parameter reads use the owner/profile captured
    /// after the first connection observation, before those reads were issued.
    pub(super) fn observe_device_values(
        &mut self,
        observation: &Result<discovery::DiscoverySnapshot, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut scopes = std::mem::take(&mut self.device_read_scopes);
        let owners = std::mem::take(&mut self.device_value_owners);
        let snapshot = match observation {
            Ok(snapshot) => snapshot,
            Err(error) => {
                for workspace in &self.devices {
                    if let Some(read) = scopes
                        .get(&workspace.read(cx).identity(cx))
                        .and_then(|scope| scope.idle)
                    {
                        workspace.update(cx, |workspace, cx| {
                            workspace.observe_mouse_idle_initial(read, None, window, cx)
                        });
                    }
                }
                self.status = format!("设备参数读取失败：{error}");
                self.request_initial_dpi_stage_reads(cx);
                cx.notify();
                return;
            }
        };
        let mut identity_publications = Vec::new();
        for observed in snapshot.devices() {
            let matches = self
                .devices
                .iter()
                .filter(|workspace| observed.matches(workspace.read(cx).device(cx)))
                .collect::<Vec<_>>();
            let [workspace] = matches.as_slice() else {
                continue;
            };
            if !owners.contains(&workspace.read(cx).identity(cx)) {
                continue;
            }
            let scope = scopes.remove(&workspace.read(cx).identity(cx));
            let values = observed.read_values().cloned();
            workspace.update(cx, |workspace, cx| {
                workspace.observe_keyboard_metadata(observed.keyboard_metadata().cloned(), cx);
                workspace.observe_read_values(values.clone(), cx);
                if let Some(read) = scope.and_then(|scope| scope.idle) {
                    workspace.observe_mouse_idle_initial(
                        read,
                        values.as_ref().and_then(|values| values.idle_raw_time),
                        window,
                        cx,
                    );
                }
                let (Some(scope), Some(values)) = (scope.map(|scope| scope.polling), values) else {
                    return;
                };
                workspace.observe_mouse_polling(
                    scope,
                    MousePollingObservation::FirmwareVersion(values.firmware),
                    cx,
                );
                let field = match observed.transport() {
                    Some(discovery::ObservedTransport::Wired) => Some(PollingField::Wired),
                    Some(discovery::ObservedTransport::Dongle) => Some(PollingField::Wireless),
                    // These source pages hide polling on BLE; no field is invented.
                    _ => None,
                };
                if let (Some(field), Some(rate)) = (field, values.polling_hz) {
                    workspace.observe_mouse_polling(
                        scope,
                        MousePollingObservation::Rate(field, rate),
                        cx,
                    );
                }
            });
            if let (Some(serial), Some(reading)) =
                (observed.keyboard_serial(), observed.keyboard_metadata())
            {
                if let Some(publication) = razer_model::source_identity::keyboard_publication(
                    observed.product_id(),
                    serial,
                    reading,
                    workspace.read(cx).device(cx).layout_id,
                    workspace.read(cx).device(cx).use_ble,
                ) {
                    identity_publications.push(publication);
                } else {
                    log::warn!(
                        "[keyboard identity cache] product {} has no publishable queried identity/source names",
                        observed.product_id()
                    );
                }
            }
        }
        // A partial snapshot can omit a requested device completely. Complete
        // its GET as unobserved so the retained page can retry a later scan.
        for workspace in &self.devices {
            if let Some(read) = scopes
                .get(&workspace.read(cx).identity(cx))
                .and_then(|scope| scope.idle)
            {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_mouse_idle_initial(read, None, window, cx)
                });
            }
        }
        if !identity_publications.is_empty() {
            let previous = self.keyboard_identity_cache_task.take();
            self.keyboard_identity_cache_pending += 1;
            self.keyboard_identity_cache_task = Some(cx.spawn(async move |this, cx| {
                if let Some(previous) = previous {
                    previous.await;
                }
                cx.background_spawn(async move {
                    let path = razer_storage::source_local::path();
                    for (policy, record) in identity_publications {
                        let policy = match policy {
                            "replace_product" => {
                                razer_storage::source_identity::CachePolicy::ReplaceProduct
                            }
                            "replace_serial" => {
                                razer_storage::source_identity::CachePolicy::ReplaceSerial
                            }
                            _ => {
                                log::warn!("[keyboard identity cache] source policy is unknown");
                                continue;
                            }
                        };
                        if let Err(error) =
                            razer_storage::source_identity::publish(&path, &record, policy)
                        {
                            log::warn!(
                                "[keyboard identity cache] local publication failed: {error:#}"
                            );
                        }
                    }
                })
                .await;
                let _ = this.update(cx, |this, cx| {
                    if super::finish_identity_cache_batch(&mut this.keyboard_identity_cache_pending)
                    {
                        this.keyboard_identity_cache_task = None;
                        if this.close_requested && this.save_task.is_none() {
                            this.continue_save_queue(true, cx);
                        }
                    }
                });
            }));
        }
        if !snapshot.errors().is_empty() {
            self.status = format!("设备参数部分读取完成：{}", snapshot.errors().join("；"));
        }
        // The basic query worker has completed; do not compete with its retained
        // path lock during discovery's first/second phase handshake.
        self.initialize_current_mouse_profiles(cx);
        self.initialize_mouse_scroll_profiles(cx);
        self.request_initial_dpi_stage_reads(cx);
        self.sync_known_devices(cx);
        self.sync_gamer_room(cx);
        let tray_widgets = self.tray_widgets(cx);
        if let Some(tray) = &mut self.tray {
            tray.set_widget_devices(tray_widgets, cx);
        }
        cx.notify();
    }

    fn request_initial_dpi_stage_reads(&mut self, cx: &mut Context<Self>) {
        let candidates = self
            .devices
            .iter()
            .filter(|workspace| {
                let device = workspace.read(cx).device(cx);
                device.product_id == 182
                    && self
                        .device_observations
                        .iter()
                        .filter(|route| {
                            route.matches(device)
                                && route.peer_product_id().is_none()
                                && matches!(
                                    route.transport(),
                                    Some(discovery::ObservedTransport::Wired)
                                )
                        })
                        .count()
                        == 1
            })
            .cloned()
            .collect::<Vec<_>>();
        for workspace in candidates {
            workspace.update(cx, |workspace, cx| workspace.finish_dpi_basic_reads(cx));
        }
    }

    pub(super) fn query_receiver_pairing(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        event: &ReceiverPairingEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if workspace.read(cx).device(cx).product_id == 179 {
            self.request_hyperpolling_pairing(workspace, event, window, cx);
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        if matches!(event.intent(), ReceiverPairingIntent::Cancel) {
            self.receiver_queries.remove(&identity);
            return;
        }
        if !matches!(event.intent(), ReceiverPairingIntent::QueryBindings) {
            // Pair/unpair/device writes remain explicit unsent UI intents.
            return;
        }
        let container = workspace.read(cx).device(cx).device_container_id.clone();
        let product_id = workspace.read(cx).device(cx).real_product_id;
        let route = receiver_route_for_owner(
            &self.device_observations,
            workspace.read(cx).device(cx),
            &container,
            product_id,
        );
        let route = match route {
            Ok(route) => route,
            Err(error) => {
                self.receiver_queries.remove(&identity);
                self.status = format!("配对信息读取不可用：{error:#}");
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_receiver_pairing(
                        ReceiverPairingObservation::failed(
                            event.session(),
                            ReceiverOperation::Bindings,
                        ),
                        cx,
                    );
                });
                cx.notify();
                return;
            }
        };
        let revision = self.discovery_revision;
        let session = event.session();
        let generation = self.receiver_queries.entry(identity.clone()).or_default();
        *generation = (session, generation.1.wrapping_add(1));
        let generation = *generation;
        cx.spawn_in(window, async move |this, cx| {
            let query_route = route.clone();
            let result = cx
                .background_spawn(async move {
                    let mut client = ServiceClient::spawn()?;
                    let result = query_route.query(&mut client);
                    let shutdown = client.request(ServiceRequest::Shutdown);
                    match (result, shutdown) {
                        (Ok(peers), Ok(_)) => Ok(peers),
                        (Err(error), _) | (_, Err(error)) => Err(error),
                    }
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.receiver_queries.get(&identity) != Some(&generation)
                    || this.discovery_revision != revision
                    || !this.devices.contains(&workspace)
                    || workspace.read(cx).identity(cx) != identity
                    || !receiver_owner_matches(
                        workspace.read(cx).device(cx),
                        &container,
                        product_id,
                    )
                    || !this.device_observations.iter().any(|observed| {
                        observed.matches(workspace.read(cx).device(cx)) && route.matches(observed)
                    })
                {
                    return;
                }
                this.receiver_queries.remove(&identity);
                let peers = result
                    .as_ref()
                    .map_err(|error| format!("{error:#}"))
                    .and_then(|projection| {
                        projection
                            .pairing_payload()
                            .map_err(|error| format!("{error:#}"))
                    })
                    .and_then(|payload| {
                        serde_json::from_value::<Vec<ReceiverPeer>>(payload)
                            .map_err(|error| error.to_string())
                    });
                let observation = match peers {
                    Ok(peers) => ReceiverPairingObservation::bindings(session, peers),
                    Err(error) => {
                        this.status = format!("读取配对信息失败：{error:#}");
                        ReceiverPairingObservation::failed(session, ReceiverOperation::Bindings)
                    }
                };
                // The feature rejects stale session IDs and replies after dismissal.
                let accepted = workspace.update(cx, |workspace, cx| {
                    workspace.observe_receiver_pairing(observation, cx)
                });
                // Deliver bindings first: publishing connection changes can start
                // a new metadata query and must not invalidate this accepted reply.
                if accepted {
                    this.publish_receiver_query(
                        &container,
                        product_id,
                        result.as_ref().ok(),
                        window,
                        cx,
                    );
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn open_receiver_device(
        &mut self,
        event: &razer_pages::features::ReceiverDeviceRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let identity = (event.product_id(), event.edition_id());
        let observed = &self.receiver_devices;
        let matches: Vec<_> = self
            .devices
            .iter()
            .filter(|workspace| {
                // Retain the exact owners accepted by this discovery. Another
                // offline/local device with the same PID/edition is not a target.
                observed.get(&workspace.read(cx).identity(cx)) == Some(&identity)
            })
            .cloned()
            .collect();
        let [workspace] = matches.as_slice() else {
            self.status = "无法确定接收器对应的设备页面；设备身份尚未唯一匹配。".into();
            cx.notify();
            return;
        };
        let key = workspace.read(cx).identity(cx);
        // The current receiver card chooses Performance for a mouse and
        // Customize for a keyboard. Retained workspaces must not keep an old tab.
        workspace.update(cx, |workspace, cx| {
            workspace.set_page(event.page(), window, cx)
        });
        self.navigate(Location::Device(key), window, cx);
    }
}

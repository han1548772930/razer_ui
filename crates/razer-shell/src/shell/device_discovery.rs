//! Read observations into retained workspaces without replacing their local drafts.
use super::*;
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !workspace
            .read(cx)
            .mouse_idle_request_matches(scope, minutes, cx)
        {
            return;
        }
        let identity = workspace.read(cx).identity(cx);
        let product_id = workspace.read(cx).device(cx).product_id;
        let container = workspace.read(cx).device(cx).device_container_id.clone();
        let routes = self
            .device_observations
            .iter()
            .filter(|observed| {
                observed.matches(workspace.read(cx).device(cx))
                    && observed.peer_product_id().is_none()
                    && matches!(
                        observed.transport(),
                        Some(discovery::ObservedTransport::Wired)
                    )
            })
            .collect::<Vec<_>>();
        let [observed] = routes.as_slice() else {
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_idle(scope, minutes, None, cx)
            });
            self.status =
                "休眠设置未发送：尚无唯一的有线设备连接；接收器转发写入尚未实现。本地草稿已保留。"
                    .into();
            cx.notify();
            return;
        };
        let physical_product_id = observed.physical_product_id();
        let route_observation = (**observed).clone();
        let retained_node = route_observation.hid_node().cloned();
        let revision = self.discovery_revision;
        // An already queued discovery read predates this mutation. It cannot
        // later replace the confirmed parameter snapshot with an older value.
        self.device_value_owners.remove(&identity);
        self.device_read_scopes.remove(&identity);
        self.status = "正在写入休眠时间并读取设备确认。".into();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    use anyhow::Context as _;
                    use razer_device::device_reads::DeviceReadValue;
                    use razer_device::device_writes::DeviceWriteSetting;
                    let cap = razer_device::device_reads::capability(product_id)
                        .context("产品读取能力未核实")?;
                    let raw_time = u16::from(minutes) * 60;
                    let setting = DeviceWriteSetting::Idle { raw_time };
                    let mut client = ServiceClient::spawn()?;
                    let result = (|| {
                        let route =
                            razer_discovery::direct::resolve(&mut client, &route_observation, cap)?;
                        match route.write(&mut client, setting)? {
                            DeviceReadValue::Idle { raw_time: observed }
                                if observed == raw_time =>
                            {
                                Ok(observed)
                            }
                            _ => anyhow::bail!("设备回读与休眠设置不一致"),
                        }
                    })();
                    // Preserve a real acknowledgement even if teardown fails; never
                    // retry a mutation just because the worker could not shut down.
                    let shutdown_error = client
                        .request(ServiceRequest::Shutdown)
                        .err()
                        .map(|error| format!("{error:#}"));
                    result.map(|observed| (observed, shutdown_error))
                })
                .await;
            let _ = this.update_in(cx, |this, _, cx| {
                if !workspace
                    .read(cx)
                    .mouse_idle_in_flight_matches(scope, minutes, cx)
                {
                    return;
                }
                if !this.devices.contains(&workspace)
                    || workspace.read(cx).identity(cx) != identity
                    || !workspace
                        .read(cx)
                        .mouse_idle_request_matches(scope, minutes, cx)
                {
                    // Restore/disconnect cannot cancel an already sent command.
                    // Its completion must release the slot without publishing an
                    // old observation or replacing the current scope's status.
                    workspace.update(cx, |workspace, cx| {
                        workspace.finish_mouse_idle(scope, minutes, None, cx)
                    });
                    return;
                }
                let route_current = this.discovery_revision == revision
                    && this.device_observations.iter().any(|observed| {
                        observed.matches(workspace.read(cx).device(cx))
                            && observed.container().eq_ignore_ascii_case(&container)
                            && observed.physical_product_id() == physical_product_id
                            && observed.hid_node() == retained_node.as_ref()
                            && observed.peer_product_id().is_none()
                            && matches!(
                                observed.transport(),
                                Some(discovery::ObservedTransport::Wired)
                            )
                    });
                let confirmed = route_current
                    .then(|| result.as_ref().ok().map(|(raw, _)| *raw))
                    .flatten();
                this.device_value_owners.remove(&identity);
                this.device_read_scopes.remove(&identity);
                workspace.update(cx, |workspace, cx| {
                    workspace.finish_mouse_idle(scope, minutes, confirmed, cx)
                });
                this.status = if !route_current {
                    "设备连接已变化，未确认当前休眠设置。本地草稿已保留。".into()
                } else {
                    match result {
                        Ok((_, None)) => format!(
                            "设备已回读确认休眠时间：{minutes} 分钟；本地配置保存仍单独处理。"
                        ),
                        Ok((_, Some(error))) => format!(
                            "设备已回读确认休眠时间：{minutes} 分钟；通信进程退出失败：{error}"
                        ),
                        Err(error) => format!(
                            "休眠设置未确认：{error:#}。本地草稿已保留，请重新读取设备状态。"
                        ),
                    }
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn query_dock_pairing(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        event: &razer_pages::features::DockPairingEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let identity = workspace.read(cx).identity(cx);
        if event.kind() == "DUALLINK_CANCEL" {
            self.receiver_queries.remove(&identity);
            self.retire_receiver_pairing(&identity);
            return;
        }
        if event.kind() != "DUALLINK_BIND_INFO" {
            self.submit_dock_pairing(workspace, event, window, cx);
            return;
        }
        self.retire_receiver_pairing(&identity);
        let device = workspace.read(cx).device(cx);
        let container = device.device_container_id.clone();
        let product_id = device.real_product_id;
        let session = event.session();
        let route =
            receiver_route_for_owner(&self.device_observations, device, &container, product_id);
        let route = match route {
            Ok(route) => route,
            Err(error) => {
                self.receiver_queries.remove(&identity);
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_dock_pairing(
                        razer_pages::features::DockPairingObservation::result(
                            session,
                            "DUALLINK_BIND_INFO",
                            Err(format!("{error:#}")),
                        ),
                        cx,
                    );
                });
                cx.notify();
                return;
            }
        };
        let retained_route = route.clone();
        let revision = self.discovery_revision;
        let generation = self.receiver_queries.entry(identity.clone()).or_default();
        *generation = (session, generation.1.wrapping_add(1));
        let generation = *generation;
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut client = ServiceClient::spawn()?;
                    let result = route.query(&mut client);
                    let shutdown = client.request(ServiceRequest::Shutdown);
                    match (result, shutdown) {
                        (Ok(value), Ok(_)) => Ok(value),
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
                        observed.matches(workspace.read(cx).device(cx))
                            && retained_route.matches(observed)
                    })
                {
                    return;
                }
                this.receiver_queries.remove(&identity);
                let payload = result
                    .as_ref()
                    .map_err(|error| format!("{error:#}"))
                    .and_then(|projection| {
                        projection
                            .pairing_payload()
                            .map_err(|error| format!("{error:#}"))
                    });
                let accepted = workspace.update(cx, |workspace, cx| {
                    workspace.observe_dock_pairing(
                        razer_pages::features::DockPairingObservation::result(
                            session,
                            "DUALLINK_BIND_INFO",
                            payload,
                        ),
                        cx,
                    )
                });
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

    pub(super) fn observe_discovery(
        &mut self,
        observation: &Result<discovery::DiscoverySnapshot, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Expire the previous observation even on failure. Absence is unknown,
        // not an invented offline reply. No profile/identity is overwritten.
        self.discovery_revision = self.discovery_revision.wrapping_add(1);
        self.receiver_queries.clear();
        self.cancel_receiver_pairing();
        self.device_observations.clear();
        self.device_read_scopes.clear();
        self.device_value_owners.clear();
        self.receiver_devices.clear();
        self.receiver_devices_complete = false;
        for workspace in &self.devices {
            workspace.update(cx, |workspace, cx| workspace.observe_connection(None, cx));
            observe_polling_connection(workspace, None, cx);
        }
        let snapshot = match observation {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.status = format!("设备发现失败：{error}；本地草稿已保留。");
                self.sync_audio_notifications(window, cx);
                self.sync_known_devices(cx);
                cx.notify();
                return;
            }
        };
        let mut errors = snapshot.errors().to_vec();
        self.device_observations = snapshot.devices().to_vec();
        errors.extend(self.apply_device_observations(snapshot.devices(), true, window, cx));
        self.rebuild_receiver_devices(cx);
        self.receiver_devices_complete = errors.is_empty();
        self.status = if errors.is_empty() {
            format!(
                "已观察到 {} 项产品接口或接收器关联 · 配置仍为本地草稿",
                snapshot.devices().len()
            )
        } else {
            format!("设备发现部分完成：{}", errors.join("；"))
        };
        self.sync_known_devices(cx);
        self.sync_gamer_room(cx);
        self.sync_audio_notifications(window, cx);
        let tray_widgets = self.tray_widgets(cx);
        if let Some(tray) = &mut self.tray {
            tray.set_widget_devices(tray_widgets, cx);
        }
        cx.notify();
    }

    fn apply_device_observations(
        &mut self,
        observations: &[discovery::ObservedDevice],
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
                observe_polling_connection(workspace, observed.transport(), cx);
                if collect_read_scopes {
                    self.device_value_owners
                        .insert(workspace.read(cx).identity(cx));
                }
                if collect_read_scopes
                    && let Some(scope) = workspace.read(cx).mouse_polling_scope(cx)
                {
                    self.device_read_scopes
                        .insert(workspace.read(cx).identity(cx), scope);
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
        for workspace in &self.devices {
            if removed
                .iter()
                .chain(incoming)
                .any(|observed| observed.matches(workspace.read(cx).device(cx)))
            {
                let unchanged = removed.iter().any(|old| {
                    old.matches(workspace.read(cx).device(cx))
                        && incoming.iter().any(|new| same_receiver_peer(old, new))
                });
                if unchanged {
                    continue;
                }
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
        }
        self.receiver_devices_complete &= projection.is_some_and(|p| p.errors().is_empty());
        self.rebuild_receiver_devices(cx);
        // Connection publication advances the polling epoch. A metadata-only
        // requery must not clear rates/firmware or invalidate outstanding reads.
        let changed = incoming
            .iter()
            .filter(|new| !removed.iter().any(|old| same_receiver_peer(old, new)))
            .cloned()
            .collect::<Vec<_>>();
        let errors = self.apply_device_observations(&changed, false, window, cx);
        self.receiver_devices_complete &= errors.is_empty();
        self.rebuild_receiver_devices(cx);
        if !errors.is_empty() {
            self.status = format!("接收器设备页面部分更新：{}", errors.join("；"));
        }
        self.sync_known_devices(cx);
        self.sync_gamer_room(cx);
        self.sync_audio_notifications(window, cx);
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
        cx: &mut Context<Self>,
    ) {
        let scopes = std::mem::take(&mut self.device_read_scopes);
        let owners = std::mem::take(&mut self.device_value_owners);
        let snapshot = match observation {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.status = format!("设备参数读取失败：{error}；本地草稿已保留。");
                self.request_initial_dpi_stage_reads(cx);
                cx.notify();
                return;
            }
        };
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
            let scope = scopes.get(&workspace.read(cx).identity(cx)).copied();
            let values = observed.read_values().cloned();
            workspace.update(cx, |workspace, cx| {
                workspace.observe_read_values(values.clone(), cx);
                let (Some(scope), Some(values)) = (scope, values) else {
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
        }
        if !snapshot.errors().is_empty() {
            self.status = format!("设备参数部分读取完成：{}", snapshot.errors().join("；"));
        }
        // The basic query worker has completed; do not compete with its retained
        // path lock during discovery's first/second phase handshake.
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

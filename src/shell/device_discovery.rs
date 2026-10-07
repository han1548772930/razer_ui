//! Read observations into retained workspaces without replacing their local drafts.
use super::*;
use crate::backend::{
    discovery,
    runtime::{ServiceClient, ServiceRequest},
};
use crate::features::mouse_polling::{MousePollingObservation, PollingConnection, PollingField};
use crate::features::{
    ReceiverOperation, ReceiverPairingEvent, ReceiverPairingIntent, ReceiverPairingObservation,
    ReceiverPeer,
};

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
    pub(super) fn query_dock_pairing(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        event: &crate::features::DockPairingEvent,
        cx: &mut Context<Self>,
    ) {
        let identity = workspace.read(cx).identity(cx);
        if event.kind() == "DUALLINK_CANCEL" {
            self.receiver_queries.remove(&identity);
            return;
        }
        // Scanning, pairing and unpairing are retained local UI intents. Only
        // the source-verified existing binding query is sent to the device.
        if event.kind() != "DUALLINK_BIND_INFO" {
            return;
        }
        let device = workspace.read(cx).device(cx);
        let container = device.device_container_id.clone();
        let product_id = device.real_product_id;
        let session = event.session();
        let generation = self.receiver_queries.entry(identity.clone()).or_default();
        *generation = (session, generation.1.wrapping_add(1));
        let generation = *generation;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut client = ServiceClient::spawn()?;
                    let result = (|| {
                        let hid = client.request(ServiceRequest::HidDevices)?;
                        let value =
                            discovery::query_receiver(&mut client, &hid, &container, product_id)?;
                        discovery::pairing_payload(product_id, &value)
                    })();
                    let shutdown = client.request(ServiceRequest::Shutdown);
                    match (result, shutdown) {
                        (Ok(value), Ok(_)) => Ok(value),
                        (Err(error), _) | (_, Err(error)) => Err(error),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.receiver_queries.get(&identity) != Some(&generation) {
                    return;
                }
                this.receiver_queries.remove(&identity);
                let result = result.map_err(|error| format!("{error:#}"));
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_dock_pairing(
                        crate::features::DockPairingObservation::result(
                            session,
                            "DUALLINK_BIND_INFO",
                            result,
                        ),
                        cx,
                    )
                });
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
        self.device_read_scopes.clear();
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
                self.sync_known_devices(cx);
                cx.notify();
                return;
            }
        };
        let mut errors = snapshot.errors().to_vec();
        for observed in snapshot.devices() {
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
                observe_polling_connection(workspace, observed.transport(), cx);
                if let Some(scope) = workspace.read(cx).mouse_polling_scope(cx) {
                    self.device_read_scopes
                        .insert(workspace.read(cx).identity(cx), scope);
                }
            }
        }
        {
            let app: &App = cx;
            self.receiver_devices = snapshot
                .devices()
                .iter()
                .filter(|observed| match observed.connection() {
                    crate::model::DeviceConnectionObservation::ReceiverPeer(1) => true,
                    crate::model::DeviceConnectionObservation::UsbPresent
                    | crate::model::DeviceConnectionObservation::HidPresent => !matches!(
                        observed.transport(),
                        Some(discovery::ObservedTransport::Dongle)
                    ),
                    _ => false,
                })
                .flat_map(|observed| {
                    self.devices.iter().filter_map(move |workspace| {
                        let device = workspace.read(app).device(app);
                        (observed.matches(device)
                            && !device.serial_number.starts_with("PREVIEW-")
                            && !device.serial_number.starts_with("DEMO-"))
                        .then(|| {
                            (
                                workspace.read(app).identity(app),
                                (device.product_id, device.edition_id),
                            )
                        })
                    })
                })
                .collect();
        }
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
        let snapshot = match observation {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.status = format!("设备参数读取失败：{error}；本地草稿已保留。");
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
        self.sync_known_devices(cx);
        self.sync_gamer_room(cx);
        cx.notify();
    }

    pub(super) fn query_receiver_pairing(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        event: &ReceiverPairingEvent,
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
        let session = event.session();
        let generation = self.receiver_queries.entry(identity.clone()).or_default();
        *generation = (session, generation.1.wrapping_add(1));
        let generation = *generation;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let mut client = ServiceClient::spawn()?;
                    let result = (|| {
                        let hid = client.request(ServiceRequest::HidDevices)?;
                        let value =
                            discovery::query_receiver(&mut client, &hid, &container, product_id)?;
                        let payload = discovery::pairing_payload(product_id, &value)?;
                        serde_json::from_value::<Vec<ReceiverPeer>>(payload)
                            .map_err(anyhow::Error::from)
                    })();
                    let shutdown = client.request(ServiceRequest::Shutdown);
                    match (result, shutdown) {
                        (Ok(peers), Ok(_)) => Ok(peers),
                        (Err(error), _) | (_, Err(error)) => Err(error),
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.receiver_queries.get(&identity) != Some(&generation) {
                    return;
                }
                let observation = match result {
                    Ok(peers) => ReceiverPairingObservation::bindings(session, peers),
                    Err(error) => {
                        this.status = format!("读取配对信息失败：{error:#}");
                        ReceiverPairingObservation::failed(session, ReceiverOperation::Bindings)
                    }
                };
                // The feature rejects stale session IDs and replies after dismissal.
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_receiver_pairing(observation, cx)
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn open_receiver_device(
        &mut self,
        event: &crate::features::ReceiverDeviceRequested,
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

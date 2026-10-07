//! Live interface/receiver observations, separate from profile loading and drafts.
//! Current host identity routing and source-derived shared protocol selection.
use super::device_identity::{self, IdentityLookup};
use super::runtime::{ServiceClient, ServiceRequest};
use crate::model::{
    Device, DeviceCategory, DeviceConnectionObservation, LocalizedText, SetupStatus,
};
use anyhow::{Context as _, ensure};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
pub(crate) enum ObservedTransport {
    Wired,
    Dongle,
    Ble,
}

#[derive(Clone)]
pub(crate) struct ObservedDevice {
    product_id: u32,
    real_product_id: u32,
    container: String,
    serial: String,
    use_ble: bool,
    transport: Option<ObservedTransport>,
    connection: DeviceConnectionObservation,
}
impl ObservedDevice {
    pub(crate) fn product_id(&self) -> u32 {
        self.product_id
    }
    pub(crate) fn matches(&self, device: &Device) -> bool {
        self.product_id == device.product_id
            && self
                .container
                .eq_ignore_ascii_case(&device.device_container_id)
    }
    pub(crate) fn connection(&self) -> DeviceConnectionObservation {
        self.connection
    }
    pub(crate) fn transport(&self) -> Option<ObservedTransport> {
        self.transport
    }
    pub(crate) fn into_device(self) -> Option<Device> {
        let product = crate::product::registered(self.product_id)?;
        let categories = product.categories();
        let category = if categories
            .iter()
            .any(|c| matches!(*c, "MOUSE" | "MOUSEPLUSMAT"))
        {
            DeviceCategory::Mouse
        } else if categories.contains(&"KEYBOARD") {
            DeviceCategory::Keyboard
        } else if categories.contains(&"KEYPAD") {
            DeviceCategory::Keypad
        } else if categories.contains(&"ACCESSORY") {
            DeviceCategory::Accessory
        } else if categories.contains(&"MOUSEMAT") {
            DeviceCategory::Mousepad
        } else if categories.iter().any(|c| c.starts_with("AUDIO")) {
            DeviceCategory::Audio
        } else if categories.iter().any(|c| c.starts_with("GAMEPAD")) {
            DeviceCategory::Controller
        } else {
            DeviceCategory::Other
        };
        // Static product naming identifies a route, never invents a hardware edition.
        let name = LocalizedText {
            values: BTreeMap::from([("en".into(), product.name().into())]),
        };
        Some(Device {
            dashboard: crate::model::DashboardDeviceMetadata {
                local_snapshot: true,
                connection_observation: Some(self.connection),
                ..Default::default()
            },
            sub_devices: None,
            source_device_settings: None,
            serial_number: self.serial,
            product_id: self.product_id,
            real_product_id: self.real_product_id,
            edition_id: 0,
            layout_id: 0,
            device_container_id: self.container,
            category,
            setup_status: SetupStatus::Unknown,
            // The feature owner may create a local draft. No service profile is supplied here.
            active_profile: String::new(),
            profiles: vec![],
            is_single_profile: false,
            is_chroma_device: false,
            has_battery: false,
            use_ble: self.use_ble,
            name: name.clone(),
            product_name: name,
            ui_window_name: String::new(),
            mw_window_name: String::new(),
            min_dpi: None,
            max_dpi: None,
            dpi_step: None,
            support_xy_dpi: false,
            power_status: None,
            dkm_keys: vec![],
            firmware_info: Default::default(),
            features: Default::default(),
            features_initialized: false,
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct DiscoverySnapshot {
    devices: Vec<ObservedDevice>,
    errors: Vec<String>,
}
impl DiscoverySnapshot {
    pub(crate) fn devices(&self) -> &[ObservedDevice] {
        &self.devices
    }
    pub(crate) fn errors(&self) -> &[String] {
        &self.errors
    }
    fn insert(&mut self, observed: ObservedDevice) {
        if let Some(existing) = self.devices.iter_mut().find(|existing| {
            existing.product_id == observed.product_id
                && existing.container.eq_ignore_ascii_case(&observed.container)
        }) {
            // A confirmed radio status conveys more than collection presence.
            // Interface enumeration must never overwrite it just due to PID order.
            if matches!(
                observed.connection,
                DeviceConnectionObservation::ReceiverPeer(_)
            ) {
                existing.connection = observed.connection;
                existing.real_product_id = observed.real_product_id;
                existing.transport = observed.transport;
                existing.use_ble = observed.use_ble;
                existing.serial.clear();
            }
            if existing.serial.is_empty()
                && !matches!(
                    existing.connection,
                    DeviceConnectionObservation::ReceiverPeer(_)
                )
            {
                existing.serial = observed.serial;
            }
        } else {
            self.devices.push(observed);
        }
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)?
        .as_str()
        .filter(|text| !text.trim().is_empty())
}

/// Requires an unambiguous current collection in the receiver's actual container.
pub(crate) fn receiver_request(
    hid: &Value,
    container: &str,
    product_id: u32,
) -> anyhow::Result<ServiceRequest> {
    let items = hid["interfaces"]
        .as_array()
        .context("HID 接口响应不是数组")?;
    let capability = u16::try_from(product_id)
        .ok()
        .and_then(super::receiver_capabilities::capability)
        .context("此产品的无线状态查询协议尚未完成当前原码核实")?;
    let paths: BTreeSet<_> = items
        .iter()
        .filter(|item| {
            item["vendor_id"].as_u64() == Some(u64::from(capability.vendor_id))
                && item["product_id"].as_u64() == Some(u64::from(capability.product_id))
                && item["claim_interface"].as_i64() == Some(i64::from(capability.claim_interface))
                && item["feature_report_bytes"].as_u64() == Some(capability.report_bytes as u64)
                && text(item, "device_container_id")
                    .is_some_and(|id| id.eq_ignore_ascii_case(container))
        })
        .filter_map(|item| text(item, "path"))
        .collect();
    ensure!(
        paths.len() == 1,
        "接收器 {container} 的查询接口缺失或不唯一"
    );
    Ok(ServiceRequest::ReceiverWirelessStatus {
        path: paths.first().unwrap().to_string(),
        device_container_id: container.to_string(),
    })
}

pub(crate) fn receiver_peers(value: &Value) -> anyhow::Result<Vec<(u32, u8)>> {
    let devices = value["devices"]
        .as_array()
        .context("接收器未返回设备列表")?;
    let mut seen = BTreeSet::new();
    devices
        .iter()
        .map(|item| {
            let pid = item["product_id"]
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .context("接收器产品编号无效")?;
            let status = item["status"]
                .as_u64()
                .and_then(|v| u8::try_from(v).ok())
                .context("接收器连接状态无效")?;
            ensure!(pid > 0 && pid <= 65535, "接收器产品编号超出协议范围");
            ensure!(
                pid == 65535 || seen.insert(pid),
                "接收器重复报告 PID {pid}，无法区分设备身份"
            );
            Ok((pid, status))
        })
        .collect()
}

/// Minimal DUALLINK_BIND_INFO from real query rows plus source catalog identity.
/// Edition/layout/serial are omitted because this query does not return them.
pub(crate) fn pairing_payload(receiver_pid: u32, value: &Value) -> anyhow::Result<Value> {
    let capability = u16::try_from(receiver_pid)
        .ok()
        .and_then(super::receiver_capabilities::capability)
        .context("接收器查询能力未核实")?;
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for (raw_pid, status) in receiver_peers(value)? {
        if raw_pid == 65535 || raw_pid == receiver_pid || !seen.insert(raw_pid) {
            continue;
        }
        let IdentityLookup::Unique(identity) =
            device_identity::lookup_receiver_peer(raw_pid, capability.peer_match_product_id)
        else {
            anyhow::bail!("配对查询 PID {raw_pid} 无法唯一映射当前产品");
        };
        let product =
            crate::product::registered(identity.product_id).context("配对产品尚无本地页面")?;
        let categories = product.categories();
        let category = if categories.contains(&"MOUSE") {
            "MOUSE"
        } else if categories.contains(&"KEYBOARD") {
            "KEYBOARD"
        } else {
            anyhow::bail!("配对 PID {raw_pid} 的产品类别不适用于底座页面")
        };
        rows.push(
            serde_json::json!({"productId":identity.product_id,"dongleId":raw_pid,
            "status":status,"category":category,"productName":{"en":product.name()}}),
        );
    }
    Ok(Value::Array(rows))
}

pub(crate) fn discover(
    client: &mut ServiceClient,
    usb: &Result<Value, String>,
    hid: &Result<Value, String>,
) -> anyhow::Result<DiscoverySnapshot> {
    let mut snapshot = DiscoverySnapshot::default();
    let mut groups: BTreeMap<(String, u32), Vec<(&Value, DeviceConnectionObservation)>> =
        BTreeMap::new();
    // USB detection and HID transport are independent source branches. A failed
    // HID descriptor read must not erase an actual physical USB observation.
    for (label, key, result, connection) in [
        (
            "USB",
            "devices",
            usb,
            DeviceConnectionObservation::UsbPresent,
        ),
        (
            "HID",
            "interfaces",
            hid,
            DeviceConnectionObservation::HidPresent,
        ),
    ] {
        let value = match result {
            Ok(value) => value,
            Err(error) => {
                snapshot.errors.push(format!("{label} 枚举失败：{error}"));
                continue;
            }
        };
        let Some(items) = value[key].as_array() else {
            snapshot.errors.push(format!("{label} 枚举响应格式无效"));
            continue;
        };
        if value["complete"].as_bool() != Some(true) {
            snapshot
                .errors
                .push(format!("{label} 枚举未完成，未发现的设备状态保持未知"));
        }
        if let Some(failures) = value["failures"]
            .as_array()
            .filter(|items| !items.is_empty())
        {
            snapshot.errors.push(format!(
                "{label} 有 {} 项读取错误，详见连接页面",
                failures.len()
            ));
        }
        for item in items {
            if item["vendor_id"].as_u64() != Some(5426) {
                continue;
            }
            let Some(pid) = item["product_id"]
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
            else {
                continue;
            };
            let Some(container) = text(item, "device_container_id") else {
                snapshot
                    .errors
                    .push(format!("PID {pid} 缺少真实设备容器，未发布到首页"));
                continue;
            };
            groups
                .entry((container.to_ascii_lowercase(), pid))
                .or_default()
                .push((item, connection));
        }
    }
    for ((container, pid), items) in groups {
        let identity = device_identity::lookup(pid);
        if let IdentityLookup::Unique(identity) = &identity {
            let serials: BTreeSet<_> = items
                .iter()
                .filter_map(|(item, _)| text(item, "serial_number"))
                .collect();
            snapshot.insert(ObservedDevice {
                product_id: identity.product_id,
                real_product_id: pid,
                container: container.clone(),
                // A receiver's USB string is not a queried wireless peer serial.
                serial: if !identity.is_dongle && serials.len() == 1 {
                    serials.first().unwrap().to_string()
                } else {
                    String::new()
                },
                use_ble: identity.is_ble,
                transport: if identity.is_ble {
                    Some(ObservedTransport::Ble)
                } else if identity.is_dongle {
                    Some(ObservedTransport::Dongle)
                } else if identity.real_product_id == identity.source_product_id {
                    Some(ObservedTransport::Wired)
                } else {
                    None
                },
                connection: if items
                    .iter()
                    .any(|(_, connection)| *connection == DeviceConnectionObservation::UsbPresent)
                {
                    DeviceConnectionObservation::UsbPresent
                } else {
                    DeviceConnectionObservation::HidPresent
                },
            });
        } else if let IdentityLookup::Ambiguous { candidates, .. } = &identity {
            snapshot.errors.push(format!(
                "USB PID {pid} 对应 {} 个产品，尚需身份读取消除歧义",
                candidates.len()
            ));
        }
        let Some(capability) = u16::try_from(pid)
            .ok()
            .and_then(super::receiver_capabilities::capability)
        else {
            continue;
        };
        let result = hid
            .as_ref()
            .map_err(|error| anyhow::anyhow!(error.clone()))
            .and_then(|hid| receiver_request(hid, &container, pid))
            .and_then(|request| client.request(request))
            .and_then(|value| receiver_peers(&value));
        match result {
            Ok(peers) => {
                let mut seen = BTreeSet::new();
                for (raw_pid, status) in peers {
                    if raw_pid == 65535 || raw_pid == pid || !seen.insert(raw_pid) {
                        continue;
                    }
                    let mapped = match device_identity::lookup_receiver_peer(
                        raw_pid,
                        capability.peer_match_product_id,
                    ) {
                        IdentityLookup::Unique(identity) => identity.product_id,
                        IdentityLookup::Unmatched { .. } => {
                            snapshot
                                .errors
                                .push(format!("查询返回 PID {raw_pid}，官方目录无对应产品"));
                            continue;
                        }
                        IdentityLookup::Ambiguous { candidates, .. } => {
                            snapshot.errors.push(format!(
                                "查询 PID {raw_pid} 对应 {} 个产品，尚需身份读取消除歧义",
                                candidates.len()
                            ));
                            continue;
                        }
                    };
                    snapshot.insert(ObservedDevice {
                        product_id: mapped,
                        real_product_id: raw_pid,
                        container: container.clone(),
                        serial: String::new(),
                        use_ble: false,
                        transport: Some(ObservedTransport::Dongle),
                        connection: DeviceConnectionObservation::ReceiverPeer(status),
                    });
                }
            }
            Err(error) => snapshot
                .errors
                .push(format!("接收器 {container}：{error:#}")),
        }
    }
    Ok(snapshot)
}

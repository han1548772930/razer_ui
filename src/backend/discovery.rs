//! Live interface/receiver observations, separate from profile loading and drafts.
//! Current host identity routing and source-derived shared protocol selection.
use super::runtime::{ServiceClient, ServiceRequest};
use super::device_identity::{self, IdentityLookup};
use crate::model::{Device, DeviceCategory, DeviceConnectionObservation, LocalizedText, SetupStatus};
use anyhow::{Context as _, ensure};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub(crate) struct ObservedDevice {
    product_id: u32,
    real_product_id: u32,
    container: String,
    serial: String,
    connection: DeviceConnectionObservation,
}
impl ObservedDevice {
    pub(crate) fn matches(&self, device: &Device) -> bool {
        self.product_id == device.product_id
            && self.container.eq_ignore_ascii_case(&device.device_container_id)
    }
    pub(crate) fn connection(&self) -> DeviceConnectionObservation { self.connection }
    pub(crate) fn into_device(self) -> Option<Device> {
        let product = crate::product::registered(self.product_id)?;
        let categories = product.categories();
        let category = if categories.iter().any(|c| matches!(*c, "MOUSE" | "MOUSEPLUSMAT")) {
            DeviceCategory::Mouse
        } else if categories.contains(&"KEYBOARD") { DeviceCategory::Keyboard
        } else if categories.contains(&"KEYPAD") { DeviceCategory::Keypad
        } else if categories.contains(&"ACCESSORY") { DeviceCategory::Accessory
        } else if categories.contains(&"MOUSEMAT") { DeviceCategory::Mousepad
        } else if categories.iter().any(|c| c.starts_with("AUDIO")) { DeviceCategory::Audio
        } else if categories.iter().any(|c| c.starts_with("GAMEPAD")) { DeviceCategory::Controller
        } else { DeviceCategory::Other };
        // Static product naming identifies a route, never invents a hardware edition.
        let name = LocalizedText { values: BTreeMap::from([("en".into(), product.name().into())]) };
        Some(Device {
            dashboard: crate::model::DashboardDeviceMetadata {
                local_snapshot: true,
                connection_observation: Some(self.connection),
                ..Default::default()
            },
            sub_devices: None, source_device_settings: None,
            serial_number: self.serial, product_id: self.product_id,
            real_product_id: self.real_product_id, edition_id: 0, layout_id: 0,
            device_container_id: self.container, category, setup_status: SetupStatus::Unknown,
            // The feature owner may create a local draft. No service profile is supplied here.
            active_profile: String::new(), profiles: vec![], is_single_profile: false,
            is_chroma_device: false, has_battery: false, use_ble: false,
            name: name.clone(), product_name: name, ui_window_name: String::new(),
            mw_window_name: String::new(), min_dpi: None, max_dpi: None, dpi_step: None,
            support_xy_dpi: false, power_status: None, dkm_keys: vec![],
            firmware_info: Default::default(), features: Default::default(), features_initialized: false,
        })
    }
}

#[derive(Clone, Default)]
pub(crate) struct DiscoverySnapshot {
    devices: Vec<ObservedDevice>,
    errors: Vec<String>,
}
impl DiscoverySnapshot {
    pub(crate) fn devices(&self) -> &[ObservedDevice] { &self.devices }
    pub(crate) fn errors(&self) -> &[String] { &self.errors }
    fn insert(&mut self, observed: ObservedDevice) {
        if let Some(existing) = self.devices.iter_mut().find(|existing|
            existing.product_id == observed.product_id && existing.container == observed.container) {
            // A confirmed radio status conveys more than collection presence.
            // Interface enumeration must never overwrite it just due to PID order.
            if matches!(observed.connection, DeviceConnectionObservation::ReceiverPeer(_)) {
                existing.connection = observed.connection;
            }
            if existing.serial.is_empty() { existing.serial = observed.serial; }
        } else {
            self.devices.push(observed);
        }
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str().filter(|text| !text.trim().is_empty())
}

/// Requires an unambiguous current collection in the receiver's actual container.
pub(crate) fn receiver_request(hid: &Value, container: &str, product_id: u32) -> anyhow::Result<ServiceRequest> {
    let items = hid["interfaces"].as_array().context("HID 接口响应不是数组")?;
    let capability = u16::try_from(product_id).ok().and_then(super::receiver_capabilities::capability)
        .context("此产品的无线状态查询协议尚未完成当前原码核实")?;
    let paths: BTreeSet<_> = items.iter().filter(|item| {
        item["vendor_id"].as_u64() == Some(u64::from(capability.vendor_id))
            && item["product_id"].as_u64() == Some(u64::from(capability.product_id))
            && item["claim_interface"].as_i64() == Some(i64::from(capability.claim_interface))
            && text(item, "device_container_id").is_some_and(|id| id.eq_ignore_ascii_case(container))
    }).filter_map(|item| text(item, "path")).collect();
    ensure!(paths.len() == 1, "接收器 {container} 的查询接口缺失或不唯一");
    Ok(ServiceRequest::ReceiverWirelessStatus {
        path: paths.first().unwrap().to_string(), device_container_id: container.to_string(),
    })
}

pub(crate) fn receiver_peers(value: &Value) -> anyhow::Result<Vec<(u32, u8)>> {
    let devices = value["devices"].as_array().context("接收器未返回设备列表")?;
    devices.iter().map(|item| {
        let pid = item["product_id"].as_u64().and_then(|v| u32::try_from(v).ok())
            .context("接收器产品编号无效")?;
        let status = item["status"].as_u64().and_then(|v| u8::try_from(v).ok())
            .context("接收器连接状态无效")?;
        Ok((pid, status))
    }).collect()
}

pub(crate) fn discover(client: &mut ServiceClient, hid: &Value) -> anyhow::Result<DiscoverySnapshot> {
    let interfaces = hid["interfaces"].as_array().context("HID 接口响应不是数组")?;
    let mut snapshot = DiscoverySnapshot::default();
    if hid["complete"].as_bool() != Some(true) {
        snapshot.errors.push("HID 枚举未完成，未发现的设备状态保持未知".into());
    }
    let mut groups: BTreeMap<(String, u32), Vec<&Value>> = BTreeMap::new();
    for item in interfaces {
        if item["vendor_id"].as_u64() != Some(5426) { continue; }
        let Some(pid) = item["product_id"].as_u64().and_then(|v| u32::try_from(v).ok()) else { continue; };
        let Some(container) = text(item, "device_container_id") else {
            snapshot.errors.push(format!("PID {pid} 缺少真实设备容器，未发布到首页"));
            continue;
        };
        groups.entry((container.to_ascii_lowercase(), pid)).or_default().push(item);
    }
    for ((container, pid), items) in groups {
        let identity = device_identity::lookup(pid);
        if let IdentityLookup::Unique(identity) = &identity {
            let serials: BTreeSet<_> = items.iter().filter_map(|item| text(item,"serial_number")).collect();
            snapshot.insert(ObservedDevice {
                product_id: identity.product_id, real_product_id: pid, container: container.clone(),
                serial: if serials.len() == 1 { serials.first().unwrap().to_string() } else { String::new() },
                connection: DeviceConnectionObservation::HidPresent,
            });
        } else if let IdentityLookup::Ambiguous { candidates, .. } = &identity {
            snapshot.errors.push(format!("USB PID {pid} 对应 {} 个产品，尚需身份读取消除歧义", candidates.len()));
        }
        if u16::try_from(pid).ok().and_then(super::receiver_capabilities::capability).is_none() { continue; }
        let result = receiver_request(hid, &container, pid).and_then(|request| client.request(request))
            .and_then(|value| receiver_peers(&value));
        match result {
            Ok(peers) => {
                let mut seen = BTreeSet::new();
                for (raw_pid, status) in peers {
                    if raw_pid == 65535 || raw_pid == pid || !seen.insert(raw_pid) { continue; }
                    let mapped = match device_identity::lookup_receiver_peer(raw_pid) {
                        IdentityLookup::Unique(identity) => identity.product_id,
                        IdentityLookup::Unmatched { .. } => { snapshot.errors.push(format!("查询返回 PID {raw_pid}，官方目录无对应产品")); continue; }
                        IdentityLookup::Ambiguous { candidates, .. } => { snapshot.errors.push(format!("查询 PID {raw_pid} 对应 {} 个产品，尚需身份读取消除歧义", candidates.len())); continue; }
                    };
                    snapshot.insert(ObservedDevice {
                        product_id: mapped, real_product_id: raw_pid, container: container.clone(),
                        serial: String::new(), connection: DeviceConnectionObservation::ReceiverPeer(status),
                    });
                }
            }
            Err(error) => snapshot.errors.push(format!("接收器 {container}：{error:#}")),
        }
    }
    Ok(snapshot)
}

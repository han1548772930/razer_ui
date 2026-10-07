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
    physical_product_id: u32,
    peer_product_id: Option<u32>,
    read_values: Option<super::device_reads::DeviceReadValues>,
}
impl ObservedDevice {
    pub(crate) fn product_id(&self) -> u32 {
        self.product_id
    }
    pub(crate) fn read_values(&self) -> Option<&super::device_reads::DeviceReadValues> {
        self.read_values.as_ref()
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
                existing.physical_product_id = observed.physical_product_id;
                existing.peer_product_id = observed.peer_product_id;
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

/// Current host `connectHidDevice` walks matching collections until one opens.
/// Multiple collections in one container are not multiple physical receivers.
fn receiver_requests(
    hid: &Value,
    container: &str,
    product_id: u32,
) -> anyhow::Result<Vec<ServiceRequest>> {
    let items = hid["interfaces"]
        .as_array()
        .context("HID 接口响应不是数组")?;
    let capability = u16::try_from(product_id)
        .ok()
        .and_then(super::receiver_capabilities::capability)
        .context("此产品的无线状态查询协议尚未完成当前原码核实")?;
    let mut seen = BTreeSet::new();
    let paths: Vec<_> = items
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
        .filter(|path| seen.insert(path.to_ascii_lowercase()))
        .collect();
    ensure!(
        !paths.is_empty(),
        "接收器 PID {product_id} 的查询接口缺失：需要接口 {}、{} 字节 Feature；请查看 HID 枚举详情",
        capability.claim_interface,
        capability.report_bytes,
    );
    Ok(paths
        .into_iter()
        .map(|path| ServiceRequest::ReceiverWirelessStatus {
            path: path.to_string(),
            device_container_id: container.to_string(),
        })
        .collect())
}

/// Restrict every attempt to a freshly observed collection of the same source
/// capability and physical container. Each worker request revalidates that exact
/// path before/after the query; failures never turn into an empty binding list.
pub(crate) fn query_receiver(
    client: &mut ServiceClient,
    hid: &Value,
    container: &str,
    product_id: u32,
) -> anyhow::Result<Value> {
    query_receiver_with(hid, container, product_id, |request| {
        client.request(request)
    })
}

fn query_receiver_with(
    hid: &Value,
    container: &str,
    product_id: u32,
    mut query: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Value> {
    let requests = receiver_requests(hid, container, product_id)?;
    let mut errors = Vec::new();
    for (index, request) in requests.into_iter().enumerate() {
        match query(request).and_then(|value| receiver_peers(&value).map(|_| value)) {
            Ok(value) => return Ok(value),
            Err(error) => errors.push(format!("候选接口 {}：{error:#}", index + 1)),
        }
    }
    anyhow::bail!("所有接收器查询接口均失败：{}", errors.join("；"))
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
                physical_product_id: pid,
                peer_product_id: None,
                read_values: None,
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
            .and_then(|hid| query_receiver(client, hid, &container, pid))
            .and_then(|value| receiver_peers(&value));
        match result {
            Ok(peers) => {
                let mut seen = BTreeSet::new();
                for (raw_pid, status) in peers {
                    // A mouse's own dongle PID identifies its wireless peer;
                    // only a standalone receiver's self row is not a product.
                    let own_dongle =
                        matches!(&identity, IdentityLookup::Unique(identity) if identity.is_dongle);
                    if raw_pid == 65535 || (raw_pid == pid && !own_dongle) || !seen.insert(raw_pid)
                    {
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
                        physical_product_id: pid,
                        peer_product_id: Some(raw_pid),
                        read_values: None,
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

/// Populate optional device telemetry only after publishing the interface/peer
/// snapshot. A slow or unavailable query must not hide an observed mouse.
pub(crate) fn read_device_values(
    client: &mut ServiceClient,
    hid: &Result<Value, String>,
    snapshot: &mut DiscoverySnapshot,
) {
    for observed in &mut snapshot.devices {
        if let Some(values) = read_observed_values(client, hid, observed) {
            for (kind, error) in &values.errors {
                snapshot.errors.push(format!(
                    "产品 {} 的 {kind} 未读取：{error}",
                    observed.product_id
                ));
            }
            observed.read_values = Some(values);
        }
    }
}

fn read_observed_values(
    client: &mut ServiceClient,
    hid: &Result<Value, String>,
    observed: &ObservedDevice,
) -> Option<super::device_reads::DeviceReadValues> {
    use super::device_reads::{self, DeviceReadTarget, DeviceReadValue, DeviceReadValues};
    let capability = device_reads::capability(observed.product_id)?;
    // A radio binding without an actual online reply is not a read target.
    if matches!(observed.transport, Some(ObservedTransport::Dongle))
        && observed.peer_product_id.is_none()
    {
        return None;
    }
    if matches!(observed.connection, DeviceConnectionObservation::ReceiverPeer(status) if status != 1)
    {
        return None;
    }
    let mut values = DeviceReadValues::default();
    let targets = (|| -> anyhow::Result<Vec<DeviceReadTarget>> {
        let hid = hid
            .as_ref()
            .map_err(|error| anyhow::anyhow!(error.clone()))?;
        let items = hid["interfaces"]
            .as_array()
            .context("缺少HID查询接口列表")?;
        let claim_interface = if observed.peer_product_id.is_some() {
            super::receiver_capabilities::capability(u16::try_from(observed.physical_product_id)?)
                .context("物理接收器查询能力未核实")?
                .claim_interface
        } else {
            capability.claim_interface
        };
        let mut paths = BTreeSet::new();
        let targets = items
            .iter()
            .filter(|item| {
                item["vendor_id"] == capability.vendor_id
                    && item["product_id"] == observed.physical_product_id
                    && item["claim_interface"] == claim_interface
                    && item["feature_report_bytes"] == capability.report_bytes
                    && text(item, "device_container_id")
                        .is_some_and(|id| id.eq_ignore_ascii_case(&observed.container))
            })
            .filter_map(|item| text(item, "path"))
            .filter(|path| paths.insert(path.to_ascii_lowercase()))
            .map(|path| DeviceReadTarget {
                product_id: observed.product_id,
                physical_product_id: observed.physical_product_id,
                peer_product_id: observed.peer_product_id,
                device_container_id: observed.container.clone(),
                path: path.into(),
            })
            .collect::<Vec<_>>();
        ensure!(!targets.is_empty(), "没有匹配实际设备身份的源查询接口");
        Ok(targets)
    })();
    for command in &capability.queries {
        let result = (|| -> anyhow::Result<()> {
            let targets = targets
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{error:#}"))?;
            let mut errors = Vec::new();
            for target in targets {
                let result = client
                    .request(ServiceRequest::DeviceRead {
                        target: target.clone(),
                        kind: command.name,
                    })
                    .and_then(|reply| {
                        ensure!(
                            reply["target"] == serde_json::to_value(target)?,
                            "设备读取响应的作用域不匹配"
                        );
                        let value: DeviceReadValue =
                            serde_json::from_value(reply["reading"].clone())?;
                        values.insert(command.name, value)
                    });
                match result {
                    Ok(()) => return Ok(()),
                    Err(error) => errors.push(format!("{error:#}")),
                }
                if client.is_stopped() {
                    break;
                }
            }
            anyhow::bail!("{}", errors.join("；"))
        })();
        if let Err(error) = result {
            values
                .errors
                .insert(command.name.name().into(), format!("{error:#}"));
        }
    }
    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Compile-only fixtures; no device, worker, DLL or test is executed in audit.
    fn receiver_collections() -> Value {
        let row = |path: &str, container: &str, interface: u8| {
            json!({
                "vendor_id": 5426, "product_id": 179, "claim_interface": interface,
                "feature_report_bytes": 91, "device_container_id": container, "path": path
            })
        };
        json!({"complete": true, "interfaces": [
            row("collection-first", "receiver-a", 0),
            row("collection-second", "RECEIVER-A", 0),
            row("other-device", "receiver-b", 0),
            row("wrong-interface", "receiver-a", 1)
        ]})
    }

    #[test]
    fn receiver_falls_back_between_collections_of_the_same_observed_device() {
        let mut visited = Vec::new();
        let result = query_receiver_with(&receiver_collections(), "receiver-a", 179, |request| {
            let ServiceRequest::ReceiverWirelessStatus { path, .. } = request else {
                panic!("unexpected request")
            };
            visited.push(path);
            if visited.len() == 1 {
                anyhow::bail!("first collection cannot open")
            }
            Ok(json!({"devices":[{"product_id":183,"status":1}]}))
        })
        .unwrap();
        assert_eq!(visited, vec!["collection-first", "collection-second"]);
        assert_eq!(receiver_peers(&result).unwrap(), vec![(183, 1)]);
        let IdentityLookup::Unique(identity) = device_identity::lookup_receiver_peer(183, false)
        else {
            panic!("current catalog must uniquely map the observed receiver PID")
        };
        assert_eq!(identity.product_id, 182);
    }

    #[test]
    fn missing_or_failed_receiver_collections_do_not_publish_an_empty_success() {
        let mut attempts = 0;
        let result = query_receiver_with(&receiver_collections(), "receiver-a", 179, |_| {
            attempts += 1;
            Ok(json!({"devices":[{"product_id":183}]}))
        });
        assert!(result.is_err());
        assert_eq!(attempts, 2);
        let result = query_receiver_with(&receiver_collections(), "absent", 179, |_| {
            panic!("must not query another container")
        });
        assert!(result.is_err());
    }
}

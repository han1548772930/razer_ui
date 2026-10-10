//! Live interface/receiver observations, separate from profile loading and drafts.
//! Current host identity routing and source-derived shared protocol selection.
use anyhow::{Context as _, ensure};
use razer_device::device_identity::{self, IdentityLookup};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_model::model::Device;
use razer_model::model::DeviceCategory;
use razer_model::model::DeviceConnectionObservation;
use razer_model::model::LocalizedText;
use razer_model::model::SetupStatus;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "discovery_startup_tests.rs"]
mod startup_tests;

#[cfg(test)]
#[path = "receiver_route_tests.rs"]
mod receiver_route_tests;

#[path = "discovery_portable.rs"]
mod portable;
#[path = "discovery_receiver_projection.rs"]
mod receiver_projection;
pub use portable::discover_portable;
pub use receiver_projection::project_hid_receiver_query;
pub use receiver_projection::{ReceiverQueryProjection, pairing_payload, project_receiver_query};

#[derive(Clone, Copy)]
pub enum ObservedTransport {
    Wired,
    Dongle,
    Ble,
}

#[derive(Clone)]
pub struct ObservedDevice {
    product_id: u32,
    real_product_id: u32,
    container: String,
    serial: String,
    use_ble: bool,
    transport: Option<ObservedTransport>,
    connection: DeviceConnectionObservation,
    physical_product_id: u32,
    peer_product_id: Option<u32>,
    read_values: Option<razer_device::device_reads::DeviceReadValues>,
    hid_node: Option<razer_device::backend::HidNode>,
}
impl ObservedDevice {
    /// Present only for an actual portable collection observation. Its scope
    /// key is a local collection identity, never a Windows ContainerId.
    pub fn hid_node(&self) -> Option<&razer_device::backend::HidNode> {
        self.hid_node.as_ref()
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn container(&self) -> &str {
        &self.container
    }
    pub fn physical_product_id(&self) -> u32 {
        self.physical_product_id
    }
    pub fn peer_product_id(&self) -> Option<u32> {
        self.peer_product_id
    }
    pub fn read_values(&self) -> Option<&razer_device::device_reads::DeviceReadValues> {
        self.read_values.as_ref()
    }
    pub fn matches(&self, device: &Device) -> bool {
        self.product_id == device.product_id
            && self
                .container
                .eq_ignore_ascii_case(&device.device_container_id)
    }
    pub fn connection(&self) -> DeviceConnectionObservation {
        self.connection
    }
    pub fn transport(&self) -> Option<ObservedTransport> {
        self.transport
    }
    pub fn into_device(self) -> Option<Device> {
        let product = razer_catalog::registered(self.product_id)?;
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
            dashboard: razer_model::model::DashboardDeviceMetadata {
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
pub struct DiscoverySnapshot {
    devices: Vec<ObservedDevice>,
    errors: Vec<String>,
}
impl DiscoverySnapshot {
    pub fn devices(&self) -> &[ObservedDevice] {
        &self.devices
    }
    pub fn errors(&self) -> &[String] {
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
                existing.hid_node = observed.hid_node.clone();
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

/// Source-compatible HID interface selection.
///
/// The original `rzHidDevices.connectHidDevice` filters the enumerated list by
/// `productId`+`vendorId`+`deviceContainerId` and then requires
/// `interface === claimInterface`; it never compares the feature report length.
/// Keep exactly that as the selection rule, and only *prefer* collections whose
/// observed feature length equals the source capability so a device that exposes
/// several matching collections keeps a deterministic order.
fn select_interface_paths<'a>(
    items: &'a [Value],
    vendor_id: u16,
    product_id: u32,
    claim_interface: u8,
    expected_feature_bytes: usize,
    container: &str,
) -> Vec<&'a str> {
    let mut matching: Vec<(&str, Option<u64>)> = Vec::new();
    let mut seen = BTreeSet::new();
    for item in items {
        if item["vendor_id"].as_u64() != Some(u64::from(vendor_id))
            || item["product_id"].as_u64() != Some(u64::from(product_id))
            || item["claim_interface"].as_i64() != Some(i64::from(claim_interface))
            || !text(item, "device_container_id")
                .is_some_and(|id| id.eq_ignore_ascii_case(container))
        {
            continue;
        }
        let Some(path) = text(item, "path") else {
            continue;
        };
        if !seen.insert(path.to_ascii_lowercase()) {
            continue;
        }
        matching.push((path, item["feature_report_bytes"].as_u64()));
    }
    let expected = expected_feature_bytes as u64;
    let (mut preferred, mut rest): (Vec<_>, Vec<_>) = matching
        .into_iter()
        .partition(|(_, bytes)| *bytes == Some(expected));
    preferred.append(&mut rest);
    preferred.into_iter().map(|(path, _)| path).collect()
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
        .and_then(razer_device::receiver_capabilities::capability)
        .context("此产品的无线状态查询协议尚未完成当前原码核实")?;
    let paths = select_interface_paths(
        items,
        capability.vendor_id,
        u32::from(capability.product_id),
        capability.claim_interface,
        capability.report_bytes,
        container,
    );
    ensure!(
        !paths.is_empty(),
        "接收器 PID {product_id} 的查询接口缺失：需要接口 {}（源不再比对 Feature 长度）；请查看 HID 枚举详情",
        capability.claim_interface,
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
pub fn query_receiver(
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

/// The startup caller retries a successful but not-yet-online first peer with
/// fresh transactions. Protocol Busy retries inside the worker do not cover
/// this state. Run only on the existing background discovery bridge.
fn startup_receiver_query_with(
    product_id: u32,
    mut query: impl FnMut() -> anyhow::Result<Value>,
    mut wait: impl FnMut(std::time::Duration),
) -> anyhow::Result<Value> {
    let mut value = query()?;
    let Some(policy) = u16::try_from(product_id)
        .ok()
        .and_then(razer_device::receiver_capabilities::capability)
        .and_then(|capability| capability.startup_retry.as_ref())
    else {
        return Ok(value);
    };
    for &delay_ms in &policy.delays_ms {
        // The source he() removes sentinel and receiver-self rows before Ne()
        // checks its first peer. Unknown status and an empty list are not 0.
        let first = receiver_peers(&value)?
            .into_iter()
            .find(|(pid, _)| *pid != 65535 && *pid != product_id);
        if !first.is_some_and(|(_, status)| status == policy.first_peer_status) {
            break;
        }
        wait(std::time::Duration::from_millis(delay_ms));
        // Errors end this bounded observation. The source's independent 1s
        // error loop still needs owner cancellation before it can be ported.
        value = query()?;
    }
    Ok(value)
}

pub use razer_device::receiver_protocol::receiver_peers;

pub fn discover(
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
                hid_node: None,
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
        if u16::try_from(pid)
            .ok()
            .and_then(razer_device::receiver_capabilities::capability)
            .is_none()
        {
            continue;
        }
        let result = hid
            .as_ref()
            .map_err(|error| anyhow::anyhow!(error.clone()))
            .and_then(|hid| {
                startup_receiver_query_with(
                    pid,
                    || query_receiver(client, hid, &container, pid),
                    std::thread::sleep,
                )
            })
            .and_then(|value| project_receiver_query(pid, &container, &value));
        match result {
            Ok(projection) => {
                let peers = projection.into_snapshot();
                snapshot.errors.extend(peers.errors);
                for observed in peers.devices {
                    snapshot.insert(observed);
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
pub fn read_device_values(
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
) -> Option<razer_device::device_reads::DeviceReadValues> {
    use razer_device::device_reads::{self, DeviceReadTarget, DeviceReadValue, DeviceReadValues};
    let capability = device_reads::capability(observed.product_id)?;
    if observed.hid_node.is_some() {
        return Some(super::direct::read_values(client, observed, capability));
    }
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
            razer_device::receiver_capabilities::capability(u16::try_from(
                observed.physical_product_id,
            )?)
            .context("物理接收器查询能力未核实")?
            .claim_interface
        } else {
            capability.claim_interface_for(observed.physical_product_id)?
        };
        let paths = select_interface_paths(
            items,
            capability.vendor_id,
            observed.physical_product_id,
            claim_interface,
            capability.report_bytes,
            &observed.container,
        );
        let targets = paths
            .into_iter()
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

    #[test]
    fn pairing_names_use_source_peer_catalog_without_inventing_metadata() {
        let payload =
            pairing_payload(179, &json!({"devices":[{"product_id":183,"status":1}]})).unwrap();
        let peer = &payload[0];
        assert_eq!(peer["productId"], 182);
        assert_eq!(peer["dongleId"], 183);
        assert_eq!(peer["category"], "MOUSE");
        assert_eq!(peer["productName"]["en"], "RAZER DEATHADDER V3 PRO");
        assert_eq!(peer["productName"]["zh-cn"], "RAZER炼狱蝰蛇 V3 专业版");
        for key in ["editionId", "layoutId", "serialNumber", "setupStatus"] {
            assert!(
                peer.get(key).is_none(),
                "{key} is not returned by this query"
            );
        }
    }
}

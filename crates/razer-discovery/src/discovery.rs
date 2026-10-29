//! Live interface/receiver observations, separate from profile loading and drafts.
//! Current host identity routing and source-derived shared protocol selection.
use anyhow::{Context as _, ensure};
use razer_device::device_identity::{self, IdentityLookup};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_model::model::Device;
use razer_model::model::DeviceCategory;
use razer_model::model::DeviceConnectionObservation;
use razer_model::model::SetupStatus;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "discovery_startup_tests.rs"]
mod startup_tests;

#[cfg(test)]
#[path = "receiver_route_tests.rs"]
mod receiver_route_tests;

#[cfg(test)]
#[path = "direct/mouse_scroll_tests.rs"]
mod mouse_scroll_tests;

#[path = "discovery_portable.rs"]
mod portable;
#[path = "discovery_receiver_projection.rs"]
mod receiver_projection;
pub use portable::discover_portable;
pub use receiver_projection::project_hid_receiver_query;
pub use receiver_projection::{ReceiverQueryProjection, pairing_payload, project_receiver_query};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    keyboard_metadata: Option<razer_device::keyboard_metadata::KeyboardMetadataReading>,
    keyboard_serial: Option<String>,
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
    pub fn device_task_scope(&self) -> anyhow::Result<razer_ipc::device_tasks::Scope> {
        match self.hid_node() {
            Some(node) => Ok(razer_ipc::device_tasks::Scope::collection(
                self.product_id(),
                node,
            )),
            None => razer_ipc::device_tasks::Scope::container(self.product_id(), self.container()),
        }
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
    pub fn keyboard_metadata(
        &self,
    ) -> Option<&razer_device::keyboard_metadata::KeyboardMetadataReading> {
        self.keyboard_metadata.as_ref()
    }
    pub fn keyboard_serial(&self) -> Option<&str> {
        self.keyboard_serial.as_deref()
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
        let edition_id = self
            .keyboard_metadata
            .as_ref()
            .map_or(0, |reading| u32::from(reading.edition_id));
        let layout_id = self
            .keyboard_metadata
            .as_ref()
            .map_or(0, |reading| u32::from(reading.keyboard_layout));
        let (edition_id, layout_id) = if self.keyboard_metadata.is_some() {
            razer_model::source_identity::variant(self.product_id, edition_id, layout_id, 0)
        } else {
            (edition_id, layout_id)
        };
        let (name, product_name) =
            razer_model::source_names::localized_names(self.product_id, edition_id);
        let connection = self.connection;
        let read_values = self.read_values;
        let mut device = Device {
            dashboard: razer_model::model::DashboardDeviceMetadata {
                local_snapshot: true,
                connection_observation: Some(self.connection),
                edition_name: self.keyboard_metadata.as_ref().and_then(|_| {
                    razer_model::source_names::edition_name(self.product_id, edition_id)
                }),
                readonly_keyboard_metadata: self.keyboard_metadata,
                ..Default::default()
            },
            sub_devices: None,
            source_device_settings: None,
            controller_power_saving: None,
            serial_number: self.serial,
            product_id: self.product_id,
            real_product_id: self.real_product_id,
            edition_id,
            layout_id,
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
            name,
            product_name,
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
        };
        device.observe_connection(Some(connection));
        device.observe_read_values(read_values);
        Some(device)
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
            source_product_id: None,
        })
        .collect())
}

pub fn source_receiver_path(
    hid: &Value,
    container: &str,
    product_id: u32,
    source_product_id: Option<u16>,
) -> anyhow::Result<String> {
    ensure!(hid["complete"] == true, "源 factory 的物理接口枚举未完成");
    let cap = razer_device::receiver_capabilities::for_source(
        u16::try_from(product_id)?,
        source_product_id,
    )?;
    let interfaces = hid["interfaces"]
        .as_array()
        .context("缺少源 factory 接口观察")?;
    let paths = select_interface_paths(
        interfaces,
        cap.vendor_id,
        product_id,
        cap.claim_interface,
        cap.report_bytes,
        container,
    );
    let [path] = paths.as_slice() else {
        anyhow::bail!("源 factory 接口缺失或不唯一");
    };
    Ok((*path).to_owned())
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

/// Factory mode is an independent probe on the current primary linker.
pub fn query_receiver_dock_mode(
    client: &mut ServiceClient,
    hid: &Value,
    container: &str,
    product_id: u32,
) -> anyhow::Result<bool> {
    razer_device::receiver_dock_mode::capability(u16::try_from(product_id)?)?;
    let requests = receiver_requests(hid, container, product_id)?;
    let mut failures = Vec::new();
    for request in requests {
        let ServiceRequest::ReceiverWirelessStatus {
            path,
            device_container_id,
            ..
        } = request
        else {
            unreachable!()
        };
        let reply = client
            .request(ServiceRequest::ReceiverDockModeRead {
                path: path.clone(),
                device_container_id: device_container_id.clone(),
            })
            .and_then(|reply| {
                ensure!(
                    reply["path"] == path
                        && reply["device_container_id"] == device_container_id
                        && reply["product_id"] == product_id
                        && reply["method"] == "getDockExtendedFirmwareVersion"
                        && reply["is_dongle"] == true,
                    "Dock mode reply identity or method differs"
                );
                Ok(true)
            });
        match reply {
            Ok(mode) => return Ok(mode),
            Err(error) => failures.push(format!("{error:#}")),
        }
    }
    anyhow::bail!("Dock mode query failed: {}", failures.join("; "))
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
                keyboard_metadata: None,
                keyboard_serial: None,
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
        observed.keyboard_metadata = None;
        observed.keyboard_serial = None;
        observed.read_values = None;
    }
    for observed in &mut snapshot.devices {
        if client.is_stopped() {
            break;
        }
        let has_metadata = razer_device::keyboard_metadata::hid_capability(
            observed.product_id,
            observed.physical_product_id,
        )
        .is_some()
            || observed.peer_product_id.is_some()
                && razer_device::keyboard_metadata::relay_capability(
                    observed.product_id,
                    observed.physical_product_id,
                )
                .is_ok()
            || razer_device::keyboard_metadata::ble_product_id(observed.product_id)
                == Some(observed.physical_product_id);
        if has_metadata {
            match super::direct::keyboard_metadata::resolve(client, observed) {
                Ok(route) => {
                    match route.read_metadata(client) {
                        Ok(reading) => observed.keyboard_metadata = Some(reading),
                        Err(error) => snapshot.errors.push(format!(
                            "产品 {} 的键盘布局与版本未读取：{error:#}",
                            observed.product_id
                        )),
                    }
                    if client.is_stopped() {
                        break;
                    }
                    match route.read_serial(client) {
                        Ok(serial) => observed.keyboard_serial = Some(serial),
                        Err(error) => snapshot.errors.push(format!(
                            "产品 {} 的键盘序列号未读取：{error:#}",
                            observed.product_id
                        )),
                    }
                }
                Err(error) => snapshot.errors.push(format!(
                    "产品 {} 的键盘身份读取路径未确认：{error:#}",
                    observed.product_id
                )),
            }
        }
        if client.is_stopped() {
            break;
        }
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
    use razer_device::device_reads::{self, DeviceReadTarget, DeviceReadValues};
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
            read_candidate_values_with(targets, command.name, &mut values, |request| {
                let reply = client.request(request);
                (reply, client.is_stopped())
            })
        })();
        if let Err(error) = result {
            values
                .errors
                .insert(command.name.name().into(), format!("{error:#}"));
        }
    }
    Some(values)
}

/// Collection fallback remains observable, but repeated adapter errors are one
/// diagnostic with an attempt count rather than an unbounded repeated footer.
/// This does not discard different failures or turn a failed query into data.
fn read_candidate_values_with(
    targets: &[razer_device::device_reads::DeviceReadTarget],
    kind: razer_device::device_reads::DeviceReadKind,
    values: &mut razer_device::device_reads::DeviceReadValues,
    mut request: impl FnMut(ServiceRequest) -> (anyhow::Result<Value>, bool),
) -> anyhow::Result<()> {
    use razer_device::device_reads::DeviceReadValue;
    ensure!(!targets.is_empty(), "没有匹配实际设备身份的源查询接口");
    let mut failures: Vec<(String, usize)> = Vec::new();
    for target in targets {
        let (reply, stopped) = request(ServiceRequest::DeviceRead {
            target: target.clone(),
            kind,
        });
        let result = reply.and_then(|reply| {
            ensure!(
                reply["target"] == serde_json::to_value(target)?,
                "设备读取响应的作用域不匹配"
            );
            let value: DeviceReadValue = serde_json::from_value(reply["reading"].clone())?;
            values.insert(kind, value)
        });
        match result {
            Ok(()) => return Ok(()),
            Err(error) => {
                let message = format!("{error:#}");
                if let Some((_, count)) = failures.iter_mut().find(|(value, _)| *value == message) {
                    *count += 1;
                } else {
                    failures.push((message, 1));
                }
            }
        }
        if stopped {
            break;
        }
    }
    let messages = failures
        .into_iter()
        .map(|(message, count)| {
            if count == 1 {
                message
            } else {
                format!("{message}（{count} 个候选接口返回相同错误）")
            }
        })
        .collect::<Vec<_>>();
    anyhow::bail!("{}", messages.join("；"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Pure fixtures; no device, worker process or DLL is executed by these tests.
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
        assert_eq!(peer["editionId"], 0);
        assert_eq!(peer["layoutId"], 0);
        for key in ["serialNumber", "setupStatus"] {
            assert!(
                peer.get(key).is_none(),
                "{key} is not returned by this query"
            );
        }
    }

    #[test]
    fn current_dpi_ble_availability_keeps_feature_only_settings_on_their_own_routes() {
        let mut observed = ObservedDevice {
            product_id: 226,
            real_product_id: 228,
            container: "{01234567-89AB-CDEF-0123-456789ABCDEF}".into(),
            serial: String::new(),
            use_ble: true,
            transport: Some(ObservedTransport::Ble),
            connection: DeviceConnectionObservation::HidPresent,
            physical_product_id: 228,
            peer_product_id: None,
            read_values: None,
            keyboard_metadata: None,
            keyboard_serial: None,
            hid_node: None,
        };
        use crate::direct;
        assert_eq!(
            direct::mouse_current_dpi::available(&observed),
            cfg!(windows)
        );
        assert_eq!(direct::mouse_rotation::available(&observed), cfg!(windows));
        assert!(!direct::mouse_current_polling::available(&observed));
        assert!(!direct::mouse_brightness::available(&observed));
        assert!(!direct::mouse_dynamic::available(&observed));
        assert!(!direct::mouse_tracking::available(&observed));
        assert!(!direct::mouse_scroll::available(&observed));
        observed.physical_product_id = 227;
        assert!(!direct::mouse_current_dpi::available(&observed));
        observed.physical_product_id = 228;
        observed.peer_product_id = Some(227);
        assert!(!direct::mouse_current_dpi::available(&observed));
        observed.peer_product_id = None;
        observed.product_id = 190;
        assert!(!direct::mouse_current_dpi::available(&observed));
    }

    #[test]
    fn discovered_devices_have_localized_source_names_without_preview_metadata() {
        let observed = ObservedDevice {
            product_id: 182,
            real_product_id: 183,
            container: "observed-container".into(),
            serial: "observed-serial".into(),
            use_ble: false,
            transport: Some(ObservedTransport::Dongle),
            connection: DeviceConnectionObservation::ReceiverPeer(1),
            physical_product_id: 179,
            peer_product_id: Some(183),
            read_values: None,
            keyboard_metadata: None,
            keyboard_serial: None,
            hid_node: None,
        };
        let device = observed.into_device().unwrap();
        assert_eq!(
            device.display_name_for_locale("zh-CN"),
            "Razer 炼狱蝰蛇 V3专业版"
        );
        assert_eq!(
            device.display_name_for_locale("en"),
            "Razer Deathadder V3 Pro"
        );
        assert_eq!(device.serial_number, "observed-serial");
        assert_eq!(device.device_container_id, "observed-container");
        assert_eq!(device.edition_id, 0);
        assert_eq!(device.layout_id, 0);
        assert!(device.dashboard.edition_name.is_none());
        assert!(device.profiles.is_empty());
        assert!(device.dashboard.readonly_values.is_none());
        assert!(!device.features_initialized);
        assert_eq!(device.setup_status, SetupStatus::Waiting);
    }

    #[test]
    fn startup_state_receiver_firmware_reaches_the_model_without_mouse_telemetry() {
        use razer_device::device_reads::{
            DeviceReadKind, DeviceReadTarget, DeviceReadValue, DeviceReadValues,
        };
        let target = DeviceReadTarget {
            product_id: 179,
            physical_product_id: 179,
            peer_product_id: None,
            device_container_id: "receiver-a".into(),
            path: "receiver-collection".into(),
        };
        let cap = razer_device::device_reads::capability(179).unwrap();
        let mut values = DeviceReadValues::default();
        assert_eq!(cap.queries.len(), 1);
        for command in &cap.queries {
            read_candidate_values_with(std::slice::from_ref(&target), command.name, &mut values, |request| {
                assert!(matches!(request, ServiceRequest::DeviceRead { target: ref requested, kind: DeviceReadKind::Firmware }
                    if serde_json::to_value(requested).unwrap() == serde_json::to_value(&target).unwrap()));
                (Ok(json!({"target": target, "reading": DeviceReadValue::Firmware { version: "1.6.2.0".into() }})), false)
            }).unwrap();
        }
        let observed = ObservedDevice {
            product_id: 179,
            real_product_id: 179,
            container: target.device_container_id,
            serial: String::new(),
            use_ble: false,
            transport: Some(ObservedTransport::Wired),
            connection: DeviceConnectionObservation::UsbPresent,
            physical_product_id: 179,
            peer_product_id: None,
            read_values: Some(values),
            keyboard_metadata: None,
            keyboard_serial: None,
            hid_node: None,
        };
        let mut device = observed.into_device().unwrap();
        assert_eq!(device.setup_status, SetupStatus::Waiting);
        device.mount_local_runtime();
        assert_eq!(device.setup_status, SetupStatus::Ready);
        assert_eq!(device.current_firmware_version(), "1.6.2.0");
        assert!(device.current_power_status().is_none());
        assert!(device.profiles.is_empty());
    }

    fn reading_targets() -> Vec<razer_device::device_reads::DeviceReadTarget> {
        (0..3)
            .map(|index| razer_device::device_reads::DeviceReadTarget {
                product_id: 182,
                physical_product_id: 179,
                peer_product_id: Some(183),
                device_container_id: "receiver-a".into(),
                path: format!("collection-{index}"),
            })
            .collect()
    }

    #[test]
    fn collection_read_errors_group_duplicates_without_losing_distinct_failures() {
        use razer_device::device_reads::{DeviceReadKind, DeviceReadValues};
        let mut values = DeviceReadValues::default();
        let mut visited = Vec::new();
        let error = read_candidate_values_with(
            &reading_targets(),
            DeviceReadKind::Polling,
            &mut values,
            |request| {
                let ServiceRequest::DeviceRead { target, .. } = request else {
                    panic!("unexpected request")
                };
                visited.push(target.path);
                let message = if visited.len() == 2 {
                    "different failure"
                } else {
                    "same failure"
                };
                (Err(anyhow::anyhow!(message)), false)
            },
        )
        .unwrap_err()
        .to_string();
        assert_eq!(visited, ["collection-0", "collection-1", "collection-2"]);
        assert_eq!(error.matches("same failure").count(), 1);
        assert!(error.contains("2 个候选接口返回相同错误"));
        assert!(error.contains("different failure"));
        assert!(values.polling_hz.is_none());
    }

    #[test]
    fn collection_read_rejects_mismatched_replies_before_accepting_the_right_target() {
        use razer_device::device_reads::{DeviceReadKind, DeviceReadValue, DeviceReadValues};
        let mut values = DeviceReadValues::default();
        let mut attempts = 0;
        read_candidate_values_with(
            &reading_targets(),
            DeviceReadKind::Polling,
            &mut values,
            |request| {
                let ServiceRequest::DeviceRead { mut target, kind } = request else {
                    panic!("unexpected request")
                };
                assert_eq!(kind, DeviceReadKind::Polling);
                attempts += 1;
                let reading = match attempts {
                    1 => {
                        target.device_container_id = "other receiver".into();
                        DeviceReadValue::Polling { hz: 8000 }
                    }
                    2 => DeviceReadValue::Battery { percent: 100 },
                    _ => DeviceReadValue::Polling { hz: 1000 },
                };
                (Ok(json!({"target": target, "reading": reading})), false)
            },
        )
        .unwrap();
        assert_eq!(attempts, 3);
        assert_eq!(values.polling_hz, Some(1000));
        assert!(values.battery_percent.is_none());
    }

    #[test]
    fn cancelled_collection_read_stops_fallback_without_publishing_a_value() {
        use razer_device::device_reads::{DeviceReadKind, DeviceReadValues};
        let mut values = DeviceReadValues::default();
        let mut attempts = 0;
        let error = read_candidate_values_with(
            &reading_targets(),
            DeviceReadKind::Polling,
            &mut values,
            |_| {
                attempts += 1;
                (Err(anyhow::anyhow!("cancelled")), true)
            },
        )
        .unwrap_err();
        assert_eq!(attempts, 1);
        assert_eq!(error.to_string(), "cancelled");
        assert!(values.polling_hz.is_none());
    }
}

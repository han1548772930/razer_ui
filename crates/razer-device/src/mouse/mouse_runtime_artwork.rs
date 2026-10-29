//! Current mouse runtimeData artwork producers. Configuration literals, real
//! queries and source failure defaults retain distinct origins. No disk drafts.
use crate::{
    backend::HidNode,
    device_reads::{DeviceReadCapability, DeviceReadTarget},
    receiver_pairing::HardwareEvents,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, time::Duration};

pub const SIDEPAD_COMMAND: [u8; 3] = [1, 0, 185];
pub const WIRELESS_COMMAND: [u8; 3] = [1, 0, 184];
pub const BLE_SIDEPAD_COMMAND: [u8; 3] = [0, 1, 145];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Collection { node: HidNode, product_id: u32 },
    WindowsContainer(DeviceReadTarget),
    WindowsBle(WindowsBleTarget),
}
impl Target {
    pub fn product_id(&self) -> u32 {
        match self {
            Self::Collection { product_id, .. } => *product_id,
            Self::WindowsContainer(t) => t.product_id,
            Self::WindowsBle(t) => t.product_id,
        }
    }
    pub fn physical_product_id(&self) -> u32 {
        match self {
            Self::Collection { node, .. } => u32::from(node.product_id),
            Self::WindowsContainer(t) => t.physical_product_id,
            Self::WindowsBle(t) => t.physical_product_id,
        }
    }
}

/// Current noble/util.hidDetectDeviceInfoParser binds an installed BLE HID
/// endpoint's immediate BTHLEDevice parent to the Bluetooth address. It never
/// substitutes a USB interface, advertising name or cached online state.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct WindowsBleTarget {
    pub product_id: u32,
    pub physical_product_id: u32,
    pub device_container_id: String,
    pub hid_path: String,
    pub device_instance_id: String,
    pub parent_instance_id: String,
    pub mac_address: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BleIdentity {
    vendor: u16,
    product: u16,
    address: u64,
}
impl BleIdentity {
    pub fn vendor_id(self) -> u16 {
        self.vendor
    }
    pub fn product_id(self) -> u16 {
        self.product
    }
    pub fn address(self) -> u64 {
        self.address
    }
}

pub fn ble_hid_identity(parent: &str) -> Option<BleIdentity> {
    let normalized = parent.to_ascii_lowercase();
    if !normalized.contains("bthledevice") {
        return None;
    }
    let vendor_at = normalized.find("_vid&")? + 7;
    let product_at = normalized.find("_pid&")? + 5;
    let vendor = u16::from_str_radix(normalized.get(vendor_at..vendor_at + 4)?, 16).ok()?;
    let product = u16::from_str_radix(normalized.get(product_at..product_at + 4)?, 16).ok()?;
    if ![0x1532, 0x068e].contains(&vendor) || product == 0 {
        return None;
    }
    let revision = normalized.get(normalized.find("_rev&")?..)?;
    let address = revision.get(revision.rfind('_')? + 1..)?.get(..12)?;
    if !address.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let address = u64::from_str_radix(address, 16).ok()?;
    if address == 0 {
        return None;
    }
    Some(BleIdentity {
        vendor,
        product,
        address,
    })
}

pub fn ble_target_from_hid(
    product_id: u32,
    physical_product_id: u32,
    container: &str,
    observation: &serde_json::Value,
) -> anyhow::Result<WindowsBleTarget> {
    let source = source(product_id).context("Current mouse BLE source is absent")?;
    ensure!(
        source.is_ble(physical_product_id),
        "Current mouse source has no such BLE identity"
    );
    WindowsBleTarget::from_hid(product_id, physical_product_id, container, observation)
}

impl WindowsBleTarget {
    /// Resolve only the installed HID parent/address. Each caller first
    /// validates its own current source's product-to-BLE-PID relationship.
    pub fn from_hid(
        product_id: u32,
        physical_product_id: u32,
        container: &str,
        observation: &serde_json::Value,
    ) -> anyhow::Result<Self> {
        ensure!(
            observation["complete"] == true,
            "BLE HID identity enumeration is incomplete"
        );
        let mut matches = Vec::new();
        for endpoint in observation["interfaces"]
            .as_array()
            .context("BLE HID interfaces are absent")?
        {
            if !endpoint["device_container_id"]
                .as_str()
                .is_some_and(|id| id.eq_ignore_ascii_case(container))
            {
                continue;
            }
            let Some(parent) = endpoint["parent_instance_id"].as_str() else {
                continue;
            };
            let Some(identity) = ble_hid_identity(parent) else {
                continue;
            };
            if u32::from(identity.product) != physical_product_id {
                continue;
            }
            ensure!(
                identity.vendor == 0x1532
                    && endpoint["vendor_id"].as_u64() == Some(u64::from(identity.vendor))
                    && endpoint["product_id"].as_u64() == Some(u64::from(identity.product)),
                "BLE HID attributes differ from the current mouse parent identity"
            );
            let path = endpoint["path"]
                .as_str()
                .context("BLE HID endpoint path is absent")?;
            let instance = endpoint["device_instance_id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .context("BLE HID instance identity is absent")?;
            ensure!(
                path.is_ascii()
                    && !path.contains('\0')
                    && path.to_ascii_lowercase().starts_with(r"\\?\hid#"),
                "BLE HID endpoint path is invalid"
            );
            matches.push((identity, parent, path, instance));
        }
        matches.sort_by_key(|(_, _, path, _)| path.to_ascii_lowercase());
        let (identity, parent, path, instance) = matches
            .first()
            .context("No actual installed BLE HID endpoint matches the mouse owner")?;
        ensure!(
            matches
                .iter()
                .all(|(next, next_parent, _, _)| next == identity
                    && next_parent.eq_ignore_ascii_case(parent)),
            "BLE mouse physical ContainerId has ambiguous parent/address identities"
        );
        ensure!(
            matches
                .windows(2)
                .all(|items| !items[0].2.eq_ignore_ascii_case(items[1].2)),
            "BLE HID collection path is duplicated"
        );
        Ok(Self {
            product_id,
            physical_product_id,
            device_container_id: container.into(),
            hid_path: (*path).into(),
            device_instance_id: (*instance).into(),
            parent_instance_id: (*parent).into(),
            mac_address: identity.address,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Source {
    product_id: u32,
    dongle_id: Option<u32>,
    ble_id: Option<u32>,
    interface: u8,
    fixed: Option<u8>,
    combo: bool,
    dual: bool,
}
impl Source {
    pub fn product_id(self) -> u32 {
        self.product_id
    }
    pub fn is_ble(self, physical: u32) -> bool {
        self.ble_id == Some(physical)
    }
    pub fn is_combo(self) -> bool {
        self.combo
    }
    pub fn supports_relay(self) -> bool {
        self.dual
    }
    pub fn fixed_status(self) -> Option<u8> {
        self.fixed
    }
    pub fn accepts(self, physical: u32) -> bool {
        physical == self.product_id
            || self.dongle_id == Some(physical)
            || self.ble_id == Some(physical)
    }
    pub fn interface(self) -> u8 {
        self.interface
    }
    pub fn transport(self, physical: u32, relay: bool) -> anyhow::Result<DeviceReadCapability> {
        ensure!(
            !self.is_ble(physical),
            "BLE artwork queries require the Protocol30 transport"
        );
        ensure!(
            !relay || self.dual,
            "Current mouse source has no DualLink relay artwork branch"
        );
        ensure!(
            relay || self.accepts(physical),
            "Physical artwork source differs"
        );
        if self.dual {
            // The sidepad query uses the same mounted physical factory as
            // device reads. Mouse radio senders retain prefix zero and omit
            // reportId, so the source host uses the actual master's interface.
            let source = crate::device_reads::capability(self.product_id)
                .context("Current artwork physical factory is absent")?;
            return if relay {
                let master = crate::receiver_capabilities::capability(u16::try_from(physical)?)
                    .context("Current artwork receiver factory is absent")?;
                source.for_duallink_master(physical, master.claim_interface)
            } else {
                source.for_physical_product(physical)
            };
        }
        Ok(DeviceReadCapability {
            product_id: self.product_id,
            source_class: "rzDevice25".into(),
            direct_pids: vec![self.product_id, physical],
            vendor_id: 0x1532,
            claim_interface: self.interface,
            claim_interfaces_by_pid: BTreeMap::new(),
            physical_transports_by_pid: Default::default(),
            report_id: 0,
            report_bytes: 91,
            transaction_prefix: 0,
            transaction_modulus: 31,
            queries: vec![],
            polling_codes: BTreeMap::new(),
            polling_value_offset: None,
            polling_default_hz: None,
            polling_physical_product_id: None,
            charging_codes: BTreeMap::new(),
            max_retry_in: 10,
            max_retry_out: if self.combo { 30 } else { 20 },
            sleep_between_out_ms: if self.combo { 10 } else { 5 },
            sleep_between_out_in_ms: if self.combo { 10 } else { 5 },
            sleep_between_in_ms: if self.combo { 10 } else { 5 },
        })
    }
    fn side_name(self, status: u8) -> &'static str {
        match (self.product_id, status) {
            (80, 1) | (103 | 147, 2) => "7ButtonSide",
            (83 | 141 | 150 | 180, 1) | (103 | 143 | 147 | 167 | 231, 3) => "12ButtonSide",
            (103 | 143 | 147 | 167 | 231, 1) => "2ButtonSide",
            (143 | 167 | 231, 2 | 4) => "6ButtonSide",
            _ => "",
        }
    }
}

pub fn source(product_id: u32) -> Option<Source> {
    let (product_id, dongle_id, ble_id, interface, fixed, combo, dual) = match product_id {
        80 | 83 | 141 => (product_id, None, None, 0, Some(1), false, false),
        150 => (150, None, None, 3, Some(1), false, false),
        180 => (180, Some(180), Some(181), 0, Some(1), false, true),
        103 | 147 => (product_id, None, None, 0, None, false, false),
        143 => (143, Some(144), Some(146), 0, None, false, true),
        167 => (167, Some(168), Some(169), 0, None, false, true),
        231 => (231, Some(232), Some(233), 3, None, false, true),
        104 | 105 => (104, Some(105), None, 0, None, true, false),
        _ => return None,
    };
    Some(Source {
        product_id,
        dongle_id,
        ble_id,
        interface,
        fixed,
        combo,
        dual,
    })
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    SourceConfiguration,
    InitialQuery,
    HardwareNotification,
    SourceFailureDefault,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Update {
    Sidepad {
        origin: Origin,
        status: Option<u8>,
        side_panel_name: String,
        panel_module_id: u8,
    },
    Combo {
        origin: Origin,
        mat_plugged: Option<bool>,
        mouse_connected: Option<bool>,
    },
    Diagnostic {
        message: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Query {
    Sidepad,
    Wireless,
}
pub trait Transport {
    fn query(
        &mut self,
        query: Query,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u8>;
}

pub fn query_hid(
    device: &dyn crate::backend::FeatureTransport,
    cap: &DeviceReadCapability,
    transaction: u8,
    query: Query,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<u8> {
    let data = crate::feature_exchange::exchange(
        device,
        cap,
        match query {
            Query::Sidepad => SIDEPAD_COMMAND,
            Query::Wireless => WIRELESS_COMMAND,
        },
        &[],
        transaction,
        validate,
    )?;
    data.first()
        .copied()
        .context("Mouse artwork query has no actual status byte")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Sidepad(u8),
    Wireless {
        connected: bool,
        product_id: Option<u16>,
    },
}

/// Protocol30 strips exactly one native callback byte; Protocol25 does not.
pub fn parse_event(raw: &[u8], ble: bool) -> anyhow::Result<Option<Event>> {
    let data = if ble {
        raw.get(1..)
            .context("Protocol30 event has no prefix byte")?
    } else {
        raw
    };
    if !matches!(data.first(), Some(5 | 9)) {
        return Ok(None);
    }
    let event = *data.get(1).context("Mouse notification has no event ID")?;
    match event {
        14 => Ok(Some(Event::Sidepad(
            *data.get(2).context("Sidepad event has no module value")?,
        ))),
        // Protocol30 V2 uses type at byte2 and the 0/1 connection enum at
        // byte3. Protocol25 V2 instead uses state at byte2 and a PID at4..6.
        53 if ble => {
            let state = *data.get(3).context("BLE wireless V2 event has no state")?;
            if !matches!(state, 0 | 1) {
                return Ok(None);
            }
            Ok(Some(Event::Wireless {
                connected: state == 1,
                product_id: None,
            }))
        }
        9 | 53 => {
            let state = *data.get(2).context("Wireless event has no state")?;
            if !matches!(state, 2 | 3) {
                return Ok(None);
            }
            let product_id = if event == 53 {
                Some(u16::from_le_bytes(
                    data.get(4..6)
                        .context("Wireless V2 event has no product ID")?
                        .try_into()?,
                ))
            } else {
                None
            };
            Ok(Some(Event::Wireless {
                connected: state == 3,
                product_id,
            }))
        }
        _ => Ok(None),
    }
}

fn sidepad(
    source: Source,
    transport: &mut dyn Transport,
    event: Option<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
    publish: &mut impl FnMut(Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let (status, origin) = if let Some(status) = source.fixed {
        (Some(status), Origin::SourceConfiguration)
    } else {
        match transport.query(Query::Sidepad, validate) {
            Ok(status) => (
                Some(status),
                if event.is_some() {
                    Origin::HardwareNotification
                } else {
                    Origin::InitialQuery
                },
            ),
            Err(error) => {
                validate()?;
                publish(Update::Diagnostic {
                    message: format!("Sidepad query failed: {error:#}"),
                })?;
                (None, Origin::SourceFailureDefault)
            }
        }
    };
    validate()?;
    let panel_module_id = event.or(status).unwrap_or(0);
    publish(Update::Sidepad {
        origin,
        status,
        side_panel_name: source.side_name(panel_module_id).into(),
        panel_module_id,
    })
}

/// The physical-device owner registers once, queries at initialization, re-reads
/// on event14, observes wireless changes and always releases event resources.
pub fn monitor(
    source: Source,
    physical: u32,
    transport: &mut dyn Transport,
    events: &mut dyn HardwareEvents,
    validate: &impl Fn() -> anyhow::Result<()>,
    publish: &mut impl FnMut(Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let result = (|| {
        validate()?;
        let registered = events.begin();
        if source.combo {
            if source.dongle_id == Some(physical) {
                publish(Update::Combo {
                    origin: Origin::SourceConfiguration,
                    mat_plugged: Some(false),
                    mouse_connected: Some(true),
                })?;
            } else {
                let (connected, origin) = match transport.query(Query::Wireless, validate) {
                    Ok(status) => (status == 1, Origin::InitialQuery),
                    Err(error) => {
                        validate()?;
                        publish(Update::Diagnostic {
                            message: format!("Hyperflux wireless query failed: {error:#}"),
                        })?;
                        (false, Origin::SourceFailureDefault)
                    }
                };
                validate()?;
                publish(Update::Combo {
                    origin,
                    mat_plugged: Some(true),
                    mouse_connected: Some(connected),
                })?;
            }
        } else {
            sidepad(source, transport, None, validate, publish)?;
        }
        registered?;
        let mut online = true;
        loop {
            validate()?;
            let received = events.receive(Duration::from_millis(50))?;
            validate()?;
            let Some(raw) = received else {
                continue;
            };
            let event = match parse_event(&raw, source.is_ble(physical)) {
                Ok(event) => event,
                Err(error) => {
                    publish(Update::Diagnostic {
                        message: format!("Mouse artwork event parse failed: {error:#}"),
                    })?;
                    continue;
                }
            };
            match event {
                Some(Event::Sidepad(value)) if online && !source.combo => {
                    sidepad(source, transport, Some(value), validate, publish)?
                }
                Some(Event::Wireless {
                    connected,
                    product_id,
                }) => {
                    if connected
                        && product_id.is_some_and(|pid| source.dongle_id != Some(u32::from(pid)))
                    {
                        continue;
                    }
                    online = connected;
                    if source.combo {
                        publish(Update::Combo {
                            origin: Origin::HardwareNotification,
                            mat_plugged: None,
                            mouse_connected: Some(connected),
                        })?;
                    }
                }
                _ => {}
            }
        }
    })();
    let cleanup = events.finish();
    match (result, cleanup) {
        (Ok(()), cleanup) => cleanup,
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "Mouse artwork event cleanup also failed: {cleanup:#}"
        ))),
    }
}

pub fn ble_query_frame(transaction: u8) -> [u8; 8] {
    [
        transaction,
        0,
        0,
        0,
        BLE_SIDEPAD_COMMAND[1],
        BLE_SIDEPAD_COMMAND[2],
        0,
        0,
    ]
}

const BLE_COMMAND_HEADER_LENGTH: usize = 8;
const BLE_COMMAND_DATA_LENGTH: usize = 20;

/// Protocol30 dispatches the header separately, then bare data fragments in
/// order. The callback returns after dispatch; GATT completion is asynchronous.
pub fn write_ble_command(
    frame: &[u8],
    validate: &dyn Fn() -> anyhow::Result<()>,
    mut write: impl FnMut(&[u8]) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    ensure!(
        frame.len() >= BLE_COMMAND_HEADER_LENGTH && frame[0] < 31,
        "Protocol30 command header/transaction differs"
    );
    let (header, data) = frame.split_at(BLE_COMMAND_HEADER_LENGTH);
    validate()?;
    write(header)?;
    for fragment in data.chunks(BLE_COMMAND_DATA_LENGTH) {
        validate()?;
        write(fragment)?;
    }
    Ok(())
}

/// Current host Protocol30 strategy receives a header notification followed by
/// bare data fragments. Byte7 is the response status; it is not a HID report.
#[derive(Debug)]
pub struct BleCommandStatus(pub u8);
impl std::fmt::Display for BleCommandStatus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Protocol30 command response status {}", self.0)
    }
}
impl std::error::Error for BleCommandStatus {}
impl BleCommandStatus {
    pub fn retryable(&self) -> bool {
        matches!(self.0, 1 | 3)
    }
}

pub struct BleResponse {
    transaction: u8,
    header: Option<Vec<u8>>,
    remaining: usize,
    data: Vec<u8>,
}
impl BleResponse {
    pub fn new(transaction: u8) -> Self {
        Self {
            transaction,
            header: None,
            remaining: 0,
            data: vec![],
        }
    }
    pub fn push(&mut self, packet: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        if self.header.is_none() {
            if packet.first() != Some(&self.transaction) {
                return Ok(None);
            }
            ensure!(
                packet.len() >= BLE_COMMAND_HEADER_LENGTH,
                "Protocol30 response header is truncated"
            );
            if packet[7] != 2 {
                return Err(BleCommandStatus(packet[7]).into());
            }
            self.remaining =
                usize::from(packet[1]) | usize::from(packet[2]) << 8 | usize::from(packet[3]) << 16;
            ensure!(
                self.remaining <= 65535,
                "Protocol30 response exceeds its bounded query"
            );
            self.header = Some(packet.to_vec());
        } else {
            let count = self.remaining.min(packet.len());
            self.data.extend_from_slice(&packet[..count]);
            self.remaining -= count;
        }
        if self.remaining != 0 {
            return Ok(None);
        }
        let mut full = self.header.take().unwrap();
        full.extend_from_slice(&self.data);
        // rzDevice30 first uses its 20-byte header boundary, then retries the
        // actual eight-byte boundary when the declared length does not match.
        let length = usize::from(full[1]) | usize::from(full[2]) << 8 | usize::from(full[3]) << 16;
        let offset = if length > 0 && full.len().saturating_sub(20) != length {
            BLE_COMMAND_HEADER_LENGTH
        } else {
            20.min(full.len())
        };
        let payload = full[offset..].to_vec();
        ensure!(
            length == 0 || payload.len() == length,
            "Protocol30 declared payload length differs"
        );
        Ok(Some(payload))
    }
}

#[cfg(test)]
#[path = "mouse_runtime_artwork_ble_tests.rs"]
mod ble_transfer_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, collections::VecDeque};
    struct Script {
        reads: VecDeque<anyhow::Result<u8>>,
        calls: usize,
    }
    impl Transport for Script {
        fn query(
            &mut self,
            _: Query,
            validate: &dyn Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<u8> {
            validate()?;
            self.calls += 1;
            self.reads.pop_front().unwrap()
        }
    }
    struct Events {
        values: VecDeque<Vec<u8>>,
        done: bool,
    }
    impl HardwareEvents for Events {
        fn begin(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        fn receive(&mut self, _: Duration) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(self.values.pop_front())
        }
        fn finish(&mut self) -> anyhow::Result<()> {
            self.done = true;
            Ok(())
        }
    }
    #[test]
    fn event14_requeries_and_keeps_query_status_distinct_from_event_side_name() {
        let mut transport = Script {
            reads: [Ok(1), Err(anyhow::anyhow!("failed read"))].into(),
            calls: 0,
        };
        let mut events = Events {
            values: [vec![5, 14, 4]].into(),
            done: false,
        };
        let stop = Cell::new(false);
        let mut updates = vec![];
        let result = monitor(
            source(231).unwrap(),
            231,
            &mut transport,
            &mut events,
            &|| {
                ensure!(!stop.get(), "canceled");
                Ok(())
            },
            &mut |update| {
                if matches!(
                    update,
                    Update::Sidepad {
                        panel_module_id: 4,
                        ..
                    }
                ) {
                    stop.set(true);
                }
                updates.push(update);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(events.done);
        assert_eq!(transport.calls, 2);
        assert!(
            matches!(&updates[0],Update::Sidepad { status:Some(1), side_panel_name, origin:Origin::InitialQuery, .. } if side_panel_name=="2ButtonSide")
        );
        assert!(matches!(&updates[1], Update::Diagnostic { .. }));
        assert!(
            matches!(&updates[2],Update::Sidepad { status:None, side_panel_name, panel_module_id:4, origin:Origin::SourceFailureDefault } if side_panel_name=="6ButtonSide")
        );
    }
    #[test]
    fn literal_sidepanel_and_combo_defaults_never_claim_a_query() {
        for (pid, physical) in [(80, 80), (180, 181), (104, 105)] {
            let mut transport = Script {
                reads: [].into(),
                calls: 0,
            };
            let mut events = Events {
                values: [].into(),
                done: false,
            };
            let stop = Cell::new(false);
            let mut updates = vec![];
            assert!(
                monitor(
                    source(pid).unwrap(),
                    physical,
                    &mut transport,
                    &mut events,
                    &|| {
                        ensure!(!stop.get(), "canceled");
                        Ok(())
                    },
                    &mut |update| {
                        stop.set(true);
                        updates.push(update);
                        Ok(())
                    }
                )
                .is_err()
            );
            assert_eq!(transport.calls, 0);
            assert!(events.done);
            assert!(matches!(
                &updates[0],
                Update::Sidepad {
                    origin: Origin::SourceConfiguration,
                    status: Some(1),
                    ..
                } | Update::Combo {
                    origin: Origin::SourceConfiguration,
                    mat_plugged: Some(false),
                    mouse_connected: Some(true)
                }
            ));
        }
    }
    #[test]
    fn ble_fragment_assembly_and_notification_prefix_are_not_hid_framing() {
        assert_eq!(ble_query_frame(3), [3, 0, 0, 0, 1, 145, 0, 0]);
        let mut reply = BleResponse::new(3);
        assert!(reply.push(&[2, 1, 0, 0, 1, 145, 0, 2]).unwrap().is_none());
        assert!(reply.push(&[3, 1, 0, 0, 1, 145, 0, 2]).unwrap().is_none());
        assert_eq!(reply.push(&[4]).unwrap(), Some(vec![4]));
        assert!(
            BleResponse::new(1)
                .push(&[1, 1, 0, 0, 1, 145, 0, 3])
                .is_err()
        );
        assert_eq!(
            parse_event(&[0, 5, 14, 4], true).unwrap(),
            Some(Event::Sidepad(4))
        );
        assert_eq!(parse_event(&[0, 5, 14, 4], false).unwrap(), None);
        assert_eq!(
            parse_event(&[0, 5, 53, 2, 1], true).unwrap(),
            Some(Event::Wireless {
                connected: true,
                product_id: None
            })
        );
        assert_eq!(
            parse_event(&[0, 5, 53, 2, 0], true).unwrap(),
            Some(Event::Wireless {
                connected: false,
                product_id: None
            })
        );
        assert!(parse_event(&[0, 5, 53, 2], true).is_err());
        assert_eq!(parse_event(&[0, 5, 53, 2, 3], true).unwrap(), None);
        assert_eq!(
            parse_event(&[5, 53, 3, 1, 144, 0], false).unwrap(),
            Some(Event::Wireless {
                connected: true,
                product_id: Some(144)
            })
        );
        assert!(parse_event(&[5, 53, 3, 1, 144], false).is_err());
        assert_eq!(
            source(143)
                .unwrap()
                .transport(144, false)
                .unwrap()
                .transaction_prefix,
            0
        );
        assert_eq!(source(231).unwrap().interface(), 3);
    }

    #[test]
    fn sidepad_queries_bind_radio_and_shared_master_report_selectors() {
        use crate::backend::FeatureTransport;
        use std::cell::RefCell;

        struct Collection {
            sent: RefCell<Vec<u8>>,
            get_id: u8,
        }
        impl FeatureTransport for Collection {
            fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
                panic!("source sender must use the collection-sized adapter")
            }
            fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
                panic!("source getter must use the collection-sized adapter")
            }
            fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
                *self.sent.borrow_mut() = report.to_vec();
                Ok(())
            }
            fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
                assert_eq!(report.len(), 91);
                assert_eq!(report[0], self.get_id);
                assert!(report[1..].iter().all(|byte| *byte == 0));
                let sent = self.sent.borrow();
                report[0] = 9;
                report[1] = 2;
                report[2] = sent[2];
                report[6..9].copy_from_slice(&sent[6..9]);
                report[9] = 3;
                Ok(91)
            }
            fn metadata(&self) -> serde_json::Value {
                serde_json::json!({"simulated":true})
            }
        }
        for (product, physical, relay, set_id, get_id, delay) in [
            (231, 231, false, 0, 3, 5),
            (231, 232, false, 3, 3, 25),
            (231, 241, true, 3, 3, 25),
            (231, 179, true, 0, 0, 25),
            (143, 144, false, 0, 0, 25),
            (167, 241, true, 3, 3, 25),
        ] {
            let mut cap = source(product).unwrap().transport(physical, relay).unwrap();
            assert_eq!(cap.transaction_prefix, 0);
            assert_eq!(cap.sleep_between_out_ms, delay);
            assert!(cap.physical_transports_by_pid.is_empty());
            cap.sleep_between_out_ms = 0;
            cap.sleep_between_out_in_ms = 0;
            cap.sleep_between_in_ms = 0;
            let device = Collection {
                sent: RefCell::default(),
                get_id,
            };
            assert_eq!(
                query_hid(&device, &cap, 4, Query::Sidepad, &|| Ok(())).unwrap(),
                3
            );
            let sent = device.sent.borrow();
            assert_eq!((sent.len(), sent[0], sent[2]), (91, set_id, 4));
            assert_eq!(&sent[6..10], &[1, 0, 185, 0]);
            assert_eq!(sent[89], sent[3..89].iter().fold(0, |sum, byte| sum ^ byte));
        }
    }

    fn ble_endpoint(path: &str, parent: &str) -> serde_json::Value {
        serde_json::json!({
            "path": path,
            "device_container_id": "{01234567-89AB-CDEF-0123-456789ABCDEF}",
            "device_instance_id": format!(r"HID\BLE_MOUSE\{path}"),
            "parent_instance_id": parent,
            "vendor_id": 0x1532,
            "product_id": 146,
        })
    }

    #[test]
    fn ble_identity_keeps_the_actual_parent_address_container_and_collection() {
        let parent = r"BTHLEDevice\{00001812-0000-1000-8000-00805F9B34FB}_Dev_VID&021532_PID&0092_REV&0100_A1B2C3D4E5F6";
        let identity = ble_hid_identity(parent).unwrap();
        assert_eq!(identity.vendor_id(), 0x1532);
        assert_eq!(identity.product_id(), 146);
        assert_eq!(identity.address(), 0xa1b2c3d4e5f6);
        assert_eq!(
            ble_hid_identity(&parent.to_ascii_lowercase()),
            Some(identity)
        );
        let first = ble_endpoint(r"\\?\hid#mouse#a", parent);
        let mut unrelated = ble_endpoint(r"\\?\hid#mouse#other", parent);
        unrelated["device_container_id"] = "{FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF}".into();
        let observation = serde_json::json!({
            "complete": true,
            "interfaces": [ble_endpoint(r"\\?\hid#mouse#b", parent), unrelated, first.clone()]
        });
        let container = "{01234567-89ab-cdef-0123-456789abcdef}";
        let target = ble_target_from_hid(143, 146, container, &observation).unwrap();
        assert_eq!(target.mac_address, identity.address());
        assert_eq!(target.device_container_id, container);
        assert_eq!(target.hid_path, first["path"].as_str().unwrap());
        assert_eq!(
            target.device_instance_id,
            first["device_instance_id"].as_str().unwrap()
        );
        assert_eq!(target.parent_instance_id, parent);
        assert!(ble_target_from_hid(143, 144, container, &observation).is_err());
        assert!(ble_target_from_hid(167, 146, container, &observation).is_err());
    }

    #[test]
    fn ble_identity_rejects_incomplete_mismatched_and_ambiguous_installed_endpoints() {
        let parent = r"BTHLEDevice\_VID&021532_PID&0092_REV&0100_A1B2C3D4E5F6";
        let container = "{01234567-89AB-CDEF-0123-456789ABCDEF}";
        let endpoint = ble_endpoint(r"\\?\hid#mouse#a", parent);
        let mut observation =
            serde_json::json!({"complete": false, "interfaces": [endpoint.clone()]});
        assert!(ble_target_from_hid(143, 146, container, &observation).is_err());
        observation["complete"] = true.into();
        observation["interfaces"][0]["product_id"] = 144.into();
        assert!(ble_target_from_hid(143, 146, container, &observation).is_err());
        observation["interfaces"] = serde_json::json!([endpoint.clone(), endpoint.clone()]);
        assert!(ble_target_from_hid(143, 146, container, &observation).is_err());
        observation["interfaces"][1] = ble_endpoint(
            r"\\?\hid#mouse#b",
            &parent.replace("A1B2C3D4E5F6", "A1B2C3D4E5F7"),
        );
        assert!(ble_target_from_hid(143, 146, container, &observation).is_err());
        observation["interfaces"] = serde_json::json!([endpoint]);
        observation["interfaces"][0]["device_instance_id"] = "".into();
        assert!(ble_target_from_hid(143, 146, container, &observation).is_err());
        for invalid in [
            parent.replace("BTHLEDevice", "USB"),
            parent.replace("1532", "FFFF"),
            parent.replace("0092", "0000"),
            parent.replace("A1B2C3D4E5F6", "000000000000"),
            parent.replace("A1B2C3D4E5F6", "A1B2C3D4E5"),
            parent.replace("A1B2C3D4E5F6", "A1B2C3D4E5GG"),
        ] {
            assert!(ble_hid_identity(&invalid).is_none(), "{invalid}");
        }
        assert_eq!(
            ble_hid_identity(&parent.replace("1532", "068E"))
                .unwrap()
                .vendor_id(),
            0x068e
        );
    }

    #[test]
    fn canceling_after_initial_query_still_finishes_a_failed_registration() {
        struct RegistrationFailure {
            finished: bool,
        }
        impl HardwareEvents for RegistrationFailure {
            fn begin(&mut self) -> anyhow::Result<()> {
                anyhow::bail!("registration failed")
            }
            fn receive(&mut self, _: Duration) -> anyhow::Result<Option<Vec<u8>>> {
                unreachable!()
            }
            fn finish(&mut self) -> anyhow::Result<()> {
                self.finished = true;
                Ok(())
            }
        }
        let mut events = RegistrationFailure { finished: false };
        let mut transport = Script {
            reads: [Ok(2)].into(),
            calls: 0,
        };
        let mut updates = vec![];
        let result = monitor(
            source(143).unwrap(),
            144,
            &mut transport,
            &mut events,
            &|| Ok(()),
            &mut |update| {
                updates.push(update);
                Ok(())
            },
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("registration failed")
        );
        assert!(events.finished);
        assert_eq!(transport.calls, 1);
        assert!(matches!(
            &updates[0],
            Update::Sidepad {
                status: Some(2),
                ..
            }
        ));
        assert!(source(103).unwrap().transport(179, true).is_err());
        assert!(source(143).unwrap().transport(146, false).is_err());
    }
}

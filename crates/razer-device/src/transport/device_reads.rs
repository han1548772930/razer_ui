//! Source-generated shared device query capabilities and session observations.
//! No local profile, route default or registry history supplies these values.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceReadKind {
    Firmware,
    Battery,
    Charging,
    Polling,
    Dpi,
    Idle,
}

impl DeviceReadKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Firmware => "firmware",
            Self::Battery => "battery",
            Self::Charging => "charging",
            Self::Polling => "polling",
            Self::Dpi => "dpi",
            Self::Idle => "idle",
        }
    }

    fn minimum_payload_bytes(self) -> usize {
        match self {
            Self::Firmware => 4,
            Self::Battery | Self::Charging | Self::Idle => 2,
            Self::Polling => 1,
            Self::Dpi => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) enum SourceReadMethod {
    #[serde(rename = "getExtendedFirmwareVersion")]
    ExtendedFirmware,
    #[serde(rename = "getBatteryLevel")]
    BatteryLevel,
    #[serde(rename = "getChargingStatus")]
    ChargingStatus,
    #[serde(rename = "getPollingRate")]
    PollingRate,
    #[serde(rename = "getProfilePollingRate")]
    ProfilePollingRate,
    #[serde(rename = "getUSBHighSpeedPollingRate")]
    UsbHighSpeedPollingRate,
    #[serde(rename = "getDpiLevel")]
    DpiLevel,
    #[serde(rename = "getTimeToSleep")]
    TimeToSleep,
}

impl SourceReadMethod {
    fn kind(self) -> DeviceReadKind {
        match self {
            Self::ExtendedFirmware => DeviceReadKind::Firmware,
            Self::BatteryLevel => DeviceReadKind::Battery,
            Self::ChargingStatus => DeviceReadKind::Charging,
            Self::PollingRate | Self::ProfilePollingRate | Self::UsbHighSpeedPollingRate => {
                DeviceReadKind::Polling
            }
            Self::DpiLevel => DeviceReadKind::Dpi,
            Self::TimeToSleep => DeviceReadKind::Idle,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::ExtendedFirmware => "getExtendedFirmwareVersion",
            Self::BatteryLevel => "getBatteryLevel",
            Self::ChargingStatus => "getChargingStatus",
            Self::PollingRate => "getPollingRate",
            Self::ProfilePollingRate => "getProfilePollingRate",
            Self::UsbHighSpeedPollingRate => "getUSBHighSpeedPollingRate",
            Self::DpiLevel => "getDpiLevel",
            Self::TimeToSleep => "getTimeToSleep",
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceReadCommand {
    name: DeviceReadKind,
    method: SourceReadMethod,
    command: [u8; 3],
    payload: Vec<u8>,
    min_response_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "SourceReadCommand")]
pub struct ReadCommand {
    pub name: DeviceReadKind,
    pub method: String,
    pub command: [u8; 3],
    pub payload: Vec<u8>,
    pub min_response_bytes: usize,
}

impl TryFrom<SourceReadCommand> for ReadCommand {
    type Error = anyhow::Error;

    fn try_from(source: SourceReadCommand) -> anyhow::Result<Self> {
        ensure!(
            source.name == source.method.kind(),
            "Read method and response kind differ"
        );
        let command = Self {
            name: source.name,
            method: source.method.name().into(),
            command: source.command,
            payload: source.payload,
            min_response_bytes: source.min_response_bytes,
        };
        command.validate_shape()?;
        Ok(command)
    }
}

impl ReadCommand {
    fn validate_shape(&self) -> anyhow::Result<()> {
        ensure!(
            self.command[0] <= 80
                && self.payload.len() <= usize::from(self.command[0])
                && self.min_response_bytes <= 80,
            "Read command or response exceeds the source report payload"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceChargingStatus {
    NoChargeBatteryFull,
    Charging,
    ChargingFailed,
    ForeignObjectDetection,
    FastCharging,
    FullCharging,
    ReachChargingLimit,
}

impl SourceChargingStatus {
    fn parse(value: &str) -> anyhow::Result<Self> {
        Ok(match value {
            "NoCharge_BatteryFull" => Self::NoChargeBatteryFull,
            "Charging" => Self::Charging,
            "ChargingFailed" => Self::ChargingFailed,
            "ForeignObjectDetection" => Self::ForeignObjectDetection,
            "FastCharging" => Self::FastCharging,
            "FullCharging" => Self::FullCharging,
            "ReachChargingLimit" => Self::ReachChargingLimit,
            _ => anyhow::bail!("Unknown source charging status: {value}"),
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::NoChargeBatteryFull => "NoCharge_BatteryFull",
            Self::Charging => "Charging",
            Self::ChargingFailed => "ChargingFailed",
            Self::ForeignObjectDetection => "ForeignObjectDetection",
            Self::FastCharging => "FastCharging",
            Self::FullCharging => "FullCharging",
            Self::ReachChargingLimit => "ReachChargingLimit",
        }
    }
}

fn polling_hz(name: &str) -> anyhow::Result<u32> {
    Ok(match name {
        "RATE_100Hz" => 100,
        "RATE_125Hz" => 125,
        "RATE_200Hz" => 200,
        "RATE_250Hz" => 250,
        "RATE_333Hz" => 333,
        "RATE_500Hz" => 500,
        "RATE_1000Hz" => 1000,
        "RATE_2000Hz" => 2000,
        "RATE_4000Hz" => 4000,
        "RATE_8000Hz" => 8000,
        _ => anyhow::bail!("Unknown source polling rate: {name}"),
    })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DeviceReadCapability {
    pub product_id: u32,
    pub source_class: String,
    pub direct_pids: Vec<u32>,
    pub vendor_id: u16,
    pub claim_interface: u8,
    /// Exact PID-keyed interface selection from DeviceInfo.claimInterface.
    /// An absent map denotes the source's scalar interface; a missing PID in
    /// a present map is unsupported and must never fall back to its first row.
    #[serde(default)]
    pub claim_interfaces_by_pid: BTreeMap<String, u8>,
    /// Complete mounted factory branches for capabilities which span wired
    /// and dongle devices. Dedicated physical capabilities leave this empty.
    #[serde(default)]
    pub physical_transports_by_pid: BTreeMap<String, PhysicalFeatureTransport>,
    pub report_id: u8,
    pub report_bytes: usize,
    pub transaction_prefix: u8,
    pub transaction_modulus: u8,
    pub queries: Vec<ReadCommand>,
    pub polling_codes: BTreeMap<String, u8>,
    /// Source parser byte: ordinary polling uses data[0], profiles use data[1].
    #[serde(default)]
    pub polling_value_offset: Option<usize>,
    /// Only populated where the original switch default is source-verified.
    #[serde(default)]
    pub polling_default_hz: Option<u32>,
    /// Polling branch proved for the physical product, not a receiver relay.
    #[serde(default)]
    pub polling_physical_product_id: Option<u32>,
    pub charging_codes: BTreeMap<String, u8>,
    pub max_retry_in: u8,
    pub max_retry_out: u8,
    pub sleep_between_out_ms: u64,
    pub sleep_between_out_in_ms: u64,
    pub sleep_between_in_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PhysicalFeatureTransport {
    pub source_class: String,
    pub claim_interface: u8,
    pub report_id: u8,
    pub transaction_prefix: u8,
    pub transaction_modulus: u8,
    pub max_retry_in: u8,
    pub max_retry_out: u8,
    pub sleep_between_out_ms: u64,
    pub sleep_between_out_in_ms: u64,
    pub sleep_between_in_ms: u64,
    pub is_duallink: bool,
}

impl DeviceReadCapability {
    fn validate_source(&self) -> anyhow::Result<()> {
        ensure!(
            self.product_id > 0
                && self.product_id <= u32::from(u16::MAX)
                && self.vendor_id != 0
                && !self.direct_pids.is_empty()
                && self
                    .direct_pids
                    .iter()
                    .all(|pid| *pid > 0 && *pid <= u32::from(u16::MAX))
                && self.report_bytes == 91
                && self.transaction_modulus > 0
                && self.max_retry_in > 0
                && self.max_retry_out > 0
                && !self.queries.is_empty(),
            "Invalid source read capability identity or transport"
        );
        let counter_bits = (0..self.transaction_modulus).fold(0, |bits, counter| bits | counter);
        ensure!(
            self.transaction_prefix & counter_bits == 0,
            "Read transaction prefix overlaps its counter"
        );
        for key in self
            .claim_interfaces_by_pid
            .keys()
            .chain(self.physical_transports_by_pid.keys())
        {
            let pid: u16 = key
                .parse()
                .context("Physical transport PID is not numeric")?;
            ensure!(
                pid != 0 && pid.to_string() == *key,
                "Physical transport PID is not canonical"
            );
        }
        for branch in self.physical_transports_by_pid.values() {
            ensure!(
                !branch.source_class.is_empty()
                    && branch.transaction_modulus > 0
                    && branch.max_retry_in > 0
                    && branch.max_retry_out > 0,
                "Invalid physical source transport"
            );
            let bits = (0..branch.transaction_modulus).fold(0, |bits, counter| bits | counter);
            ensure!(
                branch.transaction_prefix & bits == 0,
                "Physical transaction namespace overlaps its counter"
            );
        }
        let mut queries = BTreeSet::new();
        for query in &self.queries {
            query.validate_shape()?;
            ensure!(
                queries.insert(query.name.name()),
                "Repeated read response kind"
            );
            let required = if query.name == DeviceReadKind::Polling {
                self.polling_value_offset
                    .unwrap_or(1)
                    .checked_add(1)
                    .context("Polling value offset exceeds the response address space")?
            } else {
                query.name.minimum_payload_bytes()
            };
            ensure!(
                query.min_response_bytes >= required,
                "Read response contract omits decoder fields"
            );
        }
        let mut codes = BTreeSet::new();
        for (name, code) in &self.polling_codes {
            polling_hz(name)?;
            ensure!(codes.insert(code), "Ambiguous source polling code");
        }
        codes.clear();
        for (name, code) in &self.charging_codes {
            SourceChargingStatus::parse(name)?;
            ensure!(codes.insert(code), "Ambiguous source charging code");
        }
        Ok(())
    }

    /// UsbRzDeviceAction's GET uses a truthy reportId fallback, independently
    /// of SET's nullish fallback. The caller binds the physical interface first.
    pub fn get_report_id(&self) -> u8 {
        if self.report_id == 0 {
            self.claim_interface
        } else {
            self.report_id
        }
    }

    pub fn for_physical_product(&self, physical_product_id: u32) -> anyhow::Result<Self> {
        ensure!(
            self.direct_pids.contains(&physical_product_id),
            "物理 PID 不属于当前源能力"
        );
        let mut bound = self.clone();
        bound.claim_interface = self.claim_interface_for(physical_product_id)?;
        if !self.physical_transports_by_pid.is_empty() {
            let branch = self
                .physical_transports_by_pid
                .get(&physical_product_id.to_string())
                .context("当前源 factory 没有此物理 PID 分支")?;
            bound.select_physical_transport(branch);
        }
        bound.claim_interfaces_by_pid.clear();
        bound.physical_transports_by_pid.clear();
        Ok(bound)
    }

    fn select_physical_transport(&mut self, branch: &PhysicalFeatureTransport) {
        self.source_class = branch.source_class.clone();
        self.claim_interface = branch.claim_interface;
        self.report_id = branch.report_id;
        self.transaction_prefix = branch.transaction_prefix;
        self.transaction_modulus = branch.transaction_modulus;
        self.max_retry_in = branch.max_retry_in;
        self.max_retry_out = branch.max_retry_out;
        self.sleep_between_out_ms = branch.sleep_between_out_ms;
        self.sleep_between_out_in_ms = branch.sleep_between_out_in_ms;
        self.sleep_between_in_ms = branch.sleep_between_in_ms;
    }

    /// The service supplies a currently observed receiver/master only after
    /// checking its live peer association. The source uses the logical device's
    /// radio class with the master's actual PID/interface.
    pub fn for_duallink_master(
        &self,
        master_product_id: u32,
        claim_interface: u8,
    ) -> anyhow::Result<Self> {
        let branch = self
            .physical_transports_by_pid
            .values()
            .find(|branch| branch.is_duallink)
            .context("当前源 factory 没有 DualLink master 分支")?;
        let mut bound = self.clone();
        bound.select_physical_transport(branch);
        bound.claim_interface = claim_interface;
        bound.report_id = claim_interface;
        bound.direct_pids = vec![master_product_id];
        bound.claim_interfaces_by_pid.clear();
        bound.physical_transports_by_pid.clear();
        Ok(bound)
    }

    /// Common command owners can retain a logical capability while the native
    /// handle supplies the selected factory branch. Keep the allocated source
    /// counter, then apply that branch's `_getTransactionId` prefix.
    pub(crate) fn for_exchange<'a>(
        &'a self,
        transport: &dyn crate::backend::FeatureTransport,
        transaction: u8,
    ) -> anyhow::Result<(std::borrow::Cow<'a, Self>, u8)> {
        if self.physical_transports_by_pid.is_empty() {
            return Ok((std::borrow::Cow::Borrowed(self), transaction));
        }
        let Some(node) = transport.hid_node() else {
            // A pure protocol fixture already supplies its explicit source
            // namespace. This does not claim a hardware identity or read.
            return Ok((std::borrow::Cow::Borrowed(self), transaction));
        };
        let counter = (0..self.transaction_modulus)
            .find(|counter| self.transaction_prefix | counter == transaction)
            .context("Feature transaction is outside the supplied source namespace")?;
        let bound = self.for_physical_product(u32::from(node.product_id))?;
        ensure!(
            counter < bound.transaction_modulus,
            "Physical source transaction counter is invalid"
        );
        let transaction = bound.transaction_prefix | counter;
        Ok((std::borrow::Cow::Owned(bound), transaction))
    }

    /// Resolve source PID maps at the shared exchange point, including setter
    /// acknowledgements whose callers construct a capability internally.
    /// Scalar interfaces (including an explicitly selected DualLink master)
    /// remain source-owned and are never replaced with a USB descriptor value.
    pub fn get_report_id_for_transport(
        &self,
        transport: &dyn crate::backend::FeatureTransport,
    ) -> anyhow::Result<u8> {
        if self.report_id != 0 || self.claim_interfaces_by_pid.is_empty() {
            return Ok(self.get_report_id());
        }
        let node = transport
            .hid_node()
            .context("源接口映射需要实际已打开的 HID 节点")?;
        // Platform routing verifies the collection/interface identity. On
        // Windows hidapi may expose interface -1 after SetupAPI verified the
        // actual interface, so the protocol must use the source PID map.
        self.claim_interface_for(u32::from(node.product_id))
    }

    pub fn claim_interface_for(&self, physical_product_id: u32) -> anyhow::Result<u8> {
        if self.claim_interfaces_by_pid.is_empty() {
            return Ok(self.claim_interface);
        }
        self.claim_interfaces_by_pid
            .get(&physical_product_id.to_string())
            .copied()
            .with_context(|| format!("原产品接口映射不包含实际 PID {physical_product_id}"))
    }
}

#[cfg(test)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadCatalog {
    schema_version: u8,
    products: Vec<DeviceReadCapability>,
}

#[cfg(test)]
fn decode_catalog(json: &str) -> anyhow::Result<ReadCatalog> {
    let catalog: ReadCatalog = serde_json::from_str(json).context("Invalid device read catalog")?;
    ensure!(
        catalog.schema_version == 1 && !catalog.products.is_empty(),
        "Invalid device read catalog schema"
    );
    let mut products = BTreeSet::new();
    for capability in &catalog.products {
        ensure!(
            products.insert(capability.product_id),
            "Repeated device read product"
        );
        capability.validate_source()?;
    }
    Ok(catalog)
}

pub fn capability(product_id: u32) -> Option<&'static DeviceReadCapability> {
    if matches!(product_id, 164 | 179 | 241) {
        return receiver_firmware_capabilities()
            .iter()
            .find(|capability| capability.product_id == product_id);
    }
    if product_id == crate::mouse_legacy_polling::PRODUCT_ID {
        return Some(crate::mouse_legacy_polling::read_capability());
    }
    static CATALOG: OnceLock<Vec<DeviceReadCapability>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let products = crate::device_read_capabilities::products();
            for product in &products {
                product.validate_source().expect("fixed device read table");
            }
            products
        })
        .iter()
        .find(|capability| capability.product_id == product_id)
}

/// Current Linker/Uma/MultiDevices inherit rzDevice25.getExtendedFirmwareVersion
/// (84816.Fk -> 30580.MC [4,0,135]); the firmware owner publishes all four bytes.
/// Use each receiver's audited sender/interface and transaction namespace.
/// Receivers have no battery/polling/DPI query merely because they relay one.
fn receiver_firmware_capabilities() -> &'static [DeviceReadCapability] {
    static CAPABILITIES: OnceLock<Vec<DeviceReadCapability>> = OnceLock::new();
    CAPABILITIES.get_or_init(|| {
        [164, 179, 241]
            .into_iter()
            .map(|product_id| {
                let receiver = crate::receiver_capabilities::capability(product_id)
                    .expect("current receiver firmware transport");
                DeviceReadCapability {
                    product_id: u32::from(product_id),
                    source_class: receiver.source_class.clone(),
                    direct_pids: vec![u32::from(product_id)],
                    vendor_id: receiver.vendor_id,
                    claim_interface: receiver.claim_interface,
                    claim_interfaces_by_pid: BTreeMap::new(),
                    physical_transports_by_pid: BTreeMap::new(),
                    report_id: receiver.report_id,
                    report_bytes: receiver.report_bytes,
                    transaction_prefix: receiver.transaction_prefix,
                    transaction_modulus: receiver.transaction_modulus,
                    queries: vec![ReadCommand {
                        name: DeviceReadKind::Firmware,
                        method: "getExtendedFirmwareVersion".into(),
                        command: [4, 0, 135],
                        payload: vec![],
                        min_response_bytes: 4,
                    }],
                    polling_codes: BTreeMap::new(),
                    polling_value_offset: None,
                    polling_default_hz: None,
                    polling_physical_product_id: None,
                    charging_codes: BTreeMap::new(),
                    max_retry_in: receiver.max_retry_in,
                    max_retry_out: receiver.max_retry_out,
                    sleep_between_out_ms: receiver.sleep_between_out_ms,
                    sleep_between_out_in_ms: receiver.sleep_between_out_in_ms,
                    sleep_between_in_ms: receiver.sleep_between_in_ms,
                }
            })
            .collect()
    })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DeviceReadTarget {
    pub product_id: u32,
    pub physical_product_id: u32,
    pub peer_product_id: Option<u32>,
    pub device_container_id: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DeviceReadValue {
    Firmware {
        version: String,
    },
    Battery {
        percent: u8,
    },
    Charging {
        status: String,
    },
    Polling {
        hz: u32,
    },
    Dpi {
        x: u16,
        y: u16,
    },
    /// Native timeToSleep u16. UI minute/second conversion is product-owned.
    Idle {
        raw_time: u16,
    },
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct DeviceReadValues {
    pub firmware: Option<String>,
    pub battery_percent: Option<u8>,
    pub charging_status: Option<String>,
    pub polling_hz: Option<u32>,
    pub dpi: Option<(u16, u16)>,
    pub idle_raw_time: Option<u16>,
    pub errors: BTreeMap<String, String>,
}

impl DeviceReadValues {
    pub fn insert(
        &mut self,
        expected: DeviceReadKind,
        value: DeviceReadValue,
    ) -> anyhow::Result<()> {
        match (expected, value) {
            (DeviceReadKind::Firmware, DeviceReadValue::Firmware { version }) => {
                self.firmware = Some(version)
            }
            (DeviceReadKind::Battery, DeviceReadValue::Battery { percent }) => {
                self.battery_percent = Some(percent)
            }
            (DeviceReadKind::Charging, DeviceReadValue::Charging { status }) => {
                self.charging_status = Some(status)
            }
            (DeviceReadKind::Polling, DeviceReadValue::Polling { hz }) => {
                self.polling_hz = Some(hz)
            }
            (DeviceReadKind::Dpi, DeviceReadValue::Dpi { x, y }) => self.dpi = Some((x, y)),
            (DeviceReadKind::Idle, DeviceReadValue::Idle { raw_time }) => {
                self.idle_raw_time = Some(raw_time)
            }
            _ => anyhow::bail!("设备查询响应类型不匹配"),
        }
        Ok(())
    }
}

pub fn query_report(
    cap: &DeviceReadCapability,
    command: &ReadCommand,
    transaction: u8,
) -> anyhow::Result<Vec<u8>> {
    command.validate_shape()?;
    ensure!(
        cap.report_bytes == 91
            && command.command[0] <= 80
            && command.payload.len() <= usize::from(command.command[0]),
        "源查询报文规格无效"
    );
    let mut report = vec![0; cap.report_bytes];
    report[0] = cap.report_id;
    report[2] = transaction;
    report[6..9].copy_from_slice(&command.command);
    report[9..9 + command.payload.len()].copy_from_slice(&command.payload);
    report[89] = report[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    Ok(report)
}

pub enum ReadReply {
    Busy,
    Retry(String),
    Complete(DeviceReadValue),
}

pub fn decode_report(
    report: &[u8],
    cap: &DeviceReadCapability,
    command: &ReadCommand,
    transaction: u8,
) -> anyhow::Result<ReadReply> {
    command.validate_shape()?;
    ensure!(
        report.len() == cap.report_bytes && report.len() == 91,
        "设备查询响应长度不匹配"
    );
    // The host strips the returned prefix with slice(1). A GET may use the
    // interface as a selector, so it does not compare this byte with the SET ID.
    let packet = &report[1..];
    if packet[1] != transaction || packet[6..8] != command.command[1..] {
        return Ok(ReadReply::Retry("设备查询事务或命令不匹配".into()));
    }
    match packet[0] {
        1 => return Ok(ReadReply::Busy),
        2 => {}
        0 | 3 | 4 => return Ok(ReadReply::Retry(format!("设备查询响应状态 {}", packet[0]))),
        state => anyhow::bail!("设备查询不受支持或返回未知状态 {state}"),
    }
    let length = usize::from(packet[5]);
    ensure!(
        length >= command.min_response_bytes && length <= 80,
        "设备查询响应字段截断"
    );
    let required = if command.name == DeviceReadKind::Polling {
        cap.polling_value_offset
            .unwrap_or(1)
            .checked_add(1)
            .context("Polling value offset exceeds the response address space")?
    } else {
        command.name.minimum_payload_bytes()
    };
    ensure!(
        length >= required,
        "Device query response omits its typed value fields"
    );
    let data = &packet[8..8 + length];
    if let Some(expected) = command.payload.first() {
        ensure!(
            data[0] == *expected,
            "设备查询返回了其他battery/profile/class的数据"
        );
    }
    let value = match command.name {
        DeviceReadKind::Firmware => DeviceReadValue::Firmware {
            version: format!("{}.{}.{}.{}", data[0], data[1], data[2], data[3]),
        },
        DeviceReadKind::Battery => DeviceReadValue::Battery {
            percent: (u16::from(data[1]) * 100 / 255) as u8,
        },
        DeviceReadKind::Charging => DeviceReadValue::Charging {
            status: cap
                .charging_codes
                .iter()
                .find(|(_, code)| **code == data[1])
                .map(|(name, _)| SourceChargingStatus::parse(name))
                .transpose()?
                .map(|status| status.name().to_owned())
                .context("未知充电状态编码")?,
        },
        DeviceReadKind::Polling => {
            let code = data
                .get(cap.polling_value_offset.unwrap_or(1))
                .context("回报率响应缺少源码指定的字段")?;
            let name = cap
                .polling_codes
                .iter()
                .find(|(_, value)| *value == code)
                .map(|(name, _)| name);
            let name = name.with_context(|| format!("未知回报率编码：{code}"))?;
            let hz = polling_hz(name)?;
            DeviceReadValue::Polling { hz }
        }
        DeviceReadKind::Dpi => {
            // The source getter decodes the raw unsigned fields, including 0.
            // Product UI edit limits do not constrain a hardware observation.
            let x = u16::from_be_bytes([data[1], data[2]]);
            let y = u16::from_be_bytes([data[3], data[4]]);
            DeviceReadValue::Dpi { x, y }
        }
        DeviceReadKind::Idle => DeviceReadValue::Idle {
            raw_time: u16::from_be_bytes([data[0], data[1]]),
        },
    };
    Ok(ReadReply::Complete(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pure protocol fixtures. No native transport or hardware is used.
    fn reply(kind: DeviceReadKind, data: &[u8]) -> (Vec<u8>, &'static ReadCommand) {
        let cap = capability(182).unwrap();
        let command = cap.queries.iter().find(|item| item.name == kind).unwrap();
        let mut report = query_report(cap, command, 7).unwrap();
        report[1] = 2;
        report[6] = data.len() as u8;
        report[9..9 + data.len()].copy_from_slice(data);
        (report, command)
    }

    #[test]
    fn source_catalog_rejects_mismatched_methods_short_responses_and_duplicate_products() {
        let source = serde_json::json!({"schema_version":1,"products":[capability(182).unwrap()]});
        assert!(decode_catalog(&source.to_string()).is_ok());
        for mutation in 0..5 {
            let mut value = source.clone();
            match mutation {
                0 => value["products"][0]["queries"][0]["method"] = "getBatteryLevel".into(),
                1 => value["products"][0]["queries"][0]["min_response_bytes"] = 1.into(),
                2 => value["products"][0]["transaction_modulus"] = 0.into(),
                3 => {
                    let repeated = value["products"][0].clone();
                    value["products"].as_array_mut().unwrap().push(repeated);
                }
                4 => value["schema_version"] = 2.into(),
                _ => unreachable!(),
            }
            assert!(
                decode_catalog(&value.to_string()).is_err(),
                "mutation {mutation}"
            );
        }
        assert!(
            serde_json::from_str::<ReadCommand>(
                r#"{
            "name":"firmware","method":"getUnverifiedFirmware",
            "command":[4,0,135],"payload":[],"min_response_bytes":4
        }"#
            )
            .is_err()
        );
    }

    #[test]
    fn explicit_response_contract_cannot_bypass_typed_decoder_fields() {
        let cap = capability(182).unwrap();
        let (report, mut command) = {
            let (report, command) = reply(DeviceReadKind::Firmware, &[]);
            (report, command.clone())
        };
        command.min_response_bytes = 0;
        assert!(decode_report(&report, cap, &command, 7).is_err());
        let (report, command) = reply(DeviceReadKind::Polling, &[1, 8]);
        let mut cap = cap.clone();
        cap.polling_value_offset = Some(usize::MAX);
        assert!(decode_report(&report, &cap, command, 7).is_err());
    }

    #[test]
    fn reads_only_source_response_fields_and_preserves_units() {
        let cap = capability(182).unwrap();
        for (raw, expected) in [(0, 0), (128, 50), (255, 100)] {
            let (report, command) = reply(DeviceReadKind::Battery, &[0, raw]);
            assert!(matches!(
                decode_report(&report, cap, command, 7).unwrap(),
                ReadReply::Complete(DeviceReadValue::Battery { percent }) if percent == expected
            ));
        }
        let (report, command) = reply(DeviceReadKind::Polling, &[1, 8]);
        assert!(matches!(
            decode_report(&report, cap, command, 7).unwrap(),
            ReadReply::Complete(DeviceReadValue::Polling { hz: 125 })
        ));
        let (report, command) = reply(DeviceReadKind::Dpi, &[0, 3, 32, 6, 64, 0, 0]);
        assert!(matches!(
            decode_report(&report, cap, command, 7).unwrap(),
            ReadReply::Complete(DeviceReadValue::Dpi { x: 800, y: 1600 })
        ));
        for (x, y) in [(800u16, 0u16), (0, 0), (65535, 65535)] {
            let xb = x.to_be_bytes();
            let yb = y.to_be_bytes();
            let (report, command) =
                reply(DeviceReadKind::Dpi, &[0, xb[0], xb[1], yb[0], yb[1], 0, 0]);
            assert!(matches!(
                decode_report(&report, cap, command, 7).unwrap(),
                ReadReply::Complete(DeviceReadValue::Dpi { x: actual_x, y: actual_y })
                    if (actual_x, actual_y) == (x, y)
            ));
        }
        let (report, command) = reply(DeviceReadKind::Firmware, &[2, 1, 3, 0]);
        assert!(matches!(
            decode_report(&report, cap, command, 7).unwrap(),
            ReadReply::Complete(DeviceReadValue::Firmware { version }) if version == "2.1.3.0"
        ));
    }

    #[test]
    fn rejects_unknown_codes_wrong_scope_and_partial_or_stale_reports() {
        let cap = capability(182).unwrap();
        for (kind, data) in [
            (DeviceReadKind::Charging, vec![0, 99]),
            (DeviceReadKind::Polling, vec![1, 99]),
            (DeviceReadKind::Battery, vec![1, 255]),
            (DeviceReadKind::Dpi, vec![1, 3, 32, 3, 32, 0, 0]),
            (DeviceReadKind::Firmware, vec![1, 2]),
        ] {
            let (report, command) = reply(kind, &data);
            assert!(decode_report(&report, cap, command, 7).is_err());
        }
        let (mut report, command) = reply(DeviceReadKind::Battery, &[0, 255]);
        assert!(matches!(
            decode_report(&report, cap, command, 8).unwrap(),
            ReadReply::Retry(_)
        ));
        report[8] ^= 1;
        assert!(matches!(
            decode_report(&report, cap, command, 7).unwrap(),
            ReadReply::Retry(_)
        ));
        assert!(decode_report(&report[..90], cap, command, 7).is_err());
    }

    #[test]
    fn query_header_payload_and_checksum_use_mouse_transaction_namespace() {
        let cap = capability(182).unwrap();
        let command = cap
            .queries
            .iter()
            .find(|item| item.name == DeviceReadKind::Dpi)
            .unwrap();
        let report = query_report(cap, command, 7).unwrap();
        assert_eq!(&report[..10], &[0, 0, 7, 0, 0, 0, 7, 4, 133, 0]);
        assert_eq!(report[89], 7 ^ 4 ^ 133);
        assert_eq!(cap.transaction_prefix, 0);
    }

    #[test]
    fn claim_interface_selection_preserves_source_pid_map() {
        let cap = capability(664).expect("source catalog contains product 664");
        assert_eq!(cap.claim_interface, 3);
        assert_eq!(cap.claim_interfaces_by_pid.get("662"), Some(&2));
        assert_eq!(cap.claim_interfaces_by_pid.get("663"), Some(&3));
        assert_eq!(cap.claim_interfaces_by_pid.get("664"), Some(&3));
        assert_eq!(cap.claim_interface_for(662).unwrap(), 2);
        assert_eq!(cap.claim_interface_for(663).unwrap(), 3);
        assert_eq!(cap.claim_interface_for(664).unwrap(), 3);
        assert!(cap.claim_interface_for(9999).is_err());

        let scalar = capability(182).expect("source catalog contains product 182");
        assert!(scalar.claim_interfaces_by_pid.is_empty());
        assert_eq!(
            scalar.claim_interface_for(182).unwrap(),
            scalar.claim_interface
        );
    }
}
#[test]
fn startup_state_receiver_firmware_uses_own_transport_without_mouse_telemetry() {
    for (pid, interface) in [(164, 0), (179, 0), (241, 3)] {
        let cap = capability(pid).unwrap();
        assert_eq!(cap.direct_pids, [pid]);
        assert_eq!(cap.claim_interface, interface);
        assert_eq!(cap.get_report_id(), interface);
        assert_eq!(cap.queries.len(), 1);
        let command = &cap.queries[0];
        assert_eq!(command.name, DeviceReadKind::Firmware);
        assert_eq!(command.command, [4, 0, 135]);
        let mut report = vec![0; 91];
        report[0] = 9; // The host removes this prefix after GET.
        report[1] = 2;
        report[2] = 224;
        report[6..9].copy_from_slice(&[4, 0, 135]);
        report[9..13].copy_from_slice(&[2, 0, 3, 0]);
        let ReadReply::Complete(reading) = decode_report(&report, cap, command, 224).unwrap()
        else {
            panic!("firmware response must complete");
        };
        let mut values = DeviceReadValues::default();
        values.insert(DeviceReadKind::Firmware, reading).unwrap();
        assert_eq!(values.firmware.as_deref(), Some("2.0.3.0"));
        assert!(values.battery_percent.is_none());
        assert!(values.charging_status.is_none());
        assert!(values.dpi.is_none());
    }
}

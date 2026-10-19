//! Current 190/229/239 three-LED receiver configuration and notifications.
//! Profile drafts belong to the caller. A setter ACK is separate from a GET,
//! and the original quality GET object is never a source UI notification.
use crate::{
    backend::FeatureTransport,
    device_reads::{DeviceReadCapability, DeviceReadTarget},
    receiver_pairing::HardwareEvents,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const GET_CONFIGURATION: [u8; 3] = [3, 7, 149];
pub const SET_CONFIGURATION: [u8; 3] = [3, 7, 21];
pub const GET_CONNECTION_QUALITY: [u8; 3] = [1, 7, 150];
pub const GET_SERIAL_NUMBER: [u8; 3] = [22, 0, 130];
pub const GET_BATTERY_LEVEL: [u8; 3] = [2, 7, 128];
pub const GET_CHARGING_STATUS: [u8; 3] = [2, 7, 132];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "u8", into = "u8")]
pub enum Mode {
    Off,
    BatteryLevel,
    ConnectionQuality,
    PollingRate,
    DpiStage,
}
impl TryFrom<u8> for Mode {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> anyhow::Result<Self> {
        Ok(match value {
            0 => Self::Off,
            1 => Self::BatteryLevel,
            2 => Self::ConnectionQuality,
            3 => Self::PollingRate,
            4 => Self::DpiStage,
            _ => anyhow::bail!("Unknown receiver LED mode {value}"),
        })
    }
}
impl From<Mode> for u8 {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Off => 0,
            Mode::BatteryLevel => 1,
            Mode::ConnectionQuality => 2,
            Mode::PollingRate => 3,
            Mode::DpiStage => 4,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IndexedMode {
    pub index: u8,
    #[serde(rename = "ledStatus")]
    pub mode: Mode,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "Vec<IndexedMode>", into = "Vec<IndexedMode>")]
pub struct Configuration {
    leds: [Mode; 3],
}
impl Configuration {
    pub fn new(leds: [Mode; 3]) -> Self {
        Self { leds }
    }
    pub fn modes(self) -> [Mode; 3] {
        self.leds
    }
    pub fn mode(self, index: usize) -> Option<Mode> {
        self.leds.get(index).copied()
    }
    pub fn with_mode(mut self, index: usize, mode: Mode) -> anyhow::Result<Self> {
        *self
            .leds
            .get_mut(index)
            .context("Receiver LED index is outside 0..3")? = mode;
        Ok(self)
    }
}
impl Default for Configuration {
    fn default() -> Self {
        Self::new([
            Mode::ConnectionQuality,
            Mode::BatteryLevel,
            Mode::PollingRate,
        ])
    }
}
impl TryFrom<Vec<IndexedMode>> for Configuration {
    type Error = anyhow::Error;
    fn try_from(entries: Vec<IndexedMode>) -> anyhow::Result<Self> {
        ensure!(
            entries.len() == 3,
            "Receiver LED configuration requires three entries"
        );
        let mut modes = [None; 3];
        for entry in entries {
            let slot = modes
                .get_mut(usize::from(entry.index))
                .context("Receiver LED index is outside 0..3")?;
            ensure!(slot.is_none(), "Receiver LED index occurs more than once");
            *slot = Some(entry.mode);
        }
        Ok(Self::new([
            modes[0].unwrap(),
            modes[1].unwrap(),
            modes[2].unwrap(),
        ]))
    }
}
impl From<Configuration> for Vec<IndexedMode> {
    fn from(configuration: Configuration) -> Self {
        configuration
            .leds
            .into_iter()
            .enumerate()
            .map(|(index, mode)| IndexedMode {
                index: index as u8,
                mode,
            })
            .collect()
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "u8", into = "u8")]
pub enum Quality {
    Red,
    Orange,
    Yellow,
    Green,
}
impl Quality {
    pub fn code(self) -> u8 {
        self.into()
    }
    pub fn string(self) -> &'static str {
        match self {
            Self::Red => "Connection_Quality_Status_Red",
            Self::Orange => "Connection_Quality_Status_Orange",
            Self::Yellow => "Connection_Quality_Status_Yellow",
            Self::Green => "Connection_Quality_Status_Green",
        }
    }
}
impl From<Quality> for u8 {
    fn from(value: Quality) -> Self {
        match value {
            Quality::Red => 1,
            Quality::Orange => 2,
            Quality::Yellow => 3,
            Quality::Green => 4,
        }
    }
}
impl TryFrom<u8> for Quality {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> anyhow::Result<Self> {
        Ok(match value {
            1 => Self::Red,
            2 => Self::Orange,
            3 => Self::Yellow,
            4 => Self::Green,
            _ => anyhow::bail!("Unknown dongle connection quality {value}"),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(
    tag = "operation",
    content = "configuration",
    rename_all = "snake_case"
)]
pub enum Change {
    Initialize(Configuration),
    InitializeRaw(Vec<u8>),
    Set(Configuration),
    /// The original protocol setter only checks Array.isArray. Its fixed
    /// three-byte command copies the first three values and zero-fills the rest.
    SetRaw(Vec<u8>),
    ReadConfiguration,
    ReadConnectionQuality,
    ReadSerialNumber,
    ReadBatteryLevel,
    ReadChargingStatus,
}
impl Change {
    pub fn writes(&self) -> bool {
        matches!(
            self,
            Self::Initialize(_) | Self::InitializeRaw(_) | Self::Set(_) | Self::SetRaw(_)
        )
    }
}

/// ECMAScript numeric conversion for JSON configuration values. Both the
/// protocol's Uint8Array and dropdown7734's relational range check use it.
pub fn source_configuration_number(value: Option<&serde_json::Value>) -> f64 {
    use serde_json::Value;
    match value {
        None => f64::NAN,
        Some(Value::Null) => 0.,
        Some(Value::Bool(value)) => u8::from(*value) as f64,
        Some(Value::Number(value)) => value.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(value)) => {
            let value = crate::source_string::trim(value);
            if value.is_empty() {
                return 0.;
            }
            for (prefix, radix) in [
                ("0x", 16),
                ("0X", 16),
                ("0b", 2),
                ("0B", 2),
                ("0o", 8),
                ("0O", 8),
            ] {
                if let Some(digits) = value.strip_prefix(prefix) {
                    if digits.is_empty() {
                        return f64::NAN;
                    }
                    return digits
                        .chars()
                        .try_fold(0., |value, ch| {
                            ch.to_digit(radix)
                                .map(|digit| value * f64::from(radix) + f64::from(digit))
                        })
                        .unwrap_or(f64::NAN);
                }
            }
            match value {
                "Infinity" | "+Infinity" => f64::INFINITY,
                "-Infinity" => f64::NEG_INFINITY,
                _ if value
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || matches!(ch, '+' | '-' | '.' | 'e' | 'E')) =>
                {
                    value.parse().unwrap_or(f64::NAN)
                }
                _ => f64::NAN,
            }
        }
        Some(Value::Array(values)) if values.is_empty() => 0.,
        Some(Value::Array(values)) if values.len() == 1 => match &values[0] {
            Value::Null => 0.,
            Value::String(_) | Value::Number(_) | Value::Array(_) => {
                source_configuration_number(values.first())
            }
            _ => f64::NAN,
        },
        _ => f64::NAN,
    }
}

/// Preserve the feature's complete sorted array separately from the fixed
/// packet. The current source does not validate length, indices or mode enums.
pub fn prepare_source_configuration(
    value: &serde_json::Value,
) -> anyhow::Result<(serde_json::Value, Vec<u8>)> {
    use serde_json::Value;
    let mut entries = value
        .as_array()
        .context("Source receiver configuration is not an array")?
        .clone();
    ensure!(
        entries.iter().all(|entry| !entry.is_null()),
        "Source receiver configuration contains a null entry"
    );
    let less = |a: &Value, b: &Value| {
        source_configuration_number(a.get("index")) - source_configuration_number(b.get("index"))
            < 0.
    };
    // ECMAScript treats a NaN comparison as equality, which is not a total
    // order. Rust's slice sort may panic for that comparator. A stable run and
    // binary insertion keep the source comparator without imposing a schema.
    // The ordering of inconsistent comparisons is implementation-defined by JS.
    let mut sorted = entries.len().min(2);
    if entries.len() > 1 {
        let descending = less(&entries[1], &entries[0]);
        while sorted < entries.len() && less(&entries[sorted], &entries[sorted - 1]) == descending {
            sorted += 1;
        }
        if descending {
            entries[..sorted].reverse();
        }
    }
    for end in sorted..entries.len() {
        let (mut left, mut right) = (0, end);
        while left < right {
            let middle = left + (right - left) / 2;
            if less(&entries[end], &entries[middle]) {
                right = middle;
            } else {
                left = middle + 1;
            }
        }
        entries[left..=end].rotate_right(1);
    }
    let bytes = entries
        .iter()
        .map(|entry| {
            let value = source_configuration_number(entry.get("ledStatus"));
            if value.is_finite() {
                value.trunc().rem_euclid(256.) as u8
            } else {
                0
            }
        })
        .collect();
    Ok((Value::Array(entries), bytes))
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Acknowledgement {
    pub command: [u8; 3],
    pub transaction: u8,
    pub data: Vec<u8>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Report {
    pub requested: Change,
    pub acknowledgements: Vec<Acknowledgement>,
    /// GET result only. Source UI initialization uses the profile sent by SET.
    pub configuration: Option<Configuration>,
    /// Exact getter data, including short arrays and byte values outside the
    /// UI's five choices. Source getter creates one indexed entry per byte.
    #[serde(default)]
    pub raw_configuration: Option<Vec<u8>>,
    /// Protocol ASCII bytes 32..126, without trimming. None means no successful
    /// serial query; a successful empty response is Some("").
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub battery_level: Option<u8>,
    #[serde(default)]
    pub charging_status: Option<String>,
    /// GET diagnostic: its source JSON object fails the feature's string gate.
    pub quality: Option<Quality>,
    pub setter_attempted: bool,
    pub setter_sent: bool,
    pub setter_acknowledged: bool,
    /// Getter/ACK never fabricates a hardware event.
    pub source_quality_update: Option<Quality>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SerialObservation {
    pub ascii: Option<String>,
    pub runtime_serial_number: String,
    pub error: Option<String>,
}
impl SerialObservation {
    pub fn from_report(report: &Report) -> anyhow::Result<Self> {
        ensure!(
            matches!(report.requested, Change::ReadSerialNumber),
            "Serial observation requires its actual protocol query"
        );
        let runtime_serial_number = match report.serial_number.as_deref() {
            Some("") => "NOSERIALNUMBER".into(),
            Some(serial) => serial.to_owned(),
            None => String::new(),
        };
        Ok(Self {
            ascii: report.serial_number.clone(),
            runtime_serial_number,
            error: report.error.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Capability {
    product_id: u32,
    dongle_id: u32,
}
impl Capability {
    pub fn product_id(self) -> u32 {
        self.product_id
    }
    pub fn dongle_id(self) -> u32 {
        self.dongle_id
    }
    pub fn accepts(self, physical: u32, peer: Option<u32>) -> bool {
        peer.is_none() && (physical == self.product_id || physical == self.dongle_id)
    }
    pub fn initialization_allowed(
        self,
        physical: u32,
        protocol_serial: &str,
    ) -> anyhow::Result<bool> {
        ensure!(
            self.accepts(physical, None),
            "Receiver initialization physical factory differs"
        );
        Ok(physical != self.dongle_id || !protocol_serial.is_empty())
    }
    pub fn transport(self, physical: u32) -> anyhow::Result<DeviceReadCapability> {
        ensure!(
            self.accepts(physical, None),
            "Three-LED control requires its current physical factory"
        );
        crate::mouse_current_power::capability(self.product_id)
            .context("Current receiver factory is absent")?
            .transport(physical, false)
    }
}
pub fn capability(product_id: u32) -> Option<Capability> {
    Some(Capability {
        product_id,
        dongle_id: match product_id {
            190 => 191,
            229 => 230,
            239 => 240,
            _ => return None,
        },
    })
}

/// Current Electron bridge/host swallow transport exceptions into undefined.
/// Undefined SEND retries OUT; undefined GET becomes a zero body before the
/// ordinary transaction/status decision. Identity/cancellation guards propagate.
fn exchange(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    command: [u8; 3],
    payload: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
    progress: &mut (bool, bool),
) -> anyhow::Result<Acknowledgement> {
    ensure!(
        cap.report_id == 0
            && cap.report_bytes == 91
            && cap.source_class == "rzDevice25"
            && cap.transaction_prefix == 0
            && cap.transaction_modulus == 31
            && transaction < 31
            && command[0] <= 80
            && cap.max_retry_out > 0
            && cap.max_retry_in > 0,
        "Receiver LED packet/transaction differs from current Protocol25"
    );
    let mut outgoing = [0u8; 91];
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    let payload = &payload[..payload.len().min(usize::from(command[0]))];
    outgoing[9..9 + payload.len()].copy_from_slice(payload);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let mut last = String::from("No receiver LED response");
    for _ in 0..cap.max_retry_out {
        validate()?;
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_ms));
        validate()?;
        progress.0 = true;
        if let Err(error) = device.send_feature_at_collection_max(&outgoing) {
            validate()?;
            last = format!("Receiver LED send failed; delivery is not confirmed: {error:#}");
            continue;
        }
        progress.1 = true;
        validate()?;
        std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
        let mut resend = false;
        for attempt in 0..cap.max_retry_in {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
            }
            validate()?;
            let mut incoming = [0u8; 91];
            // UsbRzDeviceAction preserves SEND reportId=0 with ??, but GET's
            // truthiness fallback selects claimInterface when reportId is zero.
            incoming[0] = if cap.report_id != 0 {
                cap.report_id
            } else {
                cap.claim_interface
            };
            let received = device.get_feature_at_collection_max(&mut incoming);
            validate()?;
            let count = match received {
                Ok(count) => count,
                Err(_) => {
                    incoming.fill(0);
                    incoming.len()
                }
            };
            ensure!(
                count <= incoming.len(),
                "Receiver LED reply exceeds its buffer"
            );
            if count < 9 {
                last = "Receiver LED reply has no complete header".into();
                resend = true;
                break;
            }
            if incoming[2] != transaction || incoming[7..9] != command[1..] {
                last = "Receiver LED response transaction/command differs".into();
                resend = true;
                break;
            }
            match incoming[1] {
                1 => {
                    last = "Receiver is busy".into();
                    resend = true;
                    continue;
                }
                0 | 3 | 4 => {
                    last = format!("Receiver LED status {}", incoming[1]);
                    resend = true;
                    break;
                }
                2 => {
                    let end = (9 + usize::from(incoming[6])).min(count);
                    return Ok(Acknowledgement {
                        command,
                        transaction,
                        data: incoming[9..end].to_vec(),
                    });
                }
                other => anyhow::bail!(
                    "Receiver LED command returned unsupported/unknown status {other}"
                ),
            }
        }
        if !resend {
            break;
        }
    }
    anyhow::bail!("Receiver LED retry failed: {last}")
}

pub fn execute(
    product_id: u32,
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    change: &Change,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    let source = capability(product_id).context("Current three-LED source is absent")?;
    ensure!(
        cap.product_id == product_id && cap.direct_pids.contains(&source.dongle_id),
        "Receiver LED capability belongs to another product"
    );
    validate()?;
    let mut report = Report {
        requested: change.clone(),
        acknowledgements: vec![],
        configuration: None,
        raw_configuration: None,
        serial_number: None,
        battery_level: None,
        charging_status: None,
        quality: None,
        setter_attempted: false,
        setter_sent: false,
        setter_acknowledged: false,
        source_quality_update: None,
        error: None,
    };
    let mut setter_progress = (false, false);
    let result = (|| -> anyhow::Result<()> {
        let bytes = match change {
            Change::Initialize(configuration) | Change::Set(configuration) => {
                Some(configuration.modes().map(u8::from).to_vec())
            }
            Change::InitializeRaw(bytes) | Change::SetRaw(bytes) => Some(bytes.clone()),
            _ => None,
        };
        if let Some(bytes) = bytes {
            let ack = exchange(
                device,
                cap,
                SET_CONFIGURATION,
                &bytes,
                next()?,
                &validate,
                &mut setter_progress,
            )?;
            report.acknowledgements.push(ack);
            report.setter_acknowledged = true;
        }
        if matches!(change, Change::ReadConfiguration) {
            let ack = exchange(
                device,
                cap,
                GET_CONFIGURATION,
                &[],
                next()?,
                &validate,
                &mut (false, false),
            )?;
            report.acknowledgements.push(ack.clone());
            report.raw_configuration = Some(ack.data.clone());
            if let [a, b, c] = ack.data.as_slice()
                && let (Ok(a), Ok(b), Ok(c)) =
                    (Mode::try_from(*a), Mode::try_from(*b), Mode::try_from(*c))
            {
                report.configuration = Some(Configuration::new([a, b, c]));
            }
        }
        if matches!(change, Change::ReadSerialNumber) {
            let ack = exchange(
                device,
                cap,
                GET_SERIAL_NUMBER,
                &[],
                next()?,
                &validate,
                &mut (false, false),
            )?;
            report.serial_number = Some(
                ack.data
                    .iter()
                    .copied()
                    .filter(|byte| (32..127).contains(byte))
                    .map(char::from)
                    .collect(),
            );
            report.acknowledgements.push(ack);
        }
        if matches!(
            change,
            Change::ReadBatteryLevel | Change::ReadChargingStatus
        ) {
            let command = if matches!(change, Change::ReadBatteryLevel) {
                GET_BATTERY_LEVEL
            } else {
                GET_CHARGING_STATUS
            };
            let ack = exchange(
                device,
                cap,
                command,
                &[0, 0],
                next()?,
                &validate,
                &mut (false, false),
            )?;
            if matches!(change, Change::ReadBatteryLevel) {
                report.battery_level = ack
                    .data
                    .get(1)
                    .map(|value| (u16::from(*value) * 100 / 255) as u8);
            } else {
                report.charging_status = ack
                    .data
                    .get(1)
                    .and_then(|value| charging_status(*value))
                    .map(str::to_owned);
            }
            report.acknowledgements.push(ack);
        }
        if matches!(
            change,
            Change::Initialize(_) | Change::InitializeRaw(_) | Change::ReadConnectionQuality
        ) {
            let ack = exchange(
                device,
                cap,
                GET_CONNECTION_QUALITY,
                &[],
                next()?,
                &validate,
                &mut (false, false),
            )?;
            report.quality = ack
                .data
                .first()
                .copied()
                .and_then(|value| value.try_into().ok());
            report.acknowledgements.push(ack);
        }
        validate()
    })();
    report.setter_attempted = setter_progress.0;
    report.setter_sent = setter_progress.1;
    report.error = result.err().map(|error| format!("{error:#}"));
    Ok(report)
}

pub fn charging_status(value: u8) -> Option<&'static str> {
    Some(match value {
        0 => "NoCharge_BatteryFull",
        1 => "Charging",
        2 => "ChargingFailed",
        3 => "ForeignObjectDetection",
        4 => "FastCharging",
        5 => "FullCharging",
        6 => "ReachChargingLimit",
        _ => return None,
    })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Notification {
    ConnectionQuality {
        quality: Quality,
    },
    /// The original hardware handler re-queries battery/charging; raw byte2
    /// never replaces the battery reducer's level.
    BatteryChanged {
        external_power_removed: bool,
    },
    /// Original handler debounces this trigger by 500ms and posts charging mode
    /// DOCK/NONE before its battery query. The retained session owns the timer.
    ExternalPowerChanged {
        connected: bool,
    },
    WirelessChanged {
        connected: bool,
        /// Legacy selector 9 omits this field. V2 selector 53 reads the
        /// big-endian notification bytes 4/5, independently of the physical
        /// product ID stamped on the outer hardware-event envelope.
        #[serde(default)]
        product_id: Option<u32>,
    },
}
pub fn notification(raw: &[u8]) -> anyhow::Result<Option<Notification>> {
    if !matches!(raw.first(), Some(5 | 9)) {
        return Ok(None);
    }
    let selector = *raw
        .get(1)
        .context("Receiver notification has no event selector")?;
    match selector {
        86 => Ok(quality_event(raw)?.map(|quality| Notification::ConnectionQuality { quality })),
        49 => {
            // The generic device-type filter is inside isDualLink. These three
            // factories do not enable it, even for a secondary report ID9.
            Ok(Some(Notification::BatteryChanged {
                external_power_removed: *raw
                    .get(4)
                    .context("Receiver battery notification has no power-source state")?
                    == 0,
            }))
        }
        12 => Ok(match raw.get(2) {
            Some(0) => Some(Notification::ExternalPowerChanged { connected: false }),
            Some(1) => Some(Notification::ExternalPowerChanged { connected: true }),
            Some(_) => None,
            None => anyhow::bail!("Receiver external-power notification has no state"),
        }),
        9 | 53 => {
            if selector == 53 {
                let kind = *raw
                    .get(3)
                    .context("Receiver wireless notification has no device type")?;
                // V2 checks the string enum against MOUSE even for a non-dual
                // factory; unknown enum values have no string and pass.
                if kind == 0 || (2..=8).contains(&kind) {
                    return Ok(None);
                }
            }
            Ok(Some(Notification::WirelessChanged {
                connected: *raw
                    .get(2)
                    .context("Receiver wireless notification has no state")?
                    == 3,
                product_id: (selector == 53).then(|| {
                    // 52975.U1 is (255 & high) << 8 | (255 & low).
                    // Its bitwise conversions also turn absent bytes into 0.
                    u32::from(u16::from_be_bytes([
                        raw.get(4).copied().unwrap_or(0),
                        raw.get(5).copied().unwrap_or(0),
                    ]))
                }),
            }))
        }
        _ => Ok(None),
    }
}

pub fn quality_event(raw: &[u8]) -> anyhow::Result<Option<Quality>> {
    if !matches!(raw.first(), Some(5 | 9)) {
        return Ok(None);
    }
    if *raw
        .get(1)
        .context("Receiver notification has no event selector")?
        != 86
    {
        return Ok(None);
    }
    // Unknown enum values produce no accepted source UI string.
    Ok(Quality::try_from(
        *raw.get(2)
            .context("Receiver quality notification has no status")?,
    )
    .ok())
}
pub fn monitor(
    events: &mut dyn HardwareEvents,
    validate: &impl Fn() -> anyhow::Result<()>,
    publish: &mut impl FnMut(Quality) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    monitor_notifications(events, validate, &mut |event| {
        if let Notification::ConnectionQuality { quality } = event {
            publish(quality)?;
        }
        Ok(())
    })
}
pub fn monitor_notifications(
    events: &mut dyn HardwareEvents,
    validate: &impl Fn() -> anyhow::Result<()>,
    publish: &mut impl FnMut(Notification) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let result = (|| {
        validate()?;
        events.begin()?;
        loop {
            validate()?;
            let raw = events.receive(Duration::from_millis(50))?;
            validate()?;
            if let Some(raw) = raw {
                if let Some(event) = notification(&raw)? {
                    publish(event)?;
                }
            }
        }
    })();
    let cleanup = events.finish();
    match (result, cleanup) {
        (Ok(()), cleanup) => cleanup,
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("Receiver quality cleanup also failed: {cleanup:#}")))
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    Collection {
        node: crate::backend::HidNode,
        product_id: u32,
    },
    WindowsContainer(DeviceReadTarget),
}
impl Target {
    pub fn product_id(&self) -> u32 {
        match self {
            Self::Collection { product_id, .. } => *product_id,
            Self::WindowsContainer(target) => target.product_id,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EventBatch {
    pub operation_id: String,
    pub updates: Vec<Quality>,
    #[serde(default)]
    pub notifications: Vec<Notification>,
    pub terminal: bool,
    #[serde(default)]
    pub canceled: bool,
    pub error: Option<String>,
}

#[cfg(test)]
#[path = "mouse_indicator_led_tests.rs"]
mod tests;

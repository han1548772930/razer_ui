//! Descriptor-selected hardware HID input, separate from the mapping driver
//! stream.
//!
//! Report IDs are supplied by the current native `reportIDs` descriptor. No
//! product, report ID, capture or device capability is inferred here. The three
//! analog formats retain the original bytes and incomplete native tails.
//! Hardware selectors preserve their report framing and input payloads.
//!
//! Mapping lookup, native threshold/dynamic-key policies and observer setup
//! belong to the caller. A complete analog frame updates the existing shared
//! digital/analog state in one batch. Submission is distinct from completion.

use crate::{
    mapping_native_analog::{AnalogResolution, NativeAnalogOutput},
    mapping_native_events::{
        NativeAnalogBatchError, NativeAnalogBatchUpdate, NativeAnalogEventDispatch,
        NativeEventRuntime, NativeModifierSnapshot,
    },
    mapping_native_lookup::{Input, InputKind, parse_input},
    mapping_native_session::{NativeSession, NativeSessionTransport},
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// HardwareEventEndpointWin::Read at RVA 0x1232F8 uses ReadFile(..., 0x64, ...).
/// This is the hardware callback capacity, not the 304-byte driver stream.
pub const NATIVE_HARDWARE_REPORT_CAPACITY: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHidReportKind {
    AnalogKeyboard,
    AnalogKeyboardV2,
    AnalogKeypad,
    ControlPod,
    CoolingPad,
    RazerEvent,
    RazerKey,
    StreamController,
    HardwareEvent,
}

impl NativeHidReportKind {
    pub const fn selector(self) -> u8 {
        match self {
            Self::AnalogKeyboard => 0,
            Self::AnalogKeyboardV2 => 1,
            Self::AnalogKeypad => 2,
            Self::ControlPod => 3,
            Self::CoolingPad => 4,
            Self::RazerEvent => 5,
            Self::RazerKey => 6,
            Self::StreamController => 7,
            Self::HardwareEvent => 8,
        }
    }

    /// RVA 0xE3BC8 checks the pointer/name table at 0x2EC310. An unknown
    /// name, including the final hardware name, resolves to selector 8.
    pub fn from_native_name(name: &str) -> Self {
        match name {
            "analogKeyboardReportID" => Self::AnalogKeyboard,
            "analogKeyboardV2ReportID" => Self::AnalogKeyboardV2,
            "analogKeypadReportID" => Self::AnalogKeypad,
            "controlPodReportID" => Self::ControlPod,
            "coolingPadReportID" => Self::CoolingPad,
            "razerEventReportID" => Self::RazerEvent,
            "razerKeyReportID" => Self::RazerKey,
            "streamControllerReportID" => Self::StreamController,
            _ => Self::HardwareEvent,
        }
    }

    pub const fn analog_format(self) -> Option<NativeHidAnalogFormat> {
        match self {
            Self::AnalogKeyboard => Some(NativeHidAnalogFormat::V1Sparse),
            Self::AnalogKeyboardV2 => Some(NativeHidAnalogFormat::V2Sparse),
            Self::AnalogKeypad => Some(NativeHidAnalogFormat::V1Dense),
            _ => None,
        }
    }
}

/// Descriptor defaults from AnalogData::Reset (0xE37D0), not device observations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeHidActuation {
    pub make: u16,
    pub release: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeHidRapidTrigger {
    pub make_sensitivity: Option<u16>,
    pub break_sensitivity: Option<u16>,
    /// Presence and value are separate: 0x96D2E consumes presence (+21), while
    /// ordinary rapid dispatch consumes the boolean value (+20).
    pub continuous: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeHidAnalogData {
    pub actuation: NativeHidActuation,
    pub rapid_trigger: NativeHidRapidTrigger,
    pub key_inputs: BTreeMap<u8, Input>,
    pub max_actuation_level: Option<u16>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeHidDescriptor {
    pub bluetooth_le: bool,
    pub dual_linked: bool,
    pub turn_off_analog_to_key: bool,
    pub analog_data: NativeHidAnalogData,
    pub stick_threshold: NativeHidStickThreshold,
    /// Last analog selector in native dictionary iteration (0xE7080).
    pub analog_format: Option<NativeHidAnalogFormat>,
    report_types_available: bool,
    report_types: BTreeMap<u8, NativeHidReportKind>,
}

impl NativeHidDescriptor {
    /// A caller must have source evidence for these bindings and the device
    /// descriptor's current availability. A library's presence is insufficient.
    pub fn new(
        bluetooth_le: bool,
        reports: impl IntoIterator<Item = (u8, NativeHidReportKind)>,
    ) -> Self {
        let mut descriptor = Self {
            bluetooth_le,
            dual_linked: false,
            turn_off_analog_to_key: false,
            analog_data: NativeHidAnalogData::default(),
            stick_threshold: NativeHidStickThreshold::default(),
            analog_format: None,
            report_types_available: true,
            report_types: BTreeMap::new(),
        };
        for (id, kind) in reports {
            if let Some(format) = kind.analog_format() {
                descriptor.analog_format = Some(format);
            }
            descriptor.report_types.insert(id, kind);
        }
        descriptor
    }

    pub fn report_kind(&self, id: u8) -> Option<NativeHidReportKind> {
        self.report_types_available
            .then(|| self.report_types.get(&id).copied())
            .flatten()
    }

    /// 0xE5B6C gates report-map access through the current data provider's
    /// virtual +56 query. 0xE5A74 still reads bluetoothLE independently.
    pub fn with_report_types_available(mut self, available: bool) -> Self {
        self.report_types_available = available;
        self
    }

    /// Compatibility boundary for the native DeviceData descriptor, not an
    /// embedded JSON capability table. RVA 0xE7080 parses decimal uint32
    /// keys, narrows them to a byte, and maps string values with 0xE3BC8.
    /// Chromium dictionary iteration sorts the original string keys; the last
    /// assignment wins when distinct keys narrow to the same report byte.
    pub fn from_native_json(value: &Value) -> Result<Self, NativeHidDescriptorError> {
        let object = value
            .as_object()
            .ok_or(NativeHidDescriptorError::ExpectedObject)?;
        let bluetooth_le = match object.get("bluetoothLE") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err(NativeHidDescriptorError::InvalidBluetoothFlag),
        };
        let mut descriptor = Self::new(bluetooth_le, []);
        descriptor.dual_linked = match object.get("dualLinked") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err(NativeHidDescriptorError::InvalidDualLinkedFlag),
        };
        if let Some(reports) = object.get("reportIDs") {
            let reports = reports
                .as_object()
                .ok_or(NativeHidDescriptorError::InvalidReportMap)?;
            let mut sorted: Vec<_> = reports.iter().collect();
            sorted.sort_unstable_by(|(left, _), (right, _)| left.as_bytes().cmp(right.as_bytes()));
            for (key, name) in sorted {
                let id = native_report_key(key)? as u8;
                let name = name
                    .as_str()
                    .ok_or_else(|| NativeHidDescriptorError::InvalidReportName(key.clone()))?;
                let kind = NativeHidReportKind::from_native_name(name);
                if let Some(format) = kind.analog_format() {
                    descriptor.analog_format = Some(format);
                }
                descriptor.report_types.insert(id, kind);
            }
        }
        descriptor.turn_off_analog_to_key = match object.get("turnOffAnalogToKey") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err(NativeHidDescriptorError::InvalidField("turnOffAnalogToKey")),
        };
        if let Some(analog) = object.get("analogData") {
            descriptor.analog_data = parse_analog_data(analog)?;
        }
        if let Some(stick) = object.get("analogStick") {
            let threshold = descriptor_word(stick, "threshold")?;
            descriptor.stick_threshold = NativeHidStickThreshold::new(threshold)
                .ok_or(NativeHidDescriptorError::InvalidField("threshold"))?;
        }
        Ok(descriptor)
    }

    /// E5AF0 disables passthrough when no descriptor exists. Call on a present
    /// descriptor only; an absent descriptor must not enable fallback inputs.
    pub fn analog_passthrough(&self) -> bool {
        !self.turn_off_analog_to_key
    }
}

/// Current compiled profile analog settings (0xE871E/0xEC124/0xEC634).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeHidProfileAnalog {
    pub actuations: BTreeMap<(u8, bool), NativeHidActuation>,
    pub rapid_triggers: BTreeMap<u8, NativeHidRapidTrigger>,
    pub snaptaps: BTreeMap<u32, crate::mapping_native_snaptap::Policy>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHidDeviceConfig {
    pub descriptor: NativeHidDescriptor,
    pub profile: NativeHidProfileAnalog,
}

impl NativeHidDeviceConfig {
    pub fn from_document(document: &Value) -> Result<Self, NativeHidDescriptorError> {
        let descriptor = NativeHidDescriptor::from_native_json(document)?;
        let engine = crate::mapping_native_lookup::active_profile_engine(document)
            .map_err(|_| NativeHidDescriptorError::InvalidField("activeProfile"))?;
        let mut profile = NativeHidProfileAnalog::default();
        if let Some(rows) = engine.get("actuations") {
            let rows = rows
                .as_array()
                .ok_or(NativeHidDescriptorError::InvalidField("actuations"))?;
            for row in rows {
                // EC42F copies the low byte of a signed int32, as DB873 does
                // for WORD fields. The dictionary-key parser differs.
                let key = descriptor_word(row, "keyId")? as u8;
                let hypershift = match row.get("hypershift") {
                    None => false,
                    Some(Value::Bool(value)) => *value,
                    Some(_) => return Err(NativeHidDescriptorError::InvalidField("hypershift")),
                };
                let actuation = NativeHidActuation {
                    make: descriptor_word(row, "make")?,
                    release: descriptor_word(row, "break")?,
                };
                // EC124 permits independent thresholds, unlike EB505 defaults.
                if profile
                    .actuations
                    .insert((key, hypershift), actuation)
                    .is_some()
                {
                    return Err(NativeHidDescriptorError::DuplicateProfileKey(key));
                }
            }
        }
        if let Some(rows) = engine.get("rapidTriggers") {
            let rows = rows
                .as_object()
                .ok_or(NativeHidDescriptorError::InvalidField("rapidTriggers"))?;
            for (key, row) in rows {
                let key = u8::try_from(native_report_key(key)?)
                    .map_err(|_| NativeHidDescriptorError::InvalidAnalogKey(key.clone()))?;
                let rapid = NativeHidRapidTrigger {
                    make_sensitivity: Some(descriptor_word(row, "makeSensitivity")?),
                    break_sensitivity: Some(descriptor_word(row, "breakSensitivity")?),
                    continuous: match row.get("continuous") {
                        None => None,
                        Some(Value::Bool(value)) => Some(*value),
                        Some(_) => {
                            return Err(NativeHidDescriptorError::InvalidField("continuous"));
                        }
                    },
                };
                if profile.rapid_triggers.insert(key, rapid).is_some() {
                    return Err(NativeHidDescriptorError::DuplicateProfileKey(key));
                }
            }
        }
        // E871E/ECB47: optional dictionary; numeric aliases cannot name the
        // same group twice, and each value must be an integer in 0..4.
        if let Some(rows) = engine.get("snaptaps").and_then(Value::as_object) {
            for (group, value) in rows {
                let group = native_report_key(group)?;
                let value = value
                    .as_u64()
                    .filter(|value| *value < 4)
                    .ok_or(NativeHidDescriptorError::InvalidField("snaptaps"))?;
                if profile
                    .snaptaps
                    .insert(
                        group,
                        crate::mapping_native_snaptap::Policy::from_native(value as u32),
                    )
                    .is_some()
                {
                    return Err(NativeHidDescriptorError::InvalidField("snaptaps"));
                }
            }
        }
        Ok(Self {
            descriptor,
            profile,
        })
    }

    /// Explicit descriptor identities precede the selector's native table.
    /// The caller retains the chosen identity until physical release (7E05C).
    pub fn analog_input(&self, key: u8) -> Option<Input> {
        if let Some(input) = self.descriptor.analog_data.key_inputs.get(&key) {
            return Some(input.clone());
        }
        match self.descriptor.analog_format? {
            NativeHidAnalogFormat::V1Sparse | NativeHidAnalogFormat::V2Sparse => {
                native_keyboard_input(key)
            }
            NativeHidAnalogFormat::V1Dense => native_keypad_input(key),
        }
    }

    /// Thresholds are read from the current profile on every level transition;
    /// the mapping pair has a separate held-input lifetime (0x7E05C).
    pub fn analog_parameters(
        &self,
        key: u8,
        hypershift: bool,
        phase: crate::mapping_native_lookup::Phase,
    ) -> crate::mapping_native_composite::AnalogParameters {
        use crate::mapping_native_composite::{AnalogParameters, RapidParameters};
        use crate::mapping_native_lookup::Phase;
        let defaults = &self.descriptor.analog_data;
        let thresholds = self
            .profile
            .actuations
            .get(&(key, hypershift))
            .copied()
            .unwrap_or(defaults.actuation);
        let actuation = if phase == Phase::Press {
            thresholds.make
        } else {
            thresholds.release
        };
        let base = &defaults.rapid_trigger;
        let rapid = self.profile.rapid_triggers.get(&key).unwrap_or(base);
        // E6242 requires both sensitivities; omitted continuous is inherited
        // from the descriptor, then explicitly becomes false when absent.
        let continuous = rapid.continuous.or(base.continuous).unwrap_or(false);
        let rapid = rapid.make_sensitivity.zip(rapid.break_sensitivity).map(
            |(make_sensitivity, break_sensitivity)| RapidParameters {
                actuation,
                make_sensitivity,
                break_sensitivity,
                continuous,
            },
        );
        AnalogParameters {
            actuation,
            rapid,
            default_continuous: base.continuous.is_some(),
        }
    }
}

// 0x10C352/0x10C37A read this four-byte scancode/flag table at RVA 0x2efc30.
fn native_keyboard_input(key: u8) -> Option<Input> {
    let (scancode, flag) = match key {
        1 => (41, 0),
        2..=13 => (u16::from(key) + 0, 0),
        14 => (125, 0),
        15..=28 => (u16::from(key) - 1, 0),
        29 => (43, 0),
        30 => (58, 0),
        31..=41 => (u16::from(key) - 1, 0),
        42 => (43, 0),
        43 => (28, 0),
        44 => (42, 0),
        45 => (86, 0),
        46..=55 => (u16::from(key) - 2, 0),
        56 => (115, 0),
        57 => (54, 0),
        58 => (29, 0),
        60..=61 => (u16::from(key) - 4, 0),
        62 => (56, 2),
        64 => (29, 2),
        75..=76 => (u16::from(key) + 7, 2),
        79 => (75, 2),
        80 => (71, 2),
        81 => (79, 2),
        83 => (72, 2),
        84 => (80, 2),
        85 => (73, 2),
        86 => (81, 2),
        89 => (77, 2),
        90 => (69, 0),
        91 => (71, 0),
        92 => (75, 0),
        93 => (79, 0),
        95 => (53, 2),
        96 => (72, 0),
        97 => (76, 0),
        98 => (80, 0),
        99 => (82, 0),
        100 => (55, 0),
        101 => (73, 0),
        102 => (77, 0),
        103 => (81, 0),
        104 => (83, 0),
        105 => (74, 0),
        106 => (78, 0),
        107 => (126, 0),
        108 => (28, 2),
        110 => (1, 0),
        112..=121 => (u16::from(key) - 53, 0),
        122..=123 => (u16::from(key) - 35, 0),
        124 => (55, 2),
        125 => (70, 0),
        126 => (29, 4),
        127 => (91, 2),
        129 => (93, 2),
        150 => (123, 0),
        151 => (112, 0),
        154 => (121, 0),
        _ => return None,
    };
    Some(Input::new(InputKind::AnalogKey { scancode, flag }))
}

// 0x10C352/0x10C37A read this four-byte scancode/flag table at RVA 0x2f0030.
fn native_keypad_input(key: u8) -> Option<Input> {
    let (scancode, flag) = match key {
        1..=5 => (u16::from(key) + 1, 0),
        6..=10 => (u16::from(key) + 9, 0),
        11 => (58, 0),
        12..=15 => (u16::from(key) + 18, 0),
        16 => (42, 0),
        17..=19 => (u16::from(key) + 27, 0),
        20 => (57, 0),
        _ => return None,
    };
    Some(Input::new(InputKind::AnalogKey { scancode, flag }))
}

fn descriptor_word(value: &Value, field: &'static str) -> Result<u16, NativeHidDescriptorError> {
    value
        .get(field)
        .and_then(Value::as_i64)
        .and_then(|number| i32::try_from(number).ok())
        .map(|number| number as u16)
        .ok_or(NativeHidDescriptorError::InvalidField(field))
}

fn optional_descriptor_word(
    value: &Value,
    field: &'static str,
) -> Result<Option<u16>, NativeHidDescriptorError> {
    value
        .get(field)
        .map(|_| descriptor_word(value, field))
        .transpose()
}

fn parse_analog_data(value: &Value) -> Result<NativeHidAnalogData, NativeHidDescriptorError> {
    let actuation = NativeHidActuation {
        make: descriptor_word(value, "defaultMakeActuation")?,
        release: descriptor_word(value, "defaultBreakActuation")?,
    };
    if actuation.make < actuation.release {
        return Err(NativeHidDescriptorError::InvalidActuationOrder);
    }
    let mut data = NativeHidAnalogData {
        actuation,
        ..NativeHidAnalogData::default()
    };
    if let Some(rapid) = value.get("rapidTrigger") {
        if !rapid.is_object() {
            return Err(NativeHidDescriptorError::InvalidField("rapidTrigger"));
        }
        data.rapid_trigger = NativeHidRapidTrigger {
            make_sensitivity: optional_descriptor_word(rapid, "makeSensitivity")?,
            break_sensitivity: optional_descriptor_word(rapid, "breakSensitivity")?,
            continuous: match rapid.get("continuous") {
                None => None,
                Some(Value::Bool(value)) => Some(*value),
                Some(_) => return Err(NativeHidDescriptorError::InvalidField("continuous")),
            },
        };
    }
    if let Some(keys) = value.get("keyIdToKeyInput") {
        let keys = keys
            .as_object()
            .ok_or(NativeHidDescriptorError::InvalidField("keyIdToKeyInput"))?;
        let mut sorted: Vec<_> = keys.iter().collect();
        sorted.sort_unstable_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
        for (key, raw) in sorted {
            let id = u8::try_from(native_report_key(key)?)
                .map_err(|_| NativeHidDescriptorError::InvalidAnalogKey(key.clone()))?;
            let object = raw
                .as_object()
                .ok_or_else(|| NativeHidDescriptorError::InvalidAnalogKey(key.clone()))?;
            // The descriptor constructors consume only these base input fields;
            // they do not invoke the mapping parser's interrupts wrapper.
            let fields = ["type", "scancode", "flag", "key", "modifiers", "reportId"];
            let projected = Value::Object(
                object
                    .iter()
                    .filter(|(name, _)| fields.contains(&name.as_str()))
                    .map(|(name, value)| (name.clone(), value.clone()))
                    .collect(),
            );
            let input = parse_input(&projected)
                .map_err(|_| NativeHidDescriptorError::InvalidAnalogKey(key.clone()))?;
            if !matches!(
                input.kind,
                InputKind::AnalogKey { .. } | InputKind::RazerKey { .. }
            ) {
                return Err(NativeHidDescriptorError::InvalidAnalogKey(key.clone()));
            }
            data.key_inputs.insert(id, input);
        }
    }
    data.max_actuation_level = optional_descriptor_word(value, "maxActuationLevel")?;
    Ok(data)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeHidDescriptorError {
    ExpectedObject,
    InvalidBluetoothFlag,
    InvalidDualLinkedFlag,
    InvalidReportMap,
    InvalidReportKey(String),
    InvalidReportName(String),
    InvalidField(&'static str),
    InvalidActuationOrder,
    InvalidAnalogKey(String),
    DuplicateProfileKey(u8),
}

impl fmt::Display for NativeHidDescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid native HID descriptor: {self:?}")
    }
}

impl std::error::Error for NativeHidDescriptorError {}

fn native_report_key(key: &str) -> Result<u32, NativeHidDescriptorError> {
    let digits = key.strip_prefix('+').unwrap_or(key);
    if digits.is_empty() || !digits.bytes().all(|digit| digit.is_ascii_digit()) {
        return Err(NativeHidDescriptorError::InvalidReportKey(key.into()));
    }
    digits
        .parse()
        .map_err(|_| NativeHidDescriptorError::InvalidReportKey(key.into()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHidFraming {
    Usb,
    Ble,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHidAnalogFormat {
    V1Sparse,
    V2Sparse,
    V1Dense,
}

impl NativeHidAnalogFormat {
    pub const fn resolution(self) -> AnalogResolution {
        match self {
            Self::V1Sparse | Self::V1Dense => AnalogResolution::V1,
            Self::V2Sparse => AnalogResolution::V2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHidLevel {
    Actuation(u16),
    /// Only V2 sparse keys 251 and 252 use 0x7EB92's signed WORD branch.
    SignedStick(i16),
}

/// Byte-key hash chain ordering from A03D8/A514A and 60418/60560.
/// Preserving it matters because one key notification can interrupt another.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NativeHidKeyOrder {
    keys: Vec<u8>,
    buckets: usize,
}
impl NativeHidKeyOrder {
    fn insert(&mut self, key: u8) {
        if self.keys.contains(&key) {
            return;
        }
        if self.keys.len() + 1 > self.buckets {
            let mut buckets = if self.buckets == 0 { 2 } else { 2 * self.buckets + 1 };
            while (2..)
                .take_while(|divisor| divisor * divisor <= buckets)
                .any(|divisor| buckets % divisor == 0)
            {
                buckets += 1;
            }
            self.rehash(buckets);
        }
        let bucket = usize::from(key) % self.buckets;
        let at = self
            .keys
            .iter()
            .position(|other| usize::from(*other) % self.buckets == bucket)
            .unwrap_or(0);
        self.keys.insert(at, key);
    }

    fn rehash(&mut self, buckets: usize) {
        self.buckets = buckets;
        let Some(first) = self.keys.first() else {
            return;
        };
        let mut previous_bucket = usize::from(*first) % buckets;
        let mut seen = BTreeSet::from([previous_bucket]);
        for position in 1..self.keys.len() {
            let bucket = usize::from(self.keys[position]) % buckets;
            if bucket == previous_bucket {
                continue;
            }
            if seen.insert(bucket) {
                previous_bucket = bucket;
            } else {
                let key = self.keys.remove(position);
                let head = self
                    .keys
                    .iter()
                    .position(|other| usize::from(*other) % buckets == bucket)
                    .unwrap();
                self.keys.insert(head, key);
            }
        }
    }

    fn remove(&mut self, key: u8) {
        self.keys.retain(|other| *other != key);
    }

    fn restore(&mut self, previous: &Self, key: u8) {
        if self.keys.contains(&key) {
            return;
        }
        let Some(old) = previous.keys.iter().position(|other| *other == key) else {
            return;
        };
        let successor = previous.keys[old + 1..]
            .iter()
            .find_map(|after| self.keys.iter().position(|other| other == after));
        self.keys.insert(successor.unwrap_or(self.keys.len()), key);
        self.buckets = self.buckets.max(previous.buckets);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHidAnalogReport {
    pub format: NativeHidAnalogFormat,
    /// Duplicate sparse keys replace the prior value. Dense zeroes are absent
    /// keys, whereas sparse zero levels remain explicitly present.
    pub levels: BTreeMap<u8, NativeHidLevel>,
    /// Current native hash-chain traversal, including signed stick entries.
    pub key_order: Vec<u8>,
    /// Byte position in the parser's payload, not in the raw outer BLE frame.
    pub terminator_offset: Option<usize>,
    /// Native loops only read complete tuples. The remaining bytes, including
    /// terminator/padding, are retained and never guessed to be another key.
    pub ignored_tail: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeHidPayload {
    Analog(NativeHidAnalogReport),
    ControlPod {
        first_button: u8,
        second_button: u8,
        dial: i8,
    },
    CoolingPad {
        id: u8,
        byte1: u8,
        byte2: u8,
    },
    RazerEvent(Vec<i8>),
    RazerKeys(Vec<u8>),
    StreamController {
        key: u8,
        flag: u8,
    },
    HardwareEvent(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHidPacket {
    pub framing: NativeHidFraming,
    pub report_id: u8,
    pub kind: NativeHidReportKind,
    pub descriptor_binding: bool,
    pub dual_linked: bool,
    pub payload_offset: usize,
    pub payload: NativeHidPayload,
    raw: Vec<u8>,
}

impl NativeHidPacket {
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeHidDecodeError {
    Oversized {
        actual: usize,
        capacity: usize,
    },
    Truncated {
        format: Option<NativeHidAnalogFormat>,
        actual: usize,
        required: usize,
    },
}

impl fmt::Display for NativeHidDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid native hardware HID report: {self:?}")
    }
}

impl std::error::Error for NativeHidDecodeError {}

/// USB RVA 0xA8DE0 reads ID at descriptor bluetoothLE ? 1 : 0; for selectors
/// 0..6 that same boolean strips exactly one byte. BLE 0x53C70 always reads
/// the ID at raw[1], retains the outer byte for ControlPod, and routes
/// StreamController through the ordinary hardware-event consumer.
/// Absent/unavailable descriptors and unbound IDs route to selector 8.
pub fn decode_native_hid_packet(
    framing: NativeHidFraming,
    descriptor: Option<&NativeHidDescriptor>,
    raw: &[u8],
) -> Result<NativeHidPacket, NativeHidDecodeError> {
    if raw.len() > NATIVE_HARDWARE_REPORT_CAPACITY {
        return Err(NativeHidDecodeError::Oversized {
            actual: raw.len(),
            capacity: NATIVE_HARDWARE_REPORT_CAPACITY,
        });
    }
    let offset = match framing {
        NativeHidFraming::Ble => 1,
        NativeHidFraming::Usb => {
            usize::from(descriptor.is_some_and(|descriptor| descriptor.bluetooth_le))
        }
    };
    if raw.len() <= offset {
        return Err(NativeHidDecodeError::Truncated {
            format: None,
            actual: raw.len(),
            required: offset + 1,
        });
    }
    let report_id = raw[offset];
    let binding = descriptor.and_then(|descriptor| descriptor.report_kind(report_id));
    let selected = binding.unwrap_or(NativeHidReportKind::HardwareEvent);
    let kind = match (framing, selected) {
        (NativeHidFraming::Ble, NativeHidReportKind::StreamController) => {
            NativeHidReportKind::HardwareEvent
        }
        _ => selected,
    };
    let payload_offset = match (framing, kind) {
        (NativeHidFraming::Ble, NativeHidReportKind::ControlPod) => 0,
        _ if kind.selector() <= 6 => offset,
        _ => 0,
    };
    let bytes = &raw[payload_offset..];
    let require = |required| {
        if bytes.len() < required {
            Err(NativeHidDecodeError::Truncated {
                format: None,
                actual: bytes.len(),
                required,
            })
        } else {
            Ok(())
        }
    };
    let payload = match kind {
        NativeHidReportKind::AnalogKeyboard
        | NativeHidReportKind::AnalogKeyboardV2
        | NativeHidReportKind::AnalogKeypad => {
            NativeHidPayload::Analog(decode_analog_report(kind.analog_format().unwrap(), bytes)?)
        }
        NativeHidReportKind::ControlPod => {
            require(4)?;
            NativeHidPayload::ControlPod {
                first_button: bytes[1],
                second_button: bytes[2],
                dial: bytes[3] as i8,
            }
        }
        NativeHidReportKind::CoolingPad => {
            require(4)?;
            NativeHidPayload::CoolingPad {
                id: bytes[1],
                byte1: bytes[2],
                byte2: bytes[3],
            }
        }
        // 103814 includes the report byte and stops at the first zero.
        NativeHidReportKind::RazerEvent => NativeHidPayload::RazerEvent(
            bytes
                .iter()
                .copied()
                .take_while(|byte| *byte != 0)
                .map(|byte| byte as i8)
                .collect(),
        ),
        // Retain the entire tail: 7FE46 stops make traversal at zero, but its
        // release membership scan and retained set include bytes after zero.
        NativeHidReportKind::RazerKey => {
            require(1)?;
            NativeHidPayload::RazerKeys(bytes[1..].into())
        }
        NativeHidReportKind::StreamController => {
            require(3)?;
            NativeHidPayload::StreamController {
                key: bytes[1],
                flag: bytes[2],
            }
        }
        NativeHidReportKind::HardwareEvent => NativeHidPayload::HardwareEvent(bytes.into()),
    };
    Ok(NativeHidPacket {
        framing,
        report_id,
        kind,
        descriptor_binding: binding.is_some(),
        dual_linked: descriptor.is_some_and(|descriptor| descriptor.dual_linked),
        payload_offset,
        payload,
        raw: raw.into(),
    })
}

fn decode_analog_report(
    format: NativeHidAnalogFormat,
    bytes: &[u8],
) -> Result<NativeHidAnalogReport, NativeHidDecodeError> {
    let mut levels = BTreeMap::new();
    let mut order = NativeHidKeyOrder::default();
    let mut position = 1;
    let mut terminator_offset = None;
    if format == NativeHidAnalogFormat::V1Dense {
        for (key, &level) in bytes.iter().enumerate().skip(1) {
            if level != 0 {
                levels.insert(key as u8, NativeHidLevel::Actuation(level as u16));
                order.insert(key as u8);
            }
        }
        position = bytes.len();
    } else {
        let stride = if format == NativeHidAnalogFormat::V1Sparse {
            2
        } else {
            3
        };
        while position + stride <= bytes.len() {
            let key = bytes[position];
            if key == 0 {
                terminator_offset = Some(position);
                break;
            }
            let level = if stride == 2 {
                NativeHidLevel::Actuation(bytes[position + 1] as u16)
            } else if key == 251 || key == 252 {
                NativeHidLevel::SignedStick(i16::from_be_bytes([
                    bytes[position + 1],
                    bytes[position + 2],
                ]))
            } else {
                NativeHidLevel::Actuation(u16::from_be_bytes([
                    bytes[position + 1],
                    bytes[position + 2],
                ]))
            };
            levels.insert(key, level);
            order.insert(key);
            position += stride;
        }
    }
    Ok(NativeHidAnalogReport {
        format,
        levels,
        key_order: order.keys,
        terminator_offset,
        ignored_tail: bytes[position..].into(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHidStickThreshold(u16);

impl Default for NativeHidStickThreshold {
    fn default() -> Self {
        Self(20_000)
    }
}

impl NativeHidStickThreshold {
    /// Native analogStick.threshold parsing at 0xEBEF2 accepts positive signed
    /// WORD values except 32767; the absent descriptor default is 20000.
    pub fn new(value: u16) -> Option<Self> {
        (value > 0 && value < i16::MAX as u16).then_some(Self(value))
    }
    pub const fn value(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeHidProcessingContext {
    /// The caller supplies the current byte_1803811D9 & 1 equivalent.
    pub analog_enabled: bool,
    /// Native key gate is consulted both for updates and missing-key releases.
    pub blocked_keys: BTreeSet<u8>,
    /// Actual availability of the special native stick input object; a report
    /// or PID alone does not establish that availability.
    pub stick_available: bool,
    pub stick_threshold: NativeHidStickThreshold,
    /// Optional current descriptor maximum used by 0x7E05C before comparison.
    pub actuation_ceiling: Option<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHidAnalogPhase {
    Actuation { first: bool },
    Released,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHidAnalogEvent {
    pub key_id: u8,
    pub resolution: AnalogResolution,
    pub raw_level: u16,
    pub level: u16,
    pub previous_level: u16,
    pub modifiers: u32,
    pub phase: NativeHidAnalogPhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHidAnalogMapping {
    pub input_id: String,
    pub output: NativeAnalogOutput,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeHidKeyResolution {
    Mapped(Vec<NativeHidAnalogMapping>),
    Observed,
    Unresolved(String),
}

pub trait NativeHidHandler {
    /// Source-verified input/actuation/mapped-output lookup. This is called on
    /// first input, a level change and missing-key release, never an invented
    /// capture or observer notification. Full native dynamic-key rules remain
    /// the caller's responsibility. Keep resolution side effects transactional.
    fn resolve_analog(&mut self, event: NativeHidAnalogEvent) -> NativeHidKeyResolution;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHidSignedTransition {
    pub key_id: u8,
    pub value: i16,
    /// 251: positive=1, otherwise=0; 252: positive=2, otherwise=3.
    pub direction: u8,
    pub released: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeHidRuntimeError {
    Unresolved {
        key_id: u8,
        reason: String,
    },
    ResolutionMismatch {
        key_id: u8,
        report: AnalogResolution,
        output: AnalogResolution,
    },
    Output(NativeAnalogBatchError),
}

impl fmt::Display for NativeHidRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native hardware HID dispatch failed: {self:?}")
    }
}

impl std::error::Error for NativeHidRuntimeError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHidInputEvent {
    pub input: Input,
    /// ControlPod dial repeats one resolved action abs(signed dial) times.
    pub repetitions: u8,
    /// Release cleanup continues while the global mapping switch is disabled.
    pub mapping_allowed: bool,
    /// Whether the native handler invokes PostInputNotify/interrupt processing.
    pub notify_input: bool,
}

impl NativeHidSignedTransition {
    pub fn input(&self) -> Input {
        Input::new(InputKind::AnalogStick {
            axis: self.direction,
            flag: u8::from(self.released),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHidFrameStep {
    /// 7eb2a still cleans arbitration after a blocked physical key disappears.
    AnalogCleanup(u8),
    Analog(NativeHidAnalogEvent),
    Signed(NativeHidSignedTransition),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHidFrameDispatch {
    pub packet: NativeHidPacket,
    pub analog_events: Vec<NativeHidAnalogEvent>,
    pub steps: Vec<NativeHidFrameStep>,
    pub input_events: Vec<NativeHidInputEvent>,
    /// Special signed input transitions are returned for the native stick
    /// mapping caller. Their remap/OS delivery is not claimed by this module.
    pub signed_transitions: Vec<NativeHidSignedTransition>,
    pub suppressed_keys: Vec<u8>,
    pub output: Option<NativeAnalogEventDispatch>,
}

/// A validated traversal candidate. Dropping it leaves ingress state unchanged.
/// The shared output owner commits it after resolving and applying its events.
#[derive(Clone, Debug)]
pub struct NativeHidPreparedFrame {
    next: NativeHidRuntime,
    dispatch: NativeHidFrameDispatch,
}

impl NativeHidPreparedFrame {
    pub fn dispatch(&self) -> &NativeHidFrameDispatch {
        &self.dispatch
    }

    /// Restore one candidate key after semantic validation fails, without
    /// undoing other independently validated keys from the same HID frame.
    pub fn restore_analog_key(&mut self, previous: &NativeHidRuntime, key: u8) {
        match previous.tracked.get(&key) {
            Some(tracked) => {
                self.next.tracked.insert(key, *tracked);
                self.next
                    .tracked_order
                    .restore(&previous.tracked_order, key);
            }
            None => {
                self.next.tracked.remove(&key);
                self.next.tracked_order.remove(key);
            }
        }
    }

    /// Publish a validated key before its notification can interrupt another
    /// input. The frame owner preserves source ordering for inter-key effects.
    pub fn commit_analog_key(&self, runtime: &mut NativeHidRuntime, key: u8) {
        match self.next.tracked.get(&key) {
            Some(tracked) => {
                runtime.tracked.insert(key, *tracked);
                runtime.tracked_order.insert(key);
            }
            None => {
                runtime.tracked.remove(&key);
                runtime.tracked_order.remove(key);
            }
        }
    }

    pub fn commit_stick_key(&self, runtime: &mut NativeHidRuntime, key: u8) {
        if self.next.stick_keys.contains(&key) {
            runtime.stick_keys.insert(key);
        } else {
            runtime.stick_keys.remove(&key);
        }
    }

    pub fn commit(self, runtime: &mut NativeHidRuntime) -> NativeHidFrameDispatch {
        *runtime = self.next;
        self.dispatch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TrackedAnalogKey {
    level: u16,
}

/// State of the recovered frame ingress. Final joystick/gamepad contributions
/// remain in the same NativeEventRuntime that the driver digital stream uses.
#[derive(Clone, Debug, Default)]
pub struct NativeHidRuntime {
    tracked: BTreeMap<u8, TrackedAnalogKey>,
    tracked_order: NativeHidKeyOrder,
    stick_keys: BTreeSet<u8>,
    razer_keys: BTreeMap<u8, BTreeSet<u8>>,
}

impl NativeHidRuntime {
    pub fn tracked_level(&self, key: u8) -> Option<u16> {
        self.tracked.get(&key).map(|key| key.level)
    }
    pub fn stick_pressed(&self, key: u8) -> bool {
        self.stick_keys.contains(&key)
    }

    /// Clear ingress tracking after the shared output/session disconnect cleanup
    /// has been driven. This method sends no reports and is not a device reset.
    pub fn reset_after_disconnect(&mut self) {
        self.tracked.clear();
        self.tracked_order = NativeHidKeyOrder::default();
        self.stick_keys.clear();
        self.razer_keys.clear();
    }

    pub fn dispatch<T: NativeSessionTransport, H: NativeHidHandler>(
        &mut self,
        runtime: &mut NativeEventRuntime,
        session: &mut NativeSession<T>,
        handler: &mut H,
        packet: NativeHidPacket,
        context: &NativeHidProcessingContext,
        modifiers: NativeModifierSnapshot,
    ) -> Result<NativeHidFrameDispatch, NativeHidRuntimeError> {
        // Empty batches validate cancellation without changing output state.
        runtime
            .apply_analog_batch(session, &[])
            .map_err(NativeHidRuntimeError::Output)?;
        let mut prepared = self.prepare_frame(packet, context, modifiers);
        let mut updates = Vec::new();
        for &event in &prepared.dispatch.analog_events {
            collect_update(handler, event, &mut updates)?;
        }
        if !updates.is_empty() {
            prepared.dispatch.output = Some(
                runtime
                    .apply_analog_batch(session, &updates)
                    .map_err(NativeHidRuntimeError::Output)?,
            );
        }
        // Lookup/capability validation precedes commit. Transport receipts retain
        // individual submission failures independently of valid desired state.
        Ok(prepared.commit(self))
    }

    /// Traverse a hardware frame without session, output or ingress mutation.
    /// The caller owns lifetime validation and commits only after event handling.
    pub fn prepare_frame(
        &self,
        packet: NativeHidPacket,
        context: &NativeHidProcessingContext,
        modifiers: NativeModifierSnapshot,
    ) -> NativeHidPreparedFrame {
        let mut result = NativeHidFrameDispatch {
            packet,
            analog_events: Vec::new(),
            steps: Vec::new(),
            input_events: Vec::new(),
            signed_transitions: Vec::new(),
            suppressed_keys: Vec::new(),
            output: None,
        };
        let NativeHidPayload::Analog(report) = &result.packet.payload else {
            let mut next = self.clone();
            next.prepare_inputs(&mut result, context.analog_enabled, modifiers.source_bits());
            return NativeHidPreparedFrame {
                next,
                dispatch: result,
            };
        };
        let resolution = report.format.resolution();
        let mut next = self.clone();
        if context.analog_enabled {
            for &key in &report.key_order {
                let level = report.levels[&key];
                if context.blocked_keys.contains(&key) {
                    result.suppressed_keys.push(key);
                    continue;
                }
                match level {
                    NativeHidLevel::SignedStick(value) => {
                        let count = result.signed_transitions.len();
                        next.apply_stick(key, value, context, &mut result.signed_transitions);
                        result.steps.extend(
                            result.signed_transitions[count..]
                                .iter()
                                .copied()
                                .map(NativeHidFrameStep::Signed),
                        );
                    }
                    NativeHidLevel::Actuation(raw_level) => {
                        let level = context
                            .actuation_ceiling
                            .map_or(raw_level, |ceiling| ceiling.min(raw_level));
                        let previous = next.tracked.get(&key);
                        if previous.is_some_and(|previous| previous.level == level) {
                            continue;
                        }
                        let event = NativeHidAnalogEvent {
                            key_id: key,
                            resolution,
                            raw_level,
                            level,
                            previous_level: previous.map_or(0, |previous| previous.level),
                            modifiers: modifiers.source_bits(),
                            phase: NativeHidAnalogPhase::Actuation {
                                first: previous.is_none(),
                            },
                        };
                        result.analog_events.push(event);
                        result.steps.push(NativeHidFrameStep::Analog(event));
                        next.tracked.insert(key, TrackedAnalogKey { level });
                        next.tracked_order.insert(key);
                    }
                }
            }
        }
        // Native release cleanup runs outside the global enabled gate. Sparse
        // zero-valued entries remain present; dense zeroes are missing keys.
        for key in next.tracked_order.keys.clone() {
            let previous = next.tracked[&key];
            if report.levels.contains_key(&key) {
                continue;
            }
            next.tracked.remove(&key);
            next.tracked_order.remove(key);
            if context.blocked_keys.contains(&key) {
                result.suppressed_keys.push(key);
                result.steps.push(NativeHidFrameStep::AnalogCleanup(key));
                continue;
            }
            if resolution == AnalogResolution::V2 && (key == 251 || key == 252) {
                // This branch is present in 0x79E9E's traversal of the normal
                // pressed table. It can be reached through other handlers,
                // but cannot be inferred from the separate signed set alone.
                let count = result.signed_transitions.len();
                next.apply_stick(key, 0, context, &mut result.signed_transitions);
                result.steps.extend(
                    result.signed_transitions[count..]
                        .iter()
                        .copied()
                        .map(NativeHidFrameStep::Signed),
                );
                continue;
            }
            let event = NativeHidAnalogEvent {
                key_id: key,
                resolution,
                raw_level: 0,
                level: 0,
                previous_level: previous.level,
                modifiers: modifiers.source_bits(),
                phase: NativeHidAnalogPhase::Released,
            };
            result.analog_events.push(event);
            result.steps.push(NativeHidFrameStep::Analog(event));
        }
        // 0x7EB92 uses a different held set (+1896), and does not insert into
        // the analog missing-key release table (+1472). Missing signed entries
        // alone therefore cannot establish a native release. Explicit zero or
        // below-threshold signed entries do. Cross-handler cleanup requires its
        // separate source registration and is not inferred from missing entries.
        NativeHidPreparedFrame {
            next,
            dispatch: result,
        }
    }

    /// Nonanalog selectors use their own native held-key and repeat semantics.
    /// Generic hardware packets remain observer payloads, with no lookup input.
    fn prepare_inputs(
        &mut self,
        result: &mut NativeHidFrameDispatch,
        enabled: bool,
        modifiers: u32,
    ) {
        let mut push = |input: Input, repetitions, mapping_allowed, notify_input| {
            result.input_events.push(NativeHidInputEvent {
                input,
                repetitions,
                mapping_allowed,
                notify_input,
            });
        };
        match &result.packet.payload {
            NativeHidPayload::ControlPod {
                first_button,
                second_button,
                dial,
            } => {
                for (source, data) in [(false, *first_button), (true, *second_button)] {
                    if data != 0 {
                        push(
                            Input::new(InputKind::ControlPodButton { source, data }),
                            1,
                            enabled,
                            enabled,
                        );
                    }
                }
                if *dial != 0 {
                    push(
                        Input::new(InputKind::ControlPodDial {
                            clockwise: *dial >= 0,
                        }),
                        dial.unsigned_abs(),
                        enabled,
                        enabled,
                    );
                }
            }
            NativeHidPayload::CoolingPad { id, byte1, byte2 } => {
                push(
                    Input::new(InputKind::CoolingPad {
                        id: *id,
                        byte1: *byte1,
                        byte2: *byte2,
                    }),
                    1,
                    enabled,
                    enabled,
                );
            }
            NativeHidPayload::RazerEvent(bytes) => {
                push(
                    Input::new(InputKind::RazerEvent(bytes.clone())),
                    1,
                    enabled,
                    enabled,
                );
            }
            NativeHidPayload::RazerKeys(keys) => {
                let held = self.razer_keys.entry(result.packet.report_id).or_default();
                if enabled {
                    // Native temporary ordered sets sort and deduplicate events.
                    let makes: BTreeSet<_> = keys
                        .iter()
                        .copied()
                        .take_while(|key| *key != 0)
                        .filter(|key| !held.contains(key))
                        .collect();
                    for key in makes {
                        let mut input = Input::new(InputKind::RazerKey {
                            key: key.into(),
                            flag: 0,
                        });
                        input.modifiers = modifiers;
                        input.report_id = result
                            .packet
                            .dual_linked
                            .then_some(result.packet.report_id.into());
                        push(input, 1, true, true);
                    }
                }
                for &key in held.iter().filter(|key| !keys.contains(key)) {
                    let mut input = Input::new(InputKind::RazerKey {
                        key: key.into(),
                        flag: 1,
                    });
                    input.modifiers = modifiers;
                    input.report_id = result
                        .packet
                        .dual_linked
                        .then_some(result.packet.report_id.into());
                    push(input, 1, true, true);
                }
                *held = keys.iter().copied().filter(|key| *key != 0).collect();
            }
            NativeHidPayload::StreamController { key, flag } => {
                push(
                    Input::new(InputKind::StreamController {
                        key: (*key).into(),
                        flag: (*flag).into(),
                    }),
                    1,
                    enabled || *flag == 1,
                    true,
                );
            }
            NativeHidPayload::HardwareEvent(_) | NativeHidPayload::Analog(_) => {}
        }
    }

    fn apply_stick(
        &mut self,
        key: u8,
        value: i16,
        context: &NativeHidProcessingContext,
        result: &mut Vec<NativeHidSignedTransition>,
    ) {
        if !context.stick_available {
            return;
        }
        let above = value.unsigned_abs() >= context.stick_threshold.value();
        let transition = if above {
            self.stick_keys.insert(key)
        } else {
            self.stick_keys.remove(&key)
        };
        if transition {
            // 0x7EB92 keys the set by key ID only, not by direction. A sign
            // reversal while above threshold causes no second press. Release
            // direction comes from the current value, not a cached sign.
            let direction = if key == 251 {
                u8::from(value > 0)
            } else {
                u8::from(value > 0) ^ 3
            };
            result.push(NativeHidSignedTransition {
                key_id: key,
                value,
                direction,
                released: !above,
            });
        }
    }
}

fn collect_update<H: NativeHidHandler>(
    handler: &mut H,
    event: NativeHidAnalogEvent,
    updates: &mut Vec<NativeAnalogBatchUpdate>,
) -> Result<(), NativeHidRuntimeError> {
    match handler.resolve_analog(event) {
        NativeHidKeyResolution::Observed => {}
        NativeHidKeyResolution::Unresolved(reason) => {
            return Err(NativeHidRuntimeError::Unresolved {
                key_id: event.key_id,
                reason,
            });
        }
        NativeHidKeyResolution::Mapped(mappings) => {
            for mapping in mappings {
                let output_resolution = match &mapping.output {
                    NativeAnalogOutput::Joystick(curve) => curve.resolution,
                    NativeAnalogOutput::Gamepad(curve) => curve.resolution,
                };
                if output_resolution != event.resolution {
                    return Err(NativeHidRuntimeError::ResolutionMismatch {
                        key_id: event.key_id,
                        report: event.resolution,
                        output: output_resolution,
                    });
                }
                updates.push(NativeAnalogBatchUpdate {
                    input_id: mapping.input_id,
                    output: mapping.output,
                    raw_actuation: event.level,
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "mapping_native_hid_tests.rs"]
mod tests;

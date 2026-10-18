//! Descriptor-selected hardware HID input, separate from the mapping driver
//! stream.
//!
//! Report IDs are supplied by the current native `reportIDs` descriptor. No
//! product, report ID, capture or device capability is inferred here. The three
//! recovered analog formats retain the original bytes and incomplete native
//! tails. Other selectors remain explicit unsupported input branches.
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeHidDescriptor {
    pub bluetooth_le: bool,
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
        Self {
            bluetooth_le,
            report_types_available: true,
            report_types: reports.into_iter().collect(),
        }
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
                descriptor
                    .report_types
                    .insert(id, NativeHidReportKind::from_native_name(name));
            }
        }
        Ok(descriptor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeHidDescriptorError {
    ExpectedObject,
    InvalidBluetoothFlag,
    InvalidReportMap,
    InvalidReportKey(String),
    InvalidReportName(String),
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

    const fn minimum_bytes(self) -> usize {
        match self {
            Self::V1Sparse => 3,
            Self::V2Sparse => 4,
            Self::V1Dense => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeHidLevel {
    Actuation(u16),
    /// Only V2 sparse keys 251 and 252 use 0x7EB92's signed WORD branch.
    SignedStick(i16),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHidAnalogReport {
    pub format: NativeHidAnalogFormat,
    /// Duplicate sparse keys replace the prior value. Dense zeroes are absent
    /// keys, whereas sparse zero levels remain explicitly present.
    pub levels: BTreeMap<u8, NativeHidLevel>,
    /// Byte position in the parser's payload, not in the raw outer BLE frame.
    pub terminator_offset: Option<usize>,
    /// Native loops only read complete tuples. The remaining bytes, including
    /// terminator/padding, are retained and never guessed to be another key.
    pub ignored_tail: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeHidPayload {
    Analog(NativeHidAnalogReport),
    /// Retains a source-selected report kind with no payload decoder in this module.
    Unsupported {
        kind: NativeHidReportKind,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeHidPacket {
    pub framing: NativeHidFraming,
    pub report_id: u8,
    pub kind: NativeHidReportKind,
    pub descriptor_binding: bool,
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
/// the ID at raw[1] and strips one byte for the three analog decoders.
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
    let kind = binding.unwrap_or(NativeHidReportKind::HardwareEvent);
    let payload_offset = if kind.analog_format().is_some() { offset } else { 0 };
    let payload = match kind.analog_format() {
        Some(format) => {
            NativeHidPayload::Analog(decode_analog_report(format, &raw[payload_offset..])?)
        }
        None => NativeHidPayload::Unsupported { kind },
    };
    Ok(NativeHidPacket {
        framing,
        report_id,
        kind,
        descriptor_binding: binding.is_some(),
        payload_offset,
        payload,
        raw: raw.into(),
    })
}

fn decode_analog_report(
    format: NativeHidAnalogFormat,
    bytes: &[u8],
) -> Result<NativeHidAnalogReport, NativeHidDecodeError> {
    let required = format.minimum_bytes();
    if bytes.len() < required {
        // Native bodies silently run their missing-key cleanup on short input.
        // Rust rejects it before modifying held state or output contributions.
        return Err(NativeHidDecodeError::Truncated {
            format: Some(format),
            actual: bytes.len(),
            required,
        });
    }
    let mut levels = BTreeMap::new();
    let mut position = 1;
    let mut terminator_offset = None;
    if format == NativeHidAnalogFormat::V1Dense {
        for (key, &level) in bytes.iter().enumerate().skip(1) {
            if level != 0 {
                levels.insert(key as u8, NativeHidLevel::Actuation(level as u16));
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
            position += stride;
        }
    }
    Ok(NativeHidAnalogReport {
        format,
        levels,
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
pub struct NativeHidFrameDispatch {
    pub packet: NativeHidPacket,
    pub analog_events: Vec<NativeHidAnalogEvent>,
    /// Special signed input transitions are returned for the native stick
    /// mapping caller. Their remap/OS delivery is not claimed by this module.
    pub signed_transitions: Vec<NativeHidSignedTransition>,
    pub suppressed_keys: Vec<u8>,
    pub output: Option<NativeAnalogEventDispatch>,
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
    stick_keys: BTreeSet<u8>,
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
        self.stick_keys.clear();
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
        let mut result = NativeHidFrameDispatch {
            packet,
            analog_events: Vec::new(),
            signed_transitions: Vec::new(),
            suppressed_keys: Vec::new(),
            output: None,
        };
        let NativeHidPayload::Analog(report) = &result.packet.payload else {
            return Ok(result);
        };
        // Even observation-only or signed frames must not alter ingress after
        // cancellation. The empty batch checks lifetime without writing or
        // requiring an output capability; resolution has not run yet.
        runtime
            .apply_analog_batch(session, &[])
            .map_err(NativeHidRuntimeError::Output)?;
        let resolution = report.format.resolution();
        let mut next = self.clone();
        let mut updates = Vec::new();
        if context.analog_enabled {
            for (&key, &level) in &report.levels {
                if context.blocked_keys.contains(&key) {
                    result.suppressed_keys.push(key);
                    continue;
                }
                match level {
                    NativeHidLevel::SignedStick(value) => {
                        next.apply_stick(key, value, context, &mut result.signed_transitions);
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
                        collect_update(handler, event, &mut updates)?;
                        result.analog_events.push(event);
                        next.tracked.insert(key, TrackedAnalogKey { level });
                    }
                }
            }
        }
        // Native release cleanup runs outside the global enabled gate. Sparse
        // zero-valued entries remain present; dense zeroes are missing keys.
        for (&key, previous) in &self.tracked {
            if report.levels.contains_key(&key) {
                continue;
            }
            next.tracked.remove(&key);
            if context.blocked_keys.contains(&key) {
                result.suppressed_keys.push(key);
                continue;
            }
            if resolution == AnalogResolution::V2 && (key == 251 || key == 252) {
                // This branch is present in 0x79E9E's traversal of the normal
                // pressed table. It can be reached through other handlers,
                // but cannot be inferred from the separate signed set alone.
                next.apply_stick(key, 0, context, &mut result.signed_transitions);
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
            collect_update(handler, event, &mut updates)?;
            result.analog_events.push(event);
        }
        // 0x7EB92 uses a different held set (+1896), and does not insert into
        // the analog missing-key release table (+1472). Missing signed entries
        // alone therefore cannot establish a native release. Explicit zero or
        // below-threshold signed entries do. Cross-handler cleanup requires its
        // separate source registration and is not inferred from missing entries.
        if !updates.is_empty() {
            result.output = Some(
                runtime
                    .apply_analog_batch(session, &updates)
                    .map_err(NativeHidRuntimeError::Output)?,
            );
        }
        // Parsing, lookup, curve resolution and capability errors all occur
        // before committing ingress state. Individual submission failures are
        // retained in output receipts, separately from valid desired state.
        *self = next;
        Ok(result)
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

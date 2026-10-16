//! Current wired keyboard actuation, rapid trigger and Snap Tap Feature commands.
//! Threshold/sensitivity values are raw firmware u16 values, in big-endian order.
//! Caller supplies an observed physical collection, profile ID and session lock.
//! An ACK is kept separate from an optional, independent device readback.
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    feature_exchange,
    keyboard_settings::{self, KeyboardSettingsCapability},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};

pub const ACTIVE_PROFILE_ID: u8 = 1;
pub const ALL_KEYS: u8 = 255;
const DATA_TYPE: u8 = 0;
const ACTUATION_CHUNK: usize = 15;
const RAPID_TRIGGER_CHUNK: usize = 13;
const SNAP_TAP_PAIRS: usize = 4;
const SET_ACTUATION: [u8; 3] = [80, 2, 25];
const GET_ACTUATION: [u8; 3] = [80, 2, 153];
const SET_RAPID_TRIGGER: [u8; 3] = [80, 2, 26];
const GET_RAPID_TRIGGER: [u8; 3] = [80, 2, 154];
const SET_CONTINUOUS: [u8; 3] = [2, 2, 32];
const GET_CONTINUOUS: [u8; 3] = [2, 2, 160];
const SET_KILL_SWITCH: [u8; 3] = [4, 2, 33];
const GET_KILL_SWITCH: [u8; 3] = [4, 2, 161];
const SET_SNAP_TAP: [u8; 3] = [15, 2, 39];
const GET_SNAP_TAP: [u8; 3] = [15, 2, 167];
const SET_TARGET_OBM_KEYMAP: [u8; 3] = [1, 2, 55];

/// Static transport facts for the analog command family. This descriptor is
/// intentionally independent from `KeyboardSettingsCapability`: analog-only
/// products must not silently gain matrix-brightness support just because both
/// features happen to use the wired rzDevice25 report shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardAnalogTransportDescriptor {
    pub product_id: u32,
    pub claim_interface: u8,
    pub claim_interfaces_by_pid: &'static [(u32, u8)],
    pub hardware_profile_slots: u8,
    pub target_keymap_for_per_key: bool,
}

impl KeyboardAnalogTransportDescriptor {
    fn device_read_capability(self) -> DeviceReadCapability {
        DeviceReadCapability {
            product_id: self.product_id,
            source_class: "rzDevice25".into(),
            direct_pids: vec![self.product_id],
            vendor_id: 0x1532,
            claim_interface: self.claim_interface,
            // Analog capabilities expose the wired PID only. The descriptor's
            // scalar already selects it from the original PID map.
            claim_interfaces_by_pid: Default::default(),
            physical_transports_by_pid: Default::default(),
            report_id: 0,
            report_bytes: 91,
            transaction_prefix: 0,
            transaction_modulus: 31,
            queries: Vec::new(),
            polling_codes: std::collections::BTreeMap::new(),
            polling_value_offset: None,
            polling_default_hz: None,
            polling_physical_product_id: None,
            charging_codes: std::collections::BTreeMap::new(),
            max_retry_in: 10,
            max_retry_out: 20,
            sleep_between_out_ms: 5,
            sleep_between_out_in_ms: 5,
            sleep_between_in_ms: 5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardAnalogTransportCapability {
    pub product_id: u32,
    pub analog_v3: bool,
    pub kill_switch: bool,
    pub snap_tap: bool,
    pub category: &'static str,
    pub hardware_profile_slots: u8,
    pub target_keymap_for_per_key: bool,
    pub transport: KeyboardAnalogTransportDescriptor,
    /// A keypad caller must provide the observed target keymap before a
    /// per-key Actuation/Rapid Trigger operation. Static descriptors leave it
    /// unset; `with_target_keymap` returns a per-operation copy.
    pub target_keymap_id: Option<u8>,
}

impl KeyboardAnalogTransportCapability {
    pub fn device_read_capability(&self) -> DeviceReadCapability {
        self.transport.device_read_capability()
    }

    pub fn with_target_keymap(mut self, keymap_id: u8) -> Self {
        self.target_keymap_id = Some(keymap_id);
        self
    }
}

#[derive(Clone, Debug)]
#[doc(hidden)]
pub struct AnalogCapabilityView {
    analog_v3: bool,
    kill_switch: bool,
    snap_tap: bool,
    target_keymap_for_per_key: bool,
    target_keymap_id: Option<u8>,
    transport: DeviceReadCapability,
}

mod sealed {
    pub trait Sealed {}
}

/// Source-audited analog transport, or the compatible original keyboard
/// settings transport. Implementations are sealed so callers cannot grant
/// additional product features by constructing an arbitrary capability view.
pub trait KeyboardAnalogCapabilitySource: sealed::Sealed {
    #[doc(hidden)]
    fn analog_capability_view(&self) -> anyhow::Result<AnalogCapabilityView>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardAnalogCapability {
    pub product_id: u32,
    pub analog_v3: bool,
    pub kill_switch: bool,
    pub snap_tap: bool,
}

include!("keyboard_analog_data_generated.rs");
include!("keyboard_analog_expanded_data_generated.rs");

pub fn capability(product_id: u32) -> Option<&'static KeyboardAnalogCapability> {
    AUDITED_KEYBOARDS
        .iter()
        .find(|cap| cap.product_id == product_id)
}

pub fn transport_capability(product_id: u32) -> Option<KeyboardAnalogTransportCapability> {
    let flags = EXPANDED_KEYBOARDS
        .iter()
        .find(|cap| cap.product_id == product_id)
        .copied()
        .or_else(|| capability(product_id).copied())?;
    let transport = AUDITED_ANALOG_TRANSPORTS
        .iter()
        .find(|descriptor| descriptor.product_id == product_id)
        .copied()?;
    Some(KeyboardAnalogTransportCapability {
        product_id,
        analog_v3: flags.analog_v3,
        kill_switch: flags.kill_switch,
        snap_tap: flags.snap_tap,
        category: if product_id == 747 { "KEYPAD" } else { "KEYBOARD" },
        hardware_profile_slots: transport.hardware_profile_slots,
        target_keymap_for_per_key: transport.target_keymap_for_per_key,
        transport,
        target_keymap_id: None,
    })
}

/// Analog V3 uses the firmware button ID from the observed keyboard inventory.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ActuationPoint {
    pub key_id: u8,
    pub threshold_high: u16,
    pub threshold_low: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RapidTriggerKey {
    pub key_id: u8,
    /// Writes accept 0 or 1; observations retain any returned raw code.
    pub enabled: u8,
    pub up_sensitivity: u16,
    pub down_sensitivity: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[repr(u8)]
pub enum SnapTapMode {
    LastInput = 0,
    PriorityLeft = 1,
    PriorityRight = 2,
    Neutral = 3,
    CompareLevel = 4,
}

/// Snap Tap/kill-switch use `analogKeyID`, distinct from the V3 firmware ID.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct SnapTapPair {
    pub key_id1: u8,
    pub key_id2: u8,
    pub mode: SnapTapMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ObservedSnapTapPair {
    pub key_id1: u8,
    pub key_id2: u8,
    pub raw_mode: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyboardAnalogQuery {
    Actuation { profile_id: u8, key_ids: Vec<u8> },
    RapidTrigger { profile_id: u8, key_ids: Vec<u8> },
    ContinuousRapidTrigger { profile_id: u8 },
    KillSwitch { profile_id: u8 },
    SnapTap { profile_id: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum KeyboardAnalogChange {
    Actuation {
        profile_id: u8,
        points: Vec<ActuationPoint>,
    },
    RapidTrigger {
        profile_id: u8,
        keys: Vec<RapidTriggerKey>,
    },
    ContinuousRapidTrigger {
        profile_id: u8,
        enabled: bool,
    },
    KillSwitch {
        profile_id: u8,
        enabled: bool,
        key_id1: u8,
        key_id2: u8,
    },
    SnapTap {
        profile_id: u8,
        enabled: bool,
        pairs: Vec<SnapTapPair>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum KeyboardAnalogObservation {
    Actuation {
        profile_id: u8,
        points: Vec<ActuationPoint>,
    },
    RapidTrigger {
        profile_id: u8,
        keys: Vec<RapidTriggerKey>,
    },
    ContinuousRapidTrigger {
        profile_id: u8,
        raw_enabled: u8,
    },
    KillSwitch {
        profile_id: u8,
        raw_enabled: u8,
        key_id1: u8,
        key_id2: u8,
    },
    SnapTap {
        profile_id: u8,
        raw_enabled: u8,
        declared_pairs: u8,
        /// Preserve returned zero padding and unknown mode codes.
        pairs: Vec<ObservedSnapTapPair>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KeyboardAnalogAcknowledgement {
    pub command: [u8; 3],
    pub report_data_size: u8,
    pub payload: Vec<u8>,
    /// Actual response bytes, including a valid empty SUCCESS ACK.
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KeyboardAnalogConfirmation {
    pub acknowledgement_index: usize,
    pub observation: KeyboardAnalogObservation,
    pub matches_request: bool,
}

/// Selecting a keypad keymap is a device mutation of its own. A successful
/// acknowledgement does not confirm either the selected map or analog values.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct KeyboardAnalogTargetSelection {
    pub attempted: bool,
    pub transport_send_attempts: usize,
    pub acknowledgement: Option<KeyboardAnalogAcknowledgement>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KeyboardAnalogReadReport {
    pub target_keymap: KeyboardAnalogTargetSelection,
    pub observation: KeyboardAnalogObservation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardAnalogReadFailurePhase {
    Prepare,
    TargetKeymap,
    Read,
}

#[derive(Debug)]
pub struct KeyboardAnalogReadFailure {
    pub phase: KeyboardAnalogReadFailurePhase,
    pub target_keymap: KeyboardAnalogTargetSelection,
    pub error: anyhow::Error,
}

impl KeyboardAnalogReadFailure {
    pub fn device_state_may_have_changed(&self) -> bool {
        self.target_keymap.attempted
    }
}

impl fmt::Display for KeyboardAnalogReadFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Keyboard analog {:?}: {:#}", self.phase, self.error)
    }
}
impl std::error::Error for KeyboardAnalogReadFailure {}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct KeyboardAnalogWriteReport {
    pub prepared_commands: usize,
    /// Separate from analog-value writes, including on partial failure.
    pub target_keymap: KeyboardAnalogTargetSelection,
    /// Number of distinct write commands which actually reached the transport.
    pub attempted_writes: usize,
    /// Includes retry calls of the same command.
    pub transport_send_attempts: usize,
    pub acknowledgements: Vec<KeyboardAnalogAcknowledgement>,
    pub confirmations: Vec<KeyboardAnalogConfirmation>,
}

impl KeyboardAnalogWriteReport {
    pub fn verified(&self) -> bool {
        self.prepared_commands > 0
            && self.attempted_writes == self.prepared_commands
            && self.acknowledgements.len() == self.prepared_commands
            && self.confirmations.len() == self.prepared_commands
            && self.confirmations.iter().all(|c| c.matches_request)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardAnalogFailurePhase {
    Prepare,
    TargetKeymap,
    Write,
    Readback,
}

#[derive(Debug)]
pub struct KeyboardAnalogWriteFailure {
    pub phase: KeyboardAnalogFailurePhase,
    pub report: KeyboardAnalogWriteReport,
    pub error: anyhow::Error,
}

impl KeyboardAnalogWriteFailure {
    pub fn device_state_may_have_changed(&self) -> bool {
        self.report.attempted_writes > 0 || self.report.target_keymap.attempted
    }
}

impl fmt::Display for KeyboardAnalogWriteFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Keyboard {:?}: {:#}", self.phase, self.error)
    }
}
impl std::error::Error for KeyboardAnalogWriteFailure {}

fn validate_legacy_cap(cap: &KeyboardSettingsCapability) -> anyhow::Result<AnalogCapabilityView> {
    let analog = capability(cap.product_id)
        .context("Keyboard product lacks current analog/Snap Tap audit")?;
    let audited =
        keyboard_settings::capability(cap.product_id).context("Missing audited wired transport")?;
    let t = &cap.transport;
    let a = &audited.transport;
    ensure!(
        cap.source_class == audited.source_class
            && cap.hardware_profile_slots == audited.hardware_profile_slots
            && t.product_id == cap.product_id
            && t.source_class == "rzDevice25"
            && t.vendor_id == a.vendor_id
            && t.claim_interface == a.claim_interface
            && t.claim_interfaces_by_pid == a.claim_interfaces_by_pid
            && t.direct_pids.as_slice() == [cap.product_id]
            && t.report_bytes == 91
            && t.report_id == 0
            && t.transaction_prefix == 0
            && t.transaction_modulus == 31
            && t.max_retry_in > 0
            && t.max_retry_out > 0,
        "Keyboard capability differs from the current direct wired branch"
    );
    Ok(AnalogCapabilityView {
        analog_v3: analog.analog_v3,
        kill_switch: analog.kill_switch,
        snap_tap: analog.snap_tap,
        target_keymap_for_per_key: false,
        target_keymap_id: None,
        transport: cap.transport.clone(),
    })
}

impl sealed::Sealed for KeyboardSettingsCapability {}
impl KeyboardAnalogCapabilitySource for KeyboardSettingsCapability {
    fn analog_capability_view(&self) -> anyhow::Result<AnalogCapabilityView> {
        validate_legacy_cap(self)
    }
}

impl sealed::Sealed for KeyboardAnalogTransportCapability {}
impl KeyboardAnalogCapabilitySource for KeyboardAnalogTransportCapability {
    fn analog_capability_view(&self) -> anyhow::Result<AnalogCapabilityView> {
        let audited = transport_capability(self.product_id)
            .context("Product has no audited independent analog transport")?;
        let mut facts = *self;
        facts.target_keymap_id = None;
        ensure!(
            facts == audited,
            "Independent analog capability differs from the current product gate/transport"
        );
        if let Some(keymap_id) = self.target_keymap_id {
            ensure!(
                self.target_keymap_for_per_key && keymap_id > 0,
                "A positive target keymap is valid only for the audited keypad branch"
            );
        }
        Ok(AnalogCapabilityView {
            analog_v3: self.analog_v3,
            kill_switch: self.kill_switch,
            snap_tap: self.snap_tap,
            target_keymap_for_per_key: self.target_keymap_for_per_key,
            target_keymap_id: self.target_keymap_id,
            transport: self.transport.device_read_capability(),
        })
    }
}

fn validate_keys(keys: &[u8], limit: usize, wildcard: bool) -> anyhow::Result<()> {
    ensure!(
        !keys.is_empty() && keys.len() <= limit,
        "Keyboard key batch has invalid length"
    );
    ensure!(
        keys.iter().all(|k| *k > 0 && (wildcard || *k != ALL_KEYS)),
        "Concrete observed key IDs required"
    );
    ensure!(
        keys.iter().copied().collect::<BTreeSet<_>>().len() == keys.len(),
        "Duplicate keyboard key ID"
    );
    Ok(())
}

fn profile(query: &KeyboardAnalogQuery) -> u8 {
    match query {
        KeyboardAnalogQuery::Actuation { profile_id, .. }
        | KeyboardAnalogQuery::RapidTrigger { profile_id, .. }
        | KeyboardAnalogQuery::ContinuousRapidTrigger { profile_id }
        | KeyboardAnalogQuery::KillSwitch { profile_id }
        | KeyboardAnalogQuery::SnapTap { profile_id } => *profile_id,
    }
}

struct Request {
    command: [u8; 3],
    payload: Vec<u8>,
    data_size: u8,
}

fn query_request(
    cap: &AnalogCapabilityView,
    query: &KeyboardAnalogQuery,
) -> anyhow::Result<Request> {
    let profile = profile(query);
    ensure!(
        profile > 0,
        "Active profile is 1; stored profile IDs must come from observed inventory"
    );
    let (command, payload, data_size) = match query {
        KeyboardAnalogQuery::Actuation { key_ids, .. } => {
            ensure!(cap.analog_v3, "Analog V3 is not enabled for this product");
            validate_keys(key_ids, ACTUATION_CHUNK, false)?;
            let mut data = vec![DATA_TYPE, profile];
            for key in key_ids {
                data.extend_from_slice(&[*key, 0, 0, 0, 0]);
            }
            (GET_ACTUATION, data, (2 + 5 * key_ids.len()) as u8)
        }
        KeyboardAnalogQuery::RapidTrigger { key_ids, .. } => {
            ensure!(cap.analog_v3, "Analog V3 is not enabled for this product");
            validate_keys(key_ids, RAPID_TRIGGER_CHUNK, false)?;
            let mut data = vec![DATA_TYPE, profile];
            for key in key_ids {
                data.extend_from_slice(&[*key, 0, 0, 0, 0, 0]);
            }
            (GET_RAPID_TRIGGER, data, (2 + 6 * key_ids.len()) as u8)
        }
        KeyboardAnalogQuery::ContinuousRapidTrigger { .. } => {
            ensure!(
                cap.analog_v3,
                "Continuous RT requires the product's Analog V3 feature"
            );
            (GET_CONTINUOUS, vec![profile], 2)
        }
        KeyboardAnalogQuery::KillSwitch { .. } => {
            ensure!(
                cap.kill_switch,
                "Kill-switch feature is not enabled for this product"
            );
            (GET_KILL_SWITCH, vec![profile], 4)
        }
        KeyboardAnalogQuery::SnapTap { .. } => {
            ensure!(
                cap.snap_tap,
                "Snap Tap feature is not enabled for this product"
            );
            (GET_SNAP_TAP, vec![profile], 15)
        }
    };
    Ok(Request {
        command,
        payload,
        data_size,
    })
}

fn check_returned_keys(returned: &[u8], requested: &[u8]) -> anyhow::Result<()> {
    let a = returned.iter().copied().collect::<BTreeSet<_>>();
    let b = requested.iter().copied().collect::<BTreeSet<_>>();
    ensure!(
        returned.len() == requested.len() && a.len() == returned.len() && a == b,
        "Returned key IDs differ from requested observed keys"
    );
    Ok(())
}

fn parse(query: &KeyboardAnalogQuery, data: &[u8]) -> anyhow::Result<KeyboardAnalogObservation> {
    let profile_id = profile(query);
    match query {
        KeyboardAnalogQuery::Actuation { key_ids, .. } => {
            ensure!(
                data.len() >= 2
                    && data[..2] == [DATA_TYPE, profile_id]
                    && (data.len() - 2) % 5 == 0,
                "Actuation response is truncated or has different dataType/profile"
            );
            let points: Vec<_> = data[2..]
                .chunks_exact(5)
                .map(|r| ActuationPoint {
                    key_id: r[0],
                    threshold_high: u16::from_be_bytes([r[1], r[2]]),
                    threshold_low: u16::from_be_bytes([r[3], r[4]]),
                })
                .collect();
            check_returned_keys(
                &points.iter().map(|p| p.key_id).collect::<Vec<_>>(),
                key_ids,
            )?;
            Ok(KeyboardAnalogObservation::Actuation { profile_id, points })
        }
        KeyboardAnalogQuery::RapidTrigger { key_ids, .. } => {
            ensure!(
                data.len() >= 2
                    && data[..2] == [DATA_TYPE, profile_id]
                    && (data.len() - 2) % 6 == 0,
                "Rapid trigger response is truncated or has different dataType/profile"
            );
            let keys: Vec<_> = data[2..]
                .chunks_exact(6)
                .map(|r| RapidTriggerKey {
                    key_id: r[0],
                    enabled: r[1],
                    up_sensitivity: u16::from_be_bytes([r[2], r[3]]),
                    down_sensitivity: u16::from_be_bytes([r[4], r[5]]),
                })
                .collect();
            check_returned_keys(&keys.iter().map(|p| p.key_id).collect::<Vec<_>>(), key_ids)?;
            Ok(KeyboardAnalogObservation::RapidTrigger { profile_id, keys })
        }
        KeyboardAnalogQuery::ContinuousRapidTrigger { .. } => {
            ensure!(
                data.len() >= 2 && data[0] == profile_id,
                "Continuous RT response is truncated or has a different profile"
            );
            Ok(KeyboardAnalogObservation::ContinuousRapidTrigger {
                profile_id,
                raw_enabled: data[1],
            })
        }
        KeyboardAnalogQuery::KillSwitch { .. } => {
            ensure!(
                data.len() >= 4 && data[0] == profile_id,
                "Kill-switch response is truncated or has a different profile"
            );
            Ok(KeyboardAnalogObservation::KillSwitch {
                profile_id,
                raw_enabled: data[1],
                key_id1: data[2],
                key_id2: data[3],
            })
        }
        KeyboardAnalogQuery::SnapTap { .. } => {
            ensure!(
                data.len() >= 3
                    && data.len() <= 15
                    && data[0] == profile_id
                    && (data.len() - 3) % 3 == 0,
                "Snap Tap response is truncated or has a different profile"
            );
            ensure!(
                usize::from(data[1]) <= SNAP_TAP_PAIRS
                    && 3 + usize::from(data[1]) * 3 <= data.len(),
                "Snap Tap pair count exceeds the actual response"
            );
            let pairs = data[3..]
                .chunks_exact(3)
                .map(|r| ObservedSnapTapPair {
                    key_id1: r[0],
                    key_id2: r[1],
                    raw_mode: r[2],
                })
                .collect();
            Ok(KeyboardAnalogObservation::SnapTap {
                profile_id,
                raw_enabled: data[2],
                declared_pairs: data[1],
                pairs,
            })
        }
    }
}

fn requires_target_keymap(query: &KeyboardAnalogQuery) -> bool {
    matches!(
        query,
        KeyboardAnalogQuery::Actuation { .. } | KeyboardAnalogQuery::RapidTrigger { .. }
    )
}

fn require_target_keymap(cap: &AnalogCapabilityView) -> anyhow::Result<Option<u8>> {
    if !cap.target_keymap_for_per_key {
        return Ok(None);
    }
    let keymap_id = cap
        .target_keymap_id
        .context("Keypad analog operation requires an observed target keymap ID")?;
    ensure!(keymap_id > 0, "Target OBM keymap ID must be positive");
    Ok(Some(keymap_id))
}

fn next_wire_transaction(transaction: u8) -> anyhow::Result<u8> {
    keyboard_settings::wire_transaction(transaction)
}

fn allocate_wire_transaction(
    next_transaction: &mut impl FnMut() -> anyhow::Result<u8>,
    previous: &mut Option<u8>,
) -> anyhow::Result<u8> {
    let transaction = next_wire_transaction(next_transaction()?)?;
    ensure!(
        *previous != Some(transaction),
        "Distinct commands require separately allocated wire transactions"
    );
    *previous = Some(transaction);
    Ok(transaction)
}

fn set_target_obm_keymap(
    device: &dyn FeatureTransport,
    cap: &AnalogCapabilityView,
    keymap_id: u8,
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
    selection: &mut KeyboardAnalogTargetSelection,
) -> anyhow::Result<()> {
    let data = feature_exchange::exchange_with_report_data_size_observing_send(
        device,
        &cap.transport,
        SET_TARGET_OBM_KEYMAP,
        &[keymap_id],
        next_wire_transaction(transaction)?,
        1,
        validate,
        &mut || {
            selection.attempted = true;
            selection.transport_send_attempts += 1;
        },
    )?;
    selection.acknowledgement = Some(KeyboardAnalogAcknowledgement {
        command: SET_TARGET_OBM_KEYMAP,
        report_data_size: 1,
        payload: vec![keymap_id],
        data,
    });
    Ok(())
}

fn read_prepared(
    device: &dyn FeatureTransport,
    cap: &AnalogCapabilityView,
    query: &KeyboardAnalogQuery,
    request: &Request,
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KeyboardAnalogObservation> {
    let data = feature_exchange::exchange_with_report_data_size(
        device,
        &cap.transport,
        request.command,
        &request.payload,
        next_wire_transaction(transaction)?,
        request.data_size,
        validate,
    )?;
    parse(query, &data)
}

/// A single-command read. Keypad per-key queries require [`read_scoped`],
/// because selecting the target keymap is an additional device mutation that
/// must be recorded and must consume a transaction from the caller's allocator.
pub fn read<C: KeyboardAnalogCapabilitySource>(
    device: &dyn FeatureTransport,
    cap: &C,
    query: KeyboardAnalogQuery,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<KeyboardAnalogObservation> {
    let analog = cap.analog_capability_view()?;
    let r = query_request(&analog, &query)?;
    ensure!(
        !requires_target_keymap(&query) || !analog.target_keymap_for_per_key,
        "Keypad per-key reads require read_scoped with an observed target keymap"
    );
    read_prepared(device, &analog, &query, &r, transaction, &validate)
}

/// Select the observed keypad keymap, then perform the requested read while the
/// caller keeps the same physical collection/session lock. Both transactions
/// are allocated and validated before I/O. A failed read retains target SET
/// attempts and its raw ACK; the SET result is never reported as a readback.
pub fn read_scoped<C: KeyboardAnalogCapabilitySource>(
    device: &dyn FeatureTransport,
    cap: &C,
    query: KeyboardAnalogQuery,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> Result<KeyboardAnalogReadReport, KeyboardAnalogReadFailure> {
    let mut selection = KeyboardAnalogTargetSelection::default();
    let prepared = (|| {
        let analog = cap.analog_capability_view()?;
        let request = query_request(&analog, &query)?;
        let keymap_id = if requires_target_keymap(&query) {
            require_target_keymap(&analog)?
        } else {
            None
        };
        let mut previous = None;
        let select_transaction = if keymap_id.is_some() {
            Some(allocate_wire_transaction(
                &mut next_transaction,
                &mut previous,
            )?)
        } else {
            None
        };
        let read_transaction = allocate_wire_transaction(&mut next_transaction, &mut previous)?;
        Ok((
            analog,
            request,
            keymap_id,
            select_transaction,
            read_transaction,
        ))
    })();
    let (analog, request, keymap_id, select_transaction, read_transaction) =
        prepared.map_err(|error| KeyboardAnalogReadFailure {
            phase: KeyboardAnalogReadFailurePhase::Prepare,
            target_keymap: selection.clone(),
            error,
        })?;
    if let Some(keymap_id) = keymap_id {
        if let Err(error) = set_target_obm_keymap(
            device,
            &analog,
            keymap_id,
            select_transaction.expect("prepared target transaction"),
            &validate,
            &mut selection,
        ) {
            return Err(KeyboardAnalogReadFailure {
                phase: KeyboardAnalogReadFailurePhase::TargetKeymap,
                target_keymap: selection,
                error,
            });
        }
    }
    let observation = match read_prepared(
        device,
        &analog,
        &query,
        &request,
        read_transaction,
        &validate,
    ) {
        Ok(observation) => observation,
        Err(error) => {
            return Err(KeyboardAnalogReadFailure {
                phase: KeyboardAnalogReadFailurePhase::Read,
                target_keymap: selection,
                error,
            });
        }
    };
    Ok(KeyboardAnalogReadReport {
        target_keymap: selection,
        observation,
    })
}

struct WriteRequest {
    request: Request,
    query: KeyboardAnalogQuery,
    expected: KeyboardAnalogObservation,
}

fn prepare(
    cap: &AnalogCapabilityView,
    change: &KeyboardAnalogChange,
    verify_readback: bool,
) -> anyhow::Result<Vec<WriteRequest>> {
    let mut requests = Vec::new();
    match change {
        KeyboardAnalogChange::Actuation { profile_id, points } => {
            ensure!(
                cap.analog_v3 && *profile_id > 0,
                "Analog V3 profile is not supported"
            );
            validate_keys(
                &points.iter().map(|p| p.key_id).collect::<Vec<_>>(),
                255,
                true,
            )?;
            ensure!(
                !verify_readback || !points.iter().any(|p| p.key_id == ALL_KEYS),
                "All-key verification requires concrete observed keys"
            );
            for chunk in points.chunks(ACTUATION_CHUNK) {
                let mut payload = vec![DATA_TYPE, *profile_id];
                for point in chunk {
                    payload.push(point.key_id);
                    payload.extend_from_slice(&point.threshold_high.to_be_bytes());
                    payload.extend_from_slice(&point.threshold_low.to_be_bytes());
                }
                requests.push(WriteRequest {
                    request: Request {
                        command: SET_ACTUATION,
                        data_size: payload.len() as u8,
                        payload,
                    },
                    query: KeyboardAnalogQuery::Actuation {
                        profile_id: *profile_id,
                        key_ids: chunk.iter().map(|p| p.key_id).collect(),
                    },
                    expected: KeyboardAnalogObservation::Actuation {
                        profile_id: *profile_id,
                        points: chunk.to_vec(),
                    },
                });
            }
        }
        KeyboardAnalogChange::RapidTrigger { profile_id, keys } => {
            ensure!(
                cap.analog_v3 && *profile_id > 0,
                "Analog V3 profile is not supported"
            );
            validate_keys(
                &keys.iter().map(|p| p.key_id).collect::<Vec<_>>(),
                255,
                true,
            )?;
            ensure!(
                keys.iter().all(|p| p.enabled <= 1),
                "Rapid trigger writes require enabled 0 or 1"
            );
            ensure!(
                !verify_readback || !keys.iter().any(|p| p.key_id == ALL_KEYS),
                "All-key verification requires concrete observed keys"
            );
            for chunk in keys.chunks(RAPID_TRIGGER_CHUNK) {
                let mut payload = vec![DATA_TYPE, *profile_id];
                for key in chunk {
                    payload.extend_from_slice(&[key.key_id, key.enabled]);
                    payload.extend_from_slice(&key.up_sensitivity.to_be_bytes());
                    payload.extend_from_slice(&key.down_sensitivity.to_be_bytes());
                }
                requests.push(WriteRequest {
                    request: Request {
                        command: SET_RAPID_TRIGGER,
                        data_size: payload.len() as u8,
                        payload,
                    },
                    query: KeyboardAnalogQuery::RapidTrigger {
                        profile_id: *profile_id,
                        key_ids: chunk.iter().map(|p| p.key_id).collect(),
                    },
                    expected: KeyboardAnalogObservation::RapidTrigger {
                        profile_id: *profile_id,
                        keys: chunk.to_vec(),
                    },
                });
            }
        }
        KeyboardAnalogChange::ContinuousRapidTrigger {
            profile_id,
            enabled,
        } => {
            let query = KeyboardAnalogQuery::ContinuousRapidTrigger {
                profile_id: *profile_id,
            };
            query_request(cap, &query)?;
            requests.push(WriteRequest {
                request: Request {
                    command: SET_CONTINUOUS,
                    payload: vec![*profile_id, u8::from(*enabled)],
                    data_size: 2,
                },
                query,
                expected: KeyboardAnalogObservation::ContinuousRapidTrigger {
                    profile_id: *profile_id,
                    raw_enabled: u8::from(*enabled),
                },
            });
        }
        KeyboardAnalogChange::KillSwitch {
            profile_id,
            enabled,
            key_id1,
            key_id2,
        } => {
            let query = KeyboardAnalogQuery::KillSwitch {
                profile_id: *profile_id,
            };
            query_request(cap, &query)?;
            requests.push(WriteRequest {
                request: Request {
                    command: SET_KILL_SWITCH,
                    payload: vec![*profile_id, u8::from(*enabled), *key_id1, *key_id2],
                    data_size: 4,
                },
                query,
                expected: KeyboardAnalogObservation::KillSwitch {
                    profile_id: *profile_id,
                    raw_enabled: u8::from(*enabled),
                    key_id1: *key_id1,
                    key_id2: *key_id2,
                },
            });
        }
        KeyboardAnalogChange::SnapTap {
            profile_id,
            enabled,
            pairs,
        } => {
            let query = KeyboardAnalogQuery::SnapTap {
                profile_id: *profile_id,
            };
            query_request(cap, &query)?;
            ensure!(
                pairs.len() <= SNAP_TAP_PAIRS,
                "Snap Tap command holds at most four pairs"
            );
            let mut payload = vec![*profile_id, pairs.len() as u8, u8::from(*enabled)];
            for pair in pairs {
                payload.extend_from_slice(&[pair.key_id1, pair.key_id2, pair.mode as u8]);
            }
            requests.push(WriteRequest {
                request: Request {
                    command: SET_SNAP_TAP,
                    payload,
                    data_size: 15,
                },
                query,
                expected: KeyboardAnalogObservation::SnapTap {
                    profile_id: *profile_id,
                    raw_enabled: u8::from(*enabled),
                    declared_pairs: pairs.len() as u8,
                    pairs: pairs
                        .iter()
                        .map(|p| ObservedSnapTapPair {
                            key_id1: p.key_id1,
                            key_id2: p.key_id2,
                            raw_mode: p.mode as u8,
                        })
                        .collect(),
                },
            });
        }
    }
    Ok(requests)
}

fn matches_request(
    expected: &KeyboardAnalogObservation,
    observed: &KeyboardAnalogObservation,
) -> bool {
    match (expected, observed) {
        (
            KeyboardAnalogObservation::Actuation {
                profile_id: a,
                points: b,
            },
            KeyboardAnalogObservation::Actuation {
                profile_id: c,
                points: d,
            },
        ) => {
            let (mut b, mut d) = (b.clone(), d.clone());
            b.sort_by_key(|p| p.key_id);
            d.sort_by_key(|p| p.key_id);
            a == c && b == d
        }
        (
            KeyboardAnalogObservation::RapidTrigger {
                profile_id: a,
                keys: b,
            },
            KeyboardAnalogObservation::RapidTrigger {
                profile_id: c,
                keys: d,
            },
        ) => {
            let (mut b, mut d) = (b.clone(), d.clone());
            b.sort_by_key(|p| p.key_id);
            d.sort_by_key(|p| p.key_id);
            a == c && b == d
        }
        (
            KeyboardAnalogObservation::SnapTap {
                profile_id: a,
                raw_enabled: b,
                declared_pairs: c,
                pairs: d,
            },
            KeyboardAnalogObservation::SnapTap {
                profile_id: e,
                raw_enabled: f,
                declared_pairs: g,
                pairs: h,
            },
        ) => {
            a == e
                && b == f
                && c == g
                && h.len() >= d.len()
                && &h[..d.len()] == d.as_slice()
                && h[d.len()..]
                    .iter()
                    .all(|p| p.key_id1 == 0 && p.key_id2 == 0 && p.raw_mode == 0)
        }
        _ => expected == observed,
    }
}

/// Per-command optional readback is an explicit Rust confirmation policy.
/// Stored slot IDs must be obtained from device inventory by the caller.
/// A keypad per-key write first selects the explicitly observed keymap once.
/// Target selection has independent attempts/ACKs and is retained on failure.
/// Every SET/GET consumes a transaction from the caller's allocator.
pub fn write<C: KeyboardAnalogCapabilitySource>(
    device: &dyn FeatureTransport,
    cap: &C,
    change: KeyboardAnalogChange,
    verify_readback: bool,
    mut next_transaction: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> Result<KeyboardAnalogWriteReport, KeyboardAnalogWriteFailure> {
    let mut report = KeyboardAnalogWriteReport::default();
    let analog = match cap.analog_capability_view() {
        Ok(analog) => analog,
        Err(error) => {
            return Err(KeyboardAnalogWriteFailure {
                phase: KeyboardAnalogFailurePhase::Prepare,
                report,
                error,
            });
        }
    };
    let requests = match prepare(&analog, &change, verify_readback) {
        Ok(requests) => requests,
        Err(error) => {
            return Err(KeyboardAnalogWriteFailure {
                phase: KeyboardAnalogFailurePhase::Prepare,
                report,
                error,
            });
        }
    };
    let target_keymap_id = if matches!(
        change,
        KeyboardAnalogChange::Actuation { .. } | KeyboardAnalogChange::RapidTrigger { .. }
    ) {
        match require_target_keymap(&analog) {
            Ok(keymap_id) => keymap_id,
            Err(error) => {
                return Err(KeyboardAnalogWriteFailure {
                    phase: KeyboardAnalogFailurePhase::Prepare,
                    report,
                    error,
                });
            }
        }
    } else {
        None
    };
    report.prepared_commands = requests.len();
    let mut previous_transaction = None;
    if let Some(keymap_id) = target_keymap_id {
        let transaction =
            match allocate_wire_transaction(&mut next_transaction, &mut previous_transaction) {
                Ok(transaction) => transaction,
                Err(error) => {
                    return Err(KeyboardAnalogWriteFailure {
                        phase: KeyboardAnalogFailurePhase::TargetKeymap,
                        report,
                        error,
                    });
                }
            };
        if let Err(error) = set_target_obm_keymap(
            device,
            &analog,
            keymap_id,
            transaction,
            &validate,
            &mut report.target_keymap,
        ) {
            return Err(KeyboardAnalogWriteFailure {
                phase: KeyboardAnalogFailurePhase::TargetKeymap,
                report,
                error,
            });
        }
    }
    for r in requests {
        let transaction =
            match allocate_wire_transaction(&mut next_transaction, &mut previous_transaction) {
                Ok(t) => t,
                Err(error) => {
                    return Err(KeyboardAnalogWriteFailure {
                        phase: KeyboardAnalogFailurePhase::Write,
                        report,
                        error,
                    });
                }
            };
        let mut touched = false;
        let ack = feature_exchange::exchange_with_report_data_size_observing_send(
            device,
            &analog.transport,
            r.request.command,
            &r.request.payload,
            transaction,
            r.request.data_size,
            &validate,
            &mut || {
                if !touched {
                    report.attempted_writes += 1;
                    touched = true;
                }
                report.transport_send_attempts += 1;
            },
        );
        let data = match ack {
            Ok(data) => data,
            Err(error) => {
                return Err(KeyboardAnalogWriteFailure {
                    phase: KeyboardAnalogFailurePhase::Write,
                    report,
                    error,
                });
            }
        };
        let index = report.acknowledgements.len();
        report.acknowledgements.push(KeyboardAnalogAcknowledgement {
            command: r.request.command,
            report_data_size: r.request.data_size,
            payload: r.request.payload,
            data,
        });
        if verify_readback {
            // The caller retains the lock throughout this operation. The
            // selected keymap covers all write chunks and their readbacks;
            // an additional target SET would change the audited caller order.
            let observation =
                allocate_wire_transaction(&mut next_transaction, &mut previous_transaction)
                    .and_then(|transaction| {
                        let request = query_request(&analog, &r.query)?;
                        read_prepared(device, &analog, &r.query, &request, transaction, &validate)
                    });
            match observation {
                Ok(observation) => {
                    let matches_request = matches_request(&r.expected, &observation);
                    report.confirmations.push(KeyboardAnalogConfirmation {
                        acknowledgement_index: index,
                        observation,
                        matches_request,
                    });
                    if !matches_request {
                        return Err(KeyboardAnalogWriteFailure {
                            phase: KeyboardAnalogFailurePhase::Readback,
                            report,
                            error: anyhow::anyhow!(
                                "Keyboard readback differs from requested write"
                            ),
                        });
                    }
                }
                Err(error) => {
                    return Err(KeyboardAnalogWriteFailure {
                        phase: KeyboardAnalogFailurePhase::Readback,
                        report,
                        error,
                    });
                }
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
#[path = "keyboard_analog_tests.rs"]
mod tests;

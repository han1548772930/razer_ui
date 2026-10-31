//! Current ordinary rzDevice25 DPI tasks. Stage profile 1 and transient DPI
//! profile 0 are separate source tasks; independent X/Y is a local flag.
//! Physical receiver/BLE branches are admitted only by their source factory.
use crate::mouse_dpi_stages::{DpiStagesDraft, DpiStagesReading, PackedDpiStage};
use crate::{
    backend::FeatureTransport,
    device_reads::{DeviceReadCapability, PhysicalFeatureTransport},
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

pub const PRODUCT_ID: u32 = 190;
#[path = "mouse_current_dpi_ble.rs"]
pub mod ble;
#[cfg(test)]
#[path = "mouse_current_dpi_tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize)]
pub struct Capability {
    product_id: u32,
    minimum: u16,
    maximum: u16,
    cycle_sensitivity: bool,
    stage_level_only: bool,
    wired_pid: u32,
    dongle_pid: Option<u32>,
    transport: DeviceReadCapability,
}

impl Capability {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn minimum(&self) -> u16 {
        self.minimum
    }
    pub fn maximum(&self) -> u16 {
        self.maximum
    }
    pub fn wired_pid(&self) -> u32 {
        self.wired_pid
    }
    pub fn dongle_pid(&self) -> Option<u32> {
        self.dongle_pid
    }
    pub fn transport(&self) -> &DeviceReadCapability {
        &self.transport
    }
    pub fn is_stage_level_only(&self) -> bool {
        self.stage_level_only
    }
    pub fn is_physical_branch(&self, physical_pid: u32, is_dongle: bool) -> bool {
        if is_dongle {
            self.dongle_pid == Some(physical_pid)
                && !self
                    .transport
                    .physical_transports_by_pid
                    .get(&physical_pid.to_string())
                    .is_some_and(|branch| branch.is_duallink)
        } else {
            self.wired_pid == physical_pid
        }
    }

    /// The command owner is inherited by the current DualLink mouse sender.
    /// Binding a master still requires the source peer and two live queries.
    pub fn receiver_session(
        &self,
        physical_pid: u32,
        peer_pid: u32,
    ) -> anyhow::Result<crate::receiver_session::ReceiverSession> {
        crate::receiver_session::ReceiverSession::mouse_with_source(
            &self.transport,
            physical_pid,
            peer_pid,
        )
    }
}

pub fn capability(product_id: u32) -> Option<&'static Capability> {
    static CAPS: OnceLock<Vec<Capability>> = OnceLock::new();
    CAPS.get_or_init(|| {
        let mut capabilities: Vec<Capability> = make_mouse_current_dpi_table();
        // Current182 factory87887 selects ordinary wired and DualLink radio
        // senders. Retain those distinct physical branches for each exchange.
        if let Some(cap) = capabilities.iter_mut().find(|cap| cap.product_id == 182) {
            cap.transport = crate::device_reads::capability(182)
                .expect("current182 sender")
                .clone();
        }
        capabilities
    })
    .iter()
    .find(|cap| cap.product_id == product_id)
}

fn required(product_id: u32) -> anyhow::Result<&'static Capability> {
    capability(product_id).with_context(|| {
        format!("PID{product_id} DPI factory/protocol branch has no current source capability")
    })
}

pub fn transport_capability() -> &'static DeviceReadCapability {
    required(PRODUCT_ID)
        .expect("PID190 current DPI capability")
        .transport()
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DpiPair {
    pub x: u16,
    pub y: u16,
}

/// A source Number keeps its identity across internal IPC. Device confirmation
/// uses strict numeric comparison instead, so NaN never confirms a getter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceNumber(u64);

impl SourceNumber {
    pub fn new(value: f64) -> Self {
        Self(value.to_bits())
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.0)
    }

    pub fn strict_equals_u16(self, observed: u16) -> bool {
        self.value() == f64::from(observed)
    }

    /// JSON.stringify projects non-finite Numbers to null and writes finite
    /// integer Numbers without changing the in-memory command value.
    pub fn file_value(self) -> serde_json::Value {
        let value = self.value();
        if value.is_finite() {
            serde_json::from_str(&crate::source_string::number_text(value))
                .expect("finite source Number JSON")
        } else {
            serde_json::Value::Null
        }
    }

    fn word(self) -> u16 {
        let value = self.value();
        if value.is_finite() {
            (value.trunc().rem_euclid(4_294_967_296.0) as u32) as u16
        } else {
            0
        }
    }
}

impl From<u16> for SourceNumber {
    fn from(value: u16) -> Self {
        Self::new(f64::from(value))
    }
}

impl Serialize for SourceNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.value().is_finite() {
            serializer.serialize_f64(self.value())
        } else {
            #[derive(Serialize)]
            struct NonFinite {
                nonfinite_bits: String,
            }
            NonFinite {
                nonfinite_bits: format!("{:016x}", self.0),
            }
            .serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for SourceNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct NonFinite {
            nonfinite_bits: String,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Finite(f64),
            NonFinite(NonFinite),
        }
        use serde::de::Error as _;
        match Wire::deserialize(deserializer)? {
            Wire::Finite(value) if value.is_finite() => Ok(Self::new(value)),
            Wire::Finite(_) => Err(D::Error::custom("non-finite Number requires an IPC tag")),
            Wire::NonFinite(value) => {
                if value.nonfinite_bits.len() != 16 {
                    return Err(D::Error::custom("invalid source Number bits"));
                }
                let bits =
                    u64::from_str_radix(&value.nonfinite_bits, 16).map_err(D::Error::custom)?;
                let value = Self(bits);
                if value.value().is_finite() {
                    return Err(D::Error::custom("source Number tag must be non-finite"));
                }
                Ok(value)
            }
        }
    }
}

/// Missing/null axes remain unobserved source fields, including after SELECT's
/// JSON clone. They encode as zero but cannot strictly equal an integer getter.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct SourceStage(Option<SourceStageFields>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourceField<T> {
    Missing,
    Null,
    Value(T),
}

impl<T> Default for SourceField<T> {
    fn default() -> Self {
        Self::Missing
    }
}

impl<T> SourceField<T> {
    fn into_option(self) -> Option<T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Missing | Self::Null => None,
        }
    }
    fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl<T: Serialize> Serialize for SourceField<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Value(value) => value.serialize(serializer),
            Self::Missing | Self::Null => serializer.serialize_none(),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for SourceField<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(|value| value.map_or(Self::Null, Self::Value))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct SourceStageFields {
    #[serde(default, skip_serializing_if = "SourceField::is_missing")]
    x: SourceField<SourceNumber>,
    #[serde(default, skip_serializing_if = "SourceField::is_missing")]
    y: SourceField<SourceNumber>,
    #[serde(default, skip_serializing_if = "SourceField::is_missing")]
    visible: SourceField<bool>,
    #[serde(default, skip_serializing_if = "SourceField::is_missing")]
    independent: SourceField<bool>,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}

impl SourceStage {
    pub fn null() -> Self {
        Self(None)
    }

    pub fn is_null(&self) -> bool {
        self.0.is_none()
    }

    pub fn new(
        x: Option<SourceNumber>,
        y: Option<SourceNumber>,
        visible: Option<bool>,
        independent: Option<bool>,
    ) -> Self {
        Self(Some(SourceStageFields {
            x: x.map_or(SourceField::Missing, SourceField::Value),
            y: y.map_or(SourceField::Missing, SourceField::Value),
            visible: visible.map_or(SourceField::Missing, SourceField::Value),
            independent: independent.map_or(SourceField::Missing, SourceField::Value),
            extensions: BTreeMap::new(),
        }))
    }

    pub fn x(&self) -> Option<SourceNumber> {
        self.0.as_ref().and_then(|row| row.x.into_option())
    }
    pub fn y(&self) -> Option<SourceNumber> {
        self.0.as_ref().and_then(|row| row.y.into_option())
    }
    pub fn strict_x_equals(&self, other: &Self) -> anyhow::Result<bool> {
        let left = self
            .0
            .as_ref()
            .context("Source DPI X comparison row is null")?;
        let right = other
            .0
            .as_ref()
            .context("Source DPI X comparison row is null")?;
        Ok(match (left.x, right.x) {
            (SourceField::Missing, SourceField::Missing)
            | (SourceField::Null, SourceField::Null) => true,
            (SourceField::Value(left), SourceField::Value(right)) => left.value() == right.value(),
            _ => false,
        })
    }
    pub fn is_visible(&self) -> Option<bool> {
        self.0.as_ref().and_then(|row| row.visible.into_option())
    }
    pub fn is_independent(&self) -> Option<bool> {
        self.0
            .as_ref()
            .and_then(|row| row.independent.into_option())
    }

    pub fn from_source_file(value: &serde_json::Value) -> anyhow::Result<Self> {
        if let Some(object) = value.as_object() {
            for name in ["x", "y"] {
                ensure!(
                    object
                        .get(name)
                        .is_none_or(|value| value.is_number() || value.is_null()),
                    "Source DPI file axis is not a Number or null"
                );
            }
        }
        Ok(serde_json::from_value(value.clone())?)
    }

    pub fn file_value(&self) -> serde_json::Value {
        let Some(row) = &self.0 else {
            return serde_json::Value::Null;
        };
        let mut fields: serde_json::Map<_, _> = row.extensions.clone().into_iter().collect();
        for (key, value) in [("x", row.x), ("y", row.y)] {
            match value {
                SourceField::Missing => {}
                SourceField::Null => {
                    fields.insert(key.into(), serde_json::Value::Null);
                }
                SourceField::Value(value) => {
                    fields.insert(key.into(), value.file_value());
                }
            }
        }
        for (key, value) in [("visible", row.visible), ("independent", row.independent)] {
            match value {
                SourceField::Missing => {}
                SourceField::Null => {
                    fields.insert(key.into(), serde_json::Value::Null);
                }
                SourceField::Value(value) => {
                    fields.insert(key.into(), value.into());
                }
            }
        }
        fields.into()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SourceStages {
    #[serde(alias = "enable")]
    enabled: bool,
    #[serde(alias = "active")]
    active_stage: usize,
    stages: Vec<SourceStage>,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}

impl SourceStages {
    pub fn new(enabled: bool, active_stage: usize, stages: Vec<SourceStage>) -> Self {
        Self {
            enabled,
            active_stage,
            stages,
            extensions: BTreeMap::new(),
        }
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn active_stage(&self) -> usize {
        self.active_stage
    }
    pub fn stages(&self) -> &[SourceStage] {
        &self.stages
    }
    pub(crate) fn with_visible_count(mut self) -> Self {
        self.extensions.insert(
            "count".into(),
            self.stages
                .iter()
                .filter(|row| row.is_visible() == Some(true))
                .count()
                .into(),
        );
        self
    }
    pub fn with_replaced_stage(&self, index: usize, stage: SourceStage) -> anyhow::Result<Self> {
        let mut next = self.clone();
        *next
            .stages
            .get_mut(index)
            .context("Source DPI row is absent")? = stage;
        Ok(next)
    }
    pub fn from_source_file(value: &serde_json::Value) -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        struct FileStages {
            enable: bool,
            active: usize,
            stages: Vec<serde_json::Value>,
            #[serde(flatten)]
            extensions: BTreeMap<String, serde_json::Value>,
        }
        let file: FileStages = serde_json::from_value(value.clone())?;
        Ok(Self {
            enabled: file.enable,
            active_stage: file.active,
            stages: file
                .stages
                .iter()
                .map(SourceStage::from_source_file)
                .collect::<anyhow::Result<_>>()?,
            extensions: file.extensions,
        })
    }
    pub fn source_file_value(&self) -> serde_json::Value {
        let mut fields: serde_json::Map<_, _> = self.extensions.clone().into_iter().collect();
        fields.insert("enable".into(), self.enabled.into());
        fields.insert("active".into(), self.active_stage.into());
        fields.insert(
            "stages".into(),
            self.stages.iter().map(SourceStage::file_value).collect(),
        );
        fields.into()
    }
}

impl From<&DpiStagesDraft> for SourceStages {
    fn from(draft: &DpiStagesDraft) -> Self {
        Self::new(
            draft.enabled,
            draft.active_stage,
            draft
                .stages
                .iter()
                .map(|row| {
                    SourceStage::new(
                        Some(row.x.into()),
                        Some(row.y.into()),
                        Some(row.visible),
                        Some(row.independent),
                    )
                })
                .collect(),
        )
    }
}

impl From<DpiStagesDraft> for SourceStages {
    fn from(draft: DpiStagesDraft) -> Self {
        Self::from(&draft)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DpiChange {
    pub stages: Option<SourceStages>,
    pub active: Option<DpiPair>,
    /// Active local profile identity used by 30525.Ye to select actual OBM slots.
    #[serde(default)]
    pub profile_guid: Option<String>,
    /// ON_STAGES_INIT has device access, without a local save/version increment.
    #[serde(default)]
    pub initialize: bool,
}

impl DpiChange {
    /// The first 90012.gn identity query precedes source profile selection.
    /// This request carries no profile data and cannot change device settings.
    pub fn is_identity_read(&self) -> bool {
        self.initialize
            && self.stages.is_none()
            && self.active.is_none()
            && self.profile_guid.is_none()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PackedStages {
    pub active_stage: u8,
    pub number_of_active_stages: u8,
    pub records: Vec<PackedSourceStage>,
    #[serde(default)]
    product_id: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct PackedSourceStage {
    pub index: u8,
    pub x: Option<SourceNumber>,
    pub y: Option<SourceNumber>,
    pub z: SourceNumber,
}

impl PackedSourceStage {
    pub fn new(
        index: u8,
        x: Option<SourceNumber>,
        y: Option<SourceNumber>,
        z: SourceNumber,
    ) -> Self {
        Self { index, x, y, z }
    }
}

fn validate_pair(cap: &Capability, value: DpiPair) -> anyhow::Result<()> {
    ensure!(
        (cap.minimum..=cap.maximum).contains(&value.x)
            && (cap.minimum..=cap.maximum).contains(&value.y),
        "DPI is outside PID{}'s source range {}..={}",
        cap.product_id,
        cap.minimum,
        cap.maximum
    );
    Ok(())
}

/// He preserves findIndex=-1 -> activeStage=0 for the hidden-row unlink branch.
/// Disabled staging still declares one record, whose missing JS values become
/// zero bytes. Keep this source edge case observable instead of selecting a row.
pub fn pack(draft: &SourceStages) -> anyhow::Result<PackedStages> {
    pack_for(PRODUCT_ID, draft)
}

pub fn pack_editor_for(product_id: u32, draft: &DpiStagesDraft) -> anyhow::Result<PackedStages> {
    let cap = required(product_id)?;
    ensure!(
        draft.stages.len() == 5,
        "Current source retains exactly five local DPI stages"
    );
    ensure!(
        (1..=draft.stages.len()).contains(&draft.active_stage),
        "DPI selection is outside the local profile"
    );
    for row in &draft.stages {
        validate_pair(cap, DpiPair { x: row.x, y: row.y })?;
    }
    pack_for(product_id, &draft.into())
}

pub fn pack_for(product_id: u32, draft: &SourceStages) -> anyhow::Result<PackedStages> {
    let cap = required(product_id)?;
    let source_stages = matches!(product_id, 190 | 192 | 222 | 226 | 229 | 239);
    if !source_stages {
        ensure!(
            draft.stages.len() == 5 && (1..=5).contains(&draft.active_stage),
            "Current source retains exactly five selected local DPI stages"
        );
        for row in &draft.stages {
            validate_pair(
                cap,
                DpiPair {
                    x: editor_axis(row.x())?,
                    y: editor_axis(row.y())?,
                },
            )?;
        }
    }
    let selected = draft.active_stage.checked_sub(1);
    ensure!(
        source_stages
            || cap.cycle_sensitivity
            || draft.enabled
            || selected.is_some_and(|index| index < draft.stages.len()),
        "DPI selection is outside the source stage list"
    );
    ensure!(
        source_stages
            || draft.stages.iter().enumerate().all(|(index, row)| {
                !(cap.cycle_sensitivity || draft.enabled || Some(index) == selected)
                    || row.0.is_some()
            }),
        "Source DPI task cannot read visibility from a null row"
    );
    let rows: Vec<_> = draft
        .stages
        .iter()
        .enumerate()
        .filter(|(index, row)| {
            row.is_visible() == Some(true)
                && (cap.cycle_sensitivity || draft.enabled || Some(*index) == selected)
        })
        .collect();
    ensure!(
        rows.len() <= 11,
        "DPI stages exceed the protocol record capacity"
    );
    let active_stage = rows
        .iter()
        .position(|(index, _)| Some(*index) == selected)
        .map_or(0, |index| index as u8 + 1);
    let number_of_active_stages = if cap.cycle_sensitivity || draft.enabled {
        rows.len() as u8
    } else {
        1
    };
    let records = rows
        .into_iter()
        .enumerate()
        .map(|(index, (_, row))| PackedSourceStage {
            index: index as u8,
            x: row.x(),
            y: row.y(),
            z: 0u16.into(),
        })
        .collect();
    Ok(PackedStages {
        active_stage,
        number_of_active_stages,
        records,
        product_id,
    })
}

pub fn validate_change(change: &DpiChange) -> anyhow::Result<()> {
    validate_change_for(PRODUCT_ID, change)
}

pub fn validate_change_for(product_id: u32, change: &DpiChange) -> anyhow::Result<()> {
    let cap = required(product_id)?;
    if change.is_identity_read() {
        ensure!(
            matches!(product_id, 182 | 190),
            "This source has no audited initialization identity task"
        );
        return Ok(());
    }
    ensure!(
        change.stages.is_some() || change.active.is_some(),
        "Local XY flags need no device operation"
    );
    if let Some(stages) = &change.stages {
        pack_for(product_id, stages)?;
    }
    if let Some(active) = change.active {
        ensure!(
            product_id != 182,
            "PID182 stage edits do not dispatch the separate transient DPI task"
        );
        validate_pair(cap, active)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Acknowledgement {
    pub command: [u8; 3],
    pub transaction: u8,
    /// Protocol30 request profile; the transport does not report a profile echo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<u8>,
    pub response_bytes: usize,
    pub data: Vec<u8>,
}

fn exchange(
    device: &dyn FeatureTransport,
    cap: &Capability,
    command: [u8; 3],
    data: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Acknowledgement> {
    exchange_observing(
        device,
        cap,
        command,
        data,
        transaction,
        validate,
        &mut |_| {},
    )
}

fn exchange_observing(
    device: &dyn FeatureTransport,
    cap: &Capability,
    command: [u8; 3],
    data: &[u8],
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
    acknowledged: &mut dyn FnMut(&Acknowledgement),
) -> anyhow::Result<Acknowledgement> {
    let validate = &|| {
        validate().map_err(|error| {
            crate::backend::interrupt_exchange(
                error,
                crate::backend::ExchangeInterruption::Validation,
            )
        })
    };
    let (transport, transaction) = cap.transport().for_exchange(device, transaction)?;
    ensure!(
        transaction & 31 < transport.transaction_modulus
            && transaction & !31 == transport.transaction_prefix
            && transport.report_bytes == 91
            && data.len() <= usize::from(command[0])
            && command[0] <= 80,
        "DPI packet or transaction is outside its source layout"
    );
    let mut outgoing = [0; 91];
    outgoing[0] = transport.report_id;
    outgoing[2] = transaction;
    outgoing[6..9].copy_from_slice(&command);
    outgoing[9..9 + data.len()].copy_from_slice(data);
    outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    let cap = transport.as_ref();
    crate::feature_exchange::with_validated_exchange(
        device,
        crate::feature_exchange::with_protocol25_scope(
            crate::backend::ExchangeOptions::mutex_out(Duration::from_millis(
                cap.sleep_between_out_ms,
            )),
            &cap.source_class,
        ),
        validate,
        |device, lease| {
            let mut last = "No DPI response".to_owned();
            for out_attempt in 0..cap.max_retry_out {
                validate()?;
                let sent = device.send_feature_at_collection_max_counted(&outgoing);
                if let Err(validation) = validate() {
                    return Err(crate::backend::complete_native_io(sent, validation));
                }
                if let Err(error) = sent
                    .and_then(|count| crate::backend::require_complete_send(count, outgoing.len()))
                {
                    if crate::backend::exchange_interrupted(&error) {
                        return Err(error);
                    }
                    last = format!("{error:#}");
                    continue;
                }
                std::thread::sleep(Duration::from_millis(cap.sleep_between_out_in_ms));
                for attempt in 0..cap.max_retry_in {
                    if attempt > 0 {
                        std::thread::sleep(Duration::from_millis(cap.sleep_between_in_ms));
                    }
                    validate()?;
                    let mut incoming = [0; 91];
                    // for_exchange already selected the exact physical factory (or
                    // retained the explicitly bound master) before framing this call.
                    incoming[0] = cap.get_report_id();
                    let received = device.get_feature_at_collection_max(&mut incoming);
                    let received = match validate() {
                        Ok(()) => received,
                        Err(validation) => Err(crate::backend::complete_native_read(
                            received, &incoming, validation,
                        )),
                    };
                    let received = match received {
                        Ok(value) => value,
                        Err(error) => {
                            if crate::backend::exchange_interrupted(&error) {
                                let error = crate::feature_exchange::retain_failed_feature_reply(
                                    error,
                                    command,
                                    transaction,
                                );
                                if let Some(data) =
                                    crate::feature_exchange::acknowledged_error(&error)
                                {
                                    acknowledged(&Acknowledgement {
                                        command,
                                        transaction,
                                        profile: None,
                                        response_bytes: data.len(),
                                        data: data.to_vec(),
                                    });
                                }
                                return Err(error);
                            }
                            last = format!("{error:#}");
                            break;
                        }
                    };
                    ensure!(
                        received == incoming.len(),
                        "DPI response report length differs"
                    );
                    if incoming[2] != transaction || incoming[7..9] != command[1..] {
                        last = "DPI response transaction or command differs".into();
                        break;
                    }
                    match incoming[1] {
                        1 => {
                            last = "DPI device remains busy".into();
                        }
                        2 => {
                            let count = usize::from(incoming[6]);
                            ensure!(count <= 80, "DPI response payload is truncated");
                            let acknowledgement = Acknowledgement {
                                command,
                                transaction,
                                profile: None,
                                response_bytes: count,
                                data: incoming[9..9 + count].to_vec(),
                            };
                            acknowledged(&acknowledgement);
                            return Ok(acknowledgement);
                        }
                        0 | 3 | 4 => {
                            last = format!("DPI response status {}", incoming[1]);
                            break;
                        }
                        status => anyhow::bail!("DPI command rejected with device status {status}"),
                    }
                }
                if out_attempt + 1 < cap.max_retry_out {
                    lease.retry_release()?;
                }
            }
            anyhow::bail!("DPI command exhausted source retries: {last}")
        },
    )
}

/// PID191 is the source's ordinary rzDevice25 dongle, not a dual-link relay.
/// ue() requires the real wireless-connected byte to be one before operations.
pub fn verify_wireless_online(
    device: &dyn FeatureTransport,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    verify_wireless_online_for(PRODUCT_ID, device, transaction, validate)
}

pub fn verify_wireless_online_for(
    product_id: u32,
    device: &dyn FeatureTransport,
    transaction: u8,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let cap = required(product_id)?;
    ensure!(
        cap.dongle_pid.is_some(),
        "DPI source has no ordinary dongle branch"
    );
    let reply = exchange(device, cap, [1, 0, 184], &[0], transaction, &validate)?;
    ensure!(
        reply.data.first() == Some(&1),
        "DPI receiver has not confirmed its wireless mouse online"
    );
    Ok(())
}

fn stage_reading(ack: &Acknowledgement, profile: u8) -> anyhow::Result<DpiStagesReading> {
    let data = &ack.data;
    ensure!(
        data.len() >= 3 && (data.len() - 3).is_multiple_of(7) && data[0] == profile,
        "DPI stage response is truncated or belongs to another profile"
    );
    Ok(DpiStagesReading {
        profile_id: data[0],
        active_stage: data[1],
        number_of_active_stages: data[2],
        records: data[3..]
            .chunks_exact(7)
            .map(|row| PackedDpiStage {
                index: row[0],
                x: u16::from_be_bytes([row[1], row[2]]),
                y: u16::from_be_bytes([row[3], row[4]]),
                z: u16::from_be_bytes([row[5], row[6]]),
            })
            .collect(),
        response_bytes: data.len(),
    })
}

fn level_reading(
    cap: &Capability,
    profile_id: u8,
    ack: &Acknowledgement,
) -> anyhow::Result<DpiPair> {
    ensure!(
        ack.data.len() >= 7 && ack.data[0] == profile_id,
        "Active DPI response is truncated or belongs to another profile"
    );
    let value = DpiPair {
        x: u16::from_be_bytes([ack.data[1], ack.data[2]]),
        y: u16::from_be_bytes([ack.data[3], ack.data[4]]),
    };
    if cap.product_id != 182 {
        validate_pair(cap, value)?;
    }
    Ok(value)
}

fn stage_payload(packed: &PackedStages, profile: u8) -> Vec<u8> {
    let mut bytes = vec![0; 3 + 7 * usize::from(packed.number_of_active_stages)];
    bytes[..3].copy_from_slice(&[profile, packed.active_stage, packed.number_of_active_stages]);
    for (index, row) in packed.records.iter().enumerate() {
        let start = 3 + index * 7;
        bytes[start] = row.index;
        bytes[start + 1..start + 3]
            .copy_from_slice(&row.x.map_or(0, SourceNumber::word).to_be_bytes());
        bytes[start + 3..start + 5]
            .copy_from_slice(&row.y.map_or(0, SourceNumber::word).to_be_bytes());
        bytes[start + 5..start + 7].copy_from_slice(&row.z.word().to_be_bytes());
    }
    bytes
}

pub fn same_stages(sent: &PackedStages, got: &DpiStagesReading) -> bool {
    if got.profile_id != 1 {
        return false;
    }
    if matches!(sent.product_id, 190 | 192 | 222 | 226 | 229 | 239) {
        // The current task compares returned flat records until index zero or
        // the array boundary. It does not consume the GET's active/count header.
        for (index, observed) in got.records.iter().enumerate() {
            if observed.index == 0 {
                return true;
            }
            if !sent
                .records
                .get(index)
                .is_some_and(|expected| same_record(expected, observed))
            {
                return false;
            }
        }
        return true;
    }
    same_profile_records(sent, got)
}

fn same_record(sent: &PackedSourceStage, got: &PackedDpiStage) -> bool {
    got.index == sent.index + 1
        && sent.x.is_some_and(|value| value.strict_equals_u16(got.x))
        && sent.y.is_some_and(|value| value.strict_equals_u16(got.y))
        && sent.z.strict_equals_u16(got.z)
}

fn same_profile_records(sent: &PackedStages, got: &DpiStagesReading) -> bool {
    let records: Vec<_> = got
        .records
        .iter()
        .take_while(|row| row.index != 0)
        .collect();
    got.active_stage == sent.active_stage
        && got.number_of_active_stages == sent.number_of_active_stages
        && records.len() == sent.records.len()
        && sent
            .records
            .iter()
            .zip(records)
            .all(|(sent, got)| same_record(sent, got))
}

fn editor_draft_for(product_id: u32, source: &SourceStages) -> anyhow::Result<DpiStagesDraft> {
    let draft = DpiStagesDraft {
        enabled: source.enabled,
        active_stage: source.active_stage,
        stages: source
            .stages
            .iter()
            .map(|row| {
                Ok(crate::mouse_dpi_stages::DpiStage {
                    x: editor_axis(row.x())?,
                    y: editor_axis(row.y())?,
                    visible: row
                        .is_visible()
                        .context("Source DPI visibility is absent")?,
                    independent: row
                        .is_independent()
                        .context("Source DPI axis mode is absent")?,
                })
            })
            .collect::<anyhow::Result<_>>()?,
    };
    pack_editor_for(product_id, &draft)?;
    Ok(draft)
}

fn editor_axis(value: Option<SourceNumber>) -> anyhow::Result<u16> {
    let value = value.context("Source DPI axis is absent")?.value();
    ensure!(
        value >= 0. && value <= f64::from(u16::MAX) && value.fract() == 0.,
        "Source DPI axis is not an unsigned integer"
    );
    Ok(value as u16)
}

fn synchronize_ordinary_profile_slots(
    device: &dyn FeatureTransport,
    cap: &Capability,
    active_guid: &str,
    observed_slots: &[crate::mouse_dpi_stages::ObservedProfileSlot],
    table: &PackedStages,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<crate::mouse_dpi_stages::DpiSlotWriteResult>> {
    crate::mouse_dpi_stages::synchronize_profile_slots_with(
        active_guid,
        observed_slots,
        next,
        validate,
        |next, slot| {
            stage_reading(
                &exchange(device, cap, [80, 4, 134], &[slot], next()?, validate)?,
                slot,
            )
        },
        |before| same_profile_records(table, before),
        |next, slot| {
            let payload = stage_payload(table, slot);
            let mut completed = None;
            let response = exchange_observing(
                device,
                cap,
                [payload.len() as u8, 4, 6],
                &payload,
                next()?,
                validate,
                &mut |ack| {
                    completed = Some(crate::mouse_dpi_stages::DpiStagesAcknowledgement {
                        response_bytes: ack.response_bytes,
                        raw_data: ack.data.clone(),
                    });
                },
            );
            let ack = response.map_err(|error| match completed {
                Some(acknowledged) => error.context(acknowledged),
                None => error,
            })?;
            Ok(crate::mouse_dpi_stages::DpiStagesAcknowledgement {
                response_bytes: ack.response_bytes,
                raw_data: ack.data,
            })
        },
    )
}

/// Independent getter failures accompany the values actually observed by the
/// other source task; they never turn an unobserved field into a default.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct DpiTaskErrors {
    stages: Option<String>,
    active: Option<String>,
}

impl DpiTaskErrors {
    pub fn stages(&self) -> Option<&str> {
        self.stages.as_deref()
    }

    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    pub fn is_empty(&self) -> bool {
        self.stages.is_none() && self.active.is_none()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct DpiReport {
    pub requested: Option<DpiChange>,
    pub stages: Option<DpiStagesReading>,
    /// Present only for an actual profile-1 getter/returned setter value.
    /// A source level-only stage setter is confirmed by its protocol ACK.
    #[serde(default)]
    pub stage_level: Option<DpiPair>,
    pub active: Option<DpiPair>,
    pub acknowledgements: Vec<Acknowledgement>,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub initialization_mode_error: Option<String>,
    #[serde(default)]
    pub profile_slots: Vec<crate::mouse_dpi_stages::ObservedProfileSlot>,
    /// Source Mt only pre-reads and conditionally sets; ACK is not readback.
    #[serde(default)]
    pub profile_slot_writes: Vec<crate::mouse_dpi_stages::DpiSlotWriteResult>,
    #[serde(default)]
    pub read_errors: DpiTaskErrors,
    /// Failed mutations retain their actual observations and acknowledgements.
    /// Transport completion does not imply that the requested value was accepted.
    #[serde(default)]
    pub write_errors: DpiTaskErrors,
    pub verified: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WriteFailure {
    /// The original request's progress is diagnostic after scope loss.
    pub progress: DpiReport,
}

impl std::fmt::Display for WriteFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Mouse DPI task did not complete")
    }
}

/// Cancellation and identity/deadline checks surround every feature transfer.
/// A failed confirmation leaves the optimistic local profile to its UI owner.
pub fn execute(
    device: &dyn FeatureTransport,
    change: Option<&DpiChange>,
    next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiReport> {
    execute_for(PRODUCT_ID, device, change, next, validate)
}

pub fn execute_for(
    product_id: u32,
    device: &dyn FeatureTransport,
    change: Option<&DpiChange>,
    next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiReport> {
    let cap = required(product_id)?;
    execute_with_capability(cap, device, change, next, validate)
}

/// The current182 receiver factory selects its physical master's interface;
/// the mouse retains its own namespace and source tasks on the same handle.
pub(crate) fn execute_receiver(
    transport: &DeviceReadCapability,
    device: &dyn FeatureTransport,
    change: Option<&DpiChange>,
    next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiReport> {
    let mut cap = required(transport.product_id)?.clone();
    ensure!(
        transport.source_class == "rzDevice25DualLinkMouse"
            && cap
                .transport
                .physical_transports_by_pid
                .values()
                .any(|branch| branch.is_duallink && branch.source_class == transport.source_class),
        "Receiver DPI source sender differs"
    );
    cap.transport = transport.clone();
    execute_with_capability(&cap, device, change, next, validate)
}

/// Preserve the complete current DPI task (stage 1 / transient 0) between the
/// receiver's before/after peer observations. This is not the stage-only task.
pub fn execute_in_session(
    session: &crate::receiver_session::ReceiverSession,
    device: &dyn FeatureTransport,
    change: Option<&DpiChange>,
    next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<crate::receiver_session::Observation<DpiReport>> {
    session.observe(device, next, validate, |next, validate| {
        execute_receiver(session.transport(), device, change, next, validate)
    })
}

fn execute_with_capability(
    cap: &Capability,
    device: &dyn FeatureTransport,
    change: Option<&DpiChange>,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<DpiReport> {
    let validate = || {
        validate().map_err(|error| {
            crate::backend::interrupt_exchange(
                error,
                crate::backend::ExchangeInterruption::Validation,
            )
        })
    };
    let product_id = cap.product_id;
    if let Some(change) = change {
        validate_change_for(product_id, change)?;
    }
    let mut report = DpiReport {
        requested: change.cloned(),
        stages: None,
        stage_level: None,
        active: None,
        acknowledgements: vec![],
        serial_number: None,
        initialization_mode_error: None,
        profile_slots: vec![],
        profile_slot_writes: vec![],
        read_errors: DpiTaskErrors::default(),
        write_errors: DpiTaskErrors::default(),
        verified: false,
    };
    let result = (|| -> anyhow::Result<()> {
        if (product_id == 182 && change.is_some_and(|change| change.initialize))
            || product_id == 190 && change.is_some_and(DpiChange::is_identity_read)
        {
            let serial = crate::feature_exchange::exchange(
                device,
                cap.transport(),
                [22, 0, 130],
                &[],
                next()?,
                &validate,
            )?;
            // Current12844.u retains every printable ASCII byte, including spaces.
            let serial: String = serial
                .iter()
                .copied()
                .filter(|byte| (32..127).contains(byte))
                .map(char::from)
                .collect();
            ensure!(
                !serial.is_empty(),
                "PID{product_id} initialization has no queried device serial number"
            );
            report.serial_number = Some(serial);
            if change.is_some_and(DpiChange::is_identity_read) {
                validate()?;
                report.verified = true;
                return Ok(());
            }
            // 90012.gn -> 87887.Y ->70035.xZ=[3,0]. Y catches a mode
            // failure; it does not pretend success or prevent later profile tasks.
            let mode = (|| -> anyhow::Result<()> {
                let current = crate::feature_exchange::exchange(
                    device,
                    cap.transport(),
                    [2, 0, 132],
                    &[],
                    next()?,
                    &validate,
                )?;
                ensure!(current.len() >= 2, "PID182 mode getter is truncated");
                if current[..2] != [3, 0] {
                    crate::feature_exchange::exchange(
                        device,
                        cap.transport(),
                        [2, 0, 4],
                        &[3, 0],
                        next()?,
                        &validate,
                    )?;
                }
                Ok(())
            })();
            if let Err(error) = mode {
                if crate::backend::exchange_interrupted(&error) {
                    return Err(error);
                }
                report.initialization_mode_error = Some(format!("{error:#}"));
            }
            validate()?;
        }
        if change.is_none() || change.is_some_and(|change| change.stages.is_some()) {
            let stage_result = (|| -> anyhow::Result<()> {
                let packed = change
                    .and_then(|change| change.stages.as_ref())
                    .map(|draft| pack_for(product_id, draft))
                    .transpose()?;
                if cap.stage_level_only {
                    if let Some(packed) = &packed {
                        let stage = packed
                            .active_stage
                            .checked_sub(1)
                            .and_then(|index| packed.records.get(usize::from(index)))
                            .context("Source selected DPI stage has no numeric X/Y values")?;
                        let mut payload = vec![1];
                        payload.extend_from_slice(
                            &stage.x.map_or(0, SourceNumber::word).to_be_bytes(),
                        );
                        payload.extend_from_slice(
                            &stage.y.map_or(0, SourceNumber::word).to_be_bytes(),
                        );
                        payload.extend_from_slice(&[0, 0]);
                        let ack = exchange_observing(
                            device,
                            cap,
                            [7, 4, 5],
                            &payload,
                            next()?,
                            &validate,
                            &mut |ack| report.acknowledgements.push(ack.clone()),
                        )?;
                        if ack.data.len() >= 7 {
                            report.stage_level = Some(level_reading(cap, 1, &ack)?);
                        }
                    } else {
                        report.stage_level = Some(level_reading(
                            cap,
                            1,
                            &exchange(device, cap, [7, 4, 133], &[1], next()?, &validate)?,
                        )?);
                    }
                    return Ok(());
                }
                if let Some(packed) = &packed {
                    let payload = stage_payload(packed, 1);
                    exchange_observing(
                        device,
                        cap,
                        [payload.len() as u8, 4, 6],
                        &payload,
                        next()?,
                        &validate,
                        &mut |ack| report.acknowledgements.push(ack.clone()),
                    )?;
                }
                let observed = stage_reading(
                    &exchange(device, cap, [80, 4, 134], &[1], next()?, &validate)?,
                    1,
                )?;
                report.stages = Some(observed);
                if let Some(packed) = &packed {
                    ensure!(
                        same_stages(packed, report.stages.as_ref().expect("actual stage getter")),
                        "DPI stage setter acknowledged but readback differs"
                    );
                }
                if matches!(product_id, 182 | 190)
                    && let Some(change) = change
                    && let (Some(guid), Some(draft)) = (&change.profile_guid, &change.stages)
                {
                    ensure!(
                        !guid.is_empty(),
                        "PID{product_id} active profile GUID is empty"
                    );
                    report.profile_slots = crate::mouse_onboard::read_profile_associations(
                        device,
                        cap.transport(),
                        &mut next,
                        &validate,
                    )?
                    .into_iter()
                    .map(|slot| crate::mouse_dpi_stages::ObservedProfileSlot {
                        slot_id: slot.profile_id,
                        guid: slot.guid,
                    })
                    .collect();
                    let slot_writes = if product_id == 182 {
                        let mut stages_cap = crate::mouse_dpi_stages::capability(182)
                            .context("PID182 stage capability absent")?
                            .clone();
                        stages_cap.transport = cap.transport().clone();
                        crate::mouse_dpi_stages::synchronize_profile_slots(
                            device,
                            &stages_cap,
                            guid,
                            &report.profile_slots,
                            &editor_draft_for(product_id, draft)?,
                            &mut next,
                            &validate,
                        )
                    } else {
                        synchronize_ordinary_profile_slots(
                            device,
                            cap,
                            guid,
                            &report.profile_slots,
                            packed
                                .as_ref()
                                .context("DPI stage task has no packed table")?,
                            &mut next,
                            &validate,
                        )
                    };
                    if let Err(error) = &slot_writes
                    && let Some(progress) = error.downcast_ref::<
                        crate::mouse_dpi_stages::DpiSlotSynchronizationFailure,
                    >()
                {
                    report.profile_slot_writes = progress.writes.clone();
                }
                    report.profile_slot_writes = slot_writes?;
                    let failures: Vec<_> = report
                        .profile_slot_writes
                        .iter()
                        .filter_map(|slot| slot.error.as_deref())
                        .collect();
                    ensure!(
                        failures.is_empty(),
                        "PID{product_id} OBM stage synchronization failed: {}",
                        failures.join("; ")
                    );
                }
                Ok(())
            })();
            if let Err(error) = stage_result {
                if crate::backend::exchange_interrupted(&error) {
                    return Err(error);
                }
                if change.is_none() {
                    report.read_errors.stages = Some(format!("{error:#}"));
                } else {
                    report.write_errors.stages = Some(format!("{error:#}"));
                }
            }
            // Stage and current X/Y are independent source tasks. Device rejection
            // does not discard the other task; scope cancellation stops both.
            validate()?;
        }
        if change.is_none() || change.is_some_and(|change| change.active.is_some()) {
            let active_result = (|| -> anyhow::Result<()> {
                let mut observed = level_reading(
                    cap,
                    0,
                    &exchange(device, cap, [7, 4, 133], &[0], next()?, &validate)?,
                )?;
                if let Some(wanted) = change.and_then(|change| change.active) {
                    if observed != wanted {
                        let mut payload = vec![0];
                        payload.extend_from_slice(&wanted.x.to_be_bytes());
                        payload.extend_from_slice(&wanted.y.to_be_bytes());
                        payload.extend_from_slice(&[0, 0]);
                        exchange_observing(
                            device,
                            cap,
                            [7, 4, 5],
                            &payload,
                            next()?,
                            &validate,
                            &mut |ack| report.acknowledgements.push(ack.clone()),
                        )?;
                        observed = level_reading(
                            cap,
                            0,
                            &exchange(device, cap, [7, 4, 133], &[0], next()?, &validate)?,
                        )?;
                    }
                    report.active = Some(observed);
                    ensure!(
                        report.active == Some(wanted),
                        "Active DPI setter acknowledged but readback differs"
                    );
                }
                report.active = Some(observed);
                Ok(())
            })();
            if let Err(error) = active_result {
                if crate::backend::exchange_interrupted(&error) {
                    return Err(error);
                }
                if change.is_none() {
                    report.read_errors.active = Some(format!("{error:#}"));
                } else {
                    report.write_errors.active = Some(format!("{error:#}"));
                }
            }
        }
        validate()?;
        report.verified = report.read_errors.is_empty() && report.write_errors.is_empty();
        Ok(())
    })();
    if let Err(error) = result {
        report.verified = false;
        let message = error.to_string();
        return Err(error
            .context(WriteFailure { progress: report })
            .context(message));
    }
    Ok(report)
}

// Generated typed table: razer-device/mouse_current_dpi_data.
#[rustfmt::skip]
fn make_mouse_current_dpi_table() -> Vec<Capability> {
    vec![
        Capability{
            product_id: 80u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: true,
            wired_pid: 80u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 80u32,
                source_class: "rzDevice25LegacyNoBrightness".into(),
                direct_pids: vec![80u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![].into_iter().collect(),
                physical_transports_by_pid: Default::default(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 83u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: true,
            wired_pid: 83u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 83u32,
                source_class: "rzDevice25LegacyNoBrightness".into(),
                direct_pids: vec![83u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![].into_iter().collect(),
                physical_transports_by_pid: Default::default(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 89u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 89u32,
            dongle_pid: Some(90u32),
            transport: DeviceReadCapability{
                product_id: 89u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![89u32,90u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("89".into(),0u8),("90".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "89".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "90".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 92u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: true,
            wired_pid: 92u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 92u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![92u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("92".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "92".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 96u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 96u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 96u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![96u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("96".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "96".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 98u32,
            minimum: 200u16,
            maximum: 7200u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 98u32,
            dongle_pid: Some(98u32),
            transport: DeviceReadCapability{
                product_id: 98u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![98u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("98".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "98".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 99u32,
            minimum: 200u16,
            maximum: 7200u16,
            cycle_sensitivity: false,
            stage_level_only: true,
            wired_pid: 99u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 99u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![99u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("99".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "99".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 100u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 100u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 100u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![100u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("100".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "100".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 101u32,
            minimum: 200u16,
            maximum: 6400u16,
            cycle_sensitivity: false,
            stage_level_only: true,
            wired_pid: 101u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 101u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![101u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("101".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "101".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 103u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 103u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 103u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![103u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("103".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "103".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 104u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 104u32,
            dongle_pid: Some(105u32),
            transport: DeviceReadCapability{
                product_id: 104u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![104u32,105u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("104".into(),0u8),("105".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "104".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 10u64,
                            sleep_between_out_in_ms: 10u64,
                            sleep_between_in_ms: 10u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "105".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 10u64,
                            sleep_between_out_in_ms: 10u64,
                            sleep_between_in_ms: 10u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 10u64,
                sleep_between_out_in_ms: 10u64,
                sleep_between_in_ms: 10u64
            }
        },
        Capability{
            product_id: 106u32,
            minimum: 200u16,
            maximum: 7200u16,
            cycle_sensitivity: false,
            stage_level_only: true,
            wired_pid: 106u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 106u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![106u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("106".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "106".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 107u32,
            minimum: 200u16,
            maximum: 7200u16,
            cycle_sensitivity: false,
            stage_level_only: true,
            wired_pid: 107u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 107u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![107u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("107".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "107".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 108u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 108u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 108u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![108u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("108".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "108".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 110u32,
            minimum: 200u16,
            maximum: 6400u16,
            cycle_sensitivity: false,
            stage_level_only: true,
            wired_pid: 110u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 110u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![110u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("110".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "110".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 112u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 112u32,
            dongle_pid: Some(111u32),
            transport: DeviceReadCapability{
                product_id: 112u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![111u32,112u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("112".into(),0u8),("111".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "112".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "111".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 113u32,
            minimum: 200u16,
            maximum: 6400u16,
            cycle_sensitivity: false,
            stage_level_only: true,
            wired_pid: 113u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 113u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![113u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("113".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "113".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 115u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 115u32,
            dongle_pid: Some(114u32),
            transport: DeviceReadCapability{
                product_id: 115u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![114u32,115u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("114".into(),0u8),("115".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "114".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "115".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 116u32,
            minimum: 200u16,
            maximum: 6400u16,
            cycle_sensitivity: false,
            stage_level_only: true,
            wired_pid: 116u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 116u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![116u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("116".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "116".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 117u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 117u32,
            dongle_pid: Some(2308u32),
            transport: DeviceReadCapability{
                product_id: 117u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![117u32,2308u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("2308".into(),0u8),("117".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "2308".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "117".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 120u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 120u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 120u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![120u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("120".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "120".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 122u32,
            minimum: 100u16,
            maximum: 20000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 122u32,
            dongle_pid: Some(123u32),
            transport: DeviceReadCapability{
                product_id: 122u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![122u32,123u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("122".into(),0u8),("123".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "122".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "123".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 124u32,
            minimum: 100u16,
            maximum: 20000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 124u32,
            dongle_pid: Some(125u32),
            transport: DeviceReadCapability{
                product_id: 124u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![124u32,125u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("124".into(),0u8),("125".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "124".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "125".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 128u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 128u32,
            dongle_pid: Some(119u32),
            transport: DeviceReadCapability{
                product_id: 128u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![119u32,128u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("128".into(),0u8),("119".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "128".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "119".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 131u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 131u32,
            dongle_pid: Some(131u32),
            transport: DeviceReadCapability{
                product_id: 131u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![131u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("131".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "131".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 132u32,
            minimum: 100u16,
            maximum: 20000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 132u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 132u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![132u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("132".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "132".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 133u32,
            minimum: 100u16,
            maximum: 20000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 133u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 133u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![133u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("133".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "133".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 134u32,
            minimum: 100u16,
            maximum: 20000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 134u32,
            dongle_pid: Some(136u32),
            transport: DeviceReadCapability{
                product_id: 134u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![134u32,136u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("136".into(),0u8),("134".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "136".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "134".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 138u32,
            minimum: 200u16,
            maximum: 8500u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 138u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 138u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![138u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("138".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "138".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 140u32,
            minimum: 200u16,
            maximum: 8500u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 140u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 140u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![140u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("140".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "140".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 141u32,
            minimum: 100u16,
            maximum: 20000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 141u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 141u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![141u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("141".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "141".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 143u32,
            minimum: 100u16,
            maximum: 20000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 143u32,
            dongle_pid: Some(144u32),
            transport: DeviceReadCapability{
                product_id: 143u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![143u32,144u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("144".into(),0u8),("143".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "144".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    ),
                    (
                        "143".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 145u32,
            minimum: 100u16,
            maximum: 20000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 145u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 145u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![145u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("145".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "145".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 147u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 147u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 147u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![147u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("147".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "147".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 148u32,
            minimum: 100u16,
            maximum: 18000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 148u32,
            dongle_pid: Some(148u32),
            transport: DeviceReadCapability{
                product_id: 148u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![148u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("148".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "148".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 150u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 150u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 150u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![150u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("150".into(),3u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "150".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 152u32,
            minimum: 200u16,
            maximum: 6400u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 152u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 152u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![152u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("152".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "152".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 153u32,
            minimum: 100u16,
            maximum: 26000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 153u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 153u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![153u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("153".into(),3u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "153".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 154u32,
            minimum: 200u16,
            maximum: 12000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 154u32,
            dongle_pid: Some(154u32),
            transport: DeviceReadCapability{
                product_id: 154u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![154u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("154".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "154".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 156u32,
            minimum: 100u16,
            maximum: 14000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 156u32,
            dongle_pid: Some(156u32),
            transport: DeviceReadCapability{
                product_id: 156u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![156u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("156".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "156".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 158u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 158u32,
            dongle_pid: Some(159u32),
            transport: DeviceReadCapability{
                product_id: 158u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![158u32,159u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("158".into(),0u8),("159".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "158".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "159".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 161u32,
            minimum: 200u16,
            maximum: 8500u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 161u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 161u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![161u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("161".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "161".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 163u32,
            minimum: 200u16,
            maximum: 8500u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 163u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 163u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![163u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("163".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "163".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 165u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 165u32,
            dongle_pid: Some(166u32),
            transport: DeviceReadCapability{
                product_id: 165u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![165u32,166u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("165".into(),0u8),("166".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "165".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "166".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 167u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 167u32,
            dongle_pid: Some(168u32),
            transport: DeviceReadCapability{
                product_id: 167u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![167u32,168u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("168".into(),0u8),("167".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "168".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    ),
                    (
                        "167".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 170u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 170u32,
            dongle_pid: Some(171u32),
            transport: DeviceReadCapability{
                product_id: 170u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![170u32,171u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("170".into(),0u8),("171".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "170".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "171".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 175u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 175u32,
            dongle_pid: Some(176u32),
            transport: DeviceReadCapability{
                product_id: 175u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![175u32,176u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("176".into(),0u8),("175".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "176".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    ),
                    (
                        "175".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 178u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 178u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 178u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![178u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("178".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "178".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 180u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 180u32,
            dongle_pid: Some(180u32),
            transport: DeviceReadCapability{
                product_id: 180u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![180u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("180".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "180".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 182u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 182u32,
            dongle_pid: Some(183u32),
            transport: DeviceReadCapability{
                product_id: 182u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![182u32,183u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("182".into(),0u8),("183".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "182".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "183".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 184u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 184u32,
            dongle_pid: Some(184u32),
            transport: DeviceReadCapability{
                product_id: 184u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![184u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("184".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "184".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 185u32,
            minimum: 100u16,
            maximum: 18000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 185u32,
            dongle_pid: Some(185u32),
            transport: DeviceReadCapability{
                product_id: 185u32,
                source_class: "rzDevice25DualLinkMouse".into(),
                direct_pids: vec![185u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("185".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "185".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 25u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 25u64
            }
        },
        Capability{
            product_id: 190u32,
            minimum: 100u16,
            maximum: 45000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 190u32,
            dongle_pid: Some(191u32),
            transport: DeviceReadCapability{
                product_id: 190u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![190u32,191u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("190".into(),0u8),("191".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "190".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "191".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 192u32,
            minimum: 100u16,
            maximum: 35000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 192u32,
            dongle_pid: Some(193u32),
            transport: DeviceReadCapability{
                product_id: 192u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![192u32,193u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("192".into(),0u8),("193".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "192".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "193".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 194u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 194u32,
            dongle_pid: Some(195u32),
            transport: DeviceReadCapability{
                product_id: 194u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![194u32,195u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("194".into(),0u8),("195".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "194".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "195".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 196u32,
            minimum: 100u16,
            maximum: 20000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 196u32,
            dongle_pid: Some(197u32),
            transport: DeviceReadCapability{
                product_id: 196u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![196u32,197u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("196".into(),0u8),("197".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "196".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "197".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 199u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: true,
            stage_level_only: false,
            wired_pid: 199u32,
            dongle_pid: Some(200u32),
            transport: DeviceReadCapability{
                product_id: 199u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![199u32,200u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("200".into(),0u8),("199".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "200".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "199".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 203u32,
            minimum: 100u16,
            maximum: 35000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 203u32,
            dongle_pid: None,
            transport: DeviceReadCapability{
                product_id: 203u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![203u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("203".into(),3u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "203".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 204u32,
            minimum: 100u16,
            maximum: 35000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 204u32,
            dongle_pid: Some(205u32),
            transport: DeviceReadCapability{
                product_id: 204u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![204u32,205u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("204".into(),0u8),("205".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "204".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "205".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 208u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: true,
            stage_level_only: false,
            wired_pid: 208u32,
            dongle_pid: Some(209u32),
            transport: DeviceReadCapability{
                product_id: 208u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![208u32,209u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("208".into(),0u8),("209".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "208".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "209".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 211u32,
            minimum: 100u16,
            maximum: 18000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 211u32,
            dongle_pid: Some(212u32),
            transport: DeviceReadCapability{
                product_id: 211u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![211u32,212u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("211".into(),0u8),("212".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "211".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "212".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 214u32,
            minimum: 100u16,
            maximum: 35000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 214u32,
            dongle_pid: Some(215u32),
            transport: DeviceReadCapability{
                product_id: 214u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![214u32,215u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("214".into(),0u8),("215".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "214".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "215".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 218u32,
            minimum: 100u16,
            maximum: 26000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 218u32,
            dongle_pid: Some(219u32),
            transport: DeviceReadCapability{
                product_id: 218u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![218u32,219u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("218".into(),0u8),("219".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "218".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "219".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 221u32,
            minimum: 100u16,
            maximum: 45000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 221u32,
            dongle_pid: Some(221u32),
            transport: DeviceReadCapability{
                product_id: 221u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![221u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("221".into(),3u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "221".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 222u32,
            minimum: 100u16,
            maximum: 35000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 222u32,
            dongle_pid: Some(223u32),
            transport: DeviceReadCapability{
                product_id: 222u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![222u32,223u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("222".into(),0u8),("223".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "222".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "223".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 224u32,
            minimum: 100u16,
            maximum: 18000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 224u32,
            dongle_pid: Some(224u32),
            transport: DeviceReadCapability{
                product_id: 224u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![224u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("224".into(),0u8)].into_iter().collect(),
                physical_transports_by_pid: vec![
                    (
                        "224".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 10u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 10u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 10u64,
                sleep_between_out_in_ms: 25u64,
                sleep_between_in_ms: 10u64
            }
        },
        Capability{
            product_id: 226u32,
            minimum: 100u16,
            maximum: 50000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 226u32,
            dongle_pid: Some(227u32),
            transport: DeviceReadCapability{
                product_id: 226u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![226u32,227u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("226".into(),3u8),("227".into(),3u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "226".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "227".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 3u8,
                            report_id: 3u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 229u32,
            minimum: 100u16,
            maximum: 50000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 229u32,
            dongle_pid: Some(230u32),
            transport: DeviceReadCapability{
                product_id: 229u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![229u32,230u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("229".into(),3u8),("230".into(),3u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "229".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "230".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 231u32,
            minimum: 100u16,
            maximum: 30000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 231u32,
            dongle_pid: Some(232u32),
            transport: DeviceReadCapability{
                product_id: 231u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![231u32,232u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("232".into(),3u8),("231".into(),3u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "232".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 3u8,
                            report_id: 3u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    ),
                    (
                        "231".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 235u32,
            minimum: 100u16,
            maximum: 33000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 235u32,
            dongle_pid: Some(234u32),
            transport: DeviceReadCapability{
                product_id: 235u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![234u32,235u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("234".into(),3u8),("235".into(),3u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "234".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 3u8,
                            report_id: 3u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    ),
                    (
                        "235".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 239u32,
            minimum: 100u16,
            maximum: 50000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 239u32,
            dongle_pid: Some(240u32),
            transport: DeviceReadCapability{
                product_id: 239u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![239u32,240u32],
                vendor_id: 5426u16,
                claim_interface: 3u8,
                claim_interfaces_by_pid: vec![("240".into(),3u8),("239".into(),3u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "240".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "239".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 3u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 2594u32,
            minimum: 100u16,
            maximum: 16000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 2594u32,
            dongle_pid: Some(2593u32),
            transport: DeviceReadCapability{
                product_id: 2594u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![2593u32,2594u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("2593".into(),0u8),("2594".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "2593".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "2594".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        },
        Capability{
            product_id: 45066u32,
            minimum: 100u16,
            maximum: 35000u16,
            cycle_sensitivity: false,
            stage_level_only: false,
            wired_pid: 45066u32,
            dongle_pid: Some(45067u32),
            transport: DeviceReadCapability{
                product_id: 45066u32,
                source_class: "rzDevice25".into(),
                direct_pids: vec![45066u32,45067u32],
                vendor_id: 5426u16,
                claim_interface: 0u8,
                claim_interfaces_by_pid: vec![("45066".into(),0u8),("45067".into(),0u8)].into_iter().collect(
                ),
                physical_transports_by_pid: vec![
                    (
                        "45066".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 5u64,
                            sleep_between_out_in_ms: 5u64,
                            sleep_between_in_ms: 5u64,
                            is_duallink: false
                        }
                    ),
                    (
                        "45067".into(),
                        PhysicalFeatureTransport{
                            source_class: "rzDevice25DualLinkMouse".into(),
                            claim_interface: 0u8,
                            report_id: 0u8,
                            transaction_prefix: 0u8,
                            transaction_modulus: 31u8,
                            max_retry_in: 10u8,
                            max_retry_out: 20u8,
                            sleep_between_out_ms: 25u64,
                            sleep_between_out_in_ms: 25u64,
                            sleep_between_in_ms: 25u64,
                            is_duallink: true
                        }
                    )
                ].into_iter().collect(),
                report_id: 0u8,
                report_bytes: 91usize,
                transaction_prefix: 0u8,
                transaction_modulus: 31u8,
                queries: vec![],
                polling_codes: vec![].into_iter().collect(),
                polling_value_offset: None,
                polling_default_hz: None,
                polling_physical_product_id: None,
                charging_codes: vec![].into_iter().collect(),
                max_retry_in: 10u8,
                max_retry_out: 20u8,
                sleep_between_out_ms: 5u64,
                sleep_between_out_in_ms: 5u64,
                sleep_between_in_ms: 5u64
            }
        }
    ]
}

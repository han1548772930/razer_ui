//! Current 679 HID OBM reads in the mounted engine's command order. Each
//! observation belongs to this physical session; local profiles are not read
//! or used as a replacement for an unobserved or failed device field.
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    feature_exchange,
    keyboard_analog::{
        self, ActuationPoint, KeyboardAnalogObservation, KeyboardAnalogQuery, RapidTriggerKey,
    },
    keyboard_onboard_inventory,
    keyboard_profile_inventory::{self, ProfileInventory},
    keyboard_settings::{self, KeyboardBrightness, KeyboardSettingsCapability},
    onboard_assignment::ButtonAssignment,
    onboard_macro::{CurrentKeyboardMacroReader, MacroCatalog, MacroReading},
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const PRODUCT_ID: u32 = 679;
const GET_MULTI_ASSIGNMENT: [u8; 3] = [80, 2, 158];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadOptions {
    pub layout_id: u32,
    pub locally_known_macro_guids: BTreeSet<String>,
}

impl ReadOptions {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.locally_known_macro_guids.iter().all(|guid| !guid.is_empty()),
            "Locally known macro identity is empty");
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reading<T> {
    Unobserved,
    Observed(T),
    Failed { message: String },
}

impl<T> Reading<T> {
    pub fn observed(&self) -> Option<&T> {
        match self {
            Self::Observed(value) => Some(value),
            Self::Unobserved | Self::Failed { .. } => None,
        }
    }

    fn validate_attempt(&self) -> Result<()> {
        match self {
            Self::Unobserved => anyhow::bail!("An attempted OBM field is unobserved"),
            Self::Failed { message } => ensure!(!message.is_empty(), "OBM failure has no diagnostic"),
            Self::Observed(_) => {},
        }
        Ok(())
    }
}

/// A read failure is retained only after checking that the caller's identity,
/// deadline and cancellation guard still allow this snapshot to continue.
fn retain<T>(result: Result<T>, validate: &impl Fn() -> Result<()>) -> Result<Reading<T>> {
    validate()?;
    Ok(match result {
        Ok(value) => Reading::Observed(value),
        Err(error) => Reading::Failed {
            message: format!("{error:#}"),
        },
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Normal,
    Hypershift,
}

impl Mode {
    fn wire(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Hypershift => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyIdentity {
    pub input_id: String,
    pub firmware_key_id: u8,
    pub ignore_normal: bool,
    pub ignore_hypershift: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssignmentBatch {
    pub key_ids: Vec<u8>,
    /// The source makes at most two reads when all assignment data are zero.
    /// A failed second read leaves the actual first reply observable.
    pub attempts: Vec<Reading<Vec<ButtonAssignment>>>,
}

impl AssignmentBatch {
    pub fn observed(&self) -> Option<&[ButtonAssignment]> {
        self.attempts
            .iter()
            .rev()
            .find_map(|attempt| attempt.observed().map(Vec::as_slice))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBatch<T> {
    pub key_ids: Vec<u8>,
    pub reading: Reading<Vec<T>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeReading {
    pub mode: Mode,
    pub assignments: Vec<AssignmentBatch>,
    pub rapid_trigger: Vec<KeyBatch<RapidTriggerKey>>,
    pub actuation: Vec<KeyBatch<ActuationPoint>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KillSwitch {
    pub raw_enabled: u8,
    pub key_id1: u8,
    pub key_id2: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileReading {
    pub slot_id: u8,
    pub brightness: Reading<KeyboardBrightness>,
    /// Normal mode precedes Hypershift mode in the actual engine read chain.
    pub passes: Vec<ModeReading>,
    pub kill_switch: Reading<KillSwitch>,
}

impl ProfileReading {
    /// Both source passes write the same per-key analog field. A later failed
    /// batch cannot erase an actual result from this same device snapshot.
    pub fn actuation(&self, key_id: u8) -> Option<&ActuationPoint> {
        self.passes.iter().rev().find_map(|pass| {
            pass.actuation
                .iter()
                .filter_map(|batch| batch.reading.observed())
                .flat_map(|points| points.iter())
                .find(|point| point.key_id == key_id)
        })
    }

    pub fn rapid_trigger(&self, key_id: u8) -> Option<&RapidTriggerKey> {
        self.passes.iter().rev().find_map(|pass| {
            pass.rapid_trigger
                .iter()
                .filter_map(|batch| batch.reading.observed())
                .flat_map(|points| points.iter())
                .find(|point| point.key_id == key_id)
        })
    }

    pub fn assignment(&self, mode: Mode, key_id: u8) -> Option<&ButtonAssignment> {
        self.passes
            .iter()
            .find(|pass| pass.mode == mode)?
            .assignments
            .iter()
            .filter_map(AssignmentBatch::observed)
            .flat_map(|rows| rows.iter())
            .find(|row| row.key_id == key_id)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub product_id: u32,
    pub layout_id: u32,
    pub key_inventory: Vec<KeyIdentity>,
    pub macros: MacroCatalog,
    pub inventory: ProfileInventory,
    pub profiles: Vec<ProfileReading>,
}

impl Snapshot {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.product_id == PRODUCT_ID, "OBM snapshot belongs to another product");
        ensure!(self.key_inventory == selected_keys(self.layout_id)?,
            "OBM snapshot key inventory differs from its current layout");
        self.macros.validate()?;
        self.inventory.validate(self.product_id)?;
        ensure!(self.profiles.len() == self.inventory.slots.len(), "OBM snapshot profile count differs");
        for (profile, slot) in self.profiles.iter().zip(&self.inventory.slots) {
            ensure!(profile.slot_id == slot.slot_id, "OBM profile reading belongs to a foreign slot");
            profile.brightness.validate_attempt()?;
            if let Some(brightness) = profile.brightness.observed() {
                ensure!(brightness.profile_id == slot.slot_id
                    && brightness.percent == (u16::from(brightness.raw_value) * 100).div_ceil(255) as u8,
                    "OBM brightness differs from the observed slot or raw value");
            }
            ensure!(profile.passes.len() == 2, "OBM snapshot is missing a source mode pass");
            for (pass, mode) in profile.passes.iter().zip([Mode::Normal, Mode::Hypershift]) {
                ensure!(pass.mode == mode, "OBM snapshot mode passes are reordered");
                let keys: Vec<_> = self.key_inventory.iter().filter(|key| match mode {
                    Mode::Normal => !key.ignore_normal,
                    Mode::Hypershift => !key.ignore_hypershift,
                }).map(|key| key.firmware_key_id).collect();
                ensure!(pass.assignments.len() == keys.chunks(8).len()
                    && pass.rapid_trigger.len() == keys.chunks(13).len()
                    && pass.actuation.len() == keys.chunks(15).len(),
                    "OBM snapshot batches are missing or exceed source limits");
                for (batch, expected) in pass.assignments.iter().zip(keys.chunks(8)) {
                    ensure!(batch.key_ids == expected && (1..=2).contains(&batch.attempts.len()),
                        "OBM assignment batch keys or attempts differ");
                    for (index, attempt) in batch.attempts.iter().enumerate() {
                        attempt.validate_attempt()?;
                        if index == 1 {
                            ensure!(matches!(&batch.attempts[0], Reading::Observed(rows)
                                if rows.iter().all(|row| row.data.iter().all(|byte| *byte == 0))),
                                "OBM assignment retry lacks the source zero-data condition");
                        }
                        if let Some(rows) = attempt.observed() {
                            validate_returned_keys(expected, rows.iter().map(|row| row.key_id))?;
                            ensure!(rows.iter().all(|row| row.mode == mode.wire() && row.data_size <= 5),
                                "OBM assignment mode or data size differs");
                        }
                    }
                }
                for (batch, expected) in pass.rapid_trigger.iter().zip(keys.chunks(13)) {
                    ensure!(batch.key_ids == expected, "OBM rapid-trigger batch keys differ");
                    batch.reading.validate_attempt()?;
                    if let Some(points) = batch.reading.observed() {
                        validate_returned_keys(expected, points.iter().map(|point| point.key_id))?;
                    }
                }
                for (batch, expected) in pass.actuation.iter().zip(keys.chunks(15)) {
                    ensure!(batch.key_ids == expected, "OBM actuation batch keys differ");
                    batch.reading.validate_attempt()?;
                    if let Some(points) = batch.reading.observed() {
                        validate_returned_keys(expected, points.iter().map(|point| point.key_id))?;
                    }
                }
            }
            profile.kill_switch.validate_attempt()?;
        }
        Ok(())
    }
}

fn validate_returned_keys(expected: &[u8], actual: impl Iterator<Item = u8>) -> Result<()> {
    let actual: Vec<_> = actual.collect();
    let unique: BTreeSet<_> = actual.iter().copied().collect();
    ensure!(actual.len() == expected.len() && unique.len() == expected.len()
        && expected.iter().all(|key| unique.contains(key)), "OBM returned keys differ from the requested batch");
    Ok(())
}

fn settings_capability(cap: &DeviceReadCapability) -> Result<KeyboardSettingsCapability> {
    let audited = crate::keyboard_metadata::capability(PRODUCT_ID)
        .context("Current keyboard HID factory is absent")?;
    ensure!(
        cap.product_id == PRODUCT_ID
            && cap.source_class == audited.source_class
            && cap.vendor_id == audited.vendor_id
            && cap.claim_interface == audited.claim_interface
            && cap.direct_pids == audited.direct_pids
            && cap.claim_interfaces_by_pid == audited.claim_interfaces_by_pid
            && cap.report_bytes == 91
            && cap.report_id == 0
            && cap.transaction_prefix == 0
            && cap.transaction_modulus == 31
            && cap.max_retry_in > 0
            && cap.max_retry_out > 0,
        "Current keyboard OBM reader differs from its mounted HID factory"
    );
    let mut settings = keyboard_settings::capability(PRODUCT_ID)
        .context("Current keyboard settings factory is absent")?
        .clone();
    settings.transport = cap.clone();
    Ok(settings)
}

fn selected_keys(layout_id: u32) -> Result<Vec<KeyIdentity>> {
    let mut keys: Vec<_> = keyboard_onboard_inventory::current_679_inventory(layout_id)
        .iter()
        .map(|key| KeyIdentity {
            input_id: key.input_id.to_owned(),
            firmware_key_id: key.firmware_key_id,
            ignore_normal: key.ignore_normal,
            ignore_hypershift: key.ignore_hypershift,
        })
        .collect();
    keys.sort_unstable_by_key(|key| key.firmware_key_id);
    let mut ids = BTreeSet::new();
    let mut inputs = BTreeSet::new();
    ensure!(
        !keys.is_empty()
            && keys.iter().all(|key| key.firmware_key_id > 0
                && key.firmware_key_id < 255
                && ids.insert(key.firmware_key_id)
                && !key.input_id.is_empty()
                && inputs.insert(key.input_id.as_str())),
        "Current keyboard source key inventory is empty, invalid or repeated"
    );
    Ok(keys)
}

fn read_assignments(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    slot_id: u8,
    mode: Mode,
    keys: &[u8],
    next: &mut impl FnMut() -> Result<u8>,
    validate: &impl Fn() -> Result<()>,
) -> Result<Vec<ButtonAssignment>> {
    ensure!(
        slot_id > 0 && !keys.is_empty() && keys.len() <= 8,
        "Invalid OBM assignment batch"
    );
    let size = 1 + 9 * keys.len();
    let mut payload = vec![0; size];
    payload[0] = slot_id;
    for (index, key) in keys.iter().enumerate() {
        payload[1 + 9 * index] = *key;
        payload[2 + 9 * index] = mode.wire();
    }
    validate()?;
    let response = feature_exchange::exchange_with_report_data_size(
        device,
        cap,
        GET_MULTI_ASSIGNMENT,
        &payload,
        next()?,
        size as u8,
        validate,
    )?;
    ensure!(
        response.len() == size && response[0] == slot_id,
        "OBM assignment reply has a different slot or record count"
    );
    let mut returned = BTreeSet::new();
    let mut rows = Vec::with_capacity(keys.len());
    for row in response[1..].chunks_exact(9) {
        ensure!(
            keys.contains(&row[0]) && returned.insert(row[0]) && row[1] == mode.wire(),
            "OBM assignment reply has a foreign/repeated key or mode"
        );
        ensure!(
            row[3] <= 5,
            "OBM assignment data exceeds its five-byte record"
        );
        rows.push(ButtonAssignment {
            key_id: row[0],
            mode: row[1],
            function_id: row[2],
            data_size: row[3],
            data: row[4..9].try_into()?,
        });
    }
    Ok(rows)
}

fn read_mode(
    device: &dyn FeatureTransport,
    cap: &KeyboardSettingsCapability,
    slot_id: u8,
    mode: Mode,
    keys: &[u8],
    next: &mut impl FnMut() -> Result<u8>,
    validate: &impl Fn() -> Result<()>,
) -> Result<ModeReading> {
    let mut pass = ModeReading {
        mode,
        assignments: Vec::new(),
        rapid_trigger: Vec::new(),
        actuation: Vec::new(),
    };
    for chunk in keys.chunks(8) {
        let mut batch = AssignmentBatch {
            key_ids: chunk.to_vec(),
            attempts: Vec::new(),
        };
        for _ in 0..2 {
            let result =
                read_assignments(device, &cap.transport, slot_id, mode, chunk, next, validate);
            let reading = retain(result, validate)?;
            let retry = matches!(&reading, Reading::Observed(rows)
                if rows.iter().all(|row| row.data.iter().all(|byte| *byte == 0)));
            batch.attempts.push(reading);
            if !retry {
                break;
            }
        }
        pass.assignments.push(batch);
    }
    for chunk in keys.chunks(13) {
        validate()?;
        let result = keyboard_analog::read(
            device,
            cap,
            KeyboardAnalogQuery::RapidTrigger {
                profile_id: slot_id,
                key_ids: chunk.to_vec(),
            },
            next()?,
            validate,
        )
        .and_then(|observation| match observation {
            KeyboardAnalogObservation::RapidTrigger { keys, .. } => Ok(keys),
            _ => anyhow::bail!("OBM rapid-trigger reply has a different observation kind"),
        });
        pass.rapid_trigger.push(KeyBatch {
            key_ids: chunk.to_vec(),
            reading: retain(result, validate)?,
        });
    }
    for chunk in keys.chunks(15) {
        validate()?;
        let result = keyboard_analog::read(
            device,
            cap,
            KeyboardAnalogQuery::Actuation {
                profile_id: slot_id,
                key_ids: chunk.to_vec(),
            },
            next()?,
            validate,
        )
        .and_then(|observation| match observation {
            KeyboardAnalogObservation::Actuation { points, .. } => Ok(points),
            _ => anyhow::bail!("OBM actuation reply has a different observation kind"),
        });
        pass.actuation.push(KeyBatch {
            key_ids: chunk.to_vec(),
            reading: retain(result, validate)?,
        });
    }
    Ok(pass)
}

/// The caller owns the retained transport, transaction counter, cancellation
/// and serialization with all other commands on this physical collection.
pub fn read_hid(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    layout_id: u32,
    mut next: impl FnMut() -> Result<u8>,
    validate: &impl Fn() -> Result<()>,
) -> Result<Snapshot> {
    let settings = settings_capability(cap)?;
    let keys = selected_keys(layout_id)?;
    read_selected(device, cap, &settings, layout_id, keys, &mut next, validate)
}

fn read_selected(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    settings: &KeyboardSettingsCapability,
    layout_id: u32,
    keys: Vec<KeyIdentity>,
    next: &mut impl FnMut() -> Result<u8>,
    validate: &impl Fn() -> Result<()>,
) -> Result<Snapshot> {
    validate()?;
    let macros = CurrentKeyboardMacroReader::new(device, cap)?.load(next, validate)?;
    let inventory = keyboard_profile_inventory::read_hid(device, cap, &mut *next, validate)?;
    let mut profiles = Vec::with_capacity(inventory.slots.len());
    // Brightness for every slot precedes every assignment/analog read.
    for slot in &inventory.slots {
        let result =
            keyboard_settings::read_profile(device, settings, slot.slot_id, &mut *next, validate);
        profiles.push(ProfileReading {
            slot_id: slot.slot_id,
            brightness: retain(result, validate)?,
            passes: Vec::with_capacity(2),
            kill_switch: Reading::Unobserved,
        });
    }
    for profile in &mut profiles {
        for mode in [Mode::Normal, Mode::Hypershift] {
            let key_ids: Vec<_> = keys
                .iter()
                .filter(|key| match mode {
                    Mode::Normal => !key.ignore_normal,
                    Mode::Hypershift => !key.ignore_hypershift,
                })
                .map(|key| key.firmware_key_id)
                .collect();
            profile.passes.push(read_mode(
                device,
                settings,
                profile.slot_id,
                mode,
                &key_ids,
                next,
                validate,
            )?);
        }
    }
    // Kill-switch follows all slot assignment passes, not each slot's pass.
    for profile in &mut profiles {
        validate()?;
        let result = keyboard_analog::read(
            device,
            settings,
            KeyboardAnalogQuery::KillSwitch {
                profile_id: profile.slot_id,
            },
            next()?,
            validate,
        )
        .and_then(|observation| match observation {
            KeyboardAnalogObservation::KillSwitch {
                raw_enabled,
                key_id1,
                key_id2,
                ..
            } => Ok(KillSwitch {
                raw_enabled,
                key_id1,
                key_id2,
            }),
            _ => anyhow::bail!("OBM kill-switch reply has a different observation kind"),
        });
        profile.kill_switch = retain(result, validate)?;
    }
    validate()?;
    Ok(Snapshot {
        product_id: cap.product_id,
        layout_id,
        key_inventory: keys,
        macros,
        inventory,
        profiles,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroDetail {
    pub macro_id: u16,
    /// Source skips bytes for macros whose GUID is already locally present.
    pub content: Reading<MacroReading>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuousRapidTrigger {
    pub slot_id: u8,
    pub raw_enabled: Reading<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportDetails {
    pub macros: Vec<MacroDetail>,
    pub continuous_rapid_trigger: Vec<ContinuousRapidTrigger>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompleteReading {
    pub snapshot: Snapshot,
    pub import_details: ImportDetails,
}

impl CompleteReading {
    pub fn validate_for(&self, options: &ReadOptions) -> Result<()> {
        options.validate()?;
        ensure!(self.snapshot.layout_id == options.layout_id, "OBM reading belongs to another layout request");
        self.snapshot.validate()?;
        self.import_details.validate_for(&self.snapshot, &options.locally_known_macro_guids)
    }
}

/// The local default-mapping GUID publication between these source phases has
/// no device I/O. The caller consumes this full observation in that same local
/// merge order, while all physical reads retain a single collection lease.
pub fn read_complete(
    device: &dyn FeatureTransport, cap: &DeviceReadCapability, options: &ReadOptions,
    mut next: impl FnMut() -> Result<u8>, validate: &impl Fn() -> Result<()>,
) -> Result<CompleteReading> {
    options.validate()?;
    let snapshot = read_hid(device, cap, options.layout_id, &mut next, validate)?;
    let import_details = read_import_details(device, cap, &snapshot,
        &options.locally_known_macro_guids, &mut next, validate)?;
    let reading = CompleteReading { snapshot, import_details };
    reading.validate_for(options)?;
    validate()?;
    Ok(reading)
}

impl ImportDetails {
    pub fn validate_for(&self, snapshot: &Snapshot, known_macro_guids: &BTreeSet<String>) -> Result<()> {
        ensure!(self.macros.len() == snapshot.macros.macros.len()
            && self.continuous_rapid_trigger.len() == snapshot.inventory.slots.len(),
            "OBM import details are missing source macros or slots");
        for (detail, summary) in self.macros.iter().zip(&snapshot.macros.macros) {
            ensure!(detail.macro_id == summary.metadata.macro_id, "OBM macro details belong to another macro");
            if known_macro_guids.contains(&summary.metadata.guid) {
                ensure!(matches!(detail.content, Reading::Unobserved), "Source skips already-local macro bytes");
            } else {
                detail.content.validate_attempt()?;
                if let Some(reading) = detail.content.observed() {
                    ensure!(reading.metadata == summary.metadata
                        && reading.data_size <= snapshot.macros.inventory.storage.max_memory
                        && reading.data.len() == usize::try_from(reading.data_size)?,
                        "OBM macro detail bytes differ from their observed metadata or size");
                }
            }
        }
        for (detail, slot) in self.continuous_rapid_trigger.iter().zip(&snapshot.inventory.slots) {
            ensure!(detail.slot_id == slot.slot_id, "OBM continuous RT belongs to another slot");
            detail.raw_enabled.validate_attempt()?;
        }
        Ok(())
    }
}

/// Consumer detail reads run after the source's slot/GUID registration. Local
/// GUIDs only choose which macro bytes are requested; they never supply an
/// observed value. Continuous RT belongs to the profile conversion phase.
pub fn read_import_details(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    snapshot: &Snapshot,
    locally_known_macro_guids: &BTreeSet<String>,
    mut next: impl FnMut() -> Result<u8>,
    validate: &impl Fn() -> Result<()>,
) -> Result<ImportDetails> {
    let settings = settings_capability(cap)?;
    ensure!(
        snapshot.product_id == cap.product_id,
        "OBM detail snapshot belongs to another product"
    );
    snapshot.inventory.validate(snapshot.product_id)?;
    let reader = CurrentKeyboardMacroReader::new(device, cap)?;
    let mut macros = Vec::with_capacity(snapshot.macros.macros.len());
    for summary in &snapshot.macros.macros {
        let content = if locally_known_macro_guids.contains(&summary.metadata.guid) {
            Reading::Unobserved
        } else {
            let result = reader.read_details(
                summary,
                snapshot.macros.inventory.storage.max_memory,
                &mut next,
                validate,
            );
            retain(result, validate)?
        };
        macros.push(MacroDetail {
            macro_id: summary.metadata.macro_id,
            content,
        });
    }
    let mut continuous_rapid_trigger = Vec::with_capacity(snapshot.inventory.slots.len());
    for slot in &snapshot.inventory.slots {
        validate()?;
        let result = keyboard_analog::read(
            device,
            &settings,
            KeyboardAnalogQuery::ContinuousRapidTrigger {
                profile_id: slot.slot_id,
            },
            next()?,
            validate,
        )
        .and_then(|observation| match observation {
            KeyboardAnalogObservation::ContinuousRapidTrigger { raw_enabled, .. } => {
                Ok(raw_enabled)
            }
            _ => anyhow::bail!("OBM continuous-RT reply has a different observation kind"),
        });
        continuous_rapid_trigger.push(ContinuousRapidTrigger {
            slot_id: slot.slot_id,
            raw_enabled: retain(result, validate)?,
        });
    }
    validate()?;
    Ok(ImportDetails {
        macros,
        continuous_rapid_trigger,
    })
}

#[cfg(test)]
#[path = "keyboard_onboard_profiles_tests.rs"]
mod tests;

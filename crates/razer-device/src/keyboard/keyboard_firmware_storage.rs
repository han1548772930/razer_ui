//! The four current firmware-memory keyboards' observed-state reconstruction.
//! The caller retains one physical session, its transaction counter and guard
//! across capacity observation, formatting, restoration and the new upload.
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    feature_exchange,
    keyboard_onboard_profiles::{CompleteReading, Mode},
    keyboard_profile_inventory::ProfileMetadataObservation,
    onboard_assignment::ButtonAssignment,
    onboard_macro::{
        self, CurrentKeyboardMacroReader, MacroReading, MacroStorageInfo, MacroWriteReport,
        MacroWriteRequest, OnboardMacro,
    },
    onboard_macro_codec::{self, RebuiltMacro},
};
use anyhow::{Context as _, Result, ensure};
use serde::Serialize;
use std::{collections::BTreeMap, time::Duration};

#[cfg(test)]
#[path = "keyboard_firmware_storage_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Serialize)]
pub enum Operation {
    ResetProfile(u8),
    DeleteProfile(u8),
    Format,
    FormatStatus,
    CreateProfile(u8),
    ProfileName { slot: u8, offset: u16 },
    Assignment { slot: u8, key: u8, mode: u8 },
}

#[derive(Clone, Debug, Serialize)]
pub struct IoStep {
    pub operation: Operation,
    pub command: [u8; 3],
    pub payload: Vec<u8>,
    pub report_size: u8,
    pub attempted: bool,
    pub returned_status: Option<u8>,
    pub reply: Option<Vec<u8>>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProfileBackup {
    pub slot: u8,
    pub metadata: Vec<u8>,
    pub assignments: Vec<ButtonAssignment>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MacroBackup {
    pub reading: MacroReading,
    pub rebuilt: RebuiltMacro,
}

#[derive(Clone, Debug, Serialize)]
pub struct Reconstruction {
    pub directory: Vec<u8>,
    pub profiles: Vec<ProfileBackup>,
    pub macros: Vec<MacroBackup>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MacroReplay {
    pub report: MacroWriteReport,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct WriteReport {
    pub storage_before: Option<MacroStorageInfo>,
    pub reconstruction: Option<Reconstruction>,
    pub steps: Vec<IoStep>,
    pub format_statuses: Vec<u8>,
    /// FormatStorage returns its last command result even if its internal
    /// status has not reached 2. Keep this source branch separate from proof
    /// that formatting actually completed.
    pub source_rebuild_entered: bool,
    pub macro_restoration: Vec<MacroReplay>,
    pub storage_after_rebuild: Option<MacroStorageInfo>,
    pub upload: Option<MacroReplay>,
    pub storage_after: Option<MacroStorageInfo>,
}

impl WriteReport {
    pub fn format_confirmed(&self) -> bool {
        self.source_rebuild_entered && self.format_statuses.last() == Some(&2)
    }

    pub fn device_state_may_have_changed(&self) -> bool {
        self.steps
            .iter()
            .any(|step| step.attempted && !matches!(step.operation, Operation::FormatStatus))
            || self
                .macro_restoration
                .iter()
                .chain(self.upload.iter())
                .any(|entry| entry.report.steps.iter().any(|step| step.attempted))
    }

    pub fn confirmed(&self) -> bool {
        self.storage_after.is_some()
            && self
                .upload
                .as_ref()
                .is_some_and(|entry| entry.error.is_none())
            && (self.reconstruction.is_none() || self.format_confirmed())
            && self.steps.iter().all(|step| step.error.is_none())
            && self
                .macro_restoration
                .iter()
                .all(|entry| entry.error.is_none())
    }
}

#[derive(Debug)]
pub struct WriteFailure {
    pub progress: WriteReport,
    error: anyhow::Error,
}

impl WriteFailure {
    pub(crate) fn into_error(self) -> anyhow::Error {
        self.error
    }
}

impl std::fmt::Display for WriteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Keyboard firmware-memory operation failed; partial progress retained: {:#}",
            self.error
        )
    }
}

impl std::error::Error for WriteFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

fn profile_metadata(
    identity: &crate::keyboard_profile_inventory::ProfileIdentity,
) -> Result<Vec<u8>> {
    let owner = identity
        .owner
        .encode_utf16()
        .map(|unit| unit as u8)
        .collect::<Vec<_>>();
    ensure!(
        owner.len() <= 128,
        "Observed profile owner exceeds the source name structure"
    );
    let mut metadata = vec![0; 250];
    metadata[..16].copy_from_slice(&onboard_macro::guid_bytes(&identity.guid)?);
    let name = identity.name.as_bytes();
    let length = name.len().min(100);
    metadata[16..16 + length].copy_from_slice(&name[..length]);
    metadata[116..116 + owner.len()].copy_from_slice(&owner);
    // Reconstruction omits mappingHash, selecting the source zero-filled six bytes.
    Ok(metadata)
}

fn prepare(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    reading: &CompleteReading,
    next: &mut impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
) -> Result<Reconstruction> {
    let snapshot = &reading.snapshot;
    snapshot.validate()?;
    ensure!(
        snapshot.product_id == cap.product_id,
        "Reconstruction belongs to another keyboard"
    );
    let mut result = Reconstruction {
        directory: snapshot
            .inventory
            .slots
            .iter()
            .map(|slot| slot.slot_id)
            .collect(),
        profiles: Vec::new(),
        macros: Vec::new(),
    };
    for (slot, observed) in snapshot.inventory.slots.iter().zip(&snapshot.profiles) {
        if slot.slot_id == 1 {
            continue;
        }
        let ProfileMetadataObservation::Observed { identity, .. } = &slot.metadata else {
            anyhow::bail!(
                "Profile {} metadata is not observed before reconstruction",
                slot.slot_id
            );
        };
        let mut assignments = Vec::new();
        for mode in [Mode::Normal, Mode::Hypershift] {
            let pass = observed
                .passes
                .iter()
                .find(|pass| pass.mode == mode)
                .context("Observed assignment mode is absent")?;
            let mut ordered = BTreeMap::new();
            for batch in &pass.assignments {
                for row in batch
                    .observed()
                    .context("Cannot reconstruct a failed assignment read")?
                {
                    ordered.insert(row.key_id, *row);
                }
            }
            assignments.extend(ordered.into_values());
        }
        result.profiles.push(ProfileBackup {
            slot: slot.slot_id,
            metadata: profile_metadata(identity)?,
            assignments,
        });
    }
    let macros = CurrentKeyboardMacroReader::new(device, cap)?;
    for summary in &snapshot.macros.macros {
        guard()?;
        let saved = reading
            .import_details
            .macros
            .iter()
            .find(|entry| entry.macro_id == summary.metadata.macro_id)
            .and_then(|entry| entry.content.observed());
        if let Some(saved) = saved {
            ensure!(
                saved.metadata == summary.metadata,
                "Macro detail belongs to another observation"
            );
        }
        let needs_read = match saved {
            Some(saved) => saved.keyboard_projection(cap.product_id)?.items.is_empty(),
            None => true,
        };
        let reading = if needs_read {
            macros.read_details(
                summary,
                snapshot.macros.inventory.storage.max_memory,
                next,
                guard,
            )?
        } else {
            saved.expect("Observed nonempty macro detail").clone()
        };
        let rebuilt = onboard_macro_codec::rebuild_observed(cap.product_id, &reading)?;
        result.macros.push(MacroBackup { reading, rebuilt });
    }
    Ok(result)
}

fn io(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    report: &mut WriteReport,
    operation: Operation,
    command: [u8; 3],
    payload: Vec<u8>,
    report_size: u8,
    next: &mut impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    report.steps.push(IoStep {
        operation,
        command,
        payload,
        report_size,
        attempted: false,
        returned_status: None,
        reply: None,
        error: None,
    });
    let step = report.steps.last_mut().unwrap();
    let result = (|| {
        feature_exchange::exchange_with_report_data_size_observing_send(
            device,
            cap,
            command,
            &step.payload,
            next()?,
            report_size,
            guard,
            &mut || step.attempted = true,
        )
    })();
    step.reply = feature_exchange::acknowledged_reply(&result).map(<[u8]>::to_vec);
    step.returned_status = match &result {
        Ok(_) => Some(2),
        Err(error) => feature_exchange::returned_status(error),
    };
    step.error = result.as_ref().err().map(|error| format!("{error:#}"));
    result
}

fn interrupted(error: &anyhow::Error, guard: &impl Fn() -> Result<()>) -> bool {
    crate::backend::exchange_interrupted(error) || guard().is_err()
}

fn source_resolved(result: Result<Vec<u8>>) -> Result<()> {
    match result {
        Ok(_) => Ok(()),
        Err(error) if feature_exchange::returned_status(&error).is_some() => Ok(()),
        Err(error) => Err(error),
    }
}

fn reconstruct(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    report: &mut WriteReport,
    next: &mut impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
    wait: &mut impl FnMut(Duration) -> Result<()>,
) -> Result<()> {
    let backup = report
        .reconstruction
        .as_ref()
        .context("Reconstruction backup is absent")?
        .clone();
    let reset = cap.product_id == 579;
    let mut last_success = true;
    for slot in backup
        .directory
        .iter()
        .skip(if reset { 0 } else { 1 })
        .copied()
    {
        let result = io(
            device,
            cap,
            report,
            if reset {
                Operation::ResetProfile(slot)
            } else {
                Operation::DeleteProfile(slot)
            },
            [1, 5, if reset { 11 } else { 3 }],
            vec![slot],
            1,
            next,
            guard,
        );
        last_success = result.is_ok();
        if let Err(error) = result {
            if interrupted(&error, guard) {
                return Err(error);
            }
        }
    }
    if !last_success {
        return Ok(());
    }
    let started = io(
        device,
        cap,
        report,
        Operation::Format,
        [6, 6, 10],
        vec![0, 0, 0, 2],
        6,
        next,
        guard,
    );
    if let Err(error) = started {
        if interrupted(&error, guard) {
            return Err(error);
        }
        // A returned failing status is a resolved send; FormatStorage still polls.
        if feature_exchange::returned_status(&error).is_none() {
            return Ok(());
        }
    }
    guard()?;
    wait(Duration::from_millis(100))?;
    guard()?;
    let mut successful_reply = false;
    for attempt in 0..3 {
        let result = io(
            device,
            cap,
            report,
            Operation::FormatStatus,
            [6, 6, 138],
            vec![0, 0, 0, 2],
            6,
            next,
            guard,
        );
        match result {
            Ok(data) => {
                ensure!(
                    data.len() >= 4 && data[..2] == [0, 0] && data[3] == 2,
                    "Format reply has another storage/action selector"
                );
                successful_reply = true;
                report.format_statuses.push(data[2]);
                if data[2] == 2 {
                    break;
                }
            }
            Err(error) => {
                if interrupted(&error, guard) {
                    return Err(error);
                }
                if feature_exchange::returned_status(&error).is_some() {
                    successful_reply = false;
                } else if attempt == 2 {
                    return Ok(());
                }
            }
        }
    }
    if !successful_reply {
        return Ok(());
    }
    report.source_rebuild_entered = true;
    let mut names = cap.clone();
    let extended = cap
        .sleep_between_out_in_ms
        .checked_mul(2)
        .context("Profile delay overflow")?;
    names.sleep_between_out_in_ms = names.sleep_between_out_in_ms.max(extended);
    names.sleep_between_in_ms = names.sleep_between_in_ms.max(extended);
    for profile in &backup.profiles {
        let name_result = (|| -> Result<()> {
            source_resolved(io(
                device,
                &names,
                report,
                Operation::CreateProfile(profile.slot),
                [1, 5, 2],
                vec![profile.slot],
                1,
                next,
                guard,
            ))?;
            for offset in (0..250usize).step_by(64) {
                let count = (250 - offset).min(64);
                let mut payload = vec![profile.slot, 0, offset as u8, 0, 250];
                payload.extend_from_slice(&profile.metadata[offset..offset + count]);
                source_resolved(io(
                    device,
                    &names,
                    report,
                    Operation::ProfileName {
                        slot: profile.slot,
                        offset: offset as u16,
                    },
                    [80, 5, 8],
                    payload,
                    (count + 5) as u8,
                    next,
                    guard,
                ))?;
            }
            Ok(())
        })();
        if let Err(error) = name_result {
            if interrupted(&error, guard) {
                return Err(error);
            }
        }
        for assignment in &profile.assignments {
            source_resolved(io(
                device,
                cap,
                report,
                Operation::Assignment {
                    slot: profile.slot,
                    key: assignment.key_id,
                    mode: assignment.mode,
                },
                [80, 2, 13],
                assignment.payload(profile.slot)?,
                10,
                next,
                guard,
            ))?;
        }
    }
    let macros = OnboardMacro::for_firmware_keyboard(device, cap)?;
    for saved in &backup.macros {
        record_macro(
            macros.write_firmware_local(
                saved.reading.metadata.macro_id,
                &saved.rebuilt.request,
                next,
                guard,
            ),
            &mut report.macro_restoration,
            guard,
        )?;
    }
    Ok(())
}

fn record_macro(
    result: std::result::Result<MacroWriteReport, onboard_macro::MacroWriteFailure>,
    entries: &mut Vec<MacroReplay>,
    guard: &impl Fn() -> Result<()>,
) -> Result<()> {
    match result {
        Ok(report) => {
            entries.push(MacroReplay {
                report,
                error: None,
            });
            Ok(())
        }
        Err(error) => {
            let fatal = interrupted(error.error(), guard)
                || (matches!(
                    error.phase,
                    onboard_macro::MacroFailurePhase::Prepare
                        | onboard_macro::MacroFailurePhase::Allocate
                ) && feature_exchange::returned_status(error.error()).is_none());
            let message = error.to_string();
            entries.push(MacroReplay {
                report: error.progress.clone(),
                error: Some(message.clone()),
            });
            if fatal {
                return Err(error.into_error());
            }
            Ok(())
        }
    }
}

/// Only observed fragmentation of these four registered products selects the
/// reconstruction path. Missing local files and read errors never select it.
pub fn write_macro_with_compaction(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    reading: &CompleteReading,
    macro_id: u16,
    request: &MacroWriteRequest,
    next: &mut impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
    wait: &mut impl FnMut(Duration) -> Result<()>,
) -> std::result::Result<WriteReport, WriteFailure> {
    write_macro_with_restore_guard(
        device, cap, reading, macro_id, request, true, next, guard, wait,
    )
}

pub(crate) fn write_macro_with_restore_guard(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    reading: &CompleteReading,
    macro_id: u16,
    request: &MacroWriteRequest,
    restoration_valid: bool,
    next: &mut impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
    wait: &mut impl FnMut(Duration) -> Result<()>,
) -> std::result::Result<WriteReport, WriteFailure> {
    let mut report = WriteReport::default();
    let result = (|| -> Result<()> {
        let macros = OnboardMacro::for_firmware_keyboard(device, cap)?;
        guard()?;
        ensure!(
            reading.snapshot.product_id == cap.product_id,
            "Macro upload observation belongs to another keyboard"
        );
        reading.snapshot.validate()?;
        ensure!(
            macro_id <= 32767 && !reading.snapshot.macros.inventory.ids.contains(&macro_id),
            "Macro ID collides with observed storage"
        );
        onboard_macro::encode_metadata(request)?;
        let required = onboard_macro::required_memory(request.data.len())?;
        let before = macros.read_storage_info(next, guard)?;
        let total = before
            .available_memory
            .checked_add(before.recyclable_memory)
            .context("Macro capacity overflow")?;
        report.storage_before = Some(before.clone());
        ensure!(
            required <= total,
            "Insufficient observed keyboard macro capacity"
        );
        if required > before.available_memory {
            ensure!(
                restoration_valid,
                "Cannot reconstruct storage after an unconfirmed macro mutation"
            );
            report.reconstruction = Some(prepare(device, cap, reading, next, guard)?);
            reconstruct(device, cap, &mut report, next, guard, wait)?;
            let after = macros.read_storage_info(next, guard)?;
            let available = after.available_memory;
            report.storage_after_rebuild = Some(after);
            ensure!(
                required <= available,
                "Keyboard reconstruction did not make sufficient storage available"
            );
        }
        let mut upload = Vec::new();
        let result = record_macro(
            macros.write_firmware_local(macro_id, request, next, guard),
            &mut upload,
            guard,
        );
        report.upload = upload.pop();
        result?;
        report.storage_after = Some(macros.read_storage_info(next, guard)?);
        guard()?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(report),
        Err(error) => Err(WriteFailure {
            progress: report,
            error,
        }),
    }
}

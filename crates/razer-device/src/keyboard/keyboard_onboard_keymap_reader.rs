//! Keypad profile reads keep the target selections and both mode passes in
//! their source order. An interrupted read retains all completed observations.
use super::{AssignmentBatch, KeyBatch, Mode, Reading, read_analog_batches, read_mode, retain};
use crate::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    feature_exchange,
    keyboard_analog::{self, ActuationPoint, RapidTriggerKey},
    keyboard_onboard_inventory,
    keyboard_onboard_keymap::{self as keymap, Address, Inventory, Name},
};
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionPhase {
    Metadata,
    Assignments(Mode),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Selection {
    pub address: Address,
    pub phase: SelectionPhase,
    pub attempted: bool,
    pub acknowledgement: Option<Vec<u8>>,
    pub returned_status: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModeReading {
    pub mode: Mode,
    /// The source still queries this mode after a rejected inner selection.
    /// Such raw responses do not confirm which target keymap supplied them.
    pub target_selected: bool,
    pub assignments: Vec<AssignmentBatch>,
    pub rapid_trigger: Vec<KeyBatch<RapidTriggerKey>>,
    pub actuation: Vec<KeyBatch<ActuationPoint>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct KeymapReading {
    pub address: Address,
    pub name: Reading<Name>,
    pub passes: Vec<ModeReading>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Snapshot {
    pub profile_id: u8,
    pub layout_id: u32,
    pub inventory: Reading<Inventory>,
    pub active_keymap: Reading<u8>,
    pub keymaps: Vec<KeymapReading>,
    pub selections: Vec<Selection>,
}

/// The source catches an individual profile's keymap failure and continues
/// with later profiles. Keep the partial reads and selection acknowledgements.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Observation {
    pub snapshot: Snapshot,
    pub failure: Option<String>,
}

impl Observation {
    pub(super) fn validate_for(&self, profile_id: u8, layout_id: u32) -> Result<()> {
        let snapshot = &self.snapshot;
        ensure!(
            snapshot.profile_id == profile_id && snapshot.layout_id == layout_id,
            "Keymap observation belongs to another profile or layout"
        );
        ensure!(
            self.failure
                .as_ref()
                .is_none_or(|message| !message.is_empty()),
            "Keymap failure has no diagnostic"
        );
        snapshot.inventory.validate_attempt()?;
        let Some(inventory) = snapshot.inventory.observed() else {
            ensure!(
                self.failure.is_some()
                    && matches!(snapshot.active_keymap, Reading::Unobserved)
                    && snapshot.keymaps.is_empty()
                    && snapshot.selections.is_empty(),
                "Failed keymap inventory contains later observations"
            );
            return Ok(());
        };
        ensure!(
            inventory.profile_id() == profile_id,
            "Keymap inventory profile differs"
        );
        snapshot.active_keymap.validate_attempt()?;
        if snapshot.active_keymap.observed().is_none() {
            ensure!(
                self.failure.is_some()
                    && snapshot.keymaps.is_empty()
                    && snapshot.selections.is_empty(),
                "Failed active-keymap read contains later observations"
            );
            return Ok(());
        }
        let ids = selected_ids(inventory);
        ensure!(
            snapshot.keymaps.len() <= ids.len()
                && (self.failure.is_some() || snapshot.keymaps.len() == ids.len()),
            "Keymap observation count differs from its inventory"
        );
        let keys = keyboard_onboard_inventory::inventory(747, layout_id)
            .context("Keypad physical key inventory is absent")?;
        let mut selection_count = 0;
        for (index, (reading, id)) in snapshot.keymaps.iter().zip(ids).enumerate() {
            let address = Address::new(profile_id, id)?;
            ensure!(
                reading.address == address,
                "Keymap observations are reordered"
            );
            let partial = self.failure.is_some() && index + 1 == snapshot.keymaps.len();
            let selection = snapshot
                .selections
                .get(selection_count)
                .context("Keymap metadata selection is absent")?;
            validate_selection(selection, address, SelectionPhase::Metadata)?;
            selection_count += 1;
            if !matches!(reading.name, Reading::Unobserved) {
                ensure!(
                    selection.returned_status.is_some(),
                    "Keymap name read precedes target completion"
                );
                reading.name.validate_attempt()?;
                if let Some(name) = reading.name.observed() {
                    ensure!(
                        name.address() == address,
                        "Keymap name belongs to another address"
                    );
                }
            } else {
                ensure!(
                    partial && reading.passes.is_empty(),
                    "Unobserved keymap name has subsequent reads"
                );
            }
            if selection.returned_status != Some(2) {
                ensure!(
                    reading.passes.is_empty(),
                    "Rejected keymap selection has mapping reads"
                );
                continue;
            }
            ensure!(
                reading.passes.len() <= 2 && (partial || reading.passes.len() == 2),
                "Keymap mode passes are missing"
            );
            for (pass_index, (pass, mode)) in reading
                .passes
                .iter()
                .zip([Mode::Normal, Mode::Hypershift])
                .enumerate()
            {
                ensure!(pass.mode == mode, "Keymap modes are reordered");
                let selection = snapshot
                    .selections
                    .get(selection_count)
                    .context("Keymap assignment selection is absent")?;
                validate_selection(selection, address, SelectionPhase::Assignments(mode))?;
                ensure!(
                    selection.returned_status.is_some()
                        && pass.target_selected == (selection.returned_status == Some(2)),
                    "Keymap assignments differ from their target selection result"
                );
                selection_count += 1;
                let ids = keys
                    .iter()
                    .filter(|key| match mode {
                        Mode::Normal => !key.ignore_normal,
                        Mode::Hypershift => !key.ignore_hypershift,
                    })
                    .map(|key| key.firmware_key_id)
                    .collect::<Vec<_>>();
                super::validate_assignment_batches(&pass.assignments, &ids, mode, true)?;
                let incomplete = partial && pass_index + 1 == reading.passes.len();
                if incomplete && pass.rapid_trigger.is_empty() && pass.actuation.is_empty() {
                    continue;
                }
                super::validate_analog_batches(&pass.rapid_trigger, &pass.actuation, &ids, true)?;
            }
        }
        if snapshot.selections.len() != selection_count {
            ensure!(
                self.failure.is_some() && snapshot.selections.len() == selection_count + 1,
                "Unexpected keymap target selection"
            );
            let reading = snapshot
                .keymaps
                .last()
                .context("Keymap selection has no scope")?;
            let phase = match reading.passes.len() {
                0 => SelectionPhase::Assignments(Mode::Normal),
                1 => SelectionPhase::Assignments(Mode::Hypershift),
                _ => anyhow::bail!("Keymap selection follows complete modes"),
            };
            validate_selection(
                &snapshot.selections[selection_count],
                reading.address,
                phase,
            )?;
        }
        Ok(())
    }
}

fn validate_selection(
    selection: &Selection,
    address: Address,
    phase: SelectionPhase,
) -> Result<()> {
    ensure!(
        selection.address == address && selection.phase == phase,
        "Keymap target selection belongs to another scope"
    );
    ensure!(
        (selection.acknowledgement.is_some() == (selection.returned_status == Some(2)))
            && (selection.returned_status.is_none() || selection.attempted),
        "Keymap selection result has no send or a different status"
    );
    Ok(())
}

fn selected_ids(inventory: &Inventory) -> Vec<u8> {
    if inventory.keymap_ids().is_empty() {
        (1..=inventory.total_enabled()).collect()
    } else {
        inventory.keymap_ids().to_vec()
    }
}

pub(super) fn observe(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    profile_id: u8,
    layout_id: u32,
    next: impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
) -> Result<Observation> {
    let observation = match read(device, cap, profile_id, layout_id, next, guard) {
        Ok(snapshot) => Observation {
            snapshot,
            failure: None,
        },
        Err(failure) => {
            let (snapshot, error) = failure.into_parts();
            if crate::backend::exchange_interrupted(&error) {
                return Err(error);
            }
            guard().map_err(|error| {
                crate::backend::interrupt_exchange(
                    error,
                    crate::backend::ExchangeInterruption::Validation,
                )
            })?;
            Observation {
                snapshot,
                failure: Some(format!("{error:#}")),
            }
        }
    };
    observation.validate_for(profile_id, layout_id)?;
    Ok(observation)
}

#[derive(Debug)]
pub struct Failure {
    snapshot: Snapshot,
    error: anyhow::Error,
}

impl Failure {
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
    pub fn error(&self) -> &anyhow::Error {
        &self.error
    }
    pub fn into_parts(self) -> (Snapshot, anyhow::Error) {
        (self.snapshot, self.error)
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Keymap profile read failed: {:#}", self.error)
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.error.as_ref())
    }
}

fn select(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    address: Address,
    phase: SelectionPhase,
    next: &mut impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
    selections: &mut Vec<Selection>,
) -> Result<bool> {
    selections.push(Selection {
        address,
        phase,
        attempted: false,
        acknowledgement: None,
        returned_status: None,
    });
    let selection = selections.last_mut().unwrap();
    let result = keymap::select_target_observing_send(
        device,
        cap,
        address.keymap_id(),
        keymap::next_transaction(next, guard)?,
        guard,
        &mut || selection.attempted = true,
    );
    selection.acknowledgement = feature_exchange::acknowledged_reply(&result).map(<[u8]>::to_vec);
    match result {
        Ok(_) => {
            selection.returned_status = Some(2);
            Ok(true)
        }
        Err(error) => match feature_exchange::returned_status(&error) {
            Some(status) => {
                selection.returned_status = Some(status);
                Ok(false)
            }
            None => Err(error),
        },
    }
}

/// This read includes volatile target-keymap writes. It is not a readonly
/// diagnostic. The caller serializes the entire operation on one physical
/// session, retains the source transaction allocator, and owns its release.
pub fn read(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    profile_id: u8,
    layout_id: u32,
    mut next: impl FnMut() -> Result<u8>,
    guard: &impl Fn() -> Result<()>,
) -> std::result::Result<Snapshot, Failure> {
    let mut snapshot = Snapshot {
        profile_id,
        layout_id,
        inventory: Reading::Unobserved,
        active_keymap: Reading::Unobserved,
        keymaps: Vec::new(),
        selections: Vec::new(),
    };
    let outcome = (|| {
        keymap::validate_capability(cap)?;
        ensure!(profile_id > 0, "Keymap profile ID is zero");
        let keys = keyboard_onboard_inventory::inventory(cap.product_id, layout_id)
            .context("Current keymap physical key inventory is absent")?;
        let inventory = match keymap::read_inventory(
            device,
            cap,
            profile_id,
            keymap::next_transaction(&mut next, guard)?,
            guard,
        ) {
            Ok(inventory) => inventory,
            Err(error) => {
                snapshot.inventory = Reading::Failed {
                    message: format!("{error:#}"),
                };
                return Err(error);
            }
        };
        let ids = selected_ids(&inventory);
        snapshot.inventory = Reading::Observed(inventory);
        let active = match keymap::read_active(
            device,
            cap,
            profile_id,
            keymap::next_transaction(&mut next, guard)?,
            guard,
        ) {
            Ok(active) => active,
            Err(error) => {
                snapshot.active_keymap = Reading::Failed {
                    message: format!("{error:#}"),
                };
                return Err(error);
            }
        };
        snapshot.active_keymap = Reading::Observed(active);
        for id in ids {
            let address = Address::new(profile_id, id)?;
            snapshot.keymaps.push(KeymapReading {
                address,
                name: Reading::Unobserved,
                passes: Vec::new(),
            });
            let selected = select(
                device,
                cap,
                address,
                SelectionPhase::Metadata,
                &mut next,
                guard,
                &mut snapshot.selections,
            )?;
            let reading = snapshot.keymaps.last_mut().unwrap();
            reading.name = retain(
                keymap::read_name(device, cap, address, &mut next, guard),
                guard,
            )?;
            if !selected {
                continue;
            }
            for mode in [Mode::Normal, Mode::Hypershift] {
                let target_selected = select(
                    device,
                    cap,
                    address,
                    SelectionPhase::Assignments(mode),
                    &mut next,
                    guard,
                    &mut snapshot.selections,
                )?;
                let ids = keys
                    .iter()
                    .filter(|key| match mode {
                        Mode::Normal => !key.ignore_normal,
                        Mode::Hypershift => !key.ignore_hypershift,
                    })
                    .map(|key| key.firmware_key_id)
                    .collect::<Vec<_>>();
                let mut allocate = || keymap::next_transaction(&mut next, guard);
                let assignments =
                    read_mode(device, cap, profile_id, mode, &ids, &mut allocate, guard)?;
                reading.passes.push(ModeReading {
                    mode,
                    target_selected,
                    assignments: assignments.assignments,
                    rapid_trigger: Vec::new(),
                    actuation: Vec::new(),
                });
                let analog = read_analog_batches(
                    profile_id,
                    &ids,
                    &mut allocate,
                    guard,
                    |query, transaction| {
                        keyboard_analog::read_selected_keymap(
                            device,
                            cap,
                            id,
                            query,
                            transaction,
                            guard,
                        )
                    },
                )?;
                let pass = reading.passes.last_mut().unwrap();
                pass.rapid_trigger = analog.rapid_trigger;
                pass.actuation = analog.actuation;
            }
        }
        guard()?;
        Ok(())
    })();
    match outcome {
        Ok(()) => Ok(snapshot),
        Err(error) => Err(Failure { snapshot, error }),
    }
}

#[cfg(test)]
#[path = "keyboard_onboard_keymap_reader_tests.rs"]
mod tests;

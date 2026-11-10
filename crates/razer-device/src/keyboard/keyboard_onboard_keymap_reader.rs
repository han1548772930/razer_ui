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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ModeReading {
    pub mode: Mode,
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
) -> Result<()> {
    selections.push(Selection {
        address,
        phase,
        attempted: false,
        acknowledgement: None,
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
    result.map(|_| ())
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
        let ids = if inventory.keymap_ids().is_empty() {
            (1..=inventory.total_enabled()).collect::<Vec<_>>()
        } else {
            inventory.keymap_ids().to_vec()
        };
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
            select(
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
            for mode in [Mode::Normal, Mode::Hypershift] {
                select(
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

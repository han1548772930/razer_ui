//! Current Ec/Jl macro reconciliation before ordinary magnetic OBM mappings.
//! Firmware identity, locked references and available storage are observed at
//! execution; the shell supplies editor data and OS entropy, never firmware IDs.
use super::{ProfileIdentity, Receipt, Submission};
use crate::{
    backend::FeatureTransport,
    feature_exchange,
    keyboard_analog::KeyboardAnalogTransportCapability,
    onboard_macro::{
        KeyboardMacroMemory, MacroCatalog, MacroFailurePhase, MacroWriteFailure, MacroWriteReport,
        OnboardMacro, allocate_macro_id,
    },
    onboard_macro_codec::MacroSynchronization,
};
use anyhow::{Context as _, ensure};
use serde::Serialize;
use std::collections::{BTreeSet, VecDeque};

#[derive(Clone, Debug, Serialize)]
pub struct MacroMutation {
    pub removed: bool,
    pub progress: MacroWriteReport,
    pub failure_phase: Option<MacroFailurePhase>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MacroProgress {
    pub before: Option<MacroCatalog>,
    pub after: Option<MacroCatalog>,
    pub locked_ids: BTreeSet<u16>,
    pub mutations: Vec<MacroMutation>,
    pub(crate) calculated_memory: Option<KeyboardMacroMemory>,
}

fn record_mutation(
    receipt: &mut Receipt,
    removed: bool,
    result: Result<MacroWriteReport, MacroWriteFailure>,
) -> Result<(), MacroWriteFailure> {
    let (progress, failure_phase, message) = match &result {
        Ok(progress) => (progress.clone(), None, None),
        Err(error) => (
            error.progress.clone(),
            Some(error.phase),
            Some(error.to_string()),
        ),
    };
    receipt
        .macro_synchronization
        .as_mut()
        .unwrap()
        .mutations
        .push(MacroMutation {
            removed,
            progress,
            failure_phase,
            error: message,
        });
    result.map(|_| ())
}

fn locked_macro_ids(
    device: &dyn FeatureTransport,
    cap: &KeyboardAnalogTransportCapability,
    profiles: &[ProfileIdentity],
    context: &MacroSynchronization,
    receipt: &mut Receipt,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    guard: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<BTreeSet<u16>> {
    let locked = profiles
        .iter()
        .filter(|profile| {
            profile.profile_id != 1
                && (profile_is_locked(profile)
                    || !context
                        .profiles
                        .iter()
                        .any(|local| local.guid == profile.guid))
        })
        .collect::<Vec<_>>();
    if locked.is_empty() {
        return Ok(BTreeSet::new());
    }
    let transport = cap.device_read_capability();
    let keys = feature_exchange::exchange(device, &transport, [80, 2, 132], &[], next()?, guard)?;
    let count = usize::from(*keys.first().context("Firmware button list is empty")?);
    ensure!(
        count > 0 && keys.len() > count,
        "Firmware button list is truncated"
    );
    let keys = &keys[1..1 + count];
    ensure!(
        keys.iter().all(|key| *key > 0)
            && keys.iter().copied().collect::<BTreeSet<_>>().len() == keys.len(),
        "Firmware button list has invalid/repeated identities"
    );
    let mut result = BTreeSet::new();
    for profile in locked {
        for mode in [0, 1] {
            for keys in keys.chunks(8) {
                let mut payload = vec![profile.profile_id];
                for &key in keys {
                    payload.extend_from_slice(&[key, mode, 0, 0, 0, 0, 0, 0, 0]);
                }
                receipt.submissions.push(Submission {
                    profile_id: profile.profile_id,
                    command: [80, 2, 158],
                    payload,
                    attempted: false,
                    acknowledgement: None,
                });
                let step = receipt.submissions.last_mut().unwrap();
                let response = feature_exchange::exchange_with_report_data_size_observing_send(
                    device,
                    &transport,
                    step.command,
                    &step.payload,
                    next()?,
                    step.payload.len() as u8,
                    guard,
                    &mut || step.attempted = true,
                )?;
                step.acknowledgement = Some(response.clone());
                ensure!(
                    response.len() == 1 + 9 * keys.len() && response[0] == profile.profile_id,
                    "Locked profile assignment response has a wrong size/slot"
                );
                for (&key, assignment) in keys.iter().zip(response[1..].chunks_exact(9)) {
                    ensure!(
                        assignment[0] == key && assignment[1] == mode && assignment[3] <= 5,
                        "Locked assignment response belongs to another key/mode"
                    );
                    if [3, 4, 5, 15].contains(&assignment[2]) {
                        ensure!(assignment[3] >= 2, "Locked macro assignment is truncated");
                        result.insert(u16::from_be_bytes([assignment[4], assignment[5]]));
                    }
                }
            }
        }
    }
    Ok(result)
}

fn profile_is_locked(profile: &ProfileIdentity) -> bool {
    let raw = &profile.raw_metadata;
    if raw.len() != 250 {
        return true;
    }
    let name = &raw[16..116];
    let end = name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(name.len() - 1);
    let factory = &name[..end] == b"Factory Default";
    let owner = &raw[116..180];
    let owner = owner
        .iter()
        .copied()
        .filter(|byte| *byte != 0)
        .collect::<Vec<_>>();
    let preset = crate::keyboard_magnetic_mapping::is_onboard_preset(&profile.guid);
    // Local-first ownership has the source guest identity. Other owners keep
    // their hardware references protected even if a local GUID happens to match.
    !((preset && !factory)
        || (!(preset && factory && profile.profile_id == 2)
            && owner == crate::keyboard_magnetic_mapping::guest_profile_owner().as_bytes()))
}

pub(super) fn synchronize(
    device: &dyn FeatureTransport,
    cap: &KeyboardAnalogTransportCapability,
    active_guid: &str,
    context: &MacroSynchronization,
    receipt: &mut Receipt,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    guard: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<MacroCatalog> {
    ensure!(
        context
            .profiles
            .iter()
            .filter(|profile| profile.guid == active_guid)
            .count()
            == 1,
        "Macro synchronization active profile is absent or repeated"
    );
    let macros = OnboardMacro::for_keyboard(device, cap)?;
    receipt.macro_synchronization = Some(MacroProgress::default());
    let before = macros.load_macros(next, guard)?;
    receipt.macro_synchronization.as_mut().unwrap().before = Some(before.clone());
    let profiles = receipt.inventory.clone();
    let mut memory = KeyboardMacroMemory::observe(&before, profiles.len())?;
    receipt
        .macro_synchronization
        .as_mut()
        .unwrap()
        .calculated_memory = Some(memory);
    let locked = locked_macro_ids(device, cap, &profiles, context, receipt, next, guard)?;
    receipt.macro_synchronization.as_mut().unwrap().locked_ids = locked.clone();
    let hardware_profiles = profiles
        .iter()
        .filter(|profile| !profile_is_locked(profile))
        .map(|profile| profile.guid.as_str())
        .collect::<BTreeSet<_>>();
    let mut required = Vec::<String>::new();
    let mut standby = BTreeSet::new();
    for profile in profiles
        .iter()
        .filter(|profile| !profile_is_locked(profile))
    {
        if let Some(local) = context
            .profiles
            .iter()
            .find(|local| local.guid == profile.guid)
        {
            for guid in &local.macro_guids {
                if !required.contains(guid) {
                    required.push(guid.clone());
                }
            }
        }
    }
    let active = context
        .profiles
        .iter()
        .find(|profile| profile.guid == active_guid)
        .unwrap();
    for guid in &active.macro_guids {
        if !required.contains(guid) {
            required.push(guid.clone());
        }
    }
    for profile in &context.profiles {
        let needed =
            profile.guid == active_guid || hardware_profiles.contains(profile.guid.as_str());
        for guid in &profile.macro_guids {
            if !needed {
                standby.insert(guid.as_str());
            }
        }
    }
    let supported = context
        .macros
        .iter()
        .map(|request| request.guid.as_str())
        .collect::<BTreeSet<_>>();
    required.retain(|guid| supported.contains(guid.as_str()));
    let mut observed_ids = before.inventory.ids.clone();
    for entry in &before.macros {
        let id = entry.metadata.macro_id;
        let guid = entry.metadata.guid.as_str();
        if required.iter().any(|required| required == guid)
            || standby.contains(guid)
            || locked.contains(&id)
        {
            continue;
        }
        record_mutation(receipt, true, macros.delete_macro(id, next, guard))?;
        memory.released(entry.allocated_size_with_name)?;
        receipt
            .macro_synchronization
            .as_mut()
            .unwrap()
            .calculated_memory = Some(memory);
        observed_ids.retain(|saved| *saved != id);
    }
    let mut candidates = VecDeque::from(context.random_candidates.clone());
    for guid in required {
        if before
            .macros
            .iter()
            .any(|entry| entry.metadata.guid == guid)
        {
            continue;
        }
        let request = context
            .macros
            .iter()
            .find(|request| request.guid == guid)
            .context("Required macro source vanished")?;
        let id = allocate_macro_id(
            &observed_ids,
            || {
                candidates
                    .pop_front()
                    .context("Macro OS entropy collision bound exhausted")
            },
            context.random_candidates.len(),
        )?;
        let mut added = record_mutation(
            receipt,
            false,
            macros.write_keyboard_macro(id, request, &mut memory, next, guard),
        );
        if added
            .as_ref()
            .is_err_and(|error| error.phase == MacroFailurePhase::InsufficientStorage)
        {
            // Source Lt evicts standby entries only after an actual capacity
            // rejection. ID zero is omitted by its `isStandByMacro && id` test.
            for candidate in &before.macros {
                let candidate_id = candidate.metadata.macro_id;
                if candidate_id == 0
                    || !observed_ids.contains(&candidate_id)
                    || locked.contains(&candidate_id)
                    || !standby.contains(candidate.metadata.guid.as_str())
                    || required_contains(
                        context,
                        &hardware_profiles,
                        active_guid,
                        &candidate.metadata.guid,
                    )
                {
                    continue;
                }
                match record_mutation(
                    receipt,
                    true,
                    macros.delete_macro(candidate_id, next, guard),
                ) {
                    Ok(()) => {
                        observed_ids.retain(|saved| *saved != candidate_id);
                        memory.released(candidate.allocated_size_with_name)?;
                    }
                    Err(error) => {
                        guard()?;
                        // Lt retains a failed delete and retries the add. A
                        // non-capacity add error still stops subsequent writes.
                        if error.phase == MacroFailurePhase::Cancelled {
                            return Err(error.into());
                        }
                    }
                }
                added = record_mutation(
                    receipt,
                    false,
                    macros.write_keyboard_macro(id, request, &mut memory, next, guard),
                );
                if !added
                    .as_ref()
                    .is_err_and(|error| error.phase == MacroFailurePhase::InsufficientStorage)
                {
                    break;
                }
            }
        }
        receipt
            .macro_synchronization
            .as_mut()
            .unwrap()
            .calculated_memory = Some(memory);
        added?;
        observed_ids.push(id);
    }
    let after = macros.load_macros(next, guard)?;
    receipt.macro_synchronization.as_mut().unwrap().after = Some(after.clone());
    Ok(after)
}

fn required_contains(
    context: &MacroSynchronization,
    hardware_profiles: &BTreeSet<&str>,
    active_guid: &str,
    macro_guid: &str,
) -> bool {
    context.profiles.iter().any(|profile| {
        (profile.guid == active_guid || hardware_profiles.contains(profile.guid.as_str()))
            && profile.macro_guids.iter().any(|guid| guid == macro_guid)
    })
}

#[cfg(test)]
#[path = "keyboard_macro_submission_tests.rs"]
mod tests;

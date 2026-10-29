//! Current226 It DPI initialization, using raw OBM identities for selection.
//! Xc's projected identities are retained separately for later edit tasks.
use super::{
    GET_LEVEL, GET_STAGES, Transport, exchange, pack_for, read_inventory, read_profile_identity,
    stage_reading, synchronize_slot_ids, validate_stage_reading,
};
use crate::{
    mouse_current_dpi::{DpiPair, SourceStages},
    mouse_dpi_stages::{DpiSlotWriteResult, DpiStagesReading, ObservedProfileSlot},
    mouse_onboard::{OnboardIdentityObservation, OnboardInventory},
    mouse_runtime_artwork::WindowsBleTarget,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "mouse_current_dpi_ble_initialization_tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocalProfile {
    pub guid: String,
    pub name: String,
    pub obm_slot_ids: Vec<u8>,
    pub dpi_stages: Option<SourceStages>,
}

/// The actual ordered profile store and its current selection, never a preview
/// template or a default inferred from a missing file/device read failure.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub profiles: Vec<LocalProfile>,
    pub active_profile_guid: String,
    pub owner: String,
}

impl Request {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            !self.profiles.is_empty()
                && self
                    .profiles
                    .iter()
                    .any(|profile| profile.guid == self.active_profile_guid),
            "BLE initialization lacks the actual current local profile"
        );
        for (index, profile) in self.profiles.iter().enumerate() {
            ensure!(
                !profile.guid.is_empty()
                    && !self.profiles[..index]
                        .iter()
                        .any(|previous| previous.guid == profile.guid),
                "BLE initialization local profile identities are empty or duplicated"
            );
            validate_ids(&profile.obm_slot_ids)?;
            if let Some(stages) = &profile.dpi_stages {
                pack_for(226, stages)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Observation<T> {
    Unobserved,
    Observed { value: T },
    Failed { error: String },
}

impl<T> Observation<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Observed { value } => Some(value),
            _ => None,
        }
    }

    fn from_result(value: anyhow::Result<T>) -> Self {
        match value {
            Ok(value) => Self::Observed { value },
            Err(error) => Self::Failed {
                error: format!("{error:#}"),
            },
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        if let Self::Failed { error } = self {
            ensure!(
                !error.is_empty(),
                "BLE initialization failure omits its cause"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProfileObservation {
    pub slot_id: u8,
    /// The inventory's first entry is deliberately unnamed by the engine.
    pub identity: Observation<OnboardIdentityObservation>,
    /// Raw slot DPI is not constrained to the editor's selectable range.
    pub active: Observation<DpiPair>,
    pub stages: Observation<DpiStagesReading>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Snapshot {
    pub inventory: OnboardInventory,
    pub profiles: Vec<ProfileObservation>,
}

impl Snapshot {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_ids(&self.inventory.profile_ids)?;
        ensure!(
            !self.inventory.profile_ids.is_empty()
                && self.inventory.profile_ids.len() == usize::from(self.inventory.profile_count)
                && self.inventory.profile_count <= self.inventory.max_profiles
                && self.profiles.len() == self.inventory.profile_ids.len(),
            "BLE initialization inventory boundaries differ"
        );
        for (index, profile) in self.profiles.iter().enumerate() {
            ensure!(
                profile.slot_id == self.inventory.profile_ids[index],
                "BLE initialization slot order differs"
            );
            profile.identity.validate()?;
            profile.active.validate()?;
            profile.stages.validate()?;
            if let Some(identity) = profile.identity.value() {
                ensure!(
                    index != 0
                        && identity.profile_id == profile.slot_id
                        && identity.raw_metadata.len() == 250
                        && (1..=10).contains(&identity.attempts)
                        && identity
                            == &crate::mouse_onboard::decode_identity(
                                226,
                                profile.slot_id,
                                identity.raw_metadata.clone(),
                                identity.attempts
                            ),
                    "BLE initialization identity differs from actual metadata"
                );
            }
            if let Some(stages) = profile.stages.value() {
                ensure!(
                    profile.active.value().is_some(),
                    "BLE OBM stage getter has no preceding level result"
                );
                validate_stage_reading(stages, profile.slot_id)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReadReport {
    pub snapshot: Option<Snapshot>,
    pub error: Option<String>,
}

/// DPI slice of loadProfilesFromDevice: inventory and all identities precede
/// the ordered per-slot level/stage reads. A full OBM owner places this after
/// macro loading and before scroll/brightness/polling/mapping observation.
pub fn read_profiles(
    transport: &mut dyn Transport,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> ReadReport {
    let mut report = ReadReport {
        snapshot: None,
        error: None,
    };
    let result = (|| -> anyhow::Result<()> {
        let inventory = read_inventory(transport, validate)?;
        let profiles = inventory
            .profile_ids
            .iter()
            .map(|&slot_id| ProfileObservation {
                slot_id,
                identity: Observation::Unobserved,
                active: Observation::Unobserved,
                stages: Observation::Unobserved,
            })
            .collect();
        report.snapshot = Some(Snapshot {
            inventory,
            profiles,
        });
        let snapshot = report.snapshot.as_mut().expect("actual BLE inventory");
        for profile in snapshot.profiles.iter_mut().skip(1) {
            validate()?;
            profile.identity = Observation::from_result(read_profile_identity(
                transport,
                profile.slot_id,
                validate,
            ));
            validate()?;
        }
        for profile in &mut snapshot.profiles {
            validate()?;
            profile.active = Observation::from_result((|| -> anyhow::Result<DpiPair> {
                let ack = exchange(transport, GET_LEVEL, profile.slot_id, &[], validate)?;
                ensure!(ack.data.len() >= 6, "BLE OBM level response is truncated");
                Ok(DpiPair {
                    x: u16::from_le_bytes([ack.data[0], ack.data[1]]),
                    y: u16::from_le_bytes([ack.data[2], ack.data[3]]),
                })
            })());
            validate()?;
            // The engine catches a level failure before requesting stages.
            if profile.active.value().is_some() {
                profile.stages = Observation::from_result((|| {
                    stage_reading(
                        &exchange(transport, GET_STAGES, profile.slot_id, &[], validate)?,
                        profile.slot_id,
                    )
                })());
            }
            validate()?;
        }
        snapshot.validate()?;
        Ok(())
    })();
    if let Err(error) = result {
        report.error = Some(format!("{error:#}"));
    }
    report
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionOrigin {
    DeviceIdentity,
    LocalProfile,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectedProfile {
    pub slot_id: u8,
    pub guid: String,
    pub name: String,
    pub owner: String,
    pub origin: ProjectionOrigin,
}

/// Same-connection projected obmData used by the source's later $e edit lookup.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObservedSlots {
    pub target: WindowsBleTarget,
    pub connection_owner: String,
    pub generation: u64,
    pub profiles: Vec<ObservedProfileSlot>,
}

impl ObservedSlots {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            super::accepts(self.target.product_id, self.target.physical_product_id)
                && !self.connection_owner.is_empty()
                && !self.target.device_container_id.is_empty()
                && !self.target.hid_path.is_empty()
                && !self.target.device_instance_id.is_empty()
                && !self.target.parent_instance_id.is_empty()
                && self.target.mac_address != 0,
            "BLE OBM observation has no connection identity"
        );
        let ids: Vec<_> = self
            .profiles
            .iter()
            .map(|profile| profile.slot_id)
            .collect();
        validate_ids(&ids)?;
        ensure!(
            self.profiles.iter().all(|profile| profile.slot_id != 1),
            "BLE projected associations include reserved slot1"
        );
        Ok(())
    }

    pub fn validate_target(
        &self,
        target: &WindowsBleTarget,
        connection_owner: &str,
        generation: u64,
    ) -> anyhow::Result<()> {
        self.validate()?;
        ensure!(
            &self.target == target
                && self.connection_owner == connection_owner
                && self.generation == generation,
            "BLE OBM observation belongs to another connection"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProfileSynchronization {
    pub guid: String,
    pub selected_slots: Vec<u8>,
    pub differing_slots: Vec<u8>,
    pub writes: Vec<DpiSlotWriteResult>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Report {
    pub request: Request,
    pub read: ReadReport,
    pub profiles: Vec<ProfileSynchronization>,
    pub projected_profiles: Vec<ProjectedProfile>,
    /// All applicable source DPI tasks completed; ACKs are not slot readback.
    pub completed: bool,
    pub error: Option<String>,
}

impl Report {
    pub fn observed_slots(
        &self,
        target: WindowsBleTarget,
        connection_owner: String,
        generation: u64,
    ) -> anyhow::Result<ObservedSlots> {
        self.validate()?;
        ensure!(self.completed, "BLE initialization is incomplete");
        let observed = ObservedSlots {
            target,
            connection_owner,
            generation,
            profiles: self
                .projected_profiles
                .iter()
                .map(|profile| ObservedProfileSlot {
                    slot_id: profile.slot_id,
                    guid: profile.guid.clone(),
                })
                .collect(),
        };
        observed.validate()?;
        Ok(observed)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        self.validate_reply(&self.request)
    }

    pub fn validate_reply(&self, request: &Request) -> anyhow::Result<()> {
        self.request.validate()?;
        ensure!(
            &self.request == request,
            "BLE initialization reply belongs to another local profile request"
        );
        ensure!(
            self.completed == self.error.is_none(),
            "BLE initialization status omits its failure"
        );
        for error in [self.error.as_deref(), self.read.error.as_deref()]
            .into_iter()
            .flatten()
        {
            ensure!(
                !error.is_empty(),
                "BLE initialization failure omits its cause"
            );
        }
        if let Some(snapshot) = &self.read.snapshot {
            snapshot.validate()?;
        }
        ensure!(
            self.profiles.len() <= self.request.profiles.len(),
            "BLE initialization has unexpected local profile results"
        );
        ensure!(
            !self.completed
                || self.error.is_none()
                    && self.read.error.is_none()
                    && self.profiles.len() == self.request.profiles.len(),
            "BLE initialization completion contradicts its progress"
        );
        if let Some(snapshot) = &self.read.snapshot {
            for (progress, local) in self.profiles.iter().zip(&self.request.profiles) {
                let selected = selected_slots(snapshot, local, request);
                ensure!(
                    progress.guid == local.guid && progress.selected_slots == selected,
                    "BLE initialization profile selection differs from raw identities"
                );
                let mut expected = Vec::new();
                if let Some(stages) = &local.dpi_stages {
                    for &slot in &selected {
                        let observation = snapshot
                            .profiles
                            .iter()
                            .find(|profile| profile.slot_id == slot)
                            .expect("selected inventory slot");
                        let difference = observation
                            .stages
                            .value()
                            .context("BLE initialization stages are unobserved")
                            .and_then(|observed| differs(stages, observed));
                        match difference {
                            Ok(true) => expected.push(slot),
                            Ok(false) => {}
                            Err(_) => {
                                ensure!(
                                    progress.error.is_some(),
                                    "BLE initialization used unobserved stage fields"
                                );
                                break;
                            }
                        }
                    }
                }
                ensure!(
                    progress.differing_slots == expected && progress.writes.len() <= expected.len(),
                    "BLE initialization task slots differ from observed DPI changes"
                );
                for (write, &slot) in progress.writes.iter().zip(&expected) {
                    ensure!(
                        write.slot_id == slot,
                        "BLE initialization write order differs"
                    );
                    if let Some(before) = &write.before {
                        validate_stage_reading(before, slot)?;
                        let packed = pack_for(
                            226,
                            local
                                .dpi_stages
                                .as_ref()
                                .context("BLE initialization wrote an absent local DPI profile")?,
                        )?;
                        ensure!(
                            write.was_equal == super::same_slot(&packed, before),
                            "BLE initialization equality claim differs from real slot getter"
                        );
                    } else {
                        ensure!(
                            !write.was_equal
                                && write.acknowledged.is_none()
                                && write.error.is_some(),
                            "BLE initialization mutation lacks its preceding getter"
                        );
                    }
                    if let Some(ack) = &write.acknowledged {
                        ensure!(
                            write.before.is_some()
                                && !write.was_equal
                                && ack.response_bytes == ack.raw_data.len()
                                && ack.response_bytes <= 65535,
                            "BLE initialization setter acknowledgement is invalid"
                        );
                    }
                    ensure!(
                        !write.was_equal || write.acknowledged.is_none(),
                        "BLE initialization wrote an equal slot"
                    );
                    if write.error.is_none() {
                        ensure!(
                            write.was_equal || write.acknowledged.is_some(),
                            "BLE initialization slot claims success without a getter or acknowledgement"
                        );
                    }
                }
                if progress.error.is_none() {
                    ensure!(
                        progress.writes.len() == expected.len(),
                        "BLE initialization profile omitted its source tasks"
                    );
                }
            }
        } else {
            ensure!(
                self.profiles.is_empty() && self.projected_profiles.is_empty(),
                "BLE initialization has no raw inventory for its results"
            );
        }
        if self.completed {
            let snapshot = self
                .read
                .snapshot
                .as_ref()
                .context("BLE initialization completed without inventory")?;
            ensure!(
                self.projected_profiles == project_profiles(snapshot, &self.request)?,
                "BLE projection differs from initialization inputs"
            );
            for (profile, local) in self.profiles.iter().zip(&self.request.profiles) {
                ensure!(
                    profile.guid == local.guid
                        && profile.error.is_none()
                        && profile.writes.iter().all(|write| write.error.is_none()),
                    "BLE initialization completed with a failed profile"
                );
            }
        }
        Ok(())
    }
}

fn validate_ids(ids: &[u8]) -> anyhow::Result<()> {
    for (index, id) in ids.iter().enumerate() {
        ensure!(
            *id != 0 && !ids[..index].contains(id),
            "BLE profile IDs are zero or duplicated"
        );
    }
    Ok(())
}

fn project_profiles(
    snapshot: &Snapshot,
    request: &Request,
) -> anyhow::Result<Vec<ProjectedProfile>> {
    let mut profiles = Vec::new();
    // r.EX omits numeric slot1; identity reads omit the first inventory entry.
    for observed in snapshot
        .profiles
        .iter()
        .filter(|profile| profile.slot_id != 1)
    {
        let identity = match &observed.identity {
            Observation::Observed { value } => Some(value),
            Observation::Unobserved
                if snapshot.inventory.profile_ids.first() == Some(&observed.slot_id) =>
            {
                None
            }
            _ => anyhow::bail!(
                "BLE profile identity was not observed for slot {}",
                observed.slot_id
            ),
        };
        let name = identity.map_or("", |identity| identity.name.as_str());
        let invalid_name = name.is_empty() || name.contains("\u{fffd}\u{fffd}\u{fffd}\u{fffd}");
        let profile = if invalid_name {
            let local = request
                .profiles
                .iter()
                .find(|local| local.obm_slot_ids.contains(&observed.slot_id))
                .or_else(|| request.profiles.first())
                .context("BLE projection lacks local profile store")?;
            ProjectedProfile {
                slot_id: observed.slot_id,
                guid: local.guid.clone(),
                name: local.name.clone(),
                owner: request.owner.clone(),
                origin: ProjectionOrigin::LocalProfile,
            }
        } else {
            let identity = identity.expect("nonempty actual metadata name");
            ProjectedProfile {
                slot_id: observed.slot_id,
                guid: identity.guid.clone(),
                name: name.to_owned(),
                owner: identity.owner.replace('\0', ""),
                origin: ProjectionOrigin::DeviceIdentity,
            }
        };
        profiles.push(profile);
    }
    Ok(profiles)
}

fn selected_slots(snapshot: &Snapshot, local: &LocalProfile, request: &Request) -> Vec<u8> {
    snapshot
        .profiles
        .iter()
        .filter(|profile| {
            profile.slot_id == 1 && local.guid == request.active_profile_guid
                || profile.identity.value().is_some_and(|identity| {
                    identity.guid == local.guid
                        && identity.owner.replace('\0', "")
                            == crate::source_string::trim(&request.owner)
                })
        })
        .map(|profile| profile.slot_id)
        .collect()
}

fn differs(local: &SourceStages, observed: &DpiStagesReading) -> anyhow::Result<bool> {
    if local.enabled() != (observed.number_of_active_stages > 1) {
        return Ok(true);
    }
    if local.enabled() && usize::from(observed.active_stage) != local.active_stage() {
        return Ok(true);
    }
    let pair_equal = |index: usize, local_index: usize| -> anyhow::Result<bool> {
        let actual = observed
            .records
            .get(index)
            .context("BLE OBM stage axes are unobserved")?;
        let expected = local
            .stages()
            .get(local_index)
            .context("BLE local DPI stage is absent")?;
        ensure!(!expected.is_null(), "BLE local DPI stage is null");
        // loadProfilesFromDevice_DPI creates only x/y. It's local independent
        // comparison initializes false, without claiming a device flag exists.
        Ok(expected
            .x()
            .is_some_and(|value| value.strict_equals_u16(actual.x))
            && expected
                .y()
                .is_some_and(|value| value.strict_equals_u16(actual.y))
            && expected.is_independent() == Some(false))
    };
    if local.enabled() {
        for index in 0..usize::from(observed.number_of_active_stages) {
            if !pair_equal(index, index)? {
                return Ok(true);
            }
        }
        Ok(false)
    } else {
        let local_index = local
            .active_stage()
            .checked_sub(1)
            .context("BLE local active DPI stage is absent")?;
        // The source chooses r[0] before r[deviceActive-1]. Empty arrays keep
        // axes unobserved; no default row is synthesized to trigger a write.
        ensure!(
            !observed.records.is_empty(),
            "BLE OBM selected stage is unobserved"
        );
        Ok(!pair_equal(0, local_index)?)
    }
}

pub fn synchronize(
    transport: &mut dyn Transport,
    request: &Request,
    read: ReadReport,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    request.validate()?;
    let mut report = Report {
        request: request.clone(),
        read,
        profiles: vec![],
        projected_profiles: vec![],
        completed: false,
        error: None,
    };
    let operation = (|| -> anyhow::Result<()> {
        ensure!(
            report.read.error.is_none(),
            "BLE initialization observation failed: {}",
            report.read.error.as_deref().unwrap_or("")
        );
        let snapshot = report
            .read
            .snapshot
            .as_ref()
            .context("BLE initialization inventory is unobserved")?;
        snapshot.validate()?;
        report.projected_profiles = project_profiles(snapshot, request)?;
        for local in &request.profiles {
            validate()?;
            let selected_slots = selected_slots(snapshot, local, request);
            let mut progress = ProfileSynchronization {
                guid: local.guid.clone(),
                selected_slots,
                differing_slots: vec![],
                writes: vec![],
                error: None,
            };
            let profile_operation = (|| -> anyhow::Result<()> {
                if let Some(stages) = &local.dpi_stages {
                    for &slot in &progress.selected_slots {
                        let observed = snapshot
                            .profiles
                            .iter()
                            .find(|profile| profile.slot_id == slot)
                            .expect("selected actual inventory slot");
                        let observed = observed.stages.value().with_context(|| {
                            format!("BLE OBM stages are unobserved for slot {slot}")
                        })?;
                        if differs(stages, observed)? {
                            progress.differing_slots.push(slot);
                        }
                    }
                    let packed = pack_for(226, stages)?;
                    let writes = synchronize_slot_ids(
                        transport,
                        &progress.differing_slots,
                        &packed,
                        validate,
                    );
                    progress.writes = writes.writes;
                    if let Some(error) = writes.interrupted {
                        anyhow::bail!(error);
                    }
                    let errors: Vec<_> = progress
                        .writes
                        .iter()
                        .filter_map(|write| write.error.as_deref())
                        .collect();
                    ensure!(
                        errors.is_empty(),
                        "BLE OBM DPI initialization failed: {}",
                        errors.join("; ")
                    );
                }
                Ok(())
            })();
            if let Err(error) = profile_operation {
                progress.error = Some(format!("{error:#}"));
                report.profiles.push(progress);
                return Err(error);
            }
            report.profiles.push(progress);
        }
        validate()?;
        report.completed = true;
        Ok(())
    })();
    if let Err(error) = operation {
        report.error = Some(format!("{error:#}"));
    }
    report.validate()?;
    Ok(report)
}

pub fn execute(
    product_id: u32,
    physical_product_id: u32,
    transport: &mut dyn Transport,
    request: &Request,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    ensure!(
        super::accepts(product_id, physical_product_id),
        "BLE initialization product identity differs"
    );
    request.validate()?;
    validate()?;
    let read = read_profiles(transport, validate);
    synchronize(transport, request, read, validate)
}

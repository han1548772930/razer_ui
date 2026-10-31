//! Page-independent controller import ownership and source-document publication.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::controller_initialization::{
    ConnectionFeaturesResult, InitializationPlan, InitializationResult, ProfileDirectory, Read,
    RootField,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct ControllerInitializationRequested {
    id: uuid::Uuid,
    generation: u64,
    key: String,
    plan: InitializationPlan,
    valid: Arc<AtomicBool>,
}
impl ControllerInitializationRequested {
    pub fn plan(&self) -> &InitializationPlan {
        &self.plan
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.valid)
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
}

#[derive(Default)]
pub(super) struct Owner {
    request: Option<ControllerInitializationRequested>,
    baseline: Option<Baseline>,
    result: Option<InitializationResult>,
    error: Option<String>,
    published: bool,
    // Module 11365 sets its guard before reading and retains it after failure.
    // Canceling one connection does not recreate the source module lifetime.
    importer_started: bool,
    // Module 10511's engine has a separate lifetime. A reset of the importer
    // must not imply that its directory was re-read from a new connection.
    engine_directory: Option<ProfileDirectory>,
    mapping_projections:
        std::collections::BTreeMap<u8, razer_device::controller_mapping::MappingProjection>,
    resumed_generation: Option<u64>,
    import_saved: bool,
    connection_generation: Option<u64>,
    connection_result: Option<ConnectionFeaturesResult>,
}
struct Baseline {
    profiles: Vec<Profile>,
    active_profile: String,
    root: Option<Value>,
    power: Option<razer_device::controller_sleep::PowerSaving>,
}
impl Baseline {
    fn new(device: &Device) -> Self {
        Self {
            profiles: device.profiles.clone(),
            active_profile: device.active_profile.clone(),
            root: device.source_device_settings.clone(),
            power: device.controller_power_saving,
        }
    }
    fn matches(&self, device: &Device) -> bool {
        self.profiles == device.profiles
            && self.active_profile == device.active_profile
            && self.root == device.source_device_settings
            && self.power == device.controller_power_saving
    }
}
impl Owner {
    fn begin_connection(&mut self, generation: u64, slots: Vec<u8>) -> Option<InitializationPlan> {
        if !self.importer_started
            || !self.import_saved
            || self.connection_generation == Some(generation)
        {
            return None;
        }
        self.connection_generation = Some(generation);
        Some(InitializationPlan::connection_features(
            self.engine_directory.clone(),
            slots,
        ))
    }
    fn begin_import(&mut self) -> bool {
        if self.importer_started {
            false
        } else {
            self.importer_started = true;
            true
        }
    }
    fn invalidate(&mut self) {
        if let Some(request) = self.request.take() {
            request.valid.store(false, Ordering::Release);
        }
        self.baseline = None;
        self.result = None;
        self.mapping_projections.clear();
        self.connection_result = None;
        self.error = None;
        self.published = false;
    }
    pub(super) fn resume_after_canceled_exit(&mut self) {
        // Exit canceled the readers but did not create another physical
        // connection. Reopen only their admission gates for a real read;
        // cached settings and old results never authorize a restoration SET.
        self.invalidate();
        self.connection_generation = None;
        self.resumed_generation = None;
    }
    fn matches(&self, request: &ControllerInitializationRequested) -> bool {
        request.is_valid()
            && self.request.as_ref().is_some_and(|current| {
                current.id == request.id
                    && current.generation == request.generation
                    && current.key == request.key
                    && Arc::ptr_eq(&current.valid, &request.valid)
            })
    }
}

fn local_profile_slots(profiles: &[Profile]) -> anyhow::Result<Vec<u8>> {
    let mut slots = Vec::new();
    for profile in profiles {
        let Some(saved) = profile
            .source_settings
            .as_ref()
            .and_then(|saved| saved.pointer("/profile/obmSlotId"))
        else {
            continue;
        };
        let ids: Vec<u8> = serde_json::from_value(saved.clone())?;
        if let Some(slot) = ids.first() {
            let wire = razer_device::controller_profile::wire_slot(*slot);
            if !slots.contains(&wire) {
                slots.push(wire);
            }
        }
    }
    Ok(slots)
}

fn project_connection_features(
    previous: &[Profile],
    observed: &ConnectionFeaturesResult,
) -> anyhow::Result<Vec<Profile>> {
    ensure!(!observed.canceled, "Controller generic phase was canceled");
    let mut profiles = previous.to_vec();
    for row in &observed.socd {
        let Some(mode) = row.result.value().and_then(|observed| observed.mode()) else {
            continue;
        };
        for profile in &mut profiles {
            let Some(settings) = profile.source_settings.as_mut() else {
                continue;
            };
            let Some(slots) = settings.pointer("/profile/obmSlotId") else {
                continue;
            };
            let slots: Vec<u8> = serde_json::from_value(slots.clone())?;
            if slots
                .first()
                .copied()
                .map(razer_device::controller_profile::wire_slot)
                == Some(row.slot)
            {
                super::super::gamepad_products::project_controller_socd(settings, mode)?;
                // The source uses profiles.find(Fk(profile) == wireId).
                break;
            }
        }
    }
    Ok(profiles)
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.invalidate();
    }
}

fn plan(product_id: u32, root: Option<&Value>) -> anyhow::Result<InitializationPlan> {
    // Module 31759.eL rebuilds each real slot from getDefaultProfile before
    // module 11365.X enumerates its fields. Old profile key order is irrelevant.
    let result = InitializationPlan {
        roots: root_fields(product_id, root)?,
        profiles: Vec::new(),
        new_profile_fields: InitializationPlan::new_profile_fields(product_id),
        connection_features: None,
    };
    result.validate(product_id)?;
    Ok(result)
}

struct ImportedProfiles {
    profiles: Vec<Profile>,
    active: String,
    mappings: std::collections::BTreeMap<u8, razer_device::controller_mapping::MappingProjection>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AudioHapticIntensity {
    is_enabled: bool,
    value: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    enable_audio_to_haptics: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioHapticPolicy {
    enable_audio_to_haptics: Option<bool>,
}

#[derive(serde::Serialize)]
struct ControllerModeSetting {
    mode: razer_device::controller_initialization::ControllerMode,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct MiddlewareRoot {
    version: u8,
    vendor_id: u16,
    product_id: u32,
    category: &'static str,
    #[serde(rename = "reportIDs")]
    report_ids: std::collections::BTreeMap<u8, &'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio_haptic_intensity: Option<AudioHapticIntensity>,
    power_saving: razer_device::controller_sleep::PowerSaving,
    #[serde(skip_serializing_if = "Option::is_none")]
    controller_mode: Option<ControllerModeSetting>,
}

fn new_source_root(product_id: u32) -> anyhow::Result<Value> {
    ensure!(
        matches!(product_id, 2636 | 2676),
        "No verified controller middleware root for {product_id}"
    );
    // Module 69427 constructs these local records before any getters when
    // the whole source document is absent. Observations remain on the owner.
    Ok(serde_json::to_value(MiddlewareRoot {
        version: 1,
        vendor_id: 5426,
        product_id,
        category: "GAMEPAD",
        report_ids: [(4, "razerKeyReportID"), (5, "hardwareEventReportID")].into(),
        audio_haptic_intensity: (product_id == 2636).then_some(AudioHapticIntensity {
            is_enabled: true,
            value: 4,
            enable_audio_to_haptics: Some(true),
        }),
        power_saving: razer_device::controller_sleep::PowerSaving::new(5, true)?,
        controller_mode: (product_id == 2676).then_some(ControllerModeSetting {
            mode: razer_device::controller_initialization::ControllerMode::Xinput,
        }),
    })?)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeviceProfileMetadata {
    active_profile_guid: Option<String>,
}

fn loaded_source_profile_guid(device: &Device) -> anyhow::Result<Option<String>> {
    let Some(root) = device.source_device_settings.as_ref() else {
        return Ok(None);
    };
    ensure!(root.is_object(), "Controller local root is not an object");
    if device.serial_number.is_empty() {
        return Ok(None);
    }
    if let Some(metadata) = root.get("deviceMetadatas").filter(|value| !value.is_null()) {
        let metadata = metadata
            .as_object()
            .context("Controller serial metadata is not an object")?;
        if let Some(current) = metadata
            .get(&device.serial_number)
            .filter(|value| !value.is_null())
        {
            let current: DeviceProfileMetadata = serde_json::from_value(current.clone())?;
            return Ok(current.active_profile_guid);
        }
    }
    // The injected storage handler mirrors the previously selected local GUID
    // into root.activeProfile at load. Module 52448 subsequently changes only
    // profileData[serial].activeProfile during hardware import; 47442 still
    // consumes the captured mirror, not the newly observed directory choice.
    let mut profiles = device
        .profiles
        .iter()
        .filter(|profile| profile.id == device.active_profile);
    let selected = profiles.next();
    ensure!(
        profiles.next().is_none(),
        "Ambiguous local controller selection"
    );
    Ok(selected
        .filter(|profile| !profile.guid.is_empty())
        .map(|profile| profile.guid.clone()))
}

fn import_root_settings(
    device: &Device,
    imported: &ImportedProfiles,
    result: &InitializationResult,
) -> anyhow::Result<Value> {
    let loaded_guid = loaded_source_profile_guid(device)?;
    let mut root = match &device.source_device_settings {
        Some(root) => root.clone(),
        None => new_source_root(device.product_id)?,
    };
    let root_fields = root
        .as_object_mut()
        .context("Controller local root is not an object")?;
    if let Some(power) = result.power_saving.value() {
        root_fields.insert(
            "powerSaving".into(),
            serde_json::to_value(power.power_saving())?,
        );
    }
    if let Some(mode) = result.controller_mode.value() {
        root_fields.insert(
            "controllerMode".into(),
            serde_json::to_value(ControllerModeSetting { mode: *mode })?,
        );
    }
    for (profile, observed) in imported.profiles.iter().zip(&result.profiles) {
        if loaded_guid.as_ref() != Some(&profile.guid)
            || !observed.fields.iter().any(|read| {
                matches!(
                    read.result,
                    Read::Observed {
                        value: razer_device::controller_initialization::ProfileValue::Haptic { .. }
                    }
                )
            })
        {
            continue;
        }
        let mut haptic: AudioHapticIntensity = serde_json::from_value(
            profile
                .source_settings
                .as_ref()
                .and_then(|settings| settings.pointer("/profile/hapticIntensity"))
                .context("Imported controller profile omits its observed haptic intensity")?
                .clone(),
        )?;
        let policy = root_fields
            .get("audioHapticIntensity")
            .filter(|value| !value.is_null())
            .map(|value| serde_json::from_value::<AudioHapticPolicy>(value.clone()))
            .transpose()?;
        let enabled = policy.and_then(|policy| policy.enable_audio_to_haptics);
        haptic.enable_audio_to_haptics = if device.product_id == 2636 {
            Some(enabled.unwrap_or(false))
        } else {
            enabled
        };
        root_fields.insert("audioHapticIntensity".into(), serde_json::to_value(haptic)?);
    }
    Ok(root)
}

struct SlotProfileIdentity<'a> {
    owner_id: &'a str,
    slot: u8,
}
impl SlotProfileIdentity<'_> {
    fn into_local_id(self) -> String {
        format!(
            "controller-slot:{}:{}:{}",
            self.owner_id.len(),
            self.owner_id,
            self.slot
        )
    }
}

fn import_profiles(
    product_id: u32,
    previous: &[Profile],
    result: &InitializationResult,
) -> anyhow::Result<ImportedProfiles> {
    let directory = result
        .directory
        .value()
        .context("Controller import has no real profile directory")?;
    ensure!(
        !result.canceled && !directory.profile_ids.is_empty(),
        "Controller import is canceled or its real directory is empty"
    );
    ensure!(
        result
            .profiles
            .iter()
            .map(|profile| profile.slot)
            .eq(directory.profile_ids.iter().copied()),
        "Controller publication omits or reorders a real slot"
    );
    let previous_slots = previous
        .iter()
        .map(|profile| {
            profile
                .source_settings
                .as_ref()
                .and_then(|settings| settings.pointer("/profile/obmSlotId"))
                .map(|slots| serde_json::from_value::<Vec<u8>>(slots.clone()))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut profiles: Vec<Profile> = Vec::with_capacity(directory.profile_ids.len());
    let mut mappings = std::collections::BTreeMap::new();
    let mut active = None;
    for observed in &result.profiles {
        let name = directory
            .names
            .iter()
            .find(|name| name.slot == observed.slot)
            .and_then(|name| name.name.value())
            .context("Controller real slot has no observed name")?;
        let matches = previous_slots
            .iter()
            .enumerate()
            .filter_map(|(index, slots)| {
                slots
                    .as_ref()
                    .is_some_and(|slots| slots.contains(&observed.slot))
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        ensure!(
            matches.len() <= 1,
            "Controller slot has ambiguous local owners"
        );
        let mut profile = if let Some(index) = matches.first() {
            let previous_profile = &previous[*index];
            let mut profile = previous_profile.clone();
            let primary_slot = previous_slots[*index]
                .as_ref()
                .and_then(|slots| {
                    slots
                        .iter()
                        .find(|slot| directory.profile_ids.contains(slot))
                })
                .context("Matched controller profile has no real slot")?;
            if *primary_slot != observed.slot {
                profile.id = SlotProfileIdentity {
                    owner_id: &previous_profile.id,
                    slot: observed.slot,
                }
                .into_local_id();
            }
            profile
        } else {
            let guid = uuid::Uuid::new_v4().to_string();
            Profile {
                source_settings: None,
                settings: None,
                name: name.clone(),
                guid: guid.clone(),
                id: guid,
                dpi_stages: None,
            }
        };
        ensure!(
            !profiles.iter().any(|published| published.id == profile.id),
            "Controller directory has duplicate internal profile identities"
        );
        let imported = super::super::gamepad_products::import_controller_profile(
            product_id,
            profile.source_settings.as_ref(),
            observed,
            result.connection_probe.value(),
        )?;
        if let Some(projection) = imported.mappings {
            mappings.insert(observed.slot, projection);
        }
        let mut settings = imported.settings;
        settings["profile"]["guid"] = serde_json::json!(profile.guid);
        settings["profile"]["name"] = serde_json::json!(name);
        settings["profile"]["obmSlotId"] = serde_json::json!([observed.slot]);
        profile.name = name.clone();
        profile.source_settings = Some(settings);
        if observed.slot == directory.active_profile {
            active = Some(profile.id.clone());
        }
        profiles.push(profile);
    }
    Ok(ImportedProfiles {
        profiles,
        active: active.context("Controller active slot was not imported")?,
        mappings,
    })
}
fn root_fields(product_id: u32, root: Option<&Value>) -> anyhow::Result<Vec<RootField>> {
    Ok(if let Some(root) = root {
        root.as_object()
            .context("Controller source root is not an object")?
            .keys()
            .filter_map(|key| match key.as_str() {
                "powerSaving" => Some(RootField::PowerSaving),
                "controllerMode" if product_id == 2676 => Some(RootField::ControllerMode),
                _ => None,
            })
            .collect()
    } else if product_id == 2676 {
        vec![RootField::PowerSaving, RootField::ControllerMode]
    } else {
        vec![RootField::PowerSaving]
    })
}

fn profile_socd_observation<'a>(
    profiles: &[Profile],
    active: &str,
    connection: &'a ConnectionFeaturesResult,
) -> Option<&'a Read<razer_device::controller_initialization::SocdObservation>> {
    for row in &connection.socd {
        let owner = profiles.iter().find(|profile| {
            profile
                .source_settings
                .as_ref()
                .and_then(|saved| saved.pointer("/profile/obmSlotId/0"))
                .and_then(Value::as_u64)
                .and_then(|slot| u8::try_from(slot).ok())
                .map(razer_device::controller_profile::wire_slot)
                == Some(row.slot)
        });
        if owner.is_some_and(|owner| owner.id == active) {
            return Some(&row.result);
        }
    }
    None
}
impl SourceProductWorkspace {
    pub(super) fn controller_socd_for_active(
        &self,
    ) -> Option<&Read<razer_device::controller_initialization::SocdObservation>> {
        profile_socd_observation(
            &self.device.profiles,
            &self.device.active_profile,
            self.controller_initialization.connection_result.as_ref()?,
        )
    }
    pub(super) fn invalidate_controller_initialization(&mut self, cx: &mut Context<Self>) {
        self.invalidate_controller_socd(cx);
        self.controller_initialization.invalidate();
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| {
                body.set_controller_initialization_pending(cx)
            });
        }
    }
    pub fn begin_controller_initialization(
        &mut self,
        key: String,
        recording: bool,
        cx: &mut Context<Self>,
    ) -> Option<ControllerInitializationRequested> {
        if self.exiting
            || recording
            || !matches!(self.device.product_id, 2636 | 2676)
            || self.device.dashboard.connection_observation.is_none()
            || !matches!(self.body, FamilyBody::Gamepad(_))
        {
            return None;
        }
        if self
            .controller_initialization
            .request
            .as_ref()
            .is_some_and(|request| {
                request.key == key
                    && request.generation == self.mapping_connection_generation
                    && !(self.device.product_id == 2676
                        && request.plan.connection_features.is_none()
                        && self.controller_initialization.import_saved)
            })
        {
            return None;
        }
        if self.controller_initialization.importer_started {
            if self.device.product_id == 2676 {
                let slots = match local_profile_slots(&self.device.profiles) {
                    Ok(slots) => slots,
                    Err(error) => {
                        self.controller_initialization.error = Some(format!("{error:#}"));
                        return None;
                    }
                };
                let plan = self
                    .controller_initialization
                    .begin_connection(self.mapping_connection_generation, slots)?;
                if let Some(previous) = self.controller_initialization.request.take() {
                    previous.valid.store(false, Ordering::Release);
                }
                let request = ControllerInitializationRequested {
                    id: uuid::Uuid::new_v4(),
                    generation: self.mapping_connection_generation,
                    key,
                    plan,
                    valid: Arc::new(AtomicBool::new(true)),
                };
                self.controller_initialization.baseline = Some(Baseline::new(&self.device));
                self.controller_initialization.published = false;
                self.controller_initialization.request = Some(request.clone());
                return Some(request);
            }
            if self.controller_initialization.resumed_generation
                != Some(self.mapping_connection_generation)
            {
                self.controller_initialization.resumed_generation =
                    Some(self.mapping_connection_generation);
                if let FamilyBody::Gamepad(body) = &self.body {
                    body.update(cx, |body, cx| {
                        body.resume_controller_runtime_without_import(cx)
                    });
                }
            }
            return None;
        }
        self.invalidate_controller_initialization(cx);
        self.controller_initialization.begin_import();
        let plan = match plan(
            self.device.product_id,
            self.device.source_device_settings.as_ref(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                self.controller_initialization.error = Some(format!("{error:#}"));
                return None;
            }
        };
        let request = ControllerInitializationRequested {
            id: uuid::Uuid::new_v4(),
            generation: self.mapping_connection_generation,
            key,
            plan,
            valid: Arc::new(AtomicBool::new(true)),
        };
        self.controller_initialization.baseline = Some(Baseline::new(&self.device));
        self.controller_initialization.request = Some(request.clone());
        Some(request)
    }
    pub fn controller_initialization_matches(
        &self,
        request: &ControllerInitializationRequested,
    ) -> bool {
        !self.exiting && self.controller_initialization.matches(request)
    }
    pub fn finish_controller_initialization(
        &mut self,
        request: &ControllerInitializationRequested,
        result: Result<InitializationResult, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.controller_initialization_matches(request) {
            return false;
        }
        let result = result.and_then(|result| {
            result
                .validate_for(self.device.product_id, &request.plan)
                .map(|()| result)
                .map_err(|error| format!("{error:#}"))
        });
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                self.controller_initialization.error = Some(error);
                cx.notify();
                return false;
            }
        };
        if let Some(features) = &result.connection_features {
            if let Some(directory) = features.refreshed_directory.value() {
                self.controller_initialization.engine_directory = Some(directory.clone());
            }
            self.controller_initialization.connection_result = Some(features.clone());
            if result.canceled
                || !self
                    .controller_initialization
                    .baseline
                    .as_ref()
                    .is_some_and(|baseline| baseline.matches(&self.device))
            {
                self.controller_initialization.error = Some(
                    "Controller generic phase was canceled or its local document changed".into(),
                );
                cx.notify();
                return false;
            }
            match project_connection_features(&self.device.profiles, features) {
                Ok(profiles) => {
                    self.device.profiles = profiles;
                    self.controller_initialization.published = true;
                    self.restore_active(window, cx);
                    cx.notify();
                    return true;
                }
                Err(error) => {
                    self.controller_initialization.error = Some(format!("{error:#}"));
                    cx.notify();
                    return false;
                }
            }
        }
        if let Some(directory) = result.directory.value() {
            self.controller_initialization.engine_directory = Some(directory.clone());
        }
        self.controller_initialization.result = Some(result.clone());
        if result.canceled
            || !self
                .controller_initialization
                .baseline
                .as_ref()
                .is_some_and(|baseline| baseline.matches(&self.device))
        {
            self.controller_initialization.error = Some(
                "Controller import was canceled or local edits changed its captured document"
                    .into(),
            );
            cx.notify();
            return false;
        }
        let published = self.publish_controller_initialization(&result);
        match published {
            Ok(()) => {
                self.controller_initialization.published = true;
                self.restore_active(window, cx);
                cx.notify();
                true
            }
            Err(error) => {
                self.controller_initialization.error = Some(format!("{error:#}"));
                cx.notify();
                false
            }
        }
    }
    fn publish_controller_initialization(
        &mut self,
        result: &InitializationResult,
    ) -> anyhow::Result<()> {
        let imported = import_profiles(self.device.product_id, &self.device.profiles, result)?;
        let root = import_root_settings(&self.device, &imported, result)?;
        self.device.profiles = imported.profiles;
        self.device.active_profile = imported.active;
        self.device.source_device_settings = Some(root);
        self.controller_initialization.mapping_projections = imported.mappings;
        if let Some(power) = result.power_saving.value() {
            self.device.controller_power_saving = Some(power.power_saving());
        }
        Ok(())
    }
    pub fn complete_controller_initialization_storage(
        &mut self,
        request: &ControllerInitializationRequested,
        succeeded: bool,
        recording: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.controller_initialization_matches(request)
            || !self.controller_initialization.published
        {
            return;
        }
        if !succeeded {
            self.controller_initialization.error = Some(
                "Controller imported document could not be saved; cache restoration was not sent"
                    .into(),
            );
            cx.notify();
            return;
        }
        if request.plan.connection_features.is_none() {
            self.controller_initialization.import_saved = true;
            if self.device.product_id == 2676 {
                cx.notify();
                return;
            }
        }
        let result = self.controller_initialization.result.as_ref();
        if request.plan.connection_features.is_some() {
            self.controller_socd.ready = true;
        }
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| {
                let active = result.and_then(|result| {
                    result.directory.value().and_then(|directory| {
                        result
                            .profiles
                            .iter()
                            .find(|profile| profile.slot == directory.active_profile)
                    })
                });
                body.observe_controller_initialization_profile(active);
                body.observe_controller_socd(self.controller_socd_for_active(), cx);
                body.restore_controller_initialization_power(
                    result
                        .map(|result| &result.power_saving)
                        .unwrap_or(&Read::Unobserved),
                    recording,
                    cx,
                )
            });
        }
        self.sync_controller_socd(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::controller_initialization::{
        ProfileField, ProfileName, ProfileObservation, Read,
    };
    use std::prelude::v1::test;
    fn saved_profile(id: &str, slots: &[u8]) -> Profile {
        let observed = ProfileObservation {
            slot: slots[0],
            fields: Vec::new(),
        };
        let mut settings = super::super::super::gamepad_products::import_controller_profile(
            2676, None, &observed, None,
        )
        .unwrap()
        .settings;
        settings["profile"]["obmSlotId"] = serde_json::json!(slots);
        settings["profile"]["futureExtension"] = serde_json::json!({"retained":id});
        Profile {
            source_settings: Some(settings),
            settings: None,
            name: format!("old-{id}"),
            guid: format!("guid-{id}"),
            id: id.into(),
            dpi_stages: None,
        }
    }
    fn directory_result(slots: &[u8], active: u8) -> InitializationResult {
        InitializationResult {
            connection_probe: Read::Unobserved,
            power_saving: Read::Unobserved,
            controller_mode: Read::Unobserved,
            directory: Read::Observed {
                value: ProfileDirectory {
                    button_ids: vec![16],
                    profile_ids: slots.to_vec(),
                    active_profile: active,
                    number_of_profiles: slots.len() as u8,
                    names: slots
                        .iter()
                        .map(|slot| ProfileName {
                            slot: *slot,
                            name: Read::Observed {
                                value: format!("real-slot-{slot}"),
                            },
                        })
                        .collect(),
                },
            },
            profiles: slots
                .iter()
                .map(|slot| ProfileObservation {
                    slot: *slot,
                    fields: Vec::new(),
                })
                .collect(),
            canceled: false,
            connection_features: None,
        }
    }

    fn haptic_import_device(product: u32) -> Device {
        let mut device = razer_model::product_preview::registered_preview(product).unwrap();
        device.serial_number = "current-serial".into();
        device.profiles = vec![saved_profile("three", &[3]), saved_profile("one", &[1])];
        device.active_profile = "three".into();
        device.source_device_settings = Some(serde_json::json!({
            "audioHapticIntensity":{"isEnabled":true,"value":6},
            "futureExtension":{"retained":true}
        }));
        device
    }

    fn haptic_directory_result() -> InitializationResult {
        use razer_device::controller_initialization::{ProfileRead, ProfileValue};
        let mut result = directory_result(&[3, 1], 1);
        for (profile, scaling) in result.profiles.iter_mut().zip([0, 83]) {
            profile.fields.push(ProfileRead {
                field: ProfileField::HapticIntensity,
                result: Read::Observed {
                    value: ProfileValue::Haptic { scaling },
                },
            });
        }
        result
    }

    #[test]
    fn controller_initialization_root_haptics_keep_the_loaded_local_guid_and_product_audio_policy()
    {
        // Injected storage ge mirrors the local GUID at load; 52448's later
        // directory selection does not replace that 47442 consumer identity.
        for product in [2636, 2676] {
            for policy in [None, Some(false), Some(true)] {
                let mut device = haptic_import_device(product);
                if let Some(policy) = policy {
                    device.source_device_settings.as_mut().unwrap()["audioHapticIntensity"]["enableAudioToHaptics"] =
                        serde_json::json!(policy);
                }
                let result = haptic_directory_result();
                let imported = import_profiles(product, &device.profiles, &result).unwrap();
                assert_eq!(imported.active, "one");
                let root = import_root_settings(&device, &imported, &result).unwrap();
                assert_eq!(root["audioHapticIntensity"]["isEnabled"], false);
                assert_eq!(root["audioHapticIntensity"]["value"], 4);
                let expected_policy = if product == 2636 {
                    Some(policy.unwrap_or(false))
                } else {
                    policy
                };
                assert_eq!(
                    root["audioHapticIntensity"]
                        .get("enableAudioToHaptics")
                        .and_then(Value::as_bool),
                    expected_policy
                );
                assert_eq!(
                    root["futureExtension"],
                    serde_json::json!({"retained":true})
                );
                assert_eq!(device.active_profile, "three");
            }
        }
    }

    #[test]
    fn controller_initialization_root_haptic_identity_matches_the_injected_serial_metadata_rules() {
        for product in [2636, 2676] {
            let result = haptic_directory_result();
            let mut device = haptic_import_device(product);
            let imported = import_profiles(product, &device.profiles, &result).unwrap();
            device.source_device_settings.as_mut().unwrap()["deviceMetadatas"] = serde_json::json!({
                "other-serial":{"activeProfileGuid":"guid-one"}
            });
            assert_eq!(
                loaded_source_profile_guid(&device).unwrap().as_deref(),
                Some("guid-three")
            );
            device.source_device_settings.as_mut().unwrap()["deviceMetadatas"]["current-serial"] =
                serde_json::json!({"activeProfileGuid":"guid-one"});
            assert_eq!(
                loaded_source_profile_guid(&device).unwrap().as_deref(),
                Some("guid-one")
            );
            let root = import_root_settings(&device, &imported, &result).unwrap();
            assert_eq!(root["audioHapticIntensity"]["isEnabled"], true);
            assert_eq!(root["audioHapticIntensity"]["value"], 5);
            device.source_device_settings.as_mut().unwrap()["deviceMetadatas"]["current-serial"] =
                serde_json::json!({});
            assert!(loaded_source_profile_guid(&device).unwrap().is_none());
            let root = import_root_settings(&device, &imported, &result).unwrap();
            assert_eq!(root["audioHapticIntensity"]["value"], 6);
            device.source_device_settings.as_mut().unwrap()["deviceMetadatas"]["current-serial"] =
                serde_json::json!({"activeProfileGuid":123});
            assert!(import_root_settings(&device, &imported, &result).is_err());
            device.serial_number.clear();
            assert!(loaded_source_profile_guid(&device).unwrap().is_none());
        }
    }

    #[test]
    fn controller_initialization_failed_or_unobserved_haptics_do_not_promote_the_other_profile_or_local_defaults()
     {
        for product in [2636, 2676] {
            for state in [
                Read::Unobserved,
                Read::Failed {
                    error: "actual haptic getter failed".into(),
                },
            ] {
                let device = haptic_import_device(product);
                let mut result = haptic_directory_result();
                result.profiles[0].fields[0].result = state.clone();
                let imported = import_profiles(product, &device.profiles, &result).unwrap();
                let root = import_root_settings(&device, &imported, &result).unwrap();
                assert_eq!(root, *device.source_device_settings.as_ref().unwrap());
                assert_eq!(result.profiles[0].fields[0].result, state);
            }
            let mut device = haptic_import_device(product);
            let result = haptic_directory_result();
            let imported = import_profiles(product, &device.profiles, &result).unwrap();
            device.active_profile = "missing-local-profile".into();
            assert!(loaded_source_profile_guid(&device).unwrap().is_none());
            assert_eq!(
                import_root_settings(&device, &imported, &result).unwrap(),
                *device.source_device_settings.as_ref().unwrap()
            );
            device.source_device_settings = None;
            let root = import_root_settings(&device, &imported, &result).unwrap();
            assert_eq!(
                root["powerSaving"],
                serde_json::json!({"isEnabled":true,"value":5})
            );
            assert!(matches!(result.power_saving, Read::Unobserved));
            if product == 2636 {
                assert_eq!(
                    root["audioHapticIntensity"],
                    serde_json::json!({"isEnabled":true,"value":4,"enableAudioToHaptics":true})
                );
                assert!(root.get("controllerMode").is_none());
            } else {
                assert!(root.get("audioHapticIntensity").is_none());
                assert_eq!(root["controllerMode"], serde_json::json!({"mode":"Xinput"}));
                assert!(matches!(result.controller_mode, Read::Unobserved));
            }
        }
    }
    #[test]
    fn controller_initialization_connection_phase_waits_for_import_save_then_repeats_only_on_a_new_connection()
     {
        let mut owner = Owner::default();
        assert!(owner.begin_connection(1, vec![3, 1]).is_none());
        owner.begin_import();
        assert!(owner.begin_connection(1, vec![3, 1]).is_none());
        owner.engine_directory = directory_result(&[3, 1], 1).directory.value().cloned();
        owner.import_saved = true;
        let first = owner.begin_connection(1, vec![3, 1]).unwrap();
        assert_eq!(
            first
                .connection_features
                .unwrap()
                .retained_engine_directory
                .as_ref()
                .unwrap()
                .profile_ids,
            [3, 1]
        );
        assert!(owner.begin_connection(1, vec![3, 1]).is_none());
        owner.invalidate();
        assert!(owner.begin_connection(1, vec![3, 1]).is_none());
        let next = owner.begin_connection(2, vec![1, 3]).unwrap();
        assert!(owner.result.is_none());
        assert!(owner.connection_result.is_none());
        assert_eq!(
            next.connection_features.unwrap().local_profile_slots,
            [1, 3]
        );
        assert!(!owner.begin_import());
    }
    #[test]
    fn controller_initialization_socd_projection_updates_known_source_and_ui_fields_without_defaulting_failures()
     {
        use razer_device::controller_initialization::{SocdObservation, SocdRead};
        let mut previous = vec![
            saved_profile("three", &[3]),
            saved_profile("one", &[1]),
            saved_profile("four", &[4]),
        ];
        for profile in &mut previous {
            profile.source_settings.as_mut().unwrap()["profile"]["controllerDPadSOCD"] =
                serde_json::json!("last");
        }
        let observed = ConnectionFeaturesResult {
            refreshed_directory: Read::Unobserved,
            socd: vec![
                SocdRead {
                    slot: 3,
                    result: Read::Observed {
                        value: SocdObservation { raw_option: 4 },
                    },
                },
                SocdRead {
                    slot: 1,
                    result: Read::Failed {
                        error: "actual SOCD read failed".into(),
                    },
                },
                SocdRead {
                    slot: 4,
                    result: Read::Observed {
                        value: SocdObservation { raw_option: 255 },
                    },
                },
            ],
            canceled: false,
        };
        let projected = project_connection_features(&previous, &observed).unwrap();
        assert_eq!(
            projected[0].source_settings.as_ref().unwrap()["profile"]["controllerDPadSOCD"],
            "absoluteUp"
        );
        assert_eq!(
            projected[0].source_settings.as_ref().unwrap()["controller"]["dpad"]["socdSettings"],
            serde_json::json!({"value":"absoluteUp"})
        );
        assert_eq!(projected[1], previous[1]);
        assert_eq!(projected[2], previous[2]);
        assert!(
            matches!(&observed.socd[1].result,Read::Failed{error} if error=="actual SOCD read failed")
        );
        assert_eq!(observed.socd[2].result.value().unwrap().mode(), None);
        let mut canceled = observed;
        canceled.canceled = true;
        assert!(project_connection_features(&previous, &canceled).is_err());
    }
    #[test]
    fn controller_initialization_canceled_exit_reopens_same_connection_for_real_generic_reads_without_restarting_importer()
     {
        let mut owner = Owner::default();
        assert!(owner.begin_import());
        owner.import_saved = true;
        owner.engine_directory = directory_result(&[3, 1], 1).directory.value().cloned();
        let plan = owner.begin_connection(7, vec![3, 1]).unwrap();
        let request = ControllerInitializationRequested {
            id: uuid::Uuid::new_v4(),
            generation: 7,
            key: "retained physical collection".into(),
            plan,
            valid: Arc::new(AtomicBool::new(true)),
        };
        owner.request = Some(request.clone());
        owner.resumed_generation = Some(7);
        owner.connection_result = Some(ConnectionFeaturesResult {
            refreshed_directory: Read::Unobserved,
            socd: Vec::new(),
            canceled: false,
        });
        owner.invalidate();
        assert!(!request.is_valid());
        assert!(owner.begin_connection(7, vec![3, 1]).is_none());
        owner.resume_after_canceled_exit();
        assert!(owner.result.is_none());
        assert!(owner.connection_result.is_none());
        assert!(owner.resumed_generation.is_none());
        assert!(!owner.matches(&request));
        assert!(!owner.begin_import(), "Module lifetime guard remains set");
        let fresh = owner.begin_connection(7, vec![3, 1]).unwrap();
        assert_eq!(
            fresh
                .connection_features
                .unwrap()
                .retained_engine_directory
                .unwrap()
                .profile_ids,
            [3, 1]
        );
        assert!(owner.begin_connection(7, vec![3, 1]).is_none());
        owner.import_saved = false;
        owner.resume_after_canceled_exit();
        assert!(
            owner.begin_connection(7, vec![3, 1]).is_none(),
            "Uncommitted imports never authorize generic reads or device writes"
        );
    }
    #[test]
    fn controller_initialization_socd_zero_source_slot_maps_only_the_first_local_owner_to_real_wire_one()
     {
        use razer_device::controller_initialization::{SocdObservation, SocdRead};
        let previous = vec![
            saved_profile("zero", &[0]),
            saved_profile("one", &[1]),
            saved_profile("three", &[3]),
        ];
        assert_eq!(local_profile_slots(&previous).unwrap(), [1, 3]);
        let observed = ConnectionFeaturesResult {
            refreshed_directory: Read::Unobserved,
            socd: vec![SocdRead {
                slot: 1,
                result: Read::Observed {
                    value: SocdObservation { raw_option: 2 },
                },
            }],
            canceled: false,
        };
        let projected = project_connection_features(&previous, &observed).unwrap();
        assert_eq!(
            projected[0].source_settings.as_ref().unwrap()["profile"]["obmSlotId"],
            serde_json::json!([0])
        );
        assert_eq!(
            projected[0].source_settings.as_ref().unwrap()["profile"]["controllerDPadSOCD"],
            "first"
        );
        assert_eq!(projected[1], previous[1]);
        assert!(profile_socd_observation(&projected, "zero", &observed).is_some());
        assert!(profile_socd_observation(&projected, "one", &observed).is_none());
        let mut missing = saved_profile("missing", &[2]);
        missing.source_settings.as_mut().unwrap()["profile"]["obmSlotId"] = serde_json::json!([]);
        assert!(local_profile_slots(&[missing]).unwrap().is_empty());
    }
    #[test]
    fn controller_initialization_publication_replaces_current_device_profiles_in_real_slot_order() {
        let previous = vec![
            saved_profile("absent", &[9]),
            saved_profile("one", &[1]),
            saved_profile("three", &[3]),
        ];
        let snapshot = previous.clone();
        let imported = import_profiles(2676, &previous, &directory_result(&[3, 4, 1], 1)).unwrap();
        assert_eq!(
            imported
                .profiles
                .iter()
                .map(|profile| profile.name.as_str())
                .collect::<Vec<_>>(),
            ["real-slot-3", "real-slot-4", "real-slot-1"]
        );
        assert_eq!(imported.profiles[0].id, "three");
        assert_eq!(imported.profiles[0].guid, "guid-three");
        assert_eq!(imported.profiles[2].id, "one");
        assert_eq!(imported.active, "one");
        assert!(uuid::Uuid::parse_str(&imported.profiles[1].guid).is_ok());
        assert_eq!(imported.profiles[1].id, imported.profiles[1].guid);
        assert!(
            !imported
                .profiles
                .iter()
                .any(|profile| profile.id == "absent")
        );
        for (profile, slot) in imported.profiles.iter().zip([3, 4, 1]) {
            let saved = profile.source_settings.as_ref().unwrap();
            assert_eq!(saved["profile"]["obmSlotId"], serde_json::json!([slot]));
            assert_eq!(saved["profile"]["guid"], profile.guid);
            assert_eq!(saved["profile"]["name"], profile.name);
        }
        assert_eq!(
            imported.profiles[0].source_settings.as_ref().unwrap()["profile"]["futureExtension"],
            serde_json::json!({"retained":"three"})
        );
        assert_eq!(previous, snapshot);
    }
    #[test]
    fn controller_initialization_publication_rejects_incomplete_canceled_and_ambiguous_identity_without_mutation()
     {
        let previous = vec![saved_profile("shared", &[3, 1])];
        let snapshot = previous.clone();
        let ambiguous = vec![saved_profile("first", &[3]), saved_profile("second", &[3])];
        assert!(import_profiles(2676, &ambiguous, &directory_result(&[3], 3)).is_err());
        let mut incomplete = directory_result(&[3, 1], 1);
        incomplete.profiles.truncate(1);
        assert!(import_profiles(2676, &previous, &incomplete).is_err());
        let mut canceled = directory_result(&[3], 3);
        canceled.canceled = true;
        assert!(import_profiles(2676, &previous, &canceled).is_err());
        let mut malformed = previous.clone();
        malformed[0].source_settings.as_mut().unwrap()["profile"]["obmSlotId"] =
            serde_json::json!(["invalid"]);
        assert!(import_profiles(2676, &malformed, &directory_result(&[3], 3)).is_err());
        assert_eq!(previous, snapshot);
    }
    #[test]
    fn controller_initialization_shared_source_guid_keeps_stable_slot_ids_across_directory_order() {
        let previous = vec![saved_profile("shared", &[3, 1])];
        let first = import_profiles(2676, &previous, &directory_result(&[1, 3], 1)).unwrap();
        let reordered = import_profiles(2676, &previous, &directory_result(&[3, 1], 1)).unwrap();
        assert_eq!(first.profiles[0].id, "controller-slot:6:shared:1");
        assert_eq!(first.profiles[1].id, "shared");
        assert_eq!(reordered.profiles[0].id, first.profiles[1].id);
        assert_eq!(reordered.profiles[1].id, first.profiles[0].id);
        assert_eq!(first.active, first.profiles[0].id);
        for profile in &first.profiles {
            assert_eq!(profile.guid, "guid-shared");
            assert_eq!(
                profile.source_settings.as_ref().unwrap()["profile"]["guid"],
                "guid-shared"
            );
        }
        let reimported =
            import_profiles(2676, &first.profiles, &directory_result(&[3, 1], 1)).unwrap();
        assert_eq!(reimported.profiles[0].id, "shared");
        assert_eq!(reimported.profiles[1].id, "controller-slot:6:shared:1");
        assert_eq!(reimported.active, first.active);
    }
    #[test]
    fn controller_initialization_plan_uses_reconstructed_profile_getter_order_for_every_real_slot()
    {
        for product in [2636, 2676] {
            let planned = plan(product, Some(&serde_json::json!({"version":1}))).unwrap();
            assert!(planned.profiles.is_empty());
            assert_eq!(
                planned.new_profile_fields,
                InitializationPlan::new_profile_fields(product)
            );
            assert!(planned.new_profile_fields.contains(&ProfileField::Mappings));
            assert!(planned.roots.is_empty());
        }
        assert_eq!(
            plan(2676, None).unwrap().roots,
            [RootField::PowerSaving, RootField::ControllerMode]
        );
    }
    #[test]
    fn controller_initialization_import_once_survives_failure_and_connection_cancellation() {
        use razer_device::controller_initialization::{ProfileName, Read};
        let mut owner = Owner::default();
        assert!(owner.begin_import());
        owner.error = Some("actual import failed".into());
        assert!(!owner.begin_import());
        owner.engine_directory = Some(ProfileDirectory {
            button_ids: vec![16],
            profile_ids: vec![3],
            active_profile: 3,
            number_of_profiles: 1,
            names: vec![ProfileName {
                slot: 3,
                name: Read::Observed {
                    value: "Real profile".into(),
                },
            }],
        });
        owner.invalidate();
        assert!(!owner.begin_import());
        assert_eq!(owner.engine_directory.as_ref().unwrap().profile_ids, [3]);
        assert!(owner.result.is_none());
        assert!(owner.mapping_projections.is_empty());
        let mut replacement = Owner::default();
        assert!(replacement.begin_import());
        assert!(replacement.engine_directory.is_none());
    }
    #[test]
    fn controller_import_plan_preserves_actual_root_order_and_does_not_synthesize_missing_fields() {
        let root: Value = serde_json::from_str(r#"{"controllerMode":{"mode":"HID"},"version":1,"powerSaving":{"value":5,"isEnabled":true}}"#).unwrap();
        assert_eq!(
            root_fields(2676, Some(&root)).unwrap(),
            [RootField::ControllerMode, RootField::PowerSaving]
        );
        assert!(
            root_fields(2676, Some(&serde_json::json!({"version":1})))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            root_fields(2676, None).unwrap(),
            [RootField::PowerSaving, RootField::ControllerMode]
        );
        assert_eq!(root_fields(2636, None).unwrap(), [RootField::PowerSaving]);
        assert!(root_fields(2676, Some(&Value::Null)).is_err());
    }
    #[test]
    fn controller_initialization_owner_rejects_replaced_requests_and_drop_cancels_the_connection() {
        let request = ControllerInitializationRequested {
            id: uuid::Uuid::new_v4(),
            generation: 2,
            key: "same-controller".into(),
            plan: InitializationPlan {
                roots: vec![RootField::PowerSaving],
                profiles: vec![],
                new_profile_fields: vec![],
                connection_features: None,
            },
            valid: Arc::new(AtomicBool::new(true)),
        };
        let mut owner = Owner::default();
        owner.request = Some(request.clone());
        assert!(owner.matches(&request));
        let mut stale = request.clone();
        stale.id = uuid::Uuid::new_v4();
        assert!(!owner.matches(&stale));
        stale = request.clone();
        stale.generation = 1;
        assert!(!owner.matches(&stale));
        owner.invalidate();
        assert!(!request.is_valid());
        assert!(!owner.matches(&request));
        let mut replacement = request;
        replacement.id = uuid::Uuid::new_v4();
        replacement.valid = Arc::new(AtomicBool::new(true));
        owner.request = Some(replacement.clone());
        assert!(owner.matches(&replacement));
        drop(owner);
        assert!(!replacement.is_valid());
    }
}

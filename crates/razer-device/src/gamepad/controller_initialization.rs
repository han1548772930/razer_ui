//! Current 2636/2676 root import and real on-board profile directory.
//! A plan contains read identities and field order, never cached setting values.
use crate::{backend::FeatureTransport, controller_feature};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, time::Duration};

use super::controller_sleep::{SleepObservation, capability};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RootField {
    PowerSaving,
    ControllerMode,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ControllerMode {
    #[serde(rename = "HID")]
    Hid,
    Xbox,
    Xinput,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Read<T> {
    Unobserved,
    Observed { value: T },
    Failed { error: String },
}
impl<T> Read<T> {
    fn from_result(result: anyhow::Result<T>) -> Self {
        match result {
            Ok(value) => Self::Observed { value },
            Err(error) => Self::Failed {
                error: format!("{error:#}"),
            },
        }
    }
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Observed { value } => Some(value),
            _ => None,
        }
    }
    fn validate(&self) -> anyhow::Result<()> {
        if let Self::Failed { error } = self {
            ensure!(!error.is_empty(), "Controller read failure omits its error");
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProfileField {
    PollingRate,
    PollingRateWireless,
    Mappings,
    Brightness,
    ControllerLightingEffects,
    ControllerDigitalActuation,
    ControllerAnalogActuation,
    ControllerAnalogCircularity,
    ControllerDeadzone,
    ControllerDPadIgnoreDiagonals,
    ControllerSensitivity,
    HapticIntensity,
}
impl ProfileField {
    pub fn source_key(self) -> &'static str {
        match self {
            Self::PollingRate => "pollingRate",
            Self::PollingRateWireless => "pollingRateWireless",
            Self::Mappings => "mappings",
            Self::Brightness => "brightness",
            Self::ControllerLightingEffects => "controllerLightingEffects",
            Self::ControllerDigitalActuation => "controllerDigitalActuation",
            Self::ControllerAnalogActuation => "controllerAnalogActuation",
            Self::ControllerAnalogCircularity => "controllerAnalogCircularity",
            Self::ControllerDeadzone => "controllerDeadzone",
            Self::ControllerDPadIgnoreDiagonals => "controllerDPadIgnoreDiagonals",
            Self::ControllerSensitivity => "controllerSensitivity",
            Self::HapticIntensity => "hapticIntensity",
        }
    }
    pub fn from_source_key(key: &str) -> Option<Self> {
        Some(match key {
            "pollingRate" => Self::PollingRate,
            "pollingRateWireless" => Self::PollingRateWireless,
            "mappings" => Self::Mappings,
            "brightness" => Self::Brightness,
            "controllerLightingEffects" => Self::ControllerLightingEffects,
            "controllerDigitalActuation" => Self::ControllerDigitalActuation,
            "controllerAnalogActuation" => Self::ControllerAnalogActuation,
            "controllerAnalogCircularity" => Self::ControllerAnalogCircularity,
            "controllerDeadzone" => Self::ControllerDeadzone,
            "controllerDPadIgnoreDiagonals" => Self::ControllerDPadIgnoreDiagonals,
            "controllerSensitivity" => Self::ControllerSensitivity,
            "hapticIntensity" => Self::HapticIntensity,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfilePlan {
    /// Real slot identities from an existing document, not profile list indices.
    pub slots: Vec<u8>,
    pub fields: Vec<ProfileField>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InitializationPlan {
    pub roots: Vec<RootField>,
    pub profiles: Vec<ProfilePlan>,
    /// Current default profile's getter order for a newly observed real slot.
    pub new_profile_fields: Vec<ProfileField>,
    /// Separate generic DeviceConnected phase, after the import was saved.
    /// Retained engine data is routing context, never a new device observation.
    pub connection_features: Option<ConnectionFeaturesPlan>,
}
impl InitializationPlan {
    pub fn connection_features(
        directory: Option<ProfileDirectory>,
        local_profile_slots: Vec<u8>,
    ) -> Self {
        Self {
            roots: Vec::new(),
            profiles: Vec::new(),
            new_profile_fields: Vec::new(),
            connection_features: Some(ConnectionFeaturesPlan {
                retained_engine_directory: directory,
                local_profile_slots,
            }),
        }
    }
    pub fn new_profile_fields(product_id: u32) -> Vec<ProfileField> {
        let mut fields = Vec::new();
        if product_id == 2636 {
            fields.extend([
                ProfileField::Brightness,
                ProfileField::ControllerLightingEffects,
            ]);
        }
        fields.push(ProfileField::PollingRate);
        if product_id == 2636 {
            fields.push(ProfileField::PollingRateWireless);
        }
        fields.push(ProfileField::Mappings);
        fields.extend([
            ProfileField::ControllerDigitalActuation,
            ProfileField::ControllerAnalogActuation,
            ProfileField::ControllerAnalogCircularity,
            ProfileField::ControllerDeadzone,
            ProfileField::ControllerDPadIgnoreDiagonals,
            ProfileField::ControllerSensitivity,
            ProfileField::HapticIntensity,
        ]);
        fields
    }
    pub fn validate(&self, product_id: u32) -> anyhow::Result<()> {
        ensure!(
            capability(product_id).is_some(),
            "No current controller root importer"
        );
        if let Some(features) = &self.connection_features {
            ensure!(
                product_id == 2676
                    && self.roots.is_empty()
                    && self.profiles.is_empty()
                    && self.new_profile_fields.is_empty(),
                "Controller connection features mix an unsupported product or import phase"
            );
            if let Some(directory) = &features.retained_engine_directory {
                directory.validate()?;
            }
            validate_ids(&features.local_profile_slots)?;
            ensure!(
                features.local_profile_slots.iter().all(|slot| *slot != 0),
                "Controller SOCD local match slots must use observed Fk wire identities"
            );
            return Ok(());
        }
        ensure!(self.roots.len() <= 2, "Too many controller root fields");
        ensure!(
            self.roots
                .iter()
                .filter(|field| **field == RootField::PowerSaving)
                .count()
                <= 1
                && self
                    .roots
                    .iter()
                    .filter(|field| **field == RootField::ControllerMode)
                    .count()
                    <= 1
                && (product_id == 2676 || !self.roots.contains(&RootField::ControllerMode)),
            "Invalid controller root field plan"
        );
        let mut slots = BTreeSet::new();
        for profile in &self.profiles {
            ensure!(
                !profile.slots.is_empty(),
                "Controller profile plan omits its real slots"
            );
            for slot in &profile.slots {
                ensure!(
                    slots.insert(*slot),
                    "A controller slot has multiple profile owners"
                );
            }
            validate_fields(&profile.fields, product_id)?;
        }
        validate_fields(&self.new_profile_fields, product_id)
    }
    fn fields(&self, slot: u8) -> &[ProfileField] {
        self.profiles
            .iter()
            .find(|profile| profile.slots.contains(&slot))
            .map(|profile| profile.fields.as_slice())
            .unwrap_or(&self.new_profile_fields)
    }
}
fn validate_fields(fields: &[ProfileField], product_id: u32) -> anyhow::Result<()> {
    let mut keys = BTreeSet::new();
    for field in fields {
        ensure!(
            keys.insert(field.source_key()),
            "Duplicate controller profile field"
        );
        ensure!(
            product_id == 2636
                || !matches!(
                    field,
                    ProfileField::Brightness | ProfileField::ControllerLightingEffects
                ),
            "Controller profile field belongs to another product"
        );
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileName {
    pub slot: u8,
    pub name: Read<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileDirectory {
    pub button_ids: Vec<u8>,
    pub profile_ids: Vec<u8>,
    pub active_profile: u8,
    pub number_of_profiles: u8,
    pub names: Vec<ProfileName>,
}
impl ProfileDirectory {
    fn validate(&self) -> anyhow::Result<()> {
        validate_ids(&self.button_ids)?;
        validate_ids(&self.profile_ids)?;
        ensure!(
            self.profile_ids.contains(&self.active_profile),
            "Active controller slot is absent from its real directory"
        );
        ensure!(
            self.names.len() == self.profile_ids.len()
                && self
                    .names
                    .iter()
                    .map(|name| name.slot)
                    .eq(self.profile_ids.iter().copied()),
            "Controller profile names differ from the real slot order"
        );
        for name in &self.names {
            name.name.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConnectionFeaturesPlan {
    pub retained_engine_directory: Option<ProfileDirectory>,
    /// Current document first obmSlotId values converted through 10511.Fk for
    /// matching only. The engine's actual GET slot is not converted again.
    pub local_profile_slots: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum SocdMode {
    #[serde(rename = "neutral")]
    Neutral,
    #[serde(rename = "last")]
    Last,
    #[serde(rename = "first")]
    First,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "absoluteUp")]
    AbsoluteUp,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SocdObservation {
    pub raw_option: u8,
}
impl SocdObservation {
    pub fn mode(self) -> Option<SocdMode> {
        match self.raw_option {
            0 => Some(SocdMode::Neutral),
            1 => Some(SocdMode::Last),
            2 => Some(SocdMode::First),
            3 => Some(SocdMode::None),
            4 => Some(SocdMode::AbsoluteUp),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SocdRead {
    pub slot: u8,
    pub result: Read<SocdObservation>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConnectionFeaturesResult {
    /// Unobserved when the source engine was already initialized. Only DY's
    /// still-uninitialized branch performs the real directory transfers.
    pub refreshed_directory: Read<ProfileDirectory>,
    pub socd: Vec<SocdRead>,
    pub canceled: bool,
}
impl ConnectionFeaturesResult {
    fn validate_for(&self, plan: &ConnectionFeaturesPlan) -> anyhow::Result<()> {
        self.refreshed_directory.validate()?;
        let directory = if let Some(retained) = &plan.retained_engine_directory {
            ensure!(
                matches!(self.refreshed_directory, Read::Unobserved),
                "Cached controller engine directory was mislabeled as a fresh read"
            );
            Some(retained)
        } else {
            ensure!(
                self.canceled || !matches!(self.refreshed_directory, Read::Unobserved),
                "Controller generic phase omitted its uninitialized engine read"
            );
            self.refreshed_directory.value()
        };
        if let Some(directory) = directory {
            directory.validate()?;
            let slots = directory
                .profile_ids
                .iter()
                .copied()
                .filter(|slot| plan.local_profile_slots.contains(slot))
                .collect::<Vec<_>>();
            ensure!(
                self.socd.len() <= slots.len()
                    && self
                        .socd
                        .iter()
                        .map(|row| row.slot)
                        .eq(slots.iter().copied().take(self.socd.len()))
                    && (self.canceled || self.socd.len() == slots.len()),
                "Controller SOCD reads changed real engine slot order"
            );
        } else {
            ensure!(
                self.socd.is_empty(),
                "Controller SOCD reads have no verified engine slot directory"
            );
        }
        for row in &self.socd {
            row.result.validate()?;
            ensure!(
                !matches!(row.result, Read::Unobserved),
                "Controller SOCD row was not attempted"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProfileValue {
    Polling {
        field: ProfileField,
        observation: crate::controller_profile::PollingObservation,
    },
    Mappings {
        observation: crate::controller_profile::MappingObservation,
    },
    Brightness {
        raw: u8,
    },
    Lighting {
        effect: crate::controller_lighting::ObservedEffect,
    },
    Digital {
        values: Vec<crate::controller_triggers::DigitalTrigger>,
    },
    Analog {
        values: Vec<crate::controller_triggers::AnalogTrigger>,
    },
    Circularity {
        option: u8,
    },
    Deadzone {
        values: Vec<crate::controller_triggers::ThumbstickValue>,
        prevention: u8,
    },
    Dpad {
        option: u8,
    },
    Sensitivity {
        values: Vec<crate::controller_triggers::ThumbstickValue>,
    },
    Haptic {
        scaling: u8,
    },
}
impl ProfileValue {
    fn validate_for(&self, product_id: u32, slot: u8, button_ids: &[u8]) -> anyhow::Result<()> {
        let slot = crate::controller_profile::wire_slot(slot);
        let sides = |ids: Vec<u8>| -> anyhow::Result<()> {
            ensure!(
                ids == [1, 2],
                "Controller profile observation omits or reorders a required side"
            );
            Ok(())
        };
        match self {
            Self::Polling { field, observation } => {
                ensure!(
                    matches!(
                        field,
                        ProfileField::PollingRate | ProfileField::PollingRateWireless
                    ),
                    "Polling result has a non-polling field"
                );
                observation.validate_for(product_id, slot)?;
            }
            Self::Mappings { observation } => observation.validate_for(slot, button_ids)?,
            Self::Analog { values } => {
                sides(values.iter().map(|value| value.controller_id()).collect())?;
                for value in values {
                    value.validate()?;
                }
            }
            Self::Digital { values } => {
                sides(values.iter().map(|value| value.controller_id()).collect())?;
                for value in values {
                    value.validate()?;
                }
            }
            Self::Deadzone { values, .. } | Self::Sensitivity { values } => {
                sides(values.iter().map(|value| value.controller_id()).collect())?;
                for value in values {
                    value.validate()?;
                }
            }
            Self::Lighting { effect } => ensure!(
                effect.profile_slot == slot
                    && effect.region_id == 0
                    && effect.color_bytes.len() <= 74
                    && usize::from(effect.color_count) * 3 <= effect.color_bytes.len(),
                "Controller lighting result has another slot/region or an incomplete palette"
            ),
            _ => {}
        }
        Ok(())
    }
    pub fn field(&self) -> ProfileField {
        match self {
            Self::Polling { field, .. } => *field,
            Self::Mappings { .. } => ProfileField::Mappings,
            Self::Brightness { .. } => ProfileField::Brightness,
            Self::Lighting { .. } => ProfileField::ControllerLightingEffects,
            Self::Digital { .. } => ProfileField::ControllerDigitalActuation,
            Self::Analog { .. } => ProfileField::ControllerAnalogActuation,
            Self::Circularity { .. } => ProfileField::ControllerAnalogCircularity,
            Self::Deadzone { .. } => ProfileField::ControllerDeadzone,
            Self::Dpad { .. } => ProfileField::ControllerDPadIgnoreDiagonals,
            Self::Sensitivity { .. } => ProfileField::ControllerSensitivity,
            Self::Haptic { .. } => ProfileField::HapticIntensity,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileRead {
    pub field: ProfileField,
    pub result: Read<ProfileValue>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProfileObservation {
    pub slot: u8,
    pub fields: Vec<ProfileRead>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InitializationResult {
    pub connection_probe: Read<crate::controller_profile::ConnectionProbeObservation>,
    pub power_saving: Read<SleepObservation>,
    pub controller_mode: Read<ControllerMode>,
    pub directory: Read<ProfileDirectory>,
    pub profiles: Vec<ProfileObservation>,
    pub canceled: bool,
    pub connection_features: Option<ConnectionFeaturesResult>,
}
impl InitializationResult {
    pub fn validate_for(&self, product_id: u32, plan: &InitializationPlan) -> anyhow::Result<()> {
        plan.validate(product_id)?;
        if let Some(features) = &plan.connection_features {
            ensure!(
                matches!(self.connection_probe, Read::Unobserved)
                    && matches!(self.power_saving, Read::Unobserved)
                    && matches!(self.controller_mode, Read::Unobserved)
                    && matches!(self.directory, Read::Unobserved)
                    && self.profiles.is_empty(),
                "Controller generic phase includes an unsolicited import observation"
            );
            let observed = self
                .connection_features
                .as_ref()
                .context("Controller generic phase omits its typed result")?;
            ensure!(
                self.canceled == observed.canceled,
                "Controller generic phase cancellation differs"
            );
            return observed.validate_for(features);
        }
        ensure!(
            self.connection_features.is_none(),
            "Controller importer includes an unsolicited generic connection phase"
        );
        self.connection_probe.validate()?;
        if let Some(connection) = self.connection_probe.value() {
            connection.validate_for(product_id)?;
        }
        ensure!(
            product_id == 2636 || matches!(self.connection_probe, Read::Unobserved),
            "Unrequested controller dock probe"
        );
        ensure!(
            self.canceled
                || product_id != 2636
                || !matches!(self.connection_probe, Read::Unobserved),
            "Controller importer omitted its source connection probe"
        );
        self.power_saving.validate()?;
        self.controller_mode.validate()?;
        self.directory.validate()?;
        ensure!(
            plan.roots.contains(&RootField::PowerSaving)
                || matches!(self.power_saving, Read::Unobserved),
            "Unrequested controller Power observation"
        );
        ensure!(
            plan.roots.contains(&RootField::ControllerMode)
                || matches!(self.controller_mode, Read::Unobserved),
            "Unrequested controller mode observation"
        );
        if !self.canceled {
            ensure!(
                !plan.roots.contains(&RootField::PowerSaving)
                    || !matches!(self.power_saving, Read::Unobserved),
                "Controller importer omitted its requested Power read"
            );
            ensure!(
                !plan.roots.contains(&RootField::ControllerMode)
                    || !matches!(self.controller_mode, Read::Unobserved),
                "Controller importer omitted its requested mode read"
            );
            ensure!(
                !matches!(self.directory, Read::Unobserved),
                "Controller importer omitted its directory read"
            );
        }
        if let Read::Observed { value: directory } = &self.directory {
            directory.validate()?;
            ensure!(
                self.profiles.len() <= directory.profile_ids.len(),
                "Too many controller profile observations"
            );
            ensure!(
                self.profiles
                    .iter()
                    .map(|profile| profile.slot)
                    .eq(directory
                        .profile_ids
                        .iter()
                        .copied()
                        .take(self.profiles.len())),
                "Controller profile observations changed real slot order"
            );
            for profile in &self.profiles {
                let fields = plan.fields(profile.slot);
                ensure!(
                    profile.fields.len() <= fields.len(),
                    "Too many controller profile field observations"
                );
                ensure!(
                    profile
                        .fields
                        .iter()
                        .map(|read| read.field)
                        .eq(fields.iter().copied().take(profile.fields.len())),
                    "Controller profile observations changed field order"
                );
                for read in &profile.fields {
                    read.result.validate()?;
                    ensure!(
                        !matches!(read.result, Read::Unobserved),
                        "Controller profile entry has no attempted read"
                    );
                    if let Read::Observed { value } = &read.result {
                        ensure!(
                            value.field() == read.field,
                            "Controller profile result belongs to another field"
                        );
                        value.validate_for(product_id, profile.slot, &directory.button_ids)?;
                    }
                }
                ensure!(
                    self.canceled || profile.fields.len() == fields.len(),
                    "Controller profile import ended before its final field"
                );
            }
            ensure!(
                self.canceled || self.profiles.len() == directory.profile_ids.len(),
                "Controller import ended before its final real slot"
            );
        } else {
            ensure!(
                self.profiles.is_empty(),
                "Controller profile reads have no observed directory"
            );
        }
        Ok(())
    }
}

pub struct InitializationProtocol<
    'a,
    T: FeatureTransport + ?Sized,
    W: FnMut(Duration) -> anyhow::Result<()>,
> {
    transport: &'a T,
    product_id: u32,
    wait: W,
    transaction: u8,
}
impl<'a, T: FeatureTransport + ?Sized, W: FnMut(Duration) -> anyhow::Result<()>>
    InitializationProtocol<'a, T, W>
{
    pub fn new(transport: &'a T, product_id: u32, wait: W) -> anyhow::Result<Self> {
        ensure!(
            capability(product_id).is_some(),
            "No current controller root importer"
        );
        Ok(Self {
            transport,
            product_id,
            wait,
            transaction: 0,
        })
    }
    fn exchange(
        &mut self,
        header: [u8; 3],
        data: &[u8],
        extended: bool,
    ) -> anyhow::Result<Vec<u8>> {
        let source = capability(self.product_id).context("No controller source capability")?;
        let transaction = self.transaction;
        self.transaction = (transaction + 1) % 31;
        let request = controller_feature::report(source.report_id(), transaction, header, data)?;
        let out_delay = source.delay();
        let in_delay = if extended { out_delay * 2 } else { out_delay };
        controller_feature::exchange_with_timing(
            self.transport,
            &request,
            out_delay,
            in_delay,
            in_delay,
            &mut self.wait,
            10,
        )
    }
    fn ids(&mut self, header: [u8; 3]) -> anyhow::Result<Vec<u8>> {
        let data = self.exchange(header, &[], false)?;
        let total = usize::from(*data.first().context("Controller ID list omits its count")?);
        ensure!(data.len() > total, "Controller ID list is truncated");
        let ids = data[1..=total].to_vec();
        validate_ids(&ids)?;
        Ok(ids)
    }
    fn byte(&mut self, header: [u8; 3]) -> anyhow::Result<u8> {
        let data = self.exchange(header, &[], false)?;
        ensure!(
            data.len() == 1,
            "Controller scalar getter returned an invalid length"
        );
        Ok(data[0])
    }
    fn name(&mut self, slot: u8) -> anyhow::Result<String> {
        let mut last_error = String::from("Controller returned no profile-name bytes");
        for _ in 0..3 {
            let mut bytes = Vec::new();
            let mut total = None;
            for offset in [0u16, 64] {
                let count = if offset == 0 { 64usize } else { 16 };
                let [high, low] = offset.to_be_bytes();
                let response = match self.exchange([80, 5, 136], &[slot, high, low, 0, 0], true) {
                    Ok(response) => response,
                    Err(error) => {
                        last_error = format!("{error:#}");
                        break;
                    }
                };
                if response.len() < 5 || response[0] != slot || response[1..3] != [high, low] {
                    last_error = "Controller profile-name reply has another slot or offset".into();
                    break;
                }
                let size = usize::from(u16::from_be_bytes([response[3], response[4]]));
                if size > 80 || size % 2 != 0 || total.is_some_and(|old| old != size) {
                    last_error =
                        "Controller profile-name size is invalid or changed between chunks".into();
                    break;
                }
                total = Some(size);
                let available = response
                    .len()
                    .saturating_sub(5)
                    .min(count)
                    .min(size.saturating_sub(bytes.len()));
                bytes.extend_from_slice(&response[5..5 + available]);
                if bytes.len() >= size {
                    let mut words = bytes
                        .chunks_exact(2)
                        .map(|word| u16::from_le_bytes([word[0], word[1]]))
                        .collect::<Vec<_>>();
                    if words.first() == Some(&0xfeff) {
                        words.remove(0);
                    }
                    if let Some(end) = words.iter().position(|word| *word == 0) {
                        words.truncate(end);
                    }
                    return String::from_utf16(&words)
                        .context("Controller profile name is invalid UTF-16");
                }
                last_error = "Controller profile-name bytes are incomplete".into();
            }
        }
        anyhow::bail!("{last_error}")
    }
    fn directory(&mut self) -> anyhow::Result<ProfileDirectory> {
        // DY -> engine.init performs these actual transfers in this order.
        let button_ids = self.ids([80, 2, 132])?;
        let profile_ids = self.ids([80, 5, 129])?;
        let active_profile = self.byte([1, 5, 132])?;
        let number_of_profiles = self.byte([1, 5, 128])?;
        let names = profile_ids
            .iter()
            .map(|slot| ProfileName {
                slot: *slot,
                name: Read::from_result(self.name(*slot)),
            })
            .collect();
        let directory = ProfileDirectory {
            button_ids,
            profile_ids,
            active_profile,
            number_of_profiles,
            names,
        };
        directory.validate()?;
        Ok(directory)
    }
    fn control(&mut self, slot: u8, side: u8, length: u8, command: u8) -> anyhow::Result<Vec<u8>> {
        let data = self.exchange([length, 12, command], &[slot, side], false)?;
        ensure!(
            data.len() >= usize::from(length) && data[0..2] == [slot, side],
            "Controller control reply is truncated or belongs to another slot/side"
        );
        Ok(data)
    }
    fn profile(
        &mut self,
        slot: u8,
        field: ProfileField,
        button_ids: &[u8],
        canceled: &mut impl FnMut() -> bool,
    ) -> anyhow::Result<ProfileValue> {
        use crate::controller_triggers::{AnalogTrigger, DigitalTrigger, ThumbstickValue};
        let product_id = self.product_id;
        // Keep ProfileObservation.slot as the source/engine directory identity;
        // each field owner passes its profile through 10511.Fk before I/O.
        let slot = crate::controller_profile::wire_slot(slot);
        Ok(match field {
            ProfileField::PollingRate | ProfileField::PollingRateWireless => {
                ProfileValue::Polling {
                    field,
                    observation: crate::controller_profile::read_polling(
                        product_id,
                        slot,
                        &mut |header, data| self.exchange(header, data, false),
                    )?,
                }
            }
            ProfileField::Mappings => ProfileValue::Mappings {
                observation: crate::controller_profile::read_mappings(
                    product_id,
                    slot,
                    button_ids,
                    &mut |header, data| self.exchange(header, data, false),
                    canceled,
                )?,
            },
            ProfileField::ControllerAnalogActuation => {
                let mut values = Vec::new();
                for side in [1, 2] {
                    let range = self.control(slot, side, 4, 147)?;
                    let mode = self.control(slot, side, 3, 149)?;
                    ensure!(mode[2] <= 1, "Unknown controller trigger mode");
                    values.push(AnalogTrigger::new(side, range[2], range[3], mode[2] == 0)?);
                }
                ProfileValue::Analog { values }
            }
            ProfileField::ControllerDigitalActuation => {
                let mut values = Vec::new();
                for side in [1, 2] {
                    let point = self.control(slot, side, 4, 150)?;
                    let mode = self.control(slot, side, 3, 149)?;
                    ensure!(
                        mode[2] <= 1 && point[3] <= 1,
                        "Unknown controller digital trigger flags"
                    );
                    values.push(DigitalTrigger::new(
                        side,
                        point[2],
                        point[3] != 0,
                        mode[2] == 1,
                    )?);
                }
                ProfileValue::Digital { values }
            }
            ProfileField::ControllerAnalogCircularity => ProfileValue::Circularity {
                option: self.control(slot, 1, 3, 146)?[2],
            },
            ProfileField::ControllerDeadzone => {
                let mut values = Vec::new();
                let mut prevention = 0;
                for side in [1, 2] {
                    let data = self.control(slot, side, 9, 133)?;
                    if side == 1 {
                        prevention = data[8];
                    }
                    values.push(ThumbstickValue::new(
                        side,
                        u16::from_be_bytes([data[2], data[3]]),
                    )?);
                }
                ProfileValue::Deadzone { values, prevention }
            }
            ProfileField::ControllerSensitivity => {
                let mut values = Vec::new();
                for side in [1, 2] {
                    values.push(ThumbstickValue::new(
                        side,
                        u16::from(self.control(slot, side, 3, 130)?[2]),
                    )?);
                }
                ProfileValue::Sensitivity { values }
            }
            ProfileField::ControllerDPadIgnoreDiagonals => {
                let data = self.exchange([2, 12, 151], &[slot], false)?;
                ensure!(
                    data.len() >= 2 && data[0] == slot,
                    "Controller D-pad reply belongs to another slot"
                );
                ProfileValue::Dpad { option: data[1] }
            }
            ProfileField::HapticIntensity => ProfileValue::Haptic {
                scaling: self.control(slot, 5, 3, 134)?[2],
            },
            ProfileField::Brightness => {
                let regions = self.exchange([80, 15, 128], &[], false)?;
                let region = *regions
                    .first()
                    .context("Controller returned no lighting region")?;
                let data = self.exchange([3, 15, 132], &[slot, region], false)?;
                ensure!(
                    data.len() >= 3 && data[0..2] == [slot, region],
                    "Controller brightness reply belongs to another slot or region"
                );
                ProfileValue::Brightness { raw: data[2] }
            }
            ProfileField::ControllerLightingEffects => {
                let data = self.exchange([80, 15, 130], &[slot, 0], false)?;
                let mut effect = crate::controller_lighting::ObservedEffect::parse(&data)?;
                ensure!(
                    effect.profile_slot == slot && effect.region_id == 0,
                    "Controller lighting reply belongs to another slot or region"
                );
                if effect.effect_id == 13 {
                    let data = self.exchange([5, 15, 130], &[slot, 0, 13, 0, 1], false)?;
                    effect = crate::controller_lighting::ObservedEffect::parse(&data)?;
                    ensure!(
                        effect.profile_slot == slot
                            && effect.region_id == 0
                            && effect.effect_id == 13,
                        "Controller extended lighting reply differs"
                    );
                }
                ProfileValue::Lighting { effect }
            }
        })
    }
    pub fn execute(
        &mut self,
        plan: &InitializationPlan,
        mut canceled: impl FnMut() -> bool,
    ) -> anyhow::Result<InitializationResult> {
        plan.validate(self.product_id)?;
        let mut result = InitializationResult {
            connection_probe: Read::Unobserved,
            power_saving: Read::Unobserved,
            controller_mode: Read::Unobserved,
            directory: Read::Unobserved,
            profiles: Vec::new(),
            canceled: false,
            connection_features: None,
        };
        if let Some(features) = &plan.connection_features {
            let mut observed = ConnectionFeaturesResult {
                refreshed_directory: Read::Unobserved,
                socd: Vec::new(),
                canceled: canceled(),
            };
            if !observed.canceled && features.retained_engine_directory.is_none() {
                observed.refreshed_directory = Read::from_result(self.directory());
            }
            let directory = features
                .retained_engine_directory
                .as_ref()
                .or_else(|| observed.refreshed_directory.value())
                .cloned();
            if let Some(directory) = directory {
                for slot in directory.profile_ids {
                    if !features.local_profile_slots.contains(&slot) {
                        continue;
                    }
                    if canceled() {
                        observed.canceled = true;
                        break;
                    }
                    let read = self.exchange([2,12,155], &[slot], false).and_then(|data| {
                        ensure!(data.len()==2 && data[0]==slot, "Controller SOCD reply is incomplete or belongs to another real slot");
                        Ok(SocdObservation { raw_option:data[1] })
                    });
                    observed.socd.push(SocdRead {
                        slot,
                        result: Read::from_result(read),
                    });
                }
            }
            observed.canceled |= canceled();
            result.canceled = observed.canceled;
            result.connection_features = Some(observed);
            result.validate_for(self.product_id, plan)?;
            return Ok(result);
        }
        if self.product_id == 2636 {
            if canceled() {
                result.canceled = true;
                result.validate_for(self.product_id, plan)?;
                return Ok(result);
            }
            result.connection_probe =
                Read::from_result(crate::controller_profile::read_connection_probe(
                    self.product_id,
                    &mut |header, data| self.exchange(header, data, false),
                ));
        }
        for field in &plan.roots {
            if canceled() {
                result.canceled = true;
                break;
            }
            match field {
                RootField::PowerSaving => {
                    result.power_saving = Read::from_result(
                        self.exchange([2, 7, 131], &[0, 0], false).and_then(|data| {
                            let bytes: [u8; 2] = data.try_into().map_err(|data: Vec<u8>| {
                                anyhow::anyhow!(
                                    "Controller Power getter returned {} bytes",
                                    data.len()
                                )
                            })?;
                            Ok(SleepObservation::from_seconds(if self.product_id == 2636 {
                                u16::from_le_bytes(bytes)
                            } else {
                                u16::from_be_bytes(bytes)
                            }))
                        }),
                    );
                }
                RootField::ControllerMode => {
                    result.controller_mode =
                        Read::from_result(self.byte([1, 0, 211]).and_then(|value| match value {
                            0 => Ok(ControllerMode::Hid),
                            1 => Ok(ControllerMode::Xbox),
                            2 => Ok(ControllerMode::Xinput),
                            _ => anyhow::bail!("Unknown controller mode {value}"),
                        }));
                }
            }
        }
        if canceled() {
            result.canceled = true;
        }
        if !result.canceled {
            result.directory = Read::from_result(self.directory());
            if let Read::Observed { value: directory } = &result.directory {
                for slot in &directory.profile_ids {
                    if canceled() {
                        result.canceled = true;
                        break;
                    }
                    let mut profile = ProfileObservation {
                        slot: *slot,
                        fields: Vec::new(),
                    };
                    for field in plan.fields(*slot) {
                        if canceled() {
                            result.canceled = true;
                            break;
                        }
                        profile.fields.push(ProfileRead {
                            field: *field,
                            result: Read::from_result(self.profile(
                                *slot,
                                *field,
                                &directory.button_ids,
                                &mut canceled,
                            )),
                        });
                    }
                    result.profiles.push(profile);
                    if result.canceled {
                        break;
                    }
                }
            }
        }
        result.validate_for(self.product_id, plan)?;
        Ok(result)
    }
}
fn validate_ids(ids: &[u8]) -> anyhow::Result<()> {
    ensure!(
        ids.len() <= 79 && ids.iter().copied().collect::<BTreeSet<_>>().len() == ids.len(),
        "Controller ID list has duplicates or exceeds the report"
    );
    Ok(())
}

#[cfg(test)]
#[path = "controller_initialization_tests.rs"]
mod tests;

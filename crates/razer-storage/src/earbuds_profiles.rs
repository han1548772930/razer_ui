//! Local profile owner for the PROTOCOL40 earbuds auto-pause and PID2657 ANC
//! fields. This app owns the persisted format; it is a local profile, never a
//! device observation. Profile selection follows the source serial-metadata
//! selector: the active GUID's profile, else the first profile when there is
//! no active GUID. An active GUID that matches no profile is reported as
//! `Unmatched` and not resolved to any profile.
use anyhow::{Context as _, Result, bail};
use razer_device::earbuds_protocol40::{
    self as proto, AncChange, AncLevel, AncMode, AncSetting, supports_anc, supports_auto_pause,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::Path;

mod defaults;
mod equalizer;
mod power;
pub use power::{accept_power_saved, merge_power, read_power, read_power_profile};
pub use equalizer::{read_equalizer, merge_equalizer, initial_equalizer, read_equalizer_presets, merge_equalizer_source, initial_equalizer_presets, equalizer_defaults, initial_equalizer_source};

const AUTO_PAUSE_FIELD: &str = "audioAutoPauseControl";
const DO_NOT_DISTURB_FIELD: &str = "notDisturb";
const VOICE_PROMPT_FIELD: &str = "audioVoicePromptControl";
const GESTURE_FIELD: &str = "audioGestureControl";
const TAP_FIELD: &str = "audioTapSpeed";
const ANC_FIELD: &str = "noiseCancellation";

pub fn read_anc(
    device: &razer_model::model::Device,
    profile_id: &str,
) -> Result<Option<AncSetting>> {
    if !supports_anc(device.product_id) {
        bail!("Product does not register PID2657 ANC");
    }
    let index = workspace_profile_index(device, profile_id)?;
    let Some(control) =
        workspace_control(device.profiles[index].source_settings.as_ref(), ANC_FIELD)?
    else {
        return Ok(None);
    };
    let fields: NoiseCancellation = serde_json::from_value(Value::Object(control.clone()))
        .context("Invalid local ANC settings")?;
    fields.to_setting().map(Some)
}

pub fn merge_anc(
    device: &mut razer_model::model::Device,
    profile_id: &str,
    setting: AncSetting,
) -> Result<()> {
    read_anc(device, profile_id)?;
    let index = workspace_profile_index(device, profile_id)?;
    let field = workspace_control_mut(&mut device.profiles[index].source_settings, ANC_FIELD);
    let fields = NoiseCancellation::from_setting(setting);
    field.insert("isEnabled".into(), Value::Bool(fields.is_enabled));
    field.insert("status".into(), Value::from(fields.status));
    field.insert("level".into(), Value::from(fields.level));
    Ok(())
}

/// Stage the source initial profile after the caller proves its initialization
/// gate. This only changes the workspace draft; saving and device restore are
/// separate steps. The source provider supplies a fresh GUID and computer-name
/// prefix, not the template's empty GUID and plain "Default" name.
pub fn initialize_auto_pause_profile(
    device: &mut razer_model::model::Device,
    stable_guid: &str,
    source_name: &str,
) -> Result<()> {
    if !device.profiles.is_empty() || !device.active_profile.is_empty() {
        bail!("earbuds initial profile cannot replace an existing selection");
    }
    if stable_guid.is_empty() || source_name.is_empty() {
        bail!("earbuds initial profile requires its generated identity and source name");
    }
    let source = defaults::profile(device.product_id, stable_guid, source_name)?;
    device.profiles.push(razer_model::model::Profile {
        id: stable_guid.to_owned(),
        guid: stable_guid.to_owned(),
        name: source_name.to_owned(),
        source_settings: Some(serde_json::json!({"profile": source})),
        settings: None,
        dpi_stages: None,
    });
    device.active_profile = stable_guid.to_owned();
    Ok(())
}

fn workspace_profile_index(device: &razer_model::model::Device, profile_id: &str) -> Result<usize> {
    if !supports_auto_pause(device.product_id) {
        bail!("product {} does not register auto-pause", device.product_id);
    }
    if profile_id.is_empty() {
        bail!("auto-pause requires a workspace profile identity");
    }
    let mut matches = device
        .profiles
        .iter()
        .enumerate()
        .filter(|(_, profile)| profile.id == profile_id);
    let (index, _) = matches.next().context("auto-pause profile was removed")?;
    if matches.next().is_some() {
        bail!("auto-pause profile identity is duplicated");
    }
    Ok(index)
}

fn workspace_control<'a>(
    source: Option<&'a Value>,
    field: &str,
) -> Result<Option<&'a Map<String, Value>>> {
    let Some(source) = source else {
        return Ok(None);
    };
    let root = source
        .as_object()
        .context("audio snapshot must be an object")?;
    // Mounted audio snapshots wrap the profile; original profile documents
    // carry the same field directly. Two competing owners are ambiguous.
    let profile = match root.get("profile") {
        Some(profile) => {
            if root.contains_key(field) {
                bail!("{field} has both a profile and snapshot-level value");
            }
            profile
                .as_object()
                .context("audio profile must be an object")?
        }
        None => root,
    };
    profile
        .get(field)
        .map(|control| {
            control
                .as_object()
                .with_context(|| format!("{field} control must be an object"))
        })
        .transpose()
}

fn workspace_boolean_field(source: Option<&Value>, field: &str) -> Result<Option<bool>> {
    workspace_control(source, field)?
        .and_then(|control| control.get("isEnabled"))
        .map(|value| {
            value
                .as_bool()
                .with_context(|| format!("{field} isEnabled must be boolean"))
        })
        .transpose()
}

/// Read the selected workspace profile's local field without turning missing
/// data into a default or a device observation. Selection is captured by ID.
pub fn read_auto_pause(
    device: &razer_model::model::Device,
    profile_id: &str,
) -> Result<proto::AutoPauseProfileField> {
    let index = workspace_profile_index(device, profile_id)?;
    Ok(proto::AutoPauseProfileField {
        is_enabled: workspace_boolean_field(
            device.profiles[index].source_settings.as_ref(),
            AUTO_PAUSE_FIELD,
        )?,
    })
}

/// Read the source notDisturb field. An old profile without it remains absent;
/// reading local configuration does not initialize or observe device state.
pub fn read_do_not_disturb(
    device: &razer_model::model::Device,
    profile_id: &str,
) -> Result<Option<bool>> {
    let index = workspace_profile_index(device, profile_id)?;
    workspace_boolean_field(
        device.profiles[index].source_settings.as_ref(),
        DO_NOT_DISTURB_FIELD,
    )
}

/// Read the persisted voice-prompt field without materializing a restore default.
pub fn read_voice_prompt(
    device: &razer_model::model::Device,
    profile_id: &str,
) -> Result<Option<bool>> {
    let index = workspace_profile_index(device, profile_id)?;
    workspace_boolean_field(
        device.profiles[index].source_settings.as_ref(),
        VOICE_PROMPT_FIELD,
    )
}

pub fn read_gesture(device: &razer_model::model::Device, profile_id: &str) -> Result<Option<bool>> {
    let index = workspace_profile_index(device, profile_id)?;
    workspace_boolean_field(
        device.profiles[index].source_settings.as_ref(),
        GESTURE_FIELD,
    )
}

pub fn read_tap_speed(
    device: &razer_model::model::Device,
    profile_id: &str,
) -> Result<Option<razer_device::earbuds_gesture::TapSpeed>> {
    let index = workspace_profile_index(device, profile_id)?;
    let Some(control) =
        workspace_control(device.profiles[index].source_settings.as_ref(), TAP_FIELD)?
    else {
        return Ok(None);
    };
    let value = control
        .get("tapSpeed")
        .context("audioTapSpeed omits tapSpeed")?;
    serde_json::from_value(value.clone())
        .context("Invalid audio tap speed")
        .map(Some)
}

pub fn merge_gesture(
    device: &mut razer_model::model::Device,
    profile_id: &str,
    enabled: bool,
) -> Result<()> {
    merge_boolean_field(device, profile_id, GESTURE_FIELD, enabled)
}

pub fn merge_tap_speed(
    device: &mut razer_model::model::Device,
    profile_id: &str,
    speed: razer_device::earbuds_gesture::TapSpeed,
) -> Result<()> {
    read_tap_speed(device, profile_id)?;
    let index = workspace_profile_index(device, profile_id)?;
    workspace_control_mut(&mut device.profiles[index].source_settings, TAP_FIELD)
        .insert("tapSpeed".into(), Value::String(speed.key().into()));
    Ok(())
}

/// Merge an explicit edit into the existing workspace draft. The caller owns
/// atomic persistence and may submit device I/O only after that save succeeds.
/// Validation completes before mutation; unrelated profile fields are retained.
pub fn merge_auto_pause(
    device: &mut razer_model::model::Device,
    profile_id: &str,
    enabled: bool,
) -> Result<()> {
    merge_boolean_field(device, profile_id, AUTO_PAUSE_FIELD, enabled)
}

/// Stage an explicit DND edit for the existing atomic workspace save queue.
pub fn merge_do_not_disturb(
    device: &mut razer_model::model::Device,
    profile_id: &str,
    enabled: bool,
) -> Result<()> {
    merge_boolean_field(device, profile_id, DO_NOT_DISTURB_FIELD, enabled)
}

/// Stage an explicit voice-prompt edit; profile restore never calls this writer.
pub fn merge_voice_prompt(
    device: &mut razer_model::model::Device,
    profile_id: &str,
    enabled: bool,
) -> Result<()> {
    merge_boolean_field(device, profile_id, VOICE_PROMPT_FIELD, enabled)
}

fn merge_boolean_field(
    device: &mut razer_model::model::Device,
    profile_id: &str,
    field: &str,
    enabled: bool,
) -> Result<()> {
    let index = workspace_profile_index(device, profile_id)?;
    workspace_boolean_field(device.profiles[index].source_settings.as_ref(), field)?;
    workspace_control_mut(&mut device.profiles[index].source_settings, field)
        .insert("isEnabled".into(), Value::Bool(enabled));
    Ok(())
}

/// Call only after validating the selected profile and control object.
fn workspace_control_mut<'a>(
    source: &'a mut Option<Value>,
    field: &str,
) -> &'a mut Map<String, Value> {
    let source = source.get_or_insert_with(|| Value::Object(Map::new()));
    let root = source.as_object_mut().expect("validated audio snapshot");
    let profile = if root.contains_key(field)
        || (!root.contains_key("profile") && root.contains_key("guid"))
    {
        root
    } else {
        root.entry("profile")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("validated audio profile")
    };
    profile
        .entry(field)
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("validated audio control")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoPauseControl {
    pub is_enabled: bool,
}

/// Persisted `{isEnabled, status, level}`; `status` 10/255, `level` 1/10.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseCancellation {
    pub is_enabled: bool,
    pub status: u8,
    pub level: u8,
}

impl NoiseCancellation {
    pub fn from_setting(setting: AncSetting) -> Self {
        Self {
            is_enabled: setting.enabled,
            status: setting.mode.ui_status(),
            level: setting.level as u8,
        }
    }

    /// Boundary validation: only the source page values are accepted.
    pub fn to_setting(self) -> Result<AncSetting> {
        let mode = match self.status {
            10 => AncMode::Anc,
            255 => AncMode::Ambient,
            other => bail!("unknown persisted ANC status {other}"),
        };
        let level = match self.level {
            1 => AncLevel::Reduced,
            10 => AncLevel::Full,
            other => bail!("unknown persisted ANC level {other}"),
        };
        Ok(AncSetting {
            enabled: self.is_enabled,
            mode,
            level,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EarbudsProfile {
    pub guid: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_auto_pause_control: Option<AutoPauseControl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noise_cancellation: Option<NoiseCancellation>,
    /// Other profile fields owned elsewhere are preserved unchanged.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EarbudsProfileStore {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_guid: Option<String>,
    pub profiles: Vec<EarbudsProfile>,
}

const SCHEMA_VERSION: u32 = 1;

impl EarbudsProfileStore {
    /// MW default profile: `guid:""`, `name:"Default"`, auto-pause on and,
    /// for PID2657 only, `noiseCancellation{true,10,10}`.
    pub fn source_default(product_id: u32) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            active_guid: None,
            profiles: vec![EarbudsProfile {
                guid: String::new(),
                name: "Default".into(),
                audio_auto_pause_control: supports_auto_pause(product_id).then_some(
                    AutoPauseControl {
                        is_enabled: proto::AUTO_PAUSE_PROFILE_DEFAULT,
                    },
                ),
                noise_cancellation: supports_anc(product_id)
                    .then(|| NoiseCancellation::from_setting(proto::ANC_PROFILE_DEFAULT)),
                extra: Map::new(),
            }],
        }
    }

    pub fn select(&self) -> ProfileSelection {
        match self.active_guid.as_deref().filter(|guid| !guid.is_empty()) {
            None if self.profiles.is_empty() => ProfileSelection::Empty,
            None => ProfileSelection::Profile(0),
            Some(guid) => self
                .profiles
                .iter()
                .position(|profile| profile.guid == guid)
                .map_or_else(
                    || ProfileSelection::Unmatched(guid.to_owned()),
                    ProfileSelection::Profile,
                ),
        }
    }

    pub fn selected_mut(&mut self) -> Result<&mut EarbudsProfile> {
        match self.select() {
            ProfileSelection::Profile(index) => Ok(&mut self.profiles[index]),
            other => bail!("no selectable earbuds profile: {other:?}"),
        }
    }

    pub fn selected(&self) -> Result<&EarbudsProfile> {
        match self.select() {
            ProfileSelection::Profile(index) => Ok(&self.profiles[index]),
            other => bail!("no selectable earbuds profile: {other:?}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileSelection {
    Profile(usize),
    Empty,
    /// Open ambiguity: the active GUID names no stored profile.
    Unmatched(String),
}

impl EarbudsProfile {
    /// Restore input: a missing field stays `None` (the feature then uses
    /// its own source fallback).
    pub fn auto_pause_field(&self) -> proto::AutoPauseProfileField {
        proto::AutoPauseProfileField {
            is_enabled: self.audio_auto_pause_control.map(|field| field.is_enabled),
        }
    }

    /// `updateStorage`: a missing object starts as `{isEnabled:true}`.
    pub fn set_auto_pause(&mut self, enabled: bool) -> AutoPauseControl {
        let field = AutoPauseControl {
            is_enabled: enabled,
        };
        self.audio_auto_pause_control = Some(field);
        field
    }

    /// `updateStorage` for ANC: `level ?? 1`, other nullish fields merge
    /// with the stored value (or local default). Returns the stored value.
    pub fn set_noise_cancellation(&mut self, change: AncChange) -> Result<NoiseCancellation> {
        let base = match self.noise_cancellation {
            Some(stored) => stored.to_setting()?,
            None => proto::ANC_PROFILE_DEFAULT,
        };
        let merged = AncSetting {
            enabled: change.enabled.unwrap_or(base.enabled),
            mode: change.mode.unwrap_or(base.mode),
            level: change.level.unwrap_or(AncLevel::Reduced),
        };
        let stored = NoiseCancellation::from_setting(merged);
        self.noise_cancellation = Some(stored);
        Ok(stored)
    }
}

fn key(product_id: u32, serial: &str) -> Result<String> {
    if serial.is_empty() {
        bail!("earbuds profile storage requires a serial number");
    }
    Ok(format!("razer-ui.earbuds-profiles.{product_id}.{serial}"))
}

pub fn load(path: &Path, product_id: u32, serial: &str) -> Result<Option<EarbudsProfileStore>> {
    let Some(value) = crate::source_local::get(path, &key(product_id, serial)?)? else {
        return Ok(None);
    };
    let store: EarbudsProfileStore =
        serde_json::from_value(value).context("invalid earbuds profile store")?;
    if store.schema_version != SCHEMA_VERSION {
        bail!(
            "unsupported earbuds profile schema {}",
            store.schema_version
        );
    }
    Ok(Some(store))
}

pub fn save(path: &Path, product_id: u32, serial: &str, store: &EarbudsProfileStore) -> Result<()> {
    crate::source_local::set(
        path,
        &key(product_id, serial)?,
        &serde_json::to_value(store)?,
    )
}

/// `setupOnDeviceReady`: create the default only when storage is missing.
pub fn load_or_create(path: &Path, product_id: u32, serial: &str) -> Result<EarbudsProfileStore> {
    if let Some(store) = load(path, product_id, serial)? {
        return Ok(store);
    }
    let store = EarbudsProfileStore::source_default(product_id);
    save(path, product_id, serial, &store)?;
    Ok(store)
}

#[cfg(test)]
#[path = "earbuds_profiles_tests.rs"]
mod tests;

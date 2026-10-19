//! Current keyboard profile schemas and explicit local initialization data.
//! Constructors are consumed by a proved first initialization or user action;
//! decoding and mapping compilation never invoke them to repair missing input.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const PRODUCT_ID: u32 = 679;
pub const FACTORY_GUID: &str = "af06d371-861f-4b98-8d78-bfaef5cfdebf";
pub const FACTORY_NAME: &str = "Factory Default";
pub const SCHEMA_VERSION: u32 = 13;

pub type Extensions = BTreeMap<String, Value>;

#[path = "keyboard_high_speed_profile.rs"]
pub mod high_speed;
pub use high_speed::{HighSpeedSettings, validate_profile_settings};

fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSettings {
    pub actuation_point: ActuationPoint,
    pub switch_off_lighting: LightingOff,
    pub brightness: Brightness,
    pub quick_effects: QuickEffects,
    pub gaming_mode: GamingMode,
    pub snap_tap: SnapTap,
    pub continuous_rapid_trigger: bool,
    pub r#macro: Extensions,
    pub mappings: Vec<Mapping>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl ProfileSettings {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.actuation_point
                .values()
                .iter()
                .all(|value| { value.is_finite() && *value > 0.0 && *value <= 4.0 }),
            "Keyboard profile actuation point is out of range"
        );
        validate_common(
            &self.brightness,
            &self.switch_off_lighting,
            &self.gaming_mode,
            &self.snap_tap,
            &self.mappings,
        )
    }
}

fn validate_common(
    brightness: &Brightness,
    lighting: &LightingOff,
    gaming: &GamingMode,
    snap_tap: &SnapTap,
    mappings: &[Mapping],
) -> anyhow::Result<()> {
    ensure!(
        brightness.value <= 100 && lighting.idle_minutes > 0,
        "Keyboard profile brightness or idle duration is out of range"
    );
    gaming.validate()?;
    snap_tap.validate()?;
    for mapping in mappings {
        mapping.validate()?;
    }
    Ok(())
}

/// Current515 main21503 lacks the analog-only profile fields.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyProfile {
    pub guid: String,
    pub name: String,
    pub switch_off_lighting: LightingOff,
    pub brightness: Brightness,
    pub quick_effects: QuickEffects,
    pub gaming_mode: GamingMode,
    pub snap_tap: SnapTap,
    pub r#macro: Extensions,
    pub mappings: Vec<Mapping>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl LegacyProfile {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(!self.guid.is_empty(), "Keyboard profile GUID is absent");
        validate_common(
            &self.brightness,
            &self.switch_off_lighting,
            &self.gaming_mode,
            &self.snap_tap,
            &self.mappings,
        )
    }
}

pub fn decode_legacy_profile(value: &Value) -> anyhow::Result<LegacyProfile> {
    let profile: LegacyProfile = serde_json::from_value(value.clone())
        .context("Legacy keyboard source profile is incomplete or malformed")?;
    profile.validate()?;
    Ok(profile)
}

/// Current515 shares these explicit main21503 values with current679's
/// common lighting and gaming fields; analog settings are absent.
pub fn initial_legacy_profile() -> LegacyProfile {
    let common = initial_settings();
    LegacyProfile {
        guid: String::new(),
        name: String::new(),
        switch_off_lighting: common.switch_off_lighting,
        brightness: common.brightness,
        quick_effects: common.quick_effects,
        gaming_mode: common.gaming_mode,
        snap_tap: common.snap_tap,
        r#macro: common.r#macro,
        mappings: common.mappings,
        extensions: Extensions::new(),
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Profile {
    pub guid: String,
    pub name: String,
    #[serde(flatten)]
    pub settings: ProfileSettings,
}

impl Profile {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(!self.guid.is_empty(), "Keyboard profile GUID is absent");
        self.settings.validate()
    }
}

pub fn decode_profile(value: &Value) -> anyhow::Result<Profile> {
    let profile: Profile = serde_json::from_value(value.clone())
        .context("Keyboard source profile is incomplete or malformed")?;
    profile.validate()?;
    Ok(profile)
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ActuationPoint {
    Named {
        #[serde(rename = "0")]
        make: f64,
        #[serde(rename = "1")]
        release: f64,
        #[serde(flatten)]
        extensions: Extensions,
    },
    Ordered([f64; 2]),
}

impl ActuationPoint {
    pub fn values(&self) -> [f64; 2] {
        match self {
            Self::Named { make, release, .. } => [*make, *release],
            Self::Ordered(values) => *values,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightingOff {
    pub is_display_on: bool,
    pub is_idle_enabled: bool,
    pub idle_minutes: u32,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Brightness {
    pub is_enabled: bool,
    pub value: u8,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickEffects {
    pub selected_effect_id: u32,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamingMode {
    pub state: u8,
    pub is_windows_key_disabled: bool,
    #[serde(rename = "isAltF4Disabled")]
    pub is_alt_f4_disabled: bool,
    pub is_alt_tab_disabled: bool,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl GamingMode {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(self.state <= 2, "Keyboard gaming mode is out of range");
        for field in ["isAltSpaceDisabled", "isAltEnterDisabled"] {
            ensure!(
                self.extensions.get(field).is_none_or(Value::is_boolean),
                "Keyboard gaming mode extension has the wrong type"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapTap {
    pub is_enabled: bool,
    pub key_list: Vec<SnapTapPair>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl SnapTap {
    pub fn validate(&self) -> anyhow::Result<()> {
        let mut ids = BTreeSet::new();
        for pair in &self.key_list {
            ensure!(
                !pair.key1.is_empty() && !pair.key2.is_empty() && ids.insert(pair.id),
                "Keyboard Snap Tap key identity is empty or its pair is repeated"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SnapTapPair {
    pub key1: String,
    pub key2: String,
    pub id: u32,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub mode: Option<SnapTapMode>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SnapTapMode {
    LastInput,
    PrioLeft,
    PrioRight,
    Neutral,
    CompareLevel,
}

/// Output-specific settings are the vendor mapping compiler's extension
/// boundary. Identity and analog stages are validated before that boundary.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mapping {
    #[serde(rename = "inputID")]
    pub input_id: String,
    pub input_type: String,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_type: Option<String>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_hyper_shift: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub mapping: Option<Vec<MappingStage>>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl Mapping {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            !self.input_id.is_empty() && !self.input_type.is_empty(),
            "Keyboard mapping input identity is absent"
        );
        if self.input_type == "AnalogInput" {
            let stages = self
                .mapping
                .as_ref()
                .context("Keyboard analog mapping stages are absent")?;
            ensure!(!stages.is_empty(), "Keyboard analog mapping has no stages");
            ensure!(
                stages.iter().all(|stage| !stage.output_type.is_empty()),
                "Keyboard analog mapping output identity is absent"
            );
        } else {
            ensure!(
                self.output_type
                    .as_ref()
                    .is_some_and(|output| !output.is_empty()),
                "Keyboard mapping output identity is absent"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingStage {
    pub output_type: String,
    #[serde(flatten)]
    pub extensions: Extensions,
}

/// Exact main26061 DEFAULTPROFILE values. This is local configuration data,
/// never a fallback for failed device reads or incomplete profile documents.
pub fn initial_settings() -> ProfileSettings {
    ProfileSettings {
        actuation_point: ActuationPoint::Named {
            make: 1.2,
            release: 1.2,
            extensions: Extensions::new(),
        },
        switch_off_lighting: LightingOff {
            is_display_on: false,
            is_idle_enabled: false,
            idle_minutes: 1,
            extensions: Extensions::new(),
        },
        brightness: Brightness {
            is_enabled: true,
            value: 100,
            extensions: Extensions::new(),
        },
        quick_effects: QuickEffects {
            selected_effect_id: 3,
            extensions: Extensions::new(),
        },
        gaming_mode: GamingMode {
            state: 0,
            is_windows_key_disabled: false,
            is_alt_f4_disabled: false,
            is_alt_tab_disabled: false,
            extensions: Extensions::new(),
        },
        snap_tap: SnapTap {
            is_enabled: false,
            key_list: vec![SnapTapPair {
                key1: "KEY_A".into(),
                key2: "KEY_D".into(),
                id: 1,
                mode: None,
                extensions: Extensions::new(),
            }],
            extensions: Extensions::new(),
        },
        continuous_rapid_trigger: false,
        r#macro: Extensions::new(),
        mappings: Vec::new(),
        extensions: Extensions::new(),
    }
}

/// Final679 qn/createPreset factory profile after an absent storage key is
/// established. Later per-key preset actuation values do not replace this
/// profile's global 1.2/1.2 settings.
pub fn initial_profile() -> Profile {
    Profile {
        guid: FACTORY_GUID.into(),
        name: FACTORY_NAME.into(),
        settings: initial_settings(),
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalogData {
    pub default_make_actuation: u16,
    pub default_break_actuation: u16,
    pub key_id_to_key_input: BTreeMap<u8, AnalogInput>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct AnalogInput {
    pub r#type: String,
    pub key: u8,
    pub flag: u8,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub enum ReportKind {
    #[serde(rename = "analogKeyboardV2ReportID")]
    AnalogKeyboardV2,
    #[serde(rename = "razerKeyReportID")]
    RazerKey,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct AnalogReportIds {
    #[serde(rename = "11")]
    pub analog_keyboard_v2: ReportKind,
    #[serde(rename = "4")]
    pub razer_key: ReportKind,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl AnalogReportIds {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.analog_keyboard_v2 == ReportKind::AnalogKeyboardV2
                && self.razer_key == ReportKind::RazerKey,
            "Keyboard report metadata differs from its source kinds"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSettings {
    pub analog_data: AnalogData,
    #[serde(rename = "reportIDs")]
    pub report_ids: AnalogReportIds,
    pub selected_profile: Extensions,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl DeviceSettings {
    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.analog_data.default_make_actuation > 0
                && self.analog_data.default_break_actuation > 0,
            "Keyboard analog or report metadata is incomplete"
        );
        self.report_ids.validate()?;
        Ok(())
    }
}

pub fn decode_device_settings(value: &Value) -> anyhow::Result<DeviceSettings> {
    let settings: DeviceSettings = serde_json::from_value(value.clone())
        .context("Keyboard source device settings are incomplete or malformed")?;
    settings.validate()?;
    Ok(settings)
}

pub fn initial_device_settings() -> DeviceSettings {
    DeviceSettings {
        analog_data: AnalogData {
            default_make_actuation: 19656,
            default_break_actuation: 18018,
            key_id_to_key_input: BTreeMap::from([(
                59,
                AnalogInput {
                    r#type: "razerKey".into(),
                    key: 1,
                    flag: 0,
                    extensions: Extensions::new(),
                },
            )]),
            extensions: Extensions::new(),
        },
        report_ids: AnalogReportIds {
            analog_keyboard_v2: ReportKind::AnalogKeyboardV2,
            razer_key: ReportKind::RazerKey,
            extensions: Extensions::new(),
        },
        selected_profile: Extensions::new(),
        extensions: Extensions::new(),
    }
}

#[cfg(test)]
#[path = "keyboard_current_profile_tests.rs"]
mod tests;

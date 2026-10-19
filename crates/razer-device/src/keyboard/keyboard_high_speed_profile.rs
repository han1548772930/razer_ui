//! Current high-speed keyboard profile extensions and source device metadata.
//! Required fields follow the mounted product branch; no decoder supplies
//! initialization values for incomplete local data.
use super::{AnalogReportIds, Extensions, Profile, ReportKind, decode_profile};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HighSpeedSettings {
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub mod_tap: Option<bool>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub polling_rate: Option<f64>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub polling_rate_wireless: Option<f64>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub bottom_deadzone: Option<BottomDeadzone>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub calibrated_keys: Option<Vec<CalibratedKey>>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_enable_keyboard_keys: Option<bool>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_gamepad_override: Option<bool>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub single_key_snap_tap: Option<SingleKeySnapTap>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub interrupt_key: Option<InterruptKey>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub dual_keypress_priority: Option<DualKeypressPriority>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BottomDeadzone {
    pub value: BottomDeadzoneValue,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BottomDeadzoneValue {
    Responsive,
    Medium,
    Stable,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibratedKey {
    #[serde(rename = "inputID")]
    pub input_id: String,
    pub input_type: String,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleKeySnapTap {
    pub is_enabled: bool,
    pub key_list: Vec<SingleKeyPair>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SingleKeyPair {
    pub key_map_make: String,
    pub key_map_break: String,
    pub id: u32,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterruptKey {
    pub is_enabled: bool,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub interruption_key: Option<InterruptionKey>,
    pub interrupts: Vec<String>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InterruptionKey {
    Any,
    Dedicated,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DualKeypressPriority {
    pub is_enabled: bool,
    pub keys: Vec<String>,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl HighSpeedSettings {
    pub fn validate(&self, product_id: u32) -> anyhow::Result<()> {
        ensure!(
            product_id != 688 || self.mod_tap.is_some(),
            "Mini keyboard profile is missing Mod Tap"
        );
        if matches!(product_id, 719 | 740 | 746 | 756) {
            ensure!(
                self.polling_rate.is_some()
                    && self.bottom_deadzone.is_some()
                    && self.calibrated_keys.is_some()
                    && self.is_enable_keyboard_keys.is_some()
                    && self.is_gamepad_override.is_some()
                    && self.single_key_snap_tap.is_some()
                    && self.interrupt_key.is_some()
                    && self.dual_keypress_priority.is_some(),
                "High-speed keyboard profile is missing required settings"
            );
        }
        ensure!(
            product_id != 756 || self.polling_rate_wireless.is_some(),
            "Wireless keyboard profile is missing its polling rate"
        );
        for rate in [self.polling_rate, self.polling_rate_wireless]
            .into_iter()
            .flatten()
        {
            ensure!(
                [125., 250., 500., 1000., 2000., 4000., 8000.].contains(&rate),
                "Keyboard polling rate is unknown"
            );
        }
        if let Some(keys) = &self.calibrated_keys {
            ensure!(
                keys.iter()
                    .all(|key| !key.input_id.is_empty() && !key.input_type.is_empty()),
                "Keyboard calibration identity is empty"
            );
        }
        if let Some(settings) = &self.single_key_snap_tap {
            ensure!(
                settings
                    .key_list
                    .iter()
                    .all(|pair| !pair.key_map_make.is_empty() && !pair.key_map_break.is_empty()),
                "Keyboard single-key Snap Tap identity is empty"
            );
        }
        if let Some(settings) = &self.interrupt_key {
            ensure!(
                settings.interrupts.iter().all(|key| !key.is_empty()),
                "Keyboard interruption identity is empty"
            );
        }
        if let Some(settings) = &self.dual_keypress_priority {
            ensure!(
                settings.keys.iter().all(|key| !key.is_empty()),
                "Keyboard dual-key priority identity is empty"
            );
        }
        Ok(())
    }
}

pub fn validate_profile_settings(
    product_id: u32,
    settings: &super::ProfileSettings,
) -> anyhow::Result<()> {
    settings.validate()?;
    let high_speed: HighSpeedSettings =
        serde_json::from_value(serde_json::to_value(&settings.extensions)?)
            .context("Product-specific keyboard profile settings are malformed")?;
    high_speed.validate(product_id)?;
    ensure!(
        !matches!(product_id, 719 | 740 | 746 | 756)
            || settings
                .snap_tap
                .key_list
                .iter()
                .all(|pair| pair.mode.is_some()),
        "High-speed keyboard Snap Tap priority mode is absent"
    );
    Ok(())
}

pub fn decode_profile_for_product(product_id: u32, value: &Value) -> anyhow::Result<Profile> {
    let profile = decode_profile(value)?;
    validate_profile_settings(product_id, &profile.settings)?;
    Ok(profile)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalogData {
    pub default_make_actuation: u16,
    pub default_break_actuation: u16,
    #[serde(flatten)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSettings {
    pub analog_data: AnalogData,
    pub turn_off_analog_to_key: bool,
    pub game_mode_enabled: bool,
    #[serde(rename = "reportIDs")]
    pub report_ids: AnalogReportIds,
    pub selected_profile: Extensions,
    #[serde(flatten)]
    pub extensions: Extensions,
}

impl DeviceSettings {
    pub const FIELDS: &'static [&'static str] = &[
        "analogData",
        "turnOffAnalogToKey",
        "gameModeEnabled",
        "reportIDs",
        "selectedProfile",
    ];

    pub fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            self.analog_data.default_make_actuation > 0
                && self.analog_data.default_break_actuation > 0,
            "High-speed keyboard analog or report metadata is incomplete"
        );
        self.report_ids.validate()?;
        Ok(())
    }
}

pub fn decode_device_settings(value: &Value) -> anyhow::Result<DeviceSettings> {
    let settings: DeviceSettings = serde_json::from_value(value.clone())
        .context("High-speed keyboard source device settings are incomplete or malformed")?;
    settings.validate()?;
    Ok(settings)
}

/// Exact740/746/756 main26061 analog/report metadata for explicit source
/// initialization. This constructor is never used by compilation.
pub fn initial_device_settings() -> DeviceSettings {
    DeviceSettings {
        analog_data: AnalogData {
            default_make_actuation: 19656,
            default_break_actuation: 1638,
            extensions: Extensions::new(),
        },
        turn_off_analog_to_key: true,
        game_mode_enabled: false,
        report_ids: AnalogReportIds {
            analog_keyboard_v2: ReportKind::AnalogKeyboardV2,
            razer_key: ReportKind::RazerKey,
            extensions: Extensions::new(),
        },
        selected_profile: Extensions::new(),
        extensions: Extensions::new(),
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerTimeout {
    pub is_enabled: bool,
    pub value: f64,
    #[serde(flatten)]
    pub extensions: Extensions,
}

/// Current756 main21240 h_ is a numeric Disable/Enable enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum LowPowerMode {
    Disable,
    Enable,
}

impl TryFrom<u8> for LowPowerMode {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Disable),
            1 => Ok(Self::Enable),
            _ => Err("Unknown keyboard low-power mode"),
        }
    }
}
impl From<LowPowerMode> for u8 {
    fn from(value: LowPowerMode) -> u8 {
        match value {
            LowPowerMode::Disable => 0,
            LowPowerMode::Enable => 1,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestDeviceSettings {
    #[serde(flatten)]
    pub keyboard: DeviceSettings,
    pub dim_lighting: PowerTimeout,
    pub power_saving: PowerTimeout,
    pub low_power_mode: LowPowerMode,
}

impl LatestDeviceSettings {
    pub const FIELDS: &'static [&'static str] = &[
        "analogData",
        "turnOffAnalogToKey",
        "gameModeEnabled",
        "reportIDs",
        "selectedProfile",
        "dimLighting",
        "powerSaving",
        "lowPowerMode",
    ];

    pub fn validate(&self) -> anyhow::Result<()> {
        self.keyboard.validate()?;
        ensure!(
            [&self.dim_lighting, &self.power_saving]
                .iter()
                .all(|timeout| timeout.value.is_finite()
                    && timeout.value >= 0.
                    && timeout.value * 60. <= f64::from(u16::MAX)),
            "Keyboard power timeout is out of range"
        );
        Ok(())
    }
}

pub fn decode_latest_device_settings(value: &Value) -> anyhow::Result<LatestDeviceSettings> {
    let settings: LatestDeviceSettings = serde_json::from_value(value.clone())
        .context("Wireless keyboard source device settings are incomplete or malformed")?;
    settings.validate()?;
    Ok(settings)
}

pub fn initial_latest_device_settings() -> LatestDeviceSettings {
    LatestDeviceSettings {
        keyboard: initial_device_settings(),
        dim_lighting: PowerTimeout {
            is_enabled: true,
            value: 5.,
            extensions: Extensions::new(),
        },
        power_saving: PowerTimeout {
            is_enabled: true,
            value: 15.,
            extensions: Extensions::new(),
        },
        low_power_mode: LowPowerMode::Disable,
    }
}

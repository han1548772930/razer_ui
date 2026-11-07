//! Complete local keyboard profile fields. Template values and saved documents
//! remain distinct from current device observations and user drafts.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[path = "keyboard_local_profile_templates.rs"]
mod templates;

struct RecipeIds(Vec<String>);

impl RecipeIds {
    fn fresh(product: u32) -> Self {
        Self(
            (0..templates::dial_guid_count(product))
                .map(|_| uuid::Uuid::new_v4().to_string())
                .collect(),
        )
    }

    fn get(&self, index: usize) -> &str {
        &self.0[index]
    }
}

/// Complete local template output before the source setup/preset admission.
/// Constructing this value performs no persistence or device initialization.
#[derive(Clone, Debug)]
pub struct Recipe {
    pub device: DeviceFields,
    pub profiles: Vec<Profile>,
}

pub fn schema_version(product: u32) -> Option<u32> {
    templates::schema_version(product)
}

pub fn category(product: u32) -> Option<Category> {
    templates::category(product)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Category {
    Keyboard,
    Keypad,
}

impl Default for Category {
    fn default() -> Self { Self::Keyboard }
}

pub fn initial_template(product: u32, edition: u8, computer_name: &str) -> anyhow::Result<Recipe> {
    let ids = RecipeIds::fresh(product);
    let device = templates::device(product, &ids)
        .ok_or_else(|| anyhow::anyhow!("No current keyboard local document recipe"))?;
    let mut profiles = if product == 708 {
        templates::applications()
    } else {
        let mut profile = templates::profile(product, edition, &ids)
            .ok_or_else(|| anyhow::anyhow!("Keyboard edition has no source profile recipe"))?;
        profile.name = format!("{computer_name}-Default");
        vec![profile]
    };
    for profile in &mut profiles {
        profile.guid = uuid::Uuid::new_v4().to_string();
    }
    Ok(Recipe { device, profiles })
}

pub type EmptyObject = BTreeMap<String, serde_json::Value>;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SourceNumber {
    Unsigned(u64),
    Signed(i64),
    Decimal(f64),
    Text(String),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ActuationValue {
    Named(ActuationFields),
    Ordered([SourceNumber; 2]),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ActuationFields {
    #[serde(rename = "0")]
    pub make: SourceNumber,
    #[serde(rename = "1")]
    pub release: SourceNumber,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub guid: String,
    pub name: String,
    pub mappings: Vec<super::Mapping>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub short_name: Option<String>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub switch_off_lighting: Option<super::LightingOff>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub brightness: Option<super::Brightness>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub quick_effects: Option<super::QuickEffects>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub gaming_mode: Option<super::GamingMode>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub snap_tap: Option<super::SnapTap>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub r#macro: Option<MacroSettings>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub direction_mode: Option<u8>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub keymaps: Option<Vec<Keymap>>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub active_keymap_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub led_power_settings: Option<LedPowerSettings>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub sleep_mode_in_seconds: Option<u32>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub low_power_mode: Option<u32>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub actuation_point: Option<ActuationValue>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub key_switch_mode: Option<u8>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub dim_lighting: Option<PowerTimeout>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub function_key_primary: Option<String>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub command_dial: Option<CommandDial>,
    #[serde(rename = "inActiveQuickEffect")]
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub in_active_quick_effect: Option<InactiveEffect>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub continuous_rapid_trigger: Option<bool>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub debounce_mode: Option<u8>,
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
    pub polling_rate: Option<u32>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub polling_rate_wireless: Option<u32>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub bottom_deadzone: Option<super::high_speed::BottomDeadzone>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub calibrated_keys: Option<Vec<super::high_speed::CalibratedKey>>,
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
    pub single_key_snap_tap: Option<super::high_speed::SingleKeySnapTap>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub interrupt_key: Option<super::high_speed::InterruptKey>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub dual_keypress_priority: Option<super::high_speed::DualKeypressPriority>,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFields {
    pub version: u64,
    pub vendor_id: u32,
    pub product_id: u32,
    pub category: Category,
    #[serde(rename = "reportIDs")]
    pub report_ids: BTreeMap<u8, super::ReportKind>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub selected_profile: Option<EmptyObject>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub dim_lighting: Option<PowerTimeout>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub power_saving_value: Option<u32>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub analog_data: Option<AnalogDefaults>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub power_saving: Option<PowerTimeout>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_compose_f4: Option<bool>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub typing_mode_quick_toggle: Option<u8>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub oled_settings: Option<OledSettings>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub low_power_mode: Option<u32>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub auto_switch: Option<bool>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub switch_off_lighting: Option<super::LightingOff>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub brightness: Option<super::Brightness>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub quick_effects: Option<super::QuickEffects>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub gaming_mode: Option<super::GamingMode>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub sleep_mode_in_seconds: Option<u32>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub function_key_primary: Option<String>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub command_dial: Option<CommandDial>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub turn_off_analog_to_key: Option<bool>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub game_mode_enabled: Option<bool>,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacroSettings {
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub mouse_move_type: Option<String>,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Keymap {
    pub name: String,
    pub guid: String,
    pub mappings: Vec<super::Mapping>,
    pub slot: String,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub dpad_mode: Option<u8>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub thumbstick_output: Option<String>,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub thumbstick_orientation: Option<String>,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedPowerSettings {
    pub idle_state: u8,
    pub idle_value: u32,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerTimeout {
    pub is_enabled: bool,
    pub value: u32,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalogDefaults {
    pub default_make_actuation: u16,
    pub default_break_actuation: u16,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub key_id_to_key_input: Option<BTreeMap<u8, super::AnalogInput>>,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InactiveEffect {
    #[serde(rename = "inActiveEffectId")]
    pub in_active_effect_id: u32,
    #[serde(rename = "inActiveEffectSetting")]
    pub in_active_effect_setting: super::EffectSetting,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandDial {
    #[serde(rename = "currentModeUID")]
    pub current_mode_uid: String,
    pub modes: Vec<DialMode>,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DialMode {
    pub uid: String,
    pub is_custom: bool,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub color: Option<String>,
    pub name: String,
    pub is_enabled: bool,
    pub mappings: Vec<super::Mapping>,
    pub mode_id: u8,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeymapAssignment {
    pub r#type: String,
    pub is_clutch: bool,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalogIdentity {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerAssignment {
    pub controller_mode: ControllerMode,
    pub controller_assignment: String,
    pub analog_sensitivity_type: String,
    pub analog_sensitivity_list: SensitivityCurve,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerMode {
    pub is_analog: bool,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitivityCurve {
    pub analog_sensitivity_assignment: Vec<SensitivityPoint>,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitivityPoint {
    pub x: SourceNumber,
    pub y: SourceNumber,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoystickAssignment {
    pub joystick_mode: String,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub joystick_button_assignment: Option<String>,
    pub is_joystick_movement: bool,
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub joystick_axis_assignment: Option<JoystickAxes>,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoystickAxes {
    #[serde(rename = "X")]
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub x: Option<i16>,
    #[serde(rename = "Y")]
    #[serde(
        default,
        deserialize_with = "super::present",
        skip_serializing_if = "Option::is_none"
    )]
    pub y: Option<i16>,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapidTrigger {
    pub is_enable_up_stroke: bool,
    pub make_sensitivity: u32,
    pub break_sensitivity: u32,
    pub continuous: bool,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BacklightAssignment {
    pub backlight_assignment: String,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsShortcutAssignment {
    pub windows_shortcut_assignment: String,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultimediaAssignment {
    pub multimedia_assignment: String,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MouseAssignment {
    pub mouse_assignment: String,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledLightAssignment {
    pub oled_light_assignment: String,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledSettings {
    pub brightness: u8,
    pub oled_language: u8,
    pub time_to_home_screen_seconds: u32,
    pub time_to_dim_minutes: u32,
    pub oled_dim_display: EnabledValue,
    pub oled_low_battery_warning_display: EnabledValue,
    pub oled_screensaver: EnabledValue,
    pub home_screen_settings: OledHomeScreen,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnabledValue {
    pub enabled: bool,
    pub value: u32,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledHomeScreen {
    pub enabled: bool,
    pub r#type: u8,
    pub animation: OledMediaList,
    pub image: OledMediaList,
    pub banner: OledBanner,
    pub device_info: EmptyObject,
    pub emote: OledEmote,
    pub audio_display: OledAudioDisplay,
    pub system_info: OledSystemInfo,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledMediaList {
    pub selected_idx: u8,
    pub list: Vec<OledMedia>,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledMedia {
    pub src: String,
    pub is_custom: bool,
    pub is_enabled: bool,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledBanner {
    pub image_state: u8,
    pub image_index: u8,
    pub image_pos: u8,
    pub scroll_dir: u8,
    pub guid: String,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledEmote {
    pub id: String,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledAudioDisplay {
    pub visualizer_state: u8,
    pub visualizer_type: u8,
    pub media_state: u8,
    pub position: u8,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledSystemInfo {
    pub slides: Vec<OledSystemSlide>,
    pub interval: u32,
    pub temp_format: u8,
    pub date_format: u8,
    pub time_format: u8,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledSystemSlide {
    pub left: OledSystemField,
    pub right: OledSystemField,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OledSystemField {
    pub id: String,
    pub label: String,
    pub show_label: bool,
    pub r#type: u8,
    #[serde(flatten)]
    pub extensions: super::Extensions,
}

#[cfg(test)]
#[path = "keyboard_local_profile_tests.rs"]
mod tests;

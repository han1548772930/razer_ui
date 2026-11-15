//! Current OBM-to-profile source values. These are comparison and local
//! construction inputs, never substitutes for device observations.
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum InputType {
    KeyInput,
    DKMInput,
    AnalogInput,
    #[serde(rename = "\"AnalogInput\"")]
    QuotedAnalogInput,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SourceFlag {
    Boolean(bool),
    Text(String),
}
impl SourceFlag {
    pub fn enabled(&self) -> bool {
        match self {
            Self::Boolean(value) => *value,
            Self::Text(value) => !value.is_empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputType {
    KeyboardGroup,
    RazerKeyGroup,
    PowerKeyGroup,
    ProfileNavigationGroup,
    MultimediaGroup,
    Win8ShortcutsGroup,
    HyperShiftGroup,
    DisableGroup,
    AiLauncherGroup,
    DefaultGroup,
    MouseGroup,
    SensitivityGroup,
    MacroGroup,
    ControllerGroup,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HidInput {
    #[serde(rename = "pageID", skip_serializing_if = "Option::is_none")]
    pub page_id: Option<String>,
    #[serde(rename = "HID", skip_serializing_if = "Option::is_none")]
    pub hid: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Key {
    Hid(KeyMetadata),
    Named(String),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KeyMetadata {
    #[serde(rename = "HID")]
    pub hid: String,
    pub flag: u32,
    #[serde(rename = "pageID")]
    pub page_id: String,
    #[serde(rename = "inputID", skip_serializing_if = "Option::is_none")]
    pub input_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Turbo {
    pub is_turbo: bool,
    pub keys_per_second: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Modifiers {
    Names(Vec<String>),
    Text(String),
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Keyboard {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Key>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifiers: Option<Modifiers>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extended_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_group: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_turbo: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turbo_mode: Option<Turbo>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Razer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub razer_key_assignment: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Power {
    pub power_key_assignment: String,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileNavigation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_navigation_assignment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<u8>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Media {
    pub multimedia_assignment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_turbo: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Shortcut {
    pub windows_shortcut_assignment: String,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Launcher {
    pub assignment: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mapping {
    #[serde(rename = "inputID")]
    pub input_id: String,
    pub input_type: InputType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_hyper_shift: Option<SourceFlag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_input: Option<HidInput>,
    #[serde(rename = "KeyInput", skip_serializing_if = "Option::is_none")]
    pub capital_key_input: Option<HidInput>,
    #[serde(rename = "AnalogInput", skip_serializing_if = "Option::is_none")]
    pub analog_input: Option<HidInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dkm_input: Option<String>,
    #[serde(rename = "fwID", skip_serializing_if = "Option::is_none")]
    pub firmware_id: Option<u8>,
    #[serde(flatten)]
    pub output: Output,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping: Option<Vec<Stage>>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Output {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_type: Option<OutputType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard_group: Option<Keyboard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub razer_key_group: Option<Razer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_key_group: Option<Power>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_navigation_group: Option<ProfileNavigation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multimedia_group: Option<Media>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub win8_shortcuts_group: Option<Shortcut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_launcher_group: Option<Launcher>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouse_group: Option<Mouse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity_group: Option<Sensitivity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub macro_group: Option<Macro>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_group: Option<Controller>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Controller {
    pub controller_assignment: String,
    pub controller_mode: ControllerMode,
    pub analog_sensitivity_type: String,
    pub analog_sensitivity_list: SensitivityCurve,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControllerMode {
    pub is_analog: bool,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SensitivityCurve {
    pub analog_sensitivity_assignment: Vec<SensitivityPoint>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SensitivityPoint {
    pub x: f64,
    pub y: u16,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Mouse {
    pub mouse_assignment: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turbo_mode: Option<Turbo>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sensitivity {
    pub sensitivity_assignment: String,
    pub independent: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpi_x: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpi_y: Option<u16>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Macro {
    pub guid: String,
    pub macro_playback_option: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_count: Option<u8>,
    pub name: String,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Stage {
    #[serde(flatten)]
    pub output: Output,
    pub actuation_point: BTreeMap<u8, u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rapid_trigger: Option<UiRapidTrigger>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiRapidTrigger {
    pub is_enable_up_stroke: bool,
    pub make_sensitivity: f64,
    pub break_sensitivity: f64,
    pub continuous: bool,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Actuation {
    pub threshold_h: u16,
    pub threshold_l: u16,
    pub ui_high: f64,
    pub ui_low: f64,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RapidTrigger {
    pub enabled: bool,
    pub ui_make: f64,
    pub ui_break: f64,
    pub down_sensitivity: u16,
    pub up_sensitivity: u16,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActuationDefaults {
    pub specific: BTreeMap<String, Actuation>,
    pub rest: Actuation,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RapidTriggerDefaults {
    pub specific: BTreeMap<String, RapidTrigger>,
    pub rest: RapidTrigger,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalogDefaults {
    pub factory_preset_guid: Option<String>,
    pub default_actuation: ActuationDefaults,
    pub default_rapid_trigger: RapidTriggerDefaults,
    pub default_preset_actuation: Vec<(String, ActuationDefaults)>,
    pub default_preset_rapid_trigger: Vec<(String, RapidTriggerDefaults)>,
    pub inject_mapping: Vec<(String, Vec<Mapping>)>,
}

#[path = "keyboard_onboard_import_defaults.rs"]
mod generated;

pub fn mappings(
    product: u32,
    layout: u32,
) -> anyhow::Result<impl Iterator<Item = &'static Mapping>> {
    static ROWS: OnceLock<Vec<Mapping>> = OnceLock::new();
    let indexes = generated::mapping_indexes(product, layout)
        .ok_or_else(|| anyhow::anyhow!("Keyboard import source layout is absent"))?;
    let rows = ROWS.get_or_init(generated::mappings);
    Ok(indexes.iter().map(move |index| &rows[*index as usize]))
}
pub fn analog(product: u32) -> Option<&'static AnalogDefaults> {
    static VALUES: OnceLock<Vec<AnalogDefaults>> = OnceLock::new();
    generated::analog_index(product).map(|index| &VALUES.get_or_init(generated::analog)[index])
}

pub fn dkm_inputs(product: u32) -> &'static [&'static str] {
    generated::dkm_inputs(product)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_current_products_retain_layout_rows_and_dkm_identities() {
        let products = [
            550, 551, 552, 577, 579, 590, 599, 600, 602, 614, 617, 619, 620, 642, 647, 653, 658,
            661, 664, 677, 678, 679, 688, 691, 697, 708, 716, 717, 719, 720, 721, 724, 727, 728,
            739, 740, 741, 742, 746, 747, 752, 756,
        ];
        for product in products {
            for layout in [0, 3, 4, 6, 7, 10, 12, 15, 16, 17, 18] {
                let rows = mappings(product, layout).unwrap().collect::<Vec<_>>();
                assert!(!rows.is_empty());
                assert!(rows.iter().all(|row| !row.input_id.is_empty()));
            }
        }
        let keypad = mappings(747, 0)
            .unwrap()
            .find(|row| row.input_id == "KEY_FN")
            .unwrap();
        assert_eq!(keypad.input_type, InputType::DKMInput);
        assert_eq!(keypad.output.output_type, Some(OutputType::HyperShiftGroup));
        assert!(dkm_inputs(747).contains(&"KEY_FN"));
        assert!(
            mappings(756, 0)
                .unwrap()
                .any(|row| row.input_id == "KEY_BACKSLASH" && row.output.output_type.is_none())
        );
    }

    #[test]
    fn analog_presets_keep_keyboard_and_keypad_defaults_separate() {
        let shared = analog(679).unwrap();
        assert_eq!(shared, analog(740).unwrap());
        assert_eq!(shared.default_actuation.specific.len(), 7);
        assert!(analog(721).unwrap().default_actuation.specific.is_empty());
        assert_eq!(analog(721), analog(756));
        assert_eq!(analog(747).unwrap().default_preset_actuation.len(), 8);
        assert_eq!(
            analog(747).unwrap().default_actuation.rest.threshold_l,
            32760
        );
        assert!(analog(739).is_none());
    }

    #[test]
    fn source_representation_is_not_coerced_during_serialization() {
        let rows = generated::mappings();
        let text_flag = rows
            .iter()
            .find(|row| matches!(row.is_hyper_shift, Some(SourceFlag::Text(_))))
            .unwrap();
        assert_eq!(
            serde_json::to_value(text_flag).unwrap()["isHyperShift"],
            "true"
        );
        let text_modifiers = rows
            .iter()
            .find(|row| {
                matches!(
                    row.output
                        .keyboard_group
                        .as_ref()
                        .and_then(|group| group.modifiers.as_ref()),
                    Some(Modifiers::Text(_))
                )
            })
            .unwrap();
        assert_eq!(
            serde_json::to_value(text_modifiers).unwrap()["keyboardGroup"]["modifiers"],
            "Extended_Key"
        );
    }
}

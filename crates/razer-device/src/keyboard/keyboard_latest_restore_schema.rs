// Explicit domain schema for source-derived fixed data.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileSwitchofflighting {
    #[serde(rename = "isDisplayOn")]
    pub(super) is_display_on: bool,
    #[serde(rename = "isIdleEnabled")]
    pub(super) is_idle_enabled: bool,
    #[serde(rename = "idleMinutes")]
    pub(super) idle_minutes: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileBrightness {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileQuickeffectsSelectedeffectsetting {
    #[serde(rename = "direction")]
    pub(super) direction: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileQuickeffects {
    #[serde(rename = "selectedEffectId")]
    pub(super) selected_effect_id: u64,
    #[serde(rename = "selectedEffectSetting")]
    pub(super) selected_effect_setting:
        LatestKeyboardSourceDefaultProfileQuickeffectsSelectedeffectsetting,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileGamingmode {
    #[serde(rename = "state")]
    pub(super) state: u64,
    #[serde(rename = "isWindowsKeyDisabled")]
    pub(super) is_windows_key_disabled: bool,
    #[serde(rename = "isAltTabDisabled")]
    pub(super) is_alt_tab_disabled: bool,
    #[serde(rename = "isAltF4Disabled")]
    pub(super) is_alt_4_disabled: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileSnaptapKeylistitem {
    #[serde(rename = "key1")]
    pub(super) key_1: String,
    #[serde(rename = "key2")]
    pub(super) key_2: String,
    #[serde(rename = "id")]
    pub(super) id: u64,
    #[serde(rename = "mode")]
    pub(super) mode: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileSnaptap {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "keyList")]
    pub(super) key_list: Vec<LatestKeyboardSourceDefaultProfileSnaptapKeylistitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileBottomdeadzone {
    #[serde(rename = "value")]
    pub(super) value: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileSinglekeysnaptap {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "keyList")]
    pub(super) key_list: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileInterruptkey {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "interrupts")]
    pub(super) interrupts: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfileDualkeypresspriority {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "keys")]
    pub(super) keys: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultProfile {
    #[serde(rename = "guid")]
    pub(super) guid: String,
    #[serde(rename = "name")]
    pub(super) name: String,
    #[serde(rename = "actuationPoint")]
    pub(super) actuation_point: std::collections::BTreeMap<String, f64>,
    #[serde(rename = "switchOffLighting")]
    pub(super) switch_off_lighting: LatestKeyboardSourceDefaultProfileSwitchofflighting,
    #[serde(rename = "brightness")]
    pub(super) brightness: LatestKeyboardSourceDefaultProfileBrightness,
    #[serde(rename = "quickEffects")]
    pub(super) quick_effects: LatestKeyboardSourceDefaultProfileQuickeffects,
    #[serde(rename = "pollingRate")]
    pub(super) polling_rate: f64,
    #[serde(rename = "pollingRateWireless")]
    pub(super) polling_rate_wireless: f64,
    #[serde(rename = "gamingMode")]
    pub(super) gaming_mode: LatestKeyboardSourceDefaultProfileGamingmode,
    #[serde(rename = "snapTap")]
    pub(super) snap_tap: LatestKeyboardSourceDefaultProfileSnaptap,
    #[serde(rename = "bottomDeadzone")]
    pub(super) bottom_deadzone: LatestKeyboardSourceDefaultProfileBottomdeadzone,
    #[serde(rename = "continuousRapidTrigger")]
    pub(super) continuous_rapid_trigger: bool,
    #[serde(rename = "macro")]
    pub(super) r#macro: std::collections::BTreeMap<String, String>,
    #[serde(rename = "mappings")]
    pub(super) mappings: Vec<String>,
    #[serde(rename = "calibratedKeys")]
    pub(super) calibrated_keys: Vec<String>,
    #[serde(rename = "isEnableKeyboardKeys")]
    pub(super) is_enable_keyboard_keys: bool,
    #[serde(rename = "isGamepadOverride")]
    pub(super) is_gamepad_override: bool,
    #[serde(rename = "singleKeySnapTap")]
    pub(super) single_key_snap_tap: LatestKeyboardSourceDefaultProfileSinglekeysnaptap,
    #[serde(rename = "interruptKey")]
    pub(super) interrupt_key: LatestKeyboardSourceDefaultProfileInterruptkey,
    #[serde(rename = "dualKeypressPriority")]
    pub(super) dual_keypress_priority: LatestKeyboardSourceDefaultProfileDualkeypresspriority,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultMappingsitemKeyboardgroup {
    #[serde(rename = "key")]
    pub(super) key: String,
    #[serde(
        rename = "keyAssignment",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) key_assignment: Option<String>,
    #[serde(rename = "modifiers", default, skip_serializing_if = "Option::is_none")]
    pub(super) modifiers: Option<Vec<String>>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultMappingsitemMultimediagroup {
    #[serde(rename = "multimediaAssignment")]
    pub(super) multimedia_assignment: String,
    #[serde(rename = "useTurbo", default, skip_serializing_if = "Option::is_none")]
    pub(super) use_turbo: Option<bool>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultMappingsitemSystemgroup {
    #[serde(rename = "systemAssignment")]
    pub(super) system_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultMappingsitemBacklightgroup {
    #[serde(rename = "backlightAssignment")]
    pub(super) backlight_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultMappingsitemProfilenavigationgroup {
    #[serde(rename = "profileNavigationAssignment")]
    pub(super) profile_navigation_assignment: String,
    #[serde(rename = "guid", default, skip_serializing_if = "Option::is_none")]
    pub(super) guid: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultMappingsitemDisablegroup {
    #[serde(rename = "disableAssignment")]
    pub(super) disable_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultMappingsitemSnaptapgroup {
    #[serde(rename = "snapTapAssignment")]
    pub(super) snap_tap_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultMappingsitem {
    #[serde(rename = "outputType")]
    pub(super) output_type: String,
    #[serde(rename = "isHyperShift")]
    pub(super) is_hyper_shift: bool,
    #[serde(rename = "inputType")]
    pub(super) input_type: String,
    #[serde(rename = "inputID")]
    pub(super) input_id: String,
    #[serde(
        rename = "keyboardGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) keyboard_group: Option<LatestKeyboardSourceDefaultMappingsitemKeyboardgroup>,
    #[serde(
        rename = "multimediaGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) multimedia_group: Option<LatestKeyboardSourceDefaultMappingsitemMultimediagroup>,
    #[serde(
        rename = "systemGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) system_group: Option<LatestKeyboardSourceDefaultMappingsitemSystemgroup>,
    #[serde(
        rename = "backlightGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) backlight_group: Option<LatestKeyboardSourceDefaultMappingsitemBacklightgroup>,
    #[serde(
        rename = "profileNavigationGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) profile_navigation_group:
        Option<LatestKeyboardSourceDefaultMappingsitemProfilenavigationgroup>,
    #[serde(
        rename = "disableGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) disable_group: Option<LatestKeyboardSourceDefaultMappingsitemDisablegroup>,
    #[serde(
        rename = "snapTapGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) snap_tap_group: Option<LatestKeyboardSourceDefaultMappingsitemSnaptapgroup>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFullMappingListitemDisablegroup {
    #[serde(rename = "disableAssignment")]
    pub(super) disable_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFullMappingListitem {
    #[serde(rename = "inputID")]
    pub(super) input_id: String,
    #[serde(rename = "isHyperShift")]
    pub(super) is_hyper_shift: bool,
    #[serde(rename = "inputType")]
    pub(super) input_type: String,
    #[serde(rename = "outputType")]
    pub(super) output_type: String,
    #[serde(rename = "disableGroup")]
    pub(super) disable_group: LatestKeyboardSourceFullMappingListitemDisablegroup,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceButtonListitemKeyinput {
    #[serde(rename = "pageID")]
    pub(super) page_id: String,
    #[serde(rename = "HID")]
    pub(super) hid: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceButtonListitemKeyboardgroupKey {
    #[serde(rename = "HID")]
    pub(super) hid: String,
    #[serde(rename = "flag")]
    pub(super) flag: u64,
    #[serde(rename = "pageID")]
    pub(super) page_id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceButtonListitemKeyboardgroup {
    #[serde(rename = "key")]
    pub(super) key: LatestKeyboardSourceButtonListitemKeyboardgroupKey,
    #[serde(rename = "modifiers")]
    pub(super) modifiers: Vec<String>,
    #[serde(
        rename = "extendedKey",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) extended_key: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceButtonListitemRazerkeygroup {
    #[serde(
        rename = "razerKeyAssignment",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) razer_key_assignment: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceButtonListitemProfilenavigationgroup {
    #[serde(rename = "profileNavigationAssignment")]
    pub(super) profile_navigation_assignment: String,
    #[serde(rename = "profileId", default, skip_serializing_if = "Option::is_none")]
    pub(super) profile_id: Option<u64>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceButtonListitemMultimediagroup {
    #[serde(rename = "multimediaAssignment")]
    pub(super) multimedia_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceButtonListitemPowerkeygroup {
    #[serde(rename = "powerKeyAssignment")]
    pub(super) power_key_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceButtonListitem {
    #[serde(rename = "inputID")]
    pub(super) input_id: String,
    #[serde(rename = "inputType")]
    pub(super) input_type: String,
    #[serde(rename = "keyInput")]
    pub(super) key_input: LatestKeyboardSourceButtonListitemKeyinput,
    #[serde(
        rename = "keyboardGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) keyboard_group: Option<LatestKeyboardSourceButtonListitemKeyboardgroup>,
    #[serde(
        rename = "outputType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) output_type: Option<String>,
    #[serde(
        rename = "isHyperShift",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) is_hyper_shift: Option<bool>,
    #[serde(
        rename = "razerKeyGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) razer_key_group: Option<LatestKeyboardSourceButtonListitemRazerkeygroup>,
    #[serde(
        rename = "profileNavigationGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) profile_navigation_group:
        Option<LatestKeyboardSourceButtonListitemProfilenavigationgroup>,
    #[serde(
        rename = "multimediaGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) multimedia_group: Option<LatestKeyboardSourceButtonListitemMultimediagroup>,
    #[serde(
        rename = "powerKeyGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) power_key_group: Option<LatestKeyboardSourceButtonListitemPowerkeygroup>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput {
    #[serde(rename = "pageID")]
    pub(super) page_id: String,
    #[serde(rename = "HID")]
    pub(super) hid: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey {
    #[serde(rename = "HID")]
    pub(super) hid: String,
    #[serde(rename = "flag")]
    pub(super) flag: u64,
    #[serde(rename = "pageID")]
    pub(super) page_id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup {
    #[serde(rename = "key")]
    pub(super) key: LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroupKey,
    #[serde(rename = "modifiers")]
    pub(super) modifiers: Vec<String>,
    #[serde(
        rename = "extendedKey",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) extended_key: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardNormalmodeentryMultimediagroup {
    #[serde(rename = "multimediaAssignment")]
    pub(super) multimedia_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardNormalmodeentryWin8Shortcutsgroup {
    #[serde(rename = "windowsShortcutAssignment")]
    pub(super) windows_shortcut_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardNormalmodeentry {
    #[serde(rename = "fwID", default, skip_serializing_if = "Option::is_none")]
    pub(super) fw_id: Option<u64>,
    #[serde(rename = "inputID")]
    pub(super) input_id: String,
    #[serde(rename = "inputType")]
    pub(super) input_type: String,
    #[serde(rename = "keyInput", default, skip_serializing_if = "Option::is_none")]
    pub(super) key_input: Option<LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyinput>,
    #[serde(
        rename = "keyboardGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) keyboard_group:
        Option<LatestKeyboardSourceFirmwareKeyboardNormalmodeentryKeyboardgroup>,
    #[serde(rename = "outputType")]
    pub(super) output_type: String,
    #[serde(rename = "dkmInput", default, skip_serializing_if = "Option::is_none")]
    pub(super) dkm_input: Option<String>,
    #[serde(
        rename = "multimediaGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) multimedia_group:
        Option<LatestKeyboardSourceFirmwareKeyboardNormalmodeentryMultimediagroup>,
    #[serde(
        rename = "win8ShortcutsGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) win_8_shortcuts_group:
        Option<LatestKeyboardSourceFirmwareKeyboardNormalmodeentryWin8Shortcutsgroup>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput {
    #[serde(rename = "pageID")]
    pub(super) page_id: String,
    #[serde(rename = "HID")]
    pub(super) hid: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey {
    #[serde(rename = "HID")]
    pub(super) hid: String,
    #[serde(rename = "flag")]
    pub(super) flag: u64,
    #[serde(rename = "pageID")]
    pub(super) page_id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup {
    #[serde(rename = "key")]
    pub(super) key: LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroupKey,
    #[serde(rename = "modifiers")]
    pub(super) modifiers: Vec<String>,
    #[serde(
        rename = "extendedKey",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) extended_key: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup {
    #[serde(
        rename = "razerKeyAssignment",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) razer_key_assignment: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryProfilenavigationgroup {
    #[serde(rename = "profileNavigationAssignment")]
    pub(super) profile_navigation_assignment: String,
    #[serde(rename = "profileId", default, skip_serializing_if = "Option::is_none")]
    pub(super) profile_id: Option<u64>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup {
    #[serde(rename = "multimediaAssignment")]
    pub(super) multimedia_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryPowerkeygroup {
    #[serde(rename = "powerKeyAssignment")]
    pub(super) power_key_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryWin8Shortcutsgroup {
    #[serde(rename = "windowsShortcutAssignment")]
    pub(super) windows_shortcut_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry {
    #[serde(rename = "fwID", default, skip_serializing_if = "Option::is_none")]
    pub(super) fw_id: Option<u64>,
    #[serde(rename = "inputID")]
    pub(super) input_id: String,
    #[serde(rename = "inputType")]
    pub(super) input_type: String,
    #[serde(rename = "keyInput", default, skip_serializing_if = "Option::is_none")]
    pub(super) key_input: Option<LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyinput>,
    #[serde(
        rename = "keyboardGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) keyboard_group:
        Option<LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryKeyboardgroup>,
    #[serde(rename = "outputType")]
    pub(super) output_type: String,
    #[serde(rename = "isHyperShift")]
    pub(super) is_hyper_shift: bool,
    #[serde(
        rename = "razerKeyGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) razer_key_group:
        Option<LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryRazerkeygroup>,
    #[serde(
        rename = "profileNavigationGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) profile_navigation_group:
        Option<LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryProfilenavigationgroup>,
    #[serde(
        rename = "multimediaGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) multimedia_group:
        Option<LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryMultimediagroup>,
    #[serde(
        rename = "powerKeyGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) power_key_group:
        Option<LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryPowerkeygroup>,
    #[serde(rename = "dkmInput", default, skip_serializing_if = "Option::is_none")]
    pub(super) dkm_input: Option<String>,
    #[serde(
        rename = "win8ShortcutsGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) win_8_shortcuts_group:
        Option<LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentryWin8Shortcutsgroup>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceFirmwareKeyboard {
    #[serde(rename = "NormalMode")]
    pub(super) normal_mode:
        std::collections::BTreeMap<String, LatestKeyboardSourceFirmwareKeyboardNormalmodeentry>,
    #[serde(rename = "HypershiftMode")]
    pub(super) hypershift_mode:
        std::collections::BTreeMap<String, LatestKeyboardSourceFirmwareKeyboardHypershiftmodeentry>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceProfileSwitchEffect {
    #[serde(rename = "alwaysTriggerSlot1Effect")]
    pub(super) always_trigger_slot_1_effect: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceNavigationKeysitem {
    #[serde(rename = "name")]
    pub(super) name: String,
    #[serde(rename = "inputID", default, skip_serializing_if = "Option::is_none")]
    pub(super) input_id: Option<String>,
    #[serde(rename = "type")]
    pub(super) r#type: String,
    #[serde(rename = "scancode")]
    pub(super) scancode: u64,
    #[serde(rename = "virtualKey")]
    pub(super) virtual_key: String,
    #[serde(rename = "keyCode")]
    pub(super) key_code: String,
    #[serde(rename = "flag")]
    pub(super) flag: u64,
    #[serde(rename = "pageID", default, skip_serializing_if = "Option::is_none")]
    pub(super) page_id: Option<String>,
    #[serde(rename = "HID", default, skip_serializing_if = "Option::is_none")]
    pub(super) hid: Option<String>,
    #[serde(
        rename = "ExtendedKey",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) extended_key: Option<String>,
    #[serde(
        rename = "outputFlag",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) output_flag: Option<u64>,
    #[serde(rename = "modifier", default, skip_serializing_if = "Option::is_none")]
    pub(super) modifier: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePhysicalKeysitem {
    #[serde(rename = "inputID")]
    pub(super) input_id: String,
    #[serde(rename = "fwID")]
    pub(super) fw_id: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetProfilesitem {
    #[serde(rename = "name")]
    pub(super) name: String,
    #[serde(rename = "guid")]
    pub(super) guid: String,
    #[serde(rename = "slotId")]
    pub(super) slot_id: u64,
    #[serde(
        rename = "isFactoryPreset",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) is_factory_preset: Option<bool>,
    #[serde(
        rename = "isDefaultSelectedProfile",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) is_default_selected_profile: Option<bool>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetActuationitempayloadSpecificKeyCapsLock {
    #[serde(rename = "uiLow")]
    pub(super) ui_low: u64,
    #[serde(rename = "uiHigh")]
    pub(super) ui_high: f64,
    #[serde(rename = "thresholdL")]
    pub(super) threshold_l: u64,
    #[serde(rename = "thresholdH")]
    pub(super) threshold_h: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetActuationitempayloadSpecificKeyInsert {
    #[serde(rename = "uiLow")]
    pub(super) ui_low: u64,
    #[serde(rename = "uiHigh")]
    pub(super) ui_high: f64,
    #[serde(rename = "thresholdL")]
    pub(super) threshold_l: u64,
    #[serde(rename = "thresholdH")]
    pub(super) threshold_h: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetActuationitempayloadSpecificKeyDelete {
    #[serde(rename = "uiLow")]
    pub(super) ui_low: u64,
    #[serde(rename = "uiHigh")]
    pub(super) ui_high: f64,
    #[serde(rename = "thresholdL")]
    pub(super) threshold_l: u64,
    #[serde(rename = "thresholdH")]
    pub(super) threshold_h: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageUp {
    #[serde(rename = "uiLow")]
    pub(super) ui_low: u64,
    #[serde(rename = "uiHigh")]
    pub(super) ui_high: f64,
    #[serde(rename = "thresholdL")]
    pub(super) threshold_l: u64,
    #[serde(rename = "thresholdH")]
    pub(super) threshold_h: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetActuationitempayloadSpecificKeyHome {
    #[serde(rename = "uiLow")]
    pub(super) ui_low: u64,
    #[serde(rename = "uiHigh")]
    pub(super) ui_high: f64,
    #[serde(rename = "thresholdL")]
    pub(super) threshold_l: u64,
    #[serde(rename = "thresholdH")]
    pub(super) threshold_h: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetActuationitempayloadSpecificKeyEnd {
    #[serde(rename = "uiLow")]
    pub(super) ui_low: u64,
    #[serde(rename = "uiHigh")]
    pub(super) ui_high: f64,
    #[serde(rename = "thresholdL")]
    pub(super) threshold_l: u64,
    #[serde(rename = "thresholdH")]
    pub(super) threshold_h: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageDown {
    #[serde(rename = "uiLow")]
    pub(super) ui_low: u64,
    #[serde(rename = "uiHigh")]
    pub(super) ui_high: f64,
    #[serde(rename = "thresholdL")]
    pub(super) threshold_l: u64,
    #[serde(rename = "thresholdH")]
    pub(super) threshold_h: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetActuationitempayloadSpecific {
    #[serde(
        rename = "KEY_CAPS_LOCK",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) lock: Option<LatestKeyboardSourcePresetActuationitempayloadSpecificKeyCapsLock>,
    #[serde(
        rename = "KEY_INSERT",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) insert: Option<LatestKeyboardSourcePresetActuationitempayloadSpecificKeyInsert>,
    #[serde(
        rename = "KEY_DELETE",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) delete: Option<LatestKeyboardSourcePresetActuationitempayloadSpecificKeyDelete>,
    #[serde(
        rename = "KEY_PAGE_UP",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) up: Option<LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageUp>,
    #[serde(rename = "KEY_HOME", default, skip_serializing_if = "Option::is_none")]
    pub(super) home: Option<LatestKeyboardSourcePresetActuationitempayloadSpecificKeyHome>,
    #[serde(rename = "KEY_END", default, skip_serializing_if = "Option::is_none")]
    pub(super) end: Option<LatestKeyboardSourcePresetActuationitempayloadSpecificKeyEnd>,
    #[serde(
        rename = "KEY_PAGE_DOWN",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) down: Option<LatestKeyboardSourcePresetActuationitempayloadSpecificKeyPageDown>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum LatestKeyboardSourcePresetActuationitempayloadRestUilowchoice {
    Decimal(f64),
    Integer(u64),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetActuationitempayloadRest {
    #[serde(rename = "thresholdL")]
    pub(super) threshold_l: u64,
    #[serde(rename = "thresholdH")]
    pub(super) threshold_h: u64,
    #[serde(rename = "uiHigh")]
    pub(super) ui_high: f64,
    #[serde(rename = "uiLow")]
    pub(super) ui_low: LatestKeyboardSourcePresetActuationitempayloadRestUilowchoice,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetActuationitempayload {
    #[serde(rename = "specific")]
    pub(super) specific: LatestKeyboardSourcePresetActuationitempayloadSpecific,
    #[serde(rename = "rest")]
    pub(super) rest: LatestKeyboardSourcePresetActuationitempayloadRest,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyA {
    #[serde(rename = "enabled")]
    pub(super) enabled: bool,
    #[serde(rename = "uiMake")]
    pub(super) ui_make: f64,
    #[serde(rename = "uiBreak")]
    pub(super) ui_break: f64,
    #[serde(rename = "downSensitivity")]
    pub(super) down_sensitivity: u64,
    #[serde(rename = "upSensitivity")]
    pub(super) up_sensitivity: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyW {
    #[serde(rename = "enabled")]
    pub(super) enabled: bool,
    #[serde(rename = "uiMake")]
    pub(super) ui_make: f64,
    #[serde(rename = "uiBreak")]
    pub(super) ui_break: f64,
    #[serde(rename = "downSensitivity")]
    pub(super) down_sensitivity: u64,
    #[serde(rename = "upSensitivity")]
    pub(super) up_sensitivity: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyS {
    #[serde(rename = "enabled")]
    pub(super) enabled: bool,
    #[serde(rename = "uiMake")]
    pub(super) ui_make: f64,
    #[serde(rename = "uiBreak")]
    pub(super) ui_break: f64,
    #[serde(rename = "downSensitivity")]
    pub(super) down_sensitivity: u64,
    #[serde(rename = "upSensitivity")]
    pub(super) up_sensitivity: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyD {
    #[serde(rename = "enabled")]
    pub(super) enabled: bool,
    #[serde(rename = "uiMake")]
    pub(super) ui_make: f64,
    #[serde(rename = "uiBreak")]
    pub(super) ui_break: f64,
    #[serde(rename = "downSensitivity")]
    pub(super) down_sensitivity: u64,
    #[serde(rename = "upSensitivity")]
    pub(super) up_sensitivity: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetRapidTriggeritempayloadSpecific {
    #[serde(rename = "KEY_A", default, skip_serializing_if = "Option::is_none")]
    pub(super) a: Option<LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyA>,
    #[serde(rename = "KEY_W", default, skip_serializing_if = "Option::is_none")]
    pub(super) w: Option<LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyW>,
    #[serde(rename = "KEY_S", default, skip_serializing_if = "Option::is_none")]
    pub(super) s: Option<LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyS>,
    #[serde(rename = "KEY_D", default, skip_serializing_if = "Option::is_none")]
    pub(super) d: Option<LatestKeyboardSourcePresetRapidTriggeritempayloadSpecificKeyD>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetRapidTriggeritempayloadRest {
    #[serde(rename = "enabled")]
    pub(super) enabled: bool,
    #[serde(rename = "uiMake")]
    pub(super) ui_make: f64,
    #[serde(rename = "uiBreak")]
    pub(super) ui_break: f64,
    #[serde(rename = "downSensitivity")]
    pub(super) down_sensitivity: u64,
    #[serde(rename = "upSensitivity")]
    pub(super) up_sensitivity: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetRapidTriggeritempayload {
    #[serde(rename = "specific")]
    pub(super) specific: LatestKeyboardSourcePresetRapidTriggeritempayloadSpecific,
    #[serde(rename = "rest")]
    pub(super) rest: LatestKeyboardSourcePresetRapidTriggeritempayloadRest,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupControllermode
{
    #[serde(rename = "isAnalog")]
    pub(super) is_analog: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub(super) enum LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice
{
    Decimal(f64),
    Integer(u64),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem {
    #[serde(rename = "x")]
    pub(super) x: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitemXchoice,
    #[serde(rename = "y")]
    pub(super) y: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylist {
    #[serde(rename = "analogSensitivityAssignment")]
    pub(super) analog_sensitivity_assignment: Vec<LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylistAnalogsensitivityassignmentitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroup {
    #[serde(rename = "controllerAssignment")]
    pub(super) controller_assignment: String,
    #[serde(rename = "controllerMode")]
    pub(super) controller_mode: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupControllermode,
    #[serde(rename = "analogSensitivityType")]
    pub(super) analog_sensitivity_type: String,
    #[serde(rename = "analogSensitivityList")]
    pub(super) analog_sensitivity_list: LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroupAnalogsensitivitylist,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint {
    #[serde(rename = "0")]
    pub(super) field_0: u64,
    #[serde(rename = "1")]
    pub(super) field_1: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemRapidtrigger {
    #[serde(rename = "makeSensitivity")]
    pub(super) make_sensitivity: u64,
    #[serde(rename = "breakSensitivity")]
    pub(super) break_sensitivity: u64,
    #[serde(rename = "isEnableUpStroke")]
    pub(super) is_enable_up_stroke: bool,
    #[serde(rename = "continuous")]
    pub(super) continuous: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem {
    #[serde(rename = "outputType")]
    pub(super) output_type: String,
    #[serde(
        rename = "controllerGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) controller_group:
        Option<LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemControllergroup>,
    #[serde(rename = "actuationPoint")]
    pub(super) actuation_point:
        LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemActuationpoint,
    #[serde(
        rename = "keyboardGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) keyboard_group: Option<std::collections::BTreeMap<String, String>>,
    #[serde(
        rename = "rapidTrigger",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) rapid_trigger:
        Option<LatestKeyboardSourcePresetMappingsitempayloaditemMappingitemRapidtrigger>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourcePresetMappingsitempayloaditem {
    #[serde(rename = "inputType")]
    pub(super) input_type: String,
    #[serde(rename = "isHyperShift")]
    pub(super) is_hyper_shift: bool,
    #[serde(rename = "inputID")]
    pub(super) input_id: String,
    #[serde(rename = "mapping")]
    pub(super) mapping: Vec<LatestKeyboardSourcePresetMappingsitempayloaditemMappingitem>,
    #[serde(rename = "outputType")]
    pub(super) output_type: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceOnboardPresetsitem {
    #[serde(rename = "name")]
    pub(super) name: String,
    #[serde(rename = "guid")]
    pub(super) guid: String,
    #[serde(rename = "slotId")]
    pub(super) slot_id: u64,
    #[serde(
        rename = "isFactoryPreset",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) is_factory_preset: Option<bool>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceProfileTransferTargetPresetprofilesitem {
    #[serde(rename = "name")]
    pub(super) name: String,
    #[serde(rename = "guid")]
    pub(super) guid: String,
    #[serde(rename = "slotId")]
    pub(super) slot_id: u64,
    #[serde(
        rename = "isFactoryPreset",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) is_factory_preset: Option<bool>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceProfileTransferTargetActuationinfo {
    #[serde(rename = "defaultValue")]
    pub(super) default_value: u64,
    #[serde(rename = "max")]
    pub(super) max: u64,
    #[serde(rename = "min")]
    pub(super) min: u64,
    #[serde(rename = "unit")]
    pub(super) unit: u64,
    #[serde(rename = "minBreak")]
    pub(super) min_break: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceProfileTransferTargetActuationhighsensitivityinfo {
    #[serde(rename = "defaultValue")]
    pub(super) default_value: u64,
    #[serde(rename = "max")]
    pub(super) max: u64,
    #[serde(rename = "min")]
    pub(super) min: u64,
    #[serde(rename = "unit")]
    pub(super) unit: u64,
    #[serde(rename = "minBreak")]
    pub(super) min_break: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceProfileTransferTarget {
    #[serde(rename = "productId")]
    pub(super) product_id: u64,
    #[serde(rename = "dongleId")]
    pub(super) dongle_id: u64,
    #[serde(rename = "bleId")]
    pub(super) ble_id: u64,
    #[serde(rename = "deviceName")]
    pub(super) device_name: String,
    #[serde(rename = "category")]
    pub(super) category: String,
    #[serde(rename = "is8kAnalogDevice")]
    pub(super) is_8_k_analog_device: bool,
    #[serde(rename = "presetProfiles")]
    pub(super) preset_profiles: Vec<LatestKeyboardSourceProfileTransferTargetPresetprofilesitem>,
    #[serde(rename = "actuationInfo")]
    pub(super) actuation_info: LatestKeyboardSourceProfileTransferTargetActuationinfo,
    #[serde(rename = "actuationHighSensitivityInfo")]
    pub(super) actuation_high_sensitivity_info:
        LatestKeyboardSourceProfileTransferTargetActuationhighsensitivityinfo,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultActuationRest {
    #[serde(rename = "thresholdH")]
    pub(super) threshold_h: u64,
    #[serde(rename = "thresholdL")]
    pub(super) threshold_l: u64,
    #[serde(rename = "uiHigh")]
    pub(super) ui_high: f64,
    #[serde(rename = "uiLow")]
    pub(super) ui_low: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultActuation {
    #[serde(rename = "specific")]
    pub(super) specific: std::collections::BTreeMap<String, String>,
    #[serde(rename = "rest")]
    pub(super) rest: LatestKeyboardSourceDefaultActuationRest,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultRapidTriggerRest {
    #[serde(rename = "enabled")]
    pub(super) enabled: bool,
    #[serde(rename = "uiMake")]
    pub(super) ui_make: f64,
    #[serde(rename = "uiBreak")]
    pub(super) ui_break: f64,
    #[serde(rename = "downSensitivity")]
    pub(super) down_sensitivity: u64,
    #[serde(rename = "upSensitivity")]
    pub(super) up_sensitivity: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSourceDefaultRapidTrigger {
    #[serde(rename = "specific")]
    pub(super) specific: std::collections::BTreeMap<String, String>,
    #[serde(rename = "rest")]
    pub(super) rest: LatestKeyboardSourceDefaultRapidTriggerRest,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LatestKeyboardSource {
    #[serde(rename = "product_id")]
    pub(super) product_id: u64,
    #[serde(rename = "device_name")]
    pub(super) device_name: String,
    #[serde(rename = "guest_profile_owner")]
    pub(super) guest_profile_owner: String,
    #[serde(rename = "default_profile")]
    pub(super) default_profile: LatestKeyboardSourceDefaultProfile,
    #[serde(rename = "default_mappings")]
    pub(super) default_mappings: Vec<LatestKeyboardSourceDefaultMappingsitem>,
    #[serde(rename = "full_mapping_list")]
    pub(super) full_mapping_list: Vec<LatestKeyboardSourceFullMappingListitem>,
    #[serde(rename = "button_list")]
    pub(super) button_list: Vec<LatestKeyboardSourceButtonListitem>,
    #[serde(rename = "firmware_keyboard")]
    pub(super) firmware_keyboard: LatestKeyboardSourceFirmwareKeyboard,
    #[serde(rename = "profile_switch_effect")]
    pub(super) profile_switch_effect: LatestKeyboardSourceProfileSwitchEffect,
    #[serde(rename = "navigation_keys")]
    pub(super) navigation_keys: Vec<LatestKeyboardSourceNavigationKeysitem>,
    #[serde(rename = "physical_keys")]
    pub(super) physical_keys: Vec<LatestKeyboardSourcePhysicalKeysitem>,
    #[serde(rename = "preset_guids")]
    pub(super) preset_guids: std::collections::BTreeMap<String, String>,
    #[serde(rename = "preset_profiles")]
    pub(super) preset_profiles: Vec<LatestKeyboardSourcePresetProfilesitem>,
    #[serde(rename = "preset_actuation")]
    pub(super) preset_actuation: Vec<(String, LatestKeyboardSourcePresetActuationitempayload)>,
    #[serde(rename = "preset_rapid_trigger")]
    pub(super) preset_rapid_trigger:
        Vec<(String, LatestKeyboardSourcePresetRapidTriggeritempayload)>,
    #[serde(rename = "preset_mappings")]
    pub(super) preset_mappings: Vec<(
        String,
        Vec<LatestKeyboardSourcePresetMappingsitempayloaditem>,
    )>,
    #[serde(rename = "onboard_presets")]
    pub(super) onboard_presets: Vec<LatestKeyboardSourceOnboardPresetsitem>,
    #[serde(rename = "profile_transfer_target")]
    pub(super) profile_transfer_target: LatestKeyboardSourceProfileTransferTarget,
    #[serde(rename = "import_non_analog_ids")]
    pub(super) import_non_analog_ids: Vec<String>,
    #[serde(rename = "import_keymap_passthrough_ids")]
    pub(super) import_keymap_passthrough_ids: Vec<String>,
    #[serde(rename = "import_restricted_layouts")]
    pub(super) import_restricted_layouts: Vec<u64>,
    #[serde(rename = "import_restricted_input_ids")]
    pub(super) import_restricted_input_ids: Vec<String>,
    #[serde(rename = "default_actuation")]
    pub(super) default_actuation: LatestKeyboardSourceDefaultActuation,
    #[serde(rename = "default_rapid_trigger")]
    pub(super) default_rapid_trigger: LatestKeyboardSourceDefaultRapidTrigger,
}

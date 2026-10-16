// Explicit domain schema for source-derived fixed data.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceDefaultmappingsitemMousegroupTurbomode {
    #[serde(rename = "isTurbo")]
    pub(super) is_turbo: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceDefaultmappingsitemMousegroup {
    #[serde(rename = "mouseAssignment")]
    pub(super) mouse_assignment: String,
    #[serde(rename = "turboMode", default, skip_serializing_if = "Option::is_none")]
    pub(super) turbo_mode: Option<LegacyMouseSourceDefaultmappingsitemMousegroupTurbomode>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceDefaultmappingsitemSensitivitygroup {
    #[serde(rename = "sensitivityAssignment")]
    pub(super) sensitivity_assignment: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceDefaultmappingsitem {
    #[serde(rename = "inputID")]
    pub(super) input_id: String,
    #[serde(rename = "inputType")]
    pub(super) input_type: String,
    #[serde(
        rename = "mouseInput",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) mouse_input: Option<String>,
    #[serde(
        rename = "mouseGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) mouse_group: Option<LegacyMouseSourceDefaultmappingsitemMousegroup>,
    #[serde(rename = "outputType")]
    pub(super) output_type: String,
    #[serde(rename = "dkmInput", default, skip_serializing_if = "Option::is_none")]
    pub(super) dkm_input: Option<String>,
    #[serde(
        rename = "sensitivityGroup",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) sensitivity_group: Option<LegacyMouseSourceDefaultmappingsitemSensitivitygroup>,
    #[serde(
        rename = "isHyperShift",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(super) is_hyper_shift: Option<bool>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceDkmkeysitem {
    #[serde(rename = "inputID")]
    pub(super) input_id: String,
    #[serde(rename = "buttonKey", default, skip_serializing_if = "Option::is_none")]
    pub(super) button_key: Option<String>,
    #[serde(rename = "key")]
    pub(super) key: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceProfileDpi {
    #[serde(rename = "x")]
    pub(super) x: u64,
    #[serde(rename = "y")]
    pub(super) y: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceProfileBrightness {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceProfileSwitchofflighting {
    #[serde(rename = "isDisplayOn")]
    pub(super) is_display_on: bool,
    #[serde(rename = "isIdleEnabled")]
    pub(super) is_idle_enabled: bool,
    #[serde(rename = "idleMinutes")]
    pub(super) idle_minutes: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceProfileQuickeffects {
    #[serde(rename = "selectedEffectId")]
    pub(super) selected_effect_id: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceProfileDpistagesStagesitem {
    #[serde(rename = "x")]
    pub(super) x: u64,
    #[serde(rename = "y")]
    pub(super) y: u64,
    #[serde(rename = "independent")]
    pub(super) independent: bool,
    #[serde(rename = "visible")]
    pub(super) visible: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceProfileDpistages {
    #[serde(rename = "stages")]
    pub(super) stages: Vec<LegacyMouseSourceProfileDpistagesStagesitem>,
    #[serde(rename = "active")]
    pub(super) active: u64,
    #[serde(rename = "enable")]
    pub(super) enable: bool,
    #[serde(rename = "count")]
    pub(super) count: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSourceProfile {
    #[serde(rename = "guid")]
    pub(super) guid: String,
    #[serde(rename = "name")]
    pub(super) name: String,
    #[serde(rename = "dpi")]
    pub(super) dpi: LegacyMouseSourceProfileDpi,
    #[serde(rename = "brightness")]
    pub(super) brightness: LegacyMouseSourceProfileBrightness,
    #[serde(rename = "switchOffLighting")]
    pub(super) switch_off_lighting: LegacyMouseSourceProfileSwitchofflighting,
    #[serde(rename = "quickEffects")]
    pub(super) quick_effects: LegacyMouseSourceProfileQuickeffects,
    #[serde(rename = "mappings")]
    pub(super) mappings: Vec<String>,
    #[serde(rename = "dpiStages")]
    pub(super) dpi_stages: LegacyMouseSourceProfileDpistages,
    #[serde(rename = "pollingRate")]
    pub(super) polling_rate: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LegacyMouseSource {
    #[serde(rename = "defaultMappings")]
    pub(super) default_mappings: Vec<LegacyMouseSourceDefaultmappingsitem>,
    #[serde(rename = "dkmKeys")]
    pub(super) dkm_keys: Vec<LegacyMouseSourceDkmkeysitem>,
    #[serde(rename = "profile")]
    pub(super) profile: LegacyMouseSourceProfile,
}

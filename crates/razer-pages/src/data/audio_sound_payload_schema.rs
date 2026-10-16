// Explicit domain schema for source-derived fixed data.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsTutorialTutoriallistitemContentsitem {
    #[serde(rename = "head", default, skip_serializing_if = "Option::is_none")]
    pub(super) head: Option<String>,
    #[serde(rename = "body")]
    pub(super) body: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub(super) r#type: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsTutorialTutoriallistitem {
    #[serde(rename = "videoName")]
    pub(super) video_name: String,
    #[serde(rename = "mainTitle")]
    pub(super) main_title: String,
    #[serde(rename = "contents")]
    pub(super) contents: Vec<SoundPayloadProductsitemConfigsTutorialTutoriallistitemContentsitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsTutorial {
    #[serde(rename = "tutorialByImage")]
    pub(super) tutorial_by_image: bool,
    #[serde(rename = "tutorialList")]
    pub(super) tutorial_list: Vec<SoundPayloadProductsitemConfigsTutorialTutoriallistitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsThxSwitch {
    #[serde(rename = "showHelp")]
    pub(super) show_help: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsEqualizerBanddataconfig {
    #[serde(rename = "minValue")]
    pub(super) min_value: i64,
    #[serde(rename = "maxValue")]
    pub(super) max_value: u64,
    #[serde(rename = "totalSteps")]
    pub(super) total_steps: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsEqualizerLineConfigitem {
    #[serde(rename = "borderWidth")]
    pub(super) border_width: u64,
    #[serde(rename = "label")]
    pub(super) label: String,
    #[serde(rename = "fill")]
    pub(super) fill: bool,
    #[serde(rename = "pointBorderColor")]
    pub(super) point_border_color: Vec<String>,
    #[serde(rename = "borderColor")]
    pub(super) border_color: String,
    #[serde(rename = "pointBackgroundColor")]
    pub(super) point_background_color: Vec<String>,
    #[serde(rename = "pointRadius")]
    pub(super) point_radius: u64,
    #[serde(rename = "pointHoverRadius")]
    pub(super) point_hover_radius: u64,
    #[serde(rename = "hoverRadius")]
    pub(super) hover_radius: u64,
    #[serde(rename = "hoverBorderWidth")]
    pub(super) hover_border_width: u64,
    #[serde(rename = "lineTension")]
    pub(super) line_tension: u64,
    #[serde(rename = "data")]
    pub(super) data: Vec<i64>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsEqualizer {
    #[serde(rename = "compactMode")]
    pub(super) compact_mode: bool,
    #[serde(rename = "bandDataConfig")]
    pub(super) band_data_config: SoundPayloadProductsitemConfigsEqualizerBanddataconfig,
    #[serde(rename = "yAxisTitle")]
    pub(super) y_axis_title: Vec<String>,
    #[serde(rename = "showSequenceNumber")]
    pub(super) show_sequence_number: bool,
    #[serde(rename = "line_config")]
    pub(super) line_config: Vec<SoundPayloadProductsitemConfigsEqualizerLineConfigitem>,
    #[serde(rename = "frequencies")]
    pub(super) frequencies: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsApplications {
    #[serde(rename = "followCurrentEQ")]
    pub(super) follow_current_eq: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsVolumeGamechatbalanceconfig {
    #[serde(rename = "max")]
    pub(super) max: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsVolume {
    #[serde(rename = "showGameChatBalance")]
    pub(super) show_game_chat_balance: bool,
    #[serde(rename = "gameChatBalanceConfig")]
    pub(super) game_chat_balance_config: SoundPayloadProductsitemConfigsVolumeGamechatbalanceconfig,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsOutput {
    #[serde(rename = "windowBigIcon")]
    pub(super) window_big_icon: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsApplicationOutputsitem {
    #[serde(rename = "name")]
    pub(super) name: String,
    #[serde(rename = "guid", default, skip_serializing_if = "Option::is_none")]
    pub(super) guid: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub(super) r#type: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsRendererGeometry {
    #[serde(rename = "cvsH")]
    pub(super) cvs_h: u64,
    #[serde(rename = "cvsW")]
    pub(super) cvs_w: u64,
    #[serde(rename = "dotRadius")]
    pub(super) dot_radius: f64,
    #[serde(rename = "circleRadius")]
    pub(super) circle_radius: f64,
    #[serde(rename = "xLeft")]
    pub(super) x_left: u64,
    #[serde(rename = "xRight")]
    pub(super) x_right: u64,
    #[serde(rename = "yStart")]
    pub(super) y_start: u64,
    #[serde(rename = "lineLength")]
    pub(super) line_length: u64,
    #[serde(rename = "dotColor")]
    pub(super) dot_color: String,
    #[serde(rename = "activeColor")]
    pub(super) active_color: String,
    #[serde(rename = "hyperColor")]
    pub(super) hyper_color: String,
    #[serde(rename = "dotGap")]
    pub(super) dot_gap: u64,
    #[serde(rename = "xCenter")]
    pub(super) x_center: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsDeviceInfoFeatureminfw {
    #[serde(rename = "BluetoothVolume")]
    pub(super) bluetooth_volume: String,
    #[serde(rename = "AudioPrompts")]
    pub(super) audio_prompts: String,
    #[serde(rename = "DISABLE")]
    pub(super) disable: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsDeviceInfo {
    #[serde(rename = "productId")]
    pub(super) product_id: u64,
    #[serde(rename = "bleId", default, skip_serializing_if = "Option::is_none")]
    pub(super) ble_id: Option<u64>,
    #[serde(rename = "dongleId", default, skip_serializing_if = "Option::is_none")]
    pub(super) dongle_id: Option<u64>,
    #[serde(rename = "vendorId")]
    pub(super) vendor_id: u64,
    #[serde(rename = "deviceName")]
    pub(super) device_name: String,
    #[serde(rename = "masterGuideURL")]
    pub(super) master_guide_url: String,
    #[serde(rename = "supportPage")]
    pub(super) support_page: String,
    #[serde(rename = "virtualRouteDeviceName")]
    pub(super) virtual_route_device_name: String,
    #[serde(rename = "isGameChatAudioFunction")]
    pub(super) is_game_chat_audio_function: bool,
    #[serde(rename = "hideLightingIdle")]
    pub(super) hide_lighting_idle: bool,
    #[serde(rename = "category")]
    pub(super) category: String,
    #[serde(rename = "hasDefaultEQInGameProfile")]
    pub(super) has_default_eq_in_game_profile: bool,
    #[serde(rename = "supportOverheadSpeaker")]
    pub(super) support_overhead_speaker: bool,
    #[serde(rename = "showCompetitiveMode")]
    pub(super) show_competitive_mode: bool,
    #[serde(rename = "thxVersion")]
    pub(super) thx_version: String,
    #[serde(rename = "featureMinFw")]
    pub(super) feature_min_fw: SoundPayloadProductsitemConfigsDeviceInfoFeatureminfw,
    #[serde(rename = "xBoxId", default, skip_serializing_if = "Option::is_none")]
    pub(super) x_box_id: Option<u64>,
    #[serde(rename = "isXbox", default, skip_serializing_if = "Option::is_none")]
    pub(super) is_xbox: Option<bool>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsApplicationThxDatasetDiabloIvLauncherExe {
    #[serde(rename = "imm")]
    pub(super) imm: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsApplicationThxDatasetDiabloIvExe {
    #[serde(rename = "imm")]
    pub(super) imm: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsApplicationThxDatasetHitman3Exe {
    #[serde(rename = "imm")]
    pub(super) imm: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsApplicationThxDatasetLauncherExe {
    #[serde(rename = "imm")]
    pub(super) imm: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsApplicationThxDataset {
    #[serde(rename = "diablo iv launcher.exe")]
    pub(super) diablo_iv_launcher_exe: SoundPayloadProductsitemConfigsApplicationThxDatasetDiabloIvLauncherExe,
    #[serde(rename = "diablo iv.exe")]
    pub(super) diablo_iv_exe: SoundPayloadProductsitemConfigsApplicationThxDatasetDiabloIvExe,
    #[serde(rename = "hitman3.exe")]
    pub(super) hitman_3_exe: SoundPayloadProductsitemConfigsApplicationThxDatasetHitman3Exe,
    #[serde(rename = "launcher.exe")]
    pub(super) launcher_exe: SoundPayloadProductsitemConfigsApplicationThxDatasetLauncherExe,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsCustomizerGroupsitemGroupButtonlistitem {
    #[serde(rename = "buttonKey")]
    pub(super) button_key: String,
    #[serde(rename = "defaultValue")]
    pub(super) default_value: String,
    #[serde(rename = "isFocused")]
    pub(super) is_focused: bool,
    #[serde(rename = "isMapped")]
    pub(super) is_mapped: bool,
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "inputType")]
    pub(super) input_type: String,
    #[serde(rename = "mappingType")]
    pub(super) mapping_type: String,
    #[serde(rename = "inputID")]
    pub(super) input_id: String,
    #[serde(rename = "counter")]
    pub(super) counter: String,
    #[serde(rename = "assignment")]
    pub(super) assignment: String,
    #[serde(rename = "assignmentValue")]
    pub(super) assignment_value: String,
    #[serde(rename = "buttonPos")]
    pub(super) button_pos: String,
    #[serde(rename = "functionList")]
    pub(super) function_list: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsCustomizerGroupsitemGroup {
    #[serde(rename = "groupName")]
    pub(super) group_name: String,
    #[serde(rename = "buttonList")]
    pub(super) button_list: Vec<SoundPayloadProductsitemConfigsCustomizerGroupsitemGroupButtonlistitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsCustomizerGroupsitem {
    #[serde(rename = "group")]
    pub(super) group: SoundPayloadProductsitemConfigsCustomizerGroupsitemGroup,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsMappingActionTypes {
    #[serde(rename = "setActiveButton")]
    pub(super) set_active_button: Vec<String>,
    #[serde(rename = "setActiveKeyMapping")]
    pub(super) set_active_key_mapping: Vec<String>,
    #[serde(rename = "setMappingList")]
    pub(super) set_mapping_list: Vec<String>,
    #[serde(rename = "toggleMappingPanel")]
    pub(super) toggle_mapping_panel: Vec<String>,
    #[serde(rename = "updateButtonWithMappings")]
    pub(super) update_button_with_mappings: Vec<String>,
    #[serde(rename = "updateMappingChanged")]
    pub(super) update_mapping_changed: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsMappingAudioConstantsBitem {
    #[serde(rename = "id")]
    pub(super) id: String,
    #[serde(rename = "content")]
    pub(super) content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsMappingAudioConstantsZitem {
    #[serde(rename = "id")]
    pub(super) id: String,
    #[serde(rename = "content")]
    pub(super) content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsMappingAudioConstantsItem {
    #[serde(rename = "id")]
    pub(super) id: String,
    #[serde(rename = "content")]
    pub(super) content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsMappingAudioConstantsEitem {
    #[serde(rename = "content")]
    pub(super) content: String,
    #[serde(rename = "id")]
    pub(super) id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsMappingAudioConstants {
    #[serde(rename = "m")]
    pub(super) m: String,
    #[serde(rename = "y")]
    pub(super) y: String,
    #[serde(rename = "h")]
    pub(super) h: String,
    #[serde(rename = "g")]
    pub(super) g: String,
    #[serde(rename = "k")]
    pub(super) k: bool,
    #[serde(rename = "B")]
    pub(super) b: Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsBitem>,
    #[serde(rename = "n")]
    pub(super) n: i64,
    #[serde(rename = "z")]
    pub(super) z: Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsZitem>,
    #[serde(rename = "_")]
    pub(super) field_: Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsItem>,
    #[serde(rename = "i")]
    pub(super) i: Vec<String>,
    #[serde(rename = "E")]
    pub(super) e: Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsEitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsMappingAudioOptionsitem {
    #[serde(rename = "id")]
    pub(super) id: String,
    #[serde(rename = "content")]
    pub(super) content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsMappingAssignmentTableitem {
    #[serde(rename = "id")]
    pub(super) id: String,
    #[serde(rename = "content")]
    pub(super) content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsMappingAudioNotes {
    #[serde(rename = "BluetoothVolume")]
    pub(super) bluetooth_volume: Vec<String>,
    #[serde(rename = "FootstepScaling")]
    pub(super) footstep_scaling: Vec<String>,
    #[serde(rename = "SidetoneLevel")]
    pub(super) sidetone_level: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileBrightness {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileSwitchofflighting {
    #[serde(rename = "isDisplayOn")]
    pub(super) is_display_on: bool,
    #[serde(rename = "isIdleEnabled")]
    pub(super) is_idle_enabled: bool,
    #[serde(rename = "idleMinutes")]
    pub(super) idle_minutes: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileQuickeffects {
    #[serde(rename = "selectedEffectId")]
    pub(super) selected_effect_id: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileBassboost {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileSoundnormalization {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileVoiceclarity {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicsidetone {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicvolumenormalization {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicvoiceclarity {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicsensitivity {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicequalizer {
    #[serde(rename = "mode")]
    pub(super) mode: String,
    #[serde(rename = "customBands")]
    pub(super) custom_bands: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicpreview {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileAudioequalizer {
    #[serde(rename = "mode")]
    pub(super) mode: String,
    #[serde(rename = "customBands")]
    pub(super) custom_bands: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileAmbientnoisereduction {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "value")]
    pub(super) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileThxspatialaudio {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "mode")]
    pub(super) mode: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitemOutput {
    #[serde(rename = "type")]
    pub(super) r#type: String,
    #[serde(rename = "guid")]
    pub(super) guid: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitem {
    #[serde(rename = "appType")]
    pub(super) app_type: String,
    #[serde(rename = "output")]
    pub(super) output: SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitemOutput,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileApplications {
    #[serde(rename = "generalInputSetting")]
    pub(super) general_input_setting: Vec<SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitem>,
    #[serde(rename = "specificInputSetting")]
    pub(super) specific_input_setting: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersitem {
    #[serde(rename = "id")]
    pub(super) id: String,
    #[serde(rename = "angle")]
    pub(super) angle: i64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersoverheaditem {
    #[serde(rename = "id")]
    pub(super) id: String,
    #[serde(rename = "angle")]
    pub(super) angle: i64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileCalibration {
    #[serde(rename = "speakers")]
    pub(super) speakers: Vec<SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersitem>,
    #[serde(rename = "speakersOverhead")]
    pub(super) speakers_overhead: Vec<SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersoverheaditem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicambientnoisereduction {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigsProductDefaultprofile {
    #[serde(rename = "guid")]
    pub(super) guid: String,
    #[serde(rename = "name")]
    pub(super) name: String,
    #[serde(rename = "brightness")]
    pub(super) brightness: SoundPayloadProductsitemConfigsProductDefaultprofileBrightness,
    #[serde(rename = "switchOffLighting")]
    pub(super) switch_off_lighting: SoundPayloadProductsitemConfigsProductDefaultprofileSwitchofflighting,
    #[serde(rename = "quickEffects")]
    pub(super) quick_effects: SoundPayloadProductsitemConfigsProductDefaultprofileQuickeffects,
    #[serde(rename = "bassBoost")]
    pub(super) bass_boost: SoundPayloadProductsitemConfigsProductDefaultprofileBassboost,
    #[serde(rename = "soundNormalization")]
    pub(super) sound_normalization: SoundPayloadProductsitemConfigsProductDefaultprofileSoundnormalization,
    #[serde(rename = "voiceClarity")]
    pub(super) voice_clarity: SoundPayloadProductsitemConfigsProductDefaultprofileVoiceclarity,
    #[serde(rename = "micSideTone")]
    pub(super) mic_side_tone: SoundPayloadProductsitemConfigsProductDefaultprofileMicsidetone,
    #[serde(rename = "micVolumeNormalization")]
    pub(super) mic_volume_normalization: SoundPayloadProductsitemConfigsProductDefaultprofileMicvolumenormalization,
    #[serde(rename = "micVoiceClarity")]
    pub(super) mic_voice_clarity: SoundPayloadProductsitemConfigsProductDefaultprofileMicvoiceclarity,
    #[serde(rename = "micSensitivity")]
    pub(super) mic_sensitivity: SoundPayloadProductsitemConfigsProductDefaultprofileMicsensitivity,
    #[serde(rename = "micEqualizer")]
    pub(super) mic_equalizer: SoundPayloadProductsitemConfigsProductDefaultprofileMicequalizer,
    #[serde(rename = "micPreview")]
    pub(super) mic_preview: SoundPayloadProductsitemConfigsProductDefaultprofileMicpreview,
    #[serde(rename = "audioEqualizer")]
    pub(super) audio_equalizer: SoundPayloadProductsitemConfigsProductDefaultprofileAudioequalizer,
    #[serde(rename = "ambientNoiseReduction", default, skip_serializing_if = "Option::is_none")]
    pub(super) ambient_noise_reduction: Option<SoundPayloadProductsitemConfigsProductDefaultprofileAmbientnoisereduction>,
    #[serde(rename = "thxSpatialAudio")]
    pub(super) thx_spatial_audio: SoundPayloadProductsitemConfigsProductDefaultprofileThxspatialaudio,
    #[serde(rename = "applications")]
    pub(super) applications: SoundPayloadProductsitemConfigsProductDefaultprofileApplications,
    #[serde(rename = "calibration")]
    pub(super) calibration: SoundPayloadProductsitemConfigsProductDefaultprofileCalibration,
    #[serde(rename = "micAmbientNoiseReduction", default, skip_serializing_if = "Option::is_none")]
    pub(super) mic_ambient_noise_reduction: Option<SoundPayloadProductsitemConfigsProductDefaultprofileMicambientnoisereduction>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemConfigs {
    #[serde(rename = "tutorial")]
    pub(super) tutorial: SoundPayloadProductsitemConfigsTutorial,
    #[serde(rename = "customizer")]
    pub(super) customizer: std::collections::BTreeMap<String,String>,
    #[serde(rename = "thx_switch")]
    pub(super) thx_switch: SoundPayloadProductsitemConfigsThxSwitch,
    #[serde(rename = "equalizer")]
    pub(super) equalizer: SoundPayloadProductsitemConfigsEqualizer,
    #[serde(rename = "applications")]
    pub(super) applications: SoundPayloadProductsitemConfigsApplications,
    #[serde(rename = "volume")]
    pub(super) volume: SoundPayloadProductsitemConfigsVolume,
    #[serde(rename = "output")]
    pub(super) output: SoundPayloadProductsitemConfigsOutput,
    #[serde(rename = "application_outputs")]
    pub(super) application_outputs: Vec<SoundPayloadProductsitemConfigsApplicationOutputsitem>,
    #[serde(rename = "renderer_geometry")]
    pub(super) renderer_geometry: SoundPayloadProductsitemConfigsRendererGeometry,
    #[serde(rename = "device_info")]
    pub(super) device_info: SoundPayloadProductsitemConfigsDeviceInfo,
    #[serde(rename = "application_thx_dataset")]
    pub(super) application_thx_dataset: SoundPayloadProductsitemConfigsApplicationThxDataset,
    #[serde(rename = "customizer_groups")]
    pub(super) customizer_groups: Vec<SoundPayloadProductsitemConfigsCustomizerGroupsitem>,
    #[serde(rename = "support_overhead_speaker")]
    pub(super) support_overhead_speaker: bool,
    #[serde(rename = "mapping_exports")]
    pub(super) mapping_exports: Vec<String>,
    #[serde(rename = "mapping_action_types")]
    pub(super) mapping_action_types: SoundPayloadProductsitemConfigsMappingActionTypes,
    #[serde(rename = "mapping_audio_constants")]
    pub(super) mapping_audio_constants: SoundPayloadProductsitemConfigsMappingAudioConstants,
    #[serde(rename = "mapping_audio_options")]
    pub(super) mapping_audio_options: Vec<SoundPayloadProductsitemConfigsMappingAudioOptionsitem>,
    #[serde(rename = "mapping_assignment_table")]
    pub(super) mapping_assignment_table: Vec<SoundPayloadProductsitemConfigsMappingAssignmentTableitem>,
    #[serde(rename = "mapping_audio_notes")]
    pub(super) mapping_audio_notes: SoundPayloadProductsitemConfigsMappingAudioNotes,
    #[serde(rename = "product_exports")]
    pub(super) product_exports: Vec<String>,
    #[serde(rename = "product_DEFAULTMAPPINGS")]
    pub(super) product_defaultmappings: Vec<String>,
    #[serde(rename = "product_DEFAULTPROFILE")]
    pub(super) product_defaultprofile: SoundPayloadProductsitemConfigsProductDefaultprofile,
    #[serde(rename = "product_DEFAULT_FIRMWARE_MAPPINGS")]
    pub(super) product_mappings: Vec<String>,
    #[serde(rename = "product_OBM_SUPPORTED_MAPPING_GROUPS")]
    pub(super) product_groups: Vec<String>,
    #[serde(rename = "product_SYNAPSE_NOT_REQUIRED_MAPPING_FUNCTIONS")]
    pub(super) product_functions: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemMediaEditions0 {
    #[serde(rename = "prd")]
    pub(super) prd: String,
    #[serde(rename = "wheel-cutout")]
    pub(super) wheel_cutout: String,
    #[serde(rename = "smart-switch-button")]
    pub(super) smart_switch_button: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemMediaEditions128 {
    #[serde(rename = "prd")]
    pub(super) prd: String,
    #[serde(rename = "wheel-cutout")]
    pub(super) wheel_cutout: String,
    #[serde(rename = "smart-switch-button")]
    pub(super) smart_switch_button: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemMediaEditions129 {
    #[serde(rename = "prd")]
    pub(super) prd: String,
    #[serde(rename = "wheel-cutout")]
    pub(super) wheel_cutout: String,
    #[serde(rename = "smart-switch-button")]
    pub(super) smart_switch_button: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemMediaEditions131 {
    #[serde(rename = "prd")]
    pub(super) prd: String,
    #[serde(rename = "wheel-cutout")]
    pub(super) wheel_cutout: String,
    #[serde(rename = "smart-switch-button")]
    pub(super) smart_switch_button: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemMediaEditions132 {
    #[serde(rename = "prd")]
    pub(super) prd: String,
    #[serde(rename = "wheel-cutout")]
    pub(super) wheel_cutout: String,
    #[serde(rename = "smart-switch-button")]
    pub(super) smart_switch_button: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemMediaEditions130 {
    #[serde(rename = "prd")]
    pub(super) prd: String,
    #[serde(rename = "wheel-cutout")]
    pub(super) wheel_cutout: String,
    #[serde(rename = "smart-switch-button")]
    pub(super) smart_switch_button: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemMediaEditions {
    #[serde(rename = "0")]
    pub(super) field_0: SoundPayloadProductsitemMediaEditions0,
    #[serde(rename = "128")]
    pub(super) field_128: SoundPayloadProductsitemMediaEditions128,
    #[serde(rename = "129", default, skip_serializing_if = "Option::is_none")]
    pub(super) field_129: Option<SoundPayloadProductsitemMediaEditions129>,
    #[serde(rename = "131", default, skip_serializing_if = "Option::is_none")]
    pub(super) field_131: Option<SoundPayloadProductsitemMediaEditions131>,
    #[serde(rename = "132", default, skip_serializing_if = "Option::is_none")]
    pub(super) field_132: Option<SoundPayloadProductsitemMediaEditions132>,
    #[serde(rename = "130", default, skip_serializing_if = "Option::is_none")]
    pub(super) field_130: Option<SoundPayloadProductsitemMediaEditions130>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemMediaCss {
    #[serde(rename = "window-big-icon")]
    pub(super) window_big_icon: String,
    #[serde(rename = "common-windows-11")]
    pub(super) common_windows_11: String,
    #[serde(rename = "icon_refresh")]
    pub(super) icon_refresh: String,
    #[serde(rename = "icon_refresh.green")]
    pub(super) icon_refresh_green: String,
    #[serde(rename = "icon_search")]
    pub(super) icon_search: String,
    #[serde(rename = "icon_search.green")]
    pub(super) icon_search_green: String,
    #[serde(rename = "icon_search.grey")]
    pub(super) icon_search_grey: String,
    #[serde(rename = "icon_close_enclosed")]
    pub(super) icon_close_enclosed: String,
    #[serde(rename = "icon_close_enclosed_a")]
    pub(super) icon_close_enclosed_a: String,
    #[serde(rename = "chat-more")]
    pub(super) chat_more: String,
    #[serde(rename = "game-more")]
    pub(super) game_more: String,
    #[serde(rename = "pen_edit")]
    pub(super) pen_edit: String,
    #[serde(rename = "icon_pen_edit")]
    pub(super) icon_pen_edit: String,
    #[serde(rename = "eq_reset")]
    pub(super) eq_reset: String,
    #[serde(rename = "eq_reset_hover")]
    pub(super) eq_reset_hover: String,
    #[serde(rename = "eq_reset_active")]
    pub(super) eq_reset_active: String,
    #[serde(rename = "icon-info")]
    pub(super) icon_info: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemMediaEqIcons {
    #[serde(rename = "0")]
    pub(super) field_0: String,
    #[serde(rename = "1")]
    pub(super) field_1: String,
    #[serde(rename = "2")]
    pub(super) field_2: String,
    #[serde(rename = "3")]
    pub(super) field_3: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemMedia {
    #[serde(rename = "editions")]
    pub(super) editions: SoundPayloadProductsitemMediaEditions,
    #[serde(rename = "css")]
    pub(super) css: SoundPayloadProductsitemMediaCss,
    #[serde(rename = "reset_link")]
    pub(super) reset_link: String,
    #[serde(rename = "eq_icons")]
    pub(super) eq_icons: SoundPayloadProductsitemMediaEqIcons,
    #[serde(rename = "sword-added-icon")]
    pub(super) sword_added_icon: String,
    #[serde(rename = "search-empty")]
    pub(super) search_empty: String,
    #[serde(rename = "icon_add")]
    pub(super) icon_add: String,
    #[serde(rename = "placeholder_game_icon")]
    pub(super) placeholder_game_icon: String,
    #[serde(rename = "icon_close_enclosed_small")]
    pub(super) icon_close_enclosed_small: String,
    #[serde(rename = "icon_close_enclosed_a_small")]
    pub(super) icon_close_enclosed_a_small: String,
    #[serde(rename = "appmix_gamemode")]
    pub(super) appmix_gamemode: String,
    #[serde(rename = "appmix_thx")]
    pub(super) appmix_thx: String,
    #[serde(rename = "thx_help")]
    pub(super) thx_help: String,
    #[serde(rename = "mapping_footstep")]
    pub(super) mapping_footstep: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemInitialsSpatial {
    #[serde(rename = "isEnabled")]
    pub(super) is_enabled: bool,
    #[serde(rename = "mode")]
    pub(super) mode: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemInitialsGameChat {
    #[serde(rename = "balanceMode")]
    pub(super) balance_mode: u64,
    #[serde(rename = "actionsFromUI")]
    pub(super) actions_from_ui: Vec<String>,
    #[serde(rename = "actionsFromLocalStorage")]
    pub(super) actions_from_local_storage: Vec<String>,
    #[serde(rename = "errorActions")]
    pub(super) error_actions: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemInitials {
    #[serde(rename = "spatial")]
    pub(super) spatial: SoundPayloadProductsitemInitialsSpatial,
    #[serde(rename = "volume")]
    pub(super) volume: std::collections::BTreeMap<String,String>,
    #[serde(rename = "game_chat")]
    pub(super) game_chat: SoundPayloadProductsitemInitialsGameChat,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemCloudInitialAvailableesportspresetsitemEqualizerName {
    #[serde(rename = "en")]
    pub(super) en: String,
    #[serde(rename = "zh-CN")]
    pub(super) zh_cn: String,
    #[serde(rename = "zh-TW")]
    pub(super) zh_tw: String,
    #[serde(rename = "zh-HK", default, skip_serializing_if = "Option::is_none")]
    pub(super) zh_hk: Option<String>,
    #[serde(rename = "da-DK")]
    pub(super) da_dk: String,
    #[serde(rename = "de-DE")]
    pub(super) de_de: String,
    #[serde(rename = "es-ES")]
    pub(super) es_es: String,
    #[serde(rename = "fr-FR")]
    pub(super) fr_fr: String,
    #[serde(rename = "it-IT")]
    pub(super) it_it: String,
    #[serde(rename = "ja-JP")]
    pub(super) ja_jp: String,
    #[serde(rename = "ko-KR")]
    pub(super) ko_kr: String,
    #[serde(rename = "ru-RU")]
    pub(super) ru_ru: String,
    #[serde(rename = "sv-SE")]
    pub(super) sv_se: String,
    #[serde(rename = "pt-BR")]
    pub(super) pt_br: String,
    #[serde(rename = "zh_HKt", default, skip_serializing_if = "Option::is_none")]
    pub(super) zh_h_kt: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemCloudInitialAvailableesportspresetsitemBandLabelIdsitem {
    #[serde(rename = "label_id")]
    pub(super) label_id: i64,
    #[serde(rename = "band_index")]
    pub(super) band_index: Vec<u64>,
    #[serde(rename = "icon_start_index_position")]
    pub(super) icon_start_index_position: u64,
    #[serde(rename = "label_icon_position")]
    pub(super) label_icon_position: String,
    #[serde(rename = "label_arrow_position")]
    pub(super) label_arrow_position: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemCloudInitialAvailableesportspresetsitem {
    #[serde(rename = "equalizer_id")]
    pub(super) equalizer_id: u64,
    #[serde(rename = "equalizer_category")]
    pub(super) equalizer_category: u64,
    #[serde(rename = "equalizer_icon")]
    pub(super) equalizer_icon: String,
    #[serde(rename = "equalizer_svg_icon")]
    pub(super) equalizer_svg_icon: String,
    #[serde(rename = "is_oobe")]
    pub(super) is_oobe: bool,
    #[serde(rename = "is_oobe_enabled", default, skip_serializing_if = "Option::is_none")]
    pub(super) is_oobe_enabled: Option<bool>,
    #[serde(rename = "is_allow_swapping")]
    pub(super) is_allow_swapping: bool,
    #[serde(rename = "equalizer_name")]
    pub(super) equalizer_name: SoundPayloadProductsitemCloudInitialAvailableesportspresetsitemEqualizerName,
    #[serde(rename = "band_values")]
    pub(super) band_values: Vec<i64>,
    #[serde(rename = "footstep_values")]
    pub(super) footstep_values: Vec<u64>,
    #[serde(rename = "band_label_ids")]
    pub(super) band_label_ids: Vec<SoundPayloadProductsitemCloudInitialAvailableesportspresetsitemBandLabelIdsitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitemCloudInitial {
    #[serde(rename = "selectedPreset")]
    pub(super) selected_preset: u64,
    #[serde(rename = "enabledPresets")]
    pub(super) enabled_presets: Vec<String>,
    #[serde(rename = "disabledPresets")]
    pub(super) disabled_presets: Vec<String>,
    #[serde(rename = "tempEnabledPresets")]
    pub(super) temp_enabled_presets: Vec<String>,
    #[serde(rename = "tempDisabledPresets")]
    pub(super) temp_disabled_presets: Vec<String>,
    #[serde(rename = "tempActivePreset")]
    pub(super) temp_active_preset: (),
    #[serde(rename = "actionsFromUI")]
    pub(super) actions_from_ui: Vec<String>,
    #[serde(rename = "actionsFromLocalStorage")]
    pub(super) actions_from_local_storage: Vec<String>,
    #[serde(rename = "errorActions")]
    pub(super) error_actions: Vec<String>,
    #[serde(rename = "availableEsportsPresets")]
    pub(super) available_esports_presets: Vec<SoundPayloadProductsitemCloudInitialAvailableesportspresetsitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayloadProductsitem {
    #[serde(rename = "configs")]
    pub(super) configs: SoundPayloadProductsitemConfigs,
    #[serde(rename = "media")]
    pub(super) media: SoundPayloadProductsitemMedia,
    #[serde(rename = "initials")]
    pub(super) initials: SoundPayloadProductsitemInitials,
    #[serde(rename = "cloud_initial")]
    pub(super) cloud_initial: SoundPayloadProductsitemCloudInitial,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SoundPayload {
    #[serde(rename = "products")]
    pub(super) products: Vec<SoundPayloadProductsitem>,
}

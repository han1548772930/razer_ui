// Explicit domain schema for source-derived fixed data.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsTutorialTutoriallistitemContentsitem {
    #[serde(rename = "head", default, skip_serializing_if = "Option::is_none")]
    pub(in crate::features) head: Option<String>,
    #[serde(rename = "body")]
    pub(in crate::features) body: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub(in crate::features) r#type: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsTutorialTutoriallistitem {
    #[serde(rename = "videoName")]
    pub(in crate::features) video_name: String,
    #[serde(rename = "mainTitle")]
    pub(in crate::features) main_title: String,
    #[serde(rename = "contents")]
    pub(in crate::features) contents:
        Vec<SoundPayloadProductsitemConfigsTutorialTutoriallistitemContentsitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsTutorial {
    #[serde(rename = "tutorialByImage")]
    pub(in crate::features) tutorial_by_image: bool,
    #[serde(rename = "tutorialList")]
    pub(in crate::features) tutorial_list:
        Vec<SoundPayloadProductsitemConfigsTutorialTutoriallistitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsThxSwitch {
    #[serde(rename = "showHelp")]
    pub(in crate::features) show_help: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsEqualizerBanddataconfig {
    #[serde(rename = "minValue")]
    pub(in crate::features) min_value: i64,
    #[serde(rename = "maxValue")]
    pub(in crate::features) max_value: u64,
    #[serde(rename = "totalSteps")]
    pub(in crate::features) total_steps: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsEqualizerLineConfigitem {
    #[serde(rename = "borderWidth")]
    pub(in crate::features) border_width: u64,
    #[serde(rename = "label")]
    pub(in crate::features) label: String,
    #[serde(rename = "fill")]
    pub(in crate::features) fill: bool,
    #[serde(rename = "pointBorderColor")]
    pub(in crate::features) point_border_color: Vec<String>,
    #[serde(rename = "borderColor")]
    pub(in crate::features) border_color: String,
    #[serde(rename = "pointBackgroundColor")]
    pub(in crate::features) point_background_color: Vec<String>,
    #[serde(rename = "pointRadius")]
    pub(in crate::features) point_radius: u64,
    #[serde(rename = "pointHoverRadius")]
    pub(in crate::features) point_hover_radius: u64,
    #[serde(rename = "hoverRadius")]
    pub(in crate::features) hover_radius: u64,
    #[serde(rename = "hoverBorderWidth")]
    pub(in crate::features) hover_border_width: u64,
    #[serde(rename = "lineTension")]
    pub(in crate::features) line_tension: u64,
    #[serde(rename = "data")]
    pub(in crate::features) data: Vec<i64>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsEqualizer {
    #[serde(rename = "compactMode")]
    pub(in crate::features) compact_mode: bool,
    #[serde(rename = "bandDataConfig")]
    pub(in crate::features) band_data_config:
        SoundPayloadProductsitemConfigsEqualizerBanddataconfig,
    #[serde(rename = "yAxisTitle")]
    pub(in crate::features) y_axis_title: Vec<String>,
    #[serde(rename = "showSequenceNumber")]
    pub(in crate::features) show_sequence_number: bool,
    #[serde(rename = "line_config")]
    pub(in crate::features) line_config:
        Vec<SoundPayloadProductsitemConfigsEqualizerLineConfigitem>,
    #[serde(rename = "frequencies")]
    pub(in crate::features) frequencies: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsApplications {
    #[serde(rename = "followCurrentEQ")]
    pub(in crate::features) follow_current_eq: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsVolumeGamechatbalanceconfig {
    #[serde(rename = "max")]
    pub(in crate::features) max: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsVolume {
    #[serde(rename = "showGameChatBalance")]
    pub(in crate::features) show_game_chat_balance: bool,
    #[serde(rename = "gameChatBalanceConfig")]
    pub(in crate::features) game_chat_balance_config:
        SoundPayloadProductsitemConfigsVolumeGamechatbalanceconfig,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsOutput {
    #[serde(rename = "windowBigIcon")]
    pub(in crate::features) window_big_icon: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsApplicationOutputsitem {
    #[serde(rename = "name")]
    pub(in crate::features) name: String,
    #[serde(rename = "guid", default, skip_serializing_if = "Option::is_none")]
    pub(in crate::features) guid: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub(in crate::features) r#type: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsRendererGeometry {
    #[serde(rename = "cvsH")]
    pub(in crate::features) cvs_h: u64,
    #[serde(rename = "cvsW")]
    pub(in crate::features) cvs_w: u64,
    #[serde(rename = "dotRadius")]
    pub(in crate::features) dot_radius: f64,
    #[serde(rename = "circleRadius")]
    pub(in crate::features) circle_radius: f64,
    #[serde(rename = "xLeft")]
    pub(in crate::features) x_left: u64,
    #[serde(rename = "xRight")]
    pub(in crate::features) x_right: u64,
    #[serde(rename = "yStart")]
    pub(in crate::features) y_start: u64,
    #[serde(rename = "lineLength")]
    pub(in crate::features) line_length: u64,
    #[serde(rename = "dotColor")]
    pub(in crate::features) dot_color: String,
    #[serde(rename = "activeColor")]
    pub(in crate::features) active_color: String,
    #[serde(rename = "hyperColor")]
    pub(in crate::features) hyper_color: String,
    #[serde(rename = "dotGap")]
    pub(in crate::features) dot_gap: u64,
    #[serde(rename = "xCenter")]
    pub(in crate::features) x_center: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsDeviceInfo {
    #[serde(rename = "productId")]
    pub(in crate::features) product_id: u64,
    #[serde(rename = "bleId", default, skip_serializing_if = "Option::is_none")]
    pub(in crate::features) ble_id: Option<u64>,
    #[serde(rename = "dongleId", default, skip_serializing_if = "Option::is_none")]
    pub(in crate::features) dongle_id: Option<u64>,
    #[serde(rename = "vendorId")]
    pub(in crate::features) vendor_id: u64,
    #[serde(rename = "deviceName")]
    pub(in crate::features) device_name: String,
    #[serde(rename = "masterGuideURL")]
    pub(in crate::features) master_guide_url: String,
    #[serde(rename = "supportPage")]
    pub(in crate::features) support_page: String,
    #[serde(rename = "virtualRouteDeviceName")]
    pub(in crate::features) virtual_route_device_name: String,
    #[serde(rename = "isGameChatAudioFunction")]
    pub(in crate::features) is_game_chat_audio_function: bool,
    #[serde(rename = "hideLightingIdle")]
    pub(in crate::features) hide_lighting_idle: bool,
    #[serde(rename = "category")]
    pub(in crate::features) category: String,
    #[serde(rename = "hasDefaultEQInGameProfile")]
    pub(in crate::features) has_default_eq_in_game_profile: bool,
    #[serde(rename = "supportOverheadSpeaker")]
    pub(in crate::features) support_overhead_speaker: bool,
    #[serde(rename = "showCompetitiveMode")]
    pub(in crate::features) show_competitive_mode: bool,
    #[serde(rename = "thxVersion")]
    pub(in crate::features) thx_version: String,
    #[serde(rename = "featureMinFw")]
    pub(in crate::features) feature_min_fw: std::collections::BTreeMap<String, String>,
    #[serde(rename = "xBoxId", default, skip_serializing_if = "Option::is_none")]
    pub(in crate::features) x_box_id: Option<u64>,
    #[serde(rename = "isXbox", default, skip_serializing_if = "Option::is_none")]
    pub(in crate::features) is_xbox: Option<bool>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsApplicationThxDatasetentry {
    #[serde(rename = "imm")]
    pub(in crate::features) imm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::features) cmm: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsCustomizerGroupsitemGroupButtonlistitem
{
    #[serde(rename = "buttonKey")]
    pub(in crate::features) button_key: String,
    #[serde(rename = "defaultValue")]
    pub(in crate::features) default_value: String,
    #[serde(rename = "isFocused")]
    pub(in crate::features) is_focused: bool,
    #[serde(rename = "isMapped")]
    pub(in crate::features) is_mapped: bool,
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "inputType")]
    pub(in crate::features) input_type: String,
    #[serde(rename = "mappingType")]
    pub(in crate::features) mapping_type: String,
    #[serde(rename = "inputID")]
    pub(in crate::features) input_id: String,
    #[serde(rename = "counter")]
    pub(in crate::features) counter: String,
    #[serde(rename = "assignment")]
    pub(in crate::features) assignment: String,
    #[serde(rename = "assignmentValue")]
    pub(in crate::features) assignment_value: String,
    #[serde(rename = "buttonPos")]
    pub(in crate::features) button_pos: String,
    #[serde(rename = "functionList")]
    pub(in crate::features) function_list: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsCustomizerGroupsitemGroup {
    #[serde(rename = "groupName")]
    pub(in crate::features) group_name: String,
    #[serde(rename = "buttonList")]
    pub(in crate::features) button_list:
        Vec<SoundPayloadProductsitemConfigsCustomizerGroupsitemGroupButtonlistitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsCustomizerGroupsitem {
    #[serde(rename = "group")]
    pub(in crate::features) group: SoundPayloadProductsitemConfigsCustomizerGroupsitemGroup,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsMappingAudioConstantsBitem {
    #[serde(rename = "id")]
    pub(in crate::features) id: String,
    #[serde(rename = "content")]
    pub(in crate::features) content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsMappingAudioConstantsZitem {
    #[serde(rename = "id")]
    pub(in crate::features) id: String,
    #[serde(rename = "content")]
    pub(in crate::features) content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsMappingAudioConstantsItem {
    #[serde(rename = "id")]
    pub(in crate::features) id: String,
    #[serde(rename = "content")]
    pub(in crate::features) content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsMappingAudioConstantsEitem {
    #[serde(rename = "content")]
    pub(in crate::features) content: String,
    #[serde(rename = "id")]
    pub(in crate::features) id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsMappingAudioConstants {
    #[serde(rename = "m")]
    pub(in crate::features) m: String,
    #[serde(rename = "y")]
    pub(in crate::features) y: String,
    #[serde(rename = "h")]
    pub(in crate::features) h: String,
    #[serde(rename = "g")]
    pub(in crate::features) g: String,
    #[serde(rename = "k")]
    pub(in crate::features) k: bool,
    #[serde(rename = "B")]
    pub(in crate::features) b: Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsBitem>,
    #[serde(rename = "n")]
    pub(in crate::features) n: i64,
    #[serde(rename = "z")]
    pub(in crate::features) z: Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsZitem>,
    #[serde(rename = "_")]
    pub(in crate::features) field_: Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsItem>,
    #[serde(rename = "i")]
    pub(in crate::features) i: Vec<String>,
    #[serde(rename = "E")]
    pub(in crate::features) e: Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsEitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsMappingAudioOptionsitem {
    #[serde(rename = "id")]
    pub(in crate::features) id: String,
    #[serde(rename = "content")]
    pub(in crate::features) content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsMappingAssignmentTableitem {
    #[serde(rename = "id")]
    pub(in crate::features) id: String,
    #[serde(rename = "content")]
    pub(in crate::features) content: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileBrightness {
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "value")]
    pub(in crate::features) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileSwitchofflighting
{
    #[serde(rename = "isDisplayOn")]
    pub(in crate::features) is_display_on: bool,
    #[serde(rename = "isIdleEnabled")]
    pub(in crate::features) is_idle_enabled: bool,
    #[serde(rename = "idleMinutes")]
    pub(in crate::features) idle_minutes: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileQuickeffects {
    #[serde(rename = "selectedEffectId")]
    pub(in crate::features) selected_effect_id: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileBassboost {
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "value")]
    pub(in crate::features) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileSoundnormalization
{
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "value")]
    pub(in crate::features) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileVoiceclarity {
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "value")]
    pub(in crate::features) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicsidetone {
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "value")]
    pub(in crate::features) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicvolumenormalization
{
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "value")]
    pub(in crate::features) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicvoiceclarity {
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "value")]
    pub(in crate::features) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicsensitivity {
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "value")]
    pub(in crate::features) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicequalizer {
    #[serde(rename = "mode")]
    pub(in crate::features) mode: String,
    #[serde(rename = "customBands")]
    pub(in crate::features) custom_bands: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicpreview {
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileAudioequalizer {
    #[serde(rename = "mode")]
    pub(in crate::features) mode: String,
    #[serde(rename = "customBands")]
    pub(in crate::features) custom_bands: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileAmbientnoisereduction
{
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "value")]
    pub(in crate::features) value: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileThxspatialaudio {
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "mode")]
    pub(in crate::features) mode: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitemOutput
{
    #[serde(rename = "type")]
    pub(in crate::features) r#type: String,
    #[serde(rename = "guid")]
    pub(in crate::features) guid: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitem {
    #[serde(rename = "appType")]
    pub(in crate::features) app_type: String,
    #[serde(rename = "output")]
    pub(in crate::features) output: SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitemOutput,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileApplications {
    #[serde(rename = "generalInputSetting")]
    pub(in crate::features) general_input_setting: Vec<
        SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitem,
    >,
    #[serde(rename = "specificInputSetting")]
    pub(in crate::features) specific_input_setting: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersitem
{
    #[serde(rename = "id")]
    pub(in crate::features) id: String,
    #[serde(rename = "angle")]
    pub(in crate::features) angle: i64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersoverheaditem
{
    #[serde(rename = "id")]
    pub(in crate::features) id: String,
    #[serde(rename = "angle")]
    pub(in crate::features) angle: i64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileCalibration {
    #[serde(rename = "speakers")]
    pub(in crate::features) speakers:
        Vec<SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersitem>,
    #[serde(rename = "speakersOverhead")]
    pub(in crate::features) speakers_overhead:
        Vec<SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersoverheaditem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofileMicambientnoisereduction
{
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigsProductDefaultprofile {
    #[serde(rename = "guid")]
    pub(in crate::features) guid: String,
    #[serde(rename = "name")]
    pub(in crate::features) name: String,
    #[serde(rename = "brightness")]
    pub(in crate::features) brightness:
        SoundPayloadProductsitemConfigsProductDefaultprofileBrightness,
    #[serde(rename = "switchOffLighting")]
    pub(in crate::features) switch_off_lighting:
        SoundPayloadProductsitemConfigsProductDefaultprofileSwitchofflighting,
    #[serde(rename = "quickEffects")]
    pub(in crate::features) quick_effects:
        SoundPayloadProductsitemConfigsProductDefaultprofileQuickeffects,
    #[serde(rename = "bassBoost")]
    pub(in crate::features) bass_boost:
        SoundPayloadProductsitemConfigsProductDefaultprofileBassboost,
    #[serde(rename = "soundNormalization")]
    pub(in crate::features) sound_normalization:
        SoundPayloadProductsitemConfigsProductDefaultprofileSoundnormalization,
    #[serde(rename = "voiceClarity")]
    pub(in crate::features) voice_clarity:
        SoundPayloadProductsitemConfigsProductDefaultprofileVoiceclarity,
    #[serde(rename = "micSideTone")]
    pub(in crate::features) mic_side_tone:
        SoundPayloadProductsitemConfigsProductDefaultprofileMicsidetone,
    #[serde(rename = "micVolumeNormalization")]
    pub(in crate::features) mic_volume_normalization:
        SoundPayloadProductsitemConfigsProductDefaultprofileMicvolumenormalization,
    #[serde(rename = "micVoiceClarity")]
    pub(in crate::features) mic_voice_clarity:
        SoundPayloadProductsitemConfigsProductDefaultprofileMicvoiceclarity,
    #[serde(rename = "micSensitivity")]
    pub(in crate::features) mic_sensitivity:
        SoundPayloadProductsitemConfigsProductDefaultprofileMicsensitivity,
    #[serde(rename = "micEqualizer")]
    pub(in crate::features) mic_equalizer:
        SoundPayloadProductsitemConfigsProductDefaultprofileMicequalizer,
    #[serde(rename = "micPreview")]
    pub(in crate::features) mic_preview:
        SoundPayloadProductsitemConfigsProductDefaultprofileMicpreview,
    #[serde(rename = "audioEqualizer")]
    pub(in crate::features) audio_equalizer:
        SoundPayloadProductsitemConfigsProductDefaultprofileAudioequalizer,
    #[serde(
        rename = "ambientNoiseReduction",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(in crate::features) ambient_noise_reduction:
        Option<SoundPayloadProductsitemConfigsProductDefaultprofileAmbientnoisereduction>,
    #[serde(rename = "thxSpatialAudio")]
    pub(in crate::features) thx_spatial_audio:
        SoundPayloadProductsitemConfigsProductDefaultprofileThxspatialaudio,
    #[serde(rename = "applications")]
    pub(in crate::features) applications:
        SoundPayloadProductsitemConfigsProductDefaultprofileApplications,
    #[serde(rename = "calibration")]
    pub(in crate::features) calibration:
        SoundPayloadProductsitemConfigsProductDefaultprofileCalibration,
    #[serde(
        rename = "micAmbientNoiseReduction",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(in crate::features) mic_ambient_noise_reduction:
        Option<SoundPayloadProductsitemConfigsProductDefaultprofileMicambientnoisereduction>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemConfigs {
    #[serde(rename = "tutorial")]
    pub(in crate::features) tutorial: SoundPayloadProductsitemConfigsTutorial,
    #[serde(rename = "customizer")]
    pub(in crate::features) customizer: std::collections::BTreeMap<String, String>,
    #[serde(rename = "thx_switch")]
    pub(in crate::features) thx_switch: SoundPayloadProductsitemConfigsThxSwitch,
    #[serde(rename = "equalizer")]
    pub(in crate::features) equalizer: SoundPayloadProductsitemConfigsEqualizer,
    #[serde(rename = "applications")]
    pub(in crate::features) applications: SoundPayloadProductsitemConfigsApplications,
    #[serde(rename = "volume")]
    pub(in crate::features) volume: SoundPayloadProductsitemConfigsVolume,
    #[serde(rename = "output")]
    pub(in crate::features) output: SoundPayloadProductsitemConfigsOutput,
    #[serde(rename = "application_outputs")]
    pub(in crate::features) application_outputs:
        Vec<SoundPayloadProductsitemConfigsApplicationOutputsitem>,
    #[serde(rename = "renderer_geometry")]
    pub(in crate::features) renderer_geometry: SoundPayloadProductsitemConfigsRendererGeometry,
    #[serde(rename = "device_info")]
    pub(in crate::features) device_info: SoundPayloadProductsitemConfigsDeviceInfo,
    #[serde(rename = "application_thx_dataset")]
    pub(in crate::features) application_thx_dataset: std::collections::BTreeMap<
        String,
        SoundPayloadProductsitemConfigsApplicationThxDatasetentry,
    >,
    #[serde(rename = "customizer_groups")]
    pub(in crate::features) customizer_groups:
        Vec<SoundPayloadProductsitemConfigsCustomizerGroupsitem>,
    #[serde(rename = "support_overhead_speaker")]
    pub(in crate::features) support_overhead_speaker: bool,
    #[serde(rename = "mapping_exports")]
    pub(in crate::features) mapping_exports: Vec<String>,
    #[serde(rename = "mapping_action_types")]
    pub(in crate::features) mapping_action_types: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(rename = "mapping_audio_constants")]
    pub(in crate::features) mapping_audio_constants:
        SoundPayloadProductsitemConfigsMappingAudioConstants,
    #[serde(rename = "mapping_audio_options")]
    pub(in crate::features) mapping_audio_options:
        Vec<SoundPayloadProductsitemConfigsMappingAudioOptionsitem>,
    #[serde(rename = "mapping_assignment_table")]
    pub(in crate::features) mapping_assignment_table:
        Vec<SoundPayloadProductsitemConfigsMappingAssignmentTableitem>,
    #[serde(rename = "mapping_audio_notes")]
    pub(in crate::features) mapping_audio_notes: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(rename = "product_exports")]
    pub(in crate::features) product_exports: Vec<String>,
    #[serde(rename = "product_DEFAULTMAPPINGS")]
    pub(in crate::features) product_defaultmappings: Vec<String>,
    #[serde(rename = "product_DEFAULTPROFILE")]
    pub(in crate::features) product_defaultprofile:
        SoundPayloadProductsitemConfigsProductDefaultprofile,
    #[serde(rename = "product_DEFAULT_FIRMWARE_MAPPINGS")]
    pub(in crate::features) product_mappings: Vec<String>,
    #[serde(rename = "product_OBM_SUPPORTED_MAPPING_GROUPS")]
    pub(in crate::features) product_groups: Vec<String>,
    #[serde(rename = "product_SYNAPSE_NOT_REQUIRED_MAPPING_FUNCTIONS")]
    pub(in crate::features) product_functions: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemMedia {
    #[serde(rename = "editions")]
    pub(in crate::features) editions:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    #[serde(rename = "css")]
    pub(in crate::features) css: std::collections::BTreeMap<String, String>,
    #[serde(rename = "reset_link")]
    pub(in crate::features) reset_link: String,
    #[serde(rename = "eq_icons")]
    pub(in crate::features) eq_icons: std::collections::BTreeMap<String, String>,
    #[serde(rename = "sword-added-icon")]
    pub(in crate::features) sword_added_icon: String,
    #[serde(rename = "search-empty")]
    pub(in crate::features) search_empty: String,
    #[serde(rename = "icon_add")]
    pub(in crate::features) icon_add: String,
    #[serde(rename = "placeholder_game_icon")]
    pub(in crate::features) placeholder_game_icon: String,
    #[serde(rename = "icon_close_enclosed_small")]
    pub(in crate::features) icon_close_enclosed_small: String,
    #[serde(rename = "icon_close_enclosed_a_small")]
    pub(in crate::features) icon_close_enclosed_a_small: String,
    #[serde(rename = "appmix_gamemode")]
    pub(in crate::features) appmix_gamemode: String,
    #[serde(rename = "appmix_thx")]
    pub(in crate::features) appmix_thx: String,
    #[serde(rename = "thx_help")]
    pub(in crate::features) thx_help: String,
    #[serde(rename = "mapping_footstep")]
    pub(in crate::features) mapping_footstep: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemInitialsSpatial {
    #[serde(rename = "isEnabled")]
    pub(in crate::features) is_enabled: bool,
    #[serde(rename = "mode")]
    pub(in crate::features) mode: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemInitialsGameChat {
    #[serde(rename = "balanceMode")]
    pub(in crate::features) balance_mode: u64,
    #[serde(rename = "actionsFromUI")]
    pub(in crate::features) actions_from_ui: Vec<String>,
    #[serde(rename = "actionsFromLocalStorage")]
    pub(in crate::features) actions_from_local_storage: Vec<String>,
    #[serde(rename = "errorActions")]
    pub(in crate::features) error_actions: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemInitials {
    #[serde(rename = "spatial")]
    pub(in crate::features) spatial: SoundPayloadProductsitemInitialsSpatial,
    #[serde(rename = "volume")]
    pub(in crate::features) volume: std::collections::BTreeMap<String, String>,
    #[serde(rename = "game_chat")]
    pub(in crate::features) game_chat: SoundPayloadProductsitemInitialsGameChat,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemCloudInitialAvailableesportspresetsitemBandLabelIdsitem
{
    #[serde(rename = "label_id")]
    pub(in crate::features) label_id: i64,
    #[serde(rename = "band_index")]
    pub(in crate::features) band_index: Vec<u64>,
    #[serde(rename = "icon_start_index_position")]
    pub(in crate::features) icon_start_index_position: u64,
    #[serde(rename = "label_icon_position")]
    pub(in crate::features) label_icon_position: String,
    #[serde(rename = "label_arrow_position")]
    pub(in crate::features) label_arrow_position: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemCloudInitialAvailableesportspresetsitem {
    #[serde(rename = "equalizer_id")]
    pub(in crate::features) equalizer_id: u64,
    #[serde(rename = "equalizer_category")]
    pub(in crate::features) equalizer_category: u64,
    #[serde(rename = "equalizer_icon")]
    pub(in crate::features) equalizer_icon: String,
    #[serde(rename = "equalizer_svg_icon")]
    pub(in crate::features) equalizer_svg_icon: String,
    #[serde(rename = "is_oobe")]
    pub(in crate::features) is_oobe: bool,
    #[serde(
        rename = "is_oobe_enabled",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(in crate::features) is_oobe_enabled: Option<bool>,
    #[serde(rename = "is_allow_swapping")]
    pub(in crate::features) is_allow_swapping: bool,
    #[serde(rename = "equalizer_name")]
    pub(in crate::features) equalizer_name: std::collections::BTreeMap<String, String>,
    #[serde(rename = "band_values")]
    pub(in crate::features) band_values: Vec<i64>,
    #[serde(rename = "footstep_values")]
    pub(in crate::features) footstep_values: Vec<u64>,
    #[serde(rename = "band_label_ids")]
    pub(in crate::features) band_label_ids:
        Vec<SoundPayloadProductsitemCloudInitialAvailableesportspresetsitemBandLabelIdsitem>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::features) struct SoundPayloadProductsitemCloudInitial {
    #[serde(rename = "selectedPreset")]
    pub(in crate::features) selected_preset: u64,
    #[serde(rename = "enabledPresets")]
    pub(in crate::features) enabled_presets: Vec<String>,
    #[serde(rename = "disabledPresets")]
    pub(in crate::features) disabled_presets: Vec<String>,
    #[serde(rename = "tempEnabledPresets")]
    pub(in crate::features) temp_enabled_presets: Vec<String>,
    #[serde(rename = "tempDisabledPresets")]
    pub(in crate::features) temp_disabled_presets: Vec<String>,
    #[serde(rename = "tempActivePreset")]
    pub(in crate::features) temp_active_preset: (),
    #[serde(rename = "actionsFromUI")]
    pub(in crate::features) actions_from_ui: Vec<String>,
    #[serde(rename = "actionsFromLocalStorage")]
    pub(in crate::features) actions_from_local_storage: Vec<String>,
    #[serde(rename = "errorActions")]
    pub(in crate::features) error_actions: Vec<String>,
    #[serde(rename = "availableEsportsPresets")]
    pub(in crate::features) available_esports_presets:
        Vec<SoundPayloadProductsitemCloudInitialAvailableesportspresetsitem>,
}

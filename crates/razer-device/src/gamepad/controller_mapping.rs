//! Current 2636/2676 module 10511 -> 4442 assignment conversion and module
//! 11365 import filtering. Raw reads remain owned by the connection observation.
use crate::controller_profile::{AssignmentRead, ButtonAssignment, MappingObservation};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControllerInput(u8);
struct InputDefinition {
    firmware: u8,
    input: &'static str,
    assignment: Option<&'static str>,
}
const INPUTS: &[InputDefinition] = &[
    InputDefinition {
        firmware: 16,
        input: "BUTTON_M1",
        assignment: Some("M1_BUTTON"),
    },
    InputDefinition {
        firmware: 17,
        input: "BUTTON_M2",
        assignment: Some("M2_BUTTON"),
    },
    InputDefinition {
        firmware: 18,
        input: "BUTTON_M3",
        assignment: Some("M3_BUTTON"),
    },
    InputDefinition {
        firmware: 19,
        input: "BUTTON_M4",
        assignment: Some("M4_BUTTON"),
    },
    InputDefinition {
        firmware: 20,
        input: "BUTTON_M5",
        assignment: Some("M5_BUTTON"),
    },
    InputDefinition {
        firmware: 21,
        input: "BUTTON_M6",
        assignment: Some("M6_BUTTON"),
    },
    InputDefinition {
        firmware: 32,
        input: "BUTTON_A",
        assignment: Some("A_BUTTON"),
    },
    InputDefinition {
        firmware: 33,
        input: "BUTTON_B",
        assignment: Some("B_BUTTON"),
    },
    InputDefinition {
        firmware: 34,
        input: "BUTTON_X",
        assignment: Some("X_BUTTON"),
    },
    InputDefinition {
        firmware: 35,
        input: "BUTTON_Y",
        assignment: Some("Y_BUTTON"),
    },
    InputDefinition {
        firmware: 36,
        input: "LEFT_BUMPER",
        assignment: Some("LEFT_BUMPER"),
    },
    InputDefinition {
        firmware: 37,
        input: "RIGHT_BUMPER",
        assignment: Some("RIGHT_BUMPER"),
    },
    InputDefinition {
        firmware: 38,
        input: "LEFT_TRIGGER",
        assignment: Some("LEFT_TRIGGER"),
    },
    InputDefinition {
        firmware: 39,
        input: "RIGHT_TRIGGER",
        assignment: Some("RIGHT_TRIGGER"),
    },
    InputDefinition {
        firmware: 40,
        input: "LEFT_JS_BUTTON",
        assignment: Some("LEFT_JS_BUTTON"),
    },
    InputDefinition {
        firmware: 41,
        input: "RIGHT_JS_BUTTON",
        assignment: Some("RIGHT_JS_BUTTON"),
    },
    InputDefinition {
        firmware: 44,
        input: "DPAD_UP",
        assignment: Some("DPAD_UP"),
    },
    InputDefinition {
        firmware: 45,
        input: "DPAD_DOWN",
        assignment: Some("DPAD_DOWN"),
    },
    InputDefinition {
        firmware: 46,
        input: "DPAD_LEFT",
        assignment: Some("DPAD_LEFT"),
    },
    InputDefinition {
        firmware: 47,
        input: "DPAD_RIGHT",
        assignment: Some("DPAD_RIGHT"),
    },
    InputDefinition {
        firmware: 52,
        input: "VIEW_BUTTON",
        assignment: Some("VIEW_BUTTON"),
    },
    InputDefinition {
        firmware: 53,
        input: "MENU_BUTTON",
        assignment: Some("MENU_BUTTON"),
    },
    InputDefinition {
        firmware: 55,
        input: "LEFT_SENSITIVITY_CLUTCH",
        assignment: Some("LEFT_SENSITIVITY_CLUTCH"),
    },
    InputDefinition {
        firmware: 56,
        input: "RIGHT_SENSITIVITY_CLUTCH",
        assignment: Some("RIGHT_SENSITIVITY_CLUTCH"),
    },
    InputDefinition {
        firmware: 58,
        input: "GLOBAL_SENSITIVITY_CLUTCH",
        assignment: Some("GLOBAL_SENSITIVITY_CLUTCH"),
    },
    InputDefinition {
        firmware: 61,
        input: "SHARE_BUTTON",
        assignment: None,
    },
    InputDefinition {
        firmware: 128,
        input: "LEFT_JS_UP",
        assignment: Some("LEFT_JS_UP"),
    },
    InputDefinition {
        firmware: 129,
        input: "LEFT_JS_DOWN",
        assignment: Some("LEFT_JS_DOWN"),
    },
    InputDefinition {
        firmware: 130,
        input: "LEFT_JS_LEFT",
        assignment: Some("LEFT_JS_LEFT"),
    },
    InputDefinition {
        firmware: 131,
        input: "LEFT_JS_RIGHT",
        assignment: Some("LEFT_JS_RIGHT"),
    },
    InputDefinition {
        firmware: 132,
        input: "RIGHT_JS_UP",
        assignment: Some("RIGHT_JS_UP"),
    },
    InputDefinition {
        firmware: 133,
        input: "RIGHT_JS_DOWN",
        assignment: Some("RIGHT_JS_DOWN"),
    },
    InputDefinition {
        firmware: 134,
        input: "RIGHT_JS_LEFT",
        assignment: Some("RIGHT_JS_LEFT"),
    },
    InputDefinition {
        firmware: 135,
        input: "RIGHT_JS_RIGHT",
        assignment: Some("RIGHT_JS_RIGHT"),
    },
];
impl ControllerInput {
    pub fn from_firmware(firmware: u8) -> anyhow::Result<Self> {
        ensure!(
            INPUTS.iter().any(|input| input.firmware == firmware),
            "Unknown controller input {firmware}"
        );
        Ok(Self(firmware))
    }
    pub fn firmware(self) -> u8 {
        self.0
    }
    fn definition(self) -> &'static InputDefinition {
        INPUTS
            .iter()
            .find(|input| input.firmware == self.0)
            .expect("validated controller input")
    }
}
impl Serialize for ControllerInput {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.definition().input)
    }
}
impl<'de> Deserialize<'de> for ControllerInput {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        INPUTS
            .iter()
            .find(|input| input.input == value)
            .map(|input| Self(input.firmware))
            .ok_or_else(|| D::Error::custom("Unknown controller input identity"))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControllerTarget(ControllerInput);
impl Serialize for ControllerTarget {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(
            self.0
                .definition()
                .assignment
                .expect("validated controller assignment"),
        )
    }
}
impl<'de> Deserialize<'de> for ControllerTarget {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        INPUTS
            .iter()
            .find(|input| input.assignment == Some(value.as_str()))
            .map(|input| Self(ControllerInput(input.firmware)))
            .ok_or_else(|| D::Error::custom("Unknown controller assignment identity"))
    }
}

/// Identity is the HID from the current generic KeyInput rows, not a UI index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardKey(u8);
const KEYS: &[(u8, &str)] = &[
    (4, "KEY_A"),
    (5, "KEY_B"),
    (6, "KEY_C"),
    (7, "KEY_D"),
    (8, "KEY_E"),
    (9, "KEY_F"),
    (10, "KEY_G"),
    (11, "KEY_H"),
    (12, "KEY_I"),
    (13, "KEY_J"),
    (14, "KEY_K"),
    (15, "KEY_L"),
    (16, "KEY_M"),
    (17, "KEY_N"),
    (18, "KEY_O"),
    (19, "KEY_P"),
    (20, "KEY_Q"),
    (21, "KEY_R"),
    (22, "KEY_S"),
    (23, "KEY_T"),
    (24, "KEY_U"),
    (25, "KEY_V"),
    (26, "KEY_W"),
    (27, "KEY_X"),
    (28, "KEY_Y"),
    (29, "KEY_Z"),
    (30, "KEY_1"),
    (31, "KEY_2"),
    (32, "KEY_3"),
    (33, "KEY_4"),
    (34, "KEY_5"),
    (35, "KEY_6"),
    (36, "KEY_7"),
    (37, "KEY_8"),
    (38, "KEY_9"),
    (39, "KEY_0"),
    (40, "KEY_ENTER"),
    (41, "KEY_ESC"),
    (42, "KEY_BACKSPACE"),
    (43, "KEY_TAB"),
    (44, "KEY_SPACEBAR"),
    (45, "KEY_HYPEN"),
    (46, "KEY_EQUAL"),
    (47, "KEY_OPEN_SQUARE_BRACKET"),
    (48, "KEY_CLOSE_SQUARE_BRACKET"),
    (49, "KEY_BACKSLASH"),
    (50, "KEY_NON_US_POUND"),
    (51, "KEY_SEMICOLON"),
    (52, "KEY_APOSTROPHE"),
    (53, "KEY_TILDE"),
    (54, "KEY_COMMA"),
    (55, "KEY_PERIOD"),
    (56, "KEY_SLASH"),
    (57, "KEY_CAPS_LOCK"),
    (58, "KEY_F1"),
    (59, "KEY_F2"),
    (60, "KEY_F3"),
    (61, "KEY_F4"),
    (62, "KEY_F5"),
    (63, "KEY_F6"),
    (64, "KEY_F7"),
    (65, "KEY_F8"),
    (66, "KEY_F9"),
    (67, "KEY_F10"),
    (68, "KEY_F11"),
    (69, "KEY_F12"),
    (70, "KEY_PRINT_SCREEN"),
    (71, "KEY_SCROLL_LOCK"),
    (72, "KEY_PAUSE"),
    (73, "KEY_INSERT"),
    (74, "KEY_HOME"),
    (75, "KEY_PAGE_UP"),
    (76, "KEY_DELETE"),
    (77, "KEY_END"),
    (78, "KEY_PAGE_DOWN"),
    (79, "KEY_RIGHT_ARROW"),
    (80, "KEY_LEFT_ARROW"),
    (81, "KEY_DOWN_ARROW"),
    (82, "KEY_UP_ARROW"),
    (83, "KEY_NUMPAD_NUM_LOCK"),
    (84, "KEY_NUMPAD_SLASH"),
    (85, "KEY_NUMPAD_ASTERISK"),
    (86, "KEY_NUMPAD_DASH"),
    (87, "KEY_NUMPAD_PLUS"),
    (88, "KEY_NUMPAD_ENTER"),
    (89, "KEY_NUMPAD_1"),
    (90, "KEY_NUMPAD_2"),
    (91, "KEY_NUMPAD_3"),
    (92, "KEY_NUMPAD_4"),
    (93, "KEY_NUMPAD_5"),
    (94, "KEY_NUMPAD_6"),
    (95, "KEY_NUMPAD_7"),
    (96, "KEY_NUMPAD_8"),
    (97, "KEY_NUMPAD_9"),
    (98, "KEY_NUMPAD_0"),
    (99, "KEY_NUMPAD_PERIOD"),
    (100, "KEY_NON_US_BACKSLASH"),
    (101, "KEY_APPLICATION"),
    (103, "KEY_NUMPAD_EQUAL"),
    (104, "KEY_F13"),
    (105, "KEY_F14"),
    (106, "KEY_F15"),
    (107, "KEY_F16"),
    (108, "KEY_F17"),
    (109, "KEY_F18"),
    (110, "KEY_F19"),
    (111, "KEY_F20"),
    (112, "KEY_F21"),
    (113, "KEY_F22"),
    (114, "KEY_F23"),
    (115, "KEY_F24"),
    (135, "KEY_RO"),
    (136, "KEY_KATAKANA_HIRAGANA"),
    (137, "KEY_YEN"),
    (138, "KEY_HENKAN"),
    (139, "KEY_MUHENKAN"),
    (144, "KEY_HANGEUL"),
    (145, "KEY_HANJA"),
    (224, "KEY_LEFT_CTRL"),
    (225, "KEY_LEFT_SHIFT"),
    (226, "KEY_LEFT_ALT"),
    (227, "KEY_LEFT_GUI"),
    (228, "KEY_RIGHT_CTRL"),
    (229, "KEY_RIGHT_SHIFT"),
    (230, "KEY_RIGHT_ALT"),
    (231, "KEY_RIGHT_GUI"),
];
impl KeyboardKey {
    fn from_hid(hid: u8) -> anyhow::Result<Self> {
        ensure!(
            KEYS.iter().any(|key| key.0 == hid),
            "Controller mapping has unknown keyboard HID {hid}"
        );
        Ok(Self(hid))
    }
    fn extended(self) -> bool {
        matches!(self.0, 70 | 73..=84 | 88 | 101 | 227 | 228 | 230 | 231)
    }
}
impl Serialize for KeyboardKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(
            KEYS.iter()
                .find(|key| key.0 == self.0)
                .expect("validated keyboard identity")
                .1,
        )
    }
}
impl<'de> Deserialize<'de> for KeyboardKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        KEYS.iter()
            .find(|key| key.1 == value)
            .map(|key| Self(key.0))
            .ok_or_else(|| D::Error::custom("Unknown controller keyboard identity"))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Modifier {
    #[serde(rename = "Extended_Key")]
    Extended,
    #[serde(rename = "Left_Alt")]
    LeftAlt,
    #[serde(rename = "Left_Ctrl")]
    LeftCtrl,
    #[serde(rename = "Left_Shift")]
    LeftShift,
    #[serde(rename = "Right_Alt")]
    RightAlt,
    #[serde(rename = "Right_Ctrl")]
    RightCtrl,
    #[serde(rename = "Right_Shift")]
    RightShift,
    #[serde(rename = "Left_GUI")]
    LeftGui,
    #[serde(rename = "Right_GUI")]
    RightGui,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ExtendedKey {
    #[serde(rename = "Extended_00")]
    Extended00,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TurboMode {
    pub is_turbo: bool,
    pub keys_per_second: u16,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyboardGroup {
    pub key: KeyboardKey,
    pub modifiers: Vec<Modifier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extended_key: Option<ExtendedKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turbo_mode: Option<TurboMode>,
    /// Local keyboard-layout metadata; never claimed as a firmware observation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_group: Option<u32>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum MouseAssignment {
    Click,
    Menu,
    ScrollButton,
    Previous,
    Next,
    ScrollUp,
    ScrollDown,
    DoubleClick,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MouseGroup {
    pub mouse_assignment: MouseAssignment,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turbo_mode: Option<TurboMode>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum SensitivityAssignment {
    #[serde(rename = "DPI_Up")]
    Up,
    #[serde(rename = "DPI_Down")]
    Down,
    #[serde(rename = "DPI_CycleUp")]
    CycleUp,
    #[serde(rename = "DPI_CycleDown")]
    CycleDown,
    #[serde(rename = "DPI_Clutch")]
    Clutch,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SensitivityGroup {
    pub sensitivity_assignment: SensitivityAssignment,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<u16>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ProfileNavigation {
    Up,
    Down,
    Specific,
    CycleUp,
    CycleDown,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProfileNavigationGroup {
    pub profile_navigation_assignment: ProfileNavigation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<u8>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum PowerKey {
    SystemPower,
    SystemSleep,
    SystemWake,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Multimedia {
    MediaEject,
    MuteVolume,
    VolumeDown,
    VolumeUp,
    PrevTrack,
    MediaStop,
    Play,
    NextTrack,
    CycleApps,
    DisplayBrightnessUp,
    DisplayBrightnessDown,
    SpotlightSearch,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum WindowsShortcut {
    Screenshot,
    XboxGameBar,
    CycleApps,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum MacroPlayback {
    Once,
    NTimes,
    ContinuousHeld,
    ContinuousToggle,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MacroGroup {
    /// Module 10511 has no macro directory; module 4442 returns an empty id.
    pub id: String,
    pub macro_playback_option: MacroPlayback,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_count: Option<u8>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Direction {
    #[serde(rename = "right")]
    Right,
    #[serde(rename = "left")]
    Left,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Perfect180Group {
    pub mouse_assignment: Perfect180Assignment,
    pub direction: Direction,
    pub report_count: u16,
    /// Current source retains the numeric protocol byte, including non-bools.
    pub is_enabled: u8,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Perfect180Assignment {
    Perfect180,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EmptyGroup {}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "outputType", rename_all_fields = "camelCase")]
pub enum MappingOutput {
    #[serde(rename = "disableGroup")]
    Disabled,
    #[serde(rename = "keyboardGroup")]
    Keyboard { keyboard_group: KeyboardGroup },
    #[serde(rename = "mouseGroup")]
    Mouse { mouse_group: MouseGroup },
    #[serde(rename = "controllerGroup")]
    Controller { controller_group: ControllerGroup },
    #[serde(rename = "sensitivityGroup")]
    Sensitivity { sensitivity_group: SensitivityGroup },
    #[serde(rename = "profileNavigationGroup")]
    Profile {
        profile_navigation_group: ProfileNavigationGroup,
    },
    #[serde(rename = "powerKeyGroup")]
    Power { power_key_group: PowerKeyGroup },
    #[serde(rename = "multimediaGroup")]
    Multimedia { multimedia_group: MultimediaGroup },
    #[serde(rename = "win8ShortcutsGroup")]
    Windows {
        win8_shortcuts_group: WindowsShortcutGroup,
    },
    #[serde(rename = "razerKeyGroup")]
    Razer { razer_key_group: RazerKeyGroup },
    #[serde(rename = "macroGroup")]
    Macro { macro_group: MacroGroup },
    // The source ModeButton branch leaves mappingGroupName="".
    #[serde(rename = "hyperShiftGroup")]
    HyperShift {
        #[serde(rename = "")]
        empty: EmptyGroup,
    },
    #[serde(rename = "perfect180Group")]
    Perfect180 { perfect180_group: Perfect180Group },
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControllerGroup {
    pub controller_assignment: ControllerTarget,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PowerKeyGroup {
    pub power_key_assignment: PowerKey,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MultimediaGroup {
    pub multimedia_assignment: Multimedia,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowsShortcutGroup {
    pub windows_shortcut_assignment: WindowsShortcut,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RazerKeyGroup {
    pub razer_key_assignment: RazerKey,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RazerKey(u8);
const RAZER_KEYS: &[(u8, &str)] = &[
    (1, "FnKey"),
    (2, "RazerKey"),
    (3, "GameModeKey"),
    (4, "MacroRecordKey"),
    (5, "DisplayInternalExternal"),
    (6, "DisplayBrightnessUp"),
    (7, "DisplayBrightnessDown"),
    (8, "KeyboardBacklightUp"),
    (9, "KeyboardBacklightDown"),
    (11, "ToggleLowPowerMode"),
    (32, "MouseDPIUpButton"),
    (33, "MouseDPIDownButton"),
    (34, "MouseRightBackwardButton"),
    (35, "MouseRightForwardButton"),
    (36, "MacroKeys"),
    (70, "PrintScreen"),
    (76, "SystemSleep"),
    (79, "MacroKeys_End"),
    (80, "ProfileSwitchButton"),
    (81, "MouseSniperButton"),
    (82, "MouseDPICycleUpButton"),
    (83, "MouseDPICycleDownButton"),
    (84, "MouseScrollWheelFreeNotchSwitch"),
    (85, "HapticScrollStageUp"),
    (86, "HapticScrollStageDown"),
    (87, "HapticScrollStageCycleUp"),
    (88, "HapticScrollStageCycleDown"),
    (89, "HyperShiftButton"),
    (112, "ActuationAdjustmentMode"),
    (113, "RapidTriggerAdjustmentMode"),
    (117, "SnapTapToggle"),
    (216, "MicMute"),
];
impl Serialize for RazerKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(
            RAZER_KEYS
                .iter()
                .find(|key| key.0 == self.0)
                .expect("validated Razer key")
                .1,
        )
    }
}
impl<'de> Deserialize<'de> for RazerKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        RAZER_KEYS
            .iter()
            .find(|key| key.1 == value)
            .map(|key| Self(key.0))
            .ok_or_else(|| D::Error::custom("Unknown controller Razer key"))
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum InputType {
    ControllerInput,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceMapping {
    #[serde(rename = "inputID")]
    pub input_id: ControllerInput,
    pub input_type: InputType,
    pub controller_input: ControllerInput,
    pub is_hyper_shift: bool,
    #[serde(flatten)]
    pub output: MappingOutput,
}
impl SourceMapping {
    /// This describes source cache resolution, not SET/readback confirmation.
    pub fn has_resolved_output(&self) -> bool {
        !matches!(self.output,MappingOutput::Macro { ref macro_group } if macro_group.id.is_empty())
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalKeyboardMetadata {
    #[serde(rename = "inputID")]
    pub input_id: ControllerInput,
    pub is_hyper_shift: bool,
    pub keyboard_group: LocalKeyboardGroup,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalKeyboardGroup {
    pub key: KeyboardKey,
    pub key_group: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MappingRow {
    OmittedSelf { button_id: u8 },
    OmittedDefault { mapping: SourceMapping },
    Imported { mapping: SourceMapping },
    Failed { button_id: u8, error: String },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappingProjection {
    pub rows: Vec<MappingRow>,
    pub canceled: bool,
}
impl MappingProjection {
    pub fn all_rows_observed(&self) -> bool {
        !self.canceled
            && !self
                .rows
                .iter()
                .any(|row| matches!(row, MappingRow::Failed { .. }))
    }
    /// Module 10511 continues after a single-button error, then module 11365
    /// replaces the source cache with that partial array. Failed rows stay in
    /// this projection and the raw observation; absence is not a default read.
    pub fn source_mappings(&self) -> anyhow::Result<Vec<SourceMapping>> {
        ensure!(!self.canceled, "Controller mapping conversion was canceled");
        let mut mappings = Vec::new();
        for row in &self.rows {
            match row {
                MappingRow::Imported { mapping } => mappings.push(mapping.clone()),
                MappingRow::Failed { .. }
                | MappingRow::OmittedSelf { .. }
                | MappingRow::OmittedDefault { .. } => {}
            }
        }
        Ok(mappings)
    }
}

pub fn project_mappings(
    product_id: u32,
    observation: &MappingObservation,
    local: &[LocalKeyboardMetadata],
) -> anyhow::Result<MappingProjection> {
    ensure!(
        matches!(product_id, 2636 | 2676),
        "No verified controller mapping conversion for {product_id}"
    );
    let mut rows = Vec::new();
    for button in &observation.buttons {
        let row = match &button.result {
            AssignmentRead::Failed { error } => MappingRow::Failed {
                button_id: button.button_id,
                error: error.clone(),
            },
            AssignmentRead::Observed { assignment } => {
                assignment.validate_for(observation.profile_slot, button.button_id)?;
                if assignment.source_omits_assignment() {
                    MappingRow::OmittedSelf {
                        button_id: button.button_id,
                    }
                } else {
                    match convert_assignment(product_id, assignment) {
                        Ok(mut mapping) => {
                            if let MappingOutput::Keyboard { keyboard_group } = &mut mapping.output
                            {
                                if let Some(previous) = local.iter().find(|previous| {
                                    previous.input_id == mapping.input_id
                                        && previous.is_hyper_shift == mapping.is_hyper_shift
                                        && previous.keyboard_group.key == keyboard_group.key
                                }) {
                                    keyboard_group.key_group = previous.keyboard_group.key_group;
                                }
                            }
                            if source_default_omits(product_id, &mapping) {
                                MappingRow::OmittedDefault { mapping }
                            } else {
                                MappingRow::Imported { mapping }
                            }
                        }
                        Err(error) => MappingRow::Failed {
                            button_id: button.button_id,
                            error: format!("{error:#}"),
                        },
                    }
                }
            }
        };
        rows.push(row);
    }
    Ok(MappingProjection {
        rows,
        canceled: observation.canceled,
    })
}

fn source_default_omits(product_id: u32, mapping: &SourceMapping) -> bool {
    match &mapping.output {
        MappingOutput::Controller { controller_group }
            if (16..=21).contains(&mapping.input_id.0) =>
        {
            controller_group.controller_assignment.0 == mapping.input_id
        }
        // Module 11365 compares non-controller defaults only by input/output
        // type. It does not compare SHARE's keyboard key or modifiers.
        MappingOutput::Keyboard { .. } if product_id == 2676 && mapping.input_id.0 == 61 => true,
        _ => false,
    }
}
fn word(high: u8, low: u8) -> u16 {
    u16::from_be_bytes([high, low])
}
fn turbo(high: u8, low: u8) -> anyhow::Result<TurboMode> {
    let period = word(high, low);
    ensure!(
        period != 0,
        "Controller turbo period is zero; source JSON has a null rate"
    );
    let rate = (1000.0 / f64::from(period)).round() as u16;
    Ok(TurboMode {
        is_turbo: true,
        keys_per_second: rate,
    })
}
fn keyboard(data: [u8; 5], is_turbo: bool) -> anyhow::Result<KeyboardGroup> {
    let mut hid = data[1];
    if !is_turbo && hid == 0 && data[0] > 0 {
        hid = match data[0] {
            1 => 224,
            2 => 225,
            4 => 226,
            8 => 227,
            16 => 228,
            32 => 229,
            64 => 230,
            _ => 0,
        };
    }
    let key = KeyboardKey::from_hid(hid)?;
    let extended = key.extended();
    let mut modifiers = if extended {
        vec![Modifier::Extended]
    } else {
        Vec::new()
    };
    if is_turbo || data[0] == 0 || data[1] != 0 {
        for (mask, modifier) in [
            (4, Modifier::LeftAlt),
            (1, Modifier::LeftCtrl),
            (2, Modifier::LeftShift),
            (64, Modifier::RightAlt),
            (16, Modifier::RightCtrl),
            (32, Modifier::RightShift),
            (8, Modifier::LeftGui),
            (128, Modifier::RightGui),
        ] {
            if data[0] & mask != 0 {
                modifiers.push(modifier);
            }
        }
    }
    Ok(KeyboardGroup {
        key,
        modifiers,
        extended_key: extended.then_some(ExtendedKey::Extended00),
        turbo_mode: if is_turbo {
            Some(turbo(data[2], data[3])?)
        } else {
            None
        },
        key_group: None,
    })
}
fn mouse(raw: u8) -> anyhow::Result<MouseAssignment> {
    Ok(match raw {
        1 => MouseAssignment::Click,
        2 => MouseAssignment::Menu,
        3 => MouseAssignment::ScrollButton,
        4 | 6 => MouseAssignment::Previous,
        5 | 7 => MouseAssignment::Next,
        9 | 52 => MouseAssignment::ScrollUp,
        10 | 53 => MouseAssignment::ScrollDown,
        _ => anyhow::bail!("Controller mouse output {raw} has no generic mouseGroup"),
    })
}
pub fn convert_assignment(
    product_id: u32,
    assignment: &ButtonAssignment,
) -> anyhow::Result<SourceMapping> {
    ensure!(
        matches!(product_id, 2636 | 2676),
        "No verified controller mapping product branch"
    );
    ensure!(
        assignment.mode == 0,
        "Controller importer only reads normal mode"
    );
    assignment.function_data()?;
    let input = ControllerInput::from_firmware(assignment.button_id)?;
    let data = assignment.data;
    let output = match assignment.function_id {
        0 => MappingOutput::Disabled,
        1 | 14 => MappingOutput::Mouse {
            mouse_group: MouseGroup {
                mouse_assignment: mouse(data[0])?,
                turbo_mode: if assignment.function_id == 14 {
                    Some(turbo(data[1], data[2])?)
                } else {
                    None
                },
            },
        },
        2 | 13 => MappingOutput::Keyboard {
            keyboard_group: keyboard(data, assignment.function_id == 13)?,
        },
        3..=5 => {
            let (macro_playback_option, repeat_count) = match assignment.function_id {
                3 => (
                    if data[2] == 1 {
                        MacroPlayback::Once
                    } else {
                        MacroPlayback::NTimes
                    },
                    Some(data[2]),
                ),
                4 => (MacroPlayback::ContinuousHeld, None),
                _ => (MacroPlayback::ContinuousToggle, None),
            };
            MappingOutput::Macro {
                macro_group: MacroGroup {
                    id: String::new(),
                    macro_playback_option,
                    repeat_count,
                },
            }
        }
        6 => {
            let sensitivity_assignment = match data[0] {
                1 => SensitivityAssignment::Up,
                2 => SensitivityAssignment::Down,
                6 => SensitivityAssignment::CycleUp,
                7 => SensitivityAssignment::CycleDown,
                5 => SensitivityAssignment::Clutch,
                _ => anyhow::bail!("Unknown controller sensitivity output {}", data[0]),
            };
            let (x, y) = if data[0] == 5 {
                (word(data[1], data[2]), word(data[3], data[4]))
            } else {
                (0, 0)
            };
            MappingOutput::Sensitivity {
                sensitivity_group: SensitivityGroup {
                    sensitivity_assignment,
                    x: (x != 0).then_some(x),
                    y: (y != 0).then_some(y),
                },
            }
        }
        7 => {
            let profile_navigation_assignment = match data[0] {
                1 => ProfileNavigation::Up,
                2 => ProfileNavigation::Down,
                3 => ProfileNavigation::Specific,
                4 => ProfileNavigation::CycleUp,
                5 => ProfileNavigation::CycleDown,
                _ => anyhow::bail!("Unknown controller profile navigation {}", data[0]),
            };
            MappingOutput::Profile {
                profile_navigation_group: ProfileNavigationGroup {
                    profile_navigation_assignment,
                    profile_id: (data[0] == 3 && data[1] != 0).then_some(data[1]),
                },
            }
        }
        9 => MappingOutput::Power {
            power_key_group: PowerKeyGroup {
                power_key_assignment: match data[0] {
                    129 => PowerKey::SystemPower,
                    130 => PowerKey::SystemSleep,
                    131 => PowerKey::SystemWake,
                    _ => anyhow::bail!("Unknown controller power key {}", data[0]),
                },
            },
        },
        10 => {
            // Engine swaps the pairs before module 4442's big-endian decode.
            let multimedia_assignment = match word(data[1], data[0]) {
                184 => Multimedia::MediaEject,
                226 => Multimedia::MuteVolume,
                234 => Multimedia::VolumeDown,
                233 => Multimedia::VolumeUp,
                182 => Multimedia::PrevTrack,
                183 => Multimedia::MediaStop,
                205 => Multimedia::Play,
                181 => Multimedia::NextTrack,
                671 => Multimedia::CycleApps,
                111 => Multimedia::DisplayBrightnessUp,
                112 => Multimedia::DisplayBrightnessDown,
                545 => Multimedia::SpotlightSearch,
                raw => anyhow::bail!("Unknown controller multimedia output {raw}"),
            };
            MappingOutput::Multimedia {
                multimedia_group: MultimediaGroup {
                    multimedia_assignment,
                },
            }
        }
        11 => MappingOutput::Mouse {
            mouse_group: MouseGroup {
                mouse_assignment: MouseAssignment::DoubleClick,
                turbo_mode: None,
            },
        },
        12 => MappingOutput::HyperShift {
            empty: EmptyGroup {},
        },
        16 => {
            let target = ControllerInput::from_firmware(data[0])?;
            target
                .definition()
                .assignment
                .context("Generic controller output has no controllerGroup")?;
            MappingOutput::Controller {
                controller_group: ControllerGroup {
                    controller_assignment: ControllerTarget(target),
                },
            }
        }
        17 => {
            ensure!(
                RAZER_KEYS.iter().any(|key| key.0 == data[0]),
                "Unknown controller Razer key {}",
                data[0]
            );
            MappingOutput::Razer {
                razer_key_group: RazerKeyGroup {
                    razer_key_assignment: RazerKey(data[0]),
                },
            }
        }
        21 => MappingOutput::Windows {
            win8_shortcuts_group: WindowsShortcutGroup {
                windows_shortcut_assignment: match data[0] {
                    99 => WindowsShortcut::Screenshot,
                    100 => WindowsShortcut::XboxGameBar,
                    101 => WindowsShortcut::CycleApps,
                    _ => anyhow::bail!("Unknown controller Windows shortcut {}", data[0]),
                },
            },
        },
        22 if product_id == 2676 => MappingOutput::Perfect180 {
            perfect180_group: Perfect180Group {
                mouse_assignment: Perfect180Assignment::Perfect180,
                direction: match data[0] {
                    0 => Direction::Right,
                    1 => Direction::Left,
                    _ => anyhow::bail!("Unknown Perfect180 direction {}", data[0]),
                },
                report_count: word(data[1], data[2]),
                is_enabled: data[3],
            },
        },
        raw => anyhow::bail!(
            "Controller function {raw} has no current source conversion for {product_id}"
        ),
    };
    Ok(SourceMapping {
        input_id: input,
        input_type: InputType::ControllerInput,
        controller_input: input,
        is_hyper_shift: false,
        output,
    })
}

#[cfg(test)]
#[path = "controller_mapping_tests.rs"]
mod tests;

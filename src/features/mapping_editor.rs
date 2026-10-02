//! Source: 182 QP → 5107 and Map* chunks; 653 → 3241/2667.
//! These are local profile assignments, never a native Synapse wire format.
use super::*;
use crate::i18n;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    checkbox::Checkbox,
    input::{Input, Textarea},
    radio::Radio,
};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "local-mapping:v1:";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Assignment {
    Default,
    Keyboard {
        key: String,
        modifiers: Vec<String>,
        turbo: Option<u32>,
    },
    Mouse {
        action: String,
        turbo: Option<u32>,
    },
    Sensitivity {
        action: String,
        x: u32,
        y: u32,
        independent: bool,
    },
    Multimedia {
        action: String,
    },
    Brightness {
        action: String,
    },
    Windows {
        action: String,
    },
    Text {
        text: String,
    },
    Profile {
        action: String,
    },
    Launch {
        mode: String,
        target: String,
    },
    Hypershift,
    Disable,
    Unavailable {
        category: String,
        original: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    Default,
    Keyboard,
    Mouse,
    Sensitivity,
    Macro,
    Interdevice,
    Profile,
    Lighting,
    Brightness,
    Hypershift,
    Launch,
    Multimedia,
    Windows,
    Text,
    Disable,
}

impl Category {
    fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Keyboard => "keyboard",
            Self::Mouse => "mouse",
            Self::Sensitivity => "sensitivity",
            Self::Macro => "macro",
            Self::Interdevice => "interdevice",
            Self::Profile => "profile",
            Self::Lighting => "lighting",
            Self::Hypershift => "hypershift",
            Self::Launch => "launch",
            Self::Multimedia => "multimedia",
            Self::Windows => "windows",
            Self::Brightness => "brightness",
            Self::Text => "text",
            Self::Disable => "disable",
        }
    }
    fn capability(self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::Keyboard => "KEYBOARD_FUNCTION",
            Self::Mouse => "MOUSE_FUNCTION",
            Self::Sensitivity => "SENSITIVITY",
            Self::Macro => "MACRO",
            Self::Interdevice => "INTERDEVICE",
            Self::Profile => "SWITCH_PROFILE",
            Self::Lighting => "SWITCH_LIGHTING",
            Self::Hypershift => "RAZER_HYPERSHIFT",
            Self::Launch => "LAUNCH_PROGRAM",
            Self::Multimedia => "MULTIMEDIA",
            Self::Windows => "WINDOWS_SHORTCUT",
            Self::Brightness => "DEVICE_BRIGHTNESS",
            Self::Text => "TEXT_FUNCTION",
            Self::Disable => "DISABLE",
        }
    }
    fn label(self) -> String {
        i18n::t(self.capability())
    }
    fn all() -> [Self; 15] {
        [
            Self::Default,
            Self::Keyboard,
            Self::Mouse,
            Self::Sensitivity,
            Self::Macro,
            Self::Interdevice,
            Self::Profile,
            Self::Lighting,
            Self::Brightness,
            Self::Hypershift,
            Self::Launch,
            Self::Multimedia,
            Self::Windows,
            Self::Text,
            Self::Disable,
        ]
    }
    fn initial(self) -> Assignment {
        match self {
            Self::Default => Assignment::Default,
            Self::Keyboard => Assignment::Keyboard {
                key: String::new(),
                modifiers: vec![],
                turbo: None,
            },
            Self::Mouse => Assignment::Mouse {
                action: "Click".into(),
                turbo: None,
            },
            Self::Sensitivity => Assignment::Sensitivity {
                action: "DPI_Clutch".into(),
                x: 800,
                y: 800,
                independent: false,
            },
            Self::Multimedia => Assignment::Multimedia {
                action: "VolumeDown".into(),
            },
            Self::Brightness => Assignment::Brightness {
                action: "BrightnessUp".into(),
            },
            Self::Windows => Assignment::Windows {
                action: "Calculator".into(),
            },
            Self::Text => Assignment::Text {
                text: String::new(),
            },
            Self::Profile => Assignment::Profile {
                action: "NextProfile".into(),
            },
            Self::Launch => Assignment::Launch {
                mode: "program".into(),
                target: String::new(),
            },
            Self::Hypershift => Assignment::Hypershift,
            Self::Disable => Assignment::Disable,
            _ => Assignment::Unavailable {
                category: self.id().into(),
                original: String::new(),
            },
        }
    }
}

impl Assignment {
    fn decode(value: &str) -> Self {
        if let Some(json) = value.strip_prefix(PREFIX)
            && let Ok(assignment) = serde_json::from_str(json)
        {
            return assignment;
        }
        match value {
            "default" => Self::Default,
            "disable" => Self::Disable,
            "Hypershift" => Self::Hypershift,
            "keyboard" => Category::Keyboard.initial(),
            "LeftButton" | "LeftClick" => Self::Mouse {
                action: "Click".into(),
                turbo: None,
            },
            "RightButton" | "RightClick" => Self::Mouse {
                action: "Menu".into(),
                turbo: None,
            },
            "MiddleButton" | "ScrollClick" | "ScrollButton" => Self::Mouse {
                action: "ScrollButton".into(),
                turbo: None,
            },
            "Button4" | "Button5" => Self::Mouse {
                action: if value == "Button4" {
                    "Previous"
                } else {
                    "Next"
                }
                .into(),
                turbo: None,
            },
            "ScrollUp" | "ScrollDown" => Self::Mouse {
                action: value.into(),
                turbo: None,
            },
            "SensitivityStageUp" | "SensitivityStageDown" => Self::Sensitivity {
                action: if value.ends_with("Up") {
                    "DPI_Up"
                } else {
                    "DPI_Down"
                }
                .into(),
                x: 800,
                y: 800,
                independent: false,
            },
            _ if value.starts_with("keyboard:") => {
                let chord = &value[9..];
                let literal_plus = chord.ends_with('+');
                let mut parts = chord
                    .trim_end_matches('+')
                    .split('+')
                    .map(str::trim)
                    .collect::<Vec<_>>();
                if literal_plus {
                    parts.push("+");
                }
                let key = parts.pop().unwrap_or_default();
                let implicit_shift = SHIFTED_SYMBOLS.iter().any(|(symbol, _)| *symbol == key);
                let key = canonical_key(key).unwrap_or_else(|| key.to_string());
                let mut modifiers: Vec<String> = parts
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .map(|p| {
                        match p.to_ascii_lowercase().as_str() {
                            "ctrl" | "control" => "KEY_LEFT_CTRL",
                            "alt" => "KEY_LEFT_ALT",
                            "shift" => "KEY_LEFT_SHIFT",
                            "win" | "super" => "KEY_LEFT_GUI",
                            _ => p,
                        }
                        .to_string()
                    })
                    .collect();
                if implicit_shift
                    && !modifiers
                        .iter()
                        .any(|modifier| modifier_family(modifier) == "SHIFT")
                {
                    modifiers.push("KEY_LEFT_SHIFT".into());
                }
                Self::Keyboard {
                    key,
                    modifiers,
                    turbo: None,
                }
            }
            _ if MOUSE
                .iter()
                .chain(REPEAT_SCROLL)
                .any(|(id, _)| *id == value) =>
            {
                Self::Mouse {
                    action: value.into(),
                    turbo: None,
                }
            }
            "CycleUpSensitivityStages" | "CycleDownSensitivityStages" => Self::Sensitivity {
                action: if value == "CycleUpSensitivityStages" {
                    "DPI_CycleUp"
                } else {
                    "DPI_CycleDown"
                }
                .into(),
                x: 800,
                y: 800,
                independent: false,
            },
            _ => Self::Unavailable {
                category: "legacy".into(),
                original: value.into(),
            },
        }
    }
    fn encode(&self) -> String {
        match self {
            Self::Default => "default".into(),
            Self::Disable => "disable".into(),
            Self::Hypershift => "Hypershift".into(),
            Self::Unavailable { original, .. } if !original.is_empty() => original.clone(),
            _ => format!(
                "{PREFIX}{}",
                serde_json::to_string(self).expect("mapping serialization")
            ),
        }
    }
    fn category(&self) -> Option<Category> {
        Some(match self {
            Self::Default => Category::Default,
            Self::Keyboard { .. } => Category::Keyboard,
            Self::Mouse { .. } => Category::Mouse,
            Self::Sensitivity { .. } => Category::Sensitivity,
            Self::Multimedia { .. } => Category::Multimedia,
            Self::Windows { .. } => Category::Windows,
            Self::Brightness { .. } => Category::Brightness,
            Self::Text { .. } => Category::Text,
            Self::Profile { .. } => Category::Profile,
            Self::Launch { .. } => Category::Launch,
            Self::Hypershift => Category::Hypershift,
            Self::Disable => Category::Disable,
            Self::Unavailable { category, .. } => {
                return Category::all().into_iter().find(|c| c.id() == category);
            }
        })
    }
    fn selected(&self) -> &str {
        match self {
            Self::Keyboard { key, .. } => key,
            Self::Mouse { action, .. }
            | Self::Sensitivity { action, .. }
            | Self::Multimedia { action }
            | Self::Brightness { action }
            | Self::Windows { action }
            | Self::Profile { action } => action,
            Self::Launch { mode, .. } => mode,
            _ => "",
        }
    }
    fn turbo(&self) -> Option<u32> {
        match self {
            Self::Keyboard { turbo, .. } | Self::Mouse { turbo, .. } => *turbo,
            _ => None,
        }
    }
    fn is_primary(&self, input: &str) -> bool {
        matches!(self, Self::Mouse { action, .. } if action == "Click")
            || matches!(self, Self::Default) && matches!(input, "LeftButton" | "LeftClick")
    }
}

fn choices(data: &[(&str, &str)]) -> Vec<Choice> {
    data.iter()
        .map(|(id, label)| Choice::new(*id, i18n::t(label)))
        .collect()
}
const MOUSE: &[(&str, &str)] = &[
    ("Click", "LEFT_CLICK"),
    ("Menu", "RIGHT_CLICK"),
    ("ScrollButton", "SCROLL_CLICK"),
    ("DoubleClick", "DOUBLE_CLICK"),
    ("ScrollUp", "SCROLL_UP"),
    ("ScrollDown", "SCROLL_DOWN"),
    ("Previous", "MOUSE_BUTTON_4"),
    ("Next", "MOUSE_BUTTON_5"),
    ("ScrollLeft", "SCROLL_LEFT"),
    ("ScrollRight", "SCROLL_RIGHT"),
];
const REPEAT_SCROLL: &[(&str, &str)] = &[
    ("RepeatScrollUp", "REPEAT_SCROLL_UP"),
    ("RepeatScrollDown", "REPEAT_SCROLL_DOWN"),
    ("RepeatScrollLeft", "REPEAT_SCROLL_LEFT"),
    ("RepeatScrollRight", "REPEAT_SCROLL_RIGHT"),
];
const SENSITIVITY: &[(&str, &str)] = &[
    ("DPI_Clutch", "SENSITIVITY_CLUTCH"),
    ("DPI_Up", "SENSITIVITY_STAGE_UP"),
    ("DPI_Down", "SENSITIVITY_STAGE_DOWN"),
    ("DPI_OnTheFly", "ON_THE_FLY_SENSITIVITY"),
    ("DPI_CycleUp", "CYCLE_UP_SENSITIVITY"),
    ("DPI_CycleDown", "CYCLE_DOWN_SENSITIVITY"),
];
pub(in crate::features) const MEDIA: &[(&str, &str)] = &[
    ("VolumeDown", "VOLUME_DOWN"),
    ("VolumeUp", "VOLUME_UP"),
    ("MuteVolume", "MUTE_VOLUME"),
    ("MicVolumeUp", "MIC_VOLUME_UP"),
    ("MicVolumeDown", "MIC_VOLUME_DOWN"),
    ("MuteMic", "MUTE_MIC"),
    ("MuteAll", "MUTE_ALL"),
    ("Play", "PLAY_PAUSE"),
    ("PrevTrack", "PREVIOUS_TRACK"),
    ("NextTrack", "NEXT_TRACK"),
];
pub(in crate::features) const WINDOWS: &[(&str, &str)] = &[
    ("Calculator", "LAUNCH_CALCULATOR"),
    ("MSPaint", "LAUNCH_MSPAINT"),
    ("Notepad", "LAUNCH_NOTEPAD"),
    ("Snipping_Tool", "LAUNCH_SNIPPING_TOOL"),
    ("LaunchTaskManager", "LAUNCH_TASK_MANAGER"),
    ("MSCopilot", "LAUNCH_WINDOWS_COPILOT"),
    ("User_Directory", "OPEN_USER_DIRECTORY"),
    ("PowerUserMenu", "OPEN_SYSTEM_UTILITY"),
    ("ShowDesktop", "SHOW_DESKTOP"),
    ("CycleApps", "CYCLE_APPS"),
    ("SwitchApps", "SWITCH_APPS"),
    ("CloseApp", "CLOSE_APP"),
    ("Cut", "CUT"),
    ("Copy", "COPY"),
    ("Paste", "PASTE"),
    ("Mail", "MAIL"),
    ("ThisPC", "THIS_PC"),
    ("Refresh", "REFRESH"),
    ("DisplayBrightnessUp", "DISPLAY_BRIGHTNESS_UP"),
    ("DisplayBrightnessDown", "DISPLAY_BRIGHTNESS_DOWN"),
    ("File_Explorer", "OPEN_FILE_EXPLORER"),
    ("LockComputer", "LOCK_COMPUTER"),
    ("WindowsZoomIn", "WINDOWS_ZOOM_IN"),
    ("WindowsZoomOut", "WINDOWS_ZOOM_OUT"),
    ("OfficeZoomIn", "OFFICE_ZOOM_IN"),
    ("OfficeZoomOut", "OFFICE_ZOOM_OUT"),
];
const MODIFIERS: &[(&str, &str)] = &[
    ("KEY_LEFT_SHIFT", "左 Shift"),
    ("KEY_RIGHT_SHIFT", "右 Shift"),
    ("KEY_LEFT_CTRL", "左 Ctrl"),
    ("KEY_RIGHT_CTRL", "右 Ctrl"),
    ("KEY_LEFT_ALT", "左 Alt"),
    ("KEY_RIGHT_ALT", "右 Alt"),
    ("KEY_LEFT_GUI", "左 Win"),
];
const BRIGHTNESS: &[(&str, &str)] = &[
    ("BrightnessUp", "BRIGHTNESS_UP"),
    ("BrightnessDown", "BRIGHTNESS_DOWN"),
    ("BrightnessToggle", "BRIGHTNESS_TOGGLE"),
];
const PROFILE_NAVIGATION: &[(&str, &str)] = &[
    ("NextProfile", "NEXT_PROFILE"),
    ("PreviousProfile", "PREVIOUS_PROFILE"),
    ("CycleUp", "CYCLE_UP_PROFILE"),
    ("CycleDown", "CYCLE_DOWN_PROFILE"),
];

// Module 6114's symbol rows without inputID resolve by keyCode, then add Shift
// in MapKeyboard.getMappingData. These IDs identify UI choices, never hardware.
const SHIFTED_SYMBOLS: &[(&str, &str)] = &[
    ("~", "KEY_TILDE"),
    ("!", "KEY_1"),
    ("@", "KEY_2"),
    ("#", "KEY_3"),
    ("$", "KEY_4"),
    ("%", "KEY_5"),
    ("^", "KEY_6"),
    ("&", "KEY_7"),
    ("*", "KEY_8"),
    ("(", "KEY_9"),
    (")", "KEY_0"),
    ("_", "KEY_HYPEN"),
    ("+", "KEY_EQUAL"),
    ("{", "KEY_OPEN_SQUARE_BRACKET"),
    ("}", "KEY_CLOSE_SQUARE_BRACKET"),
    ("|", "KEY_BACKSLASH"),
    (":", "KEY_SEMICOLON"),
    ("\"", "KEY_APOSTROPHE"),
    ("<", "KEY_COMMA"),
    (">", "KEY_PERIOD"),
    ("?", "KEY_SLASH"),
];

pub(super) fn key_groups() -> Vec<Choice> {
    choices(&[
        ("record", "KB_KEY_RECORDING"),
        ("alphanumeric", "KB_ALPHANUMERIC"),
        ("function", "KB_FUNCTION"),
        ("numpad", "KB_NUMPAD"),
        ("navigation", "KB_NAVIGATION"),
        ("modifiers", "KB_MODIFIERS"),
        ("symbols", "KB_SYMBOLS"),
    ])
}
const KEYS: &[(&str, &str, &str)] = include!("mapping_keys.rs");
fn key_choices(group: &str) -> Vec<Choice> {
    if group == "record" {
        return vec![];
    }
    if group == "symbols" {
        return [
            "`", "~", "!", "@", "#", "$", "%", "^", "&", "*", "(", ")", "-", "=", "_", "+", "[",
            "]", "\\", "{", "}", "|", ";", "'", ":", "\"", ",", ".", "/", "<", ">", "?",
        ]
        .into_iter()
        .filter_map(|symbol| {
            if SHIFTED_SYMBOLS.iter().any(|(name, _)| *name == symbol) {
                Some(Choice::new(format!("symbol:{symbol}"), symbol))
            } else {
                KEYS.iter()
                    .find(|(kind, _, label)| *kind == "symbols" && *label == symbol)
                    .map(|(_, id, label)| Choice::new(*id, *label))
            }
        })
        .collect();
    }
    KEYS.iter()
        .filter(|(kind, _, _)| *kind == group)
        .map(|(_, id, label)| Choice::new(*id, *label))
        .collect()
}
fn valid_key(key: &str) -> bool {
    KEYS.iter().any(|(_, id, _)| *id == key)
}
pub(in crate::features) fn canonical_key(key: &str) -> Option<String> {
    if valid_key(key) {
        return Some(key.into());
    }
    let lower = key.to_ascii_lowercase();
    let mapped = match lower.as_str() {
        "enter" | "return" => "KEY_ENTER",
        "escape" | "esc" => "KEY_ESC",
        "space" | "spacebar" | " " => "KEY_SPACEBAR",
        "tab" => "KEY_TAB",
        "backspace" => "KEY_BACKSPACE",
        "delete" => "KEY_DELETE",
        "insert" => "KEY_INSERT",
        "home" => "KEY_HOME",
        "end" => "KEY_END",
        "pageup" | "page-up" | "page up" => "KEY_PAGE_UP",
        "pagedown" | "page-down" | "page down" => "KEY_PAGE_DOWN",
        "printscreen" | "print-screen" | "print screen" => "KEY_PRINT_SCREEN",
        "pause" => "KEY_PAUSE",
        "capslock" | "caps-lock" | "caps lock" => "KEY_CAPS_LOCK",
        "scrolllock" | "scroll-lock" | "scroll lock" => "KEY_SCROLL_LOCK",
        "numlock" | "num-lock" | "num lock" => "KEY_NUMPAD_NUM_LOCK",
        "menu" | "apps" => "KEY_APPLICATION",
        "ctrl" | "control" => "KEY_LEFT_CTRL",
        "alt" => "KEY_LEFT_ALT",
        "shift" => "KEY_LEFT_SHIFT",
        "win" | "windows" | "super" => "KEY_LEFT_GUI",
        "up" => "KEY_UP_ARROW",
        "down" => "KEY_DOWN_ARROW",
        "left" => "KEY_LEFT_ARROW",
        "right" => "KEY_RIGHT_ARROW",
        "-" => "KEY_HYPEN",
        "=" => "KEY_EQUAL",
        "[" => "KEY_OPEN_SQUARE_BRACKET",
        "]" => "KEY_CLOSE_SQUARE_BRACKET",
        "\\" => "KEY_BACKSLASH",
        ";" => "KEY_SEMICOLON",
        "'" => "KEY_APOSTROPHE",
        "`" => "KEY_TILDE",
        "," => "KEY_COMMA",
        "." => "KEY_PERIOD",
        "/" => "KEY_SLASH",
        _ => {
            if let Some((_, key)) = SHIFTED_SYMBOLS.iter().find(|(symbol, _)| *symbol == key) {
                return Some((*key).into());
            }
            if let Some((_, id, _)) = KEYS
                .iter()
                .find(|(_, _, label)| label.eq_ignore_ascii_case(key))
            {
                return Some((*id).into());
            }
            let id = format!("KEY_{}", key.to_ascii_uppercase());
            return valid_key(&id).then_some(id);
        }
    };
    Some(mapped.into())
}
pub(in crate::features) fn key_label(key: &str) -> String {
    KEYS.iter()
        .find(|(_, id, _)| *id == key)
        .map(|(_, _, label)| (*label).to_string())
        .unwrap_or_else(|| key.into())
}
fn modifier_family(key: &str) -> &str {
    key.strip_prefix("KEY_LEFT_")
        .or_else(|| key.strip_prefix("KEY_RIGHT_"))
        .unwrap_or(key)
}
fn assign_key_choice(assignment: &mut Assignment, selected: &str, optional_modifiers: &[String]) {
    let Assignment::Keyboard { key, .. } = assignment else {
        return;
    };
    if let Some(symbol) = selected.strip_prefix("symbol:")
        && let Some((_, physical)) = SHIFTED_SYMBOLS.iter().find(|(name, _)| *name == symbol)
    {
        *key = (*physical).into();
    } else {
        *key = selected.into();
    }
    update_key_modifiers(assignment, Some(selected), optional_modifiers);
}
fn update_key_modifiers(
    assignment: &mut Assignment,
    selected_symbol: Option<&str>,
    optional_modifiers: &[String],
) {
    let Assignment::Keyboard { key, modifiers, .. } = assignment else {
        return;
    };
    modifiers.clear();
    modifiers.extend_from_slice(optional_modifiers);
    // MapKeyboard.getMappingData adds a symbol's required Shift independently
    // of the optional modifier buttons, including after they are unchecked.
    let requires_shift = selected_symbol
        .and_then(|selected| selected.strip_prefix("symbol:"))
        .is_some_and(|symbol| {
            SHIFTED_SYMBOLS
                .iter()
                .any(|(name, physical)| *name == symbol && *physical == key)
        });
    if requires_shift
        && !modifiers
            .iter()
            .any(|modifier| modifier_family(modifier) == "SHIFT")
    {
        modifiers.push("KEY_LEFT_SHIFT".into());
    }
}
fn normalized_number(text: &str, turbo: bool, fallback: u32) -> u32 {
    let value = text.trim().parse::<u32>().unwrap_or(fallback);
    if turbo {
        value.clamp(1, 20)
    } else {
        value.clamp(100, 30000).div_ceil(50) * 50
    }
}
pub(in crate::features) fn normalized_website(target: &str) -> Option<String> {
    let value = target.trim();
    if value.is_empty() || value.chars().any(char::is_whitespace) {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    let (prefix, address) = if lower.starts_with("https://") {
        ("https://", &value[8..])
    } else if lower.starts_with("http://") {
        ("http://", &value[7..])
    } else if value.contains("://") {
        return None;
    } else {
        ("https://", value)
    };
    let authority = address.split(['/', '?', '#']).next().unwrap_or_default();
    let mut parts = authority.split(':');
    let host = parts.next().unwrap_or_default();
    if let Some(port) = parts.next()
        && (parts.next().is_some() || !port.parse::<u16>().is_ok_and(|port| port > 0))
    {
        return None;
    }
    let domain = host.contains('.')
        && host.split('.').all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if host.is_empty()
        || authority.contains('@')
        || !(host.eq_ignore_ascii_case("localhost") || domain)
    {
        return None;
    }
    Some(format!("{prefix}{address}"))
}

#[cfg(test)]
fn categories(pid: u32, input: &str, hypershift: bool) -> Vec<Category> {
    categories_for_layout(pid, 1, input, hypershift)
}
fn categories_for_layout(pid: u32, layout: u32, input: &str, hypershift: bool) -> Vec<Category> {
    let supported = |category: &Category| {
        if hypershift && *category == Category::Hypershift {
            return false;
        }
        if pid == 653 {
            return crate::features::customize_drawer::keyboard_mapping_input(layout, input)
                .is_some_and(|key| {
                    key.is_enabled
                        && !key.disabled
                        && !(hypershift && key.disable_hypershift_mapping)
                        && key.function_list.iter().any(|f| f == category.capability())
                });
        }
        if pid != 182 {
            return false;
        }
        if *category == Category::Brightness {
            return false;
        }
        match input {
            "ScrollUp" | "ScrollDown" => matches!(
                category,
                Category::Default
                    | Category::Keyboard
                    | Category::Mouse
                    | Category::Macro
                    | Category::Multimedia
                    | Category::Disable
            ),
            "CycleUpSensitivityStages" | "DKM_SB_03" => *category != Category::Hypershift,
            "LeftButton" | "LeftClick" | "RightButton" | "RightClick" | "MiddleButton"
            | "ScrollButton" | "Button4" | "Button5" => true,
            _ => false,
        }
    };
    Category::all().into_iter().filter(supported).collect()
}

impl DeviceWorkspace {
    pub(in crate::features) fn mapping_is_disabled(&self, value: &str) -> bool {
        matches!(Assignment::decode(value), Assignment::Disable)
    }

    pub(in crate::features) fn mapping_input_enabled(&self, input: &str) -> bool {
        if categories_for_layout(self.pid(), self.device.layout_id, input, self.hypershift)
            .is_empty()
        {
            return false;
        }
        if self.pid() == 182 && !self.hypershift && matches!(input, "LeftButton" | "LeftClick") {
            return self.primary_click_after("LeftButton", &Assignment::Disable);
        }
        true
    }
    pub(in crate::features) fn mapping_summary(&self, value: &str) -> (String, String) {
        let assignment = Assignment::decode(value);
        let category = assignment
            .category()
            .map(Category::label)
            .unwrap_or_else(|| "已存映射".into());
        let description = match &assignment {
            Assignment::Keyboard { key, modifiers, .. } => modifiers
                .iter()
                .map(|m| key_label(m))
                .chain(std::iter::once(key_label(key)))
                .collect::<Vec<_>>()
                .join(" + "),
            Assignment::Mouse { action, .. } => MOUSE
                .iter()
                .chain(REPEAT_SCROLL)
                .find(|(id, _)| id == action)
                .map(|(_, label)| i18n::t(label))
                .unwrap_or_else(|| action.clone()),
            Assignment::Sensitivity { action, .. } => SENSITIVITY
                .iter()
                .find(|(id, _)| id == action)
                .map(|(_, label)| i18n::t(label))
                .unwrap_or_else(|| action.clone()),
            Assignment::Multimedia { action } => MEDIA
                .iter()
                .find(|(id, _)| id == action)
                .map(|(_, label)| i18n::t(label))
                .unwrap_or_else(|| action.clone()),
            Assignment::Windows { action } => WINDOWS
                .iter()
                .find(|(id, _)| id == action)
                .map(|(_, label)| i18n::t(label))
                .unwrap_or_else(|| action.clone()),
            Assignment::Brightness { action } => BRIGHTNESS
                .iter()
                .find(|(id, _)| id == action)
                .map(|(_, label)| i18n::t(label))
                .unwrap_or_else(|| action.clone()),
            Assignment::Text { text } => text.clone(),
            Assignment::Launch { target, .. } => target.clone(),
            Assignment::Profile { action } => self
                .device
                .profiles
                .iter()
                .find(|p| action == &format!("profile:{}", p.id))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| {
                    PROFILE_NAVIGATION
                        .iter()
                        .find(|(id, _)| *id == action)
                        .map(|(_, label)| i18n::t(label))
                        .unwrap_or_else(|| action.clone())
                }),
            Assignment::Unavailable { original, .. } if !original.is_empty() => {
                "尚未支持的映射".into()
            }
            _ => category.clone(),
        };
        (category, description)
    }
    fn mapping_action(&self) -> Option<Assignment> {
        self.mapping
            .as_ref()
            .map(|draft| Assignment::decode(&draft.value))
    }

    fn mapping_symbol_choice(&self, cx: &App) -> Option<String> {
        (self.mapping_key_group == "symbols")
            .then(|| self.controls.mapping.read(cx).selected_value().cloned())
            .flatten()
    }

    fn update_mapping_modifiers(
        &mut self,
        cx: &mut Context<Self>,
        update: impl FnOnce(&mut Vec<String>),
    ) {
        update(&mut self.mapping_optional_modifiers);
        let symbol = self.mapping_symbol_choice(cx);
        let optional_modifiers = self.mapping_optional_modifiers.clone();
        self.update_mapping(cx, |action| {
            update_key_modifiers(action, symbol.as_deref(), &optional_modifiers);
        });
        // Explicit selection can change while the encoded chord stays the same:
        // a symbol still requires Shift after its optional Shift is unchecked.
        cx.notify();
    }

    fn mapping_categories(&self) -> Vec<Category> {
        self.mapping
            .as_ref()
            .map(|draft| {
                if let Some(uid) = &draft.dial_mode {
                    return self.dial_mapping_categories(uid, &draft.input);
                }
                categories_for_layout(
                    self.pid(),
                    self.device.layout_id,
                    &draft.input,
                    self.hypershift,
                )
            })
            .unwrap_or_default()
    }
    fn dial_mapping_categories(&self, uid: &str, input: &str) -> Vec<Category> {
        if self.pid() != 653
            || !matches!(input, "ScrollLeft" | "ScrollRight")
            || !self
                .settings()
                .keyboard
                .dial_modes
                .iter()
                .any(|mode| mode.uid == uid && mode.is_custom)
        {
            return vec![];
        }
        // These original inputs have disabled=true to hide them from the
        // ordinary image/list, but isEnabled/functionList allow custom modes.
        let source =
            crate::features::customize_drawer::keyboard_mapping_input(self.device.layout_id, input);
        Category::all()
            .into_iter()
            .filter(|category| {
                source.is_some_and(|source| {
                    source.is_enabled
                        && source
                            .function_list
                            .iter()
                            .any(|value| value == category.capability())
                })
            })
            .collect()
    }
    fn mapping_options(&self, action: &Assignment) -> Vec<Choice> {
        match action {
            Assignment::Keyboard { .. } => key_choices(&self.mapping_key_group),
            Assignment::Mouse { .. } => {
                let mut data = choices(MOUSE);
                if self.pid() == 182 && self.turbo_supported() {
                    data.extend(choices(REPEAT_SCROLL));
                }
                data
            }
            Assignment::Sensitivity { .. } => choices(SENSITIVITY)
                .into_iter()
                .filter(|item| {
                    let bottom = self.mapping.as_ref().is_some_and(|d| {
                        matches!(d.input.as_str(), "CycleUpSensitivityStages" | "DKM_SB_03")
                    });
                    !(bottom && matches!(item.id(), "DPI_Clutch" | "DPI_OnTheFly")
                        || self.hypershift && item.id() == "DPI_OnTheFly")
                })
                .collect(),
            Assignment::Multimedia { .. } => choices(MEDIA),
            Assignment::Windows { .. } => choices(WINDOWS),
            Assignment::Brightness { .. } => choices(BRIGHTNESS),
            Assignment::Profile { .. } => self
                .device
                .profiles
                .iter()
                .filter(|profile| profile.id != self.device.active_profile)
                .map(|profile| Choice::new(format!("profile:{}", profile.id), profile.name.clone()))
                .collect(),
            Assignment::Launch { .. } => choices(&[("program", "程序"), ("website", "网站")]),
            _ => vec![],
        }
    }
    fn turbo_supported(&self) -> bool {
        self.mapping.as_ref().is_some_and(|draft| {
            if draft.dial_mode.is_some() {
                return false;
            }
            let source = if self.pid() == 653 {
                crate::features::customize_drawer::keyboard_mapping_input(
                    self.device.layout_id,
                    &draft.input,
                )
            } else {
                None
            };
            if source.is_some_and(|source| {
                self.mapping_action()
                    .and_then(|action| action.category())
                    .is_some_and(|category| {
                        source
                            .disable_turbo_in_assignment
                            .iter()
                            .any(|disabled| disabled == category.capability())
                    })
            }) {
                return false;
            }
            let input = source
                .map(|source| source.button_key.as_str())
                .unwrap_or(&draft.input);
            !matches!(
                input,
                "ScrollUp"
                    | "ScrollDown"
                    | "CycleUpSensitivityStages"
                    | "DKM_SB_03"
                    | "DIAL_RIGHT_ROTATION"
                    | "DIAL_CLICK"
                    | "DIAL_LEFT_ROTATION"
                    | "RepeatScrollUp"
                    | "RepeatScrollDown"
                    | "MEDIA_PREV"
                    | "MEDIA_PAUSE_PLAY"
                    | "MEDIA_NEXT"
                    | "MEDIA_MUTE"
                    | "MEDIA_VOLUME_UP"
                    | "MEDIA_VOLUME_DOWN"
            )
        })
    }
    pub(super) fn mapping_valid(&self) -> bool {
        !self.mapping_dirty() || self.mapping_error().is_none()
    }
    fn mapping_error(&self) -> Option<&'static str> {
        let draft = self.mapping.as_ref()?;
        let action = Assignment::decode(&draft.value);
        if matches!(action, Assignment::Unavailable { .. }) {
            return Some("此映射需要原生服务或尚未支持的格式。可以保留现有映射，或选择其他功能。");
        }
        if !action
            .category()
            .is_some_and(|c| self.mapping_categories().contains(&c))
        {
            return Some("此输入或当前层不支持该功能。");
        }
        if self.pid() == 182 && !self.hypershift && !self.primary_click_after(&draft.input, &action)
        {
            return Some("请先为另一个按钮分配左键单击；标准层必须保留一个主点击按钮。");
        }
        if let Some(rate) = action.turbo()
            && (!self.turbo_supported() || !(1..=20).contains(&rate))
        {
            return Some("Turbo 速度须为每秒 1–20 次。");
        }
        match &action {
            Assignment::Keyboard { key, modifiers, .. } => {
                if !valid_key(key) {
                    return Some("请录制按键，或从按键组中选择一个按键。");
                }
                if modifiers
                    .iter()
                    .any(|m| !MODIFIERS.iter().any(|(id, _)| id == m))
                {
                    return Some("请重新选择有效的修饰键。");
                }
                if modifiers.iter().enumerate().any(|(index, modifier)| {
                    modifiers[..index]
                        .iter()
                        .any(|earlier| modifier_family(earlier) == modifier_family(modifier))
                }) {
                    return Some("每种修饰键只能选择左侧或右侧的一个。");
                }
            }
            Assignment::Text { text } if text.is_empty() || text.encode_utf16().count() > 250 => {
                return Some("请输入 1–250 个字符的文本。");
            }
            Assignment::Sensitivity {
                action,
                x,
                y,
                independent,
            } => {
                if action == "DPI_Clutch"
                    && (!(100..=30000).contains(x)
                        || x % 50 != 0
                        || *independent && (!(100..=30000).contains(y) || y % 50 != 0))
                {
                    return Some("DPI 须为 100–30000，步长为 50。");
                }
            }
            Assignment::Profile { .. } if self.device.profiles.len() <= 1 => {
                return Some("请先创建另一个配置文件。");
            }
            Assignment::Launch { mode, target } => {
                if target.trim().is_empty() {
                    return Some("请输入程序路径或网站地址。");
                }
                if mode == "website" && normalized_website(target).is_none() {
                    return Some("请输入有效的网站地址，例如 https://www.razer.com。");
                }
            }
            _ => {}
        }
        if matches!(
            action,
            Assignment::Mouse { .. }
                | Assignment::Sensitivity { .. }
                | Assignment::Multimedia { .. }
                | Assignment::Brightness { .. }
                | Assignment::Windows { .. }
                | Assignment::Profile { .. }
                | Assignment::Launch { .. }
        ) && !matches!(&action, Assignment::Profile { action } if PROFILE_NAVIGATION.iter().any(|(id, _)| *id == action))
            && !self
                .mapping_options(&action)
                .iter()
                .any(|item| item.id() == action.selected())
        {
            return Some("请选择此输入支持的功能。");
        }
        None
    }
    fn primary_click_after(&self, input: &str, candidate: &Assignment) -> bool {
        [
            "LeftButton",
            "RightButton",
            "MiddleButton",
            "Button4",
            "Button5",
            "ScrollUp",
            "ScrollDown",
            "CycleUpSensitivityStages",
        ]
        .iter()
        .any(|key| {
            if *key == input {
                candidate.is_primary(key)
            } else {
                Assignment::decode(
                    self.settings()
                        .bindings
                        .get(*key)
                        .map(String::as_str)
                        .unwrap_or("default"),
                )
                .is_primary(key)
            }
        })
    }
    fn update_mapping(&mut self, cx: &mut Context<Self>, update: impl FnOnce(&mut Assignment)) {
        let Some(draft) = &mut self.mapping else {
            return;
        };
        let mut action = Assignment::decode(&draft.value);
        let before = action.clone();
        update(&mut action);
        if let Assignment::Launch { mode, target } = &mut action
            && mode == "website"
            && let Some(normalized) = normalized_website(target)
        {
            *target = normalized;
        }
        if action == before {
            return;
        }
        draft.value = if action == Assignment::decode(&draft.original) {
            draft.original.clone()
        } else {
            action.encode()
        };
        self.changed(cx);
    }
    fn select_mapping_category(
        &mut self,
        category: Category,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.mapping_categories().contains(&category) {
            return;
        }
        self.mapping_expanded = false;
        self.mapping_recording = false;
        if self.mapping_action().and_then(|a| a.category()) != Some(category) {
            if category == Category::Keyboard {
                self.mapping_key_group = "record".into();
                self.mapping_optional_modifiers.clear();
                self.mapping_modifiers_enabled = false;
            }
            let mut initial = category.initial();
            let clutch_supported = self
                .mapping_options(&initial)
                .iter()
                .any(|item| item.id() == "DPI_Clutch");
            if let Assignment::Sensitivity { action, .. } = &mut initial
                && !clutch_supported
            {
                *action = "DPI_Up".into();
            }
            self.update_mapping(cx, |action| *action = initial);
        }
        self.sync_mapping_controls(window, cx);
        cx.notify();
    }
    pub(super) fn open_mapping(
        &mut self,
        input: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.mapping_input_enabled(&input) {
            return;
        }
        let bindings = if self.hypershift {
            &self.settings().hypershift_bindings
        } else {
            &self.settings().bindings
        };
        let value = bindings
            .get(&input)
            .cloned()
            .unwrap_or_else(|| "default".into());
        self.begin_mapping(input, value, None, window, cx);
    }
    pub(super) fn open_dial_mapping(
        &mut self,
        uid: String,
        input: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.dial_mapping_categories(&uid, &input).is_empty() {
            return;
        }
        let value = self
            .settings()
            .keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == uid)
            .and_then(|mode| mode.mappings.get(&input))
            .cloned()
            .unwrap_or_else(|| "disable".into());
        self.begin_mapping(input, value, Some(uid), window, cx);
    }
    fn begin_mapping(
        &mut self,
        input: String,
        value: String,
        dial_mode: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mapping = Some(MappingDraft {
            input,
            original: value.clone(),
            value,
            dial_mode,
        });
        self.mapping_key_group = "record".into();
        self.mapping_optional_modifiers = match self.mapping_action() {
            Some(Assignment::Keyboard { modifiers, .. }) => modifiers,
            _ => vec![],
        };
        self.mapping_modifiers_enabled = !self.mapping_optional_modifiers.is_empty();
        self.mapping_recording = false;
        self.mapping_expanded = false;
        self.sync_mapping_controls(window, cx);
        window.focus(&self.mapping_focus, cx);
    }
    fn sync_mapping_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.mapping_action() else {
            return;
        };
        let items = self.mapping_options(&action);
        let selected = if let Assignment::Keyboard { key, modifiers, .. } = &action
            && self.mapping_key_group == "symbols"
        {
            // Keep the chosen symbol distinct from a physical key plus optional
            // Shift. A Turbo/slider redraw must not turn '=' + Shift into '+'.
            self.mapping_symbol_choice(cx)
                .filter(|selected| {
                    if let Some(symbol) = selected.strip_prefix("symbol:") {
                        SHIFTED_SYMBOLS
                            .iter()
                            .any(|(name, physical)| *name == symbol && *physical == key)
                            && modifiers
                                .iter()
                                .any(|modifier| modifier_family(modifier) == "SHIFT")
                    } else {
                        selected == key
                    }
                })
                .unwrap_or_else(|| key.clone())
        } else {
            action.selected().to_string()
        };
        self.controls.mapping.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            state.set_selected_value(&selected, window, cx);
        });
        self.controls.mapping_group.update(cx, |state, cx| {
            state.set_selected_value(&self.mapping_key_group, window, cx)
        });
        let target = match &action {
            Assignment::Launch { target, .. } => target.as_str(),
            _ => "",
        };
        self.controls
            .mapping_text
            .update(cx, |state, cx| state.set_value(target, window, cx));
        let paragraph = match &action {
            Assignment::Text { text } => text.as_str(),
            _ => "",
        };
        self.controls
            .mapping_paragraph
            .update(cx, |state, cx| state.set_value(paragraph, window, cx));
        if let Assignment::Sensitivity { x, y, .. } = action {
            self.controls
                .mapping_x
                .update(cx, |state, cx| state.set_value(x.to_string(), window, cx));
            self.controls
                .mapping_y
                .update(cx, |state, cx| state.set_value(y.to_string(), window, cx));
        }
        self.controls.mapping_rate.update(cx, |state, cx| {
            state.set_value(action.turbo().unwrap_or(7).to_string(), window, cx)
        });
        self.sync_mapping_sliders(window, cx);
    }
    fn sync_mapping_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.mapping_action() else {
            return;
        };
        let mut values = [None, None, action.turbo().map(|v| v as f32)];
        if let Assignment::Sensitivity { x, y, .. } = action {
            values[0] = ((100..=30000).contains(&x) && x % 50 == 0).then_some(x as f32);
            values[1] = ((100..=30000).contains(&y) && y % 50 == 0).then_some(y as f32);
        }
        for (index, value) in values.into_iter().enumerate() {
            if let Some(value) = value
                && (index != 2 || (1.0..=20.0).contains(&value))
                && self.controls.mapping_sliders[index]
                    .read(cx)
                    .value()
                    .start()
                    != value
            {
                self.controls.mapping_sliders[index]
                    .update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
    }
    pub(super) fn install_mapping_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.subscribe_in(
            &self.controls.mapping,
            window,
            |this, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    if !this.mapping_action().is_some_and(|action| {
                        this.mapping_options(&action)
                            .iter()
                            .any(|choice| choice.id() == id)
                    }) {
                        return;
                    }
                    let optional_modifiers = if this.mapping_key_group == "modifiers" {
                        vec![]
                    } else {
                        this.mapping_optional_modifiers.clone()
                    };
                    this.update_mapping(cx, |action| match action {
                        Assignment::Keyboard { .. } => {
                            assign_key_choice(action, id, &optional_modifiers)
                        }
                        Assignment::Mouse { action, .. }
                        | Assignment::Sensitivity { action, .. }
                        | Assignment::Multimedia { action }
                        | Assignment::Brightness { action }
                        | Assignment::Windows { action }
                        | Assignment::Profile { action } => *action = id.clone(),
                        Assignment::Launch { mode, .. } => *mode = id.clone(),
                        _ => {}
                    });
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &self.controls.mapping_group,
            window,
            |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    if !key_groups().iter().any(|group| group.id() == id) {
                        return;
                    }
                    if this.mapping_key_group == *id {
                        return;
                    }
                    this.mapping_key_group = id.clone();
                    this.mapping_recording = false;
                    // MapKeyboard.changeKeyType retains the optional modifier
                    // buttons. Recording and the Modifiers group do not use them.
                    let optional_modifiers = if matches!(id.as_str(), "record" | "modifiers") {
                        vec![]
                    } else {
                        this.mapping_optional_modifiers.clone()
                    };
                    let key = key_choices(id)
                        .first()
                        .map(|choice| choice.id().to_string())
                        .unwrap_or_default();
                    this.update_mapping(cx, |action| {
                        assign_key_choice(action, &key, &optional_modifiers);
                    });
                    this.sync_mapping_controls(window, cx);
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &self.controls.mapping_text,
            window,
            |this, input, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    this.update_mapping(cx, |action| {
                        if let Assignment::Launch { target, .. } = action {
                            *target = value;
                        }
                    });
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &self.controls.mapping_paragraph,
            window,
            |this, input, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    let value = input.read(cx).value().to_string();
                    this.update_mapping(cx, |action| {
                        if let Assignment::Text { text } = action {
                            *text = value;
                        }
                    });
                }
            },
        ));
        for (input, index) in [
            (&self.controls.mapping_x, 0),
            (&self.controls.mapping_y, 1),
            (&self.controls.mapping_rate, 2),
        ] {
            self.subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, input, event, window, cx| {
                    let normalize =
                        matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. });
                    if !matches!(event, InputEvent::Change) && !normalize {
                        return;
                    }
                    let text = input.read(cx).value().to_string();
                    let value = if normalize {
                        normalized_number(
                            &text,
                            index == 2,
                            this.controls.mapping_sliders[index]
                                .read(cx)
                                .value()
                                .start() as u32,
                        )
                    } else {
                        text.parse::<u32>().unwrap_or(0)
                    };
                    this.update_mapping(cx, |action| match action {
                        Assignment::Sensitivity {
                            x, y, independent, ..
                        } if index < 2 => {
                            if index == 0 {
                                *x = value;
                                if !*independent {
                                    *y = value;
                                }
                            } else {
                                *y = value
                            }
                        }
                        Assignment::Keyboard { turbo, .. } | Assignment::Mouse { turbo, .. }
                            if index == 2 && turbo.is_some() =>
                        {
                            *turbo = Some(value)
                        }
                        _ => {}
                    });
                    if normalize {
                        input.update(cx, |input, cx| {
                            input.set_value(value.to_string(), window, cx)
                        });
                    }
                    this.sync_mapping_sliders(window, cx);
                },
            ));
        }
        for (index, slider) in self
            .controls
            .mapping_sliders
            .clone()
            .into_iter()
            .enumerate()
        {
            self.subscriptions.push(cx.subscribe_in(
                &slider,
                window,
                move |this, _, event, window, cx| {
                    if let SliderEvent::Change(value) = event {
                        let value = value.start().round() as u32;
                        this.update_mapping(cx, |action| match action {
                            Assignment::Sensitivity {
                                x, y, independent, ..
                            } if index < 2 => {
                                if index == 0 {
                                    *x = value;
                                    if !*independent {
                                        *y = value;
                                    }
                                } else {
                                    *y = value
                                }
                            }
                            Assignment::Keyboard { turbo, .. }
                            | Assignment::Mouse { turbo, .. }
                                if index == 2 && turbo.is_some() =>
                            {
                                *turbo = Some(value)
                            }
                            _ => {}
                        });
                        this.sync_mapping_controls(window, cx);
                    }
                },
            ));
        }
    }
    fn mapping_select(&self, action: &Assignment) -> AnyElement {
        surface::select(&self.controls.mapping)
            .items(self.mapping_options(action))
            .id("mapping-action")
            .accessibility_label("映射操作")
            .w(surface::css(210.))
            .into_any_element()
    }
    fn browse_mapping_program(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = &self.mapping else {
            return;
        };
        let identity = (
            self.device.active_profile.clone(),
            self.hypershift,
            draft.input.clone(),
        );
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(i18n::t("PROGRAM").into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            if let Ok(Ok(Some(paths))) = picker.await
                && let Some(path) = paths.first()
            {
                let target = path.to_string_lossy().into_owned();
                let _ = view.update_in(cx, |this, window, cx| {
                    if this.device.active_profile != identity.0
                        || this.hypershift != identity.1
                        || !this
                            .mapping
                            .as_ref()
                            .is_some_and(|draft| draft.input == identity.2)
                    {
                        return;
                    }
                    this.update_mapping(cx, |action| {
                        if let Assignment::Launch {
                            mode,
                            target: current,
                        } = action
                            && mode == "program"
                        {
                            *current = target;
                        }
                    });
                    this.sync_mapping_controls(window, cx);
                });
            }
        })
        .detach();
    }
    fn capture_mapping_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if !self.mapping_recording {
            return;
        }
        // Capture before control key bindings: Space/Enter/Tab are valid assignments.
        cx.stop_propagation();
        if event.is_held {
            return;
        }
        if event.keystroke.key == "escape" {
            self.mapping_recording = false;
            cx.notify();
            return;
        }
        let Some(key) = canonical_key(&event.keystroke.key) else {
            return;
        };
        let held = event.keystroke.modifiers;
        let implicit_shift = SHIFTED_SYMBOLS
            .iter()
            .any(|(symbol, _)| *symbol == event.keystroke.key);
        let modifiers = [
            (held.control, "KEY_LEFT_CTRL"),
            (held.alt, "KEY_LEFT_ALT"),
            (held.shift || implicit_shift, "KEY_LEFT_SHIFT"),
            (held.platform, "KEY_LEFT_GUI"),
        ]
        .into_iter()
        .filter(|(on, modifier)| *on && *modifier != key)
        .map(|(_, modifier)| modifier.into())
        .collect();
        self.update_mapping(cx, |action| {
            if let Assignment::Keyboard {
                key: current,
                modifiers: current_modifiers,
                ..
            } = action
            {
                *current = key;
                *current_modifiers = modifiers;
            }
        });
        self.mapping_recording = false;
        cx.notify();
    }
    pub(in crate::features) fn render_mapping_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(draft) = &self.mapping else {
            return div().into_any_element();
        };
        let action = Assignment::decode(&draft.value);
        let selected = action.category();
        let input_label = crate::features::customize_drawer::keyboard_mapping_input(
            self.device.layout_id,
            &draft.input,
        )
        .filter(|_| self.pid() == 653)
        .map(|key| i18n::t(&key.default_value))
        .unwrap_or_else(|| match draft.input.as_str() {
            "LeftButton" => "左键单击".into(),
            "RightButton" => "右键单击".into(),
            "MiddleButton" => "滚轮单击".into(),
            "Button4" => "鼠标按钮 4".into(),
            "Button5" => "鼠标按钮 5".into(),
            "ScrollUp" => "向上滚动".into(),
            "ScrollDown" => "向下滚动".into(),
            "CycleUpSensitivityStages" => "向上循环灵敏度阶段".into(),
            _ => draft.input.clone(),
        });
        let mut body = v_flex().gap(surface::css(10.)).w(surface::css(210.)).child(
            div()
                .font_family("RazerF5")
                .text_size(surface::css(16.))
                .text_color(cx.theme().primary)
                .mb(surface::css(10.))
                .child(
                    selected
                        .map(Category::label)
                        .unwrap_or_else(|| "已存映射".into()),
                ),
        );
        match &action {
            Assignment::Default => {
                body = body.child(surface::note(
                    format!("恢复此输入的默认功能：{input_label}"),
                    cx,
                ));
            }
            Assignment::Disable => {
                body = body.child(surface::note("此输入将不执行任何操作。", cx));
            }
            Assignment::Hypershift => {
                body = body.child(surface::note(
                    "按住此按钮时启用 Hypershift 层，松开后恢复标准层。",
                    cx,
                ));
            }
            Assignment::Keyboard { key, modifiers, .. } => {
                body = body.child(
                    surface::select(&self.controls.mapping_group)
                        .items(key_groups())
                        .id("mapping-key-group")
                        .accessibility_label("按键组")
                        .w(surface::css(210.)),
                );
                if self.mapping_key_group == "record" {
                    let label = if self.mapping_recording {
                        "按下要分配的组合键…".into()
                    } else if key.is_empty() {
                        "录制按键".into()
                    } else {
                        modifiers
                            .iter()
                            .map(|m| key_label(m))
                            .chain(std::iter::once(key_label(key)))
                            .collect::<Vec<_>>()
                            .join(" + ")
                    };
                    body = body.child(
                        div().id("mapping-key-recorder").child(
                            Button::new("mapping-record")
                                .label(label)
                                .w_full()
                                .when(self.mapping_recording, |button| button.primary())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.mapping_recording = true;
                                    window.focus(&this.mapping_focus, cx);
                                    cx.notify();
                                })),
                        ),
                    );
                } else {
                    body = body.child(self.mapping_select(&action));
                    if self.mapping_key_group != "modifiers" {
                        body = body.child(
                            Checkbox::new("mapping-include-modifiers")
                                .label(i18n::t("TEXT_INCLUDE_MODIFIER"))
                                .checked(self.mapping_modifiers_enabled)
                                .on_change(cx.listener(|this, checked, _, cx| {
                                    this.mapping_modifiers_enabled = *checked;
                                    if !checked {
                                        this.update_mapping_modifiers(cx, Vec::clear);
                                    }
                                    cx.notify();
                                })),
                        );
                        if self.mapping_modifiers_enabled {
                            body = body.child(h_flex().flex_wrap().gap(surface::css(4.)).children(
                                MODIFIERS.iter().map(|(id, label)| {
                                    let id = *id;
                                    let enabled =
                                        self.mapping_optional_modifiers.iter().any(|m| m == id);
                                    Button::new(SharedString::from(format!(
                                        "mapping-modifier-{id}"
                                    )))
                                    .label(*label)
                                    .compact()
                                    .w(surface::css(102.))
                                    .when(enabled, |button| button.primary())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.update_mapping_modifiers(cx, |modifiers| {
                                            if enabled {
                                                modifiers.retain(|m| m != id);
                                            } else {
                                                modifiers.retain(|modifier| {
                                                    modifier_family(modifier) != modifier_family(id)
                                                });
                                                modifiers.push(id.into());
                                            }
                                        });
                                    }))
                                }),
                            ));
                        }
                    }
                }
            }
            Assignment::Brightness { action } => {
                body = body.children(BRIGHTNESS.iter().map(|(id, label)| {
                    let id = *id;
                    Radio::new(SharedString::from(format!("mapping-brightness-{id}")))
                        .label(i18n::t(label))
                        .checked(action == id)
                        .on_change(cx.listener(move |this, _, _, cx| {
                            this.update_mapping(cx, |action| {
                                if let Assignment::Brightness { action } = action {
                                    *action = id.into();
                                }
                            });
                        }))
                }));
            }
            Assignment::Text { text } => {
                body = body
                    .child(
                        div().id("mapping-text").test_support().child(
                            Textarea::new(&self.controls.mapping_paragraph)
                                .accessibility_id("mapping-text-input")
                                .aria_label("输入文本")
                                .h(surface::css(96.)),
                        ),
                    )
                    .child(
                        div()
                            .text_size(surface::css(12.))
                            .text_color(cx.theme().muted_foreground)
                            .text_right()
                            .child(format!("{}/250", text.encode_utf16().count())),
                    );
            }
            Assignment::Launch { mode, .. } => {
                body = body
                    .children(
                        [("program", "PROGRAM"), ("website", "WEBSITE")]
                            .into_iter()
                            .map(|(id, label)| {
                                Radio::new(SharedString::from(format!("mapping-launch-{id}")))
                                    .label(i18n::t(label))
                                    .checked(mode == id)
                                    .on_change(cx.listener(move |this, _, window, cx| {
                                        this.update_mapping(cx, |action| {
                                            if let Assignment::Launch { mode, target } = action
                                                && mode != id
                                            {
                                                *mode = id.into();
                                                target.clear();
                                            }
                                        });
                                        this.sync_mapping_controls(window, cx);
                                    }))
                            }),
                    )
                    .child(
                        Input::new(&self.controls.mapping_text)
                            .id("mapping-launch-target")
                            .aria_label("程序路径或网站地址"),
                    );
                if mode == "program" {
                    body = body.child(
                        Button::new("mapping-launch-browse")
                            .label(i18n::t("BROWSE"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.browse_mapping_program(window, cx)
                            })),
                    );
                }
            }
            Assignment::Profile {
                action: profile_action,
            } => {
                let single_profile = self.device.profiles.len() <= 1;
                body = body
                    .children(PROFILE_NAVIGATION.iter().map(|(id, label)| {
                        let id = *id;
                        Radio::new(SharedString::from(format!("mapping-profile-{id}")))
                            .label(i18n::t(label))
                            .checked(profile_action == id)
                            .disabled(single_profile)
                            .on_change(cx.listener(move |this, _, window, cx| {
                                this.update_mapping(cx, |action| {
                                    *action = Assignment::Profile { action: id.into() }
                                });
                                this.sync_mapping_controls(window, cx);
                            }))
                    }))
                    .child(
                        Radio::new("mapping-profile-specific")
                            .label(i18n::t("SPECIFIC_PROFILE"))
                            .checked(profile_action.starts_with("profile:"))
                            .disabled(single_profile)
                            .on_change(cx.listener(|this, _, window, cx| {
                                if let Some(profile) = this
                                    .device
                                    .profiles
                                    .iter()
                                    .find(|profile| profile.id != this.device.active_profile)
                                {
                                    let action = format!("profile:{}", profile.id);
                                    this.update_mapping(cx, |assignment| {
                                        *assignment = Assignment::Profile { action }
                                    });
                                    this.sync_mapping_controls(window, cx);
                                }
                            })),
                    );
                if profile_action.starts_with("profile:") {
                    body = body.child(self.mapping_select(&action));
                }
            }
            Assignment::Sensitivity {
                action: sensitivity,
                independent,
                ..
            } => {
                body = body.child(self.mapping_select(&action));
                if sensitivity == "DPI_Clutch" {
                    body = body
                        .child(
                            Checkbox::new("mapping-independent")
                                .label("独立 X/Y 灵敏度")
                                .checked(*independent)
                                .on_change(cx.listener(|this, checked, window, cx| {
                                    this.update_mapping(cx, |action| {
                                        if let Assignment::Sensitivity {
                                            independent, x, y, ..
                                        } = action
                                        {
                                            *independent = *checked;
                                            if !checked {
                                                *y = *x;
                                            }
                                        }
                                    });
                                    this.sync_mapping_controls(window, cx);
                                })),
                        )
                        .child(
                            Input::new(&self.controls.mapping_x)
                                .id("mapping-dpi-x")
                                .aria_label("X DPI")
                                .h(surface::css(27.)),
                        )
                        .child(
                            Slider::new(&self.controls.mapping_sliders[0]).w(surface::css(210.)),
                        );
                    if *independent {
                        body = body
                            .child(
                                Input::new(&self.controls.mapping_y)
                                    .id("mapping-dpi-y")
                                    .aria_label("Y DPI")
                                    .h(surface::css(27.)),
                            )
                            .child(
                                Slider::new(&self.controls.mapping_sliders[1])
                                    .w(surface::css(210.)),
                            );
                    }
                    body = body.child(surface::note("100–30000 DPI · 步长 50", cx));
                } else if sensitivity == "DPI_OnTheFly" {
                    body = body.child(surface::note("按住此按钮并滚动滚轮以调整灵敏度。", cx));
                } else {
                    body = body
                        .children(
                            self.settings()
                                .sensitivity
                                .stages
                                .iter()
                                .zip(&self.settings().sensitivity.slots)
                                .filter(|(_, slot)| slot.enabled)
                                .map(|(values, slot)| {
                                    surface::note(
                                        if !slot.independent {
                                            format!(
                                                "{} {} — {}",
                                                i18n::t("STAGE"),
                                                slot.id,
                                                values[0]
                                            )
                                        } else {
                                            format!(
                                                "{} {} — X: {}  Y: {}",
                                                i18n::t("STAGE"),
                                                slot.id,
                                                values[0],
                                                values[1]
                                            )
                                        },
                                        cx,
                                    )
                                }),
                        )
                        .child(
                            Button::new("mapping-configure-sensitivity")
                                .label(i18n::t("CONFIGURE_SENSITIVITY_SETTINGS"))
                                .ghost()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.continue_with(Continue::Page(Tab::Performance), window, cx)
                                })),
                        );
                }
            }
            Assignment::Unavailable { category, .. } => {
                let reason = match category.as_str() {
                    "macro" => "宏列表需要原生宏服务，当前不可用。",
                    "interdevice" => "跨设备映射需要原生设备服务，当前不可用。",
                    "lighting" => "灯光映射需要 Chroma 应用和配置资源，当前不可用。",
                    _ => "现有映射格式尚未支持；关闭可保留原值。",
                };
                body = body.child(surface::note(reason, cx));
            }
            _ => {
                body = body.child(self.mapping_select(&action));
            }
        }
        if matches!(
            action,
            Assignment::Mouse { .. } | Assignment::Keyboard { .. }
        ) && self.turbo_supported()
        {
            let turbo = action.turbo();
            body = body.child(
                Checkbox::new("mapping-turbo")
                    .label(i18n::t("ENABLE_TURBO"))
                    .checked(turbo.is_some())
                    .on_change(cx.listener(|this, checked: &bool, window, cx| {
                        this.update_mapping(cx, |action| {
                            if let Assignment::Keyboard { turbo, .. }
                            | Assignment::Mouse { turbo, .. } = action
                            {
                                *turbo = checked.then_some(7);
                            }
                        });
                        this.sync_mapping_controls(window, cx);
                    })),
            );
            if turbo.is_some() {
                body = body
                    .child(div().text_size(surface::css(12.)).child("每秒次数（1–20）"))
                    .child(
                        Input::new(&self.controls.mapping_rate)
                            .id("mapping-turbo-rate")
                            .aria_label("Turbo 每秒次数")
                            .h(surface::css(27.)),
                    )
                    .child(Slider::new(&self.controls.mapping_sliders[2]).w(surface::css(210.)));
            }
        }
        if let Some(error) = self.mapping_error() {
            body = body.child(
                div()
                    .id("mapping-validation")
                    .text_size(surface::css(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(error),
            );
        }
        body = body.child(
            h_flex()
                .gap(surface::css(10.))
                .mt(surface::css(10.))
                .child(
                    Button::new("mapping-cancel")
                        .label("取消")
                        .w(surface::css(100.))
                        .h(surface::css(27.))
                        .text_size(surface::css(12.))
                        .line_height(surface::css(14.))
                        .rounded(cx.theme().font_size * (3. / 16.))
                        .border_1()
                        .border_color(cx.theme().title_bar)
                        .py_0()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.continue_with(Continue::CloseMapping, window, cx)
                        })),
                )
                .child(
                    Button::new("mapping-apply")
                        .label("保存")
                        .primary()
                        .w(surface::css(100.))
                        .h(surface::css(27.))
                        .text_size(surface::css(12.))
                        .line_height(surface::css(14.))
                        .rounded(cx.theme().font_size * (3. / 16.))
                        .border_1()
                        .border_color(cx.theme().title_bar)
                        .py_0()
                        .disabled(self.mapping_error().is_some())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.finish_mapping(window, cx);
                        })),
                ),
        );
        let rail_width = if self.mapping_expanded { 230. } else { 40. };
        let rail = v_flex()
            .id("mapping-categories")
            .absolute()
            .left_0()
            .top_0()
            .bottom_0()
            .w(surface::css(rail_width))
            .bg(cx.theme().sidebar)
            .overflow_y_scroll()
            .on_hover(cx.listener(|this, hover, _, cx| {
                this.mapping_expanded = *hover;
                cx.notify();
            }))
            .children(self.mapping_categories().into_iter().map(|category| {
                let active = selected == Some(category);
                BaseButton::new(SharedString::from(format!(
                    "mapping-category-{}",
                    category.id()
                )))
                .accessibility_label(category.label())
                .h(surface::css(40.))
                .w_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(surface::css(20.))
                .px(surface::css(10.))
                .text_size(surface::css(12.))
                .text_color(if active {
                    cx.theme().primary
                } else {
                    cx.theme().foreground
                })
                .when(active, |button| button.bg(cx.theme().popover))
                .hover(|style| style.bg(cx.theme().list_hover))
                .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.select_mapping_category(category, window, cx)
                }))
                .child(
                    img(SharedString::from(format!(
                        "synapse/mapping-{}{}.{}",
                        category.id(),
                        if active { "-active" } else { "" },
                        if category == Category::Lighting {
                            "png"
                        } else {
                            "svg"
                        }
                    )))
                    .size(surface::css(20.))
                    .flex_shrink_0(),
                )
                .when(self.mapping_expanded, |button| {
                    button.child(category.label())
                })
            }));
        v_flex()
            .id("mapping-editor")
            .track_focus(&self.mapping_focus)
            .capture_key_down(cx.listener(|this, event, _, cx| this.capture_mapping_key(event, cx)))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && !this.mapping_recording {
                    cx.stop_propagation();
                    this.continue_with(Continue::CloseMapping, window, cx);
                }
            }))
            .w(surface::css(292.))
            .min_h_0()
            .max_h_full()
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().sidebar_border)
            .rounded(surface::css(5.))
            // Original .key-config.open: 0 0 20px #000000b3.
            .shadow(vec![BoxShadow {
                color: cx.theme().title_bar.opacity(0.7),
                offset: point(Pixels::ZERO, Pixels::ZERO),
                blur_radius: cx.theme().font_size * (20. / 16.),
                spread_radius: Pixels::ZERO,
                inset: false,
            }])
            .child(
                h_flex()
                    .relative()
                    .h(surface::css(36.))
                    .flex_shrink_0()
                    .bg(cx.theme().sidebar)
                    .rounded_t(surface::css(4.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .justify_center()
                    .child(
                        div()
                            .max_w(surface::css(200.))
                            .truncate()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .text_color(cx.theme().muted_foreground)
                            .child(input_label),
                    )
                    .child(
                        BaseButton::new("mapping-close")
                            .accessibility_label("关闭按键映射")
                            .absolute()
                            .right_0()
                            .top_0()
                            .w(surface::css(36.))
                            .h(surface::css(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|button| {
                                button.bg(crate::ui::theme::DropdownColors::new().hover())
                            })
                            .active(|button| button.bg(cx.theme().title_bar.opacity(0.1)))
                            .child(img("synapse/mapping-close.svg").size(surface::css(20.)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.continue_with(Continue::CloseMapping, window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .h(surface::css(570.))
                    .max_h(surface::css(570.))
                    .child(
                        div()
                            .id("mapping-body")
                            .ml(surface::css(40.))
                            .w(surface::css(250.))
                            .h_full()
                            .overflow_y_scroll()
                            .p(surface::css(20.))
                            .child(body),
                    )
                    .child(rail),
            )
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;

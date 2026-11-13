//! Current Customize key tips. Page variants share one presentation and owner.
use super::*;
use razer_widgets::theme::KeyboardMappingTipColors as Colors;
use serde::Deserialize;
use std::{cell::RefCell, time::Duration};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Classic,
    Analog,
}

#[derive(Clone, Copy)]
struct Policy {
    family: Family,
    keypad: bool,
    legacy: bool,
    function_primary: bool,
    mapped_kind: bool,
    raw_default_kind: bool,
    xena: bool,
    remappable_fn: bool,
}
fn policy(pid: u32) -> Option<Policy> {
    let family = match pid {
        555 | 580 | 614 | 642 | 678 | 679 | 688 | 719 | 720 | 721 | 728 | 740 | 741 | 742 | 746
        | 747 | 756 => Family::Analog,
        515 | 521 | 529 | 534 | 542 | 545 | 550 | 551 | 552 | 554 | 556 | 565 | 567 | 574 | 575
        | 577 | 579 | 585 | 590 | 591 | 592 | 593 | 599 | 600 | 602 | 605 | 606 | 617 | 619
        | 620 | 631 | 647 | 653 | 654 | 655 | 658 | 659 | 660 | 661 | 664 | 673 | 674 | 675
        | 677 | 689 | 691 | 697 | 708 | 716 | 717 | 724 | 727 | 739 | 752 | 2595 | 2596 => {
            Family::Classic
        }
        _ => return None,
    };
    Some(Policy {
        family,
        keypad: matches!(pid, 555 | 580 | 747),
        legacy: pid == 752,
        function_primary: matches!(pid, 631 | 708 | 717),
        mapped_kind: pid == 642,
        raw_default_kind: pid == 756,
        xena: matches!(pid, 716 | 724),
        remappable_fn: pid == 747,
    })
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum Counter {
    Text(String),
    Number(serde_json::Number),
}
impl Counter {
    fn text(&self) -> String {
        match self {
            Self::Text(value) => value.clone(),
            Self::Number(value) => value.to_string(),
        }
    }
}
#[derive(Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Group {
    name: Option<String>,
    key: Option<String>,
    modifiers: Option<Vec<String>>,
    #[serde(rename = "interPID")]
    inter_pid: Option<u32>,
    inter_device_type: Option<String>,
    #[serde(rename = "InterDeviceType")]
    capitalized_inter_device_type: Option<String>,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ButtonData {
    button_key: String,
    #[serde(rename = "inputID")]
    input_id: Option<String>,
    counter: Option<Counter>,
    assignment: Option<String>,
    assignment_value: Option<String>,
    default_value: Option<String>,
    assignment_group: Option<Group>,
    secondary_assignment: Option<String>,
    secondary_assignment_value: Option<String>,
    secondary_assignment_group: Option<Group>,
    #[serde(rename = "MappingGroup")]
    mapping_group: Option<String>,
    is_mapped: Option<bool>,
    is_none_tip: Option<bool>,
    disabled_key: Option<bool>,
    #[serde(rename = "unRemappable")]
    unremappable: Option<bool>,
    disable_hypershift_mapping: Option<bool>,
    #[serde(rename = "hasHWFunc")]
    has_hw_func: Option<bool>,
    #[serde(rename = "hasSWFunc")]
    has_sw_func: Option<bool>,
    is_require_synapse: Option<bool>,
    default_require_synapse: Option<bool>,
    condition_text: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GameMode {
    state: u8,
    is_game_on: Option<bool>,
}

#[derive(Clone)]
enum Anchor {
    Shape(String),
    Keypad(String),
    Popup(Rc<Cell<Bounds<Pixels>>>),
}
impl Anchor {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Shape(a), Self::Shape(b)) | (Self::Keypad(a), Self::Keypad(b)) => a == b,
            (Self::Popup(a), Self::Popup(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}
#[derive(Clone)]
struct Target {
    input: String,
    anchor: Anchor,
    button: ButtonData,
}
#[derive(Default)]
pub(super) struct State {
    target: Option<Target>,
    pending: Option<Target>,
    generation: u64,
    closing: bool,
    popup_regions: BTreeMap<String, Rc<Cell<Bounds<Pixels>>>>,
}
impl State {
    fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.target = None;
        self.pending = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    Neutral,
    Required,
    Hyper,
    Unavailable,
}
impl Tone {
    fn border(self) -> Hsla {
        match self {
            Self::Neutral => Colors::border(),
            Self::Required => Colors::required(),
            Self::Hyper => Colors::hyper(),
            Self::Unavailable => Colors::unavailable(),
        }
    }
}
struct Notice {
    text: String,
    tone: Option<Tone>,
}
struct Card {
    index: String,
    peer_name: Option<String>,
    kind: Option<String>,
    name: String,
    secondary: Option<(String, String)>,
    notice: Option<Notice>,
    border: Tone,
    compact: bool,
    unlimited: bool,
    tall_name: bool,
    secondary_padding: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Peer {
    Unobserved,
    Present,
    Absent,
}
struct PresentationContext {
    pid: u32,
    policy: Policy,
    shift: bool,
    primary: Option<razer_device::keyboard_function_swap::FunctionKeyPrimary>,
    gaming: Option<GameMode>,
    peer: Peer,
}

fn reserved(pid: u32, input: &str) -> bool {
    matches!(
        input,
        "KEY_WINDOWS"
            | "KEY_FN"
            | "LEFT_FN"
            | "RIGHT_FN"
            | "DKM_KB_LEFTKNOB"
            | "DKM_SB_65"
            | "DKM_SB_67"
            | "DKM_SB_66"
    ) || pid != 756 && input == "DKM_KB_XBOX"
}
fn media(pid: u32, input: &str) -> bool {
    matches!(
        input,
        "MEDIA_PREV"
            | "MEDIA_PAUSE_PLAY"
            | "MEDIA_NEXT"
            | "DIAL_LEFT_ROTATION"
            | "DIAL_CLICK"
            | "DIAL_RIGHT_ROTATION"
    ) || !matches!(pid, 752 | 756) && input == "DKM_KB_SIDE2"
}
fn function_key(input: &str) -> bool {
    matches!(
        input,
        "KEY_F1"
            | "KEY_F2"
            | "KEY_F3"
            | "KEY_F4"
            | "KEY_F5"
            | "KEY_F6"
            | "KEY_F7"
            | "KEY_F8"
            | "KEY_F9"
            | "KEY_F10"
            | "KEY_F11"
            | "KEY_F12"
    )
}
fn special_name(value: &str) -> bool {
    matches!(value, "Razer Hypershift" | "Joystick (Analog)")
}

fn localize(value: &str, policy: Policy) -> String {
    let key = value
        .chars()
        .map(|c| if c.is_whitespace() || c == '/' { '_' } else { c })
        .collect::<String>()
        .to_uppercase();
    let recognized = match policy.family {
        Family::Classic => {
            matches!(
                key.as_str(),
                "BLUETOOTH"
                    | "BRIGHTNESS_DOWN"
                    | "BRIGHTNESS_UP"
                    | "CANNOT_REMAPPED_IN_HYPERSHIFT_MODE"
                    | "CYCLE_APPS"
                    | "CYCLE_UP_PROFILE"
                    | "DEVICE_NOT_CONNECTED"
                    | "KEYBOARD_BACKLIGHT"
                    | "KEY_SYSTEM_GROUP"
                    | "MACRO_MODULE_IS_NOT_INSTALLED_MSG"
                    | "MAPPING_REQUIRES_RAZER_SYNAPSE"
                    | "MENUKEY_DESC_GAMING_MODE_ON"
                    | "MIC_MUTE"
                    | "MUTE_VOLUME"
                    | "NEXT_TRACK"
                    | "PLAY_PAUSE"
                    | "PREVIOUS_TRACK"
                    | "PRINT_SCREEN"
                    | "SCREEN_DISPLAY"
                    | "SCREEN_LOCK"
                    | "SECOND_KEY_TIP"
                    | "SENSITIVITY"
                    | "SLEEP"
                    | "SWITCH_KEYBOARD_MODE"
                    | "SWITCH_KEYMAP"
                    | "SWITCH_PROFILE"
                    | "TOGGLE_GAME_MODE"
                    | "TOGGLE_MACRO_MODE"
                    | "VOLUME_DOWN"
                    | "VOLUME_UP"
                    | "WINKEY_DESC_GAMING_MODE_ON"
            ) || policy.xena && key == "AI_PROMPT_MASTER"
        }
        Family::Analog => matches!(
            key.as_str(),
            "ACTUATION"
                | "ANALOG"
                | "BOTTOM"
                | "DELETE"
                | "DOUBLE_TAP"
                | "DOWN"
                | "HIGH"
                | "JOYSTICK"
                | "LED_TOGGLE_INDICATORS"
                | "LEFT"
                | "MENU"
                | "MUTE"
                | "MUTE_VOLUME"
                | "NEXT"
                | "NEXT_TRACK"
                | "PAUSE"
                | "PLAY_PAUSE"
                | "PREVIOUS"
                | "PREVIOUS_TRACK"
                | "QUICK"
                | "RIGHT"
                | "SCROLL_CLICK"
                | "SCROLL_DOWN"
                | "SCROLL_UP"
                | "SINGLE_TAP"
                | "SLEEP"
                | "TOP"
                | "TRIPLE_TAP"
                | "UP"
                | "VOLUME"
                | "VOLUME_DOWN"
                | "VOLUME_UP"
        ),
    };
    if recognized {
        return t(&key);
    }
    let alias = match value {
        "Decrease Brightness" => Some("BRIGHTNESS_DOWN"),
        "Increase Brightness" => Some("BRIGHTNESS_UP"),
        "SYSTEM GROUP" => Some("KEY_SYSTEM_GROUP"),
        "synapse last used" if policy.family == Family::Classic => Some("SYNAPSE_LAST_USED"),
        "profile slot 1" if policy.family == Family::Classic => Some("PROFILE_SLOT_1"),
        "profile slot 2" if policy.family == Family::Classic => Some("PROFILE_SLOT_2"),
        "profile slot 3" if policy.family == Family::Classic => Some("PROFILE_SLOT_3"),
        "profile slot 4" if policy.family == Family::Classic => Some("PROFILE_SLOT_4"),
        _ => None,
    };
    t(alias.unwrap_or(value))
}
fn peer_kind(group: Option<&Group>) -> String {
    match group {
        Some(group) if group.inter_device_type.as_deref() == Some("profileNavigationGroup") => {
            t("SWITCH_PROFILE")
        }
        Some(group) if group.capitalized_inter_device_type.as_deref() == Some("keymapGroup") => {
            t("SWITCH_KEYMAP")
        }
        Some(_) => t("SENSITIVITY"),
        None => String::new(),
    }
}
impl PresentationContext {
    fn game_active(&self) -> bool {
        self.gaming
            .as_ref()
            .is_some_and(|game| game.state == 1 || game.state == 2 && game.is_game_on == Some(true))
    }
    fn system_function(&self, button: &ButtonData) -> bool {
        self.policy.function_primary
            && button
                .input_id
                .as_deref()
                .is_some_and(|id| function_key(id) || self.pid == 717 && id == "KEY_WINDOWS")
            && !(self.pid == 717 && button.assignment.as_deref() == Some("RAZER_HYPERSHIFT"))
    }
    fn function_primary(&self) -> bool {
        self.primary == Some(razer_device::keyboard_function_swap::FunctionKeyPrimary::Function)
    }
    fn multimedia_primary(&self) -> bool {
        self.primary == Some(razer_device::keyboard_function_swap::FunctionKeyPrimary::Multimedia)
    }
    fn system_layer(&self, button: &ButtonData) -> bool {
        self.system_function(button)
            && (self.shift && self.function_primary() || !self.shift && self.multimedia_primary())
    }
    fn readonly(&self, button: &ButtonData) -> bool {
        let id = button.input_id.as_deref().unwrap_or("");
        let hyper = button.assignment.as_deref() == Some("RAZER_HYPERSHIFT");
        let remappable = self.policy.remappable_fn
            && matches!(
                id,
                "KEY_FN" | "DKM_KEYPAD_103" | "DKM_KEYPAD_104" | "KEY_ID_RIGHT_STICK_BTN"
            )
            && !self.shift;
        let fixed =
            reserved(self.pid, id) && !remappable && (self.policy.legacy || !hyper || !self.shift);
        let restricted_layer = self.function_primary()
            || self.primary.is_none() && !self.policy.keypad
            || self.policy.family == Family::Analog && self.policy.keypad
            || self.policy.family == Family::Classic
                && self.multimedia_primary()
                && !self.system_function(button);
        let gaming_key = if self.policy.family == Family::Classic {
            matches!(id, "KEY_APPLICATION" | "DKM_F6")
                || id == "DKM_D2"
                    && (!self.shift || self.gaming.as_ref().is_some_and(|g| g.state == 2))
        } else {
            id == "KEY_APPLICATION"
        };
        fixed
            || button.unremappable == Some(true)
            || button.disabled_key == Some(true)
            || button.disable_hypershift_mapping == Some(true)
                && !hyper
                && self.shift
                && restricted_layer
            || self.system_layer(button)
            || self.policy.family == Family::Classic
                && (id == "DKM_D2" || self.shift && button.has_hw_func == Some(true))
            || self.game_active() && gaming_key
    }
    fn requires(&self, button: &ButtonData) -> bool {
        let id = button.input_id.as_deref().unwrap_or("");
        button.is_require_synapse == Some(true)
            || self.policy.family == Family::Classic
                && (!self.policy.legacy
                    && (media(self.pid, &button.button_key) && button.is_mapped == Some(true)
                        || self.policy.xena && id == "DKM_SB_247")
                    || button.default_value.as_deref() != Some("DISABLE")
                        && button.has_sw_func == Some(true))
    }
    fn kind(&self, button: &ButtonData, secondary: bool) -> Option<String> {
        let assignment = if secondary {
            button.secondary_assignment.as_deref()
        } else {
            button.assignment.as_deref()
        }?;
        if button.assignment.as_deref() == Some("INTERDEVICE") {
            return Some(peer_kind(button.assignment_group.as_ref()));
        }
        if !secondary && assignment == "DISABLE" {
            return None;
        }
        let mapping_kind = if self.policy.family == Family::Analog {
            self.shift && !(self.policy.mapped_kind && button.is_mapped == Some(true))
        } else {
            button.disable_hypershift_mapping == Some(true)
                && (self.shift && self.function_primary()
                    || !self.shift && self.multimedia_primary() && self.system_function(button)
                    || self.shift && self.primary.is_none())
        };
        if mapping_kind && let Some(kind) = &button.mapping_group {
            return Some(localize(kind, self.policy));
        }
        Some(t(assignment))
    }
    fn card(&self, button: &ButtonData, primary_name: String, secondary_name: String) -> Card {
        let mapped = button.is_mapped == Some(true);
        let disabled = mapped && button.assignment_value.as_deref() == Some("DISABLE");
        let hyper = button.assignment.as_deref() == Some("RAZER_HYPERSHIFT");
        let required = self.requires(button);
        let mut border = if disabled {
            Tone::Unavailable
        } else if mapped {
            if self.shift { Tone::Hyper } else { Tone::Required }
        } else {
            Tone::Neutral
        };
        let mut notice = None;
        let id = button.input_id.as_deref().unwrap_or("");
        if self.readonly(button) {
            let gaming_key = self.game_active()
                && if self.policy.family == Family::Classic {
                    matches!(
                        id,
                        "KEY_WINDOWS" | "DKM_KB_XBOX" | "KEY_APPLICATION" | "DKM_F6"
                    ) || id == "DKM_D2" && !self.shift
                } else {
                    matches!(id, "KEY_WINDOWS" | "KEY_APPLICATION")
                };
            let message = if gaming_key {
                if matches!(id, "KEY_WINDOWS" | "DKM_KB_XBOX")
                    && !(self.policy.family == Family::Classic && self.shift)
                {
                    "WINKEY_DESC_GAMING_MODE_ON"
                } else {
                    "MENUKEY_DESC_GAMING_MODE_ON"
                }
            } else {
                "SECOND_KEY_TIP"
            };
            if self.game_active() {
                border = if gaming_key {
                    Tone::Unavailable
                } else {
                    Tone::Neutral
                };
            }
            notice = Some(Notice {
                text: t(message),
                tone: Some(if gaming_key {
                    Tone::Unavailable
                } else {
                    Tone::Neutral
                }),
            });
            if self.policy.xena
                && id == "DKM_D2"
                && !self.shift
                && self.gaming.as_ref().is_some_and(|game| game.state == 0)
            {
                notice = None;
            }
        } else {
            let standard_exception = id == "KEY_APPLICATION"
                || !self.shift
                    && !self.policy.keypad
                    && if self.policy.family == Family::Classic {
                        self.function_primary()
                    } else {
                        self.multimedia_primary()
                    };
            let disconnected =
                button.assignment.as_deref() == Some("INTERDEVICE") && self.peer == Peer::Absent;
            if standard_exception {
                if self.policy.family == Family::Classic && disconnected {
                    border = Tone::Unavailable;
                    notice = Some(Notice {
                        text: t("DEVICE_NOT_CONNECTED"),
                        tone: Some(border),
                    });
                } else if self.policy.family == Family::Classic && !self.policy.legacy && required {
                    border = if self.shift { Tone::Hyper } else { Tone::Required };
                    notice = Some(Notice {
                        text: t("MAPPING_REQUIRES_RAZER_SYNAPSE"),
                        tone: Some(border),
                    });
                }
            } else if hyper && self.shift {
                border = Tone::Unavailable;
                notice = Some(Notice {
                    text: t("CANNOT_REMAPPED_IN_HYPERSHIFT_MODE"),
                    tone: Some(border),
                });
            } else if disconnected {
                border = Tone::Unavailable;
                notice = Some(Notice {
                    text: t("DEVICE_NOT_CONNECTED"),
                    tone: Some(border),
                });
            } else if required
                || self.policy.family == Family::Analog
                    && (button.default_require_synapse == Some(true)
                        || self.policy.keypad && mapped)
            {
                border = if self.shift { Tone::Hyper } else { Tone::Required };
                notice = Some(Notice {
                    text: t("MAPPING_REQUIRES_RAZER_SYNAPSE"),
                    tone: Some(border),
                });
            } else if self.policy.family == Family::Classic {
                border = Tone::Neutral;
            }
        }
        if self.policy.family == Family::Classic
            && let Some(condition) = button.condition_text.as_ref().filter(|v| !v.is_empty())
        {
            notice = Some(Notice {
                text: if required {
                    format!("{}\n{}", t(condition), t("MAPPING_REQUIRES_RAZER_SYNAPSE"))
                } else {
                    t(condition)
                },
                tone: if required {
                    notice.and_then(|n| n.tone)
                } else {
                    Some(Tone::Neutral)
                },
            });
        }
        let counter = button
            .counter
            .as_ref()
            .map(Counter::text)
            .unwrap_or_else(|| "key".into());
        let index = if matches!(counter.as_str(), "SINGLE_TAP" | "DOUBLE_TAP" | "TRIPLE_TAP") {
            t(&counter)
        } else if self.policy.xena && counter == "AI Prompt Master" {
            t("AI_PROMPT_MASTER")
        } else {
            counter.clone()
        };
        let compact = disabled
            || button.assignment_value.as_deref().is_some_and(|value| {
                if self.policy.legacy {
                    value == "Razer Hypershift"
                } else {
                    special_name(value)
                }
            });
        let mut kind = (!compact).then(|| self.kind(button, false)).flatten();
        if self.policy.family == Family::Analog
            && button
                .assignment_group
                .as_ref()
                .and_then(|g| g.key.as_deref())
                == button.input_id.as_deref()
            && button
                .assignment_group
                .as_ref()
                .and_then(|g| g.key.as_ref())
                .is_some()
            && button.assignment.as_deref() != Some("DISABLE")
        {
            kind = Some(if self.policy.raw_default_kind {
                "KEYBOARD".into()
            } else {
                t("KEYBOARD_FUNCTION")
            });
        }
        let secondary = (self.policy.family == Family::Analog
            && button.secondary_assignment_group.is_some()
            && button
                .secondary_assignment
                .as_deref()
                .is_some_and(|kind| kind != "DEFAULT"))
        .then(|| (self.kind(button, true).unwrap_or_default(), secondary_name));
        Card {
            index: if matches!(counter.as_str(), "ß" | "à" | "è" | "é" | "ù" | "ç") {
                index.to_lowercase()
            } else {
                index.to_uppercase()
            },
            peer_name: (button.assignment.as_deref() == Some("INTERDEVICE")).then(|| {
                button
                    .assignment_group
                    .as_ref()
                    .and_then(|g| g.name.clone())
                    .unwrap_or_default()
            }),
            kind,
            name: localize(&primary_name, self.policy),
            secondary,
            notice,
            border,
            compact,
            unlimited: hyper && self.shift,
            tall_name: self.policy.family == Family::Classic && counter == "Å",
            secondary_padding: self.policy.family == Family::Analog
                && button
                    .secondary_assignment_value
                    .as_ref()
                    .is_some_and(|value| !value.is_empty()),
        }
    }
}

impl KeyboardProductWorkspace {
    pub(in crate::features::keyboard_products) fn reset_mapping_key_tip(&mut self) {
        self.analog_gamepad.mapping.key_tip.invalidate();
    }
    pub(in crate::features::keyboard_products) fn reconnect_mapping_key_tip(&mut self) {
        self.reset_mapping_key_tip();
        self.analog_gamepad.mapping.key_tip.closing = false;
    }
    pub(in crate::features::keyboard_products) fn stop_mapping_key_tip(&mut self) {
        self.reset_mapping_key_tip();
        self.analog_gamepad.mapping.key_tip.closing = true;
    }
    pub(in crate::features::keyboard_products) fn mapping_key_tip_supported(&self) -> bool {
        self.page == "TAB_CUSTOMIZE" && policy(self.spec.product_id).is_some()
    }
    pub(in crate::features::keyboard_products) fn hover_mapping_key_tip(
        &mut self,
        input: &str,
        region: &str,
        entered: bool,
        cx: &mut Context<Self>,
    ) {
        self.capture_key_tip(input, Anchor::Shape(region.into()), entered, cx);
    }
    pub(super) fn hover_keypad_key_tip(
        &mut self,
        input: &str,
        entered: bool,
        cx: &mut Context<Self>,
    ) {
        self.capture_key_tip(input, Anchor::Keypad(input.into()), entered, cx);
    }
    pub(in crate::features::keyboard_products) fn hover_special_key_tip(
        &mut self,
        input: &str,
        entered: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self
            .analog_gamepad
            .mapping
            .key_tip
            .popup_regions
            .get(input)
            .cloned()
        else {
            return;
        };
        self.capture_key_tip(input, Anchor::Popup(bounds), entered, cx);
    }
    pub(in crate::features::keyboard_products) fn special_key_tip_bounds(
        &self,
        input: &str,
    ) -> Option<Rc<Cell<Bounds<Pixels>>>> {
        self.analog_gamepad
            .mapping
            .key_tip
            .popup_regions
            .get(input)
            .cloned()
    }
    pub(in crate::features::keyboard_products) fn dismiss_mapping_popup_tip(
        &mut self,
        position: Point<Pixels>,
    ) {
        if self.analog_gamepad.mapping.key_tip.target.as_ref().is_some_and(|target| {
            matches!(&target.anchor, Anchor::Popup(bounds) if !bounds.get().contains(&position))
        }) { self.reset_mapping_key_tip(); }
    }
    pub(in crate::features::keyboard_products) fn prepare_mapping_key_tip_regions(&mut self) {
        let regions = self
            .mapping_buttons()
            .iter()
            .filter_map(|row| row.get("buttonKey").and_then(Value::as_str))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let state = &mut self.analog_gamepad.mapping.key_tip;
        state.invalidate();
        state.popup_regions = regions
            .into_iter()
            .map(|id| (id, Rc::new(Cell::new(Bounds::default()))))
            .collect();
    }
    fn capture_key_tip(
        &mut self,
        input: &str,
        anchor: Anchor,
        entered: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.mapping_key_tip_supported() || self.analog_gamepad.mapping.key_tip.closing {
            return;
        }
        if !entered {
            let state = &self.analog_gamepad.mapping.key_tip;
            if state
                .target
                .iter()
                .chain(state.pending.iter())
                .any(|target| target.input == input && target.anchor.same(&anchor))
            {
                self.reset_mapping_key_tip();
            }
            return;
        }
        if matches!(input, "mediaVolume" | "pauseandplay") {
            self.reset_mapping_key_tip();
            return;
        }
        if self
            .analog_gamepad
            .mapping
            .key_tip
            .target
            .as_ref()
            .is_some_and(|target| target.input == input && target.anchor.same(&anchor))
        {
            return;
        }
        let Some(raw) = self.mapping_buttons().iter().find(|row| {
            row.get("inputID").and_then(Value::as_str) == Some(input)
                || row.get("buttonKey").and_then(Value::as_str) == Some(input)
        }) else {
            return;
        };
        let button = self.mapping_active_button(raw);
        let button = match serde_json::from_value::<ButtonData>(button) {
            Ok(button) => button,
            Err(error) => {
                self.reset_mapping_key_tip();
                cx.emit(super::super::super::KeyboardLegacyRuntimeStatus(format!(
                    "Invalid key tooltip data: {error}"
                )));
                return;
            }
        };
        let target = Target {
            input: input.into(),
            anchor,
            button,
        };
        self.analog_gamepad.mapping.key_tip.generation = self
            .analog_gamepad
            .mapping
            .key_tip
            .generation
            .wrapping_add(1);
        let generation = self.analog_gamepad.mapping.key_tip.generation;
        if self.spec.product_id == 708 {
            self.analog_gamepad.mapping.key_tip.pending = Some(target);
            let owner = cx.entity().downgrade();
            cx.defer(move |cx| {
                let _ = owner.update(cx, |this, cx| {
                    if this.analog_gamepad.mapping.key_tip.generation == generation
                        && !this.analog_gamepad.mapping.key_tip.closing
                    {
                        let state = &mut this.analog_gamepad.mapping.key_tip;
                        state.target = state.pending.take();
                        cx.notify();
                    }
                });
            });
        } else {
            self.analog_gamepad.mapping.key_tip.target = Some(target);
        }
    }

    fn key_tip_context(&self, button: &ButtonData) -> anyhow::Result<PresentationContext> {
        let policy = policy(self.spec.product_id).ok_or_else(|| {
            anyhow::anyhow!("Key tooltip product is outside the current source scope")
        })?;
        let primary = policy
            .function_primary
            .then(|| self.function_primary())
            .flatten();
        anyhow::ensure!(
            !policy.function_primary
                || primary.is_some()
                || self.draft.get("functionKeyPrimary").is_none(),
            "Unknown function-key primary value"
        );
        let gaming = self
            .draft
            .get("gamingMode")
            .cloned()
            .map(serde_json::from_value::<GameMode>)
            .transpose()?;
        anyhow::ensure!(
            gaming.as_ref().is_none_or(|game| game.state <= 2),
            "Unknown gaming-mode value"
        );
        let peer = match self.mapping_inter_observed_devices() {
            None => Peer::Unobserved,
            Some(devices) => {
                if button
                    .assignment_group
                    .as_ref()
                    .and_then(|g| g.inter_pid)
                    .is_some_and(|pid| {
                        devices.iter().any(|device| {
                            device.get("productId").and_then(Value::as_u64) == Some(u64::from(pid))
                        })
                    })
                {
                    Peer::Present
                } else {
                    Peer::Absent
                }
            }
        };
        Ok(PresentationContext {
            pid: self.spec.product_id,
            policy,
            shift: self.hypershift,
            primary,
            gaming,
            peer,
        })
    }
    fn key_tip_assignment(
        &self,
        button: &ButtonData,
        context: &PresentationContext,
        secondary: bool,
    ) -> String {
        let group = if secondary {
            button.secondary_assignment_group.as_ref()
        } else {
            button.assignment_group.as_ref()
        };
        let value = if secondary {
            button.secondary_assignment_value.as_deref()
        } else {
            button.assignment_value.as_deref()
        };
        let translate_keys = if context.policy.family == Family::Classic {
            self.observed_mapping_system_layout()
                .is_some_and(|id| id != 0)
        } else {
            self.analog_gamepad.layout.is_some_and(|id| id != 0)
        };
        if translate_keys
            && let Some(group) = group
            && let Some(key) = &group.key
        {
            return group
                .modifiers
                .iter()
                .flatten()
                .chain(std::iter::once(key))
                .map(|key| self.mapping_key_name_for_layout(key, self.analog_gamepad.layout))
                .filter(|name| !name.is_empty())
                .collect::<Vec<_>>()
                .join(" + ");
        }
        value.unwrap_or("").into()
    }
    fn key_tip_name(&self, button: &ButtonData, context: &PresentationContext) -> String {
        let Some(assignment) = button.assignment_value.as_deref() else {
            return String::new();
        };
        let mapped = button.is_mapped == Some(true);
        let default = button.default_value.as_deref().unwrap_or("");
        let counter = button
            .counter
            .as_ref()
            .map(Counter::text)
            .unwrap_or_default();
        let media = media(context.pid, &button.button_key);
        let side = matches!(
            button.button_key.as_str(),
            "DKM_KB_SIDE1" | "DKM_KB_SIDE2" | "DKM_KB_SIDE3"
        );
        if context.policy.function_primary {
            let id = button.input_id.as_deref().unwrap_or("");
            if mapped
                || context.shift && button.has_hw_func == Some(true)
                || reserved(context.pid, id)
                || context.shift && context.function_primary()
                || context.shift && context.multimedia_primary() && button.button_key == "DKM_D2"
                || !context.shift && context.multimedia_primary() && context.system_function(button)
            {
                return assignment.into();
            }
            return if !mapped
                && (media
                    || side
                    || default == "DISABLE"
                    || default != "DISABLE" && button.has_sw_func == Some(true))
            {
                default.into()
            } else {
                counter
            };
        }
        if !mapped
            && (media || default == "DISABLE" || context.policy.family == Family::Classic && side)
        {
            return default.into();
        }
        if context.shift || mapped {
            return self.key_tip_assignment(button, context, false);
        }
        if assignment == "Razer Hypershift"
            || context.policy.keypad
            || context.policy.family == Family::Classic
                && button.input_id.as_deref().is_some_and(|id| {
                    matches!(
                        id,
                        "DKM_M_01" | "DKM_M_02" | "DKM_M_03" | "DKM_M_04" | "DKM_M_05" | "DKM_M_06"
                    )
                })
        {
            assignment.into()
        } else {
            counter
        }
    }
    pub(in crate::features::keyboard_products) fn mapping_key_tip(&self) -> Option<AnyElement> {
        if !self.mapping_key_tip_supported() {
            return None;
        }
        let target = self.analog_gamepad.mapping.key_tip.target.as_ref()?;
        if target.button.is_none_tip == Some(true) {
            return None;
        }
        let card = match self.key_tip_context(&target.button) {
            Ok(context) => context.card(
                &target.button,
                self.key_tip_name(&target.button, &context),
                self.key_tip_assignment(&target.button, &context, true),
            ),
            Err(error) => Card {
                index: target.input.clone(),
                peer_name: None,
                kind: None,
                name: format!("Invalid mapping data: {error}"),
                secondary: None,
                notice: None,
                border: Tone::Unavailable,
                compact: false,
                unlimited: false,
                tall_name: false,
                secondary_padding: false,
            },
        };
        let policy = policy(self.spec.product_id)?;
        Some(
            razer_assets::deferred_resource(KeyTip {
                card,
                anchor: target.anchor.clone(),
                policy,
                shapes: self.analog_gamepad.floating.key_bounds.clone(),
                keypad: self.analog_gamepad.mapping.button_regions.clone(),
                viewport: self.analog_gamepad.mapping.viewport_bounds.clone(),
            })
            .with_priority(999)
            .into_any_element(),
        )
    }
}

type ShapeBounds =
    Rc<RefCell<BTreeMap<String, (&'static razer_assets::KeyboardKey, Bounds<Pixels>)>>>;
#[derive(IntoElement)]
struct KeyTip {
    card: Card,
    anchor: Anchor,
    policy: Policy,
    shapes: ShapeBounds,
    keypad: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    viewport: Rc<Cell<Bounds<Pixels>>>,
}
impl Card {
    fn label(&self) -> String {
        std::iter::once(self.index.as_str())
            .chain(self.peer_name.as_deref())
            .chain(self.kind.as_deref())
            .chain(std::iter::once(self.name.as_str()))
            .chain(self.secondary.as_ref().map(|(_, name)| name.as_str()))
            .chain(self.notice.as_ref().map(|notice| notice.text.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn render(self) -> AnyElement {
        let label = self.label();
        let name_line = if self.tall_name { 26. } else { 18. };
        let header = v_flex()
            .py(surface::css(8.))
            .px(surface::css(10.))
            .child(
                div()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(14.))
                    .mb(surface::css(6.))
                    .text_color(Colors::index())
                    .child(self.index),
            )
            .children(self.peer_name.map(|name| {
                div()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(14.))
                    .mb(surface::css(6.))
                    .text_color(Colors::index())
                    .child(name.to_uppercase())
            }))
            .children(self.kind.map(|kind| {
                div()
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .child(kind.to_uppercase())
            }))
            .child(
                div()
                    .id("keyboard-key-tip-name")
                    .role(Role::Label)
                    .test_support()
                    .aria_label(self.name.clone())
                    .text_size(surface::css(18.))
                    .line_height(surface::css(name_line))
                    .min_h(surface::css(19.))
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(self.name),
            );
        let details = v_flex()
            .when(self.secondary_padding, |body| {
                body.px(surface::css(10.)).pb(surface::css(10.))
            })
            .child(header)
            .children(self.secondary.map(|(kind, name)| {
                v_flex()
                    .mt(surface::css(20.))
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(14.))
                            .mb(surface::css(6.))
                            .text_color(Colors::index())
                            .child(t("SECONDARYFUNCTIONCAPS").to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .child(kind.to_uppercase()),
                    )
                    .child(
                        div()
                            .text_size(surface::css(18.))
                            .line_height(surface::css(18.))
                            .min_h(surface::css(19.))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(t(&name)),
                    )
            }));
        v_flex()
            .id("keyboard-mapping-key-tip")
            .role(Role::Tooltip)
            .test_support()
            .aria_label(label)
            .font_family("Roboto")
            .text_color(Colors::text())
            .line_height(surface::css(14.))
            .text_center()
            .min_h(surface::css(if self.compact { 60. } else { 75. }))
            .when(!self.unlimited, |tip| tip.max_w(surface::css(300.)))
            .border_1()
            .border_color(self.border.border())
            .bg(Colors::background())
            .child(details)
            .children(self.notice.map(|notice| {
                div()
                    .id("keyboard-key-tip-notice")
                    .role(Role::Label)
                    .test_support()
                    .aria_label(notice.text.clone())
                    .text_size(surface::css(12.))
                    .py(surface::css(2.))
                    .px(surface::css(10.))
                    .when_some(notice.tone, |message, tone| {
                        message
                            .bg(tone.border())
                            .text_color(if tone == Tone::Neutral {
                                Colors::text()
                            } else {
                                Colors::dark_text()
                            })
                    })
                    .child(notice.text)
            }))
            .with_animation(
                "keyboard-key-tip-opacity",
                Animation::new(Duration::from_millis(300))
                    .with_easing(|amount| gpui_kit::base::motion::Easing::Ease.sample(amount)),
                |tip, amount| tip.opacity(amount),
            )
            .into_any_element()
    }
}
impl RenderOnce for KeyTip {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        PositionedTip {
            anchor: self.anchor,
            policy: self.policy,
            shapes: self.shapes,
            keypad: self.keypad,
            viewport: self.viewport,
            child: self.card.render(),
            child_layout: None,
        }
    }
}
struct PositionedTip {
    anchor: Anchor,
    policy: Policy,
    shapes: ShapeBounds,
    keypad: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    viewport: Rc<Cell<Bounds<Pixels>>>,
    child: AnyElement,
    child_layout: Option<LayoutId>,
}
impl IntoElement for PositionedTip {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for PositionedTip {
    type RequestLayoutState = ();
    type PrepaintState = bool;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let child = self.child.request_layout(window, cx);
        self.child_layout = Some(child);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    display: Display::Flex,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let target = match &self.anchor {
            Anchor::Shape(region) => self.shapes.borrow().get(region).map(|(_, bounds)| *bounds),
            Anchor::Keypad(input) => self.keypad.borrow().get(input).copied(),
            Anchor::Popup(bounds) => Some(bounds.get()),
        };
        let Some(target) =
            target.filter(|bounds| bounds.size.width > px(0.) && bounds.size.height > px(0.))
        else {
            return false;
        };
        let Some(layout) = self.child_layout else {
            return false;
        };
        let child = window.layout_bounds(layout);
        let viewport = self.viewport.get();
        let gap = surface::css(5.).to_pixels(window.rem_size());
        let mut left = px(f32::from(target.center().x - child.size.width / 2.).ceil());
        if self.policy.family == Family::Classic && viewport.size.width > px(0.) {
            if left < viewport.left() {
                left = viewport.left() + gap;
            }
            if left + child.size.width > viewport.right() {
                left = viewport.right() - child.size.width - gap;
            }
        }
        let top_edge = viewport.top()
            + if self.policy.family == Family::Classic {
                surface::css(10.).to_pixels(window.rem_size())
            } else {
                px(0.)
            };
        let above = target.top() - child.size.height - gap;
        let top = if above >= top_edge {
            above
        } else {
            target.bottom()
                + if self.policy.family == Family::Analog {
                    gap
                } else {
                    px(0.)
                }
        };
        window.with_element_offset(point(left, top) - bounds.origin, |window| {
            self.child.prepaint(window, cx)
        });
        true
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        painted: &mut bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if *painted {
            self.child.paint(window, cx);
        }
    }
}

#[cfg(test)]
#[path = "keyboard_mapping_key_tooltip_tests.rs"]
mod tests;

//! Current keyboard ButtonPanel and mapping popup; vendor values stay unencoded.
//! Receipts: tools/audit_keyboard_mapping_drawer.cjs. No vendor code is executed.
use super::*;

#[derive(Deserialize)]
struct Navigation {
    product_id: u32,
    confirm_mapping_history: bool,
    confirm_mapping_navigation: bool,
}
use gpui_kit::component::button::ButtonVariants;
use serde::Serialize;
use std::{cell::Cell, rc::Rc};

#[path = "keyboard_mapping_header.rs"]
mod header;
#[path = "keyboard_mapping_keypad_artwork.rs"]
mod keypad_artwork;
pub use keypad_artwork::KeyboardKeypadThumbstickObservation;
pub use keypad_artwork::source_keypad_product_image;
#[path = "keyboard_mapping_keypad_panel.rs"]
mod keypad_panel;
#[path = "keyboard_mapping_panel_dropdown.rs"]
mod panel_dropdown;
#[path = "keyboard_mapping_panel_index.rs"]
mod panel_index;
#[path = "keyboard_mapping_panel_secondary.rs"]
mod panel_secondary;
#[path = "keyboard_mapping_panel_value.rs"]
mod panel_value;
#[path = "keyboard_mapping_position.rs"]
mod position;

#[path = "keyboard_mapping_active_button.rs"]
mod active_button;
#[path = "keyboard_mapping_audio_brightness.rs"]
mod audio_brightness;
pub use audio_brightness::{
    KeyboardMappingBrightnessObservation, KeyboardMappingBrightnessReadRequested,
};
#[path = "keyboard_mapping_dial.rs"]
mod dial;
#[path = "keyboard_mapping_dynamic.rs"]
mod dynamic;
pub use dial::{KeyboardMappingDialObservation, KeyboardMappingDialReadRequested};
#[path = "keyboard_mapping_ai.rs"]
mod ai;
#[path = "keyboard_mapping_blade.rs"]
mod blade;
#[path = "keyboard_mapping_controller.rs"]
mod controller;
#[path = "keyboard_mapping_joystick.rs"]
mod joystick;
#[path = "keyboard_mapping_keyboard.rs"]
mod keyboard;
pub use ai::KeyboardMappingAiObservation;
#[path = "keyboard_mapping_app_specific.rs"]
mod app_specific;
#[path = "keyboard_mapping_inter_device.rs"]
mod inter_device;
pub use inter_device::KeyboardMappingInterDeviceConfigureRequested;
pub use inter_device::KeyboardMappingInterDeviceObservation;
#[path = "keyboard_mapping_confirmation.rs"]
mod confirmation;
#[path = "keyboard_mapping_sensitivity_warning.rs"]
mod sensitivity_warning;
pub use sensitivity_warning::{
    KeyboardMappingSensitivityWarningHidden, KeyboardMappingSensitivityWarningReadRequested,
};
#[path = "keyboard_mapping_keymap.rs"]
mod keymap;
#[path = "keyboard_mapping_lighting.rs"]
mod lighting;
#[path = "keyboard_mapping_macro.rs"]
mod macros;
#[path = "keyboard_mapping_notices.rs"]
mod notices;
#[path = "keyboard_mapping_options.rs"]
mod options;
#[path = "keyboard_mapping_targets.rs"]
mod targets;
#[path = "keyboard_mapping_turbo.rs"]
mod turbo;
pub use macros::KeyboardMappingMacroObservation;
#[path = "keyboard_mapping_profiles.rs"]
mod profiles;
pub use profiles::KeyboardMappingProfilesObservation;
#[path = "keyboard_mapping_text_launch.rs"]
mod text_launch;
#[path = "keyboard_mapping_two_tap.rs"]
mod two_tap;

impl KeyboardProductWorkspace {
    pub(in super::super) fn interrupt_recording_targets(&self) -> &'static [Value] {
        &source(self.spec.product_id).key_targets
    }

    pub(in super::super) fn interrupt_recording_key_name(&self, input: &str) -> String {
        self.recording_key_name(input)
    }

    pub(in super::super) fn analog_default_points(&self) -> Value {
        two_tap::default_points(self)
    }
}

#[derive(Deserialize)]
struct Source {
    product_id: u32,
    default_buttons: Vec<Value>,
    layouts: Vec<Layout>,
    key_targets: Vec<Value>,
    options: BTreeMap<String, Options>,
    no_turbo: Vec<String>,
    keymap_choices: Vec<Value>,
    profile_choices: Vec<Value>,
    lighting_choices: Vec<Value>,
    macro_choices: Vec<Value>,
    scroll_macro_choices: Vec<Value>,
    sequence_macro_choices: Vec<Value>,
    phased_macro_choices: Vec<Value>,
    scroll_macro_inputs: Vec<String>,
    #[serde(default)]
    macro_legacy_without_phase: bool,
    key_groups: Vec<Value>,
    record_layouts: Vec<u32>,
    record_excluded_a: Vec<String>,
    record_excluded_b: Vec<String>,
}
#[derive(Deserialize)]
struct Options {
    group: String,
    field: String,
    choices: Vec<Value>,
}
#[derive(Deserialize)]
struct Layout {
    layout_id: u32,
    buttons: Vec<Value>,
}
fn source(pid: u32) -> &'static Source {
    static DATA: OnceLock<Vec<Source>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_all_mapping_catalog())
        .iter()
        .find(|row| row.product_id == pid)
        .expect("audited current keyboard caller")
}
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Edit {
    button: Value,
    shift: bool,
    from_panel: bool,
    slots: [Slot; 2],
    secondary_enabled: bool,
    original_secondary_enabled: bool,
}
#[derive(Clone, Deserialize, Serialize)]
struct Slot {
    original: Value,
    staged: Value,
    category: String,
    key_group: u32,
    #[serde(default)]
    modifier_included: bool,
    #[serde(default)]
    joystick: joystick::State,
    #[serde(default)]
    dial: dial::Draft,
    #[serde(default)]
    audio_brightness: audio_brightness::Draft,
    #[serde(default)]
    dynamic: dynamic::Draft,
}
impl Slot {
    fn new(staged: Value) -> Self {
        Self {
            category: category(&staged).into(),
            key_group: staged
                .pointer("/keyboardGroup/keyGroup")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            modifier_included: staged
                .pointer("/keyboardGroup/modifiers")
                .and_then(Value::as_array)
                .is_some_and(|values| !values.is_empty()),
            original: staged.clone(),
            staged,
            joystick: Default::default(),
            dial: Default::default(),
            audio_brightness: Default::default(),
            dynamic: Default::default(),
        }
    }
    fn dirty(&self) -> bool {
        if self.category == "DYNAMIC_KEY_STROKE" {
            return self.dynamic.changed;
        }
        self.original != self.staged
            || self.joystick.changed
            || self.dial.changed
            || self.audio_brightness.changed
    }
}
impl Edit {
    fn dirty(&self) -> bool {
        self.slots[0].dirty()
            || self.secondary_enabled != self.original_secondary_enabled
            || self.secondary_enabled && self.slots[1].dirty()
    }
    fn mark_clean(&mut self) {
        for slot in &mut self.slots {
            slot.original = slot.staged.clone();
            slot.joystick.changed = false;
            slot.dial.changed = false;
            slot.audio_brightness.changed = false;
            slot.dynamic.changed = false;
        }
        self.original_secondary_enabled = self.secondary_enabled;
    }
}
#[derive(Clone)]
pub(crate) enum Next {
    Key(String, bool),
    Drawer,
    Hyper,
    Close,
    Page(String),
    HistoryPage(String, usize),
}
#[derive(Default)]
pub(super) struct State {
    profiles: profiles::State,
    macros: macros::State,
    keyboard: keyboard::State,
    ai: ai::State,
    app_specific: app_specific::State,
    inter_device: inter_device::State,
    options: options::State,
    turbo: turbo::State,
    targets: targets::State,
    controller: [controller::State; 2],
    two_tap: two_tap::State,
    text_launch: text_launch::State,
    joystick_activation: Option<Subscription>,
    edit: Option<Edit>,
    pending_save: Option<Next>,
    can_save_after_prompt: bool,
    hypershift_confirmation: bool,
    sensitivity_warning: sensitivity_warning::State,
    dial: dial::State,
    audio_brightness: audio_brightness::State,
    dynamic: dynamic::State,
    filter_customized: bool,
    filter_menu: bool,
    category_menu: [bool; 2],
    scroll: ScrollHandle,
    page_scroll: ScrollHandle,
    viewport_bounds: Rc<Cell<Bounds<Pixels>>>,
    app_frame_bounds: Rc<Cell<Bounds<Pixels>>>,
    button_regions: Rc<std::cell::RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    keypad_mode_hover: Option<u64>,
    keypad_mode_bounds: [Rc<Cell<Bounds<Pixels>>>; 3],
    keypad_thumbstick: Option<(f64, f64)>,
    keypad_artwork_mounted: bool,
    keypad_animation_registered: bool,
    keypad_flicker: [Option<std::time::Instant>; 2],
    keypad_flicker_last_timer: Option<Rc<Cell<bool>>>,
    keypad_help_shown: bool,
    keypad_help_bounds: Rc<Cell<Bounds<Pixels>>>,
    keypad_widget_bounds: Rc<Cell<Bounds<Pixels>>>,
    keypad_product_image: keypad_artwork::ProductImageRuntime,
    popup_region: Option<String>,
    header: header::State,
    panel_index: panel_index::State,
    panel_value: panel_value::State,
    panel_secondary: panel_secondary::State,
    panel_dropdown: panel_dropdown::State,
}
fn text(value: &Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_else(|| {
        if value.is_null() {
            String::new()
        } else {
            value.to_string()
        }
    })
}
fn panel_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
fn panel_row_visible(button: &Value, hypershift: bool) -> bool {
    !panel_truthy(&button["disabled"])
        && button.get("isSidePanelList").is_none_or(panel_truthy)
        && button.get("isKeyToggle").is_none_or(|toggle| {
            (!panel_truthy(toggle) && !hypershift) || (panel_truthy(toggle) && hypershift)
        })
        && text(&button["counter"]) != "mediaVolume"
}
fn category(mapping: &Value) -> &'static str {
    match mapping["outputType"].as_str() {
        Some("keyboardGroup") if mapping["outputTypeSubset"] == "appSpecific" => "APP_SPECIFIC",
        Some("keyboardGroup") => "KEYBOARD_FUNCTION",
        Some("disableGroup") => "DISABLE",
        Some("defaultGroup") | None => "DEFAULT",
        Some("controllerGroup") => "CONTROLLER",
        Some("joystickGroup") => "JOYSTICK",
        Some("keymapGroup") => "SWITCH_KEYMAP",
        Some("mouseGroup") => "MOUSE_FUNCTION",
        Some("macroGroup") => "MACRO",
        Some("multimediaGroup") => "MULTIMEDIA",
        Some("win8ShortcutsGroup") => "WINDOWS_SHORTCUT",
        Some("backlightGroup") => "DEVICE_BRIGHTNESS",
        Some("hyperShiftGroup") => "RAZER_HYPERSHIFT",
        Some("profileNavigationGroup") => "SWITCH_PROFILE",
        Some("lightPacGroup") => "SWITCH_LIGHTING",
        Some("launchGroup") => "LAUNCH_PROGRAM",
        Some("textBlockGroup") => "TEXT_FUNCTION",
        Some("interDeviceGroup") => "INTERDEVICE",
        Some("aiLauncherGroup") => "AI_LAUNCHER",
        Some("audioGroup") => "AUDIO_FUNCTION",
        Some("brightnessGlobalGroup") => "GLOBAL_BRIGHTNESS",
        Some("dynamicKeyStrokeGroup") => "DYNAMIC_KEY_STROKE",
        _ => "DEFAULT",
    }
}

impl KeyboardProductWorkspace {
    pub(in crate::features) fn mapping_viewport_bounds(&self) -> Rc<Cell<Bounds<Pixels>>> {
        self.analog_gamepad.mapping.viewport_bounds.clone()
    }
    pub(in crate::features) fn bind_mapping_page_scroll(&mut self, scroll: ScrollHandle) {
        self.analog_gamepad.mapping.page_scroll = scroll;
    }
    pub(in crate::features) fn mapping_app_frame_bounds(&self) -> Option<Rc<Cell<Bounds<Pixels>>>> {
        keypad_panel::supported(self.spec.product_id)
            .then(|| self.analog_gamepad.mapping.app_frame_bounds.clone())
    }
    fn mapping_selection_matches(&self, id: &str) -> bool {
        self.analog_gamepad
            .mapping
            .edit
            .as_ref()
            .is_some_and(|edit| {
                edit.button["buttonKey"].as_str() == Some(id)
                    || edit.button["inputID"].as_str() == Some(id)
            })
    }
    /// Return true while the source mapping confirmation owns navigation.
    pub fn defer_mapping_page(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        if !self.mapping_before_page(key, cx) {
            return true;
        }
        self.update_keymap_bar_page(key);
        false
    }
    /// History keeps the source root's original index and keymap visibility.
    pub fn defer_mapping_history_page(
        &mut self,
        key: &str,
        history_index: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        static DATA: OnceLock<Vec<Navigation>> = OnceLock::new();
        let confirmed = DATA
            .get_or_init(|| read_keyboard_mapping_navigation_catalog())
            .iter()
            .any(|row| row.product_id == self.spec.product_id && row.confirm_mapping_history);
        if confirmed
            && self
                .analog_gamepad
                .mapping
                .edit
                .as_ref()
                .is_some_and(Edit::dirty)
        {
            self.analog_gamepad.mapping.pending_save =
                Some(Next::HistoryPage(key.into(), history_index));
            cx.notify();
            return true;
        }
        false
    }
    pub(in super::super) fn mapping_before_page(
        &mut self,
        key: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.page == key {
            return true;
        }
        static DATA: OnceLock<Vec<Navigation>> = OnceLock::new();
        let confirmed = DATA
            .get_or_init(|| read_keyboard_mapping_navigation_catalog())
            .iter()
            .any(|row| row.product_id == self.spec.product_id && row.confirm_mapping_navigation);
        if confirmed
            && self
                .analog_gamepad
                .mapping
                .edit
                .as_ref()
                .is_some_and(Edit::dirty)
        {
            self.analog_gamepad.mapping.pending_save = Some(Next::Page(key.into()));
            cx.notify();
            return false;
        }
        true
    }
    pub(in super::super) fn mapping_buttons(&self) -> &[Value] {
        if let Some(buttons) = self.source_artwork.observation().and_then(|observation| {
            super::super::source_artwork::buttons(observation, self.page == "TAB_CUSTOMIZE")
        }) {
            return buttons;
        }
        let source = source(self.spec.product_id);
        self.analog_gamepad
            .layout
            .and_then(|id| source.layouts.iter().find(|layout| layout.layout_id == id))
            .map(|layout| layout.buttons.as_slice())
            .unwrap_or(&source.default_buttons)
    }
    fn current_mapping(&self, input: &str, shift: bool) -> Option<&Value> {
        self.draft
            .pointer(&self.mapping_path())
            .and_then(Value::as_array)?
            .iter()
            .find(|mapping| {
                mapping["inputID"].as_str() == Some(input)
                    && (!shift || mapping["isHyperShift"].as_bool().unwrap_or(false))
            })
    }
    pub(in super::super) fn analog_mapping_next(
        &mut self,
        next: Next,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.customize_controls_disabled() {
            return;
        }
        if matches!(&next, Next::Key(id, _) if self.mapping_selection_matches(id)) {
            return;
        }
        if self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .is_some_and(Edit::dirty)
        {
            self.analog_gamepad.mapping.pending_save = Some(next);
            cx.notify();
            return;
        }
        self.analog_mapping_continue(next, window, cx);
    }
    fn analog_mapping_continue(&mut self, next: Next, window: &mut Window, cx: &mut Context<Self>) {
        match next {
            Next::Key(id, from_panel) => {
                let Some(button) = self
                    .mapping_buttons()
                    .iter()
                    .find(|button| {
                        button["inputID"].as_str() == Some(&id)
                            || button["buttonKey"].as_str() == Some(&id)
                    })
                    .cloned()
                else {
                    return;
                };
                let button = self.mapping_active_button(&button);
                if !button["isEnabled"].as_bool().unwrap_or(true)
                    || button["disabled"].as_bool().unwrap_or(false)
                    || (self.hypershift
                        && button["disableHypershiftMapping"]
                            .as_bool()
                            .unwrap_or(false))
                {
                    return;
                }
                if self.mapping_selection_matches(&id) {
                    return;
                }
                let entry = self
                    .current_mapping(button["inputID"].as_str().unwrap_or(&id), self.hypershift);
                let staged = entry
                    .and_then(|entry| {
                        if entry["mapping"].is_array() {
                            entry.pointer("/mapping/0")
                        } else {
                            Some(entry)
                        }
                    })
                    .cloned()
                    .unwrap_or_else(|| json!({"outputType":"defaultGroup","isDefault":true}));
                let secondary = entry.and_then(|entry| entry.pointer("/mapping/1")).cloned();
                let secondary_enabled = secondary.as_ref().is_some_and(|slot| {
                    slot["outputType"].is_string() && slot["outputType"] != "defaultGroup"
                });
                let analog = button["inputType"] == "AnalogInput";
                let defaults = if analog {
                    two_tap::default_points(self)
                } else {
                    Value::Null
                };
                let mut primary = staged;
                if analog && primary["actuationPoint"].is_null() {
                    primary["actuationPoint"] = defaults.clone();
                }
                let secondary = secondary.unwrap_or_else(|| two_tap::disabled_secondary(defaults));
                self.analog_gamepad.mapping.popup_region = (!from_panel)
                    .then(|| {
                        self.analog_gamepad
                            .floating
                            .key_bounds
                            .borrow()
                            .iter()
                            .filter(|(_, (key, _))| key.id == id)
                            .min_by_key(|(_, (_, bounds))| {
                                if bounds.contains(&window.mouse_position()) {
                                    0
                                } else {
                                    1
                                }
                            })
                            .map(|(region, _)| region.clone())
                    })
                    .flatten();
                self.selected_key = Some(id);
                self.analog_gamepad.mapping.header.hovered = false;
                self.analog_gamepad
                    .mapping
                    .header
                    .tip_bounds
                    .set(Bounds::default());
                self.analog_gamepad.mapping.edit = Some(Edit {
                    button,
                    shift: self.hypershift,
                    from_panel,
                    slots: [Slot::new(primary), Slot::new(secondary)],
                    secondary_enabled,
                    original_secondary_enabled: secondary_enabled,
                });
                self.analog_gamepad.mapping.category_menu = [false; 2];
                self.analog_gamepad.mapping.controller = Default::default();
                self.init_two_tap_sliders(cx);
                self.init_mapping_text_launch(0, window, cx);
                self.init_mapping_text_launch(1, window, cx);
                self.init_mapping_macro(0, window, cx);
                self.init_mapping_macro(1, window, cx);
                self.init_mapping_keyboard(0, window, cx);
                self.init_mapping_keyboard(1, window, cx);
                self.init_mapping_ai(0, window, cx);
                self.init_mapping_ai(1, window, cx);
                self.init_mapping_app_specific(0, cx);
                self.init_mapping_app_specific(1, cx);
                self.init_mapping_inter_device(0, window, cx);
                self.init_mapping_inter_device(1, window, cx);
                self.init_mapping_options(0, window, cx);
                self.init_mapping_options(1, window, cx);
                self.init_mapping_turbo(0, window, cx);
                self.init_mapping_turbo(1, window, cx);
                self.init_mapping_targets(0, window, cx);
                self.init_mapping_targets(1, window, cx);
                self.init_mapping_dial(0, true, window, cx);
                self.init_mapping_dial(1, true, window, cx);
                self.init_mapping_audio_brightness(0, true, window, cx);
                self.init_mapping_audio_brightness(1, true, window, cx);
                self.init_mapping_dynamic(0, window, cx);
                self.init_mapping_dynamic(1, window, cx);
            }
            Next::Drawer => {
                self.button_drawer_open = !self.button_drawer_open;
                if !self.button_drawer_open
                    && self
                        .analog_gamepad
                        .mapping
                        .edit
                        .as_ref()
                        .is_some_and(|edit| edit.from_panel)
                {
                    self.clear_analog_mapping();
                }
            }
            Next::Hyper => {
                self.hypershift = !self.hypershift;
                self.clear_analog_mapping();
                self.hovered_key = None;
            }
            Next::Close => self.clear_analog_mapping(),
            Next::Page(page_key) => {
                self.clear_analog_mapping();
                cx.emit(KeyboardNavigationRequested::new(page_key));
            }
            Next::HistoryPage(page_key, history_index) => {
                self.clear_analog_mapping();
                cx.emit(
                    KeyboardNavigationRequested::new(page_key).with_history_index(history_index),
                );
            }
        }
        cx.notify();
    }
    pub(in super::super) fn clear_analog_mapping(&mut self) {
        self.selected_key = None;
        self.analog_gamepad.mapping.edit = None;
        self.analog_gamepad.mapping.pending_save = None;
        self.analog_gamepad.mapping.can_save_after_prompt = false;
        self.analog_gamepad.mapping.hypershift_confirmation = false;
        self.analog_gamepad.mapping.sensitivity_warning.dismiss();
        self.analog_gamepad.mapping.dial.dismiss();
        self.analog_gamepad.mapping.audio_brightness.dismiss();
        self.analog_gamepad.mapping.dynamic.dismiss();
        self.analog_gamepad.mapping.category_menu = [false; 2];
        self.analog_gamepad.mapping.controller = Default::default();
        self.analog_gamepad.mapping.two_tap = two_tap::State::default();
        self.analog_gamepad.mapping.joystick_activation = None;
        self.analog_gamepad.mapping.text_launch.clear();
        self.analog_gamepad.mapping.macros.dismiss();
        self.analog_gamepad.mapping.keyboard.dismiss();
        self.analog_gamepad.mapping.ai.dismiss();
        self.analog_gamepad.mapping.app_specific.dismiss();
        self.analog_gamepad.mapping.inter_device.dismiss();
        self.analog_gamepad.mapping.options.dismiss();
        self.analog_gamepad.mapping.turbo.dismiss();
        self.analog_gamepad.mapping.targets.dismiss();
    }
    pub(in super::super) fn analog_mapping_snapshot(&self, snapshot: &mut Value) {
        // Deliberately local: neither a device mapping nor a successful vendor save.
        if let Some(edit) = &self.analog_gamepad.mapping.edit {
            snapshot["_localAnalogMappingDraft"] =
                serde_json::to_value(edit).expect("mapping draft serialization");
        } else if let Some(object) = snapshot.as_object_mut() {
            object.remove("_localAnalogMappingDraft");
        }
    }
    pub(in super::super) fn restore_analog_mapping(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_analog_mapping();
        let edit = self
            .draft
            .as_object_mut()
            .and_then(|draft| draft.remove("_localAnalogMappingDraft"))
            .and_then(|value| serde_json::from_value::<Edit>(value).ok());
        if let Some(edit) = edit {
            let valid = self.mapping_buttons().iter().any(|button| {
                button["buttonKey"] == edit.button["buttonKey"]
                    && button["inputID"] == edit.button["inputID"]
            });
            if valid {
                self.selected_key = edit.button["buttonKey"].as_str().map(str::to_owned);
                self.hypershift = edit.shift;
                self.analog_gamepad.mapping.edit = Some(edit);
                self.init_two_tap_sliders(cx);
                self.init_mapping_text_launch(0, window, cx);
                self.init_mapping_text_launch(1, window, cx);
                self.init_mapping_macro(0, window, cx);
                self.init_mapping_macro(1, window, cx);
                self.init_mapping_keyboard(0, window, cx);
                self.init_mapping_keyboard(1, window, cx);
                self.init_mapping_ai(0, window, cx);
                self.init_mapping_ai(1, window, cx);
                self.init_mapping_app_specific(0, cx);
                self.init_mapping_app_specific(1, cx);
                self.init_mapping_inter_device(0, window, cx);
                self.init_mapping_inter_device(1, window, cx);
                self.init_mapping_options(0, window, cx);
                self.init_mapping_options(1, window, cx);
                self.init_mapping_turbo(0, window, cx);
                self.init_mapping_turbo(1, window, cx);
                self.init_mapping_targets(0, window, cx);
                self.init_mapping_targets(1, window, cx);
                self.init_mapping_dial(0, true, window, cx);
                self.init_mapping_dial(1, true, window, cx);
                self.init_mapping_audio_brightness(0, true, window, cx);
                self.init_mapping_audio_brightness(1, true, window, cx);
                self.init_mapping_dynamic(0, window, cx);
                self.init_mapping_dynamic(1, window, cx);
            }
        }
    }
    fn stage_analog_category(
        &mut self,
        name: &str,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let option = source(self.spec.product_id).options.get(name);
        let Some(edit) = &mut self.analog_gamepad.mapping.edit else {
            return;
        };
        let metadata = edit.slots[slot].staged.clone();
        edit.slots[slot].category = name.into();
        edit.slots[slot].joystick = Default::default();
        edit.slots[slot].dial = Default::default();
        edit.slots[slot].audio_brightness = Default::default();
        edit.slots[slot].dynamic = Default::default();
        edit.slots[slot].staged = match name {
            "DISABLE" => json!({"outputType":"disableGroup"}),
            "TEXT_FUNCTION" => json!({"outputType":"textBlockGroup","textBlockGroup":{"text":""}}),
            "LAUNCH_PROGRAM" => json!({"outputType":"launchGroup","launchGroup":{}}),
            "KEYBOARD_FUNCTION" => {
                json!({"outputType":"keyboardGroup","keyboardGroup":{"keyGroup":0,"key":"","modifiers":[]}})
            }
            "CONTROLLER" => controller::payload(self.spec.product_id, "", "STANDARD", None),
            "JOYSTICK" => joystick::payload(self.spec.product_id, false, 0),
            "DEFAULT" => json!({"outputType":"defaultGroup","isDefault":true}),
            "RAZER_HYPERSHIFT" => json!({"outputType":"hyperShiftGroup"}),
            "MACRO" => json!({"outputType":"macroGroup","macroGroup":{}}),
            "AI_LAUNCHER" => {
                json!({"outputType":"aiLauncherGroup","aiLauncherGroup":{"assignment":"OPEN_PROMPT_COMPOSER","param":"chat","isUseVoiceInput":false}})
            }
            "APP_SPECIFIC" => {
                json!({"outputType":"keyboardGroup","outputTypeSubset":"appSpecific","keyboardGroup":{"key":"","modifiers":[]}})
            }
            "INTERDEVICE" => json!({"outputType":"interDeviceGroup","interDeviceGroup":{}}),
            "DIAL_MULTI_FUNCTION" => edit.slots[slot].original.clone(),
            "AUDIO_FUNCTION" => json!({"outputType":"audioGroup","audioGroup":{}}),
            "GLOBAL_BRIGHTNESS" => {
                json!({"outputType":"brightnessGlobalGroup","brightnessGlobalGroup":{}})
            }
            "DYNAMIC_KEY_STROKE" => {
                json!({"outputType":"dynamicKeyStrokeGroup","dynamicKeyStrokeGroup":{}})
            }
            "SWITCH_LIGHTING" => json!({"outputType":"lightPacGroup","lightPacGroup":{}}),
            "SWITCH_PROFILE" => {
                json!({"outputType":"profileNavigationGroup","profileNavigationGroup":{"profileNavigationAssignment":"NextProfile"}})
            }
            "SWITCH_KEYMAP" => {
                json!({"outputType":"keymapGroup","keymapGroup":{"type":"specificKeymap","isClutch":false}})
            }
            _ => edit.slots[slot].staged.clone(),
        };
        if let Some(option) = option {
            if let Some(choice) = option.choices.first() {
                edit.slots[slot].staged = json!({"outputType":option.group});
                edit.slots[slot].staged[&option.group] = json!({});
                edit.slots[slot].staged[&option.group][&option.field] = choice["id"].clone();
                if name == "MULTIMEDIA"
                    && matches!(
                        choice["content"].as_str(),
                        Some("VOLUME_UP" | "VOLUME_DOWN")
                    )
                {
                    edit.slots[slot].staged[&option.group]["useTurbo"] = json!(true);
                }
            }
        }
        for field in ["actuationPoint", "rapidTrigger", "isEnableCustomRelease"] {
            if let Some(value) = metadata.get(field) {
                edit.slots[slot].staged[field] = value.clone();
            }
        }
        if slot == 0 && !two_tap::secondary_available(self) {
            if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
                edit.secondary_enabled = false;
            }
        }
        self.analog_gamepad.mapping.category_menu[slot] = false;
        self.analog_gamepad.mapping.can_save_after_prompt = false;
        self.init_mapping_text_launch(slot, window, cx);
        if name == "SWITCH_KEYMAP" {
            self.stage_specific_mapping_keymap(slot, None, cx);
        }
        if name == "SWITCH_LIGHTING" {
            self.init_mapping_lighting(slot, cx);
        }
        self.init_mapping_macro(slot, window, cx);
        self.init_mapping_keyboard(slot, window, cx);
        self.init_mapping_ai(slot, window, cx);
        self.init_mapping_app_specific(slot, cx);
        self.init_mapping_inter_device(slot, window, cx);
        self.init_mapping_options(slot, window, cx);
        self.init_mapping_turbo(slot, window, cx);
        self.init_mapping_targets(slot, window, cx);
        self.init_mapping_dial(slot, true, window, cx);
        self.init_mapping_audio_brightness(slot, true, window, cx);
        self.init_mapping_dynamic(slot, window, cx);
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn save_analog_mapping(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(edit) = self.analog_gamepad.mapping.edit.clone() else {
            return false;
        };
        if !two_tap::valid(self, &edit) {
            return false;
        }
        let path = self.mapping_path();
        let Some(mappings) = self.draft.pointer(&path).and_then(Value::as_array) else {
            return false;
        };
        let mut edit = edit;
        text_launch::prepare_save(&mut edit);
        inter_device::prepare_save(self, &mut edit);
        for slot in &mut edit.slots {
            if let Some(payload) = dynamic::staged(slot) {
                two_tap::replace_slot(slot, payload);
            }
            if let Some(payload) = audio_brightness::staged(self.spec.product_id, slot) {
                two_tap::replace_slot(slot, payload);
            }
        }
        let merged = if edit.slots[0].category == "DIAL_MULTI_FUNCTION" {
            dial::merge(self, &edit, mappings)
        } else {
            two_tap::merge(self, &edit, mappings)
        };
        if let Some(target) = self.draft.pointer_mut(&path) {
            *target = json!(merged);
        }
        self.clear_analog_mapping();
        cx.emit(KeyboardProductChanged);
        self.request_actuation_mapping(cx);
        self.show_mapping_login_warning();
        cx.notify();
        true
    }

    pub(in super::super) fn mapping_customize_layer(
        &self,
        page: AnyElement,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        div()
            .relative()
            .w_full()
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                this.source_special_pointer(event.position, cx);
            }))
            .child(
                h_flex()
                    .items_start()
                    .children(
                        (self.button_drawer_open && !keypad_panel::supported(self.spec.product_id))
                            .then(|| deferred(self.analog_mapping_drawer(cx)).with_priority(150)),
                    )
                    // Current Keypad body left/width use the literal 230px;
                    // the independent fixed drawer is actually 430px wide.
                    .children(
                        (self.button_drawer_open && keypad_panel::supported(self.spec.product_id))
                            .then(|| div().w(surface::css(230.)).flex_shrink_0()),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            div()
                                .when(
                                    self.button_drawer_open
                                        && keypad_panel::supported(self.spec.product_id),
                                    |widgets| widgets.min_w(surface::css(1000.)),
                                )
                                .child(page),
                        ),
                    ),
            )
            .children(
                (self.button_drawer_open && keypad_panel::supported(self.spec.product_id))
                    .then(|| deferred(self.keypad_mapping_drawer(cx)).with_priority(10)),
            )
            .children((!keypad_panel::supported(self.spec.product_id)).then(|| {
                deferred(panel_value::Tip {
                    data: self.analog_gamepad.mapping.panel_value.data.clone(),
                    shown: self.analog_gamepad.mapping.panel_value.shown,
                    position: self.analog_gamepad.mapping.panel_value.position.clone(),
                    viewport: self.analog_gamepad.mapping.viewport_bounds.clone(),
                })
                .with_priority(999)
            }))
            .children(self.analog_mapping_popup(window, cx).map(|popup| {
                if let Some(edit) = self.analog_gamepad.mapping.edit.as_ref() {
                    return deferred(position::PopupPosition::new(self, edit, popup))
                        .with_priority(103)
                        .into_any_element();
                }
                div()
                    .absolute()
                    .top(surface::css(0.))
                    .left(surface::css(if self.button_drawer_open {
                        keypad_panel::width(self.spec.product_id)
                    } else {
                        20.
                    }))
                    .child(popup)
                    .into_any_element()
            }))
            .children(self.analog_mapping_confirmation(cx))
            .children(self.mapping_login_warning_element(cx))
            .into_any_element()
    }

    pub(in super::super) fn analog_mapping_drawer(&self, cx: &Context<Self>) -> AnyElement {
        if keypad_panel::supported(self.spec.product_id) {
            return self.keypad_mapping_drawer(cx);
        }
        let state = &self.analog_gamepad.mapping;
        let mut rows = self
            .mapping_buttons()
            .iter()
            .map(|button| self.mapping_active_button(button))
            .filter(|button| {
                (!self.hypershift
                    || !panel_truthy(&button["HID"])
                    || !panel_truthy(&button["disableHypershiftMapping"]))
                    && (!state.filter_customized || panel_truthy(&button["isMapped"]))
            })
            .collect::<Vec<_>>();
        if rows.iter().all(|button| {
            button["counter"]
                .as_f64()
                .is_some_and(|value| value.fract() == 0.)
        }) {
            rows.sort_by(|a, b| {
                a["counter"]
                    .as_f64()
                    .unwrap()
                    .partial_cmp(&b["counter"].as_f64().unwrap())
                    .expect("finite source counters")
            });
        }
        v_flex()
            .w(surface::css(230.))
            .h_full()
            .bg(rgb(0x222222))
            .flex_shrink_0()
            .child(
                h_flex()
                    .h(surface::css(67.))
                    .border_b_1()
                    .border_color(rgb(0x5d5d5d))
                    .items_center()
                    .justify_center()
                    .child(panel_dropdown::control(self, cx))
                    .child(
                        BaseButton::new("analog-drawer-close")
                            .size(surface::css(26.))
                            .p_0()
                            .border_1()
                            .border_color(rgba(0x00000000))
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|close| close.border_color(rgb(0x5d5d5d)))
                            .active(|close| close.border_color(rgb(0x44d62c)))
                            .child(panel_dropdown::close_icon(self.spec.product_id))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.analog_mapping_next(Next::Drawer, window, cx)
                            })),
                    ),
            )
            .child(
                v_flex()
                    .id("analog-drawer-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&state.scroll)
                    .children(
                        rows.into_iter()
                            .enumerate()
                            .filter(|(_, button)| panel_row_visible(button, self.hypershift))
                            .map(|(index, button)| {
                                let key = text(&button["buttonKey"]);
                                let mapped = panel_truthy(&button["isMapped"]);
                                let disabled = !button["isEnabled"].as_bool().unwrap_or(true);
                                let label = t(&text(
                                    &button
                                        [if mapped { "assignmentValue" } else { "defaultValue" }],
                                ));
                                let row_bounds = Rc::new(Cell::new(Bounds::default()));
                                let measured_row = row_bounds.clone();
                                BaseButton::new(SharedString::from(format!(
                                    "analog-drawer-row-{key}"
                                )))
                                .min_h(surface::css(50.))
                                .p(surface::css(9.))
                                .w_full()
                                .flex()
                                .items_center()
                                .disabled(disabled)
                                .bg(if self.selected_key.as_ref() == Some(&key) {
                                    rgb(0x111111)
                                } else {
                                    rgb(0x222222)
                                })
                                .hover(|row| row.bg(rgb(0x383838)))
                                .active(|row| row.bg(rgb(0x111111)))
                                .on_prepaint(move |rect, _, _| measured_row.set(rect))
                                .child(
                                    div()
                                        .w(surface::css(56.))
                                        .ml(surface::css(-10.))
                                        .text_size(surface::css(14.))
                                        .line_height(surface::css(14.))
                                        .text_center()
                                        .text_color(rgb(if disabled { 0x707070 } else { 0xcccccc }))
                                        .child(panel_index::Index::new(
                                            self,
                                            index,
                                            text(&button["counter"]),
                                            disabled,
                                            cx,
                                        )),
                                )
                                .child(
                                    v_flex()
                                        .w(surface::css(164.))
                                        .border_l_1()
                                        .border_color(rgb(0x707070))
                                        .pl(surface::css(5.))
                                        .line_height(surface::css(16.))
                                        .text_left()
                                        .text_color(if mapped {
                                            if self.hypershift {
                                                rgb(0xfd8611)
                                            } else {
                                                rgb(0x44d62c)
                                            }
                                        } else {
                                            rgb(if disabled { 0x707070 } else { 0xcccccc })
                                        })
                                        .child(
                                            div()
                                                .text_size(surface::css(10.))
                                                .text_color(rgb(0x707070))
                                                .child(
                                                    t(button["assignment"]
                                                        .as_str()
                                                        .unwrap_or("DEFAULT"))
                                                    .to_uppercase(),
                                                ),
                                        )
                                        .child(panel_value::Binding {
                                            index,
                                            label,
                                            raw: text(&button["assignmentValue"]),
                                            mapped,
                                            row: row_bounds,
                                            owner: cx.entity().downgrade(),
                                        }),
                                )
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.analog_mapping_next(
                                            Next::Key(key.clone(), true),
                                            window,
                                            cx,
                                        )
                                    },
                                ))
                            }),
                    ),
            )
            .into_any_element()
    }

    pub(in super::super) fn analog_mapping_popup(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let edit = self.analog_gamepad.mapping.edit.as_ref()?;
        let expanded = edit.secondary_enabled && edit.button["inputType"] == "AnalogInput";
        let dynamic = edit.slots[0].category == "DYNAMIC_KEY_STROKE";
        let width = if dynamic {
            600.
        } else if expanded {
            if two_tap::extended(self.spec.product_id) {
                582.
            } else {
                543.
            }
        } else {
            292.
        };
        let ordinary = edit.button["inputType"] != "AnalogInput";
        let popup = v_flex()
            .w(surface::css(width))
            .h_full()
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .rounded_t(surface::css(5.))
            .rounded_bl(surface::css(5.))
            .rounded_br(surface::css(if expanded { 0. } else { 5. }))
            .overflow_hidden()
            .shadow(vec![BoxShadow {
                color: rgba(0x000000b3).into(),
                offset: point(px(0.), px(0.)),
                blur_radius: surface::css(20.).to_pixels(window.rem_size()),
                spread_radius: px(0.),
                inset: false,
            }])
            .child(
                h_flex()
                    .id("keyboard-mapping-header")
                    .relative()
                    .h(surface::css(36.))
                    .px(surface::css(10.))
                    .items_center()
                    .flex_shrink_0()
                    .bg(rgb(0x222222))
                    .rounded_t(surface::css(5.))
                    .text_color(rgb(0x999999))
                    .on_prepaint({
                        let bounds = self.analog_gamepad.mapping.header.bounds.clone();
                        move |rect, _, _| bounds.set(rect)
                    })
                    .on_hover(cx.listener(|this, hover, window, cx| {
                        let header = &mut this.analog_gamepad.mapping.header;
                        let hovered = *hover
                            || (header.hovered
                                && header.tip_bounds.get().contains(&window.mouse_position()));
                        if header.hovered != hovered {
                            header.hovered = hovered;
                            cx.notify();
                        }
                    }))
                    .when(ordinary, |head| {
                        head.child(header::Title {
                            label: text(&edit.button["counter"]),
                            owner: cx.entity().downgrade(),
                            bounds: self.analog_gamepad.mapping.header.bounds.clone(),
                            tip_bounds: self.analog_gamepad.mapping.header.tip_bounds.clone(),
                            hovered: self.analog_gamepad.mapping.header.hovered,
                        })
                    })
                    .when(!ordinary, |head| {
                        head.child(
                            div()
                                .flex_1()
                                .text_center()
                                .child(text(&edit.button["defaultValue"])),
                        )
                    })
                    .child(
                        BaseButton::new("analog-mapping-close")
                            .accessibility_label(t("CLOSE"))
                            .absolute()
                            .right(surface::css(0.))
                            .top(surface::css(0.))
                            .size(surface::css(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|close| close.bg(rgba(0xffffff1a)))
                            .active(|close| close.bg(rgba(0x0000001a)))
                            .child(
                                img("synapse/keyboard-mapping-recording/close.svg")
                                    .size(surface::css(20.)),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.analog_mapping_next(Next::Close, window, cx)
                            })),
                    ),
            )
            .child(
                h_flex()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .items_start()
                    .child(self.analog_mapping_slot(0, cx))
                    .when(expanded, |row| row.child(self.analog_mapping_slot(1, cx)))
                    .when(ordinary, |row| {
                        row.child(deferred(self.mapping_function_blade(0, cx)).with_priority(105))
                    }),
            )
            .into_any_element();
        Some(
            blade::PopupSize {
                content: popup,
                width,
            }
            .into_any_element(),
        )
    }
    fn analog_mapping_slot(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let state = &self.analog_gamepad.mapping;
        let edit = state.edit.as_ref().expect("mounted mapping slot");
        let ordinary = edit.button["inputType"] != "AnalogInput";
        let has_blade = ordinary || slot == 0 || two_tap::extended(self.spec.product_id);
        let mut popup = v_flex()
            .id(SharedString::from(format!(
                "keyboard-mapping-function-body-{slot}"
            )))
            .w(surface::css(if has_blade { 292. } else { 251. }))
            .when(has_blade, |panel| {
                panel
                    .ml(surface::css(40.))
                    .w(surface::css(250.))
                    .h_full()
                    .min_h_0()
                    .overflow_y_scroll()
            })
            .when(slot == 1, |panel| {
                panel.border_l_1().border_color(rgb(0x5d5d5d))
            });
        if edit.slots[slot].category == "DYNAMIC_KEY_STROKE" {
            popup = popup.w(surface::css(560.));
        }
        if has_blade {
            popup = popup.child(
                h_flex()
                    .mx(surface::css(20.))
                    .my(surface::css(20.))
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .font_family("RazerF5")
                    .text_color(rgb(0x44d62c))
                    .child(t(&edit.slots[slot].category).to_uppercase())
                    .when(
                        edit.slots[slot].category == "DYNAMIC_KEY_STROKE",
                        |heading| {
                            heading
                                .gap(surface::css(10.))
                                .child(self.mapping_dynamic_heading_help(slot, cx))
                        },
                    ),
            );
        } else {
            popup = popup.child(
                div()
                    .mx(surface::css(20.))
                    .mt(surface::css(20.))
                    .child(t(&edit.slots[slot].category)),
            );
        }
        if slot == 1 {
            popup = popup.child(
                div()
                    .mx(surface::css(20.))
                    .child(t("SECONDARYFUNCTIONCAPS")),
            );
        }
        if !has_blade && state.category_menu[slot] {
            popup = popup.child(
                v_flex()
                    .id(SharedString::from(format!(
                        "analog-mapping-categories-{slot}"
                    )))
                    .max_h(surface::css(240.))
                    .overflow_y_scroll()
                    .children(
                        edit.button["functionList"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|value| {
                                let name = value.as_str()?.to_owned();
                                if !two_tap::category_visible(self, slot, &name) {
                                    return None;
                                }
                                Some(
                                    BaseButton::new(SharedString::from(format!(
                                        "analog-category-{slot}-{name}"
                                    )))
                                    .px(surface::css(20.))
                                    .py(surface::css(5.))
                                    .child(t(&name))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.stage_analog_category(&name, slot, window, cx)
                                    })),
                                )
                            }),
                    ),
            );
        }
        popup = popup.children(self.mapping_options_element(slot));
        if edit.slots[slot].category == "CONTROLLER" {
            popup = popup.child(controller::element(self, slot, cx));
        }
        if edit.slots[slot].category == "JOYSTICK" {
            popup = popup.child(joystick::element(self, slot, cx));
        }
        if edit.slots[slot].category == "DEFAULT" {
            popup = popup.child(
                div()
                    .mx(surface::css(20.))
                    .text_center()
                    .child(text(&edit.button["defaultValue"])),
            );
        }
        if edit.slots[slot].category == "RAZER_HYPERSHIFT" {
            popup = popup.child(
                v_flex()
                    .mx(surface::css(20.))
                    .mb(surface::css(20.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(rgb(0x707070))
                    .child(t("RAZER_HYPERSHIFT_MSG1"))
                    .child(div().h(surface::css(17.)))
                    .child(t("RAZER_HYPERSHIFT_MSG2")),
            );
        }
        if edit.slots[slot].category == "SWITCH_KEYMAP" {
            popup = popup.child(self.mapping_keymap_element(slot, cx));
        }
        if edit.slots[slot].category == "SWITCH_PROFILE" {
            popup = popup.child(self.mapping_profiles_element(slot, cx));
        }
        if edit.slots[slot].category == "SWITCH_LIGHTING" {
            popup = popup.child(self.mapping_lighting_element(slot, cx));
        }
        if edit.slots[slot].category == "MACRO" {
            popup = popup.child(self.mapping_macro_element(slot, cx));
        }
        if edit.slots[slot].category == "KEYBOARD_FUNCTION" {
            popup = popup.child(self.mapping_keyboard_element(slot, cx));
        }
        if edit.slots[slot].category == "AI_LAUNCHER" {
            popup = popup.child(self.mapping_ai_element(slot, cx));
        }
        if edit.slots[slot].category == "APP_SPECIFIC" {
            popup = popup.child(self.mapping_app_specific_element(slot, cx));
        }
        if edit.slots[slot].category == "INTERDEVICE" {
            popup = popup.child(self.mapping_inter_device_element(slot, cx));
        }
        if matches!(
            edit.slots[slot].category.as_str(),
            "AUDIO_FUNCTION" | "GLOBAL_BRIGHTNESS"
        ) {
            popup = popup.child(self.mapping_audio_brightness_element(slot, cx));
        }
        if edit.slots[slot].category == "DYNAMIC_KEY_STROKE" {
            popup = popup.child(self.mapping_dynamic_element(slot, cx));
        }
        if edit.slots[slot].category == "DIAL_MULTI_FUNCTION" {
            popup = popup.child(self.mapping_dial_element(slot, cx));
        }

        popup = popup.children(self.mapping_turbo_element(slot, cx));
        popup = popup.children(text_launch::element(self, slot, cx));
        popup = popup.child(self.mapping_prerequisite_element(slot, cx));
        if edit.slots[0].category == "SWITCH_PROFILE" && self.mapping_profile_rows().len() == 1 {
            popup = popup.child(
                div()
                    .mx(surface::css(20.))
                    .pb(surface::css(10.))
                    .child(t("SWITCH_PROFILE_FUNCTION_TIP")),
            );
        }
        popup = popup.children(two_tap::actuation_element(self, slot, cx));
        let chroma_installed = self
            .lighting_observation
            .as_ref()
            .is_some_and(|observation| observation.chroma_installed_module);
        let requires_missing_chroma =
            edit.slots[0].category == "SWITCH_LIGHTING" && !chroma_installed;
        let source_footer_disabled = edit.slots[slot].category == "SWITCH_PROFILE"
            && self.mapping_profile_rows().len() <= 1
            || edit.slots[slot].category == "SWITCH_LIGHTING"
                && self
                    .lighting_observation
                    .as_ref()
                    .is_none_or(|observation| observation.chroma_profiles.is_empty());
        if (!edit.secondary_enabled || slot == 1) && !requires_missing_chroma {
            popup = popup.child(
                h_flex()
                    .p(surface::css(20.))
                    .gap(surface::css(10.))
                    .justify_end()
                    .child(
                        Button::new(SharedString::from(format!("analog-mapping-cancel-{slot}")))
                            .label(t("CANCEL"))
                            .outline()
                            .disabled(source_footer_disabled)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clear_analog_mapping();
                                cx.emit(KeyboardProductChanged);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("analog-mapping-save-{slot}")))
                            .label(t("SAVE"))
                            .primary()
                            .disabled(
                                (!edit.dirty() && !state.can_save_after_prompt)
                                    || !two_tap::valid(self, edit),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.request_mapping_save(slot, cx);
                            })),
                    ),
            );
        }
        popup = popup.children(two_tap::secondary_element(self, slot, cx));
        if ordinary {
            popup.into_any_element()
        } else {
            h_flex()
                .relative()
                .w(surface::css(if has_blade { 292. } else { 251. }))
                .h_full()
                .min_h_0()
                .child(popup)
                .when(has_blade, |panel| {
                    panel.child(deferred(self.mapping_function_blade(slot, cx)).with_priority(105))
                })
                .into_any_element()
        }
    }
    pub(in super::super) fn analog_mapping_confirmation(
        &self,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(warning) = self.mapping_sensitivity_warning_element(cx) {
            return Some(warning);
        }
        if self.analog_gamepad.mapping.hypershift_confirmation {
            return Some(
                deferred(confirmation::Dialog {
                    content: self.mapping_hypershift_confirmation_card(cx),
                })
                .with_priority(106)
                .into_any_element(),
            );
        }
        self.analog_gamepad.mapping.pending_save.as_ref()?;
        Some(
            deferred(confirmation::Dialog {
                content: self.mapping_confirmation_card(cx),
            })
            .with_priority(106)
            .into_any_element(),
        )
    }
}

// Generated catalog reader: razer-pages/keyboard_all_mapping_data.
fn read_keyboard_all_mapping_catalog() -> Vec<Source> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Source>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Source> {
        let value = match r.tag()? {
            0 => Source {
                product_id: r.child(decode_2)?,
                default_buttons: r.child(decode_3)?,
                layouts: r.child(decode_5)?,
                key_targets: r.child(decode_3)?,
                options: r.child(decode_7)?,
                no_turbo: r.child(decode_10)?,
                keymap_choices: r.child(decode_3)?,
                profile_choices: r.child(decode_3)?,
                lighting_choices: r.child(decode_3)?,
                macro_choices: r.child(decode_3)?,
                scroll_macro_choices: r.child(decode_3)?,
                sequence_macro_choices: r.child(decode_3)?,
                phased_macro_choices: r.child(decode_3)?,
                scroll_macro_inputs: r.child(decode_10)?,
                macro_legacy_without_phase: Default::default(),
                key_groups: r.child(decode_3)?,
                record_layouts: r.child(decode_11)?,
                record_excluded_a: r.child(decode_10)?,
                record_excluded_b: r.child(decode_10)?,
            },
            1 => Source {
                product_id: r.child(decode_2)?,
                default_buttons: r.child(decode_3)?,
                layouts: r.child(decode_5)?,
                key_targets: r.child(decode_3)?,
                options: r.child(decode_7)?,
                no_turbo: r.child(decode_10)?,
                keymap_choices: r.child(decode_3)?,
                profile_choices: r.child(decode_3)?,
                lighting_choices: r.child(decode_3)?,
                macro_choices: r.child(decode_3)?,
                scroll_macro_choices: r.child(decode_3)?,
                sequence_macro_choices: r.child(decode_3)?,
                phased_macro_choices: r.child(decode_3)?,
                scroll_macro_inputs: r.child(decode_10)?,
                macro_legacy_without_phase: r.child(decode_12)?,
                key_groups: r.child(decode_3)?,
                record_layouts: r.child(decode_11)?,
                record_excluded_a: r.child(decode_10)?,
                record_excluded_b: r.child(decode_10)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Layout>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Layout> {
        let value = match r.tag()? {
            0 => Layout {
                layout_id: r.child(decode_2)?,
                buttons: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, Options>> {
        let value = match r.tag()? {
            0 => r.map(decode_8, decode_9)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Options> {
        let value = match r.tag()? {
            0 => Options {
                group: r.child(decode_8)?,
                field: r.child(decode_8)?,
                choices: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_8)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<u32>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_ALL_MAPPING_DATA,
        0,
        "7e6c96f4d695c1ea4484f69aa503d5dd5d97723ffa614185779fa1f1f299b4a8",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_keyboard_all_mapping_data_decodes() {
    let _ = read_keyboard_all_mapping_catalog();
}
// End generated catalog reader: razer-pages/keyboard_all_mapping_data.

// Generated catalog reader: razer-pages/keyboard_mapping_navigation_data.
fn read_keyboard_mapping_navigation_catalog() -> Vec<Navigation> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Navigation>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Navigation> {
        let value = match r.tag()? {
            0 => Navigation {
                product_id: r.child(decode_2)?,
                confirm_mapping_history: r.child(decode_3)?,
                confirm_mapping_navigation: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_NAVIGATION_DATA,
        0,
        "fb04227c7283170e3255f125606f4a6e381d7125cc9f7d85c82df79b219ade9f",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_keyboard_mapping_navigation_data_decodes() {
    let _ = read_keyboard_mapping_navigation_catalog();
}
// End generated catalog reader: razer-pages/keyboard_mapping_navigation_data.

//! Current 678/679/688 ButtonPanel and mapping popup; vendor values stay unencoded.
//! Receipts: tools/audit_keyboard_mapping_drawer.cjs. No vendor code is executed.
use super::*;
use gpui_kit::component::button::ButtonVariants;
use serde::Serialize;

#[path = "keyboard_mapping_controller.rs"]
mod controller;
#[path = "keyboard_mapping_joystick.rs"]
mod joystick;
#[path = "keyboard_mapping_two_tap.rs"]
mod two_tap;

impl KeyboardProductWorkspace {
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
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_mapping_drawer_data.json"))
            .expect("audited mapping drawer metadata")
    })
    .iter()
    .find(|row| row.product_id == pid)
    .expect("audited analog caller")
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
    joystick: joystick::State,
}
impl Slot {
    fn new(staged: Value) -> Self {
        Self {
            category: category(&staged).into(),
            key_group: staged
                .pointer("/keyboardGroup/keyGroup")
                .and_then(Value::as_u64)
                .unwrap_or(1) as u32,
            original: staged.clone(),
            staged,
            joystick: Default::default(),
        }
    }
}
impl Edit {
    fn dirty(&self) -> bool {
        self.slots[0].original != self.slots[0].staged
            || self.slots[0].joystick.changed
            || self.secondary_enabled != self.original_secondary_enabled
            || self.secondary_enabled && self.slots[1].original != self.slots[1].staged
            || self.secondary_enabled && self.slots[1].joystick.changed
    }
    fn mark_clean(&mut self) {
        for slot in &mut self.slots {
            slot.original = slot.staged.clone();
            slot.joystick.changed = false;
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
}
#[derive(Default)]
pub(super) struct State {
    controller: [controller::State; 2],
    two_tap: two_tap::State,
    joystick_activation: Option<Subscription>,
    edit: Option<Edit>,
    pending_save: Option<Next>,
    filter_customized: bool,
    filter_menu: bool,
    category_menu: [bool; 2],
    target_menu: [bool; 2],
    scroll: ScrollHandle,
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
fn category(mapping: &Value) -> &'static str {
    match mapping["outputType"].as_str() {
        Some("keyboardGroup") => "KEYBOARD_FUNCTION",
        Some("disableGroup") => "DISABLE",
        Some("defaultGroup") | None => "DEFAULT",
        Some("controllerGroup") => "CONTROLLER",
        Some("joystickGroup") => "JOYSTICK",
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
        Some("appSpecificGroup") => "APP_SPECIFIC",
        _ => "DEFAULT",
    }
}

impl KeyboardProductWorkspace {
    pub(in super::super) fn mapping_buttons(&self) -> &[Value] {
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
                    && mapping["isHyperShift"].as_bool().unwrap_or(false) == shift
            })
    }
    pub(in super::super) fn analog_mapping_next(&mut self, next: Next, cx: &mut Context<Self>) {
        if matches!(&next, Next::Key(id, _) if self.selected_key.as_ref()==Some(id)) {
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
        self.analog_mapping_continue(next, cx);
    }
    fn analog_mapping_continue(&mut self, next: Next, cx: &mut Context<Self>) {
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
                if !button["isEnabled"].as_bool().unwrap_or(true)
                    || button["disabled"].as_bool().unwrap_or(false)
                    || (self.hypershift
                        && button["disableHypershiftMapping"]
                            .as_bool()
                            .unwrap_or(false))
                {
                    return;
                }
                if self.selected_key.as_ref() == Some(&id) {
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
                let defaults = two_tap::default_points(self);
                let mut primary = staged;
                if analog && primary["actuationPoint"].is_null() {
                    primary["actuationPoint"] = defaults.clone();
                }
                let secondary = secondary.unwrap_or_else(|| two_tap::disabled_secondary(defaults));
                self.selected_key = Some(id);
                self.analog_gamepad.mapping.edit = Some(Edit {
                    button,
                    shift: self.hypershift,
                    from_panel,
                    slots: [Slot::new(primary), Slot::new(secondary)],
                    secondary_enabled,
                    original_secondary_enabled: secondary_enabled,
                });
                self.analog_gamepad.mapping.category_menu = [false; 2];
                self.analog_gamepad.mapping.target_menu = [false; 2];
                self.analog_gamepad.mapping.controller = Default::default();
                self.init_two_tap_sliders(cx);
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
        }
        cx.notify();
    }
    pub(in super::super) fn clear_analog_mapping(&mut self) {
        self.selected_key = None;
        self.analog_gamepad.mapping.edit = None;
        self.analog_gamepad.mapping.pending_save = None;
        self.analog_gamepad.mapping.category_menu = [false; 2];
        self.analog_gamepad.mapping.target_menu = [false; 2];
        self.analog_gamepad.mapping.controller = Default::default();
        self.analog_gamepad.mapping.two_tap = two_tap::State::default();
        self.analog_gamepad.mapping.joystick_activation = None;
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
    pub(in super::super) fn restore_analog_mapping(&mut self, cx: &mut Context<Self>) {
        self.clear_analog_mapping();
        if !self.spec.analog_gamepad_layout() {
            return;
        }
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
            }
        }
    }
    fn stage_analog_category(&mut self, name: &str, slot: usize, cx: &mut Context<Self>) {
        let option = source(self.spec.product_id).options.get(name);
        let Some(edit) = &mut self.analog_gamepad.mapping.edit else {
            return;
        };
        let metadata = edit.slots[slot].staged.clone();
        edit.slots[slot].category = name.into();
        edit.slots[slot].joystick = Default::default();
        edit.slots[slot].staged = match name {
            "DISABLE" => json!({"outputType":"disableGroup"}),
            "KEYBOARD_FUNCTION" => {
                json!({"outputType":"keyboardGroup","keyboardGroup":{"keyGroup":1,"key":"","modifiers":[]}})
            }
            "CONTROLLER" => controller::payload(self.spec.product_id, "", "STANDARD", None),
            "JOYSTICK" => joystick::payload(self.spec.product_id, false, 0),
            "DEFAULT" => json!({"outputType":"defaultGroup","isDefault":true}),
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
        let merged = two_tap::merge(self, &edit, mappings);
        if let Some(target) = self.draft.pointer_mut(&path) {
            *target = json!(merged);
        }
        self.clear_analog_mapping();
        cx.emit(KeyboardProductChanged);
        self.request_actuation_mapping(cx);
        cx.notify();
        true
    }

    pub(in super::super) fn analog_mapping_drawer(&self, cx: &Context<Self>) -> AnyElement {
        let state = &self.analog_gamepad.mapping;
        let mut rows = self
            .mapping_buttons()
            .iter()
            .filter(|button| {
                !button["disabled"].as_bool().unwrap_or(false)
                    && button["isSidePanelList"].as_bool() != Some(false)
                    && match button["isKeyToggle"].as_bool() {
                        None => true,
                        Some(toggle) => toggle == self.hypershift,
                    }
                    && (!self.hypershift
                        || button["HID"].is_null()
                        || !button["disableHypershiftMapping"]
                            .as_bool()
                            .unwrap_or(false))
                    && (!state.filter_customized
                        || self
                            .current_mapping(
                                button["inputID"].as_str().unwrap_or_default(),
                                self.hypershift,
                            )
                            .is_some())
                    && button["counter"] != "mediaVolume"
            })
            .collect::<Vec<_>>();
        if rows
            .iter()
            .all(|button| button["counter"].as_i64().is_some())
        {
            rows.sort_by_key(|button| button["counter"].as_i64());
        }
        v_flex()
            .w(surface::css(230.))
            .h_full()
            .bg(rgb(0x222222))
            .flex_shrink_0()
            .child(
                h_flex()
                    .h(surface::css(67.))
                    .px(surface::css(10.))
                    .items_center()
                    .child(
                        BaseButton::new("analog-drawer-filter")
                            .flex_1()
                            .child(t(if state.filter_customized {
                                "CUSTOMIZED"
                            } else {
                                "ALL_BUTTONS"
                            }))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.analog_gamepad.mapping.filter_menu =
                                    !this.analog_gamepad.mapping.filter_menu;
                                cx.notify();
                            })),
                    )
                    .child(BaseButton::new("analog-drawer-close").child("×").on_click(
                        cx.listener(|this, _, _, cx| this.analog_mapping_next(Next::Drawer, cx)),
                    )),
            )
            .when(state.filter_menu, |panel| {
                panel.children([(false, "ALL_BUTTONS"), (true, "CUSTOMIZED")].map(
                    |(custom, label)| {
                        BaseButton::new(SharedString::from(format!("analog-filter-{custom}")))
                            .child(t(label))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.analog_gamepad.mapping.filter_customized = custom;
                                this.analog_gamepad.mapping.filter_menu = false;
                                cx.notify();
                            }))
                    },
                ))
            })
            .child(
                v_flex()
                    .id("analog-drawer-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&state.scroll)
                    .children(rows.into_iter().map(|button| {
                        let input = text(&button["inputID"]);
                        let key = text(&button["buttonKey"]);
                        let mapping = self.current_mapping(&input, self.hypershift);
                        let mapped = mapping.is_some();
                        let primary = mapping
                            .and_then(|mapping| mapping.pointer("/mapping/0"))
                            .or(mapping);
                        let label = primary
                            .and_then(|mapping| mapping.pointer("/keyboardGroup/key"))
                            .map(text)
                            .unwrap_or_else(|| text(&button["assignmentValue"]));
                        BaseButton::new(SharedString::from(format!("analog-drawer-row-{key}")))
                            .min_h(surface::css(50.))
                            .p(surface::css(9.))
                            .w_full()
                            .flex()
                            .items_center()
                            .disabled(!button["isEnabled"].as_bool().unwrap_or(true))
                            .bg(if self.selected_key.as_ref() == Some(&key) {
                                rgb(0x111111)
                            } else {
                                rgb(0x222222)
                            })
                            .hover(|row| row.bg(rgb(0x383838)))
                            .child(
                                div()
                                    .w(surface::css(56.))
                                    .ml(surface::css(-10.))
                                    .child(text(&button["counter"])),
                            )
                            .child(
                                v_flex()
                                    .w(surface::css(164.))
                                    .border_l_1()
                                    .border_color(rgb(0x707070))
                                    .pl(surface::css(5.))
                                    .text_color(if mapped {
                                        if self.hypershift {
                                            rgb(0xfd8611)
                                        } else {
                                            rgb(0x44d62c)
                                        }
                                    } else {
                                        rgb(0xcccccc)
                                    })
                                    .child(div().text_size(surface::css(10.)).child(t(
                                        primary.map(category).unwrap_or_else(|| {
                                            button["assignment"].as_str().unwrap_or("DEFAULT")
                                        }),
                                    )))
                                    .child(
                                        div()
                                            .text_size(surface::css(14.))
                                            .overflow_hidden()
                                            .child(label),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.analog_mapping_next(Next::Key(key.clone(), true), cx)
                            }))
                    })),
            )
            .into_any_element()
    }

    pub(in super::super) fn analog_mapping_popup(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let edit = self.analog_gamepad.mapping.edit.as_ref()?;
        let expanded = edit.secondary_enabled && edit.button["inputType"] == "AnalogInput";
        let width = if expanded { 543. } else { 292. };
        Some(
            v_flex()
                .w(surface::css(width))
                .bg(rgb(0x222222))
                .border_1()
                .border_color(rgb(0x5d5d5d))
                .child(
                    h_flex()
                        .h(surface::css(36.))
                        .px(surface::css(10.))
                        .items_center()
                        .bg(rgb(0x111111))
                        .child(div().flex_1().child(text(&edit.button["defaultValue"])))
                        .child(
                            BaseButton::new("analog-mapping-close")
                                .accessibility_label(t("CLOSE"))
                                .child("\u{00d7}")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.analog_mapping_next(Next::Close, cx)
                                })),
                        ),
                )
                .child(
                    h_flex()
                        .items_start()
                        .child(self.analog_mapping_slot(0, cx))
                        .when(expanded, |row| row.child(self.analog_mapping_slot(1, cx))),
                )
                .into_any_element(),
        )
    }
    fn analog_mapping_slot(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let state = &self.analog_gamepad.mapping;
        let edit = state.edit.as_ref().expect("mounted mapping slot");
        let source = source(self.spec.product_id);
        let mut popup = v_flex()
            .w(surface::css(if slot == 0 { 292. } else { 251. }))
            .when(slot == 1, |panel| {
                panel.border_l_1().border_color(rgb(0x5d5d5d))
            });
        if slot == 0 || two_tap::extended(self.spec.product_id) {
            popup = popup.child(
                BaseButton::new(SharedString::from(format!(
                    "analog-mapping-category-{slot}"
                )))
                .m(surface::css(20.))
                .child(t(&edit.slots[slot].category))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.analog_gamepad.mapping.category_menu[slot] =
                        !this.analog_gamepad.mapping.category_menu[slot];
                    cx.notify();
                })),
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
        if state.category_menu[slot] {
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
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.stage_analog_category(&name, slot, cx)
                                    })),
                                )
                            }),
                    ),
            );
        }
        if let Some(options) = source.options.get(&edit.slots[slot].category) {
            popup = popup.child(
                v_flex()
                    .mx(surface::css(20.))
                    .id(SharedString::from(format!("analog-mapping-options-{slot}")))
                    .max_h(surface::css(200.))
                    .overflow_y_scroll()
                    .children(options.choices.iter().map(|choice| {
                        let name = text(&choice["content"]);
                        let id = text(&choice["id"]);
                        let group = options.group.clone();
                        let field = options.field.clone();
                        let selected = edit.slots[slot].staged[&group][&field] == id;
                        Button::new(SharedString::from(format!(
                            "analog-mapping-option-{slot}-{id}"
                        )))
                        .label(t(&name))
                        .outline()
                        .selected(selected)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                let mut payload = json!({"outputType":group});
                                payload[&group] = json!({});
                                payload[&group][&field] = json!(id);
                                two_tap::replace_slot(&mut edit.slots[slot], payload);
                                if edit.slots[slot].category == "MULTIMEDIA"
                                    && matches!(name.as_str(), "VOLUME_UP" | "VOLUME_DOWN")
                                {
                                    edit.slots[slot].staged[&group]["useTurbo"] = json!(true);
                                }
                            }
                            cx.emit(KeyboardProductChanged);
                            cx.notify();
                        }))
                    })),
            );
            if edit.slots[slot].category == "MOUSE_FUNCTION"
                && !source
                    .no_turbo
                    .iter()
                    .any(|id| edit.button["buttonKey"] == id.as_str())
                && edit.button["inputType"] != "ControllerInput"
                && !edit.button["disableTurboInAssignment"]
                    .as_array()
                    .is_some_and(|rows| rows.iter().any(|row| row == "MOUSE_FUNCTION"))
            {
                let enabled = edit.slots[slot]
                    .staged
                    .pointer("/mouseGroup/turboMode/isTurbo")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let speed = edit.slots[slot]
                    .staged
                    .pointer("/mouseGroup/turboMode/keysPerSecond")
                    .and_then(Value::as_u64)
                    .unwrap_or(7);
                popup =
                    popup
                        .child(
                            Checkbox::new(SharedString::from(format!("analog-mouse-turbo-{slot}")))
                                .label(t("ENABLE_TURBO"))
                                .checked(enabled)
                                .on_change(cx.listener(move |this, enabled: &bool, _, cx| {
                                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                        if *enabled {
                                            edit.slots[slot].staged["mouseGroup"]["turboMode"] =
                                                json!({"isTurbo":true,"keysPerSecond":7});
                                        } else if let Some(group) =
                                            edit.slots[slot].staged["mouseGroup"].as_object_mut()
                                        {
                                            group.remove("turboMode");
                                        }
                                    }
                                    cx.emit(KeyboardProductChanged);
                                    cx.notify();
                                })),
                        )
                        .when(enabled, |popup| {
                            popup.child(
                                h_flex()
                                    .mx(surface::css(20.))
                                    .gap_2()
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "analog-mouse-turbo-minus-{slot}"
                                        )))
                                        .label("−")
                                        .outline()
                                        .disabled(speed <= 1)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if let Some(edit) =
                                                &mut this.analog_gamepad.mapping.edit
                                            {
                                                edit.slots[slot].staged["mouseGroup"]["turboMode"]
                                                    ["keysPerSecond"] =
                                                    json!(speed.saturating_sub(1).max(1));
                                            }
                                            cx.emit(KeyboardProductChanged);
                                            cx.notify();
                                        })),
                                    )
                                    .child(speed.to_string())
                                    .child(
                                        Button::new(SharedString::from(format!(
                                            "analog-mouse-turbo-plus-{slot}"
                                        )))
                                        .label("+")
                                        .outline()
                                        .disabled(speed >= 20)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if let Some(edit) =
                                                &mut this.analog_gamepad.mapping.edit
                                            {
                                                edit.slots[slot].staged["mouseGroup"]["turboMode"]
                                                    ["keysPerSecond"] = json!((speed + 1).min(20));
                                            }
                                            cx.emit(KeyboardProductChanged);
                                            cx.notify();
                                        })),
                                    ),
                            )
                        });
            }
        }
        if edit.slots[slot].category == "CONTROLLER" {
            popup = popup.child(controller::element(self, slot, cx));
        }
        if edit.slots[slot].category == "JOYSTICK" {
            popup = popup.child(joystick::element(self, slot, cx));
        }
        if edit.slots[slot].category == "KEYBOARD_FUNCTION" {
            popup = popup
                .child(
                    h_flex().mx(surface::css(20.)).flex_wrap().gap_1().children(
                        [
                            (1, "Alphanumeric"),
                            (2, "Function"),
                            (3, "Numpad"),
                            (4, "Navigation"),
                            (5, "Modifiers"),
                            (6, "Symbols"),
                        ]
                        .map(|(group, label)| {
                            Button::new(SharedString::from(format!(
                                "analog-key-group-{slot}-{group}"
                            )))
                            .label(label)
                            .outline()
                            .selected(edit.slots[slot].key_group == group)
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                        edit.slots[slot].key_group = group;
                                    }
                                    cx.notify();
                                },
                            ))
                        }),
                    ),
                )
                .child(
                    BaseButton::new(SharedString::from(format!("analog-key-target-menu-{slot}")))
                        .m(surface::css(20.))
                        .child(text(&edit.slots[slot].staged["keyboardGroup"]["key"]))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.analog_gamepad.mapping.target_menu[slot] =
                                !this.analog_gamepad.mapping.target_menu[slot];
                            cx.notify();
                        })),
                );
            if state.target_menu[slot] {
                let kind = match edit.slots[slot].key_group {
                    2 => "Function",
                    3 => "Numpad",
                    4 => "Navigation",
                    5 => "Modifiers",
                    6 => "Symbols",
                    _ => "Alphanumeric",
                };
                popup = popup.child(
                    v_flex()
                        .id(SharedString::from(format!("analog-key-targets-{slot}")))
                        .max_h(surface::css(180.))
                        .overflow_y_scroll()
                        .children(
                            source
                                .key_targets
                                .iter()
                                .filter(|key| key["type"] == kind && key["inputID"].is_string())
                                .map(|key| {
                                    let id = text(&key["inputID"]);
                                    BaseButton::new(SharedString::from(format!(
                                        "analog-key-target-{slot}-{id}"
                                    )))
                                    .px(surface::css(20.))
                                    .py(surface::css(5.))
                                    .child(text(&key["name"]))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                            edit.slots[slot].staged["keyboardGroup"]["key"] = json!(id);
                                            edit.slots[slot].staged["keyboardGroup"]["keyGroup"] =
                                                json!(edit.slots[slot].key_group);
                                            if edit.slots[slot].key_group == 5 {
                                                edit.slots[slot].staged["keyboardGroup"]["modifiers"] =
                                                    json!([]);
                                            }
                                        }
                                        this.analog_gamepad.mapping.target_menu[slot] = false;
                                        cx.emit(KeyboardProductChanged);
                                        cx.notify();
                                    }))
                                }),
                        ),
                );
            }
            popup = popup.child(
                v_flex().mx(surface::css(20.)).children(
                    source
                        .key_targets
                        .iter()
                        .filter(|key| {
                            matches!(
                                key["inputID"].as_str(),
                                Some(
                                    "KEY_LEFT_CTRL"
                                        | "KEY_RIGHT_CTRL"
                                        | "KEY_LEFT_SHIFT"
                                        | "KEY_RIGHT_SHIFT"
                                        | "KEY_LEFT_ALT"
                                        | "KEY_RIGHT_ALT"
                                )
                            )
                        })
                        .map(|key| {
                            let id = text(&key["inputID"]);
                            let opposite = id.replace("LEFT_", "RIGHT_");
                            let opposite = if opposite == id {
                                id.replace("RIGHT_", "LEFT_")
                            } else {
                                opposite
                            };
                            let checked = edit.slots[slot].staged["keyboardGroup"]["modifiers"]
                                .as_array()
                                .is_some_and(|values| values.iter().any(|value| value == &id));
                            Checkbox::new(SharedString::from(format!(
                                "analog-key-modifier-{slot}-{id}"
                            )))
                            .label(text(&key["name"]))
                            .checked(checked)
                            .on_change(cx.listener(
                                move |this, value: &bool, _, cx| {
                                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                        let mut modifiers =
                                            edit.slots[slot].staged["keyboardGroup"]["modifiers"]
                                                .as_array()
                                                .cloned()
                                                .unwrap_or_default();
                                        modifiers.retain(|modifier| {
                                            modifier != &id && (!*value || modifier != &opposite)
                                        });
                                        if *value {
                                            modifiers.push(json!(id));
                                        }
                                        edit.slots[slot].staged["keyboardGroup"]["modifiers"] =
                                            json!(modifiers);
                                    }
                                    cx.emit(KeyboardProductChanged);
                                    cx.notify();
                                },
                            ))
                        }),
                ),
            );
        }

        popup = popup.children(two_tap::actuation_element(self, slot, cx));
        if !edit.secondary_enabled || slot == 1 {
            popup = popup.child(
                h_flex()
                    .p(surface::css(20.))
                    .gap(surface::css(10.))
                    .justify_end()
                    .child(
                        Button::new(SharedString::from(format!("analog-mapping-cancel-{slot}")))
                            .label(t("CANCEL"))
                            .outline()
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
                            .disabled(!edit.dirty() || !two_tap::valid(self, edit))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.save_analog_mapping(cx);
                            })),
                    ),
            );
        }
        popup = popup.children(two_tap::secondary_element(self, slot, cx));
        popup.into_any_element()
    }
    pub(in super::super) fn analog_mapping_confirmation(
        &self,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        self.analog_gamepad.mapping.pending_save.as_ref()?;
        Some(
            v_flex()
                .w(surface::css(420.))
                .p(surface::css(20.))
                .gap(surface::css(20.))
                .bg(rgb(0x222222))
                .border_1()
                .border_color(rgb(0x707070))
                .child(t("SAVE_CHANGES"))
                .child(
                    h_flex()
                        .gap(surface::css(10.))
                        .justify_end()
                        .child(
                            Button::new("analog-confirm-dismiss")
                                .label(t("CANCEL"))
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.analog_gamepad.mapping.pending_save = None;
                                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                        edit.mark_clean();
                                    }
                                    cx.emit(KeyboardProductChanged);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("analog-confirm-discard")
                                .label(t("DONT_SAVE"))
                                .outline()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    // Root dontSave() invokes only dontSaveActionRef; these callers
                                    // supplied nextActionRef only, so the queued action is not run.
                                    this.analog_gamepad.mapping.pending_save = None;
                                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                        edit.mark_clean();
                                    }
                                    cx.emit(KeyboardProductChanged);
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("analog-confirm-save")
                                .label(t("SAVE"))
                                .primary()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let next = this.analog_gamepad.mapping.pending_save.clone();
                                    if this.save_analog_mapping(cx) {
                                        if let Some(next) = next {
                                            this.analog_mapping_continue(next, cx);
                                        }
                                    }
                                })),
                        ),
                )
                .into_any_element(),
        )
    }
}

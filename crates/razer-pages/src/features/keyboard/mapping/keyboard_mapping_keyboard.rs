//! Current MapKeyboard 96223 and focused recording component 56193.
use super::*;
use crate::features::controls::Choice;
use gpui_kit::component::IndexPath;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use razer_widgets::keyboard_capture::{Capture, RawKey};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct State {
    groups: [Option<Entity<SelectState<Vec<Choice>>>>; 2],
    targets: [Option<Entity<SelectState<Vec<Choice>>>>; 2],
    focus: [Option<FocusHandle>; 10],
    subscriptions: [Vec<Subscription>; 2],
    recording: Option<usize>,
    capture: Option<Capture>,
    timer: Option<Task<()>>,
    generation: u64,
    pressed: Vec<String>,
    modifiers: Vec<String>,
    released: Vec<String>,
    combination: Option<String>,
    reset_modifiers_at: Option<Instant>,
    redirect_fn: bool,
    redirect_pressed: BTreeMap<u64, &'static Value>,
    system_keyboard_layout: Option<u32>,
    clear_hover: [bool; 10],
}
impl State {
    pub(super) fn stop(&mut self) {
        self.capture = None;
        self.timer = None;
        self.recording = None;
        self.generation = self.generation.wrapping_add(1);
        self.pressed.clear();
        self.modifiers.clear();
        self.released.clear();
        self.combination = None;
        self.reset_modifiers_at = None;
        self.redirect_fn = false;
        self.redirect_pressed.clear();
        self.clear_hover = [false; 10];
    }
    pub(super) fn dismiss(&mut self) {
        self.stop();
        self.groups = Default::default();
        self.targets = Default::default();
        self.focus = Default::default();
        self.subscriptions = Default::default();
    }
}
fn is_modifier(id: &str) -> bool {
    matches!(
        id,
        "KEY_RIGHT_ALT"
            | "KEY_LEFT_ALT"
            | "KEY_LEFT_SHIFT"
            | "KEY_RIGHT_SHIFT"
            | "KEY_LEFT_GUI"
            | "KEY_LEFT_CTRL"
            | "KEY_RIGHT_CTRL"
    )
}
fn recording_placeholder() -> String {
    let label = t("REBIND_KEY_TO");
    let first = label.chars().next().map(char::len_utf8).unwrap_or(0);
    format!("{}{}...", &label[..first], label[first..].to_lowercase())
}
fn raw_target(workspace: &KeyboardProductWorkspace, event: &RawKey) -> Option<&'static Value> {
    let source = source(workspace.spec.product_id);
    let alternate = workspace
        .analog_gamepad
        .layout
        .is_some_and(|layout| source.record_layouts.contains(&layout));
    let excluded = if alternate {
        &source.record_excluded_a
    } else {
        &source.record_excluded_b
    };
    let flag = if event.extended() { 2 } else { 0 };
    source
        .key_targets
        .iter()
        .find(|key| {
            key["scancode"].as_u64() == Some(event.scan() as u64)
                && key
                    .get("outputFlag")
                    .or_else(|| key.get("flag"))
                    .and_then(Value::as_u64)
                    == Some(flag)
                && key["inputID"]
                    .as_str()
                    .is_some_and(|id| !excluded.iter().any(|item| item == id))
        })
        .or_else(|| {
            // The source browser fallback takes the first keyCode match, except
            // these explicit KeyboardEvent.code aliases. No left-side inference.
            let alias = match event.code() {
                0x91 => Some("KEY_SCROLL_LOCK"),
                0x13 => Some("KEY_PAUSE"),
                0x90 => Some("KEY_NUMPAD_NUM_LOCK"),
                _ => None,
            };
            source.key_targets.iter().find(|key| {
                alias
                    .map(|id| key["inputID"] == id)
                    .unwrap_or_else(|| key["keyCode"] == event.code().to_string())
            })
        })
}

#[derive(Deserialize)]
struct RecordingSource {
    product_id: u32,
    fn_maps: BTreeMap<String, BTreeMap<String, String>>,
    mini_products: Vec<u32>,
    key_names: BTreeMap<String, BTreeMap<String, Value>>,
}
fn recording_source(pid: u32) -> &'static RecordingSource {
    static DATA: OnceLock<Vec<RecordingSource>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_recording_catalog())
        .iter()
        .find(|row| row.product_id == pid)
        .expect("audited keyboard recording source")
}

impl KeyboardProductWorkspace {
    pub(super) fn recording_key_name(&self, input_id: &str) -> String {
        let names = &recording_source(self.spec.product_id).key_names;
        let table = self
            .analog_gamepad
            .mapping
            .keyboard
            .system_keyboard_layout
            .and_then(|layout| names.get(&layout.to_string()))
            .or_else(|| names.get("default"));
        let value = table.and_then(|table| table.get(input_id));
        let Some(value) = value else {
            return names
                .get("default")
                .and_then(|table| table.get(input_id))
                .and_then(Value::as_str)
                .unwrap_or(input_id)
                .into();
        };
        value
            .as_str()
            .or_else(|| value["DEFAULT"].as_str())
            .unwrap_or(input_id)
            .into()
    }
    /// Observed system keyboard layout; no default or device success is invented.
    pub fn observe_mapping_system_keyboard_layout(&mut self, layout: u32, cx: &mut Context<Self>) {
        self.analog_gamepad.mapping.keyboard.system_keyboard_layout = Some(layout);
        cx.notify();
    }
    /// Current inputredirect payload handler. The service adapter must provide
    /// the original event productId and JSON input; native subscription and
    /// enable/disable lifecycle are separate, unfinished integration work.
    pub fn consume_mapping_input_redirect(
        &mut self,
        product_id: u32,
        input: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(slot) = self.analog_gamepad.mapping.keyboard.recording else {
            return false;
        };
        let Ok(mut input) = serde_json::from_str::<Value>(input) else {
            return false;
        };
        if let Some(key) = input["key"].as_str() {
            if let Some(redirect) = self.spec.config["DeviceInfo"]["redirectKeyInput"]
                .as_array()
                .and_then(|rows| rows.iter().find(|row| row["key"] == key))
            {
                input["type"] = json!("keyboard");
                input["scancode"] = redirect["scancodeRedirect"].clone();
            }
        }
        if input["type"] == "razerKey" {
            self.analog_gamepad.mapping.keyboard.redirect_fn = input["flag"].as_u64() == Some(0);
            return true;
        }
        if input["type"] != "keyboard" && input["type"] != "analogKey" {
            return false;
        }
        let (Some(mut scan), Some(mut flag)) = (input["scancode"].as_u64(), input["flag"].as_u64())
        else {
            return false;
        };
        let source = source(self.spec.product_id);
        let alternate = self
            .analog_gamepad
            .layout
            .is_some_and(|layout| source.record_layouts.contains(&layout));
        let excluded = if alternate {
            &source.record_excluded_a
        } else {
            &source.record_excluded_b
        };
        let candidates = source
            .key_targets
            .iter()
            .filter(|key| !excluded.iter().any(|id| key["inputID"] == id.as_str()))
            .collect::<Vec<_>>();
        let recording = recording_source(self.spec.product_id);
        if self.spec.config["DeviceInfo"]["isMiniKeyboard"] == true
            || recording.mini_products.contains(&product_id)
        {
            let transformed = (|| {
                let Some(original) = candidates.iter().find(|row| {
                    row["scancode"].as_u64() == Some(scan)
                        && row["outputFlag"]
                            .as_u64()
                            .map(|output| output + flag)
                            .unwrap_or(flag)
                            == flag
                }) else {
                    return (scan, flag);
                };
                let original_id = original["inputID"].as_str().unwrap_or_default();
                let japanese = self.analog_gamepad.layout == Some(12)
                    || self.analog_gamepad.mapping.keyboard.system_keyboard_layout == Some(12);
                let swap = self.spec.config["DeviceInfo"]["AnalogGenVersion"] == "analogV2"
                    && japanese
                    && matches!(original_id, "KEY_ESC" | "KEY_TILDE");
                let fn_active = self.analog_gamepad.mapping.keyboard.redirect_fn;
                let mapped = if fn_active {
                    if recording.mini_products.contains(&product_id) {
                        recording
                            .fn_maps
                            .get(&product_id.to_string())
                            .and_then(|mapping| mapping.get(original_id))
                            .map(String::as_str)
                    } else {
                        self.spec.config["DEFAULTMAPPINGS"]
                            .as_array()
                            .and_then(|rows| rows.iter().find(|row| row["inputID"] == original_id))
                            .and_then(|row| row["keyboardGroup"]["key"].as_str())
                    }
                } else {
                    Some(original_id)
                };
                let Some(mapped) = mapped else {
                    return (scan, flag);
                };
                let eligible = candidates.iter().any(|row| {
                    row["inputID"] == mapped
                        && matches!(
                            row["type"].as_str(),
                            Some("Function" | "Navigation" | "Modifiers")
                        )
                }) || matches!(mapped, "KEY_ESC" | "KEY_TILDE");
                if !fn_active || eligible {
                    let swapped = if swap {
                        match mapped {
                            "KEY_ESC" => "KEY_TILDE",
                            "KEY_TILDE" => "KEY_ESC",
                            other => other,
                        }
                    } else {
                        mapped
                    };
                    if let Some(target) = candidates.iter().find(|row| row["inputID"] == swapped) {
                        scan = target["scancode"].as_u64().unwrap_or(scan);
                        if fn_active && !swap {
                            flag = target["outputFlag"]
                                .as_u64()
                                .map(|output| output + flag)
                                .unwrap_or(flag);
                        }
                    }
                }
                (scan, flag)
            })();
            scan = transformed.0;
            flag = transformed.1;
        }
        let Some(key) = candidates.into_iter().find(|key| {
            key["scancode"].as_u64() == Some(scan)
                && key
                    .get("outputFlag")
                    .filter(|value| value.is_number())
                    .or_else(|| key.get("flag"))
                    .and_then(Value::as_u64)
                    .is_some_and(|base| flag == base || flag == base + 1)
        }) else {
            return false;
        };
        let state = &mut self.analog_gamepad.mapping.keyboard;
        let down = flag % 2 == 0;
        if down {
            state.redirect_pressed.insert(scan, key);
            if state
                .redirect_pressed
                .get(&29)
                .is_some_and(|key| key["outputFlag"] == 4)
                && state.redirect_pressed.contains_key(&69)
            {
                state.redirect_pressed.clear();
                return true;
            }
        } else {
            state.redirect_pressed.clear();
        }
        self.consume_mapping_keyboard_target(slot, key, down, cx);
        true
    }
    fn mapping_key_candidates(&self, group: u32) -> Vec<&'static Value> {
        let source = source(self.spec.product_id);
        let kind = source
            .key_groups
            .iter()
            .find(|row| row["id"] == group)
            .and_then(|row| row["content"].as_str())
            .unwrap_or_default();
        source
            .key_targets
            .iter()
            .filter(|key| key["type"] == kind)
            .collect()
    }
    fn stage_mapping_keyboard_target(&mut self, slot: usize, index: usize, cx: &mut Context<Self>) {
        let Some(edit) = self.analog_gamepad.mapping.edit.as_ref() else {
            return;
        };
        let group = edit.slots[slot].key_group;
        let Some(target) = self.mapping_key_candidates(group).get(index).copied() else {
            return;
        };
        let source = source(self.spec.product_id);
        let mut modifiers = edit.slots[slot].staged["keyboardGroup"]["modifiers"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if group == 5 {
            modifiers.clear();
        }
        let mut key = target;
        if let Some(modifier_code) = target["modifier"].as_str() {
            if !modifiers.iter().any(|id| {
                source
                    .key_targets
                    .iter()
                    .any(|row| row["inputID"] == *id && row["keyCode"] == modifier_code)
            }) {
                if let Some(modifier) = source
                    .key_targets
                    .iter()
                    .find(|row| row["keyCode"] == modifier_code)
                {
                    modifiers.push(modifier["inputID"].clone());
                }
            }
            if let Some(base) = source
                .key_targets
                .iter()
                .find(|row| row["keyCode"] == target["keyCode"])
            {
                key = base;
            }
        }
        let Some(id) = key["inputID"].as_str() else {
            return;
        };
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            edit.slots[slot].staged["keyboardGroup"]["keyGroup"] = json!(group);
            edit.slots[slot].staged["keyboardGroup"]["key"] = json!(id);
            edit.slots[slot].staged["keyboardGroup"]["modifiers"] = json!(modifiers);
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    fn change_mapping_keyboard_group(
        &mut self,
        slot: usize,
        group: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.analog_gamepad.mapping.keyboard.stop();
        let Some(edit) = &mut self.analog_gamepad.mapping.edit else {
            return;
        };
        edit.slots[slot].key_group = group;
        edit.slots[slot].staged["keyboardGroup"]["keyGroup"] = json!(group);
        edit.slots[slot].staged["keyboardGroup"]["key"] = json!("");
        if group != 0 {
            self.stage_mapping_keyboard_target(slot, 0, cx);
        }
        let candidates = self
            .mapping_key_candidates(group)
            .iter()
            .enumerate()
            .map(|(index, row)| Choice::new(index.to_string(), text(&row["name"])))
            .collect::<Vec<_>>();
        if let Some(targets) = &self.analog_gamepad.mapping.keyboard.targets[slot] {
            targets.update(cx, |targets, cx| {
                targets.set_items(candidates, window, cx);
                targets.set_selected_index((group != 0).then_some(IndexPath::new(0)), window, cx);
            });
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    pub(super) fn init_mapping_keyboard(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(edit) = self.analog_gamepad.mapping.edit.as_ref() else {
            return;
        };
        if edit.slots[slot].category != "KEYBOARD_FUNCTION" {
            if self.analog_gamepad.mapping.keyboard.recording == Some(slot) {
                self.analog_gamepad.mapping.keyboard.stop();
            }
            return;
        }
        let group = edit.slots[slot].staged["keyboardGroup"]["keyGroup"]
            .as_u64()
            .unwrap_or(0) as u32;
        let groups = source(self.spec.product_id)
            .key_groups
            .iter()
            .map(|row| Choice::localized(text(&row["id"]), text(&row["label"])))
            .collect::<Vec<_>>();
        let candidates = self.mapping_key_candidates(group);
        let selected = candidates
            .iter()
            .position(|row| row["inputID"] == edit.slots[slot].staged["keyboardGroup"]["key"]);
        let targets = candidates
            .iter()
            .enumerate()
            .map(|(index, row)| Choice::new(index.to_string(), text(&row["name"])))
            .collect::<Vec<_>>();
        let groups = cx.new(|cx| {
            SelectState::new(groups, Some(IndexPath::new(group as usize)), window, cx)
                .searchable(false)
        });
        let targets = cx.new(|cx| {
            SelectState::new(targets, selected.map(IndexPath::new), window, cx).searchable(false)
        });
        let group_subscription =
            cx.subscribe_in(&groups, window, move |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Ok(group) = value.parse() {
                        this.change_mapping_keyboard_group(slot, group, window, cx);
                    }
                }
            });
        let target_subscription =
            cx.subscribe_in(&targets, window, move |this, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Ok(index) = value.parse() {
                        this.stage_mapping_keyboard_target(slot, index, cx);
                    }
                }
            });
        let state = &mut self.analog_gamepad.mapping.keyboard;
        state.groups[slot] = Some(groups);
        state.targets[slot] = Some(targets);
        state.focus[slot] = Some(cx.focus_handle());
        state.subscriptions[slot] = vec![group_subscription, target_subscription];
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            edit.slots[slot].key_group = group;
            edit.slots[slot].modifier_included =
                edit.slots[slot].staged["keyboardGroup"]["modifiers"]
                    .as_array()
                    .is_some_and(|modifiers| !modifiers.is_empty());
        }
    }
    fn start_mapping_keyboard_recording(
        &mut self,
        slot: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.analog_gamepad.mapping.keyboard.recording == Some(slot)
            || self.customize_controls_disabled()
        {
            return;
        }
        if self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .is_some_and(|edit| slot < 2 && edit.slots[slot].category == "APP_SPECIFIC")
        {
            self.activate_mapping_app_custom(slot, cx);
        }
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let allowed = if slot >= 2 {
            dynamic::recording_group(self, slot).is_some_and(|group| group["keyGroup"] == 0)
        } else {
            edit.slots[slot].category == "KEYBOARD_FUNCTION" && edit.slots[slot].key_group == 0
                || edit.slots[slot].category == "APP_SPECIFIC" && app_specific::custom(self, slot)
        };
        if !allowed {
            return;
        }
        self.analog_gamepad.mapping.keyboard.stop();
        let Some(capture) = Capture::start_stream(window) else {
            return;
        };
        let state = &mut self.analog_gamepad.mapping.keyboard;
        state.capture = Some(capture);
        state.recording = Some(slot);
        let generation = state.generation;
        if let Some(focus) = &state.focus[slot] {
            focus.focus(window, cx);
        }
        let timer = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let keep = this
                    .update_in(cx, |this, window, cx| {
                        if this.analog_gamepad.mapping.keyboard.generation != generation {
                            return false;
                        }
                        if !window.is_window_active() {
                            this.analog_gamepad.mapping.keyboard.stop();
                            cx.notify();
                            return false;
                        }
                        let events = this
                            .analog_gamepad
                            .mapping
                            .keyboard
                            .capture
                            .as_mut()
                            .and_then(Capture::take_all);
                        let Some(events) = events else {
                            this.analog_gamepad.mapping.keyboard.stop();
                            cx.notify();
                            return false;
                        };
                        for event in events {
                            this.consume_mapping_keyboard_key(slot, &event, cx);
                        }
                        let state = &mut this.analog_gamepad.mapping.keyboard;
                        if state
                            .reset_modifiers_at
                            .is_some_and(|deadline| Instant::now() >= deadline)
                        {
                            state.modifiers.clear();
                            state.reset_modifiers_at = None;
                        }
                        state.recording == Some(slot)
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        });
        self.analog_gamepad.mapping.keyboard.timer = Some(timer);
        cx.notify();
    }
    fn consume_mapping_keyboard_key(
        &mut self,
        slot: usize,
        event: &RawKey,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = raw_target(self, event) else {
            return;
        };
        self.consume_mapping_keyboard_target(slot, key, event.down(), cx);
    }
    fn consume_mapping_keyboard_target(
        &mut self,
        slot: usize,
        key: &'static Value,
        down: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = key["inputID"].as_str() else {
            return;
        };
        let state = &mut self.analog_gamepad.mapping.keyboard;
        if state.recording != Some(slot) {
            return;
        }
        if down {
            if !state.pressed.iter().any(|item| item == id) {
                state.pressed.push(id.into());
            }
            if is_modifier(id) && !state.modifiers.iter().any(|item| item == id) {
                state.modifiers.push(id.into());
            }
            return;
        }
        if !state.pressed.iter().any(|item| item == id) {
            return;
        }
        state.pressed.retain(|item| item != id);
        let selected = if is_modifier(id) {
            if !state.released.iter().any(|item| item == id) {
                state.released.push(id.into());
            }
            if state.modifiers.len() != state.released.len() || state.modifiers.is_empty() {
                return;
            }
            let selected = if state.combination.take().is_some() {
                None
            } else {
                Some(state.modifiers.clone())
            };
            state.released.clear();
            state.reset_modifiers_at = Some(
                Instant::now()
                    + if state.pressed.is_empty() {
                        Duration::ZERO
                    } else {
                        Duration::from_millis(300)
                    },
            );
            selected
        } else {
            if state.combination.as_deref() == Some(id) {
                return;
            }
            let mut selected = state.modifiers.clone();
            if !selected.is_empty() && state.pressed.iter().any(|id| selected.contains(id)) {
                state.combination = Some(id.into());
            }
            selected.push(id.into());
            Some(selected)
        };
        let Some(mut selected) = selected else { return };
        // Preserve the original ue() conversion, whose find callback returns
        // a truthy array even when no Ctrl key is present.
        for id in &mut selected {
            if id == "KEY_CANCEL" {
                *id = "KEY_PAUSE".into();
            }
        }
        let Some(key) = selected.pop() else { return };
        if slot >= 2 {
            self.change_mapping_dynamic_recording(slot, key, selected, cx);
            return;
        }
        if let Some(edit) = &mut self.analog_gamepad.mapping.edit {
            edit.slots[slot].staged["keyboardGroup"]["key"] = json!(key);
            edit.slots[slot].staged["keyboardGroup"]["modifiers"] = json!(selected);
            edit.slots[slot].staged["keyboardGroup"]["keyGroup"] = json!(0);
        }
        self.retain_mapping_app_recording(slot);
        cx.emit(KeyboardProductChanged);
        cx.notify();
    }
    pub(super) fn init_mapping_keyboard_recording(&mut self, slot: usize, cx: &mut Context<Self>) {
        self.analog_gamepad.mapping.keyboard.stop();
        self.analog_gamepad.mapping.keyboard.focus[slot] = Some(cx.focus_handle());
    }
    pub(super) fn mapping_keyboard_recording_active(&self, slot: usize) -> bool {
        self.analog_gamepad.mapping.keyboard.recording == Some(slot)
    }
    pub(super) fn mapping_keyboard_recording_element(
        &self,
        slot: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mapping edit");
        let state = &self.analog_gamepad.mapping.keyboard;
        let group = if slot >= 2 {
            dynamic::recording_group(self, slot).expect("mounted Dynamic recording action")
        } else if edit.slots[slot].category == "APP_SPECIFIC" {
            app_specific::recording_group(self, slot)
        } else {
            &edit.slots[slot].staged["keyboardGroup"]
        };
        let mut ids = group["modifiers"].as_array().cloned().unwrap_or_default();
        if group["key"].as_str().is_some_and(|key| !key.is_empty()) {
            ids.push(group["key"].clone());
        }
        let display = ids
            .iter()
            .filter_map(Value::as_str)
            .map(|id| self.recording_key_name(id))
            .filter(|name| !name.is_empty())
            .collect::<Vec<_>>()
            .join(" + ");
        let recording = state.recording == Some(slot);
        let empty = display.is_empty();
        let focus = state.focus[slot].as_ref().expect("keyboard focus");
        div()
            .id(SharedString::from(format!("keyboard-map-recording-{slot}")))
            .relative()
            .w_full()
            .min_h(surface::css(27.))
            .p(surface::css(5.))
            .border_1()
            .border_color(rgb(if recording { 0x44d62c } else { 0x5d5d5d }))
            .cursor_pointer()
            .track_focus(focus)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.start_mapping_keyboard_recording(slot, window, cx)
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.analog_gamepad.mapping.keyboard.stop();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .when(empty, |text| text.text_color(rgb(0x707070)))
                    .child(if empty {
                        if recording {
                            recording_placeholder()
                        } else {
                            t("TEXT_CLICK_TO_RECORD")
                        }
                    } else {
                        display
                    }),
            )
            .when(!empty, |field| {
                field.child(
                    BaseButton::new(SharedString::from(format!(
                        "keyboard-map-recording-clear-{slot}"
                    )))
                    .size(surface::css(16.))
                    .absolute()
                    .right(surface::css(5.))
                    .top(surface::css(5.))
                    .overflow_hidden()
                    .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                        this.analog_gamepad.mapping.keyboard.clear_hover[slot] = *on;
                        cx.notify();
                    }))
                    .child(
                        img(if state.clear_hover[slot] {
                            "synapse/keyboard-mapping-recording/clear-hover.svg"
                        } else {
                            "synapse/keyboard-mapping-recording/clear.svg"
                        })
                        .size(surface::css(20.))
                        .absolute()
                        .left(surface::css(-2.))
                        .top(surface::css(-2.)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        if slot >= 2 {
                            this.change_mapping_dynamic_recording(slot, String::new(), vec![], cx);
                            return;
                        }
                        if this
                            .analog_gamepad
                            .mapping
                            .edit
                            .as_ref()
                            .is_some_and(|edit| edit.slots[slot].category == "APP_SPECIFIC")
                        {
                            this.clear_mapping_app_recording(slot, cx);
                            return;
                        }
                        if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                            edit.slots[slot].staged["keyboardGroup"]["key"] = json!("");
                            edit.slots[slot].staged["keyboardGroup"]["modifiers"] = json!([]);
                        }
                        this.retain_mapping_app_recording(slot);
                        cx.emit(KeyboardProductChanged);
                        cx.notify();
                    })),
                )
            })
            .into_any_element()
    }
    pub(super) fn mapping_keyboard_element(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mapping edit");
        let state = &self.analog_gamepad.mapping.keyboard;
        let source = source(self.spec.product_id);
        let mapping = &edit.slots[slot];
        let group = &mapping.staged["keyboardGroup"];
        let mut panel = v_flex().mx(surface::css(20.)).gap(surface::css(10.));
        if let Some(groups) = &state.groups[slot] {
            panel = panel.child(
                Select::new(groups)
                    .w(surface::css(210.))
                    .h(surface::css(27.))
                    .menu_max_h(surface::css(180.))
                    .placeholder(""),
            );
        }
        if mapping.key_group == 0 {
            panel = panel.child(self.mapping_keyboard_recording_element(slot, cx));
        } else if let Some(targets) = &state.targets[slot] {
            panel = panel.child(
                Select::new(targets)
                    .w(surface::css(210.))
                    .h(surface::css(27.))
                    .menu_max_h(surface::css(180.))
                    .placeholder(""),
            );
        }
        if !matches!(mapping.key_group, 0 | 5) && edit.button["inputType"] != "ControllerInput" {
            panel = panel.child(
                h_flex()
                    .child(
                        Checkbox::new(SharedString::from(format!(
                            "keyboard-map-include-modifier-{slot}"
                        )))
                        .label(t("TEXT_INCLUDE_MODIFIER"))
                        .checked(mapping.modifier_included)
                        .on_click(cx.listener(
                            move |this, enabled: &bool, _, cx| {
                                if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                    edit.slots[slot].modifier_included = *enabled;
                                    if !*enabled {
                                        edit.slots[slot].staged["keyboardGroup"]["modifiers"] =
                                            json!([]);
                                    }
                                }
                                cx.emit(KeyboardProductChanged);
                                cx.notify();
                            },
                        )),
                    )
                    .child(surface::help_control(
                        SharedString::from(format!("keyboard-map-include-modifier-help-{slot}")),
                        t("MAPPING_ADD_MODIFIER_TIP"),
                    )),
            );
            if mapping.modifier_included {
                for (label, code) in [
                    ("SHIFT", "16"),
                    ("CTRL", "17"),
                    ("ALT", "18"),
                    ("WIN", "91"),
                ] {
                    let mut row = h_flex().gap(surface::css(5.));
                    for flag in [0u64, 2] {
                        let Some(key) = source
                            .key_targets
                            .iter()
                            .find(|key| key["keyCode"] == code && key["flag"] == flag)
                        else {
                            continue;
                        };
                        let id = text(&key["inputID"]);
                        let opposite = source
                            .key_targets
                            .iter()
                            .find(|key| key["keyCode"] == code && key["flag"] != flag)
                            .map(|key| text(&key["inputID"]));
                        let selected = group["modifiers"]
                            .as_array()
                            .is_some_and(|rows| rows.iter().any(|row| row == &id));
                        row = row.child(
                            Button::new(SharedString::from(format!(
                                "keyboard-map-modifier-{slot}-{id}"
                            )))
                            .label(label)
                            .outline()
                            .selected(selected)
                            .flex_1()
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                        let mut modifiers =
                                            edit.slots[slot].staged["keyboardGroup"]["modifiers"]
                                                .as_array()
                                                .cloned()
                                                .unwrap_or_default();
                                        modifiers.retain(|modifier| {
                                            modifier != &id
                                                && (selected
                                                    || opposite
                                                        .as_deref()
                                                        .is_none_or(|other| modifier != other))
                                        });
                                        if !selected {
                                            modifiers.push(json!(id));
                                        }
                                        edit.slots[slot].staged["keyboardGroup"]["modifiers"] =
                                            json!(modifiers);
                                    }
                                    cx.emit(KeyboardProductChanged);
                                    cx.notify();
                                },
                            )),
                        );
                    }
                    panel = panel.child(row);
                }
            }
        }
        panel.into_any_element()
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_recording_data.
fn read_keyboard_mapping_recording_catalog() -> Vec<RecordingSource> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<RecordingSource>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<RecordingSource> {
        let value = match r.tag()? {
            0 => RecordingSource {
                product_id: r.child(decode_2)?,
                fn_maps: r.child(decode_3)?,
                mini_products: r.child(decode_6)?,
                key_names: r.child(decode_7)?,
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
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<u32>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, serde_json::Value>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_8)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, serde_json::Value>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_9)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_RECORDING_DATA,
        0,
        "c63c5556f404a8b25c7c7ff0b1301f7852527deeb96681e3a2fd914725737731",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_keyboard_mapping_recording_data_decodes() {
    let _ = read_keyboard_mapping_recording_catalog();
}
// End generated catalog reader: razer-pages/keyboard_mapping_recording_data.

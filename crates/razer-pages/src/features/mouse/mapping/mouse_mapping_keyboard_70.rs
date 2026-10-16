//! PID70 MapKeyboard.d6edead1 factory6223; main6114's complete144-key UI
//! table; 2048.61e611c9 focused recorder. No vendor script is evaluated.
use super::super::Choice;
use super::*;
use crate::features::source_key::SourceKey;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    IndexPath,
    select::{SelectEvent, SelectState},
};
use razer_widgets::{
    keyboard_capture::{Capture, RawKey},
    source_slider::SourceSlider,
    stepper::{Stepper, StepperEvent},
    theme::MouseMappingColors,
};
use std::time::{Duration, Instant};

const GROUPS: &[(&str, &str)] = &[
    ("Key Recording", "KB_KEY_RECORDING"),
    ("Alphanumeric", "KB_ALPHANUMERIC"),
    ("Function", "KB_FUNCTION"),
    ("Numpad", "KB_NUMPAD"),
    ("Navigation", "KB_NAVIGATION"),
    ("Modifiers", "KB_MODIFIERS"),
    ("Symbols", "KB_SYMBOLS"),
];
fn keys() -> &'static [SourceKey] {
    static KEYS: OnceLock<Vec<SourceKey>> = OnceLock::new();
    KEYS.get_or_init(|| crate::data::mouse_mapping_keyboard_70_data!())
}
fn key(id: &str) -> Option<&'static SourceKey> {
    keys()
        .iter()
        .find(|key| key.input_id.as_deref() == Some(id))
}
fn modifier(id: &str) -> bool {
    matches!(
        id,
        "KEY_LEFT_CTRL"
            | "KEY_RIGHT_CTRL"
            | "KEY_LEFT_SHIFT"
            | "KEY_RIGHT_SHIFT"
            | "KEY_LEFT_ALT"
            | "KEY_RIGHT_ALT"
            | "KEY_LEFT_GUI"
    )
}
fn choices(group: usize) -> Vec<Choice> {
    keys()
        .iter()
        .enumerate()
        .filter(|(_, key)| key.kind == GROUPS[group].0)
        .map(|(i, key)| Choice::new(i.to_string(), key.name.clone()))
        .collect()
}

// Source2048 W table for the other attached mini keyboards. PID70 itself
// has no AnalogGenVersion/isMiniKeyboard/redirectKeyInput branch.
fn mini_fn_key(pid: u64, id: &str) -> Option<&'static str> {
    let common = match id {
        "KEY_1" => Some("KEY_F1"),
        "KEY_2" => Some("KEY_F2"),
        "KEY_3" => Some("KEY_F3"),
        "KEY_4" => Some("KEY_F4"),
        "KEY_5" => Some("KEY_F5"),
        "KEY_6" => Some("KEY_F6"),
        "KEY_7" => Some("KEY_F7"),
        "KEY_8" => Some("KEY_F8"),
        "KEY_9" => Some("KEY_F9"),
        "KEY_0" => Some("KEY_F10"),
        "KEY_HYPEN" => Some("KEY_F11"),
        "KEY_EQUAL" => Some("KEY_F12"),
        _ => None,
    };
    common.or_else(|| match pid {
        599 | 617 | 642 => match id {
            "KEY_ESC" => Some("KEY_TILDE"),
            "KEY_BACKSPACE" => Some("KEY_DELETE"),
            "KEY_I" => Some("KEY_UP_ARROW"),
            "KEY_O" => Some("KEY_SCROLL_LOCK"),
            "KEY_P" => Some("KEY_PRINT_SCREEN"),
            "KEY_OPEN_SQUARE_BRACKET" => Some("KEY_PAGE_UP"),
            "KEY_CLOSE_SQUARE_BRACKET" => Some("KEY_HOME"),
            "KEY_J" => Some("KEY_LEFT_ARROW"),
            "KEY_K" => Some("KEY_DOWN_ARROW"),
            "KEY_L" => Some("KEY_RIGHT_ARROW"),
            "KEY_SEMICOLON" => Some("KEY_PAGE_DOWN"),
            "KEY_APOSTROPHE" => Some("KEY_END"),
            "KEY_PERIOD" => Some("KEY_PAUSE"),
            "KEY_SLASH" => Some("KEY_INSERT"),
            _ => None,
        },
        600 | 625 | 626 | 697 | 698 | 699 => match id {
            "KEY_O" => Some("KEY_PRINT_SCREEN"),
            "KEY_P" => Some("KEY_SCROLL_LOCK"),
            "KEY_OPEN_SQUARE_BRACKET" => Some("KEY_PAUSE"),
            "KEY_APOSTROPHE" => Some("KEY_HOME"),
            "KEY_SLASH" => Some("KEY_END"),
            _ => None,
        },
        688 => match id {
            "KEY_ESC" => Some("KEY_TILDE"),
            "KEY_BACKSPACE" => Some("KEY_DELETE"),
            "KEY_Y" => Some("KEY_INSERT"),
            "KEY_P" => Some("KEY_PRINT_SCREEN"),
            "KEY_H" => Some("KEY_HOME"),
            "KEY_J" => Some("KEY_PAGE_UP"),
            "KEY_N" => Some("KEY_END"),
            "KEY_M" => Some("KEY_PAGE_DOWN"),
            "KEY_RIGHT_SHIFT" => Some("KEY_UP_ARROW"),
            "KEY_APPLICATION" => Some("KEY_DOWN_ARROW"),
            "KEY_RIGHT_ALT" => Some("KEY_LEFT_ARROW"),
            "KEY_RIGHT_CTRL" => Some("KEY_RIGHT_ARROW"),
            _ => None,
        },
        _ => None,
    })
}

#[derive(Default)]
pub(super) struct State {
    group: usize,
    selected: Option<usize>,
    key: Option<String>,
    modifiers: Vec<String>,
    include_modifiers: bool,
    key_value: String,
    system_layout: Option<u8>,
    groups: Option<Entity<SelectState<Vec<Choice>>>>,
    targets: Option<Entity<SelectState<Vec<Choice>>>>,
    turbo: bool,
    disable_turbo: bool,
    speed: Value,
    stepper: Option<Entity<Stepper>>,
    slider: Option<Entity<SliderState>>,
    syncing: bool,
    clear_hover: bool,
    focus: Option<FocusHandle>,
    capture: Option<Capture>,
    native_capture: Option<super::super::mapping_key_capture_runtime::Owner>,
    task: Option<Task<()>>,
    pressed: Vec<String>,
    held_modifiers: Vec<String>,
    released: Vec<String>,
    combination: Option<String>,
    reset_at: Option<Instant>,
    native_fn: bool,
    native_down: BTreeMap<u64, SourceKey>,
}
impl State {
    pub(super) fn stop(&mut self) {
        self.capture = None;
        self.native_capture = None;
        self.task = None;
        self.pressed.clear();
        self.held_modifiers.clear();
        self.released.clear();
        self.combination = None;
        self.reset_at = None;
        self.native_fn = false;
        self.native_down.clear();
    }
}

impl MouseProductWorkspace {
    pub(super) fn initialize_mouse_keyboard_70(
        &mut self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let original = &button["assignmentGroup"];
        let mapped = button["assignment"] == "KEYBOARD_FUNCTION";
        let group = if mapped {
            original["keyGroup"].as_u64().unwrap_or(0).min(6) as usize
        } else {
            0
        };
        let id = mapped.then(|| original["key"].as_str()).flatten();
        let modifiers = if mapped {
            original["modifiers"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|id| id.as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let selected = if group == 0 {
            None
        } else {
            let candidates = keys()
                .iter()
                .enumerate()
                .filter(|(_, key)| key.kind == GROUPS[group].0);
            candidates
                .clone()
                .find(|(_, candidate)| {
                    if group == 6 && !modifiers.is_empty() {
                        id.and_then(key)
                            .is_some_and(|key| key.key_code == candidate.key_code)
                    } else {
                        candidate.input_id.as_deref() == id
                    }
                })
                .or_else(|| candidates.into_iter().next())
                .map(|(index, _)| index)
        };
        let speed = if mapped {
            original["turboMode"]
                .get("keysPerSecond")
                .cloned()
                .unwrap_or(json!(7))
        } else {
            json!(7)
        };
        let numeric = speed
            .as_f64()
            .or_else(|| speed.as_str().and_then(|v| v.parse().ok()))
            .unwrap_or(7.)
            .clamp(1., 20.);
        let key_value = if mapped {
            button["assignmentValue"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        } else {
            String::new()
        };
        let groups = cx.new(|cx| {
            SelectState::new(
                GROUPS
                    .iter()
                    .enumerate()
                    .map(|(i, (_, label))| Choice::localized(i.to_string(), *label))
                    .collect::<Vec<_>>(),
                Some(IndexPath::new(group)),
                window,
                cx,
            )
        });
        let target_choices = choices(group);
        let target_index = selected.and_then(|index| {
            target_choices
                .iter()
                .position(|choice| choice.id() == index.to_string())
        });
        let targets = cx.new(|cx| {
            SelectState::new(target_choices, target_index.map(IndexPath::new), window, cx)
        });
        let stepper = cx.new(|cx| {
            Stepper::new(
                "mouse70-keyboard-turbo",
                numeric,
                (1., 20., 1.),
                false,
                false,
                Some(2),
                window,
                cx,
            )
            .in_custom_keymapping()
            .live_update()
        });
        let slider = cx.new(|_| {
            SliderState::new()
                .min(1.)
                .max(20.)
                .step(1.)
                .default_value(numeric as f32)
        });
        let group_sub = cx.subscribe_in(&groups, window, |this, _, event, window, cx| {
            let SelectEvent::Confirm(Some(id)) = event else {
                return;
            };
            let Ok(group) = id.parse::<usize>() else {
                return;
            };
            if group >= GROUPS.len() {
                return;
            }
            let state = &mut this.mapping_current_state.keyboard;
            state.stop();
            state.group = group;
            state.key = None;
            state.key_value.clear();
            let items = choices(group);
            state.selected = items.first().and_then(|choice| choice.id().parse().ok());
            if let Some(target) = &state.targets {
                target.update(cx, |target, cx| {
                    target.set_items(items, window, cx);
                    target.set_selected_index(
                        (group != 0).then_some(IndexPath::new(0)),
                        window,
                        cx,
                    );
                });
            }
            this.mapping_current_state.can_save = group != 0;
            this.mapping_current_state.changed = group != 0;
            cx.notify();
        });
        let target_sub = cx.subscribe(&targets, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(id)) = event
                && let Ok(index) = id.parse::<usize>()
            {
                this.mapping_current_state.keyboard.selected = Some(index);
                this.mapping_current_state.can_save = true;
                this.mapping_current_state.changed = true;
                cx.notify();
            }
        });
        let stepper_sub = cx.subscribe_in(
            &stepper,
            window,
            |this, _, event: &StepperEvent, window, cx| {
                if this.mapping_current_state.keyboard.syncing {
                    return;
                }
                let value = event
                    .draft
                    .clone()
                    .map_or_else(|| json!(event.value), Value::String);
                this.change_mouse_keyboard_turbo_70(value, window, cx);
            },
        );
        let slider_sub = cx.subscribe_in(
            &slider,
            window,
            |this, _, event: &SliderEvent, window, cx| {
                if this.mapping_current_state.keyboard.syncing {
                    return;
                }
                if let SliderEvent::Release(value) = event {
                    this.change_mouse_keyboard_turbo_70(
                        json!(value.start().round() as u32),
                        window,
                        cx,
                    );
                }
            },
        );
        self.mapping_current_state.keyboard = State {
            group,
            selected,
            key: id.map(str::to_owned),
            include_modifiers: !modifiers.is_empty(),
            modifiers,
            key_value,
            system_layout: match razer_platform::keyboard_layout::get() {
                Ok(klid) => Some(razer_device::keyboard_layout::system_layout(klid)),
                Err(error) => {
                    eprintln!("[mouse70-keyboard] System keyboard layout: {error:#}");
                    None
                }
            },
            disable_turbo: matches!(
                button["buttonKey"].as_str(),
                Some(
                    "ScrollUp"
                        | "ScrollDown"
                        | "CycleUpSensitivityStages"
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
            ) || button["inputType"] == "ControllerInput"
                || button["disableTurboInAssignment"]
                    .as_array()
                    .is_some_and(|values| values.iter().any(|value| value == "KEYBOARD_FUNCTION")),
            groups: Some(groups),
            targets: Some(targets),
            turbo: mapped && original["turboMode"]["isTurbo"] == true,
            speed,
            stepper: Some(stepper),
            slider: Some(slider),
            focus: Some(cx.focus_handle()),
            ..Default::default()
        };
        self.mapping_current_state.subscriptions.extend([
            group_sub,
            target_sub,
            stepper_sub,
            slider_sub,
        ]);
        self.mapping_current_state.can_save = false;
        self.mapping_current_state.changed = false;
        self.mapping_current_state.basic_requires_synapse = false;
    }

    fn change_mouse_keyboard_turbo_70(
        &mut self,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.mapping_current_state.keyboard;
        if state.speed == value {
            return;
        }
        state.speed = value.clone();
        state.syncing = true;
        let numeric = value
            .as_f64()
            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
            .unwrap_or(0.)
            .clamp(1., 20.);
        if !value.is_string()
            && let Some(stepper) = &state.stepper
        {
            stepper.update(cx, |stepper, cx| {
                stepper.sync_value(numeric, 1., false, window, cx)
            });
        }
        if let Some(slider) = &state.slider {
            slider.update(cx, |slider, cx| {
                slider.set_value(numeric as f32, window, cx)
            });
        }
        state.syncing = false;
        if !state.key_value.is_empty() {
            self.mapping_current_state.can_save = true;
            self.mapping_current_state.changed = true;
        }
        cx.notify();
    }

    pub(super) fn mouse_keyboard_70_payload(&self, button: &Value) -> Option<Vec<Value>> {
        let state = &self.mapping_current_state.keyboard;
        let mut modifiers = if state.group == 5 {
            Vec::new()
        } else {
            state.modifiers.clone()
        };
        let selected = if state.group == 0 {
            state.key.as_deref().and_then(key)?
        } else {
            keys().get(state.selected?)?
        };
        let selected = if let Some(code) = selected.modifier.as_deref() {
            let modifier = keys().iter().find(|key| key.key_code == code)?;
            if !modifiers
                .iter()
                .any(|id| key(id).is_some_and(|key| key.key_code == modifier.key_code))
            {
                modifiers.push(modifier.input_id.clone()?);
            }
            keys()
                .iter()
                .find(|key| key.key_code == selected.key_code)?
        } else {
            selected
        };
        let mut group =
            json!({"keyGroup":state.group,"key":selected.input_id,"modifiers":modifiers});
        if state.turbo {
            group["turboMode"] = json!({"isTurbo":true,"keysPerSecond":state.speed});
        }
        Some(vec![
            json!({"inputID":button["inputID"],"inputType":button["inputType"],"isHyperShift":self.hypershift,"outputType":"keyboardGroup","keyboardGroup":group}),
        ])
    }

    fn start_mouse_keyboard_recording_70(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.mapping_current_state.keyboard;
        if state.capture.is_some() || state.group != 0 {
            return;
        }
        let Some(capture) = Capture::start_stream(window) else {
            eprintln!("[mouse70-keyboard] Focused keyboard capture could not start");
            return;
        };
        state.capture = Some(capture);
        match super::super::mapping_key_capture_runtime::Owner::start() {
            Ok(owner) => state.native_capture = Some(owner),
            Err(error) => eprintln!("[mouse70-keyboard] Native capture startup: {error:#}"),
        }
        if let Some(focus) = &state.focus {
            focus.focus(window, cx);
        }
        let input = self.mapping_input.clone();
        state.task = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let keep = view
                    .update_in(cx, |view, window, cx| {
                        if view.mapping_input != input
                            || view.mapping_current_state.function != "KEYBOARD_FUNCTION"
                            || !view.active
                            || view.page != "TAB_CUSTOMIZE"
                        {
                            view.mapping_current_state.keyboard.stop();
                            return false;
                        }
                        let state = &mut view.mapping_current_state.keyboard;
                        if !window.is_window_active()
                            || !state
                                .focus
                                .as_ref()
                                .is_some_and(|focus| focus.is_focused(window))
                        {
                            state.stop();
                            cx.notify();
                            return false;
                        }
                        let Some(events) = state.capture.as_mut().and_then(Capture::take_all)
                        else {
                            state.stop();
                            cx.notify();
                            return false;
                        };
                        let native = state.native_capture.as_ref().map(|owner| owner.drain()).unwrap_or_default();
                        for event in events {
                            view.consume_mouse_keyboard_70(&event, cx);
                        }
                        for reply in native {
                            use super::super::mapping_key_capture_runtime::Reply;
                            match reply {
                                Reply::Ready(receipt) => {
                                    if receipt["unresolved_devices"].as_array().is_some_and(|items| !items.is_empty()) {
                                        eprintln!("[mouse70-keyboard] Native capture unresolved endpoints: {}", receipt["unresolved_devices"]);
                                    }
                                }
                                Reply::Failed(error) => {
                                    eprintln!("[mouse70-keyboard] Native capture: {error}");
                                    view.mapping_current_state.keyboard.native_capture = None;
                                }
                                Reply::Events(events) => for event in events {
                                    view.consume_native_mouse_keyboard_70(&event, cx);
                                },
                            }
                        }
                        let state = &mut view.mapping_current_state.keyboard;
                        if state.reset_at.is_some_and(|at| Instant::now() >= at) {
                            state.held_modifiers.clear();
                            state.reset_at = None;
                        }
                        state.capture.is_some()
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn consume_mouse_keyboard_70(&mut self, event: &RawKey, cx: &mut Context<Self>) {
        // Match the source browser fallback's keyCode/explicit code aliases.
        // The focused capture preserves the event stream without global hooks.
        let alias = match event.code() {
            0x91 => Some("KEY_SCROLL_LOCK"),
            0x13 => Some("KEY_PAUSE"),
            0x90 => Some("KEY_NUMPAD_NUM_LOCK"),
            _ => None,
        };
        let Some(target) = keys().iter().find(|key| {
            alias
                .map(|id| key.input_id.as_deref() == Some(id))
                .unwrap_or_else(|| key.key_code == event.code().to_string())
        }) else {
            return;
        };
        let Some(id) = target.input_id.as_deref() else {
            return;
        };
        self.consume_mouse_keyboard_key_70(id, event.down(), cx);
    }

    fn consume_native_mouse_keyboard_70(&mut self, event: &Value, cx: &mut Context<Self>) {
        let mut input = event["input"].clone();
        if input["type"] == "razerKey" {
            self.mapping_current_state.keyboard.native_fn = input["flag"].as_u64() == Some(0);
            return;
        }
        if !matches!(input["type"].as_str(), Some("keyboard" | "analogKey")) {
            return;
        }
        let Some(mut flag) = input["flag"].as_u64() else {
            return;
        };
        let pid = event["deviceInfo"]["productId"]
            .as_u64()
            .unwrap_or_default();
        if [688, 642, 697, 698, 699, 599, 617, 600, 625, 626].contains(&pid)
            && self.mapping_current_state.keyboard.native_fn
        {
            if let Some(original) = keys()
                .iter()
                .find(|key| {
                    key.input_id.as_deref() != Some("KEY_NON_US_POUND")
                        && Some(key.scancode) == input["scancode"].as_u64()
                        && key.output_flag.unwrap_or(0) == 0
                })
                .and_then(|key| key.input_id.as_deref())
            {
                if let Some(mapped) = mini_fn_key(pid, original).and_then(key) {
                    input["scancode"] = json!(mapped.scancode);
                    flag += mapped.output_flag.unwrap_or(0);
                }
            }
        }
        let Some(target) = keys().iter().find(|key| {
            // PID70 has no keyboard layoutId. Source G excludes non-US pound;
            // system layout controls displayed names independently.
            if key.input_id.as_deref() == Some("KEY_NON_US_POUND") {
                return false;
            }
            let base = key.output_flag.unwrap_or(key.flag);
            Some(key.scancode) == input["scancode"].as_u64() && (base == flag || base + 1 == flag)
        }) else {
            return;
        };
        let Some(id) = target.input_id.as_deref() else {
            return;
        };
        let state = &mut self.mapping_current_state.keyboard;
        if flag % 2 == 0 {
            state.native_down.insert(target.scancode, target.clone());
            if state
                .native_down
                .get(&29)
                .is_some_and(|key| key.output_flag == Some(4))
                && state.native_down.contains_key(&69)
            {
                state.native_down.clear();
                return;
            }
        } else {
            state.native_down.clear();
        }
        self.consume_mouse_keyboard_key_70(id, flag % 2 == 0, cx);
    }

    fn consume_mouse_keyboard_key_70(&mut self, id: &str, down: bool, cx: &mut Context<Self>) {
        let state = &mut self.mapping_current_state.keyboard;
        if down {
            if !state.pressed.iter().any(|value| value == id) {
                state.pressed.push(id.to_owned());
            }
            if modifier(id) && !state.held_modifiers.iter().any(|value| value == id) {
                state.held_modifiers.push(id.to_owned());
            }
            return;
        }
        if !state.pressed.iter().any(|value| value == id) {
            return;
        }
        state.pressed.retain(|value| value != id);
        let selected = if modifier(id) {
            if !state.released.iter().any(|value| value == id) {
                state.released.push(id.to_owned());
            }
            if state.held_modifiers.len() != state.released.len() || state.held_modifiers.is_empty()
            {
                return;
            }
            let selected = if state.combination.take().is_some() {
                None
            } else {
                Some(state.held_modifiers.clone())
            };
            state.released.clear();
            state.reset_at = Some(
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
            let mut values = state.held_modifiers.clone();
            if !values.is_empty() && state.pressed.iter().any(|id| values.contains(id)) {
                state.combination = Some(id.to_owned());
            }
            values.push(id.to_owned());
            Some(values)
        };
        let Some(mut ids) = selected else {
            return;
        };
        for id in &mut ids {
            if id == "KEY_CANCEL" {
                *id = "KEY_PAUSE".into();
            }
        }
        let Some(last) = ids.pop() else {
            return;
        };
        state.key = Some(last.clone());
        state.modifiers = ids;
        state.key_value = last;
        self.mapping_current_state.changed = true;
        self.mapping_current_state.can_save = true;
        cx.notify();
    }

    pub(super) fn mouse_keyboard_70_body(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = &self.mapping_current_state.keyboard;
        let mut body = v_flex().mb(surface::css(20.));
        if let Some(groups) = &state.groups {
            body = body.child(
                razer_widgets::synapse_select::select(groups)
                    .items(
                        GROUPS
                            .iter()
                            .enumerate()
                            .map(|(i, (_, label))| Choice::localized(i.to_string(), *label))
                            .collect(),
                    )
                    .mb(surface::css(10.))
                    .w_full(),
            );
        }
        if state.group == 0 {
            let recording = state.capture.is_some();
            let label = state
                .modifiers
                .iter()
                .map(String::as_str)
                .chain(state.key.as_deref())
                .map(|id| razer_device::keyboard_layout::key_name(state.system_layout, id))
                .collect::<Vec<_>>()
                .join(" + ");
            let label = if label.is_empty() {
                if recording {
                    {
                        let translated = t("REBIND_KEY_TO");
                        let mut chars = translated.chars();
                        format!(
                            "{}{}...",
                            chars.next().unwrap_or_default(),
                            chars.as_str().to_lowercase()
                        )
                    }
                } else {
                    t("TEXT_CLICK_TO_RECORD")
                }
            } else {
                label
            };
            let mut recorder = div()
                .id("mouse70-key-recorder")
                .relative()
                .min_h(surface::css(27.))
                .mb(surface::css(10.))
                .border_1()
                .border_color(if recording {
                    MouseMappingColors::selected_border()
                } else {
                    MouseMappingColors::recording_border()
                })
                .p(surface::css(5.))
                .cursor_pointer()
                .child(div().pr(surface::css(20.)).child(label))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.start_mouse_keyboard_recording_70(window, cx)
                }))
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.mapping_current_state.keyboard.stop();
                    cx.notify();
                }));
            if let Some(focus) = &state.focus {
                recorder = recorder.track_focus(focus);
            }
            if state.key.is_some() {
                recorder = recorder.child(
                    BaseButton::new("mouse70-keyboard-clear")
                        .absolute()
                        .right(surface::css(5.))
                        .top(surface::css(5.))
                        .size(surface::css(16.))
                        .p_0()
                        .overflow_hidden()
                        .on_hover(cx.listener(|this, on, _, cx| {
                            this.mapping_current_state.keyboard.clear_hover = *on;
                            cx.notify();
                        }))
                        .child(
                            img(if state.clear_hover {
                                "synapse/keyboard-mapping-recording/clear-hover.svg"
                            } else {
                                "synapse/keyboard-mapping-recording/clear.svg"
                            })
                            .absolute()
                            .left(surface::css(-2.))
                            .top(surface::css(-2.))
                            .size(surface::css(20.)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            let state = &mut this.mapping_current_state.keyboard;
                            state.key = None;
                            state.modifiers.clear();
                            state.key_value.clear();
                            this.mapping_current_state.can_save = false;
                            this.mapping_current_state.changed = true;
                            cx.notify();
                        })),
                );
            }
            body = body.child(recorder);
        } else if let Some(targets) = &state.targets {
            body = body.child(
                razer_widgets::synapse_select::select(targets)
                    .items(choices(state.group))
                    .mb(surface::css(10.))
                    .w_full(),
            );
        }
        if !matches!(state.group, 0 | 5) {
            body = body.child(
                h_flex()
                    .gap(surface::css(5.))
                    .mb(surface::css(10.))
                    .child(
                        Checkbox::new("mouse70-keyboard-modifiers")
                            .label(t("TEXT_INCLUDE_MODIFIER"))
                            .checked(state.include_modifiers)
                            .on_click(cx.listener(|this, on, _, cx| {
                                let state = &mut this.mapping_current_state.keyboard;
                                state.include_modifiers = *on;
                                if !on {
                                    state.modifiers.clear();
                                }
                                this.mapping_current_state.changed = true;
                                this.mapping_current_state.can_save = true;
                                cx.notify();
                            })),
                    )
                    .child(surface::help_control(
                        "mouse70-keyboard-modifiers-tip",
                        t("MAPPING_ADD_MODIFIER_TIP"),
                    )),
            );
            if state.include_modifiers {
                for (name, left, right) in [
                    ("SHIFT", "KEY_LEFT_SHIFT", Some("KEY_RIGHT_SHIFT")),
                    ("CTRL", "KEY_LEFT_CTRL", Some("KEY_RIGHT_CTRL")),
                    ("ALT", "KEY_LEFT_ALT", Some("KEY_RIGHT_ALT")),
                    ("WIN", "KEY_LEFT_GUI", None),
                ] {
                    body = body.child(h_flex().justify_between().mb(surface::css(5.)).children(
                        std::iter::once(left).chain(right).map(|id| {
                            let selected = state.modifiers.iter().any(|value| value == id);
                            let other_selected = state.modifiers.iter().any(|value| {
                                (value == left || Some(value.as_str()) == right) && value != id
                            });
                            BaseButton::new(SharedString::from(format!("mouse70-keyboard-{id}")))
                                .px(surface::css(16.))
                                .py(surface::css(7.))
                                .text_size(surface::css(12.))
                                .bg(MouseMappingColors::modifier_background())
                                .border_1()
                                .border_color(if selected {
                                    MouseMappingColors::selected_border()
                                } else {
                                    MouseMappingColors::modifier_border()
                                })
                                .rounded(surface::css(3.))
                                .opacity(if other_selected { 0.3 } else { 1. })
                                .hover(|style| {
                                    style
                                        .bg(MouseMappingColors::modifier_hover())
                                        .border_color(MouseMappingColors::selected_border())
                                })
                                .selected(selected)
                                .child(name)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let state = &mut this.mapping_current_state.keyboard;
                                    let selected = state.modifiers.iter().any(|value| value == id);
                                    state.modifiers.retain(|value| {
                                        value != left && Some(value.as_str()) != right
                                    });
                                    if !selected {
                                        state.modifiers.push(id.to_owned());
                                    }
                                    this.mapping_current_state.changed = true;
                                    this.mapping_current_state.can_save = true;
                                    cx.notify();
                                }))
                        }),
                    ));
                }
            }
        }
        if !state.disable_turbo {
            body = body.child(
                h_flex()
                    .gap(surface::css(5.))
                    .mb(surface::css(10.))
                    .child(
                        Checkbox::new("mouse70-keyboard-turbo-enabled")
                            .label(t("ENABLE_TURBO"))
                            .checked(state.turbo)
                            .on_click(cx.listener(|this, on, _, cx| {
                                let state = &mut this.mapping_current_state.keyboard;
                                state.turbo = *on;
                                if !state.key_value.is_empty() {
                                    this.mapping_current_state.changed = true;
                                    this.mapping_current_state.can_save = true;
                                }
                                cx.notify();
                            })),
                    )
                    .child(surface::help_control(
                        "mouse70-keyboard-turbo-tip",
                        t("MAPPING_TURBO_CONTROL_TIP"),
                    )),
            );
            if state.turbo {
                body = body
                    .child(div().mb(surface::css(5.)).child(t("SPEED_PER_SEC")))
                    .children(state.stepper.clone());
                if let Some(slider) = &state.slider {
                    body = body
                        .child(SourceSlider::new(
                            slider,
                            (slider.read(cx).value().start() - 1.) / 19.,
                        ))
                        .child(h_flex().justify_between().child("1").child("20"));
                }
            }
        }
        let _ = window;
        body.into_any_element()
    }
}

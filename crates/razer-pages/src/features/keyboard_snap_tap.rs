//! Current 515 ordinary Customize → $l → Jl/ql. Local drafts only.
//! Reducer, input tables, labels and CSS receipts: snap-tap-current-evidence.json.
use super::*;
use gpui_kit::base::{Button as BaseButton, Dialog};
use razer_widgets::theme::SnapTapColors as Colors;
use serde::Serialize;
use std::{cell::Cell, rc::Rc, time::Duration};

const LOCAL: &str = "_snapTapLocalV1";

#[derive(Clone, Deserialize, Serialize)]
struct Pair {
    key1: String,
    key2: String,
    id: usize,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}
impl Pair {
    fn complete(&self) -> bool {
        !self.key1.is_empty() && !self.key2.is_empty()
    }
}
#[derive(Clone, Deserialize, Serialize)]
struct Configuration {
    #[serde(rename = "isEnabled")]
    enabled: bool,
    #[serde(rename = "keyList")]
    pairs: Vec<Pair>,
}
#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Ready,
    Key1,
    Key2,
}
#[derive(Clone, Copy, PartialEq)]
enum Message {
    Introduction,
    Duplicate,
    Success,
}
impl Message {
    fn label(self) -> &'static str {
        match self {
            Self::Introduction => "SNAP_TAP_INTRODUCE_TEXT",
            Self::Duplicate => "CREATE_SNAP_TAP_WARNING_MESSAGE",
            Self::Success => "CREATE_SNAP_TAP_SUCCESS_MESSAGE",
        }
    }
}
#[derive(Deserialize)]
struct SnapTapData {
    defaults: Configuration,
    forbidden: Vec<String>,
    inputs: Vec<Value>,
    razer_keys: Vec<Value>,
    layouts: Value,
}
fn data() -> &'static SnapTapData {
    static DATA: OnceLock<SnapTapData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_snap_tap_data.json"))
            .expect("audited Snap Tap data")
    })
}

/// Only a real middleware adapter may publish these observations. No adapter is
/// connected yet; native keystrokes are a separate local editing input channel.
#[derive(Clone)]
#[allow(dead_code)]
pub enum SnapTapObservation {
    Configuration(Value),
    AdjustmentMode(bool),
    Layout(u32),
    InputRedirect(Value),
}

pub(super) struct State {
    config: Configuration,
    staged: Vec<Pair>,
    phase: Phase,
    editing: Option<usize>,
    message: Message,
    pause: bool,
    local: bool,
    layout: Option<u32>,
    adjustment: Option<bool>,
    prompt: bool,
    focus: FocusHandle,
    prompt_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    capture_generation: u64,
    success_timer: Option<Task<()>>,
    pair_bounds: Rc<Cell<Bounds<Pixels>>>,
    add_bounds: Rc<Cell<Bounds<Pixels>>>,
    tooltip: Option<Point<Pixels>>,
}
impl State {
    pub(super) fn new(cx: &mut App) -> Self {
        let config = data().defaults.clone();
        Self {
            staged: config.pairs.clone(),
            config,
            phase: Phase::Ready,
            editing: None,
            message: Message::Introduction,
            pause: false,
            local: false,
            layout: None,
            adjustment: None,
            prompt: false,
            focus: cx.focus_handle(),
            prompt_focus: cx.focus_handle(),
            return_focus: None,
            capture_generation: 0,
            success_timer: None,
            pair_bounds: Rc::new(Cell::new(Bounds::default())),
            add_bounds: Rc::new(Cell::new(Bounds::default())),
            tooltip: None,
        }
    }
    fn finish(&mut self) {
        self.phase = Phase::Ready;
        self.pause = false;
    }
    fn commit(&mut self, pairs: Vec<Pair>) {
        self.config.pairs = pairs.clone();
        self.staged = pairs;
        self.local = true;
    }
    fn begin(&mut self, id: usize, phase: Phase, window: &mut Window, cx: &mut App) {
        self.success_timer = None;
        self.pause = false;
        self.phase = phase;
        self.editing = Some(id);
        self.capture_generation = self.capture_generation.wrapping_add(1);
        self.tooltip = None;
        self.focus.focus(window, cx);
    }
    // ql beforeunload: preserve staged key1 only after advancing to KEY2;
    // incomplete new pairs never escape into local persisted configuration.
    fn snapshot(&self) -> Value {
        let mut config = self.config.clone();
        if self.phase == Phase::Key2 {
            if let Some(staged) = self.staged.iter().find(|p| Some(p.id) == self.editing) {
                if let Some(pair) = config.pairs.iter_mut().find(|p| p.id == staged.id) {
                    pair.key1 = staged.key1.clone();
                }
            }
        }
        config.pairs.retain(Pair::complete);
        serde_json::to_value(config).expect("local Snap Tap snapshot")
    }
    fn remove(&mut self, id: usize) {
        // Preserve the source callback's pre-update warning/list values.
        let rollback = !self.staged.iter().any(|p| !p.complete())
            && self
                .config
                .pairs
                .iter()
                .find(|p| p.id == id)
                .is_some_and(|p| p.key1 != p.key2)
            && self.message == Message::Duplicate;
        if self.editing == Some(id) {
            self.editing = None;
            self.message = Message::Introduction;
            self.finish();
        } else if let Some(editing) = &mut self.editing {
            if id < *editing {
                *editing -= 1;
            }
        }
        let pairs = if rollback {
            self.config.pairs.clone()
        } else {
            self.staged
                .iter()
                .filter(|p| p.id != id)
                .cloned()
                .enumerate()
                .map(|(i, mut p)| {
                    p.id = i + 1;
                    p
                })
                .collect()
        };
        self.commit(pairs);
    }
    fn blur(&mut self) {
        let pairs: Vec<_> = self
            .staged
            .iter()
            .filter(|p| {
                p.complete() && !(self.message == Message::Duplicate && Some(p.id) == self.editing)
            })
            .cloned()
            .collect();
        if self.staged.len() == 1 && pairs.is_empty() {
            self.staged = self.config.pairs.clone();
        } else {
            self.commit(pairs);
        }
        self.message = Message::Introduction;
        self.finish();
        self.tooltip = None;
    }
    fn accept(&mut self, input: &str) -> bool {
        if !self.config.enabled || self.phase == Phase::Ready {
            return false;
        }
        if self.pause && input == "KEY_NUMPAD_NUM_LOCK" {
            self.pause = false;
            return false;
        }
        self.pause = input == "KEY_PAUSE";
        let mut next = self.staged.clone();
        let Some(pair) = next.iter_mut().find(|p| Some(p.id) == self.editing) else {
            return false;
        };
        match self.phase {
            Phase::Key1 => {
                pair.key1 = input.into();
                self.message = Message::Introduction;
            }
            Phase::Key2 => pair.key2 = input.into(),
            Phase::Ready => return false,
        }
        if data().forbidden.iter().any(|v| v == input)
            || next
                .iter()
                .map(|p| usize::from(p.key1 == input) + usize::from(p.key2 == input))
                .sum::<usize>()
                > 1
        {
            self.message = Message::Duplicate;
            return false;
        }
        self.staged = next.clone();
        self.local = true;
        if self.phase == Phase::Key1 {
            self.phase = Phase::Key2;
            self.capture_generation = self.capture_generation.wrapping_add(1);
            false
        } else {
            self.commit(next);
            self.finish();
            self.message = Message::Success;
            true
        }
    }
    fn name(&self, input: &str, razer: &[Value]) -> String {
        if let Some(key) = razer.iter().find(|key| key["inputID"] == input) {
            return key["name"].as_str().unwrap_or(input).to_owned();
        }
        let layouts = &data().layouts;
        let layout = self.layout.map(|id| id.to_string());
        let table = layout
            .as_ref()
            .and_then(|id| layouts.get(id))
            .unwrap_or(&layouts["default"]);
        let name = table.get(input).or_else(|| layouts["default"].get(input));
        name.and_then(|n| {
            n.as_str()
                .or_else(|| n["KEYBOARD"].as_str())
                .or_else(|| n["DEFAULT"].as_str())
        })
        .unwrap_or(input)
        .to_owned()
    }
}

impl KeyboardProductWorkspace {
    pub fn captures_snap_keys(&self) -> bool {
        self.page == "TAB_CUSTOMIZE"
            && self
                .snap_tap
                .as_ref()
                .is_some_and(|s| s.config.enabled && s.phase != Phase::Ready && !s.prompt)
    }
    pub fn capture_snap_key_up(&mut self, event: &KeyUpEvent, cx: &mut Context<Self>) {
        if self.captures_snap_keys() {
            self.snap_key_up(event, cx);
        }
    }
    pub(super) fn init_snap_tap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id != 515 {
            return;
        }
        self.snap_tap = Some(State::new(cx));
        self.subscriptions
            .push(cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.blur_snap_tap(cx);
                }
            }));
    }
    pub(super) fn snap_snapshot(&self, snapshot: &mut Value) {
        if let Some(state) = &self.snap_tap {
            if state.local {
                snapshot[LOCAL] = state.snapshot();
            }
        }
    }
    pub(super) fn restore_snap_tap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(old) = &self.snap_tap else {
            return;
        };
        let (layout, adjustment) = (old.layout, old.adjustment);
        if old.prompt {
            if let Some(focus) = &old.return_focus {
                focus.focus(window, cx);
            }
        }
        let mut state = State::new(cx);
        state.layout = layout;
        state.adjustment = adjustment;
        if let Some(config) = self
            .draft
            .get(LOCAL)
            .and_then(|v| serde_json::from_value::<Configuration>(v.clone()).ok())
        {
            state.config = config;
            state.config.pairs.retain(Pair::complete);
            state.staged = state.config.pairs.clone();
            state.local = true;
        }
        self.snap_tap = Some(state);
    }
    fn publish_snap_tap(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = &self.snap_tap {
            if state.local {
                self.draft[LOCAL] = state.snapshot();
                cx.emit(KeyboardProductChanged);
            }
        }
        cx.notify();
    }
    pub(super) fn blur_snap_tap(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.snap_tap {
            state.tooltip = None;
            if state.config.enabled {
                if state.phase != Phase::Ready {
                    state.blur();
                } else {
                    // ql's window blur also clears SUCCESS while idle. An
                    // unchanged observed list does not create a local override.
                    state.message = Message::Introduction;
                    state.pause = false;
                }
            }
            self.publish_snap_tap(cx);
        }
    }
    pub fn leave_snap_tap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.snap_tap {
            if state.local {
                state.config = serde_json::from_value(state.snapshot()).unwrap();
                state.staged = state.config.pairs.clone();
            }
            state.finish();
            state.editing = None;
            state.message = Message::Introduction;
            state.success_timer = None;
            state.tooltip = None;
            if state.prompt {
                if let Some(focus) = &state.return_focus {
                    focus.focus(window, cx);
                }
            }
            state.prompt = false;
            self.publish_snap_tap(cx);
        }
    }
    fn toggle_snap_tap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        if state.adjustment == Some(true) {
            state.return_focus = window.focused(cx);
            state.prompt = true;
            state.prompt_focus.focus(window, cx);
            cx.notify();
            return;
        }
        state.config.enabled = !state.config.enabled;
        state.local = true;
        if !state.config.enabled {
            state.tooltip = None;
            if state.message == Message::Duplicate {
                if let Some(id) = state.editing {
                    state.remove(id);
                }
            }
        } else if state.phase != Phase::Ready {
            state.focus.focus(window, cx);
        }
        self.publish_snap_tap(cx);
    }
    fn add_snap_pair(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        if !state.config.enabled || state.phase != Phase::Ready || state.staged.len() >= 4 {
            return;
        }
        let id = state.config.pairs.len() + 1;
        state.config.pairs.push(Pair {
            key1: String::new(),
            key2: String::new(),
            id,
            extra: BTreeMap::new(),
        });
        state.staged = state.config.pairs.clone();
        state.local = true;
        state.begin(id, Phase::Key1, window, cx);
        self.publish_snap_tap(cx);
    }
    fn edit_snap_key(
        &mut self,
        id: usize,
        phase: Phase,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        if state.config.enabled && state.staged.iter().any(|p| p.id == id) {
            if state.phase == Phase::Ready {
                state.begin(id, phase, window, cx);
            } else {
                // Native focus may have moved after a rejected outside click.
                // Re-enter capture without changing the source recording phase.
                state.focus.focus(window, cx);
            }
            cx.notify();
        }
    }
    fn snap_input(&mut self, input: &str, cx: &mut Context<Self>) {
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        if state.accept(input) {
            state.success_timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                let _ = this.update(cx, |this, cx| {
                    if let Some(state) = &mut this.snap_tap {
                        state.message = Message::Introduction;
                    }
                    cx.notify();
                });
            }));
        }
        self.publish_snap_tap(cx);
    }
    #[allow(dead_code)]
    pub fn observe_snap_tap(
        &mut self,
        observation: SnapTapObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        match observation {
            SnapTapObservation::Configuration(value) => {
                // Observations do not silently overwrite an explicit local draft.
                if !state.local {
                    if let Ok(mut config) = serde_json::from_value::<Configuration>(value) {
                        config.pairs.retain(Pair::complete);
                        state.staged = config.pairs.clone();
                        state.config = config;
                        state.finish();
                    }
                }
            }
            SnapTapObservation::Layout(id) => state.layout = Some(id),
            SnapTapObservation::AdjustmentMode(running) => {
                state.adjustment = Some(running);
                if !running && state.prompt {
                    state.prompt = false;
                    if let Some(focus) = state.return_focus.take() {
                        focus.focus(window, cx);
                    }
                }
            }
            SnapTapObservation::InputRedirect(input) => {
                if self.page != "TAB_CUSTOMIZE"
                    || !state.config.enabled
                    || state.phase == Phase::Ready
                {
                    return;
                }
                let kind = input["type"].as_str().unwrap_or("");
                if !matches!(kind, "keyboard" | "analogKey" | "razerKey") {
                    return;
                }
                let Some(flag) = input["flag"].as_u64().filter(|f| f % 2 == 1) else {
                    return;
                };
                let key = data()
                    .inputs
                    .iter()
                    .find(|key| {
                        let base = key
                            .get("outputFlag")
                            .and_then(Value::as_u64)
                            .or_else(|| key["flag"].as_u64());
                        key["scancode"].as_u64() == input["scancode"].as_u64()
                            && base.is_some_and(|b| flag == b || flag == b + 1)
                    })
                    .or_else(|| {
                        if kind != "razerKey" {
                            return None;
                        }
                        let id = input["key"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| input["key"].to_string());
                        self.snap_razer_keys().iter().find(|key| key["key"] == id)
                    })
                    .and_then(|key| key["inputID"].as_str())
                    .map(str::to_owned);
                if let Some(key) = key {
                    self.snap_input(&key, cx);
                }
            }
        }
        cx.notify();
    }
    fn snap_razer_keys(&self) -> &[Value] {
        self.spec
            .config
            .get("DKM_KEYS")
            .and_then(Value::as_array)
            .unwrap_or_else(|| &data().razer_keys)
    }
    fn snap_key_up(&mut self, event: &KeyUpEvent, cx: &mut Context<Self>) {
        if !self
            .snap_tap
            .as_ref()
            .is_some_and(|s| s.config.enabled && s.phase != Phase::Ready)
        {
            return;
        }
        cx.stop_propagation();
        if event.keystroke.modifiers.platform {
            return;
        }
        // gpui-pre-windows events.rs parse_immutable collapses keypad Enter and
        // modifiers; do not infer their location, or numeric-keypad identity.
        let key = event.keystroke.key.as_str();
        let input = if key.len() == 1 && key.as_bytes()[0].is_ascii_alphabetic() {
            Some(format!("KEY_{}", key.to_ascii_uppercase()))
        } else {
            let code = match key {
                "space" => 32,
                "backspace" => 8,
                "tab" => 9,
                "escape" => 27,
                "menu" => 93,
                "up" => 38,
                "down" => 40,
                "left" => 37,
                "right" => 39,
                "home" => 36,
                "end" => 35,
                "pageup" => 33,
                "pagedown" => 34,
                "insert" => 45,
                "delete" => 46,
                _ => key
                    .strip_prefix('f')
                    .and_then(|f| f.parse::<u32>().ok())
                    .filter(|f| (1..=24).contains(f))
                    .map(|f| 111 + f)
                    .unwrap_or(0),
            }
            .to_string();
            data()
                .inputs
                .iter()
                .find(|v| v["keyCode"] == code)
                .and_then(|v| v["inputID"].as_str())
                .map(str::to_owned)
        };
        if let Some(input) = input {
            self.snap_input(&input, cx);
        }
    }
    fn snap_outside(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        if !state.config.enabled
            || state.phase == Phase::Ready
            || state.pair_bounds.get().contains(&position)
            || state.add_bounds.get().contains(&position)
        {
            return;
        }
        if state.staged.iter().any(|p| !p.complete()) {
            state.message = Message::Duplicate;
        } else if state.message != Message::Duplicate {
            state.commit(state.staged.clone());
            state.finish();
        }
        self.publish_snap_tap(cx);
    }

    pub(super) fn snap_panel(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let state = self.snap_tap.as_ref()?;
        let enabled = state.config.enabled;
        let has_copilot = self
            .spec
            .keys
            .iter()
            .any(|k| matches!(k["inputID"].as_str(), Some("DKM_F6" | "DKM_D2")));
        let pair_bounds = state.pair_bounds.clone();
        let add_bounds = state.add_bounds.clone();
        let rows = v_flex()
            .relative()
            .children(state.staged.iter().map(|pair| {
                let id = pair.id;
                h_flex()
                    .gap(surface::css(10.))
                    .mb(surface::css(10.))
                    .children(
                        [(Phase::Key1, &pair.key1), (Phase::Key2, &pair.key2)]
                            .into_iter()
                            .map(|(phase, key)| {
                                let recording = state.editing == Some(id) && state.phase == phase;
                                let warning = recording && state.message == Message::Duplicate;
                                let color = if warning {
                                    Colors::warning()
                                } else {
                                    Colors::accent()
                                };
                                let label = state.name(key, self.snap_razer_keys());
                                let text = div()
                                    .mx(surface::css(10.))
                                    .text_size(surface::css(11.))
                                    .child(label.clone())
                                    .when(recording, |text| {
                                        text.min_w(surface::css(38.))
                                            .h(surface::css(25.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .whitespace_nowrap()
                                    });
                                let text = if recording && enabled {
                                    text.with_animation(
                                        SharedString::from(format!(
                                            "snap-blink-{id}-{}-{}-{}",
                                            phase as u8, warning, state.capture_generation
                                        )),
                                        Animation::new(Duration::from_secs(1)).repeat(),
                                        move |text, delta| {
                                            let strength = (delta * 2. - 1.).abs();
                                            text.bg(color.opacity(strength)).text_color(
                                                Colors::text()
                                                    .blend(Colors::panel().opacity(strength)),
                                            )
                                        },
                                    )
                                    .into_any_element()
                                } else {
                                    text.into_any_element()
                                };
                                BaseButton::new(SharedString::from(format!(
                                    "snap-515-key-{id}-{}",
                                    phase as u8
                                )))
                                .accessibility_label(format!(
                                    "Snap Tap {id}, {}: {label}",
                                    phase as u8
                                ))
                                .disabled(!enabled)
                                .min_w(surface::css(64.))
                                .h(surface::css(44.))
                                .border(surface::css(2.))
                                .border_color(if recording { color } else { Colors::border() })
                                .rounded(surface::css(4.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(Colors::text())
                                .focus_visible(|s| s.border_color(Colors::accent()))
                                .child(text)
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.edit_snap_key(id, phase, window, cx)
                                    },
                                ))
                            }),
                    )
                    .when(id != 1, |row| {
                        row.child(
                            BaseButton::new(SharedString::from(format!("snap-515-delete-{id}")))
                                .accessibility_label(t("DELETE"))
                                .disabled(!enabled)
                                .size(surface::css(20.))
                                .ml(surface::css(20.))
                                .child(
                                    div()
                                        .size_full()
                                        .relative()
                                        .group(SharedString::from(format!("snap-delete-{id}")))
                                        .child(
                                            img("synapse/snap-tap-icon_delete.svg")
                                                .size_full()
                                                .absolute()
                                                .inset_0()
                                                .group_hover(
                                                    SharedString::from(format!("snap-delete-{id}")),
                                                    |s| s.hidden(),
                                                ),
                                        )
                                        .child(
                                            img("synapse/snap-tap-icon_delete_snap.svg")
                                                .size_full()
                                                .absolute()
                                                .inset_0()
                                                .hidden()
                                                .group_hover(
                                                    SharedString::from(format!("snap-delete-{id}")),
                                                    |s| s.visible(),
                                                ),
                                        ),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(state) = &mut this.snap_tap {
                                        if state.config.enabled {
                                            state.remove(id);
                                        }
                                    }
                                    this.publish_snap_tap(cx);
                                })),
                        )
                    })
            }))
            .child(
                canvas(
                    move |bounds, _, _| {
                        pair_bounds.set(bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        let add_disabled = !enabled || state.phase != Phase::Ready || state.staged.len() >= 4;
        let add = div()
            .relative()
            .id("snap-515-add-wrapper")
            .on_hover(cx.listener(move |this, hovered, window, cx| {
                if let Some(state) = &mut this.snap_tap {
                    state.tooltip = if *hovered && !add_disabled {
                        Some(window.mouse_position())
                    } else {
                        None
                    };
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                if let Some(state) = &mut this.snap_tap {
                    if state.tooltip.is_some() {
                        state.tooltip = Some(event.position);
                        cx.notify();
                    }
                }
            }))
            .child(
                BaseButton::new("snap-515-add")
                    .accessibility_label(t("SNAP_TAP_ADD_BUTTON_TOOLTIP"))
                    .disabled(add_disabled)
                    .opacity(if add_disabled && enabled { 0.3 } else { 1. })
                    .w(surface::css(64.))
                    .h(surface::css(44.))
                    .border(surface::css(2.))
                    .rounded(surface::css(5.))
                    .border_color(Colors::border())
                    .text_color(Colors::border())
                    .text_size(surface::css(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child("+")
                    .when(!add_disabled, |b| {
                        b.hover(|s| {
                            s.border_color(Colors::accent())
                                .text_color(Colors::accent())
                        })
                    })
                    .focus_visible(|s| s.border_color(Colors::accent()))
                    .on_click(cx.listener(|this, _, window, cx| this.add_snap_pair(window, cx))),
            )
            .child(
                canvas(
                    move |bounds, _, _| {
                        add_bounds.set(bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        Some(
            surface::panel_with_title_switch(
                t("SNAP_TAP_HEADER"),
                surface::SynapseSwitch::new("snap-515-enable")
                    .checked(enabled)
                    .accessibility_label(t("SNAP_TAP_HEADER"))
                    .on_change(cx.listener(|this, _, window, cx| this.toggle_snap_tap(window, cx))),
                surface::help_control(
                    "snap-515-help",
                    t(if has_copilot {
                        "SNAP_TAP_TOOLTIP_COPILOT"
                    } else {
                        "SNAP_TAP_TOOLTIP_MENU"
                    }),
                ),
                cx,
            )
            .id("snap-515-panel")
            .track_focus(&state.focus)
            .capture_key_up(cx.listener(|this, event, _, cx| this.snap_key_up(event, cx)))
            .capture_key_down(cx.listener(|this, _: &KeyDownEvent, _, cx| {
                if this
                    .snap_tap
                    .as_ref()
                    .is_some_and(|s| s.config.enabled && s.phase != Phase::Ready)
                {
                    cx.stop_propagation();
                }
            }))
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.snap_outside(event.position, cx)
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.snap_outside(event.position, cx)
                }),
            )
            .child(div().mb(surface::css(20.)).child(t("SNAP_TAP_DESC")))
            .child(
                v_flex()
                    .mt(surface::css(10.))
                    .opacity(if enabled { 1. } else { 0.3 })
                    .child(
                        h_flex()
                            .items_start()
                            .justify_between()
                            .child(rows)
                            .child(add),
                    )
                    .child(
                        div()
                            .mt(surface::css(10.))
                            .text_size(surface::css(14.))
                            .text_color(match state.message {
                                Message::Introduction => Colors::muted(),
                                Message::Duplicate => Colors::warning(),
                                Message::Success => Colors::success(),
                            })
                            .child(t(state.message.label())),
                    ),
            )
            .child(
                div()
                    .mt(surface::css(10.))
                    .text_size(surface::css(12.))
                    .text_color(Colors::muted())
                    .child("本地草稿，尚未写入设备。部分按键的精确录入尚待接入。"),
            )
            .into_any_element(),
        )
    }

    pub(super) fn snap_overlay(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.snap_tap.as_ref()?;
        if self.page != "TAB_CUSTOMIZE" {
            return None;
        }
        if state.prompt {
            return Some(
                Dialog::new(cx)
                    .focus_handle(state.prompt_focus.clone())
                    .close_on_backdrop_press(false)
                    .close_on_escape(false)
                    .on_cancel(|_, _, _| false)
                    .on_ok(|_, _, _| false)
                    .backdrop(div().absolute().inset_0().bg(Colors::backdrop()))
                    .popup(
                        div()
                            .absolute()
                            .left_0()
                            .bottom(relative(0.5))
                            .w(window.viewport_size().width)
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .w(surface::css(400.))
                                    .px(surface::css(30.))
                                    .py(surface::css(20.))
                                    .border_1()
                                    .border_color(Colors::warning())
                                    .rounded(surface::css(5.))
                                    .bg(Colors::panel())
                                    .text_color(Colors::border())
                                    .text_size(surface::css(14.))
                                    .font_family("Roboto")
                                    .line_height(surface::css(16.8))
                                    .text_center()
                                    .child(
                                        h_flex()
                                            .justify_center()
                                            .mb(surface::css(20.))
                                            .text_size(surface::css(16.))
                                            .text_color(Colors::warning())
                                            .child(
                                                img("synapse/snap-tap-warning.svg")
                                                    .size(surface::css(25.))
                                                    .mr(surface::css(10.)),
                                            )
                                            .child(t("EXIT_ADJUSTMENT_MODE").to_uppercase()),
                                    )
                                    .child(t("EXIT_ADJUSTMENT_MODE_DES")),
                            ),
                    )
                    .into_any_element(),
            );
        }
        state.tooltip.map(|position| {
            deferred(
                gpui_kit::base::Positioner::corner(
                    Anchor::TopLeft,
                    position + point(px(10.), px(20.)),
                )
                .margin(px(0.))
                .child(
                    gpui_kit::base::Tooltip::new("snap-515-add-tooltip")
                        .border_1()
                        .border_color(Colors::tooltip_border())
                        .bg(Colors::panel())
                        .text_color(Colors::border())
                        .px(surface::css(10.))
                        .py(surface::css(8.))
                        .font_family("Roboto")
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .child(t("SNAP_TAP_ADD_BUTTON_TOOLTIP")),
                ),
            )
            .with_priority(3)
            .into_any_element()
        })
    }
}

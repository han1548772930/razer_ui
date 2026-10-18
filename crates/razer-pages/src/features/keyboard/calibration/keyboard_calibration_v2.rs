//! PID 756's mounted magnetic-switch calibration (3021 / 66663).
//! UI intent and observations are separate; a submitted command is never a
//! successful measurement or persistent write. See calibration_v2_data.json.
use super::*;
use gpui_kit::base::Button as BaseButton;
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};
#[path = "keyboard_calibration_v2_artwork.rs"]
mod artwork;

const START: &str = "ON_START_ANALOG_MAGNETIC_KEY_CALIBRATION";
const STOP: &str = "ON_STOP_ANALOG_MAGNETIC_KEY_CALIBRATION";
const RESET: &str = "ON_RESET_ANALOG_MAGNETIC_KEY_CALIBRATION";
const GET_KEYS: &str = "ON_GET_ANALOG_MAGNETIC_KEY_CALIBRATED_KEY_LIST";
const GET_SWITCH: &str = "ON_GET_ANALOG_MAGNETIC_KEY_SWITCHTYPE";
const SET_SWITCH: &str = "ON_SET_ANALOG_MAGNETIC_KEY_SWITCHTYPE";
const RESET_SWITCH: &str = "ON_RESET_ANALOG_MAGNETIC_KEY_SWITCHTYPE";

#[derive(Clone)]
pub struct KeyboardNavigationRequested {
    page_key: String,
    history_index: Option<usize>,
}
impl KeyboardNavigationRequested {
    pub fn new(page_key: impl Into<String>) -> Self {
        Self {
            page_key: page_key.into(),
            history_index: None,
        }
    }
    pub fn with_history_index(mut self, index: usize) -> Self {
        self.history_index = Some(index);
        self
    }
    pub fn history_index(&self) -> Option<usize> {
        self.history_index
    }
    pub fn page_key(&self) -> &str {
        &self.page_key
    }
}
impl EventEmitter<KeyboardNavigationRequested> for KeyboardProductWorkspace {}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    Calibrating,
    Saving,
}
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Prompt {
    #[default]
    None,
    Start,
    Generic,
    BottomOut,
    ResetCalibration,
    ResetSwitch,
    Unsaved,
}
#[derive(Default)]
pub(super) struct State {
    phase: Phase,
    prompt: Prompt,
    pub(super) selected: BTreeSet<String>,
    calibrated: BTreeSet<String>,
    measuring: Vec<Value>,
    target: Option<String>,
    pub(super) switches: BTreeMap<String, Value>,
    seconds: u32,
    countdown_tick: Option<Instant>,
    saving_started: Option<Instant>,
    switch_segment: bool,
    auto_start_after_switch: bool,
    dont_show: bool,
    waiting_start_preference: bool,
    pending_page: Option<String>,
    generation: u64,
    timer: Option<Task<()>>,
    bottom_out: Option<Entity<SliderState>>,
    /// Observation of transport/power state, not a guessed connection mode.
    pub(super) is_ble: Option<bool>,
    pub(super) normal_mode: bool,
    panel_bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    artwork: artwork::Artwork,
    selected_count_tip: Option<Point<Pixels>>,
    selected_count_bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
}
#[derive(Deserialize)]
struct Data {
    labels: BTreeMap<String, String>,
    switches: Vec<Switch>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Switch {
    switch_index: u32,
    name: String,
    sub_label: String,
}
fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_calibration_v2_catalog())
}
fn label(symbol: &str) -> String {
    t(&data().labels[symbol])
}
fn formatted(symbol: &str, values: &[(&str, String)]) -> String {
    let mut result = label(symbol);
    for (name, value) in values {
        result = result.replace(&format!("{{{{{name}}}}}"), value);
    }
    result
}
impl State {
    fn apply(&mut self, action: &str, payload: &Value) -> bool {
        match action {
            "MW_UPDATE_ANALOG_MAGNETIC_KEY_CALIBRATED_KEY_LIST_TO_UI" => {
                let Some(keys) = payload.as_array() else {
                    return false;
                };
                self.calibrated = keys
                    .iter()
                    .filter_map(|key| {
                        key.as_str()
                            .or_else(|| key["inputId"].as_str())
                            .map(str::to_owned)
                    })
                    .collect();
                self.phase = Phase::Idle;
                self.measuring.clear();
                self.target = None;
                self.seconds = 0;
                self.timer = None;
            }
            "MW_UPDATE_ANALOG_MAGNETIC_KEY_CALIBRATION_STATUS_TO_UI" => {
                if self.phase == Phase::Calibrating {
                    self.seconds = 60;
                    self.countdown_tick = Some(Instant::now());
                }
                let Some(id) = payload["inputId"].as_str().filter(|s| !s.is_empty()) else {
                    return true;
                };
                let success = payload["status"] == "calibrationSuccess";
                if success {
                    for key in &mut self.measuring {
                        if key["status"] == "calibrationStart"
                            || key["status"] == "calibrationFailed"
                        {
                            key["status"] = json!("calibrationIdle");
                        }
                    }
                }
                if let Some(key) = self.measuring.iter_mut().find(|key| key["inputId"] == id) {
                    *key = payload.clone();
                } else {
                    self.measuring.push(payload.clone());
                }
                if success {
                    self.target = self
                        .measuring
                        .iter()
                        .find(|key| key["status"] != "calibrationSuccess")
                        .and_then(|key| key["inputId"].as_str())
                        .map(str::to_owned);
                } else if payload["status"] == "calibrationStart"
                    || payload["status"] == "calibrationFailed"
                {
                    self.target = Some(id.to_owned());
                }
            }
            "MW_UPDATE_ANALOG_MAGNETIC_KEY_SWITCHTYPE_TO_UI" => {
                let Some(switches) = payload.as_array() else {
                    return false;
                };
                self.switches.clear();
                self.apply_switches(switches);
            }
            "MW_UPDATE_ANALOG_MAGNETIC_KEY_SWITCHTYPE_STATUS_TO_UI" => {
                if payload["status"] == "Success"
                    && let Some(switches) = payload["switchTypes"].as_array()
                {
                    self.apply_switches(switches);
                }
            }
            _ => return false,
        }
        true
    }
    fn apply_switches(&mut self, switches: &[Value]) {
        for switch in switches {
            if let Some(id) = switch["inputId"].as_str().filter(|id| !id.is_empty())
                && switch["switchIndex"].is_number()
            {
                let mut value = json!({"switchIndex":switch["switchIndex"]});
                if switch["keyTravel"].is_number() {
                    value["keyTravel"] = switch["keyTravel"].clone();
                }
                self.switches.insert(id.to_owned(), value);
            }
        }
    }
    fn measured_count(&self) -> usize {
        let mut keys = self.calibrated.clone();
        keys.extend(
            self.measuring
                .iter()
                .filter(|key| key["status"] == "calibrationSuccess")
                .filter_map(|key| key["inputId"].as_str().map(str::to_owned)),
        );
        keys.len()
    }
    fn switch_index(&self) -> Option<u32> {
        let values = self
            .selected
            .iter()
            .map(|id| {
                self.switches
                    .get(id)
                    .and_then(|s| s["switchIndex"].as_u64())
                    .unwrap_or(1) as u32
            })
            .collect::<BTreeSet<_>>();
        if values.is_empty() {
            Some(1)
        } else if values.len() == 1 {
            values.first().copied()
        } else {
            None
        }
    }
}
fn start_button(id: &'static str, text: String, primary: bool, disabled: bool) -> BaseButton {
    BaseButton::new(id)
        .disabled(disabled)
        .h(surface::css(30.))
        .min_w(surface::css(155.))
        .px(surface::css(16.))
        .py(surface::css(7.))
        .rounded(surface::css(3.))
        .border_1()
        .border_color(if disabled { rgb(0x222222) } else { rgb(0) })
        .bg(if disabled {
            rgb(0x191919)
        } else if primary {
            rgb(0x44d62c)
        } else {
            rgb(0x707070)
        })
        .text_color(if disabled {
            rgb(0x444444)
        } else if primary {
            rgb(0)
        } else {
            rgb(0xcccccc)
        })
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .flex()
        .items_center()
        .justify_center()
        .when(!disabled, |button| button.hover(|style| style.opacity(0.8)))
        .child(text.to_uppercase())
}
fn reset_button(id: &'static str, switch: bool, disabled: bool) -> BaseButton {
    BaseButton::new(id)
        .group(id)
        .disabled(disabled)
        .h(surface::css(if switch { 27. } else { 30. }))
        .min_w(surface::css(if switch { 90. } else { 155. }))
        .when(switch, |button| {
            button.pl(surface::css(10.)).pr(surface::css(16.))
        })
        .flex()
        .items_center()
        .gap(surface::css(if switch { 3. } else { 7. }))
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .text_color(rgb(0xcccccc))
        .when(disabled, |button| button.opacity(0.3))
        .when(!disabled, |button| {
            button.hover(|style| style.text_color(rgb(0x44d62c)))
        })
        .child(
            div()
                .relative()
                .size(surface::css(20.))
                .flex_shrink_0()
                .child(img("synapse/keyboard-calibration-v2-refresh.svg").size_full())
                .when(!disabled, |view| {
                    view.child(
                        img("synapse/keyboard-calibration-v2-refresh-active.svg")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .opacity(0.)
                            .group_hover(id, |style| style.opacity(1.)),
                    )
                }),
        )
        .child(div().underline().child(label("O6d").to_uppercase()))
}

impl KeyboardProductWorkspace {
    /// True means retain the caller's navigation state until Don't Save emits
    /// KeyboardNavigationRequested. Continue or Close never emits it.
    pub fn defer_calibration_v2_page(&mut self, page_key: &str, cx: &mut Context<Self>) -> bool {
        !self.calibration_v2_before_page(page_key, cx)
    }
    pub(super) fn init_calibration_v2(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id != 756 {
            return;
        }
        self.calibration_v2.normal_mode = true;
        self.calibration_v2.bottom_out = Some(cx.new(|_| {
            SliderState::new()
                .min(20.)
                .max(40.)
                .step(1.)
                .default_value(40.)
        }));
    }
    fn v2_message(&mut self, action: &str, payload: Option<Value>, cx: &mut Context<Self>) {
        let mut message = json!({"type":action});
        if let Some(payload) = payload {
            message["payload"] = payload;
        }
        self.request_actuation_message(message, cx);
    }
    pub(super) fn enter_calibration_v2(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id == 756 && self.calibration_v2.is_ble != Some(true) {
            self.v2_message(GET_SWITCH, None, cx);
            if self.calibration_v2.phase == Phase::Calibrating {
                self.start_calibration_v2_timer(window, cx);
            }
        }
    }
    pub(super) fn unmount_calibration_v2_page(&mut self) {
        self.cancel_pending_calibration_start();
        self.calibration_v2.artwork.unmount();
        self.calibration_v2.timer = None;
        self.calibration_v2.generation = self.calibration_v2.generation.wrapping_add(1);
    }
    pub fn observe_calibration_v2_connection(
        &mut self,
        is_ble: bool,
        normal_mode: bool,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 756 {
            return;
        }
        if self.calibration_v2.is_ble != Some(is_ble)
            || self.calibration_v2.normal_mode != normal_mode
        {
            self.cancel_pending_calibration_start();
        }
        let first_non_ble = !is_ble && self.calibration_v2.is_ble != Some(false);
        self.calibration_v2.is_ble = Some(is_ble);
        self.calibration_v2.normal_mode = normal_mode;
        if first_non_ble {
            self.v2_message(GET_KEYS, None, cx);
            self.v2_message(GET_SWITCH, None, cx);
        }
        cx.notify();
    }
    pub fn observe_calibration_v2(
        &mut self,
        action: &str,
        payload: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_saving = self.calibration_v2.phase == Phase::Saving;
        if self.spec.product_id != 756 || !self.calibration_v2.apply(action, payload) {
            return;
        }
        self.calibration_v2
            .artwork
            .target_changed(self.calibration_v2.target.as_deref());
        if action == "MW_UPDATE_ANALOG_MAGNETIC_KEY_CALIBRATION_STATUS_TO_UI" {
            self.calibration_v2.artwork.observe_key(payload);
            self.start_calibration_v2_artwork_clock(window, cx);
        }
        if was_saving && self.calibration_v2.phase != Phase::Saving {
            self.calibration_v2_save_feedback(window, cx);
        }
        if action == "MW_UPDATE_ANALOG_MAGNETIC_KEY_SWITCHTYPE_STATUS_TO_UI"
            && self.calibration_v2.auto_start_after_switch
        {
            self.calibration_v2.auto_start_after_switch = false;
            if payload["status"] == "Success" {
                self.calibration_v2.switch_segment = false;
                self.start_or_save_calibration_v2(window, cx);
            }
        }
        self.refresh_actuation_selection(window, cx);
        cx.notify();
    }
    fn v2_input_ids(&self) -> Vec<String> {
        self.spec
            .keys
            .iter()
            .filter(|key| key["inputType"] == "AnalogInput")
            .filter_map(|key| key["inputID"].as_str().map(str::to_owned))
            .collect()
    }
    pub(super) fn calibration_v2_key_enabled(&self, id: &str) -> bool {
        self.spec.product_id == 756
            && self.calibration_v2.phase == Phase::Idle
            && !(self.factory_default_profile && self.calibration_v2.normal_mode)
            && self
                .spec
                .keys
                .iter()
                .any(|key| key["inputID"] == id && key["inputType"] == "AnalogInput")
    }
    pub(super) fn select_calibration_v2_key(
        &mut self,
        id: &str,
        control: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.calibration_v2_key_enabled(id) {
            return;
        }
        self.cancel_pending_calibration_start();
        if !self.calibration_v2.selected.remove(id) {
            if !control {
                self.calibration_v2.selected.clear();
            }
            self.calibration_v2.selected.insert(id.to_owned());
        }
        cx.notify();
    }
    pub fn cancel_calibration_v2(&mut self, cx: &mut Context<Self>) {
        self.cancel_pending_calibration_start();
        if self.calibration_v2.phase == Phase::Calibrating {
            self.stop_calibration_v2(false, cx);
        }
    }
    pub(super) fn cancel_pending_calibration_start(&mut self) {
        self.calibration_v2.waiting_start_preference = false;
    }
    pub(super) fn calibration_v2_before_page(
        &mut self,
        page: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id == 756
            && self.page == "TAB_CALIBRATION"
            && self.page != page
            && self.calibration_v2.phase != Phase::Idle
        {
            self.calibration_v2.pending_page = Some(page.to_owned());
            self.calibration_v2.prompt = Prompt::Unsaved;
            cx.notify();
            return false;
        }
        true
    }
    fn stop_calibration_v2(&mut self, save: bool, cx: &mut Context<Self>) {
        let state = &mut self.calibration_v2;
        state.phase = if save { Phase::Saving } else { Phase::Idle };
        state.saving_started = save.then(Instant::now);
        state.timer = None;
        state.generation = state.generation.wrapping_add(1);
        state.seconds = 0;
        state.target = None;
        self.v2_message(
            STOP,
            Some(json!({"stopMagneticKeyCalibration":{"save":save}})),
            cx,
        );
        cx.notify();
    }
    fn start_or_save_calibration_v2(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.calibration_v2.phase == Phase::Saving || self.calibration_v2.artwork.feedback_lock {
            return;
        }
        if self.calibration_v2.phase == Phase::Calibrating {
            self.stop_calibration_v2(true, cx);
            return;
        }
        let preferences = self.ui_preferences.read(cx);
        if preferences.closing() {
            return;
        }
        let preference = preferences.snapshot(UiPreference::MagneticCalibrationSkipPrompt);
        if !preference.ready() {
            self.calibration_v2.waiting_start_preference = true;
            return;
        }
        if preference.value() {
            self.begin_calibration_v2(window, cx);
        } else {
            self.calibration_v2.prompt = Prompt::Start;
            self.calibration_v2.dont_show = false;
            cx.notify();
        }
    }
    pub(super) fn resume_calibration_after_preference(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.calibration_v2.waiting_start_preference
            || !self
                .ui_preferences
                .read(cx)
                .snapshot(UiPreference::MagneticCalibrationSkipPrompt)
                .ready()
        {
            return;
        }
        self.calibration_v2.waiting_start_preference = false;
        if self.page == "TAB_CALIBRATION" && self.calibration_v2.is_ble != Some(true) {
            self.start_or_save_calibration_v2(window, cx);
        }
    }
    fn begin_calibration_v2(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let all = self.v2_input_ids();
        let keys = all
            .into_iter()
            .filter(|id| {
                self.calibration_v2.selected.is_empty() || self.calibration_v2.selected.contains(id)
            })
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return;
        }
        let state = &mut self.calibration_v2;
        state.selected = keys.iter().cloned().collect();
        state.phase = Phase::Calibrating;
        state.switch_segment = false;
        state.prompt = Prompt::None;
        state.seconds = 60;
        state.countdown_tick = Some(Instant::now());
        state.target = keys.first().cloned();
        state.artwork.target_changed(state.target.as_deref());
        state.measuring = keys
            .into_iter()
            .map(|id| json!({"inputId":id,"status":"calibrationIdle"}))
            .collect();
        let payload = json!({"calibratingKeys":state.measuring});
        self.start_calibration_v2_timer(window, cx);
        self.start_calibration_v2_artwork_clock(window, cx);
        self.v2_message(START, Some(payload), cx);
        cx.notify();
    }
    fn start_calibration_v2_timer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.calibration_v2;
        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        state.timer = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let keep = view
                    .update_in(cx, |this, _, cx| {
                        let state = &mut this.calibration_v2;
                        if state.generation != generation || state.phase != Phase::Calibrating {
                            return false;
                        }
                        state.seconds = state.seconds.saturating_sub(1);
                        state.countdown_tick = Some(Instant::now());
                        if state.seconds == 0 {
                            this.stop_calibration_v2(false, cx);
                            return false;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
    }
    fn set_calibration_v2_switch(&mut self, index: u32, travel: u32, cx: &mut Context<Self>) {
        let switches = self
            .calibration_v2
            .selected
            .iter()
            .map(|id| json!({"inputId":id,"switchIndex":index,"keyTravel":travel}))
            .collect::<Vec<_>>();
        if switches.is_empty() {
            return;
        }
        // Source reducer updates this draft before its response; only the
        // MW status observation below can trigger automatic calibration.
        self.calibration_v2.apply_switches(&switches);
        self.v2_message(
            SET_SWITCH,
            Some(json!({"setMagneticKeySwitchType":switches})),
            cx,
        );
        self.v2_message(GET_KEYS, None, cx);
        cx.notify();
    }
    pub(super) fn calibration_v2_page(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let state = &self.calibration_v2;
        if state.is_ble == Some(true) {
            return div().into_any_element();
        }
        let busy = state.phase != Phase::Idle;
        let saving = state.phase == Phase::Saving;
        let interaction_locked = saving || state.artwork.feedback_lock;
        let calibrating = state.phase == Phase::Calibrating;
        let locked = self.factory_default_profile && state.normal_mode;
        let panel_bounds = state.panel_bounds.clone();
        let mut body = v_flex()
            .relative()
            .w_full()
            .child(self.keyboard_image(true, cx))
            .child(
                layout::columns().child(layout::column_width(
                    680.,
                    surface::panel_with_help(
                        label("PLN"),
                        surface::help_control("calibration-v2-help", label("GeV")),
                        cx,
                    )
                    .on_prepaint(move |bounds, _, _| panel_bounds.set(bounds))
                    .w(surface::css(680.))
                    .min_w(surface::css(680.))
                    .max_w(surface::css(680.))
                    .mt(surface::css(20.))
                    .mb_0()
                    .min_h(surface::css(273.))
                    .child(h_flex().children([(false, "n0Y"), (true, "aL5")].map(
                        |(switch, symbol)| {
                            let active = state.switch_segment == switch;
                            let disabled = locked || interaction_locked || switch && calibrating;
                            BaseButton::new(SharedString::from(format!("v2-segment-{switch}")))
                                .role(Role::Tab)
                                .aria_selected(active)
                                .disabled(disabled)
                                .when(disabled, |button| button.opacity(0.3))
                                .p(surface::css(1.))
                                .pb_0()
                                .rounded_t(surface::css(5.))
                                .when(active, |button| {
                                    button.bg(linear_gradient(
                                        180.,
                                        linear_color_stop(rgb(0x44d62c), 0.),
                                        linear_color_stop(rgba(0x44d62c00), 1.),
                                    ))
                                })
                                .child(
                                    h_flex()
                                        .rounded_t(surface::css(4.))
                                        .when(active, |view| view.bg(rgb(0)))
                                        .pl(surface::css(12.))
                                        .pr(surface::css(16.))
                                        .py(surface::css(6.))
                                        .gap(surface::css(if switch { 6. } else { 10. }))
                                        .text_size(surface::css(14.))
                                        .line_height(surface::css(17.))
                                        .text_color(if active {
                                            rgb(0xcccccc)
                                        } else {
                                            rgb(0x999999)
                                        })
                                        .child(
                                            img(if switch {
                                                "synapse/keyboard-calibration-v2-switch.svg"
                                            } else {
                                                "synapse/keyboard-calibration-v2-calibrate.svg"
                                            })
                                            .size(surface::css(20.)),
                                        )
                                        .child(label(symbol)),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.calibration_v2.switch_segment = switch;
                                    cx.notify();
                                }))
                        },
                    )))
                    .child(if state.switch_segment {
                        let selected = state.switch_index();
                        v_flex()
                            .p(surface::css(20.))
                            .min_h(surface::css(249.))
                            .bg(rgb(0))
                            .text_color(rgb(0xcccccc))
                            .line_height(surface::css(17.))
                            .child(label("_Ix"))
                            .child(
                                h_flex()
                                    .gap(surface::css(12.))
                                    .mt(surface::css(20.))
                                    .child(
                                        Button::new("v2-select-all")
                                            .label(label("S$g").to_uppercase())
                                            .w(surface::css(146.))
                                            .min_w(surface::css(90.))
                                            .h(surface::css(29.))
                                            .bg(rgb(0x707070))
                                            .border_1()
                                            .border_color(rgba(0x0000004d))
                                            .text_color(rgb(0xcccccc))
                                            .text_size(surface::css(12.))
                                            .disabled(
                                                locked
                                                    || interaction_locked
                                                    || state.selected.len()
                                                        == self.v2_input_ids().len(),
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.calibration_v2.selected =
                                                    this.v2_input_ids().into_iter().collect();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        reset_button(
                                            "v2-reset-switch",
                                            true,
                                            locked || busy || interaction_locked,
                                        )
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.calibration_v2.prompt = Prompt::ResetSwitch;
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .mt(surface::css(20.))
                                    .border_1()
                                    .border_color(rgb(0x5d5d5d))
                                    .min_h(surface::css(90.))
                                    .max_h(surface::css(184.))
                                    .when(
                                        locked || interaction_locked || state.selected.is_empty(),
                                        |view| view.opacity(0.3),
                                    )
                                    .children(data().switches.iter().map(|switch| {
                                        let index = switch.switch_index;
                                        BaseButton::new(SharedString::from(format!(
                                            "v2-switch-{index}"
                                        )))
                                        .role(Role::RadioButton)
                                        .aria_selected(selected == Some(index))
                                        .aria_toggled(if selected == Some(index) {
                                            Toggled::True
                                        } else {
                                            Toggled::False
                                        })
                                        .accessibility_label(t(&switch.name))
                                        .disabled(
                                            locked
                                                || interaction_locked
                                                || state.selected.is_empty(),
                                        )
                                        .w_full()
                                        .flex()
                                        .items_start()
                                        .gap(surface::css(10.))
                                        .min_h(surface::css(46.))
                                        .px(surface::css(10.))
                                        .py(surface::css(5.))
                                        .child(
                                            div()
                                                .relative()
                                                .size(surface::css(20.))
                                                .flex_shrink_0()
                                                .rounded_full()
                                                .border_1()
                                                .border_color(rgb(0x737373))
                                                .when(selected == Some(index), |view| {
                                                    view.child(
                                                        div()
                                                            .absolute()
                                                            .inset(surface::css(4.))
                                                            .rounded_full()
                                                            .bg(rgb(0x44d62c)),
                                                    )
                                                }),
                                        )
                                        .child(
                                            v_flex()
                                                .child(
                                                    div()
                                                        .text_color(rgb(0xcccccc))
                                                        .text_size(surface::css(14.))
                                                        .line_height(surface::css(17.))
                                                        .child(t(&switch.name)),
                                                )
                                                .child(
                                                    div()
                                                        .mt(surface::css(5.))
                                                        .text_size(surface::css(12.))
                                                        .text_color(rgb(0x999999))
                                                        .child(t(&switch.sub_label)),
                                                ),
                                        )
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                if index == 2 {
                                                    this.calibration_v2.prompt = Prompt::Generic;
                                                    let travel = this
                                                        .calibration_v2
                                                        .selected
                                                        .iter()
                                                        .map(|id| {
                                                            this.calibration_v2
                                                                .switches
                                                                .get(id)
                                                                .and_then(|v| {
                                                                    v["keyTravel"].as_u64()
                                                                })
                                                                .unwrap_or(40)
                                                        })
                                                        .collect::<BTreeSet<_>>();
                                                    let value = if travel.len() == 1 {
                                                        *travel.first().unwrap()
                                                    } else {
                                                        40
                                                    };
                                                    if let Some(slider) =
                                                        &this.calibration_v2.bottom_out
                                                    {
                                                        slider.update(cx, |slider, cx| {
                                                            slider.set_value(
                                                                value.clamp(20, 40) as f32,
                                                                window,
                                                                cx,
                                                            )
                                                        });
                                                    }
                                                } else {
                                                    this.set_calibration_v2_switch(index, 40, cx);
                                                }
                                                cx.notify();
                                            }),
                                        )
                                    })),
                            )
                            .child(
                                h_flex()
                                    .mt(surface::css(20.))
                                    .gap(surface::css(12.))
                                    .text_color(rgb(0xcccccc))
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(14.))
                                    .child(formatted(
                                        "T4K",
                                        &[("count", state.selected.len().to_string())],
                                    ))
                                    .child(
                                        BaseButton::new("v2-selected-count-info")
                                            .size(surface::css(14.))
                                            .rounded_full()
                                            .bg(rgb(0x999999))
                                            .text_color(rgb(0x222222))
                                            .text_size(surface::css(10.))
                                            .font_weight(FontWeight::BOLD)
                                            .line_height(surface::css(14.))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child("?")
                                            .accessibility_label(label("v9G"))
                                            .on_prepaint({
                                                let bounds = state.selected_count_bounds.clone();
                                                move |rect, _, _| bounds.set(rect)
                                            })
                                            .on_hover(cx.listener(
                                                |this, hovered: &bool, window, cx| {
                                                    this.calibration_v2.selected_count_tip =
                                                        hovered.then(|| {
                                                            let bounds = this
                                                                .calibration_v2
                                                                .selected_count_bounds
                                                                .get();
                                                            let scale =
                                                                f32::from(window.rem_size()) / 16.;
                                                            point(
                                                                bounds
                                                                    .right()
                                                                    .min(
                                                                        window
                                                                            .viewport_size()
                                                                            .width
                                                                            - px(290. * scale),
                                                                    )
                                                                    .max(px(10. * scale)),
                                                                bounds
                                                                    .top()
                                                                    .min(
                                                                        window
                                                                            .viewport_size()
                                                                            .height
                                                                            - px(158. * scale),
                                                                    )
                                                                    .max(px(10. * scale)),
                                                            )
                                                        });
                                                    cx.notify();
                                                },
                                            )),
                                    ),
                            )
                            .into_any_element()
                    } else {
                        let failed = state
                            .measuring
                            .iter()
                            .filter(|key| key["status"] == "calibrationFailed")
                            .count();
                        v_flex()
                            .p(surface::css(20.))
                            .min_h(surface::css(185.))
                            .bg(rgb(0))
                            .text_color(rgb(0xcccccc))
                            .line_height(surface::css(17.))
                            .child(label("K8$"))
                            .child(
                                h_flex()
                                    .mt(surface::css(18.))
                                    .children(busy.then(|| {
                                        start_button(
                                            "v2-cancel-calibration",
                                            label("d4T"),
                                            false,
                                            locked,
                                        )
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                this.stop_calibration_v2(false, cx)
                                            }),
                                        )
                                    }))
                                    .child(
                                        start_button(
                                            "v2-start-save",
                                            label(if calibrating { "fqB" } else { "UTs" }),
                                            calibrating,
                                            locked || interaction_locked,
                                        )
                                        .on_click(
                                            cx.listener(|this, _, window, cx| {
                                                this.start_or_save_calibration_v2(window, cx)
                                            }),
                                        ),
                                    ),
                            )
                            .children((!saving || failed == 0).then(|| {
                                v_flex()
                                    .mt(surface::css(13.))
                                    .text_color(rgb(0x999999))
                                    .child(
                                        div()
                                            .when(busy, |view| {
                                                view.text_color(if failed > 0 {
                                                    rgb(0xfd8611)
                                                } else {
                                                    rgb(0x44d62c)
                                                })
                                            })
                                            .child(label(if calibrating && failed > 0 {
                                                "tyI"
                                            } else {
                                                "EgE"
                                            })),
                                    )
                                    .child(if calibrating && failed > 0 {
                                        formatted("RkU", &[("count", failed.to_string())])
                                    } else if busy {
                                        formatted(
                                            "sas",
                                            &[
                                                ("number", state.measured_count().to_string()),
                                                ("total", self.v2_input_ids().len().to_string()),
                                            ],
                                        )
                                    } else {
                                        label("A$4")
                                    })
                            }))
                            .children(busy.then(|| {
                                h_flex()
                                    .mt(surface::css(24.))
                                    .min_h(surface::css(54.))
                                    .px(surface::css(12.))
                                    .py(surface::css(10.))
                                    .gap(surface::css(14.))
                                    .rounded(surface::css(3.))
                                    .bg(rgb(0x1f1f1f))
                                    .text_color(rgb(0x999999))
                                    .line_height(surface::css(18.))
                                    .child(
                                        img("synapse/camera-section-warning.svg")
                                            .w(surface::css(28.))
                                            .h(surface::css(24.))
                                            .flex_shrink_0(),
                                    )
                                    .child(label("xrI"))
                            }))
                            .child(
                                h_flex().mt(surface::css(20.)).child(
                                    reset_button(
                                        "v2-reset-calibration",
                                        false,
                                        locked || busy || interaction_locked,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.calibration_v2.prompt = Prompt::ResetCalibration;
                                            cx.notify();
                                        },
                                    )),
                                ),
                            )
                            .into_any_element()
                    }),
                )),
            )
            .when(locked, |view| view.opacity(0.3));
        if saving {
            let bounds = state.artwork.keyboard_bounds();
            body = body.child(
                razer_assets::deferred_resource(
                    anchored()
                        .anchor(Anchor::TopCenter)
                        .position(point(
                            bounds.left() + bounds.size.width / 2.,
                            bounds.top() + bounds.size.height * 0.45
                                - window.rem_size() * 22. / 16.,
                        ))
                        .child(
                            h_flex()
                                .h(surface::css(44.))
                                .px(surface::css(20.))
                                .gap(surface::css(11.))
                                .border_1()
                                .border_color(rgb(0x707070))
                                .rounded(surface::css(3.))
                                .bg(rgb(0x111111))
                                .text_color(rgb(0xcccccc))
                                .text_size(surface::css(14.))
                                .line_height(surface::css(18.))
                                .child(artwork::status_ring(
                                    true,
                                    state.saving_started,
                                    0,
                                    cx.reduce_motion(),
                                ))
                                .child(label("Bfm")),
                        ),
                )
                .with_priority(4),
            );
        }
        if locked {
            body = body.child(calibration::factory_warning(window));
        }
        if calibrating && (1..=15).contains(&state.seconds) {
            body = body.child(
                razer_assets::deferred_resource(
                    anchored()
                        .anchor(Anchor::TopRight)
                        .position(point(
                            window.viewport_size().width - window.rem_size() * 18. / 16.,
                            window.rem_size() * 100. / 16.,
                        ))
                        .child(
                            h_flex()
                                .items_start()
                                .w(surface::css(400.))
                                .min_h(surface::css(112.))
                                .p(surface::css(22.))
                                .gap(surface::css(14.))
                                .border_1()
                                .border_color(rgb(0xfd8611))
                                .rounded(surface::css(3.))
                                .bg(rgb(0x111111))
                                .text_color(rgb(0x999999))
                                .child(artwork::status_ring(
                                    false,
                                    state.countdown_tick,
                                    state.seconds,
                                    cx.reduce_motion(),
                                ))
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_color(rgb(0xcccccc))
                                                .text_size(surface::css(14.))
                                                .line_height(surface::css(17.))
                                                .child(label("u_e")),
                                        )
                                        .child(
                                            div()
                                                .mt(surface::css(2.))
                                                .text_size(surface::css(14.))
                                                .line_height(surface::css(17.))
                                                .child(formatted(
                                                    "KnT",
                                                    &[("seconds", "15".into())],
                                                )),
                                        )
                                        .child(
                                            div()
                                                .mt(surface::css(12.))
                                                .text_color(rgb(0x666666))
                                                .text_size(surface::css(12.))
                                                .line_height(surface::css(18.))
                                                .child(format!("00:{:02}", state.seconds)),
                                        ),
                                ),
                        ),
                )
                .with_priority(6),
            );
        }
        if let Some(position) = state.selected_count_tip.filter(|_| state.switch_segment) {
            body = body.child(
                razer_assets::deferred_resource(
                    anchored().position(position).child(
                        div()
                            .max_w(surface::css(260.))
                            .px(surface::css(10.))
                            .py(surface::css(8.))
                            .border_1()
                            .border_color(rgb(0x5d5d5d))
                            .bg(rgb(0))
                            .text_color(rgb(0xcccccc))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(16.))
                            .child(gpui_kit::base::TextView::html(
                                "v2-selected-count-tip",
                                formatted(
                                    "YFf",
                                    &[
                                        ("count", state.selected.len().to_string()),
                                        ("max", "4".into()),
                                    ],
                                ),
                            )),
                    ),
                )
                .with_priority(10),
            );
        }
        if state.prompt != Prompt::None {
            body = body.child(self.calibration_v2_prompt(window, cx));
        }
        body.into_any_element()
    }
    fn calibration_v2_prompt(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let state = &self.calibration_v2;
        let close = |this: &mut Self, cx: &mut Context<Self>| {
            this.calibration_v2.prompt = Prompt::None;
            cx.notify();
        };
        let (title, description, width) = match state.prompt {
            Prompt::Start => ("tu2", "jib", 420.),
            Prompt::Generic => ("QbX", "mak", 446.),
            Prompt::BottomOut => ("f9s", "Pln", 850.),
            Prompt::ResetCalibration => ("lD4", "PHL", 300.),
            Prompt::ResetSwitch => ("iL2", "CVn", 300.),
            Prompt::Unsaved => ("AGD", "Qbu", 400.),
            Prompt::None => unreachable!(),
        };
        let travel = state
            .bottom_out
            .as_ref()
            .map(|slider| slider.read(cx).value().start())
            .unwrap_or(40.);
        let mut content = v_flex()
            .w(surface::css(width))
            .p(surface::css(20.))
            .gap(surface::css(14.))
            .border_1()
            .border_color(cx.theme().warning)
            .bg(cx.theme().popover)
            .child(
                div()
                    .text_size(surface::css(16.))
                    .text_center()
                    .child(label(title)),
            )
            .child(
                gpui_kit::base::TextView::html(
                    "v2-prompt-description",
                    formatted(
                        description,
                        &[
                            ("count", state.selected.len().to_string()),
                            ("distance", format!("{:.1}", travel / 10.)),
                        ],
                    ),
                )
                .selectable(false)
                .scrollable(false),
            );
        if matches!(state.prompt, Prompt::BottomOut | Prompt::Unsaved) {
            content = content.relative().child(
                BaseButton::new("v2-prompt-close")
                    .absolute()
                    .top_0()
                    .right_0()
                    .w(surface::css(40.))
                    .h(surface::css(36.))
                    .child(
                        razer_assets::resource_svg()
                            .path("synapse/mapping-close.svg")
                            .size(surface::css(24.)),
                    )
                    .accessibility_label(label("Ndl"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.calibration_v2.pending_page = None;
                        close(this, cx);
                    })),
            );
        }
        match state.prompt {
            Prompt::Start => {
                content = content
                    .child(
                        img("synapse/camera-section-warning.svg")
                            .w(surface::css(24.))
                            .h(surface::css(21.)),
                    )
                    .child(
                        Checkbox::new("v2-dont-show")
                            .label(label("Sk8"))
                            .checked(state.dont_show)
                            .on_change(cx.listener(|this, value: &bool, _, cx| {
                                this.calibration_v2.dont_show = *value;
                                cx.notify();
                            })),
                    )
                    .child(Button::new("v2-confirm-start").label(label("OK")).on_click(
                        cx.listener(|this, _, window, cx| {
                            this.ui_preferences.update(cx, |owner, cx| {
                                owner.set(
                                    UiPreference::MagneticCalibrationSkipPrompt,
                                    this.calibration_v2.dont_show,
                                    cx,
                                );
                            });
                            this.begin_calibration_v2(window, cx);
                        }),
                    ));
            }
            Prompt::Generic => {
                content = content
                    .children(
                        ["mLw", "wEe", "ADb"]
                            .map(|symbol| div().child(format!("• {}", label(symbol)))),
                    )
                    .child(label("EVj"))
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(
                                Button::new("v2-generic-proceed")
                                    .label(label("h9P"))
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.calibration_v2.prompt = Prompt::BottomOut;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("v2-use-existing")
                                    .label(label("qm1"))
                                    .on_click(cx.listener(move |this, _, _, cx| close(this, cx))),
                            ),
                    );
            }
            Prompt::BottomOut => {
                content = content
                    .child(
                        h_flex()
                            .h(surface::css(264.))
                            .justify_center()
                            .items_center()
                            .children(state.bottom_out.as_ref().map(|slider| {
                                Slider::new(slider).vertical().h(surface::css(204.))
                            })),
                    )
                    .child(label("mup"))
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(
                                Button::new("v2-bottom-cancel")
                                    .label(label("bOp"))
                                    .outline()
                                    .on_click(cx.listener(move |this, _, _, cx| close(this, cx))),
                            )
                            .child(Button::new("v2-bottom-next").label(label("K29")).on_click(
                                cx.listener(|this, _, _, cx| {
                                    let travel = this
                                        .calibration_v2
                                        .bottom_out
                                        .as_ref()
                                        .map(|slider| slider.read(cx).value().start() as u32)
                                        .unwrap_or(40);
                                    this.calibration_v2.auto_start_after_switch = true;
                                    this.calibration_v2.prompt = Prompt::None;
                                    this.set_calibration_v2_switch(2, travel, cx);
                                }),
                            )),
                    );
            }
            Prompt::ResetCalibration | Prompt::ResetSwitch => {
                let switch = state.prompt == Prompt::ResetSwitch;
                content = content.child(
                    Button::new("v2-reset-confirm")
                        .label(label("Uty"))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.calibration_v2.prompt = Prompt::None;
                            this.calibration_v2.selected.clear();
                            if switch {
                                this.calibration_v2.switches.clear();
                                this.v2_message(RESET_SWITCH, None, cx);
                                this.v2_message(GET_KEYS, None, cx);
                            } else {
                                this.calibration_v2.phase = Phase::Saving;
                                this.calibration_v2.saving_started = Some(Instant::now());
                                this.calibration_v2
                                    .artwork
                                    .reset(this.calibration_v2.calibrated.clone());
                                this.calibration_v2.calibrated.clear();
                                this.calibration_v2.measuring.clear();
                                this.v2_message(RESET, None, cx);
                                this.start_calibration_v2_artwork_clock(window, cx);
                            }
                            cx.notify();
                        })),
                );
            }
            Prompt::Unsaved => {
                content = content
                    .child(
                        gpui_kit::base::TextView::html("v2-unsaved-note", label("Je6"))
                            .selectable(false)
                            .scrollable(false),
                    )
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(
                                Button::new("v2-dont-save")
                                    .label(label("set"))
                                    .outline()
                                    .disabled(state.phase == Phase::Saving)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.stop_calibration_v2(false, cx);
                                        this.calibration_v2.prompt = Prompt::None;
                                        if let Some(page) = this.calibration_v2.pending_page.take()
                                        {
                                            this.set_page(&page, window, cx);
                                            cx.emit(KeyboardNavigationRequested::new(page));
                                        }
                                    })),
                            )
                            .child(Button::new("v2-continue").label(label("BjV")).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.calibration_v2.pending_page = None;
                                    close(this, cx);
                                }),
                            )),
                    );
            }
            Prompt::None => {}
        }
        let unit = window.rem_size() / 16.;
        let viewport = window.viewport_size();
        let bottom_out = state.prompt == Prompt::BottomOut;
        if matches!(state.prompt, Prompt::ResetCalibration | Prompt::ResetSwitch) {
            let bounds = state.panel_bounds.get();
            return razer_assets::deferred_resource(
                div()
                    .child(
                        anchored()
                            .anchor(Anchor::TopLeft)
                            .position(bounds.origin)
                            .child(
                                div()
                                    .w(bounds.size.width)
                                    .h(bounds.size.height)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.calibration_v2.prompt = Prompt::None;
                                            cx.notify();
                                        }),
                                    ),
                            ),
                    )
                    .child(
                        anchored()
                            .anchor(Anchor::LeftCenter)
                            .position(point(
                                bounds.origin.x + (bounds.size.width - unit * width) / 2.,
                                bounds.origin.y + bounds.size.height * 0.75,
                            ))
                            .child(
                                content
                                    .border_color(cx.theme().danger)
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    }),
                            ),
                    ),
            )
            .with_priority(2)
            .into_any_element();
        }
        let top = if bottom_out { 106. } else { 86. };
        let backdrop = div()
            .absolute()
            .w(viewport.width)
            .h(viewport.height - unit * top)
            .flex()
            .items_center()
            .justify_center()
            .bg(cx.theme().background.opacity(0.8))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if matches!(
                        this.calibration_v2.prompt,
                        Prompt::ResetCalibration | Prompt::ResetSwitch
                    ) {
                        this.calibration_v2.prompt = Prompt::None;
                        cx.notify();
                    }
                }),
            );
        let card_y = match state.prompt {
            Prompt::Generic => 280.,
            Prompt::BottomOut => 106.,
            _ => 230.,
        };
        let card_x = (viewport.width - unit * width) / 2.;
        if bottom_out {
            content = content.h(viewport.height - unit * 106.);
        }
        razer_assets::deferred_resource(
            div()
                .child(
                    anchored()
                        .anchor(Anchor::TopLeft)
                        .position(point(px(0.), unit * top))
                        .child(backdrop),
                )
                .child(
                    anchored()
                        .anchor(if bottom_out {
                            Anchor::TopLeft
                        } else {
                            Anchor::LeftCenter
                        })
                        .position(point(card_x, unit * card_y))
                        .child(
                            content
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                        ),
                ),
        )
        .with_priority(2)
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn measurements_are_transient_until_calibrated_list_observation() {
        let mut state = State::default();
        state.phase = Phase::Calibrating;
        state.seconds = 4;
        state.measuring = vec![
            json!({"inputId":"KEY_A","status":"calibrationStart"}),
            json!({"inputId":"KEY_B","status":"calibrationIdle"}),
        ];
        assert!(state.apply(
            "MW_UPDATE_ANALOG_MAGNETIC_KEY_CALIBRATION_STATUS_TO_UI",
            &json!({"inputId":"KEY_A","status":"calibrationSuccess"})
        ));
        assert_eq!(state.measured_count(), 1);
        assert!(state.calibrated.is_empty());
        assert_eq!(state.seconds, 60);
        assert_eq!(state.target.as_deref(), Some("KEY_B"));
        assert!(state.apply(
            "MW_UPDATE_ANALOG_MAGNETIC_KEY_CALIBRATED_KEY_LIST_TO_UI",
            &json!(["KEY_A"])
        ));
        assert!(state.calibrated.contains("KEY_A"));
        assert!(state.measuring.is_empty());
        assert!(state.phase == Phase::Idle);
    }
    #[::core::prelude::v1::test]
    fn failed_switch_response_does_not_replace_observed_switches() {
        let mut state = State::default();
        state.apply(
            "MW_UPDATE_ANALOG_MAGNETIC_KEY_SWITCHTYPE_TO_UI",
            &json!([{"inputId":"KEY_A","switchIndex":1,"keyTravel":40}]),
        );
        state.apply("MW_UPDATE_ANALOG_MAGNETIC_KEY_SWITCHTYPE_STATUS_TO_UI",&json!({"status":"Failed","switchTypes":[{"inputId":"KEY_A","switchIndex":2,"keyTravel":20}]}));
        assert_eq!(state.switches["KEY_A"]["switchIndex"], 1);
        assert!(!state.apply("MW_UPDATE_ANALOG_MAGNETIC_KEY_SWITCHTYPE_TO_UI", &json!({})));
        assert_eq!(state.switches.len(), 1);
    }
}

// Generated catalog reader: razer-pages/keyboard_calibration_v2_data.
fn read_keyboard_calibration_v2_catalog() -> Data {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Data> {
        let value = match r.tag()? {
            0 => Data {
                labels: r.child(decode_1)?,
                switches: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_2, decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Switch>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Switch> {
        let value = match r.tag()? {
            0 => Switch {
                switch_index: r.child(decode_5)?,
                name: r.child(decode_2)?,
                sub_label: r.child(decode_2)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_CALIBRATION_V2_DATA,
        0,
        "2a202daef886d0b931a65d2c429b17a52bac148b852cf858d081158fd4c2ae24",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_calibration_v2_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_calibration_v2_data_decodes() {
        let _ = super::read_keyboard_calibration_v2_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_calibration_v2_data.

//! Current Seiren V3 Pro/V3 Chroma normal EQ6007, wrappers6299/1422,
//! actions1541/3523/5444 and main `fi` reducer. The local draft and its
//! source previews are separate from native write acknowledgements.
use super::*;
use gpui_kit::base::motion::{self, Easing, Presence, Transition};
use gpui_kit::base::{Button as BaseButton, Popover, Select as BaseSelect};
use gpui_kit::component::IndexPath;
use razer_widgets::{
    source_text::SourceText,
    surface::css,
    synapse_select::{self, PointerSelectPlacement, PointerSelectStyle, SelectOptionDecoration},
};
use std::{cell::Cell, rc::Rc, time::Duration};

#[path = "audio_seiren_eq_theme.rs"]
pub(super) mod colors;
#[path = "audio_seiren_eq_graph.rs"]
mod graph;
#[path = "audio_seiren_eq_knob.rs"]
mod knob;
#[path = "audio_seiren_eq_math.rs"]
mod math;
#[path = "audio_seiren_eq_model.rs"]
mod model;
#[path = "audio_seiren_eq_number.rs"]
mod number;
use colors::Colors as C;
use model::{Band, Model, label};

const MONITORS: [(&str, &str); 3] = [
    ("mic", "/profile/micVolume"),
    ("headphones", "/profile/volume"),
    ("sidetone", "/device/micSideTone"),
];

#[derive(Clone, Copy)]
enum Drag {
    Band(u8),
    QHandle(u8, bool),
}
#[derive(Default)]
pub(super) struct State {
    pub(super) live: razer_device::seiren_mixer::LiveState,
    numbers: BTreeMap<String, Entity<number::Number>>,
    faders: Vec<Entity<SliderState>>,
    knobs: BTreeMap<String, Entity<SliderState>>,
    dial: Option<Entity<SelectState<Vec<Choice>>>>,
    filter: Option<Entity<SelectState<Vec<Choice>>>>,
    pub(super) wrapper_bounds: Rc<Cell<Bounds<Pixels>>>,
    subscriptions: Vec<Subscription>,
    monitor_preview: [Value; 3],
    monitor_observed: [Value; 3],
    monitor_saved: [Value; 2],
    monitor_tasks: [Option<Task<()>>; 3],
    effects_local: Value,
    effects_observed: Value,
    pub(super) effects_committed: Value,
    effects_throttle: Option<Task<()>>,
    effects_commit: Option<Task<()>>,
    committed_eq: Value,
    filter_focus: Option<FocusHandle>,
    filter_menu_focus: Option<FocusHandle>,
    filter_open: bool,
    filter_cursor: usize,
    filter_value: Option<String>,
    filter_keyboard_navigation: bool,
    drag: Option<Drag>,
    hovered_band: Option<u8>,
    hovered_effect: Option<&'static str>,
    pressed_effect: Option<&'static str>,
    output_sample_rate: Option<u32>,
    capture_sample_rate: Option<u32>,
    bins: Vec<f32>,
    spectrum: math::Spectrum,
    resampler: math::Resampler,
    levels: [[f32; 2]; 2],
    peaks: [[f32; 2]; 2],
    peak_task: Option<Task<()>>,
    tutorial_open: bool,
    tutorial_type: usize,
    sidetone_tooltip: Option<Point<Pixels>>,
}

impl State {
    fn record_native_levels(&mut self, levels: &razer_device::seiren_eq::NativeLevels) {
        use razer_device::seiren_mixer::{EndpointObservation, HEADPHONES, MICROPHONE, Reading};
        for (index, id, level) in [
            (0, MICROPHONE, &levels.microphone),
            (1, HEADPHONES, &levels.headphones),
        ] {
            self.monitor_observed[index] = Value::Null;
            if level.value().is_finite()
                && (0. ..=100.).contains(&level.value())
                && level.value().fract() == 0.
            {
                let _ = self.live.observe_endpoint(
                    id,
                    EndpointObservation {
                        level: level.value() as u32,
                        muted: !level.is_enabled(),
                    },
                );
            } else {
                self.live
                    .set_endpoint(id, Reading::Failed("Invalid Marie volume readback".into()));
            }
        }
    }

    fn record_live(&mut self, observed: razer_device::seiren_mixer::LiveState) -> bool {
        for (index, id) in [
            razer_device::seiren_mixer::MICROPHONE,
            razer_device::seiren_mixer::HEADPHONES,
        ]
        .into_iter()
        .enumerate()
        {
            if self.live.endpoint(id) != observed.endpoint(id) {
                self.monitor_observed[index] = Value::Null;
            }
        }
        self.live = observed;
        let unavailable = self.live.meters().value().is_none();
        if unavailable {
            self.levels = [[0.; 2]; 2];
            self.peaks = [[0.; 2]; 2];
            self.peak_task = None;
        }
        unavailable
    }
    fn sync_monitor(&mut self, index: usize, value: Value) {
        if !value.is_null() && self.monitor_observed[index] != value {
            self.monitor_observed[index] = value.clone();
            self.monitor_preview[index] = value;
        }
    }
    fn sync_filter(&mut self, kind: &str) {
        if self.filter_value.as_deref() != Some(kind) {
            self.filter_value = Some(kind.to_owned());
            self.filter_open = false;
            self.reset_filter_cursor();
        }
    }

    fn reset_filter_cursor(&mut self) {
        self.filter_keyboard_navigation = false;
        self.filter_cursor = model::FILTERS
            .iter()
            .position(|kind| Some(*kind) == self.filter_value.as_deref())
            .unwrap_or(2);
    }

    fn set_filter_open(&mut self, open: bool) {
        if open && !self.filter_open {
            self.reset_filter_cursor();
        }
        self.filter_open = open;
    }

    pub(super) fn clear_capture(&mut self) -> bool {
        let visible = !self.bins.is_empty();
        if !visible && self.capture_sample_rate.is_none() {
            return false;
        }
        self.capture_sample_rate = None;
        self.bins.clear();
        self.spectrum.clear();
        self.resampler.clear();
        visible
    }
}

fn supported(pid: u32) -> bool {
    matches!(pid, 1422 | 1446)
}

fn filter_choices() -> Vec<Choice> {
    model::FILTERS
        .into_iter()
        .map(|kind| Choice::new(kind, kind))
        .collect()
}

#[cfg(test)]
mod state_tests {
    use super::State;

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn seiren_audio_readback_enters_observed_state_without_a_window_and_keeps_drafts_separate() {
        use razer_device::{
            seiren_eq::{Level, NativeLevels},
            seiren_mixer::{HEADPHONES, MICROPHONE},
        };
        let mut state = State::default();
        state.monitor_preview = [
            serde_json::json!({"value":82}),
            serde_json::json!({"value":10}),
            serde_json::Value::Null,
        ];
        let draft = state.monitor_preview.clone();
        state.record_native_levels(&NativeLevels {
            microphone: Level::new(false, 37.),
            headphones: Level::new(true, 61.),
        });
        assert_eq!(
            state
                .live
                .endpoint(MICROPHONE)
                .unwrap()
                .value()
                .unwrap()
                .level,
            37
        );
        assert!(
            state
                .live
                .endpoint(MICROPHONE)
                .unwrap()
                .value()
                .unwrap()
                .muted
        );
        assert_eq!(
            state
                .live
                .endpoint(HEADPHONES)
                .unwrap()
                .value()
                .unwrap()
                .level,
            61
        );
        assert_eq!(state.monitor_preview, draft);
        assert!(state.monitor_observed[0].is_null());
        state.record_native_levels(&NativeLevels {
            microphone: Level::new(true, 37.5),
            headphones: Level::new(true, 101.),
        });
        assert!(state.live.endpoint(MICROPHONE).unwrap().value().is_none());
        assert!(state.live.endpoint(HEADPHONES).unwrap().value().is_none());
        assert_eq!(state.monitor_preview, draft);
    }

    #[test]
    fn seiren_audio_failed_disconnected_and_reconnected_observations_never_fall_back_to_saved_values()
     {
        use razer_device::seiren_mixer::{EndpointObservation, LiveState, MICROPHONE, Reading};
        let mut state = State::default();
        state.monitor_preview[0] = serde_json::json!({"value":82});
        let mut observed = LiveState::default();
        observed
            .observe_endpoint(
                MICROPHONE,
                EndpointObservation {
                    level: 37,
                    muted: false,
                },
            )
            .unwrap();
        assert!(state.record_live(observed));
        assert_eq!(
            state
                .live
                .endpoint(MICROPHONE)
                .unwrap()
                .value()
                .unwrap()
                .level,
            37
        );
        state.record_live(LiveState::failed("endpoint read failed".into()));
        assert!(
            matches!(state.live.endpoint(MICROPHONE), Some(Reading::Failed(error)) if error == "endpoint read failed")
        );
        state.record_live(LiveState::disconnected());
        assert_eq!(
            state.live.endpoint(MICROPHONE),
            Some(&Reading::Disconnected)
        );
        state.record_live(LiveState::default());
        assert_eq!(state.live.endpoint(MICROPHONE), Some(&Reading::Unobserved));
        let mut reconnected = LiveState::default();
        reconnected
            .observe_endpoint(
                MICROPHONE,
                EndpointObservation {
                    level: 61,
                    muted: true,
                },
            )
            .unwrap();
        state.record_live(reconnected);
        assert_eq!(
            state
                .live
                .endpoint(MICROPHONE)
                .unwrap()
                .value()
                .unwrap()
                .level,
            61
        );
        assert_eq!(state.monitor_preview[0]["value"], 82);
    }
    #[test]
    fn seiren_monitor_changed_source_value_replaces_temporary_drag_value() {
        let mut state = State::default();
        state.sync_monitor(0, serde_json::json!({"value":37,"isEnabled":true}));
        state.monitor_preview[0]["value"] = serde_json::json!(82);
        state.sync_monitor(0, serde_json::json!({"value":37,"isEnabled":true}));
        assert_eq!(state.monitor_preview[0]["value"], 82);
        state.sync_monitor(0, serde_json::json!({"value":61,"isEnabled":false}));
        assert_eq!(
            state.monitor_preview[0],
            serde_json::json!({"value":61,"isEnabled":false})
        );
    }
    #[test]
    fn seiren_eq_filter_refresh_preserves_navigation_until_value_changes() {
        let mut state = State::default();
        state.sync_filter("peaking");
        state.set_filter_open(true);
        state.filter_cursor = 4;
        state.sync_filter("peaking");
        assert!(state.filter_open);
        assert_eq!(state.filter_cursor, 4);
        state.sync_filter("lowpass");
        assert!(!state.filter_open);
        assert_eq!(state.filter_cursor, 0);
    }

    #[test]
    fn seiren_eq_filter_reopening_discards_uncommitted_cursor() {
        let mut state = State::default();
        state.sync_filter("highpass");
        state.set_filter_open(true);
        state.filter_cursor = 3;
        state.filter_keyboard_navigation = true;
        state.set_filter_open(false);
        state.set_filter_open(true);
        assert_eq!(state.filter_cursor, 1);
        assert!(!state.filter_keyboard_navigation);
        assert_eq!(state.filter_value.as_deref(), Some("highpass"));
    }

    #[test]
    fn seiren_eq_inactive_capture_clears_spectrum_without_changing_meter_observations() {
        let mut state = State::default();
        state.output_sample_rate = Some(48_000);
        state.capture_sample_rate = Some(48_000);
        state.bins = vec![-10.; 4096];
        state.levels = [[0.25; 2]; 2];
        state.spectrum.push(&[0.5; 64]);
        state.resampler.push(&[0.25; 64], 96_000, 48_000);
        assert!(state.clear_capture());
        assert!(state.bins.is_empty());
        assert!(state.spectrum.bins().is_none());
        assert_eq!(state.capture_sample_rate, None);
        assert_eq!(state.output_sample_rate, Some(48_000));
        assert_eq!(state.levels, [[0.25; 2]; 2]);
        assert!(!state.clear_capture());
        assert!(state.resampler.push(&[0.], 96_000, 48_000).is_empty());
    }
}
fn effects_defaults() -> Value {
    let mut effects = json!({});
    for id in model::EFFECTS {
        effects[id] = json!({"isEnabled":true,"value":50,"min":0,"max":100,"step":1});
    }
    effects
}
fn panel(title: &str, tips: &str, width: f32, cx: &App) -> Div {
    surface::panel_with_help(
        t(title),
        surface::help_control(
            SharedString::from(format!("seiren-eq-help-{title}")),
            t(tips),
        ),
        cx,
    )
    .w(css(width))
    .min_w(css(width))
    .max_w(css(width))
    .font_family("Roboto")
    .bg(C::surface())
    .text_color(C::text())
}
fn source_button(id: impl Into<ElementId>, selected: bool) -> BaseButton {
    BaseButton::new(id)
        .selected(selected)
        .bg(if selected { C::active() } else { C::surface() })
        .border_1()
        .border_color(if selected { C::selected() } else { C::border() })
        .rounded(css(4.))
        .text_size(css(12.))
        .text_color(C::text())
        .hover(|style| style.border_color(C::selected()))
}

impl AudioProductWorkspace {
    pub(super) fn seiren_endpoint_available(&self, index: usize) -> bool {
        self.seiren_endpoint_value(index).is_some()
    }
    pub(super) fn seiren_microphone_volume_preview(&self) -> &Value {
        &self.seiren_eq.monitor_preview[0]
    }
    pub(super) fn preview_seiren_microphone_volume(&mut self, value: Value) {
        self.seiren_eq.monitor_preview[0] = value;
    }
    fn seiren_endpoint_value(&self, index: usize) -> Option<Value> {
        use razer_device::seiren_mixer::{HEADPHONES, MICROPHONE};
        let id = *[MICROPHONE, HEADPHONES].get(index)?;
        let value = self.seiren_eq.live.endpoint(id)?.value()?;
        Some(json!({"isEnabled": !value.muted, "value": value.level}))
    }
    pub(in crate::features) fn observe_seiren_live(
        &mut self,
        observed: razer_device::seiren_mixer::LiveState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.record_seiren_live(observed, cx);
        self.sync_seiren_eq(window, cx);
    }
    pub(in crate::features) fn record_seiren_live(
        &mut self,
        observed: razer_device::seiren_mixer::LiveState,
        cx: &mut Context<Self>,
    ) {
        if !supported(self.spec.product_id) {
            return;
        }
        if self.seiren_eq.record_live(observed) {
            self.clear_seiren_stream_mixer_meters();
        }
        self.defer_seiren_microphone_volume_sync(cx);
        cx.notify();
    }
    pub(super) fn seiren_microphone_native_levels(&self) -> [f32; 2] {
        self.seiren_eq.levels[0]
    }
    pub(super) fn initialize_seiren_eq_draft(&mut self) {
        if !supported(self.spec.product_id) {
            return;
        }
        self.draft["device"]["parametric"] =
            serde_json::to_value(Model::initial()).expect("source EQ model");
        self.draft["device"]["eqEffects"] = effects_defaults();
        self.draft["device"]["micAgc"] = json!({"isEnabled":false});
    }
    pub(super) fn initialize_seiren_eq(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !supported(self.spec.product_id) {
            return;
        }
        if self.spec.product_id == 1446 {
            self.seiren_eq.filter_focus = Some(cx.focus_handle());
            self.seiren_eq.filter_menu_focus = Some(cx.focus_handle());
            self.seiren_eq
                .subscriptions
                .push(cx.observe_window_activation(window, |this, window, cx| {
                    if !window.is_window_active() && this.seiren_eq.filter_open {
                        this.seiren_eq.set_filter_open(false);
                        cx.notify();
                    }
                }));
        }
        let properties = [
            ("frequency", 1000., (20., 20000., 1.), false, 5),
            ("q", 1., (0.3, 15., 0.1), true, 5),
            ("gain", 0., (-12., 12., 0.1), true, 5),
        ];
        for (name, value, range, decimal, length) in properties {
            let entity = cx.new(|cx| {
                number::Number::new(
                    value,
                    range,
                    decimal,
                    length,
                    number::Presentation::Property,
                    window,
                    cx,
                )
            });
            let subscription = cx.subscribe_in(
                &entity,
                window,
                move |this, _, event: &number::Changed, window, cx| {
                    if this.syncing {
                        return;
                    }
                    let id = this
                        .seiren_model()
                        .and_then(|model| model.active().and_then(|p| p.selected()).map(|b| b.id));
                    if let Some(id) = id {
                        this.seiren_eq_field(id, name, json!(event.0), true, window, cx);
                    }
                },
            );
            self.seiren_eq.numbers.insert(name.into(), entity);
            self.seiren_eq.subscriptions.push(subscription);
        }
        for (index, (name, path)) in MONITORS.into_iter().enumerate() {
            let volume = self.draft.pointer(path).cloned().unwrap_or_default();
            let value = volume["value"].as_f64().unwrap_or(0.);
            self.seiren_eq.monitor_preview[index] = volume.clone();
            self.seiren_eq.monitor_observed[index] = volume;
            let fader = cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(100.)
                    .step(1.)
                    .default_value(value as f32)
            });
            self.seiren_eq.subscriptions.push(cx.subscribe_in(
                &fader,
                window,
                move |this, _, event: &SliderEvent, window, cx| {
                    if this.syncing {
                        return;
                    }
                    let SliderEvent::Change(value) = event else {
                        return;
                    };
                    this.seiren_monitor_preview(index, f64::from(value.start()), window, cx);
                },
            ));
            self.seiren_eq.faders.push(fader);
            let display = if index == 0 {
                model::gain_db(value).round()
            } else {
                value
            };
            let range = if index == 0 { (4., 40., 4.) } else { (0., 100., 1.) };
            let entity = cx.new(|cx| {
                number::Number::new(
                    display,
                    range,
                    false,
                    3,
                    number::Presentation::Monitoring,
                    window,
                    cx,
                )
            });
            self.seiren_eq.subscriptions.push(cx.subscribe_in(
                &entity,
                window,
                move |this, _, event: &number::Changed, window, cx| {
                    if this.syncing {
                        return;
                    }
                    this.seiren_monitor_preview(
                        index,
                        if index == 0 {
                            model::gain_percent(event.0)
                        } else {
                            event.0
                        },
                        window,
                        cx,
                    );
                },
            ));
            self.seiren_eq.numbers.insert(name.into(), entity);
        }
        for id in model::EFFECTS {
            let state = cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(100.)
                    .step(1.)
                    .default_value(50.)
            });
            self.seiren_eq.subscriptions.push(cx.subscribe_in(
                &state,
                window,
                move |this, _, event: &SliderEvent, window, cx| {
                    if this.syncing {
                        return;
                    }
                    let SliderEvent::Change(value) = event else {
                        return;
                    };
                    this.seiren_effect_preview(id, f64::from(value.start()), window, cx);
                },
            ));
            self.seiren_eq.knobs.insert(id.into(), state);
        }
        if self.spec.product_id == 1422 {
            let choices = vec![
                Choice::localized("0", model::data().labels["u1l"].clone()),
                Choice::localized("1", model::data().labels["E5g"].clone()),
            ];
            let dial = cx.new(|cx| {
                SelectState::new(
                    choices,
                    Some(IndexPath::new(
                        self.draft["profile"]["dialControl"].as_u64().unwrap_or(0) as usize,
                    )),
                    window,
                    cx,
                )
            });
            self.seiren_eq.subscriptions.push(cx.subscribe_in(
                &dial,
                window,
                |this, _, event: &SelectEvent<Vec<Choice>>, window, cx| {
                    if this.syncing {
                        return;
                    }
                    if let SelectEvent::Confirm(Some(value)) = event {
                        if let Ok(value) = value.parse::<u8>() {
                            this.edit("/profile/dialControl", json!(value), window, cx);
                            this.submit_seiren_eq_payload(
                                "ON_SET_DIAL_CONTROL",
                                json!({"dialControl":value}),
                                cx,
                            );
                        }
                    }
                },
            ));
            self.seiren_eq.dial = Some(dial);
            let selected = self
                .seiren_model()
                .and_then(|model| {
                    model
                        .active()
                        .and_then(|preset| preset.selected())
                        .map(|band| band.kind.clone())
                })
                .and_then(|kind| model::FILTERS.iter().position(|value| *value == kind))
                .map(IndexPath::new);
            let filter = cx.new(|cx| SelectState::new(filter_choices(), selected, window, cx));
            self.seiren_eq.subscriptions.push(cx.subscribe_in(
                &filter,
                window,
                |this, _, event: &SelectEvent<Vec<Choice>>, window, cx| {
                    if this.syncing {
                        return;
                    }
                    if let SelectEvent::Confirm(Some(kind)) = event {
                        this.seiren_confirm_filter(kind, window, cx);
                    }
                },
            ));
            self.seiren_eq.filter = Some(filter);
        }
        self.seiren_eq.committed_eq = self.draft["device"]["parametric"].clone();
        self.seiren_eq.effects_committed = self.draft["device"]["eqEffects"].clone();
        self.seiren_eq.monitor_saved = std::array::from_fn(|index| {
            self.draft
                .pointer(MONITORS[index].1)
                .cloned()
                .unwrap_or_default()
        });
    }
    pub(super) fn restore_seiren_eq(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !supported(self.spec.product_id) {
            return;
        }
        self.dismiss_seiren_eq(cx);
        if let Some(model) = saved.and_then(|saved| Model::restore(&saved["device"]["parametric"]))
        {
            self.draft["device"]["parametric"] =
                serde_json::to_value(model).expect("restored source EQ");
        }
        if let Some(saved) = saved {
            for id in model::EFFECTS {
                let input = &saved["device"]["eqEffects"][id];
                if let (Some(enabled), Some(value)) = (
                    input["isEnabled"].as_bool(),
                    input["value"].as_f64().filter(|v| v.is_finite()),
                ) {
                    self.draft["device"]["eqEffects"][id]["isEnabled"] = json!(enabled);
                    self.draft["device"]["eqEffects"][id]["value"] =
                        json!(value.clamp(0., 100.).round());
                }
            }
            if let Some(enabled) = saved["device"]["micAgc"]["isEnabled"].as_bool() {
                self.draft["device"]["micAgc"]["isEnabled"] = json!(enabled);
            }
        }
        self.seiren_eq.committed_eq = self.draft["device"]["parametric"].clone();
        self.seiren_eq.effects_committed = self.draft["device"]["eqEffects"].clone();
        self.seiren_eq.monitor_saved = std::array::from_fn(|index| {
            self.draft
                .pointer(MONITORS[index].1)
                .cloned()
                .unwrap_or_default()
        });
        self.seiren_eq.monitor_observed = std::array::from_fn(|_| Value::Null);
        self.seiren_eq.effects_observed = Value::Null;
        self.sync_seiren_eq(window, cx);
    }
    pub(super) fn seiren_eq_snapshot(&self, snapshot: &mut Value) {
        if !supported(self.spec.product_id) {
            return;
        }
        if !self.seiren_eq.committed_eq.is_null() {
            snapshot["device"]["parametric"] = self.seiren_eq.committed_eq.clone();
        }
        if !self.seiren_eq.effects_committed.is_null() {
            snapshot["device"]["eqEffects"] = self.seiren_eq.effects_committed.clone();
        }
        // currentaudio_streamMixer explicitly marks the system mic/headphone
        // endpoint volume operations Storage_NoSave.
        for (index, (_, path)) in MONITORS[..2].iter().enumerate() {
            if let Some(target) = snapshot.pointer_mut(path) {
                *target = self.seiren_eq.monitor_saved[index].clone();
            }
        }
    }
    pub(super) fn dismiss_seiren_eq(&mut self, cx: &mut Context<Self>) {
        self.seiren_eq.monitor_tasks = std::array::from_fn(|_| None);
        self.seiren_eq.effects_throttle = None;
        self.seiren_eq.effects_commit = None;
        self.seiren_eq.drag = None;
        self.seiren_eq.hovered_band = None;
        self.seiren_eq.hovered_effect = None;
        self.seiren_eq.pressed_effect = None;
        self.seiren_eq.filter_open = false;
        self.seiren_eq.wrapper_bounds.set(Bounds::default());
        self.seiren_eq.peak_task = None;
        self.seiren_eq.sidetone_tooltip = None;
        self.seiren_eq.levels = [[0.; 2]; 2];
        self.seiren_eq.peaks = [[0.; 2]; 2];
        self.seiren_eq.clear_capture();
        self.seiren_eq.monitor_observed = std::array::from_fn(|_| Value::Null);
        self.seiren_eq.effects_observed = Value::Null;
        for number in self.seiren_eq.numbers.values() {
            number.update(cx, |number, _| number.cancel());
        }
    }
    pub(super) fn sync_seiren_eq(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !supported(self.spec.product_id) {
            return;
        }
        let syncing = self.syncing;
        self.syncing = true;
        if let Some(model) = self.seiren_model() {
            if let Some(band) = model.active().and_then(|p| p.selected()) {
                for (name, value, disabled) in [
                    ("frequency", band.frequency.round(), false),
                    ("q", if band.shelf() { 1. } else { band.q }, band.shelf()),
                    (
                        "gain",
                        if band.pass() { 0. } else { band.gain },
                        band.pass(),
                    ),
                ] {
                    if let Some(entity) = self.seiren_eq.numbers.get(name) {
                        entity.update(cx, |entity, cx| entity.sync(value, disabled, window, cx));
                    }
                }
                if let Some(filter) = &self.seiren_eq.filter {
                    if filter.read(cx).selected_value() != Some(&band.kind) {
                        filter.update(cx, |filter, cx| {
                            filter.set_selected_value(&band.kind, window, cx);
                        });
                    }
                } else {
                    self.seiren_eq.sync_filter(&band.kind);
                }
            }
        }
        let agc = self.draft["device"]["micAgc"]["isEnabled"] == true;
        let stream = self.draft["profile"]["streamMixerSettings"]["isStreamMixerEnabled"] == true;
        for (index, (name, path)) in MONITORS.into_iter().enumerate() {
            let value = if index < 2 {
                self.seiren_endpoint_value(index).unwrap_or(Value::Null)
            } else {
                self.draft.pointer(path).cloned().unwrap_or_default()
            };
            // UI6007 T mirrors changed props with useEffect even while its
            // 300 ms callback is pending. A temporary slider value has no
            // priority over a later observed endpoint update.
            self.seiren_eq.sync_monitor(index, value);
            let volume = &self.seiren_eq.monitor_preview[index];
            let value = volume["value"].as_f64().unwrap_or(0.);
            let disabled = (index < 2 && !self.seiren_endpoint_available(index))
                || volume["isEnabled"] != true
                || (index == 0 && agc)
                || (index == 2 && stream);
            if let Some(fader) = self.seiren_eq.faders.get(index) {
                fader.update(cx, |fader, cx| fader.set_value(value as f32, window, cx));
            }
            if let Some(entity) = self.seiren_eq.numbers.get(name) {
                entity.update(cx, |entity, cx| {
                    entity.sync(
                        if index == 0 {
                            model::gain_db(value).round()
                        } else {
                            value
                        },
                        disabled,
                        window,
                        cx,
                    )
                });
            }
        }
        let effects = &self.draft["device"]["eqEffects"];
        if self.seiren_eq.effects_observed != *effects {
            self.seiren_eq.effects_observed = effects.clone();
            self.seiren_eq.effects_local = effects.clone();
        }
        for (id, knob) in &self.seiren_eq.knobs {
            let value = self.seiren_eq.effects_local[id]["value"]
                .as_f64()
                .unwrap_or(50.);
            knob.update(cx, |knob, cx| knob.set_value(value as f32, window, cx));
        }
        if let Some(dial) = &self.seiren_eq.dial {
            let value = self.draft["profile"]["dialControl"]
                .as_u64()
                .unwrap_or(0)
                .to_string();
            dial.update(cx, |dial, cx| dial.set_selected_value(&value, window, cx));
        }
        self.syncing = syncing;
    }
    fn seiren_model(&self) -> Option<Model> {
        Model::from_draft(&self.draft["device"]["parametric"])
    }
    pub(super) fn seiren_eq_active_payload(&self) -> Option<Value> {
        let model = self.seiren_model()?;
        Some(model.active()?.payload(&self.seiren_eq.effects_committed))
    }
    fn seiren_set_model(
        &mut self,
        model: Model,
        commit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft["device"]["parametric"] =
            serde_json::to_value(model).expect("source EQ serialization");
        if commit {
            self.seiren_eq.committed_eq = self.draft["device"]["parametric"].clone();
            cx.emit(AudioProductChanged);
        }
        self.sync_seiren_eq(window, cx);
        cx.notify();
    }
    fn seiren_eq_field(
        &mut self,
        id: u8,
        field: &str,
        value: Value,
        commit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut model) = self.seiren_model() else {
            return;
        };
        let Some(band) = model
            .active_mut()
            .and_then(|p| p.bands.iter_mut().find(|b| b.id == id))
        else {
            return;
        };
        band.update(field, &value);
        let written = band.clone();
        self.seiren_set_model(model, commit, window, cx);
        if commit {
            self.seiren_submit_band(&written, cx);
        }
    }
    fn seiren_submit_band(&mut self, band: &Band, cx: &mut Context<Self>) {
        self.submit_seiren_eq_payload(
            "ON_SET_PARAMETRIC_EQ_SETTINGS",
            json!({"parametricEQ":band.payload()}),
            cx,
        );
    }
    fn seiren_select_band(&mut self, id: u8, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut model) = self.seiren_model() else {
            return;
        };
        if let Some(preset) = model.active_mut() {
            preset.selected = id;
        }
        self.seiren_set_model(model, false, window, cx);
    }
    pub(super) fn seiren_select_preset(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut model) = self.seiren_model() else {
            return;
        };
        if !model.data.iter().any(|p| p.preset == name) {
            return;
        }
        model.selected = name.into();
        self.seiren_eq.drag = None;
        let payload = model
            .active()
            .unwrap()
            .payload(&self.seiren_eq.effects_committed);
        self.seiren_set_model(model, true, window, cx);
        self.submit_seiren_eq_payload("ON_SET_ACTIVE_PARAMETRIC_EQ", payload, cx);
    }
    fn seiren_add_band(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut model) = self.seiren_model() else {
            return;
        };
        let Some(writes) = model.active_mut().and_then(|p| p.add()) else {
            return;
        };
        self.seiren_set_model(model, true, window, cx);
        for band in writes {
            self.seiren_submit_band(&band, cx);
        }
    }
    fn seiren_remove_band(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut model) = self.seiren_model() else {
            return;
        };
        let Some(removed) = model.active_mut().and_then(|p| p.remove_selected()) else {
            return;
        };
        self.seiren_set_model(model, true, window, cx);
        self.seiren_submit_band(&removed, cx);
    }
    fn seiren_reset(&mut self, flat: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut model) = self.seiren_model() else {
            return;
        };
        let Some(preset) = model.active_mut() else {
            return;
        };
        let writes = if flat { preset.flatten() } else { preset.reset() };
        let high_pass = json!(preset.high_pass);
        self.seiren_set_model(model, true, window, cx);
        for band in writes {
            self.seiren_submit_band(&band, cx);
        }
        if !flat {
            self.submit_seiren_eq_payload("ON_SET_HIGH_PASS_FILTER", high_pass, cx);
        }
    }
    fn seiren_high_pass(
        &mut self,
        field: &str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut model) = self.seiren_model() else {
            return;
        };
        let Some(preset) = model.active_mut() else {
            return;
        };
        if field == "isEnabled" {
            if let Some(value) = value.as_bool() {
                preset.high_pass.enabled = value;
            }
        } else if let Some(value) = value
            .as_u64()
            .and_then(|v| u8::try_from(v).ok())
            .filter(|v| model::HIGH_PASS.iter().any(|(n, _)| n == v))
        {
            preset.high_pass.value = value;
        }
        let payload = json!(preset.high_pass);
        self.seiren_set_model(model, true, window, cx);
        self.submit_seiren_eq_payload("ON_SET_HIGH_PASS_FILTER", payload, cx);
    }
    fn seiren_monitor_preview(
        &mut self,
        index: usize,
        value: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if (index < 2 && !self.seiren_endpoint_available(index))
            || self.seiren_eq.monitor_preview[index]["isEnabled"] != true
        {
            return;
        }
        self.seiren_eq.monitor_preview[index]["value"] = json!(value.clamp(0., 100.));
        let value = self.seiren_eq.monitor_preview[index].clone();
        self.seiren_eq.monitor_tasks[index] = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.seiren_eq.monitor_tasks[index] = None;
                this.seiren_monitor_commit(index, value, window, cx);
            });
        }));
        self.sync_seiren_eq(window, cx);
        cx.notify();
    }
    fn seiren_monitor_commit(
        &mut self,
        index: usize,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let changed = self
            .draft
            .pointer(MONITORS[index].1)
            .is_some_and(|old| old["isEnabled"] != value["isEnabled"]);
        let (operation, key) = match index {
            0 => (
                if changed {
                    "ON_SET_MIC_VOLUME_ENABLED"
                } else {
                    "ON_SET_MIC_VOLUME_VALUE"
                },
                "micVolume",
            ),
            1 => (
                if changed {
                    "ON_CHANGE_VOLUME_ENABLED"
                } else {
                    "ON_CHANGE_VOLUME_VALUE"
                },
                "volume",
            ),
            _ => ("ON_SET_MIC_SIDE_TONE", "micSideTone"),
        };
        if index < 2 {
            if let Some(target) = self.draft.pointer_mut(MONITORS[index].1) {
                *target = value.clone();
            }
            self.sync_seiren_eq(window, cx);
            cx.notify();
        } else {
            self.edit(MONITORS[index].1, value.clone(), window, cx);
        }
        self.submit_seiren_eq_payload(operation, json!({(key):value}), cx);
    }
    fn seiren_monitor_mute(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index < 2 && !self.seiren_endpoint_available(index) {
            return;
        }
        let mut value = self.seiren_eq.monitor_preview[index].clone();
        value["isEnabled"] = json!(value["isEnabled"] != true);
        self.seiren_eq.monitor_preview[index] = value.clone();
        // T's earlier debounced value remains pending when mute is immediate.
        self.seiren_monitor_commit(index, value, window, cx);
    }
    fn seiren_effect_preview(
        &mut self,
        id: &'static str,
        value: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.draft["device"]["eqEffects"][id]["isEnabled"] != true {
            return;
        }
        let mut next = self.draft["device"]["eqEffects"][id].clone();
        next["value"] = json!(value.clamp(0., 100.).round());
        self.seiren_eq.effects_local[id] = next.clone();
        if self.seiren_eq.effects_throttle.is_none() {
            self.draft["device"]["eqEffects"][id] = next.clone();
            self.seiren_eq.effects_throttle = Some(cx.spawn_in(window, async move |view, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(20))
                    .await;
                let _ = view.update_in(cx, |this, _, _| this.seiren_eq.effects_throttle = None);
            }));
        }
        self.seiren_eq.effects_commit = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.seiren_effect_commit(id, next, window, cx)
            });
        }));
        cx.notify();
    }
    fn seiren_effect_commit(
        &mut self,
        id: &str,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft["device"]["eqEffects"][id] = value.clone();
        self.seiren_eq.effects_committed[id] = value.clone();
        self.sync_seiren_eq(window, cx);
        cx.emit(AudioProductChanged);
        let operation = match id {
            "mic_deesser" => "ON_SET_MIC_DEESSER",
            "vocal_bass" => "ON_SET_VOCAL_BASS",
            _ => "ON_SET_VOCAL_EXCITER",
        };
        self.submit_seiren_eq_payload(
            operation,
            json!({"isEnabled":value["isEnabled"],"value":value["value"]}),
            cx,
        );
        cx.notify();
    }
    /// Live observations only. An absent device/capture clears the spectrum.
    pub fn observe_seiren_eq_audio(
        &mut self,
        output_sample_rate: Option<u32>,
        frequency_bins: Option<Vec<f32>>,
        microphone_levels: Option<[f32; 2]>,
        headphone_levels: Option<[f32; 2]>,
        cx: &mut Context<Self>,
    ) {
        if !supported(self.spec.product_id) {
            return;
        }
        self.seiren_eq.output_sample_rate = output_sample_rate.filter(|rate| *rate > 0);
        self.seiren_eq.bins = frequency_bins
            .filter(|bins| bins.len() == 4096)
            .unwrap_or_default();
        for (index, observed) in [microphone_levels, headphone_levels]
            .into_iter()
            .enumerate()
        {
            if let Some(observed) = observed {
                self.seiren_eq.levels[index] = observed;
            }
        }
        for values in &mut self.seiren_eq.levels {
            for value in values {
                if !value.is_finite() {
                    *value = 0.;
                }
                *value = value.clamp(0., 100.);
            }
        }
        for index in 0..2 {
            for channel in 0..2 {
                self.seiren_eq.peaks[index][channel] =
                    self.seiren_eq.peaks[index][channel].max(self.seiren_eq.levels[index][channel]);
            }
        }
        if self.seiren_eq.peak_task.is_none() {
            self.seiren_eq.peak_task = Some(cx.spawn(async move |view, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                    if view
                        .update(cx, |this, cx| {
                            for index in 0..2 {
                                for channel in 0..2 {
                                    let level = this.seiren_eq.levels[index][channel];
                                    this.seiren_eq.peaks[index][channel] =
                                        if level > this.seiren_eq.peaks[index][channel] {
                                            level
                                        } else {
                                            (this.seiren_eq.peaks[index][channel] - 0.5).max(0.)
                                        };
                                }
                            }
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }));
        }
        cx.notify();
    }
    /// Storage_NoSave endpoint readback refreshes controls only. It is never a
    /// local profile edit or a user mutation event.
    pub fn observe_seiren_native_levels(
        &mut self,
        levels: &razer_device::seiren_eq::NativeLevels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.record_seiren_native_levels(levels, cx);
        self.sync_seiren_eq(window, cx);
    }
    pub(super) fn record_seiren_native_levels(
        &mut self,
        levels: &razer_device::seiren_eq::NativeLevels,
        cx: &mut Context<Self>,
    ) {
        if !supported(self.spec.product_id) {
            return;
        }
        self.seiren_eq.record_native_levels(levels);
        self.defer_seiren_microphone_volume_sync(cx);
        cx.notify();
    }
    pub(super) fn observe_seiren_meter_event(&mut self, event: &Value, cx: &mut Context<Self>) {
        let Some(entries) = event
            .pointer("/levelMeters/meterInfo")
            .and_then(Value::as_array)
        else {
            return;
        };
        let mut microphone = None;
        let mut headphones = None;
        for entry in entries {
            let Some(id) = entry["deviceId"].as_u64().filter(|id| *id != 0) else {
                continue;
            };
            let Some(values) = entry["levelMeter"].as_array() else {
                continue;
            };
            let Some(left) = values
                .first()
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite())
            else {
                continue;
            };
            let right = values
                .get(1)
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite())
                .unwrap_or(left);
            let level = [left as f32, right as f32];
            match id {
                131072 => microphone = Some(level),
                65536 => headphones = Some(level),
                _ => {}
            }
        }
        if let Some(level) = microphone {
            self.set_seiren_microphone_external_level(level[0]);
        }
        let bins = (!self.seiren_eq.bins.is_empty()).then(|| self.seiren_eq.bins.clone());
        self.observe_seiren_eq_audio(
            self.seiren_eq.output_sample_rate,
            bins,
            microphone,
            headphones,
            cx,
        );
    }
    pub fn observe_seiren_eq_pcm(
        &mut self,
        output_sample_rate: Option<u32>,
        capture_sample_rate: u32,
        mono_pcm: &[f32],
        microphone_levels: Option<[f32; 2]>,
        headphone_levels: Option<[f32; 2]>,
        cx: &mut Context<Self>,
    ) {
        if !supported(self.spec.product_id) {
            return;
        }
        let analysis_rate = output_sample_rate
            .filter(|rate| *rate > 0)
            .unwrap_or(capture_sample_rate);
        if self.seiren_eq.capture_sample_rate != Some(analysis_rate) {
            self.seiren_eq.spectrum.clear();
        }
        self.seiren_eq.capture_sample_rate = Some(analysis_rate);
        let samples = self
            .seiren_eq
            .resampler
            .push(mono_pcm, capture_sample_rate, analysis_rate);
        self.seiren_eq.spectrum.push(&samples);
        let bins = self.seiren_eq.spectrum.bins();
        self.observe_seiren_eq_audio(
            output_sample_rate,
            bins,
            microphone_levels,
            headphone_levels,
            cx,
        );
    }

    pub(super) fn seiren_eq_page(
        &self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if key != "TAB_EQ" || !supported(self.spec.product_id) {
            return None;
        }
        let narrow =
            f32::from(window.viewport_size().width) <= 1200. * f32::from(window.rem_size()) / 16.;
        let row = || {
            h_flex()
                .min_w(css(1220.))
                .flex_grow(1.)
                .flex_shrink_0()
                .gap(css(20.))
                .justify_center()
                .items_stretch()
        };
        Some(
            surface::page_columns()
                .max_w(css(1920.))
                .when(narrow, |body| body.justify_start())
                .child(
                    row()
                        .child(self.seiren_microphone(cx))
                        .child(self.seiren_visualizer(cx)),
                )
                .child(
                    row()
                        .child(self.seiren_bands(cx))
                        .child(self.seiren_properties(window, cx))
                        .child(self.seiren_effects(cx)),
                )
                .into_any_element(),
        )
    }
    fn seiren_visualizer(&self, cx: &mut Context<Self>) -> AnyElement {
        let model = self.seiren_model();
        let selected = model
            .as_ref()
            .map(|model| model.selected.as_str())
            .unwrap_or("podcast");
        let default = model
            .as_ref()
            .and_then(Model::active)
            .is_some_and(|preset| preset.is_default());
        let tabs = h_flex().items_center().gap(css(10.)).children(
            model::PRESETS
                .into_iter()
                .zip(["k5I", "$wb", "Et3", "SFW", "dRK"])
                .map(|(name, alias)| {
                    source_button(
                        SharedString::from(format!("seiren-eq-preset-{name}")),
                        name == selected,
                    )
                    .accessibility_label(label(alias))
                    .min_w(css(150.))
                    .min_h(css(36.))
                    .px(css(12.))
                    .py(css(6.))
                    .child(label(alias).to_uppercase())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.seiren_select_preset(name, window, cx)
                    }))
                }),
        );
        panel(
            "PARAMETRIC_EQUALIZER",
            &model::data().labels["qZE"],
            1000.,
            cx,
        )
        .pb(css(20.))
        .mb(css(20.))
        .child(
            v_flex()
                .relative()
                .w(css(1000.))
                .min_w(css(1000.))
                .pb(css(20.))
                .mb(css(20.))
                .child(
                    h_flex()
                        .w(css(905.))
                        .h(css(40.))
                        .items_center()
                        .justify_between()
                        .mb(css(20.))
                        .child(tabs)
                        .child(
                            BaseButton::new("seiren-eq-reset")
                                .accessibility_label(label("Odj"))
                                .disabled(default)
                                .opacity(if default { 0.3 } else { 1. })
                                .flex()
                                .items_center()
                                .gap(css(5.))
                                .p_0()
                                .text_size(css(14.))
                                .text_color(C::text())
                                .hover(|style| style.text_color(C::selected()))
                                .child(
                                    razer_assets::resource_svg()
                                        .path("synapse/seiren-eq-reset.svg")
                                        .size(css(20.)),
                                )
                                .child(div().underline().child(label("Odj")))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.seiren_reset(false, window, cx)
                                })),
                        ),
                )
                .child(self.seiren_eq_graph(cx)),
        )
        .into_any_element()
    }
    fn seiren_bands(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = h_flex().flex_wrap().gap_x(css(10.)).w(css(120.));
        if let Some(model) = self.seiren_model() {
            if let Some(preset) = model.active() {
                let mut bands = preset.bands.clone();
                bands.sort_by_key(|band| band.id);
                for band in bands {
                    let id = band.id;
                    body = body.child(
                        source_button(("seiren-eq-band", u64::from(id)), preset.selected == id)
                            .w(css(55.))
                            .h(css(27.))
                            .mb(css(10.))
                            .rounded(css(3.))
                            .bg(C::surface())
                            .text_color(C::band(id))
                            .when(preset.selected == id, |button| {
                                button.bg(C::active()).border_color(C::focus())
                            })
                            .child((id + 1).to_string())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.seiren_select_band(id, window, cx)
                            })),
                    );
                }
                if preset.bands.len() < 8 {
                    if let Some(id) = preset.first_unused() {
                        body = body.child(
                            source_button("seiren-eq-add-band", false)
                                .w(css(55.))
                                .h(css(27.))
                                .mb(css(10.))
                                .text_color(C::band(id))
                                .child("+")
                                .accessibility_label(t("ADD"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.seiren_add_band(window, cx)
                                })),
                        );
                    }
                }
            }
        }
        panel("EQ_BAND", &model::data().labels["jgQ"], 200., cx)
            .child(body)
            .into_any_element()
    }
    fn seiren_properties(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut body = v_flex().gap(css(10.));
        if let Some(model) = self.seiren_model() {
            if let Some(preset) = model.active() {
                if let Some(band) = preset.selected() {
                    body = body.child(
                        h_flex()
                            .items_center()
                            .gap(css(10.))
                            .child(div().child(label("aag").to_uppercase()))
                            .child(self.seiren_filter_select(band, window, cx))
                            .child(
                                BaseButton::new("seiren-eq-filter-tutorial")
                                    .p_0()
                                    .size(css(14.))
                                    .child(surface::help_control(
                                        "seiren-eq-filter-type-help",
                                        label("tiu"),
                                    ))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.seiren_eq.tutorial_open = true;
                                        cx.notify();
                                    })),
                            ),
                    );
                    for (field, alias, tips, unit) in [
                        ("frequency", "Kde", "WwL", "Hz"),
                        ("q", "ynE", "lgi", ""),
                        ("gain", "B$k", "tkd", ""),
                    ] {
                        if let Some(number) = self.seiren_eq.numbers.get(field) {
                            body = body.child(
                                h_flex()
                                    .gap(css(10.))
                                    .items_center()
                                    .child(div().child(label(alias).to_uppercase()))
                                    .child(
                                        h_flex()
                                            .gap(css(8.))
                                            .pt(css(2.))
                                            .relative()
                                            .child(number.clone())
                                            .when(!unit.is_empty(), |row| {
                                                row.child(
                                                    div()
                                                        .absolute()
                                                        .right(css(38.))
                                                        .top(css(7.))
                                                        .text_size(css(14.))
                                                        .child(unit),
                                                )
                                            })
                                            .child(surface::help_control(
                                                SharedString::from(format!(
                                                    "seiren-eq-property-help-{field}"
                                                )),
                                                label(tips),
                                            )),
                                    ),
                            );
                        }
                    }
                }
                body = body.child(
                    BaseButton::new("seiren-eq-remove-band")
                        .accessibility_label(label("oJm"))
                        .disabled(preset.bands.len() <= 1)
                        .opacity(if preset.bands.len() <= 1 { 0.5 } else { 1. })
                        .absolute()
                        .right(css(20.))
                        .bottom(css(44.))
                        .px(css(16.))
                        .py(css(8.))
                        .text_size(css(14.))
                        .line_height(css(16.))
                        .text_color(C::text())
                        .underline()
                        .hover(|style| style.text_color(C::selected()))
                        .child(label("oJm").to_uppercase())
                        .on_click(
                            cx.listener(|this, _, window, cx| this.seiren_remove_band(window, cx)),
                        ),
                );
            }
        }
        panel("EQ_PROPERTIES", &model::data().labels["jlp"], 400., cx)
            .relative()
            .child(body)
            .into_any_element()
    }
    fn seiren_filter_select(
        &self,
        band: &Band,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(filter) = &self.seiren_eq.filter {
            let images = model::data()
                .filters
                .iter()
                .map(|filter| {
                    (
                        filter.kind.clone(),
                        SelectOptionDecoration::new().content_image(
                            filter.asset.clone(),
                            filter.width,
                            filter.height,
                        ),
                    )
                })
                .collect();
            return synapse_select::select_pointer(filter)
                .id("seiren-eq-filter-select")
                .items(filter_choices())
                .pointer_option_decorations(images)
                .pointer_style(
                    PointerSelectStyle::new()
                        .trigger_height(css(35.))
                        .row_height(css(32.)),
                )
                .pointer_placement(PointerSelectPlacement::ViewportBoundary)
                .accessibility_label(label("aag"))
                .w(css(96.))
                .into_any_element();
        }
        let Some(focus) = &self.seiren_eq.filter_focus else {
            return div().into_any_element();
        };
        let Some(menu_focus) = &self.seiren_eq.filter_menu_focus else {
            return div().into_any_element();
        };
        let owner = cx.entity().downgrade();
        let open_owner = owner.clone();
        let trigger_owner = owner.clone();
        let confirm_owner = owner.clone();
        let selected = model::data()
            .filters
            .iter()
            .find(|filter| filter.kind == band.kind)
            .unwrap_or(&model::data().filters[2]);
        let select = BaseSelect::new("seiren-eq-filter-select")
            .open(self.seiren_eq.filter_open)
            .focus_handle(focus)
            .content_focus_handle(menu_focus)
            .accessibility_label(label("aag"))
            .accessibility_value(band.kind.clone())
            .w(css(96.))
            .h(css(35.))
            .on_open_change(move |open, _, cx| {
                let _ = trigger_owner.update(cx, |this, cx| {
                    this.seiren_eq.set_filter_open(open);
                    cx.notify();
                });
            })
            .on_confirm(move |window, cx| {
                let _ = confirm_owner.update(cx, |this, cx| {
                    let kind = model::FILTERS[this.seiren_eq.filter_cursor];
                    this.seiren_confirm_filter(kind, window, cx);
                });
            });
        let trigger = h_flex()
            .id("seiren-eq-filter-trigger")
            .w(css(96.))
            .h(css(35.))
            .px(css(5.))
            .py(css(4.))
            .items_center()
            .justify_center()
            .border_1()
            .border_color(if self.seiren_eq.filter_open {
                C::selected()
            } else {
                cx.theme().input
            })
            .hover(|style| style.border_color(C::selected()))
            .child(
                img(SharedString::from(selected.asset.clone()))
                    .w(css(selected.width))
                    .h(css(selected.height)),
            );
        let cursor = self.seiren_eq.filter_cursor;
        let keyboard_navigation = self.seiren_eq.filter_keyboard_navigation;
        let selected_kind = band.kind.clone();
        let trigger_focus = focus.clone();
        let menu_focus = menu_focus.clone();
        let arrow_angle = motion::transition(
            "seiren-filter-arrow",
            if self.seiren_eq.filter_open {
                std::f32::consts::PI
            } else {
                0.
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::Ease),
            window,
            cx,
        );
        let popup = Popover::new("seiren-eq-filter-popup")
            .open(self.seiren_eq.filter_open)
            .track_focus(&menu_focus)
            .offset(css(1.).to_pixels(window.rem_size()))
            .trigger_with(move |_, window, _| {
                trigger
                    .when(trigger_focus.is_focused(window), |trigger| {
                        trigger.border_color(C::selected())
                    })
                    .child(
                        h_flex()
                            .ml_auto()
                            .w(css(29.))
                            .h(css(25.))
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .child(
                                razer_assets::resource_svg()
                                    .flex_shrink_0()
                                    .path("synapse/expand.svg")
                                    .size(css(10.))
                                    .with_transformation(Transformation::rotate(radians(
                                        arrow_angle,
                                    ))),
                            ),
                    )
                    .into_any_element()
            })
            .on_open_change(move |open, window, cx| {
                let _ = open_owner.update(cx, |this, cx| {
                    if !*open
                        && window.is_window_active()
                        && this
                            .seiren_eq
                            .filter_menu_focus
                            .as_ref()
                            .is_some_and(|focus| focus.contains_focused(window, cx))
                    {
                        if let Some(focus) = &this.seiren_eq.filter_focus {
                            focus.focus(window, cx);
                        }
                    }
                    this.seiren_eq.set_filter_open(*open);
                    cx.notify();
                });
            })
            .content(move |_, window, cx| {
                razer_assets::resource_scope({
                    let up = owner.clone();
                    let down = owner.clone();
                    let confirm = owner.clone();
                    let cancel = owner.clone();
                    let colors = razer_widgets::theme::DropdownColors::new();
                    let reveal = Presence::new("seiren-filter-options-reveal", true)
                        .transition(
                            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
                        )
                        .sample(window, cx)
                        .progress;
                    let options = v_flex()
                        .id("seiren-eq-filter-options")
                        .role(Role::ListBox)
                        .track_focus(&menu_focus)
                        .key_context("Select")
                        .w(css(96.))
                        .h(css(162.0_f32.min(180. * reveal)))
                        .overflow_hidden()
                        .border_1()
                        .border_color(cx.theme().input)
                        .bg(colors.surface())
                        .on_action(move |_: &gpui_kit::base::actions::Cancel, window, cx| {
                            cx.stop_propagation();
                            let _ = cancel.update(cx, |this, cx| {
                                this.seiren_eq.set_filter_open(false);
                                if let Some(focus) = &this.seiren_eq.filter_focus {
                                    focus.focus(window, cx);
                                }
                                cx.notify();
                            });
                        })
                        .on_action(move |_: &gpui_kit::base::actions::Confirm, window, cx| {
                            cx.stop_propagation();
                            let _ = confirm.update(cx, |this, cx| {
                                let kind = model::FILTERS[this.seiren_eq.filter_cursor];
                                this.seiren_confirm_filter(kind, window, cx);
                            });
                        })
                        .on_action(move |_: &gpui_kit::base::actions::SelectUp, _, cx| {
                            let _ = up.update(cx, |this, cx| {
                                this.seiren_eq.filter_keyboard_navigation = true;
                                this.seiren_eq.filter_cursor =
                                    (this.seiren_eq.filter_cursor + 4) % 5;
                                cx.notify();
                            });
                        })
                        .on_action(move |_: &gpui_kit::base::actions::SelectDown, _, cx| {
                            let _ = down.update(cx, |this, cx| {
                                this.seiren_eq.filter_keyboard_navigation = true;
                                this.seiren_eq.filter_cursor =
                                    (this.seiren_eq.filter_cursor + 1) % 5;
                                cx.notify();
                            });
                        })
                        .children(model::data().filters.iter().enumerate().map(
                            |(index, filter)| {
                                let owner = owner.clone();
                                let kind = filter.kind.clone();
                                BaseButton::new(SharedString::from(format!(
                                    "seiren-eq-filter-option-{kind}"
                                )))
                                .focusable(false)
                                .selected(filter.kind == selected_kind)
                                .accessibility_label(kind.clone())
                                .role(Role::ListBoxOption)
                                .aria_selected(filter.kind == selected_kind)
                                .when(index == cursor, |option| option.aria_active_descendant())
                                .w_full()
                                .h(css(32.))
                                .min_h(css(32.))
                                .flex_shrink_0()
                                .overflow_hidden()
                                .p(css(8.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .when(keyboard_navigation && index == cursor, |option| {
                                    option.bg(colors.hover())
                                })
                                .hover(|style| style.bg(colors.hover()))
                                .child(
                                    img(SharedString::from(filter.asset.clone()))
                                        .w(css(filter.width))
                                        .h(css(filter.height)),
                                )
                                .on_click(
                                    move |_, window, cx| {
                                        let _ = owner.update(cx, |this, cx| {
                                            this.seiren_confirm_filter(&kind, window, cx);
                                        });
                                    },
                                )
                            },
                        ));
                    div().w(css(96.)).h(css(162.)).child(options)
                })
            });
        select.child(popup).into_any_element()
    }
    fn seiren_confirm_filter(&mut self, kind: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id == 1446 {
            self.seiren_eq.set_filter_open(false);
        }
        if let Some(id) = self.seiren_model().and_then(|model| {
            model
                .active()
                .and_then(|preset| preset.selected())
                .map(|band| band.id)
        }) {
            self.seiren_eq_field(id, "type", json!(kind), true, window, cx);
        }
        if let Some(focus) = self.seiren_eq.filter_focus.clone() {
            window.defer(cx, move |window, cx| {
                if window.is_window_active() {
                    focus.focus(window, cx);
                }
            });
        }
        cx.notify();
    }
    fn seiren_effects(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut knobs = h_flex().justify_between();
        for (id, alias) in model::EFFECTS.into_iter().zip(["J5p", "cFh", "BAn"]) {
            if let Some(state) = self.seiren_eq.knobs.get(id) {
                let enabled = self.draft["device"]["eqEffects"][id]["isEnabled"] == true;
                let footer = h_flex()
                    .gap(css(10.))
                    .items_center()
                    .justify_center()
                    .mt(css(10.))
                    .text_size(css(14.))
                    .child(label(alias))
                    .child(
                        surface::SynapseSwitch::new(SharedString::from(format!(
                            "seiren-eq-effect-{id}"
                        )))
                        .checked(enabled)
                        .accessibility_label(label(alias))
                        .on_change(cx.listener(
                            move |this, enabled, window, cx| {
                                let mut next = this.draft["device"]["eqEffects"][id].clone();
                                next["isEnabled"] = json!(*enabled);
                                this.seiren_effect_commit(id, next, window, cx);
                            },
                        )),
                    );
                knobs = knobs.child(
                    div()
                        .id(SharedString::from(format!("seiren-eq-effect-hover-{id}")))
                        .on_hover(cx.listener(move |this, hovered, _, cx| {
                            if this.seiren_eq.pressed_effect.is_none() {
                                this.seiren_eq.hovered_effect =
                                    if *hovered { Some(id) } else { None };
                                cx.notify();
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.seiren_eq.pressed_effect = Some(id);
                                cx.notify();
                            }),
                        )
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.seiren_eq.pressed_effect = None;
                                this.seiren_eq.hovered_effect = Some(id);
                                cx.notify();
                            }),
                        )
                        .on_mouse_up_out(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.seiren_eq.pressed_effect = None;
                                this.seiren_eq.hovered_effect = None;
                                cx.notify();
                            }),
                        )
                        .child(
                            knob::SeirenKnob::new(state, label(alias), "", enabled)
                                .footer(footer.into_any_element()),
                        ),
                );
            }
        }
        let mut body = v_flex().child(knobs);
        if let Some(model) = self.seiren_model() {
            if let Some(preset) = model.active() {
                let enabled = preset.high_pass.enabled;
                body = body.child(
                    v_flex()
                        .id("seiren-eq-highpass")
                        .mt(css(20.))
                        .on_hover(cx.listener(|this, hovered, _, cx| {
                            if this.seiren_eq.pressed_effect.is_none() {
                                this.seiren_eq.hovered_effect =
                                    if *hovered { Some("high_pass_filter") } else { None };
                                cx.notify();
                            }
                        }))
                        .child(
                            h_flex().gap(css(5.)).child(label("bwm")).child(
                                surface::SynapseSwitch::new("seiren-eq-highpass-enabled")
                                    .checked(enabled)
                                    .accessibility_label(label("bwm"))
                                    .on_change(cx.listener(|this, value, window, cx| {
                                        this.seiren_high_pass(
                                            "isEnabled",
                                            json!(*value),
                                            window,
                                            cx,
                                        )
                                    })),
                            ),
                        )
                        .child(
                            h_flex()
                                .gap(css(10.))
                                .mt(css(10.))
                                .opacity(if enabled { 1. } else { 0.3 })
                                .children(model::HIGH_PASS.into_iter().map(
                                    |(value, frequency)| {
                                        source_button(
                                            ("seiren-eq-highpass-frequency", u64::from(value)),
                                            preset.high_pass.value == value,
                                        )
                                        .disabled(!enabled)
                                        .px(css(16.))
                                        .py(css(7.))
                                        .child(format!("{frequency}Hz"))
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.seiren_high_pass(
                                                    "value",
                                                    json!(value),
                                                    window,
                                                    cx,
                                                )
                                            }),
                                        )
                                    },
                                )),
                        ),
                );
            }
        }
        panel("EQ_EFFECT", &model::data().labels["n$q"], 580., cx)
            .child(body)
            .into_any_element()
    }
    fn seiren_microphone(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut gain = v_flex()
            .items_center()
            .gap(css(10.))
            .w(css(63.))
            .h(css(325.))
            .px(css(5.))
            .py(css(10.))
            .bg(C::inset())
            .rounded(css(5.))
            .child(div().text_size(css(12.)).child(label("$Qz")));
        gain = gain.child(self.seiren_fader(0, cx));
        let monitoring = v_flex()
            .items_center()
            .gap(css(10.))
            .w(css(104.))
            .h(css(325.))
            .px(css(5.))
            .py(css(10.))
            .bg(C::inset())
            .rounded(css(5.))
            .child(div().text_size(css(12.)).child(label("Rp1")))
            .child(
                h_flex()
                    .items_start()
                    .gap(css(5.))
                    .child(self.seiren_fader(1, cx))
                    .child(self.seiren_fader(2, cx))
                    .child(
                        div()
                            .mt(css(232.))
                            .text_size(css(14.))
                            .line_height(css(14.))
                            .child("%"),
                    ),
            );
        let mut card = panel("MICROPHONE", &model::data().labels["yUD"], 200., cx)
            .px(css(20.))
            .pb(css(12.))
            .child(
                h_flex()
                    .items_start()
                    .justify_center()
                    .gap(css(10.))
                    .mt(css(-10.))
                    .child(gain)
                    .child(monitoring),
            );
        if let Some(dial) = &self.seiren_eq.dial {
            card = card.child(
                div()
                    .relative()
                    .mt(css(10.))
                    .w(css(160.))
                    .child(
                        synapse_select::select_pointer(dial)
                            .items(vec![
                                Choice::localized("0", model::data().labels["u1l"].clone()),
                                Choice::localized("1", model::data().labels["E5g"].clone()),
                            ])
                            .pointer_style(
                                PointerSelectStyle::new()
                                    .text_insets(css(30.), css(30.))
                                    .menu_padding_left(css(2.)),
                            )
                            .pointer_placement(PointerSelectPlacement::WrapperBoundary(
                                self.seiren_eq.wrapper_bounds.clone(),
                            ))
                            .accessibility_label(label("u1l")),
                    )
                    .child(
                        img("synapse/seiren-eq-dial.svg")
                            .absolute()
                            .left(css(6.))
                            .top(css(5.))
                            .w(css(15.))
                            .h(css(17.)),
                    ),
            );
        }
        card.into_any_element()
    }
    fn seiren_fader(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        graph::fader(self, index, cx)
    }
    /// Za remains mounted by the product root across all normal local pages.
    pub(in crate::features) fn seiren_eq_tutorial(
        &self,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !supported(self.spec.product_id) || !self.seiren_eq.tutorial_open {
            return None;
        }
        let selected = self.seiren_eq.tutorial_type;
        let types = [
            ("peaking", "aN0", "o7h", "h_T"),
            ("highpass", "VHj", "huC", "aMk"),
            ("lowpass", "noQ", "jRc", "aaF"),
            ("highshelf", "cam", "a0l", "hID"),
            ("lowshelf", "kgw", "a2B", "hKT"),
        ];
        let (_, title, first, second) = types[selected];
        Some(
            div()
                .absolute()
                .inset_0()
                .bg(C::overlay())
                .child(
                    h_flex()
                        .absolute()
                        .left(relative(0.5))
                        .top(relative(0.45))
                        .ml(css(-400.))
                        .mt(css(-210.))
                        .min_w(css(800.))
                        .min_h(css(420.))
                        .bg(C::surface())
                        .border(css(1.5))
                        .border_color(C::selected())
                        .rounded(css(3.))
                        .font_family("Roboto")
                        .text_size(css(14.))
                        .line_height(css(17.))
                        .text_color(C::text())
                        .child(
                            BaseButton::new("seiren-eq-tutorial-close-icon")
                                .absolute()
                                .right(css(10.))
                                .top(css(10.))
                                .p_0()
                                .size(css(24.))
                                .accessibility_label(label("Ndl"))
                                .child(img("synapse/seiren-eq-close.svg").size_full())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.seiren_eq.tutorial_open = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            v_flex()
                                .w(css(320.))
                                .max_h(css(420.))
                                .py(css(50.))
                                .pl(css(5.))
                                .child(
                                    img(SharedString::from(format!(
                                        "synapse/seiren-eq-tutorial-{}.png",
                                        selected + 1
                                    )))
                                    .w(css(300.))
                                    .h(css(191.)),
                                )
                                .child(h_flex().gap(css(10.)).mt(css(10.)).children(
                                    types.into_iter().enumerate().map(
                                        |(index, (kind, alias, _, _))| {
                                            source_button(
                                                ("seiren-eq-tutorial-filter", index as u64),
                                                selected == index,
                                            )
                                            .w(css(52.))
                                            .h(css(38.))
                                            .rounded(css(3.))
                                            .bg(C::surface())
                                            .accessibility_label(label(alias))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                img(SharedString::from(format!(
                                                    "synapse/seiren-eq-tutorial-{kind}.svg"
                                                )))
                                                .w(css([39., 36., 35., 35., 36.][index]))
                                                .h(css(17.)),
                                            )
                                            .on_click(
                                                cx.listener(move |this, _, _, cx| {
                                                    this.seiren_eq.tutorial_type = index;
                                                    cx.notify();
                                                }),
                                            )
                                        },
                                    ),
                                )),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .justify_between()
                                .py(css(50.))
                                .px(css(48.))
                                .child(
                                    v_flex()
                                        .gap(css(20.))
                                        .child(
                                            div()
                                                .text_size(css(20.))
                                                .line_height(css(20.))
                                                // The onboarding selector's
                                                // .1px tracking is inherited.
                                                .child(
                                                    SourceText::new(
                                                        "seiren-eq-tutorial-heading",
                                                        label("DYl").to_uppercase(),
                                                    )
                                                    .letter_spacing(0.1),
                                                ),
                                        )
                                        .child(
                                            v_flex()
                                                .gap(css(5.))
                                                .text_size(css(13.5))
                                                .line_height(css(17.))
                                                .child(
                                                    div()
                                                        .font_family("RazerF5")
                                                        .text_size(css(14.))
                                                        .line_height(css(14.))
                                                        .text_color(C::selected())
                                                        .child(
                                                            SourceText::new(
                                                                "seiren-eq-tutorial-filter-title",
                                                                label(title).to_uppercase(),
                                                            )
                                                            .letter_spacing(0.1),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            SourceText::new(
                                                "seiren-eq-tutorial-description-first",
                                                label(first),
                                            )
                                            .letter_spacing(0.1),
                                        )
                                        .child(
                                            SourceText::new(
                                                "seiren-eq-tutorial-description-second",
                                                label(second),
                                            )
                                            .letter_spacing(0.1),
                                        ),
                                )
                                .child(
                                    BaseButton::new("seiren-eq-tutorial-close")
                                        .px(css(16.))
                                        .py(css(8.))
                                        .bg(C::selected())
                                        .text_color(C::knob())
                                        .hover(|style| style.opacity(0.8))
                                        .child(
                                            SourceText::new(
                                                "seiren-eq-tutorial-close-label",
                                                label("Ndl"),
                                            )
                                            .letter_spacing(0.1),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.seiren_eq.tutorial_open = false;
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}

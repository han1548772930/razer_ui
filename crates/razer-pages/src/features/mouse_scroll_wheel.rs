//! Source-described scroll wheel editing with local drafts and independent observations.
use gpui_kit::base::{Button as BaseButton, Positioner, Tooltip};
use gpui_kit::component::{
    Disableable, h_flex,
    slider::{SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n::t;
use razer_widgets::source_slider::SourceSlider;
use razer_widgets::surface;
use razer_widgets::theme::ScrollWheelColors as Colors;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

const FIELDS: [&str; 6] = [
    "scrollMode",
    "disabledModes",
    "accelerationEnabled",
    "accelerationLevel",
    "smartReelEnabled",
    "smartReelLevel",
];
pub(super) const LOCAL_FIELDS: &str = "_scrollWheelLocalFieldsV1";

#[derive(Deserialize)]
struct Mode {
    id: String,
    label: String,
    #[serde(default, rename = "disableLabel")]
    disable_label: String,
}
#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Presentation {
    #[default]
    ThreeModes,
    TwoModeLevels,
    TwoModeSwitches,
}
#[derive(Deserialize)]
struct TwoModeStyle {
    background: u32,
    text: u32,
    selected_text: u32,
    height: f32,
    radius: f32,
    padding: f32,
    pill_height: f32,
    pill_radius: f32,
    pill_padding_x: f32,
    pill_padding_y: f32,
    message_margin: f32,
    switch_margin: f32,
    mode_margin: f32,
    twoway_margin: f32,
    uppercase: bool,
}
#[derive(Deserialize)]
pub(super) struct ScrollWheelSpec {
    product_id: u32,
    pub(super) profile_key: String,
    defaults: Value,
    modes: Vec<Mode>,
    level_min: f32,
    level_max: f32,
    level_step: f32,
    max_disabled_modes: usize,
    locking_mode: String,
    #[serde(default)]
    presentation: Presentation,
    #[serde(default)]
    two_mode_style: Option<TwoModeStyle>,
    #[serde(default)]
    help_key: String,
    #[serde(default)]
    mode_description: String,
    #[serde(default)]
    acceleration_descriptions: Vec<String>,
    #[serde(default)]
    smart_reel_descriptions: Vec<String>,
}
pub(super) fn source_spec(product_id: u32) -> Option<&'static ScrollWheelSpec> {
    static DATA: OnceLock<Vec<ScrollWheelSpec>> = OnceLock::new();
    DATA.get_or_init(|| {
        let specs: Vec<ScrollWheelSpec> =
            serde_json::from_str(include_str!("mouse_scroll_wheel_data.json"))
                .expect("validated current scroll wheel capabilities");
        for spec in &specs {
            assert!(
                spec.presentation == Presentation::TwoModeSwitches
                    || (spec.level_min.is_finite()
                        && spec.level_max.is_finite()
                        && spec.level_min < spec.level_max
                        && spec.level_step > 0.)
            );
            assert!(spec.max_disabled_modes < spec.modes.len());
            assert!(
                spec.locking_mode.is_empty()
                    || spec.modes.iter().any(|mode| mode.id == spec.locking_mode)
            );
            assert!(spec.presentation == Presentation::ThreeModes || spec.modes.len() == 2);
        }
        specs
    })
    .iter()
    .find(|spec| spec.product_id == product_id)
}

/// Projections of the six current reducer observation actions, not UI write acknowledgements.
/// A real transport publisher is still required. This module never invokes a device API.
#[derive(Clone)]
#[allow(dead_code)]
pub enum ScrollWheelObservation {
    Mode(String),
    DisabledModes(Vec<String>),
    Acceleration(bool),
    AccelerationLevel(u8),
    SmartReel(bool),
    SmartReelLevel(u8),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Feature {
    Acceleration,
    SmartReel,
}
impl Feature {
    fn id(self) -> &'static str {
        match self {
            Self::Acceleration => "wheel-acceleration",
            Self::SmartReel => "wheel-smart-reel",
        }
    }
    fn enabled(self) -> &'static str {
        match self {
            Self::Acceleration => "accelerationEnabled",
            Self::SmartReel => "smartReelEnabled",
        }
    }
    fn level(self) -> &'static str {
        match self {
            Self::Acceleration => "accelerationLevel",
            Self::SmartReel => "smartReelLevel",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Acceleration => "SCROLL_ACCELERATION",
            Self::SmartReel => "SMART_REEL",
        }
    }
    fn descriptions(self) -> &'static [&'static str] {
        match self {
            Self::Acceleration => &["SCROLL_ACCELERATION_LEVEL_DESC"],
            Self::SmartReel => &["SMART_REEL_DESC", "SMART_REEL_LEVEL_DESC"],
        }
    }
}

pub(super) struct Changed;
pub(super) struct ScrollWheelEditor {
    spec: &'static ScrollWheelSpec,
    draft: Value,
    /// Explicit local fields win over observation. Unedited observed values are
    /// never copied into the parent profile snapshot.
    local_fields: BTreeSet<String>,
    observed: Value,
    sliders: BTreeMap<Feature, Entity<SliderState>>,
    previewing: BTreeSet<Feature>,
    hovered_lock: Option<Feature>,
    tooltip: Option<Point<Pixels>>,
    subscriptions: Vec<Subscription>,
}
impl EventEmitter<Changed> for ScrollWheelEditor {}
impl ScrollWheelEditor {
    pub(super) fn new(
        spec: &'static ScrollWheelSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self {
            spec,
            draft: spec.defaults.clone(),
            local_fields: BTreeSet::new(),
            observed: json!({}),
            sliders: BTreeMap::new(),
            previewing: BTreeSet::new(),
            hovered_lock: None,
            tooltip: None,
            subscriptions: Vec::new(),
        };
        for feature in [Feature::Acceleration, Feature::SmartReel] {
            if spec.presentation == Presentation::TwoModeSwitches {
                break;
            }
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(spec.level_min)
                    .max(spec.level_max)
                    .step(spec.level_step)
                    .default_value(
                        spec.defaults[feature.level()]
                            .as_f64()
                            .unwrap_or(f64::from(spec.level_min)) as f32,
                    )
            });
            this.subscriptions.push(cx.subscribe_in(
                &slider,
                window,
                move |this, _, event, window, cx| {
                    if !this.active(feature) {
                        this.previewing.remove(&feature);
                        this.sync_slider(feature, window, cx);
                        return;
                    }
                    match event {
                        SliderEvent::Change(_) => {
                            this.previewing.insert(feature);
                            cx.notify();
                        }
                        SliderEvent::Release(value) => {
                            // Source module 130 with no callOnChangeOnEveryStep or
                            // debounceTime commits on mouse release, not every step.
                            if this.previewing.remove(&feature) {
                                this.edit(
                                    feature.level(),
                                    json!(
                                        value
                                            .start()
                                            .clamp(this.spec.level_min, this.spec.level_max)
                                            .round() as u8
                                    ),
                                );
                                this.changed(cx);
                            }
                        }
                    }
                },
            ));
            this.sliders.insert(feature, slider);
        }
        this.subscriptions
            .push(cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.deactivate(window, cx);
                }
            }));
        this
    }
    pub(super) fn snapshot(&self) -> Value {
        self.draft.clone()
    }
    pub(super) fn local_fields(&self) -> Value {
        json!(self.local_fields)
    }
    pub(super) fn restore(
        &mut self,
        saved: Option<&Value>,
        local_fields: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft = self.spec.defaults.clone();
        self.local_fields.clear();
        // Existing local scrollWheel data remains local; it is not a device read.
        if let Some(saved) = saved.and_then(Value::as_object) {
            self.draft
                .as_object_mut()
                .unwrap()
                .extend(saved.iter().map(|(k, v)| (k.clone(), v.clone())));
            self.local_fields.extend(
                FIELDS
                    .into_iter()
                    .filter(|k| saved.contains_key(*k) && self.spec.defaults.get(*k).is_some())
                    .map(str::to_owned),
            );
        }
        if let Some(fields) = local_fields.and_then(Value::as_array) {
            self.local_fields = fields
                .iter()
                .filter_map(Value::as_str)
                .filter(|field| FIELDS.contains(field) && self.spec.defaults.get(*field).is_some())
                .map(str::to_owned)
                .collect();
        }
        // Runtime profile values must be observed again for the newly selected profile.
        self.observed = json!({});
        self.deactivate(window, cx);
    }
    pub(super) fn deactivate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.previewing.clear();
        self.hovered_lock = None;
        self.tooltip = None;
        self.sync_sliders(window, cx);
        cx.notify();
    }
    fn value(&self, field: &str) -> &Value {
        if !self.local_fields.contains(field) {
            if let Some(value) = self.observed.get(field) {
                return value;
            }
        }
        &self.draft[field]
    }
    fn disabled_modes(&self) -> Vec<String> {
        if self.spec.max_disabled_modes == 0 {
            return Vec::new();
        }
        self.value("disabledModes")
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    }
    fn locked(&self) -> bool {
        self.disabled_modes()
            .iter()
            .any(|mode| mode == &self.spec.locking_mode)
    }
    fn active(&self, feature: Feature) -> bool {
        !self.feature_locked(feature) && self.value(feature.enabled()) == true
    }
    fn feature_locked(&self, feature: Feature) -> bool {
        self.locked()
            || (self.spec.presentation == Presentation::TwoModeSwitches
                && feature == Feature::SmartReel
                && self.value("scrollMode") == "FreeSpin")
    }
    fn edit(&mut self, field: &str, value: Value) {
        self.draft[field] = value;
        self.local_fields.insert(field.to_owned());
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(Changed);
        cx.notify();
    }
    fn sync_slider(&self, feature: Feature, window: &mut Window, cx: &mut Context<Self>) {
        let Some(slider) = self.sliders.get(&feature) else {
            return;
        };
        let value = self
            .value(feature.level())
            .as_f64()
            .unwrap_or(f64::from(self.spec.level_min))
            .clamp(
                f64::from(self.spec.level_min),
                f64::from(self.spec.level_max),
            ) as f32;
        slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
    }
    fn sync_sliders(&self, window: &mut Window, cx: &mut Context<Self>) {
        for feature in [Feature::Acceleration, Feature::SmartReel] {
            self.sync_slider(feature, window, cx);
        }
    }
    fn select_mode(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.spec.modes.iter().any(|mode| mode.id == id)
            || self.disabled_modes().iter().any(|mode| mode == id)
        {
            return;
        }
        // fe selects immediately; its 300ms debounce belongs to the deferred
        // service write request. This is an explicit, immediate local draft.
        let id = if self.spec.presentation == Presentation::ThreeModes {
            id.to_owned()
        } else {
            // Source TwoWay routes either half (including the selected half)
            // to the same toggle handler.
            if self.value("scrollMode").as_str() == Some(self.spec.modes[1].id.as_str()) {
                self.spec.modes[0].id.clone()
            } else {
                self.spec.modes[1].id.clone()
            }
        };
        self.edit("scrollMode", json!(id));
        self.changed(cx);
    }
    fn toggle_disabled(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.max_disabled_modes == 0 || !self.spec.modes.iter().any(|mode| mode.id == id) {
            return;
        }
        let mut modes = self.disabled_modes();
        if modes.iter().any(|mode| mode == id) {
            modes.retain(|mode| mode != id);
        } else {
            if modes.len() >= self.spec.max_disabled_modes {
                return;
            }
            modes.push(id.to_owned());
            if self.value("scrollMode") == id {
                if let Some(fallback) = self
                    .spec
                    .modes
                    .iter()
                    .find(|mode| !modes.contains(&mode.id))
                {
                    self.edit("scrollMode", json!(fallback.id));
                }
            }
        }
        self.edit("disabledModes", json!(modes));
        // Locking a feature never overwrites its enabled value in the draft.
        self.deactivate(window, cx);
        self.changed(cx);
    }
    fn toggle_feature(&mut self, feature: Feature, window: &mut Window, cx: &mut Context<Self>) {
        if self.feature_locked(feature) {
            return;
        }
        self.edit(feature.enabled(), json!(!self.active(feature)));
        self.previewing.remove(&feature);
        self.sync_slider(feature, window, cx);
        self.changed(cx);
    }
    pub(super) fn observe(
        &mut self,
        observation: ScrollWheelObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (field, value) = match observation {
            ScrollWheelObservation::Mode(mode) => ("scrollMode", json!(mode)),
            ScrollWheelObservation::DisabledModes(modes) => ("disabledModes", json!(modes)),
            ScrollWheelObservation::Acceleration(enabled) => {
                ("accelerationEnabled", json!(enabled))
            }
            ScrollWheelObservation::AccelerationLevel(level) => ("accelerationLevel", json!(level)),
            ScrollWheelObservation::SmartReel(enabled) => ("smartReelEnabled", json!(enabled)),
            ScrollWheelObservation::SmartReelLevel(level) => ("smartReelLevel", json!(level)),
        };
        if self.spec.defaults.get(field).is_none() {
            return;
        }
        self.observed[field] = value;
        if !self.local_fields.contains(field) {
            // A mode observation must not discard an unrelated level preview.
            for feature in [Feature::Acceleration, Feature::SmartReel] {
                if field == feature.level() {
                    self.sync_slider(feature, window, cx);
                } else if (field == "disabledModes" || field == feature.enabled())
                    && !self.active(feature)
                {
                    self.previewing.remove(&feature);
                    self.sync_slider(feature, window, cx);
                }
            }
            if !self.locked() {
                self.hovered_lock = None;
                self.tooltip = None;
            }
        }
        cx.notify();
    }
    fn lock_hover(&mut self, feature: Feature, hovered: bool, cx: &mut Context<Self>) {
        if hovered && self.locked() {
            self.hovered_lock = Some(feature);
            self.tooltip = None;
        } else if self.hovered_lock == Some(feature) {
            self.hovered_lock = None;
            self.tooltip = None;
        }
        cx.notify();
    }
    fn tooltip_move(&mut self, feature: Feature, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.hovered_lock == Some(feature) && self.locked() {
            self.tooltip = Some(position);
            cx.notify();
        }
    }
    fn section(&self, feature: Feature, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let legacy = self.spec.presentation == Presentation::TwoModeSwitches;
        let style = self.spec.two_mode_style.as_ref();
        let locked = self.feature_locked(feature);
        let active = self.active(feature);
        let opacity = if self.spec.presentation == Presentation::ThreeModes {
            surface::fade_opacity(feature.id(), if active { 1. } else { 0.4 }, 200, window, cx)
        } else {
            // Two-mode source mounts Slider directly; its own 0.3 fade is
            // sufficient. The additional 0.4 wrapper belongs to three modes.
            1.
        };
        let tag_opacity = surface::fade_opacity(
            (ElementId::from(feature.id()), "tags"),
            if active { 1. } else { 0.3 },
            300,
            window,
            cx,
        );
        let slider = self.sliders.get(&feature);
        let descriptions = match feature {
            Feature::Acceleration => &self.spec.acceleration_descriptions,
            Feature::SmartReel => &self.spec.smart_reel_descriptions,
        };
        let descriptions: Vec<&str> = if descriptions.is_empty() {
            feature.descriptions().to_vec()
        } else {
            descriptions.iter().map(String::as_str).collect()
        };
        v_flex()
            .id(feature.id())
            .test_support()
            .mt(surface::css(
                if legacy && feature == Feature::Acceleration {
                    0.
                } else {
                    20.
                },
            ))
            .when(
                self.spec.presentation == Presentation::TwoModeSwitches && locked,
                |view| view.opacity(0.3),
            )
            .child(
                h_flex()
                    .items_center()
                    .mb(surface::css(if style.is_some() { 0. } else { 6. }))
                    .child(
                        div()
                            .mr(surface::css(style.map_or(10., |style| style.switch_margin)))
                            .text_color(Colors::text())
                            .text_size(surface::css(14.))
                            .child(t(feature.title()).to_uppercase()),
                    )
                    .child(
                        div()
                            .id((ElementId::from(feature.id()), "switch-wrapper"))
                            .on_hover(cx.listener(move |this, hovered, _, cx| {
                                this.lock_hover(feature, *hovered, cx)
                            }))
                            .on_mouse_move(cx.listener(
                                move |this, event: &MouseMoveEvent, _, cx| {
                                    this.tooltip_move(feature, event.position, cx)
                                },
                            ))
                            .child(
                                surface::SynapseSwitch::new((
                                    ElementId::from(feature.id()),
                                    "switch",
                                ))
                                .accessibility_label(t(feature.title()))
                                .checked(
                                    if self.spec.presentation == Presentation::TwoModeSwitches {
                                        self.value(feature.enabled()) == true
                                    } else {
                                        active
                                    },
                                )
                                .disabled(locked)
                                .on_change(cx.listener(
                                    move |this, _, window, cx| {
                                        this.toggle_feature(feature, window, cx)
                                    },
                                )),
                            ),
                    ),
            )
            .children(descriptions.iter().map(|key| {
                div()
                    .mt(surface::css(style.map_or(0., |style| style.message_margin)))
                    .text_color(Colors::muted())
                    .text_size(surface::css(13.))
                    .line_height(surface::css(18.))
                    .child(t(key))
            }))
            .when_some(slider, |view, slider| {
                view.child(
                    div()
                        .id((ElementId::from(feature.id()), "slider-wrapper"))
                        .test_support()
                        .opacity(opacity)
                        .on_hover(cx.listener(move |this, hovered, _, cx| {
                            this.lock_hover(feature, *hovered, cx)
                        }))
                        .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                            this.tooltip_move(feature, event.position, cx)
                        }))
                        .child(
                            div()
                                .relative()
                                .mt(surface::css(10.))
                                .mb(surface::css(20.))
                                .h(surface::css(36.))
                                .child(
                                    SourceSlider::new(
                                        slider,
                                        (slider.read(cx).value().start() - self.spec.level_min)
                                            / (self.spec.level_max - self.spec.level_min),
                                    )
                                    .enabled(active),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .bottom(surface::css(-2.))
                                        .w_full()
                                        .opacity(tag_opacity)
                                        .text_color(Colors::muted())
                                        .child(surface::slider_tags(
                                            &t("LOW"),
                                            Some(&t("MEDIUM")),
                                            &t("HIGH"),
                                            None,
                                        )),
                                ),
                        ),
                )
            })
            .into_any_element()
    }
}
impl Render for ScrollWheelEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let two_mode = self.spec.presentation != Presentation::ThreeModes;
        let legacy = self.spec.presentation == Presentation::TwoModeSwitches;
        let style = self.spec.two_mode_style.as_ref();
        let disabled = self.disabled_modes();
        let selected = self.value("scrollMode").as_str().unwrap_or("").to_owned();
        let pills = h_flex()
            .items_center()
            .h(surface::css(style.map_or(36., |style| style.height)))
            .p(surface::css(style.map_or(4., |style| style.padding)))
            .mb(surface::css(if two_mode { 0. } else { 16. }))
            .border_1()
            .border_color(Colors::border())
            .rounded(surface::css(style.map_or(18., |style| style.radius)))
            .bg(style.map_or_else(Colors::pills, |style| rgb(style.background).into()))
            .overflow_hidden()
            .hover(|s| s.border_color(Colors::accent()))
            .children(self.spec.modes.iter().map(|mode| {
                let active = selected == mode.id;
                let id = mode.id.clone();
                let is_disabled = disabled.contains(&mode.id);
                let color = surface::fade_color(
                    SharedString::from(format!("scroll-wheel-pill-color-{id}")),
                    if active {
                        style.map_or_else(Colors::selected_text, |style| {
                            rgb(style.selected_text).into()
                        })
                    } else {
                        style.map_or_else(Colors::pill_text, |style| rgb(style.text).into())
                    },
                    200,
                    window,
                    cx,
                );
                BaseButton::new(SharedString::from(format!("scroll-wheel-pill-{id}")))
                    .accessibility_label(t(&mode.label))
                    .selected(active)
                    .disabled(is_disabled)
                    .opacity(if is_disabled { 0.4 } else { 1. })
                    .rounded(surface::css(style.map_or(14., |style| style.pill_radius)))
                    .px(surface::css(
                        style.map_or(16., |style| style.pill_padding_x),
                    ))
                    .py(surface::css(style.map_or(4., |style| style.pill_padding_y)))
                    .when_some(style, |button, style| {
                        button.h(surface::css(style.pill_height))
                    })
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(color)
                    .whitespace_nowrap()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(active, |b| b.bg(Colors::accent()))
                    .when(!is_disabled, |b| b.cursor_pointer())
                    .focus_visible(|b| b.underline())
                    .child(if style.is_some_and(|style| style.uppercase) {
                        t(&mode.label).to_uppercase()
                    } else {
                        t(&mode.label)
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.select_mode(&id, cx)))
            }));
        let checks = v_flex()
            .gap(surface::css(8.))
            .children(self.spec.modes.iter().map(|mode| {
                let checked = disabled.contains(&mode.id);
                let locked = disabled.len() >= self.spec.max_disabled_modes && !checked;
                let id = mode.id.clone();
                surface::check_item(
                    SharedString::from(format!("scroll-wheel-disable-{id}")),
                    t(&mode.disable_label),
                    checked,
                    locked,
                    window,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.toggle_disabled(&id, window, cx)),
                )
            }));
        let tooltip = self.tooltip.filter(|_| self.locked()).map(|position| {
            deferred(
                Positioner::corner(Anchor::TopLeft, position + point(px(16.), px(12.)))
                    .margin(px(0.))
                    .child(
                        Tooltip::new("scroll-wheel-locked-tooltip")
                            .max_w(surface::css(320.))
                            .bg(Colors::tooltip())
                            .border_1()
                            .border_color(Colors::border())
                            .text_color(Colors::text())
                            .font_family("Roboto")
                            .text_size(surface::css(14.))
                            .px(surface::css(10.))
                            .py(surface::css(8.))
                            .child(t("ENABLE_FREE_SPIN_TO_USE_FEATURE")),
                    ),
            )
            .with_priority(999999)
        });
        surface::panel_with_control(
            t("SCROLL_WHEEL"),
            surface::help_control(
                "scroll-wheel-help",
                t(if self.spec.help_key.is_empty() {
                    "SCROLL_WHEEL_TOOLTIP_V2"
                } else {
                    &self.spec.help_key
                }),
            ),
            cx,
        )
        .id("scroll-wheel-editor")
        .test_support()
        .role(Role::Group)
        .aria_label(t("SCROLL_WHEEL"))
        .font_family("Roboto")
        .when(legacy, |panel| {
            panel.child(self.section(Feature::Acceleration, window, cx))
        })
        .child(
            v_flex()
                .mt(surface::css(style.map_or(0., |style| style.mode_margin)))
                .mb(surface::css(if two_mode { 0. } else { 20. }))
                .child(
                    div()
                        .mb(surface::css(style.map_or(6., |style| style.message_margin)))
                        .text_color(Colors::text())
                        .text_size(surface::css(14.))
                        .child(t("SCROLL_MODE").to_uppercase()),
                )
                .child(
                    div()
                        .mb(surface::css(style.map_or(12., |style| style.twoway_margin)))
                        .text_color(Colors::muted())
                        .text_size(surface::css(13.))
                        .line_height(surface::css(18.))
                        .child(t(if self.spec.mode_description.is_empty() {
                            "SCROLL_THREE_MODE_DESC"
                        } else {
                            &self.spec.mode_description
                        })),
                )
                // inline-flex: keep the pill strip at intrinsic width.
                .child(h_flex().child(pills))
                .when(self.spec.max_disabled_modes > 0, |view| {
                    view.child(
                        div()
                            .mb(surface::css(10.))
                            .text_color(Colors::muted())
                            .text_size(surface::css(13.))
                            .child(t("SCROLL_DISABLE_MODES_DESC")),
                    )
                    .child(checks)
                })
                .when(
                    self.spec.presentation == Presentation::TwoModeLevels,
                    |view| {
                        view.child(
                            div()
                                .mt(surface::css(20.))
                                .h(surface::css(1.))
                                .bg(Colors::border()),
                        )
                    },
                ),
        )
        .when(
            self.spec.presentation == Presentation::ThreeModes,
            |panel| panel.child(self.section(Feature::Acceleration, window, cx)),
        )
        .child(self.section(Feature::SmartReel, window, cx))
        .when(
            self.spec.presentation == Presentation::TwoModeLevels,
            |panel| panel.child(self.section(Feature::Acceleration, window, cx)),
        )
        .child(
            div()
                .mt(surface::css(10.))
                .text_color(Colors::muted())
                .text_size(surface::css(12.))
                .child("本地草稿，尚未写入设备。"),
        )
        .children(tooltip)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Feature, Presentation, ScrollWheelEditor, ScrollWheelObservation, ScrollWheelSpec,
        source_spec,
    };
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, ElementId, TestAppContext, px, size};
    use serde_json::json;
    use std::sync::OnceLock;

    #[gpui_kit::test]
    fn two_mode_controls_toggle_selected_mode_and_keep_family_specific_smart_reel_lock(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let catalog: Vec<ScrollWheelSpec> =
            serde_json::from_str(include_str!("mouse_scroll_wheel_data.json")).unwrap();
        for presentation in [Presentation::TwoModeLevels, Presentation::TwoModeSwitches] {
            let pid = catalog
                .iter()
                .find(|spec| spec.presentation == presentation)
                .unwrap()
                .product_id;
            let spec = source_spec(pid).unwrap();
            let mut owner = None;
            let handle = cx.open_window(size(px(1000.), px(1100.)), |window, cx| {
                let editor = cx.new(|cx| ScrollWheelEditor::new(spec, window, cx));
                owner = Some(editor.clone());
                Root::new(editor, window, cx)
            });
            let owner = owner.unwrap();
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                assert!(window.try_find("scroll-wheel-disable-Tactile").is_none());
                assert!(window.try_find("scroll-wheel-pill-MicroTactile").is_none());
                let smart = (ElementId::from("wheel-smart-reel"), "switch");
                let slider = (ElementId::from("wheel-smart-reel"), "slider-wrapper");
                assert_eq!(
                    window.try_find(slider).is_some(),
                    presentation == Presentation::TwoModeLevels
                );
                assert_eq!(owner.read(cx).snapshot()["scrollMode"], "FreeSpin");
                if presentation == Presentation::TwoModeSwitches {
                    assert_eq!(window.find(smart.clone()).disabled(), Some(true));
                    window.click(smart.clone(), cx);
                    assert!(owner.read(cx).local_fields.is_empty());
                }
                // Clicking the selected half toggles to the other source mode.
                window.click("scroll-wheel-pill-FreeSpin", cx);
                assert_eq!(owner.read(cx).snapshot()["scrollMode"], "Tactile");
                window.click(smart.clone(), cx);
                assert_eq!(owner.read(cx).snapshot()["smartReelEnabled"], true);
                window.click("scroll-wheel-pill-Tactile", cx);
                assert_eq!(owner.read(cx).snapshot()["scrollMode"], "FreeSpin");
                assert_eq!(owner.read(cx).snapshot()["smartReelEnabled"], true);
                if presentation == Presentation::TwoModeSwitches {
                    assert_eq!(window.find(smart.clone()).disabled(), Some(true));
                    assert_eq!(window.find(smart.clone()).checked(), Some(true));
                    window.click(smart, cx);
                    assert_eq!(owner.read(cx).snapshot()["smartReelEnabled"], true);
                    assert!(owner.read(cx).snapshot().get("smartReelLevel").is_none());
                } else {
                    window.click(smart, cx);
                    assert_eq!(owner.read(cx).snapshot()["smartReelEnabled"], false);
                }
                assert!(owner.read(cx).snapshot().get("disabledModes").is_none());
            })
            .unwrap();
        }
    }

    #[gpui_kit::test]
    fn capability_controls_limits_locking_and_preserves_local_ownership(cx: &mut TestAppContext) {
        // Deliberately differs from the current product's mode lock, limit and
        // level range. This fixture is never a discovered or preview device.
        static FIXTURE: OnceLock<ScrollWheelSpec> = OnceLock::new();
        let spec = FIXTURE.get_or_init(|| serde_json::from_value(json!({
            "product_id": 0, "profile_key": "scrollWheel", "defaults": {
                "scrollMode": "Tactile", "disabledModes": [],
                "accelerationEnabled": true, "accelerationLevel": 7,
                "smartReelEnabled": true, "smartReelLevel": 3
            }, "modes": [
                {"id": "Tactile", "label": "TACTILE", "disableLabel": "DISABLE_TACTILE"},
                {"id": "FreeSpin", "label": "FREE_SPIN", "disableLabel": "DISABLE_FREE_SPIN"},
                {"id": "MicroTactile", "label": "MICRO_TACTILE", "disableLabel": "DISABLE_MICRO_TACTILE"}
            ], "level_min": 1, "level_max": 8, "level_step": 1,
            "max_disabled_modes": 1, "locking_mode": "MicroTactile"
        })).unwrap());
        cx.update(gpui_kit::init);
        cx.open_window(size(px(1000.), px(900.)), |window, cx| {
            let editor = cx.new(|cx| ScrollWheelEditor::new(spec, window, cx));
            editor.update(cx, |editor, cx| {
                editor.sync_sliders(window, cx);
                assert_eq!(
                    editor.sliders[&Feature::Acceleration]
                        .read(cx)
                        .value()
                        .start(),
                    7.
                );
                editor.toggle_disabled("Tactile", window, cx);
                assert_eq!(editor.value("scrollMode"), "FreeSpin");
                editor.toggle_disabled("MicroTactile", window, cx);
                assert_eq!(editor.disabled_modes(), vec!["Tactile"]);
                editor.toggle_disabled("Tactile", window, cx);
                editor.toggle_disabled("MicroTactile", window, cx);
                assert!(editor.locked());
                assert!(!editor.active(Feature::Acceleration));
                assert_eq!(editor.snapshot()["accelerationEnabled"], true);
                editor.toggle_disabled("MicroTactile", window, cx);
                assert!(editor.active(Feature::Acceleration));
                editor.observe(ScrollWheelObservation::AccelerationLevel(8), window, cx);
                assert_eq!(
                    editor.sliders[&Feature::Acceleration]
                        .read(cx)
                        .value()
                        .start(),
                    8.
                );
                // Observation affects presentation, never the saved local value.
                assert_eq!(editor.snapshot()["accelerationLevel"], 7);
                assert!(!editor.local_fields.contains("accelerationLevel"));
            });
            Root::new(editor, window, cx)
        });
    }
}

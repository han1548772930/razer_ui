//! Current 226 ordinary Customize → fe/Se, with local drafts and independent observations.
use crate::{
    i18n::t,
    ui::{source_slider::SourceSlider, surface, theme::ScrollWheelColors as Colors},
};
use gpui_kit::base::{Button as BaseButton, Positioner, Tooltip};
use gpui_kit::component::{
    Disableable, h_flex,
    slider::{SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
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
    #[serde(rename = "disableLabel")]
    disable_label: String,
}
#[derive(Deserialize)]
struct WheelData {
    defaults: Value,
    modes: Vec<Mode>,
}
fn data() -> &'static WheelData {
    static DATA: OnceLock<WheelData> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("mouse_226_scroll_data.json"))
            .expect("current 226 scroll data")
    })
}

/// Projections of the six current reducer observation actions, not UI write acknowledgements.
/// A real transport publisher is still required. This module never invokes a device API.
#[derive(Clone)]
#[allow(dead_code)]
pub(crate) enum ScrollWheelObservation {
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
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            draft: data().defaults.clone(),
            local_fields: BTreeSet::new(),
            observed: json!({}),
            sliders: BTreeMap::new(),
            previewing: BTreeSet::new(),
            hovered_lock: None,
            tooltip: None,
            subscriptions: Vec::new(),
        };
        for feature in [Feature::Acceleration, Feature::SmartReel] {
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(4.)
                    .step(1.)
                    .default_value(0.)
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
                                    json!(value.start().clamp(0., 4.).round() as u8),
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
        self.draft = data().defaults.clone();
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
                    .filter(|k| saved.contains_key(*k))
                    .map(str::to_owned),
            );
        }
        if let Some(fields) = local_fields.and_then(Value::as_array) {
            self.local_fields = fields
                .iter()
                .filter_map(Value::as_str)
                .filter(|field| FIELDS.contains(field))
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
        self.value("disabledModes")
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    }
    fn locked(&self) -> bool {
        self.disabled_modes().iter().any(|mode| mode == "FreeSpin")
    }
    fn active(&self, feature: Feature) -> bool {
        !self.locked() && self.value(feature.enabled()) == true
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
        let value = self
            .value(feature.level())
            .as_f64()
            .unwrap_or(0.)
            .clamp(0., 4.) as f32;
        self.sliders[&feature].update(cx, |slider, cx| slider.set_value(value, window, cx));
    }
    fn sync_sliders(&self, window: &mut Window, cx: &mut Context<Self>) {
        for feature in [Feature::Acceleration, Feature::SmartReel] {
            self.sync_slider(feature, window, cx);
        }
    }
    fn select_mode(&mut self, id: &str, cx: &mut Context<Self>) {
        if !data().modes.iter().any(|mode| mode.id == id)
            || self.disabled_modes().iter().any(|mode| mode == id)
        {
            return;
        }
        // fe selects immediately; its 300ms debounce belongs to the deferred
        // service write request. This is an explicit, immediate local draft.
        self.edit("scrollMode", json!(id));
        self.changed(cx);
    }
    fn toggle_disabled(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !data().modes.iter().any(|mode| mode.id == id) {
            return;
        }
        let mut modes = self.disabled_modes();
        if modes.iter().any(|mode| mode == id) {
            modes.retain(|mode| mode != id);
        } else {
            if modes.len() >= 2 {
                return;
            }
            modes.push(id.to_owned());
            if self.value("scrollMode") == id {
                if let Some(fallback) = data().modes.iter().find(|mode| !modes.contains(&mode.id)) {
                    self.edit("scrollMode", json!(fallback.id));
                }
            }
        }
        self.edit("disabledModes", json!(modes));
        // Never overwrite either enabled value when FreeSpin becomes disabled.
        self.deactivate(window, cx);
        self.changed(cx);
    }
    fn toggle_feature(&mut self, feature: Feature, window: &mut Window, cx: &mut Context<Self>) {
        if self.locked() {
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
        let locked = self.locked();
        let active = self.active(feature);
        let opacity =
            surface::fade_opacity(feature.id(), if active { 1. } else { 0.4 }, 200, window, cx);
        let tag_opacity = surface::fade_opacity(
            (ElementId::from(feature.id()), "tags"),
            if active { 1. } else { 0.3 },
            300,
            window,
            cx,
        );
        let slider = &self.sliders[&feature];
        let progress = slider.read(cx).value().start() / 4.;
        v_flex()
            .mt(surface::css(20.))
            .child(
                h_flex()
                    .items_center()
                    .mb(surface::css(6.))
                    .child(
                        div()
                            .mr(surface::css(10.))
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
                                .checked(active)
                                .disabled(locked)
                                .on_change(cx.listener(
                                    move |this, _, window, cx| {
                                        this.toggle_feature(feature, window, cx)
                                    },
                                )),
                            ),
                    ),
            )
            .children(feature.descriptions().iter().map(|key| {
                div()
                    .text_color(Colors::muted())
                    .text_size(surface::css(13.))
                    .line_height(surface::css(18.))
                    .child(t(key))
            }))
            .child(
                div()
                    .id((ElementId::from(feature.id()), "slider-wrapper"))
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
                            .child(SourceSlider::new(slider, progress).enabled(active))
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
            .into_any_element()
    }
}
impl Render for ScrollWheelEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.disabled_modes();
        let selected = self.value("scrollMode").as_str().unwrap_or("").to_owned();
        let pills = h_flex()
            .items_center()
            .h(surface::css(36.))
            .p(surface::css(4.))
            .mb(surface::css(16.))
            .border_1()
            .border_color(Colors::border())
            .rounded(surface::css(18.))
            .bg(Colors::pills())
            .overflow_hidden()
            .hover(|s| s.border_color(Colors::accent()))
            .children(data().modes.iter().map(|mode| {
                let active = selected == mode.id;
                let id = mode.id.clone();
                let is_disabled = disabled.contains(&mode.id);
                let color = surface::fade_color(
                    SharedString::from(format!("scroll-226-pill-color-{id}")),
                    if active {
                        Colors::selected_text()
                    } else {
                        Colors::pill_text()
                    },
                    200,
                    window,
                    cx,
                );
                BaseButton::new(SharedString::from(format!("scroll-226-pill-{id}")))
                    .accessibility_label(t(&mode.label))
                    .selected(active)
                    .disabled(is_disabled)
                    .opacity(if is_disabled { 0.4 } else { 1. })
                    .rounded(surface::css(14.))
                    .px(surface::css(16.))
                    .py(surface::css(4.))
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
                    .child(t(&mode.label))
                    .on_click(cx.listener(move |this, _, _, cx| this.select_mode(&id, cx)))
            }));
        let checks = v_flex()
            .gap(surface::css(8.))
            .children(data().modes.iter().map(|mode| {
                let checked = disabled.contains(&mode.id);
                let locked = disabled.len() >= 2 && !checked;
                let id = mode.id.clone();
                surface::check_item(
                    SharedString::from(format!("scroll-226-disable-{id}")),
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
                        Tooltip::new("scroll-226-locked-tooltip")
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
            surface::help_control("scroll-226-help", t("SCROLL_WHEEL_TOOLTIP_V2")),
            cx,
        )
        .font_family("Roboto")
        .child(
            v_flex()
                .mb(surface::css(20.))
                .child(
                    div()
                        .mb(surface::css(6.))
                        .text_color(Colors::text())
                        .text_size(surface::css(14.))
                        .child(t("SCROLL_MODE").to_uppercase()),
                )
                .child(
                    div()
                        .mb(surface::css(12.))
                        .text_color(Colors::muted())
                        .text_size(surface::css(13.))
                        .line_height(surface::css(18.))
                        .child(t("SCROLL_THREE_MODE_DESC")),
                )
                // inline-flex: keep the pill strip at intrinsic width.
                .child(h_flex().child(pills))
                .child(
                    div()
                        .mb(surface::css(10.))
                        .text_color(Colors::muted())
                        .text_size(surface::css(13.))
                        .child(t("SCROLL_DISABLE_MODES_DESC")),
                )
                .child(checks),
        )
        .child(self.section(Feature::Acceleration, window, cx))
        .child(self.section(Feature::SmartReel, window, cx))
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

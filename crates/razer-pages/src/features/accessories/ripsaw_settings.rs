//! PID 3331 `D` / `S`: one monitoring widget with four vertical volume rows.
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderTrack};
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::source_slider::source_thumb;
use razer_widgets::source_tooltip::{SourceTooltip, SourceTooltipKind};
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct State {
    pressed: BTreeSet<String>,
    tip_widths: BTreeMap<String, f32>,
}
impl State {
    pub(super) fn unmount(&mut self) {
        self.pressed.clear();
    }
}

/// Original Redux -> BroadcastChannel envelope; neither a device ack nor a save.
#[derive(Clone)]
pub struct RipsawMonitoringRequested {
    owner: EntityId,
    source_message: Value,
}
impl RipsawMonitoringRequested {
    pub fn owner(&self) -> EntityId {
        self.owner
    }
    pub fn source_message(&self) -> &Value {
        &self.source_message
    }
}
impl EventEmitter<RipsawMonitoringRequested> for SourceControls {}

const ROWS: [(&str, &str, &str); 4] = [
    ("masterVolume", "MASTER_VOLUME", "ON_SET_MASTER_VOLUME"),
    ("pcVolume", "PC", "ON_SET_PC_VOLUME"),
    ("hdmiVolume", "HDMI_1", "ON_SET_HDMI_VOLUME"),
    ("micVolume", "MIC", "ON_SET_MIC_VOLUME"),
];

fn source_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

impl SourceControls {
    pub fn accepts_ripsaw_monitoring(
        &self,
        request: &RipsawMonitoringRequested,
        owner: EntityId,
    ) -> bool {
        self.spec.product_id == 3331 && request.owner == owner
    }

    pub fn observe_ripsaw_monitoring(
        &mut self,
        source_payload: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 3331 {
            return;
        }
        // Current MW_UPDATE_AUDIO_MONITORING_TO_UI maps speakerVolume to
        // masterVolume and only replaces fields actually present in its payload.
        for (incoming, field) in [
            ("speakerVolume", "masterVolume"),
            ("pcVolume", "pcVolume"),
            ("hdmiVolume", "hdmiVolume"),
            ("micVolume", "micVolume"),
        ] {
            if let Some(value) = source_payload
                .get(incoming)
                .filter(|value| source_truthy(value))
            {
                self.draft["runtime"][field] = value.clone();
            }
        }
        self.sync(window, cx);
        // An observation is not a locally edited profile or a write response.
        cx.notify();
    }

    pub(super) fn submit_ripsaw_value(
        &mut self,
        field: &str,
        value: Option<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((_, _, action)) = ROWS.iter().find(|row| row.0 == field) else {
            return;
        };
        let settings = &mut self.draft["runtime"][field];
        if let Some(value) = value {
            if !value.is_finite() {
                return;
            }
            settings["value"] = serde_json::json!(value.round().clamp(0., 100.) as u8);
        } else {
            settings["isEnabled"] = Value::Bool(settings["isEnabled"] != true);
        }
        let payload = serde_json::json!({field: settings.clone()});
        // The source posts Date.now() for every slider release, including an
        // unchanged release. Its window.currentTimerTick queue is separate.
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or_default();
        self.sync(window, cx);
        cx.emit(SourceControlsChanged);
        cx.emit(RipsawMonitoringRequested {
            owner: cx.entity_id(),
            source_message: serde_json::json!({"type": action, "timerTick": tick, "payload": payload}),
        });
        cx.notify();
    }

    pub(super) fn ripsaw_slider_event(
        &mut self,
        key: &str,
        event: &SliderEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(field) = key.strip_prefix("3331:") else {
            return;
        };
        match event {
            SliderEvent::Change(value) if !self.ripsaw.pressed.contains(field) => {
                self.submit_ripsaw_value(field, Some(value.start()), window, cx)
            }
            SliderEvent::Release(value) => {
                self.ripsaw.pressed.remove(field);
                self.submit_ripsaw_value(field, Some(value.start()), window, cx);
            }
            SliderEvent::Change(_) => cx.notify(),
        }
    }

    fn ripsaw_row(
        &self,
        field: &'static str,
        label: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = format!("3331:{field}");
        let state = &self.sliders[&key];
        let value = state.read(cx).value().start();
        let progress = value / 100.;
        let enabled = self.draft["runtime"][field]["isEnabled"] == true;
        let tip_owner = cx.entity().downgrade();
        let old_tip_width = self.ripsaw.tip_widths.get(field).copied();
        let focus = window
            .use_keyed_state(("ripsaw-range-focus", state.entity_id()), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let pointer_focus = focus.clone();
        // Source rotates its 180px container around its center. The 165px
        // input/track starts at x=0; visually its endpoints are y=43..208.
        let track = BaseSlider::new(state)
            .vertical()
            .disabled(!enabled)
            .relative()
            .size_full()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(surface::css(5.))
                    .w(surface::css(6.))
                    .rounded(surface::css(3.))
                    .bg(rgba(0x44d62c4d))
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .w_full()
                            .h(surface::css(8. + 147. * progress))
                            .rounded(surface::css(3.))
                            .bg(rgb(0x44d62c)),
                    ),
            )
            .child(
                SliderTrack::new(state)
                    .axis(Axis::Vertical)
                    .disabled(!enabled)
                    .size_full()
                    .child(
                        SliderIndicator::new(state)
                            .absolute()
                            .top(surface::css(8.))
                            .bottom(surface::css(8.))
                            .w_full()
                            .child(
                                source_thumb(state, enabled, window, cx)
                                    .axis(Axis::Vertical)
                                    .absolute()
                                    .bottom(relative(progress))
                                    .mb(surface::css(-8.))
                                    .size(surface::css(16.))
                                    .rounded(surface::css(8.)),
                            ),
                    ),
            );
        v_flex()
            .relative()
            .h_full()
            .min_w(surface::css(95.))
            .items_center()
            .justify_between()
            .child(
                div()
                    .line_height(surface::css(19.))
                    .child(razer_i18n::t(label)),
            )
            .child(
                div()
                    .id(("ripsaw-range", state.entity_id()))
                    .absolute()
                    .top(surface::css(43.))
                    .left(surface::css(39.5))
                    .w(surface::css(16.))
                    .h(surface::css(165.))
                    .track_focus(&focus)
                    // Source uses pointer-events:none, not the disabled input
                    // attribute: a muted range retains native keyboard input.
                    .tab_stop(true)
                    .opacity(surface::fade_opacity(
                        ("ripsaw-range-opacity", state.entity_id()),
                        if enabled { 1. } else { 0.3 },
                        300,
                        window,
                        cx,
                    ))
                    .capture_any_mouse_down(cx.listener(
                        move |this, event: &MouseDownEvent, window, cx| {
                            if enabled && event.button == MouseButton::Left {
                                this.ripsaw.pressed.insert(field.into());
                                pointer_focus.focus(window, cx);
                            }
                        },
                    ))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        let current = this.sliders[&format!("3331:{field}")]
                            .read(cx)
                            .value()
                            .start();
                        let next = match event.keystroke.key.as_str() {
                            "up" | "right" => current + 1.,
                            "down" | "left" => current - 1.,
                            "home" => 0.,
                            "end" => 100.,
                            _ => return,
                        };
                        this.submit_ripsaw_value(field, Some(next), window, cx);
                        cx.stop_propagation();
                    }))
                    .child(track)
                    // Source updateValue sets tip left=143*p-4 before rotation.
                    .child(
                        div()
                            .absolute()
                            .top(surface::css(
                                158. - 143. * progress
                                    - self
                                        .ripsaw
                                        .tip_widths
                                        .get(field)
                                        .copied()
                                        .unwrap_or_default()
                                        / 2.,
                            ))
                            .left(surface::css(-20.))
                            .w_0()
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .whitespace_nowrap()
                                    .px(surface::css(8.))
                                    .py(surface::css(4.))
                                    .rounded(surface::css(3.))
                                    .bg(rgb(0x44d62c))
                                    .text_color(rgb(0x212121))
                                    .text_size(surface::css(12.))
                                    .line_height(surface::css(14.))
                                    .on_prepaint(move |bounds, window, cx| {
                                        let width = f32::from(bounds.size.width)
                                            / f32::from(window.rem_size())
                                            * 16.;
                                        if old_tip_width != Some(width) {
                                            window.defer(cx, move |_, cx| {
                                                let _ = tip_owner.update(cx, |this, cx| {
                                                    this.ripsaw
                                                        .tip_widths
                                                        .insert(field.into(), width);
                                                    cx.notify();
                                                });
                                            });
                                        }
                                    })
                                    .child(format!("{value:.0}")),
                            ),
                    ),
            )
            .child(
                BaseButton::new(SharedString::from(format!("ripsaw-{field}-mute")))
                    .accessibility_label(razer_i18n::t(label))
                    .size(surface::css(20.))
                    .child(
                        img(if enabled {
                            "synapse/ripsaw-settings/icon_audio_high.svg"
                        } else {
                            "synapse/ripsaw-settings/icon_audio_mute.svg"
                        })
                        .size_full(),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.submit_ripsaw_value(field, None, window, cx)
                    })),
            )
            .into_any_element()
    }

    pub(super) fn render_ripsaw_settings(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut rows = h_flex().h(surface::css(250.)).w_full().justify_around();
        for (field, label, _) in ROWS {
            rows = rows.child(self.ripsaw_row(field, label, window, cx));
        }
        v_flex()
            .w_full()
            .min_w(surface::css(1240.))
            .max_w(surface::css(1240.))
            .mx_auto()
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(rgb(0xcccccc))
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(surface::css(250.))
                    .child(surface::dot_background(cx))
                    .child(
                        h_flex().relative().justify_center().child(
                            img("synapse/ripsaw-settings/ripsaw-top-3x.png")
                                .w(surface::css(600.))
                                .h(surface::css(250.)),
                        ),
                    ),
            )
            .child(
                v_flex()
                    .relative()
                    .w(surface::css(600.))
                    .mx_auto()
                    .my(surface::css(10.))
                    .py(surface::css(30.))
                    .px(surface::css(40.))
                    .rounded(surface::css(5.))
                    .bg(rgb(0x111111))
                    .child(
                        h_flex().mb(surface::css(20.)).justify_between().child(
                            div()
                                .font_family("RazerF5")
                                .text_size(surface::css(16.))
                                .text_color(rgb(0x44d62c))
                                .child(razer_i18n::t("AUDIO_MONITORING").to_uppercase()),
                        ),
                    )
                    .child(
                        v_flex()
                            .mb(surface::css(30.))
                            .child(razer_i18n::t("AUDIO_MONITORING_DESC"))
                            .child(razer_i18n::t("AUDIO_MONITORING_NOTE")),
                    )
                    .child(rows)
                    .child(
                        div()
                            .absolute()
                            .right(surface::css(10.))
                            .top(surface::css(10.))
                            .child(
                                SourceTooltip::new(
                                    "ripsaw-monitoring-help",
                                    razer_i18n::t("AUDIO_MONITORING_TOOLTIP"),
                                    300.,
                                )
                                .kind(SourceTooltipKind::ReceiverWidgetPortal)
                                .trigger(|hovered, window, cx| {
                                    BaseButton::new("ripsaw-monitoring-help-trigger")
                                        .accessibility_label(razer_i18n::t(
                                            "AUDIO_MONITORING_TOOLTIP",
                                        ))
                                        .size(surface::css(14.))
                                        .rounded(surface::css(7.))
                                        .bg(surface::fade_color(
                                            "ripsaw-monitoring-help-background",
                                            if hovered {
                                                rgba(0xffffff4d).into()
                                            } else {
                                                rgb(0x4a4a4a).into()
                                            },
                                            300,
                                            window,
                                            cx,
                                        ))
                                        .hover(|button| button.cursor_pointer())
                                        .child(
                                            img("synapse/ripsaw-settings/tooltip_questionmark.svg")
                                                .size_full(),
                                        )
                                        .into_any_element()
                                }),
                            ),
                    ),
            )
            .into_any_element()
    }
}

//! Product 179's mounted Windows Customize root (9473/mE -> Te + OE).
use super::*;
use gpui_kit::base::{
    Radio as BaseRadio,
    motion::{self, Easing, Transition},
};
use std::time::{Duration, Instant};

#[path = "receiver_page.rs"]
mod page;
#[path = "receiver_page_state.rs"]
mod page_state;
#[path = "receiver_pairing_state.rs"]
mod pairing_state;
#[path = "receiver_pairing_view.rs"]
mod pairing_view;
pub(crate) use page_state::{ReceiverDeviceRequested, ReceiverDevicesObservation};
use pairing_state::PairingState;
pub(crate) use pairing_state::{
    ReceiverCategory, ReceiverOperation, ReceiverPairingEvent, ReceiverPairingIntent,
    ReceiverPairingObservation, ReceiverPeer, ReceiverProgress,
};

pub(super) struct ReceiverState {
    indicator_since: Instant,
    pairing: PairingState,
    failure_recovery: Option<(pairing_state::FailureRecovery, Task<()>)>,
    success_close: Option<(pairing_state::SuccessClose, Task<()>)>,
    pairing_window: Option<AnyWindowHandle>,
    return_focus: Option<FocusHandle>,
    active: bool,
    page: page_state::ReceiverPageState,
    page_retry: Option<(page_state::PageRetry, Task<()>)>,
}
impl Default for ReceiverState {
    fn default() -> Self {
        Self {
            indicator_since: Instant::now(),
            pairing: PairingState::default(),
            failure_recovery: None,
            success_close: None,
            pairing_window: None,
            return_focus: None,
            active: false,
            page: page_state::ReceiverPageState::default(),
            page_retry: None,
        }
    }
}
impl ReceiverState {
    pub(super) fn restart_indicator(&mut self) {
        self.indicator_since = Instant::now();
    }
}

#[derive(Deserialize)]
struct IndicatorMode {
    mode: u64,
    base: String,
    layers: Vec<IndicatorLayer>,
}
#[derive(Deserialize)]
struct IndicatorLayer {
    asset: String,
    values: String,
    begin: String,
    dur: String,
    id: Option<String>,
    #[serde(rename = "repeatCount")]
    repeat: Option<String>,
}
fn modes() -> &'static [IndicatorMode] {
    static MODES: OnceLock<Vec<IndicatorMode>> = OnceLock::new();
    MODES.get_or_init(|| {
        serde_json::from_str(include_str!("receiver_indicator_data.json"))
            .expect("current receiver SVG timing")
    })
}
fn opacity(layer: &IndicatorLayer, elapsed: f32) -> f32 {
    let duration: f32 = layer
        .dur
        .trim_end_matches('s')
        .parse()
        .expect("audited SMIL duration");
    // Warning mode is a chained 0.3s + 0.3s + 2s sequence. There is no
    // repeatCount: dot_ani3.end restarts dot_ani1, exactly as the source SVG.
    let local = if let Some(id) = &layer.id {
        let start = match id.as_str() {
            "dot_ani1" => 0.,
            "dot_ani2" => 0.3,
            "dot_ani3" => 0.6,
            _ => return 0.,
        };
        let local = elapsed.rem_euclid(2.6) - start;
        if !(0. ..duration).contains(&local) {
            return 0.;
        }
        local
    } else {
        let begin: f32 = layer
            .begin
            .trim_end_matches('s')
            .parse()
            .expect("audited SMIL begin");
        if elapsed < begin {
            return 0.;
        }
        if layer.repeat.as_deref() == Some("indefinite") {
            (elapsed - begin).rem_euclid(duration)
        } else {
            (elapsed - begin).min(duration)
        }
    };
    let values: Vec<f32> = layer
        .values
        .split(';')
        .map(|v| v.parse().expect("audited SMIL opacity"))
        .collect();
    let position = local / duration * (values.len() - 1) as f32;
    let index = (position.floor() as usize).min(values.len() - 2);
    values[index] + (values[index + 1] - values[index]) * (position - index as f32)
}

fn indicator_image(mode: u64, since: Instant, cx: &App) -> AnyElement {
    let mode = modes()
        .iter()
        .find(|entry| entry.mode == mode)
        .unwrap_or(&modes()[0]);
    let frame = move |elapsed: f32| {
        div()
            .relative()
            .w(surface::css(243.))
            .h(surface::css(187.))
            .child(img(SharedString::from(mode.base.clone())).size_full())
            .children(mode.layers.iter().map(|layer| {
                img(SharedString::from(layer.asset.clone()))
                    .absolute()
                    .inset_0()
                    .size_full()
                    .opacity(opacity(layer, elapsed))
            }))
    };
    if mode.layers.is_empty() || cx.reduce_motion() {
        frame(0.).into_any_element()
    } else {
        div()
            .w(surface::css(243.))
            .h(surface::css(187.))
            .with_animation(
                SharedString::from(format!("receiver-indicator-smil-{}", mode.mode)),
                Animation::new(Duration::from_secs(1))
                    .repeat()
                    .with_easing(linear),
                move |view, _| view.child(frame(since.elapsed().as_secs_f32())),
            )
            .into_any_element()
    }
}

impl SourceControls {
    pub(crate) fn set_receiver_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.receiver.active = active;
        self.sync_receiver_page_activity(cx);
    }
    pub(super) fn sync_receiver_page_activity(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id != 179 {
            return;
        }
        let active = self.receiver.active && self.page == "TAB_CUSTOMIZE";
        if active == self.receiver.page.active {
            return;
        }
        self.receiver.page.active = active;
        let event = if active && !self.receiver.pairing.is_open() {
            self.receiver.page.begin()
        } else {
            self.receiver.page.suspend()
        };
        if let Some(event) = event {
            cx.emit(event);
        }
        self.sync_receiver_page_retry(cx);
        cx.notify();
    }
    pub(crate) fn observe_receiver_devices(
        &mut self,
        devices: ReceiverDevicesObservation,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 179 {
            return;
        }
        let connected = self.receiver.page.connected();
        if !self.receiver.page.observe_devices(devices) {
            return;
        }
        if connected != self.receiver.page.connected() && !self.receiver.pairing.is_open() {
            if let Some(event) = self.receiver.page.begin() {
                cx.emit(event);
            }
        }
        self.sync_receiver_page_retry(cx);
        cx.notify();
    }
    fn sync_receiver_page_retry(&mut self, cx: &mut Context<Self>) {
        let retry = self.receiver.page.retry();
        if self.receiver.page_retry.as_ref().map(|(ticket, _)| *ticket) == retry {
            return;
        }
        self.receiver.page_retry = None;
        if let Some(ticket) = retry {
            let task = cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let _ = this.update(cx, |this, cx| {
                    this.receiver.page_retry = None;
                    if let Some(event) = this.receiver.page.resume_retry(ticket) {
                        cx.emit(event);
                        cx.notify();
                    }
                });
            });
            self.receiver.page_retry = Some((ticket, task));
        }
    }
    pub(super) fn render_receiver(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mode = self
            .draft
            .pointer("/runtime/indicatorLedStatus")
            .and_then(Value::as_u64)
            .unwrap_or(1);
        let compact = surface::stacked_device_columns(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
        );
        let mut options = v_flex()
            .w(surface::css(310.))
            .flex_shrink_0()
            .gap(surface::css(10.));
        for (value, label, description, height) in [
            (
                1,
                "CONNECTION_STATUS",
                "POWER_INDICATOR_CONNECTION_STATUS_DESC",
                50.,
            ),
            (2, "BATTERY_STATUS", "BATTERY_STATUS_DESC", 64.),
            (
                3,
                "BATTERY_WARNING_ONLY",
                "POWER_INDICATOR_BATTERY_WARNING_ONLY_DESC",
                50.,
            ),
        ] {
            let progress = motion::transition(
                (ElementId::from("receiver-radio-dot"), value.to_string()),
                if mode == value { 1_f32 } else { 0_f32 },
                Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
                window,
                cx,
            );
            options = options.child(
                v_flex()
                    .w(surface::css(310.))
                    .h(surface::css(height))
                    .flex_shrink_0()
                    .child(
                        BaseRadio::new(SharedString::from(format!("receiver-indicator-{value}")))
                            .checked(mode == value)
                            .accessibility_label(crate::i18n::t(label))
                            .flex()
                            .items_center()
                            .h(surface::css(20.))
                            .gap(surface::css(10.))
                            .child(
                                div()
                                    .relative()
                                    .size(surface::css(20.))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(rgb(0x737373))
                                    .child(
                                        div()
                                            .absolute()
                                            .left(surface::css(9. - 5. * progress))
                                            .top(surface::css(9. - 5. * progress))
                                            .size(surface::css(10. * progress))
                                            .rounded_full()
                                            .bg(rgb(0x44d62c))
                                            .opacity(progress),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(20.))
                                    .text_color(rgb(0xcccccc))
                                    .child(crate::i18n::t(label)),
                            )
                            .on_change({
                                let entity = cx.weak_entity();
                                move |_, _, window, cx| {
                                    let _ = entity.update(cx, |this, cx| {
                                        let next = serde_json::json!(value);
                                        if this.draft.pointer("/runtime/indicatorLedStatus")
                                            != Some(&next)
                                        {
                                            this.receiver.indicator_since = Instant::now();
                                        }
                                        this.edit("179:indicator-mode", next, window, cx);
                                    });
                                }
                            }),
                    )
                    .child(
                        div()
                            .ml(surface::css(30.))
                            .w(surface::css(280.))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .text_color(rgb(0x999999))
                            .opacity(0.7)
                            .child(crate::i18n::t(description)),
                    ),
            );
        }
        let left = self.receiver_pairing_widget(cx);
        let right = surface::panel(crate::i18n::t("INDICATOR_LED_V2"), cx)
            .w(surface::css(600.))
            .child(
                v_flex()
                    .w(surface::css(520.))
                    .h(surface::css(227.))
                    .gap(surface::css(20.))
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .text_color(rgb(0xcccccc))
                            .child(crate::i18n::t("INDICATOR_LED_DESC")),
                    )
                    .child(
                        h_flex()
                            .items_start()
                            .my(surface::css(5.))
                            .child(options)
                            .child(
                                div()
                                    .w(surface::css(200.))
                                    .h(surface::css(192.))
                                    .flex_shrink_0()
                                    .child(indicator_image(
                                        mode,
                                        self.receiver.indicator_since,
                                        cx,
                                    )),
                            ),
                    ),
            );
        let columns = [left, right.into_any_element()].into_iter().map(|widget| {
            div()
                .w(surface::css(600.))
                .flex_shrink_0()
                .when(compact, |column| column.mx(surface::css(30.)))
                .child(widget)
        });
        v_flex()
            .w_full()
            .min_w(surface::css(600.))
            .pt(surface::css(10.))
            .px(surface::css(20.))
            .pb(surface::css(20.))
            .font_family("Roboto")
            .text_color(rgb(0xcccccc))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .mx_auto()
                    .w_full()
                    .max_w(surface::css(1240.))
                    .child(surface::product_banner(179, 0, 0, cx))
                    .children(columns),
            )
            .children(
                self.receiver
                    .pairing
                    .is_open()
                    .then(|| self.receiver_pairing_modal(window, cx)),
            )
            .into_any_element()
    }

    fn receiver_pairing_modal(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        // G/se mounts LOADING; only typed, current-session observations may
        // expose loaded/service-result states. Local intents never fake them.
        let size = window.viewport_size();
        let unit = window.rem_size() / 16.;
        // Electron mounts the product WebContents below its 42px TabUI.
        // G's fixed coordinates and 100vh belong to that WebContents viewport.
        let content_top = unit * 42.;
        let content_height = (size.height - content_top).max(px(0.));
        let width = (unit * 850.).min(size.width);
        deferred(
            anchored()
                .anchor(Anchor::TopLeft)
                .position(point(px(0.), content_top))
                .child(
                    div()
                        .relative()
                        .w(size.width)
                        .h(content_height)
                        .occlude()
                        .bg(rgba(0x000000b3))
                        .child(
                            v_flex()
                                .id("receiver-pairing-modal")
                                .test_support()
                                .role(Role::Dialog)
                                .aria_label(crate::i18n::t("PAIRING_UTILITY"))
                                .absolute()
                                .left((size.width - width) / 2.)
                                .top(unit * 100.)
                                .w(width)
                                .h(content_height)
                                .track_focus(&self.focus)
                                .bg(rgb(0x111111))
                                .border_1()
                                .border_color(rgb(0x515151))
                                .rounded(surface::css(5.))
                                .overflow_y_scroll()
                                .child(
                                    h_flex()
                                        .relative()
                                        .w_full()
                                        .h(surface::css(36.))
                                        .flex_shrink_0()
                                        .items_center()
                                        .justify_center()
                                        .bg(rgb(0x222222))
                                        .rounded_tl(surface::css(5.))
                                        .rounded_tr(surface::css(5.))
                                        .border_b_1()
                                        .border_color(rgb(0x515151))
                                        .text_size(surface::css(14.))
                                        .line_height(surface::css(36.))
                                        .text_color(rgb(0x999999))
                                        .child(crate::i18n::t("PAIRING_UTILITY").to_uppercase())
                                        .child(
                                            gpui_kit::base::Button::new("receiver-close-pairing")
                                                .accessibility_label(crate::i18n::t("CLOSE"))
                                                .absolute()
                                                .right(surface::css(5.))
                                                .size(surface::css(20.))
                                                .child(img("synapse/mapping-close.svg").size_full())
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.close_receiver_pairing(window, cx);
                                                })),
                                        ),
                                )
                                .child(self.receiver_pairing_body(cx)),
                        ),
                ),
        )
        .with_priority(1001)
        .into_any_element()
    }
}

#[derive(IntoElement)]
struct ReceiverSpinner;
impl RenderOnce for ReceiverSpinner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let frame = |phase: f32| {
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let scale = f32::from(bounds.size.width) / 100.;
                    let rotation = if phase <= 0.5 {
                        phase * 360.
                    } else {
                        180. + (phase - 0.5) * 1080.
                    };
                    let fraction = if phase <= 0.5 {
                        0.1 + phase * 0.8
                    } else {
                        0.5 - (phase - 0.5) * 0.8
                    };
                    for (start, length, alpha) in [
                        (0., std::f32::consts::TAU, 0.3),
                        (rotation.to_radians(), fraction * std::f32::consts::TAU, 1.),
                    ] {
                        let radius = 30. * scale;
                        let project = |angle: f32| {
                            bounds.center()
                                + point(px(angle.cos() * radius), px(angle.sin() * radius))
                        };
                        let mut path = PathBuilder::stroke(px(10. * scale));
                        path.move_to(project(start));
                        // Two true arcs also cover the background circle; equal
                        // start/end points would degenerate a single SVG arc.
                        path.arc_to(
                            point(px(radius), px(radius)),
                            px(0.),
                            false,
                            true,
                            project(start + length / 2.),
                        );
                        path.arc_to(
                            point(px(radius), px(radius)),
                            px(0.),
                            false,
                            true,
                            project(start + length),
                        );
                        if let Ok(path) = path.build() {
                            window.paint_path(path, Hsla::from(rgb(0x44d62c)).opacity(alpha));
                        }
                        if alpha == 1. {
                            // spinner.ef2d0235.svg has square foreground caps.
                            // GPUI's stroke uses butt caps; add the half-stroke
                            // tangent extension at both ends of the source arc.
                            for (angle, direction) in [(start, -1.), (start + length, 1.)] {
                                let endpoint = project(angle);
                                let normal = point(
                                    px(angle.cos() * 5. * scale),
                                    px(angle.sin() * 5. * scale),
                                );
                                let extension = point(
                                    px(-angle.sin() * 5. * scale * direction),
                                    px(angle.cos() * 5. * scale * direction),
                                );
                                let mut cap = PathBuilder::fill();
                                cap.move_to(endpoint + normal);
                                cap.line_to(endpoint - normal);
                                cap.line_to(endpoint - normal + extension);
                                cap.line_to(endpoint + normal + extension);
                                cap.close();
                                if let Ok(cap) = cap.build() {
                                    window.paint_path(cap, rgb(0x44d62c));
                                }
                            }
                        }
                    }
                },
            )
            .size(surface::css(20.))
        };
        if cx.reduce_motion() {
            frame(0.25).into_any_element()
        } else {
            div()
                .size(surface::css(20.))
                .with_animation(
                    "receiver-pairing-spinner",
                    Animation::new(Duration::from_secs(2))
                        .repeat()
                        .with_easing(linear),
                    move |view, phase| view.child(frame(phase)),
                )
                .into_any_element()
        }
    }
}

//! Source UI7405 footer/wizard presentation. Original CSS sizes are expressed
//! through the product scale; Base owns buttons, dialog focus and dismissal.
use super::super::audio_seiren_eq::colors::Colors as C;
use super::*;
use gpui_kit::base::{
    Button as BaseButton, Dialog, Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack,
    motion::{self, Easing, IterationCount, Keyframe, Keyframes, Timing, Transition},
};
use razer_widgets::source_text::SourceText;

#[derive(IntoElement)]
struct MicrophoneFooter {
    owner: WeakEntity<AudioProductWorkspace>,
    pid: u32,
    live: bool,
    rate: u64,
    recording: bool,
    recording_generation: u64,
    playing: bool,
    pending: bool,
    has_clip: bool,
    trigger_focus: Option<FocusHandle>,
    waveform: Option<AnyElement>,
}

fn record_button(id: &'static str) -> BaseButton {
    BaseButton::new(id)
        .p_0()
        .border_0()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .styles(|style| {
            style.disabled(|style| style.opacity(0.5).cursor(CursorStyle::OperationNotAllowed))
        })
}

#[derive(IntoElement)]
struct RecordingSpinner;

impl RenderOnce for RecordingSpinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let rotation = motion::animate_keyframes(
            "seiren-mic-playback-spin",
            &Keyframes::try_new([Keyframe::new(0., 0.), Keyframe::new(1., 360.)])
                .expect("complete source spin keyframes"),
            Timing::new(Duration::from_millis(800)).iterations(IterationCount::Infinite),
            window,
            cx,
        )
        .value;
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let scale = f32::from(window.rem_size()) / 16.;
                let centre = bounds.center();
                let radius = (f32::from(bounds.size.width) / 2. - scale).max(scale);
                for (start, end, color) in [
                    (
                        rotation - 135.,
                        rotation + 135.,
                        C::microphone_spinner_track(),
                    ),
                    (rotation + 135., rotation + 225., C::timer()),
                ] {
                    let at = |degrees: f32| {
                        centre
                            + point(
                                px(degrees.to_radians().cos() * radius),
                                px(degrees.to_radians().sin() * radius),
                            )
                    };
                    let mut path = PathBuilder::stroke(px(2. * scale));
                    path.move_to(at(start));
                    let pieces = ((end - start) / 90.).ceil() as u32;
                    for piece in 1..=pieces {
                        path.arc_to(
                            point(px(radius), px(radius)),
                            px(0.),
                            false,
                            true,
                            at(start + (end - start) * piece as f32 / pieces as f32),
                        );
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .size(surface::css(12.))
    }
}

impl RenderOnce for MicrophoneFooter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            owner,
            pid,
            live,
            rate,
            recording,
            recording_generation,
            playing,
            pending,
            has_clip,
            trigger_focus,
            waveform,
        } = self;
        let policy = Transition::new(Duration::from_millis(400))
            .easing(Easing::cubic_bezier(0.4, 0., 0.2, 1.).expect("source footer easing"));
        let height = motion::transition(
            "seiren-mic-footer-height",
            if live { 145. } else { 45. },
            policy,
            window,
            cx,
        );
        let live_background = surface::fade_color(
            "seiren-mic-live-background",
            if live { C::selected() } else { C::remainder() },
            200,
            window,
            cx,
        );
        let live_color = surface::fade_color(
            "seiren-mic-live-color",
            if live { C::surface() } else { C::white() },
            200,
            window,
            cx,
        );
        let setup_pointer = surface::pointer_state("seiren-mic-setup", window, cx);
        let (setup_hovered, _) = setup_pointer.read(cx).sample();
        let setup_background = surface::fade_color(
            "seiren-mic-setup-background",
            if setup_hovered {
                C::microphone_setup_hover()
            } else {
                C::active()
            },
            200,
            window,
            cx,
        );
        let setup_owner = owner.clone();
        let setup = BaseButton::new("seiren-mic-setup")
            .bg(setup_background)
            .border_1()
            .border_color(C::text())
            .rounded(surface::css(3.))
            .min_w(surface::css(104.))
            .h(surface::css(34.))
            .py(surface::css(10.))
            .px(surface::css(16.))
            .text_size(surface::css(12.))
            .font_weight(FontWeight::NORMAL)
            .text_color(C::text())
            .whitespace_nowrap()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .when_some(trigger_focus.as_ref(), |button, focus| {
                button.track_focus(focus)
            })
            .child(text(pid, "_jK").to_uppercase())
            .on_click(move |_, window, cx| {
                let _ = setup_owner.update(cx, |this, cx| {
                    this.open_seiren_microphone_wizard(window, cx)
                });
            });
        let setup = surface::track_pointer(setup, &setup_pointer, window);
        let rate_pointer = surface::pointer_state("seiren-mic-rate", window, cx);
        let (rate_hovered, rate_pressed) = rate_pointer.read(cx).sample();
        let rate_border = surface::fade_color(
            "seiren-mic-rate-border",
            if rate_hovered { C::selected() } else { C::border() },
            300,
            window,
            cx,
        );
        let mut rate_tabs = h_flex().gap(surface::css(5.)).size_full();
        for option in [48_000u64, 96_000] {
            let selected = rate == option;
            let background = if selected {
                C::selected()
            } else if rate_hovered && rate_pressed {
                C::white().opacity(0.1)
            } else {
                C::surface()
            };
            let background = surface::fade_color(
                ("seiren-mic-rate-background", option),
                background,
                200,
                window,
                cx,
            );
            let color = surface::fade_color(
                ("seiren-mic-rate-color", option),
                if selected { C::surface() } else { C::text() },
                200,
                window,
                cx,
            );
            rate_tabs = rate_tabs.child(
                div()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .rounded(surface::css(36.))
                    .bg(background)
                    .text_color(color)
                    .text_size(surface::css(12.))
                    .line_height(surface::css(26.))
                    .text_center()
                    .child(format!("{}kHz", option / 1000)),
            );
        }
        let rate_owner = owner.clone();
        let rate_tabs = BaseButton::new("seiren-mic-rate")
            .w(surface::css(135.))
            .min_w(surface::css(120.))
            .h(surface::css(36.))
            .border_1()
            .border_color(rate_border)
            .rounded(surface::css(36.))
            .p(surface::css(5.))
            .cursor_pointer()
            .child(rate_tabs)
            .accessibility_label(format!("{}: {}kHz", text(pid, "hbV"), rate / 1000))
            .on_click(move |_, _, cx| {
                let _ = rate_owner.update(cx, |this, cx| {
                    this.seiren_sampling_rate(if rate == 48_000 { 96_000 } else { 48_000 }, cx)
                });
            });
        let rate_tabs = surface::track_pointer(rate_tabs, &rate_pointer, window);
        let width = f32::from(window.bounds().size.width) * 16. / f32::from(window.rem_size());
        let (label_margin, rate_gap) = if width <= 1200. {
            (4., 4.)
        } else if width <= 1400. {
            (6., 6.)
        } else {
            (10., 0.)
        };
        let record_owner = owner.clone();
        let record_tip = text(pid, if recording { "$pN" } else { "RLI" });
        let record_opacity = if recording {
            motion::animate_keyframes(
                ElementId::from(("seiren-mic-record-pulse", recording_generation)),
                &Keyframes::try_new([
                    Keyframe::new(0., 1.).ease(Easing::Ease),
                    // The mounted 684 CSS overrides 38/287's global pulse.
                    Keyframe::new(0.5, 0.4).ease(Easing::Ease),
                    Keyframe::new(1., 1.),
                ])
                .expect("complete source pulse keyframes"),
                Timing::new(Duration::from_millis(1500)).iterations(IterationCount::Infinite),
                window,
                cx,
            )
            .value
        } else {
            1.
        };
        let record_filter = motion::transition(
            "seiren-mic-record-brightness",
            if recording { 1. } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        let record_image = if record_filter > 0. {
            razer_assets::filtered_resource_svg(
                asset(pid, "record"),
                C::microphone_record_filters(record_filter),
            )
        } else {
            asset(pid, "record").into()
        };
        let record = record_button("seiren-mic-record")
            .selected(recording)
            .disabled(playing)
            .accessibility_label(record_tip.clone())
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(record_tip.clone()).build(window, cx)
            })
            .child(
                img(record_image)
                    .size(surface::css(24.))
                    .object_fit(ObjectFit::Fill)
                    .opacity(record_opacity),
            )
            .on_click(move |_, _, cx| {
                let _ = record_owner.update(cx, |this, cx| this.toggle_seiren_recording(cx));
            });
        let loading = pending && !playing;
        let ready_filter = motion::transition(
            "seiren-mic-play-ready-filter",
            if has_clip { 1. } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        let play_opacity = surface::fade_opacity(
            "seiren-mic-play-opacity",
            if has_clip { 1. } else { 0.5 },
            200,
            window,
            cx,
        );
        let play_icon = if loading {
            RecordingSpinner.into_any_element()
        } else if playing {
            img(asset(pid, "stop"))
                .size(surface::css(24.))
                .object_fit(ObjectFit::Fill)
                .into_any_element()
        } else {
            let image = if ready_filter > 0. {
                razer_assets::filtered_resource_svg(
                    asset(pid, "play"),
                    C::microphone_play_filters(ready_filter),
                )
            } else {
                asset(pid, "play").into()
            };
            img(image)
                .w(surface::css(12.))
                .h(surface::css(16.))
                .object_fit(ObjectFit::Fill)
                .opacity(play_opacity)
                .into_any_element()
        };
        let play_owner = owner.clone();
        let play = record_button("seiren-mic-play")
            .disabled(!has_clip || recording || loading)
            .selected(playing)
            .accessibility_label(text(pid, if playing { "VWS" } else { "FoH" }))
            .when(!playing && !loading, |button| {
                let label = text(pid, "FoH");
                button.tooltip(move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(label.clone()).build(window, cx)
                })
            })
            .child(play_icon)
            .on_click(move |_, _, cx| {
                let _ = play_owner.update(cx, |this, cx| this.toggle_seiren_playback(cx));
            });
        let live_owner = owner;
        let controls = h_flex()
            .id("seiren-mic-controls-row")
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .w_full()
            .h(surface::css(45.))
            .min_h(surface::css(45.))
            .flex_shrink_0()
            .px(surface::css(20.))
            .justify_between()
            .overflow_x_scroll()
            .overflow_y_hidden()
            .bg(C::black())
            .child(
                h_flex()
                    .gap(surface::css(20.))
                    .flex_shrink_0()
                    .child(
                        h_flex()
                            .gap(surface::css(20.))
                            .h(surface::css(45.))
                            .flex_shrink_0()
                            .child(
                                BaseButton::new("seiren-mic-live-preview")
                                    .min_w(surface::css(134.))
                                    .h(surface::css(26.))
                                    .px(surface::css(14.))
                                    .rounded(surface::css(26.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(surface::css(12.))
                                    .line_height(surface::css(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .whitespace_nowrap()
                                    .selected(live)
                                    .bg(live_background)
                                    .text_color(live_color)
                                    .cursor_pointer()
                                    .hover(|style| style.opacity(0.9))
                                    .active(|style| style.opacity(0.8))
                                    .child(
                                        text(pid, if live { "s7T" } else { "KLv" }).to_uppercase(),
                                    )
                                    .on_click(move |_, _, cx| {
                                        let _ = live_owner.update(cx, |this, cx| {
                                            this.seiren_mic.live = !this.seiren_mic.live;
                                            this.sync_seiren_microphone_runtime(cx);
                                            cx.notify();
                                        });
                                    }),
                            )
                            .child(setup),
                    )
                    .child(
                        h_flex()
                            .h(surface::css(45.))
                            .flex_shrink_0()
                            .gap(surface::css(rate_gap))
                            .px(surface::css(rate_gap))
                            .child(
                                div()
                                    .text_size(surface::css(12.))
                                    .text_color(C::white())
                                    .mr(surface::css(label_margin))
                                    .child(text(pid, "hbV").to_uppercase()),
                            )
                            .child(rate_tabs),
                    ),
            )
            .child(
                h_flex()
                    .h(surface::css(45.))
                    .flex_shrink_0()
                    .ml_auto()
                    .child(
                        h_flex()
                            .my(surface::css(16.))
                            .px(surface::css(20.))
                            .gap(surface::css(10.))
                            .child(
                                div()
                                    .text_size(surface::css(12.))
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(C::white())
                                    // `.mic-recording-playback__title`.
                                    .child(
                                        SourceText::new(
                                            "seiren-mic-recording-title",
                                            text(pid, "eQC").to_uppercase(),
                                        )
                                        .letter_spacing(0.5),
                                    ),
                            )
                            .child(h_flex().gap(surface::css(10.)).child(record).child(play)),
                    ),
            );
        div()
            .relative()
            .w_full()
            .h(surface::css(height))
            .flex_shrink_0()
            .bg(C::black())
            .font_family("Roboto")
            .overflow_hidden()
            .when_some(waveform, |footer, waveform| {
                footer.child(
                    div()
                        .absolute()
                        .bottom(surface::css(45.))
                        .left_0()
                        .right_0()
                        .child(waveform),
                )
            })
            .child(controls)
    }
}

fn meter(level: f32, peak: f32, muted: bool) -> Div {
    let level = if muted { 0. } else { level.clamp(0., 100.) / 100. };
    div()
        .relative()
        .w(surface::css(10.))
        .h(surface::css(220.))
        .bg(C::black())
        .overflow_hidden()
        .opacity(if muted { 0.3 } else { 1. })
        .children(
            [
                (0., 2. / 3., C::selected()),
                (2. / 3., 1. / 6., C::meter_yellow()),
                (5. / 6., 1. / 6., C::meter_red()),
            ]
            .into_iter()
            .filter_map(move |(bottom, height, color)| {
                let height = (level - bottom).clamp(0., height);
                (height > 0.).then(|| {
                    div()
                        .absolute()
                        .bottom(relative(bottom))
                        .h(relative(height))
                        .w_full()
                        .bg(color)
                })
            }),
        )
        .child(
            div()
                .absolute()
                .bottom(relative(peak.clamp(0., 100.) / 100.))
                .h(surface::css(4.))
                .w_full()
                .bg(C::peak().opacity(0.7)),
        )
}

fn wizard_connector() -> Div {
    v_flex()
        .w(surface::css(2.))
        .h(surface::css(30.))
        .my(surface::css(10.))
        .gap(surface::css(2.))
        .children((0..8).map(|_| div().w_full().h(surface::css(2.)).bg(C::selected())))
}
/// UI7405 `.modal button`; footer controls use a separate source rule.
fn wizard_button(id: impl Into<ElementId>, primary: bool) -> BaseButton {
    BaseButton::new(id)
        .w(surface::css(90.))
        .h(surface::css(27.))
        .mr(surface::css(10.))
        .p_0()
        .pt(surface::css(6.))
        .pb(surface::css(7.))
        .rounded(surface::css(3.))
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .font_family("Roboto")
        .font_weight(FontWeight::NORMAL)
        .bg(if primary { C::selected() } else { C::remainder() })
        .text_color(if primary { C::black() } else { C::white() })
        .styles(|style| style.disabled(|style| style.opacity(0.5)))
        .when(primary, |button| {
            button.hover(|style| style.bg(C::wizard_primary_hover()))
        })
}
impl AudioProductWorkspace {
    fn seiren_wizard_mic_control(&self, cx: &mut Context<Self>) -> AnyElement {
        let wizard = self.seiren_mic.wizard.as_ref().expect("mounted wizard");
        let state = wizard.fader.as_ref().expect("retained wizard fader");
        let volume = &self.draft["profile"]["micVolume"];
        let muted = volume["isEnabled"] != true;
        let progress = volume["value"].as_f64().unwrap_or(0.).clamp(0., 100.) as f32 / 100.;
        let levels = if self
            .seiren_mic
            .measurement
            .as_ref()
            .is_some_and(|capture| capture.started.is_some())
        {
            wizard.levels
        } else {
            self.seiren_microphone_native_levels()
        };
        v_flex()
            .absolute()
            .left(surface::css(12.))
            .top(relative(0.5))
            .mt(surface::css(-99.2))
            .items_center()
            .child(
                div()
                    .relative()
                    .w(surface::css(24.))
                    .h(surface::css(220.))
                    .child(
                        h_flex()
                            .gap(surface::css(4.))
                            .child(meter(levels[0], wizard.peaks[0], muted))
                            .child(meter(levels[1], wizard.peaks[1], muted)),
                    )
                    .child(
                        BaseSlider::new(state)
                            .vertical()
                            .disabled(muted)
                            .size_full()
                            .absolute()
                            .inset_0()
                            .opacity(if muted { 0.3 } else { 1. })
                            .child(
                                SliderTrack::new(state)
                                    .axis(Axis::Vertical)
                                    .disabled(muted)
                                    .size_full()
                                    .child(
                                        SliderIndicator::new(state).size_full().relative().child(
                                            SliderThumb::new(state)
                                                .axis(Axis::Vertical)
                                                .disabled(muted)
                                                .absolute()
                                                .left(surface::css(2.))
                                                .bottom(relative(progress))
                                                .mb(surface::css(-7.33 / 2.))
                                                .w(surface::css(20.))
                                                .h(surface::css(7.33))
                                                .border_1()
                                                .border_color(C::thumb_border())
                                                .bg(C::thumb()),
                                        ),
                                    ),
                            ),
                    ),
            )
            .child(
                BaseButton::new("seiren-wizard-mute")
                    .p_0()
                    .mt(surface::css(8.))
                    .size(surface::css(20.))
                    .text_color(C::text())
                    .hover(|style| style.text_color(C::selected()))
                    .accessibility_label(t("MICROPHONE"))
                    .child(
                        svg()
                            .path(if muted {
                                "synapse/seiren-eq-mic-off.svg"
                            } else {
                                "synapse/seiren-eq-mic.svg"
                            })
                            .size_full(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        let mut volume = this.draft["profile"]["micVolume"].clone();
                        volume["isEnabled"] = json!(volume["isEnabled"] != true);
                        this.seiren_microphone_set_volume(volume, cx);
                    })),
            )
            .into_any_element()
    }
    pub(crate) fn seiren_microphone_footer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.seiren_microphone_mounted() {
            return None;
        }
        Some(
            MicrophoneFooter {
                owner: cx.weak_entity(),
                pid: self.spec.product_id,
                live: self.seiren_mic.live,
                rate: self.draft["profile"]["samplingRate"]
                    .as_u64()
                    .unwrap_or(48_000),
                recording: self
                    .seiren_mic
                    .record
                    .as_ref()
                    .is_some_and(|record| record.started.is_some()),
                recording_generation: self
                    .seiren_mic
                    .record
                    .as_ref()
                    .map_or(0, |record| record.request.session_id()),
                playing: self.seiren_mic.playing,
                pending: self.seiren_mic.playback.is_some(),
                has_clip: self.seiren_mic.clip.is_some(),
                trigger_focus: self.seiren_mic.trigger_focus.clone(),
                waveform: self
                    .seiren_mic
                    .live
                    .then(|| self.seiren_microphone_waveform()),
            }
            .into_any_element(),
        )
    }
    pub(crate) fn seiren_microphone_wizard(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let wizard = self.seiren_mic.wizard.as_ref()?;
        let pid = self.spec.product_id;
        let content = if wizard.intro {
            v_flex()
                .items_center()
                .px(surface::css(50.))
                .pt(surface::css(20.))
                .pb(surface::css(8.))
                .child(
                    div()
                        .text_center()
                        .text_size(surface::css(14.))
                        .w_full()
                        .max_w(surface::css(600.))
                        .line_height(surface::css(22.4))
                        .mb(surface::css(70.))
                        .child(text(pid, "po7")),
                )
                .child(
                    div()
                        .text_center()
                        .text_size(surface::css(14.))
                        .w_full()
                        .max_w(surface::css(600.))
                        .line_height(surface::css(22.4))
                        .mb(surface::css(10.))
                        .child(text(pid, "iGP")),
                )
                .child(
                    h_flex()
                        .gap(surface::css(20.))
                        .justify_center()
                        .child(
                            wizard_button("seiren-wizard-cancel", false)
                                .child(text(pid, "bOp").to_uppercase())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.close_seiren_microphone_wizard(false, window, cx)
                                })),
                        )
                        .child(
                            wizard_button("seiren-wizard-begin", true)
                                .child(text(pid, "Qay").to_uppercase())
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.seiren_wizard_next(cx)),
                                ),
                        ),
                )
                .into_any_element()
        } else {
            let title = ["kIq", "oV6", "N_T", "v1N", "iuc", "mW3"][wizard.step];
            let mut information = text(pid, title).to_string();
            if wizard.step < 2 {
                information.push_str("\n\n");
                information.push_str(&text(pid, if wizard.step == 0 { "pwH" } else { "_iR" }));
                if wizard.step == 0 {
                    information.push_str("\n\n");
                    information.push_str(&text(pid, "iOp"));
                }
            }
            let mut body = v_flex()
                .relative()
                .items_center()
                .w_full()
                .pt(surface::css(20.))
                .child(
                    div()
                        .text_center()
                        .px(surface::css(50.))
                        .text_size(surface::css(14.))
                        .mb(surface::css(20.))
                        .child(information),
                );
            if wizard.step < 2 {
                body = body.child(wizard_connector()).child(
                    BaseButton::new("seiren-wizard-measure")
                        .relative()
                        .w(surface::css(160.))
                        .min_w(surface::css(160.))
                        .h(surface::css(52.))
                        .p_0()
                        .px(surface::css(16.))
                        .bg(C::surface())
                        .border_1()
                        .border_color(C::border())
                        .rounded(surface::css(3.))
                        .hover(|style| style.border_color(C::selected()))
                        .flex_col()
                        .items_center()
                        .justify_start()
                        .child(
                            img(asset(pid, "measure-record"))
                                .size(surface::css(20.))
                                .mt(surface::css(7.))
                                .mb(surface::css(5.)),
                        )
                        .child(
                            div()
                                .absolute()
                                .bottom(surface::css(6.))
                                .left_0()
                                .w_full()
                                .text_center()
                                .text_size(surface::css(12.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(C::text())
                                .child(text(pid, "gvn")),
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.start_seiren_wizard_measurement(cx);
                            }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.start_seiren_wizard_measurement(cx)
                        })),
                );
                if let Some(measurement) = self
                    .seiren_mic
                    .measurement
                    .as_ref()
                    .filter(|measurement| !measurement.request.is_canceled())
                {
                    let mut status = v_flex()
                        .items_center()
                        .mt(surface::css(20.))
                        .gap(surface::css(12.))
                        .text_size(surface::css(14.))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(C::timer())
                        .child(
                            // Both waiting and elapsed source branches share
                            // `.recording-timer` and its .5px tracking.
                            SourceText::new(
                                "seiren-wizard-recording-timer",
                                measurement
                                    .started
                                    .map(|start| format!("{}s", start.elapsed().as_secs()))
                                    .unwrap_or_else(|| text(pid, "xoD").to_string()),
                            )
                            .letter_spacing(0.5),
                        );
                    if measurement.started.is_some() {
                        status = status.child(
                            // `.modal button` outranks the unqualified stop
                            // class; only its :hover/:active colors win.
                            wizard_button("seiren-wizard-stop-measure", false)
                                .hover(|style| style.bg(C::wizard_stop_hover()))
                                .active(|style| style.bg(C::wizard_stop_pressed()))
                                .child(text(pid, "VWS").to_uppercase())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.stop_seiren_wizard_measurement(true);
                                    cx.notify();
                                })),
                        );
                    }
                    body = body.child(status);
                }
                if let Some(error) = &self.seiren_mic.recording_error {
                    body = body.child(
                        div()
                            .mt(surface::css(20.))
                            .text_color(C::recording_error())
                            .text_size(surface::css(14.))
                            .child(format!("Error: {error}")),
                    );
                }
                body = body.child(self.seiren_wizard_mic_control(cx));
            } else if wizard.step < 5 {
                let options: Vec<(&str, &str)> = match wizard.step {
                    2 => vec![
                        ("VBV", "noise-none"),
                        ("RDC", "noise-fan"),
                        ("_N5", "noise-environment"),
                    ],
                    3 => vec![
                        ("ZDW", "usage-streaming"),
                        ("MOz", "usage-casting"),
                        ("f8_", "usage-gaming"),
                        ("a$P", "usage-music"),
                        ("zmI", "usage-general"),
                    ],
                    _ => vec![("5-15CM", "distance-near"), ("15-30CM", "distance-far")],
                };
                let selected = match wizard.step {
                    2 => wizard.ai,
                    3 => Some(wizard.eq),
                    _ => Some(wizard.distance),
                };
                let step = wizard.step;
                let mut rows = v_flex()
                    .gap(surface::css(10.))
                    .px(surface::css(30.))
                    .items_center();
                for row in options.chunks(3) {
                    let mut line = h_flex().gap(surface::css(10.)).justify_center();
                    for (label, key) in row {
                        let index = options.iter().position(|option| option.1 == *key).unwrap();
                        line = line.child(
                            BaseButton::new(SharedString::from(format!(
                                "seiren-wizard-choice-{key}"
                            )))
                            .w(surface::css(250.))
                            .h(surface::css(180.))
                            .flex_shrink_0()
                            .p_0()
                            .px(surface::css(20.))
                            .pb(surface::css(28.))
                            .rounded(surface::css(5.))
                            .bg(C::surface())
                            .font_family("Roboto")
                            .when(selected == Some(index), |card| {
                                card.shadow(vec![BoxShadow {
                                    color: C::selected(),
                                    offset: point(px(0.), px(0.)),
                                    blur_radius: px(0.),
                                    spread_radius: window.rem_size() * (2. / 16.),
                                    inset: false,
                                }])
                            })
                            .selected(selected == Some(index))
                            .flex_col()
                            .items_stretch()
                            .justify_start()
                            .child(
                                v_flex()
                                    .relative()
                                    .w_full()
                                    .h_full()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            // CSS box-category is content-box:
                                            // height 18 plus padding-top 10.
                                            .w_full()
                                            .h(surface::css(28.))
                                            .pt(surface::css(10.))
                                            .flex_shrink_0()
                                            .text_center()
                                            .text_size(surface::css(14.))
                                            .line_height(surface::css(18.))
                                            .text_color(C::remainder())
                                            .child(
                                                if step == 4 {
                                                    label.to_string()
                                                } else {
                                                    text(pid, label).to_string()
                                                }
                                                .to_uppercase(),
                                            ),
                                    )
                                    .child(
                                        img(asset(pid, key))
                                            .mt_auto()
                                            .flex_shrink_0()
                                            .w(surface::css(210.))
                                            .h(surface::css(118.))
                                            .object_fit(ObjectFit::Contain),
                                    )
                                    // Ca omits image_foot_text, but de still
                                    // mounts its fixed-height empty tag.
                                    .child(div().w_full().h(surface::css(28.)).flex_shrink_0()),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.seiren_wizard_select(step, index, window, cx);
                                }),
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.seiren_wizard_select(step, index, window, cx)
                                },
                            )),
                        );
                    }
                    rows = rows.child(line);
                }
                body = body.child(wizard_connector()).child(rows);
            } else {
                let value = |value: Option<f64>, gain: bool| {
                    value
                        .map(|value| {
                            if gain {
                                format!("{:+.1} dB", value + 4.)
                            } else {
                                format!("{value} dB")
                            }
                        })
                        .unwrap_or_else(|| "Calculating...".into())
                };
                let eq = ["Podcast", "Arena", "FPS-Callout", "Studio", "Custom"][wizard.eq_result];
                let mut settings = v_flex()
                    .w_full()
                    .max_w(surface::css(500.))
                    .px(surface::css(50.));
                for (label, value) in [
                    ("u1l", value(wizard.gain, true)),
                    ("fVe", value(wizard.noise, false)),
                    (
                        "G7g",
                        format!(
                            "{}{}",
                            wizard.ai_result,
                            if wizard.ai_result == 0 { "" } else { "%" }
                        ),
                    ),
                    ("K79", eq.into()),
                    (
                        "cFh",
                        if wizard.bass_result == 0 { "0 dB" } else { "+3 dB" }.into(),
                    ),
                    (
                        "BAn",
                        if wizard.exciter_result == 0 {
                            "0 dB"
                        } else {
                            "+2 dB"
                        }
                        .into(),
                    ),
                ] {
                    settings = settings.child(
                        h_flex()
                            .justify_between()
                            .text_size(surface::css(14.))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("{}:", text(pid, label))),
                            )
                            .child(value),
                    );
                }
                body = body.child(settings.min_h(surface::css(200.)));
            }
            let final_step = wizard.step == 5;
            let disabled = (wizard.step == 0 && !wizard.speech_done)
                || (wizard.step == 1 && !wizard.environment_done);
            body.child(
                h_flex()
                    .mt(surface::css(50.))
                    .justify_center()
                    .child(
                        wizard_button("seiren-wizard-back", false)
                            .child(text(pid, if final_step { "uqI" } else { "IUp" }).to_uppercase())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if final_step {
                                    this.restart_seiren_wizard(cx);
                                } else {
                                    this.seiren_wizard_back(cx);
                                }
                            })),
                    )
                    .child(
                        wizard_button("seiren-wizard-next", true)
                            .disabled(disabled)
                            .child(text(pid, if final_step { "pJk" } else { "K29" }).to_uppercase())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if final_step {
                                    this.close_seiren_microphone_wizard(true, window, cx);
                                } else {
                                    this.seiren_wizard_next(cx);
                                }
                            })),
                    ),
            )
            .into_any_element()
        };
        Some(
            Dialog::new(cx)
                .focus_handle(
                    self.seiren_mic
                        .focus
                        .as_ref()
                        .expect("microphone modal focus")
                        .clone(),
                )
                .close_on_backdrop_press(false)
                .on_ok(|_, _, _| false)
                .on_cancel({
                    // `on_cancel` owns the dismissal decision, so the wizard is
                    // closed through the entity and the decision returned.
                    let entity = cx.entity().downgrade();
                    move |_, window, cx| {
                        let _ = entity.update(cx, |this, cx| {
                            this.close_seiren_microphone_wizard(false, window, cx)
                        });
                        true
                    }
                })
                .backdrop(div().size_full().bg(C::wizard_overlay()))
                .popup(razer_assets::resource_scope(
                    v_flex()
                        .min_w(surface::css(544.))
                        .max_w(relative(0.9))
                        .max_h(relative(0.9))
                        .pb(surface::css(16.))
                        .border_1()
                        .border_color(C::wizard_border())
                        .bg(C::knob())
                        .rounded(surface::css(5.))
                        .text_color(C::text())
                        .font_family("Roboto")
                        .child(
                            h_flex()
                                .relative()
                                .h(surface::css(36.))
                                .justify_center()
                                .border_b_1()
                                .border_color(C::wizard_border())
                                .text_size(surface::css(14.))
                                .text_color(C::thumb())
                                .line_height(surface::css(36.))
                                .child(text(pid, "t5W").to_uppercase())
                                .child(
                                    BaseButton::new("seiren-wizard-close")
                                        .absolute()
                                        .right(surface::css(5.))
                                        .p_0()
                                        .size(surface::css(20.))
                                        .cursor_pointer()
                                        .accessibility_label(t("CLOSE"))
                                        .child(
                                            img("synapse/seiren-eq-close.svg")
                                                .size(surface::css(20.)),
                                        )
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.close_seiren_microphone_wizard(false, window, cx)
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .id("seiren-wizard-body")
                                .min_h_0()
                                .overflow_y_scroll()
                                .child(content),
                        ),
                ))
                .into_any_element(),
        )
    }
}

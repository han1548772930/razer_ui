//! Source UI7405 footer/wizard presentation. Original CSS sizes are expressed
//! through the product scale; Base owns buttons, dialog focus and dismissal.
use super::super::audio_seiren_eq::colors::Colors as C;
use super::*;
use gpui_kit::base::{
    Button as BaseButton, Dialog, Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack,
};

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

fn button(id: impl Into<ElementId>, primary: bool) -> BaseButton {
    BaseButton::new(id)
        .h(surface::css(27.))
        .min_w(surface::css(90.))
        .px(surface::css(12.))
        .rounded(surface::css(3.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(surface::css(12.))
        .font_family("Roboto")
        .bg(if primary { C::selected() } else { C::remainder() })
        .text_color(if primary { C::black() } else { C::white() })
        .styles(|style| style.disabled(|style| style.opacity(0.3)))
        .hover(|style| style.opacity(0.9))
        .active(|style| style.opacity(0.8))
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
        let pid = self.spec.product_id;
        let live = self.seiren_mic.live;
        let rate = self.draft["profile"]["samplingRate"]
            .as_u64()
            .unwrap_or(48_000);
        let recording = self.seiren_mic.record.is_some();
        let playing = self.seiren_mic.playing;
        let pending = self.seiren_mic.playback.is_some();
        Some(
            v_flex()
                .w_full()
                .flex_shrink_0()
                .bg(C::black())
                .font_family("Roboto")
                .when(live, |footer| {
                    footer.child(self.seiren_microphone_waveform())
                })
                .child(
                    h_flex()
                        .h(surface::css(45.))
                        .min_h(surface::css(45.))
                        .w_full()
                        .px(surface::css(20.))
                        .justify_between()
                        .gap(surface::css(20.))
                        .child(
                            h_flex()
                                .gap(surface::css(20.))
                                .flex_shrink_0()
                                .child(
                                    button("seiren-mic-live-preview", live)
                                        .min_w(surface::css(134.))
                                        .h(surface::css(26.))
                                        .rounded(surface::css(26.))
                                        .selected(live)
                                        .child(text(pid, if live { "s7T" } else { "KLv" }))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.seiren_mic.live = !this.seiren_mic.live;
                                            this.sync_seiren_microphone_runtime(cx);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    button("seiren-mic-setup", false)
                                        .bg(C::active())
                                        .border_1()
                                        .border_color(C::text())
                                        .min_w(surface::css(104.))
                                        .h(surface::css(34.))
                                        .when_some(
                                            self.seiren_mic.trigger_focus.as_ref(),
                                            |button, focus| button.track_focus(focus),
                                        )
                                        .child(text(pid, "_jK"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.open_seiren_microphone_wizard(window, cx)
                                        })),
                                )
                                .child(
                                    h_flex()
                                        .gap(surface::css(4.))
                                        .child(
                                            div()
                                                .text_size(surface::css(12.))
                                                .text_color(C::white())
                                                .child(text(pid, "hbV")),
                                        )
                                        .child(
                                            h_flex()
                                                .w(surface::css(135.))
                                                .h(surface::css(36.))
                                                .border_1()
                                                .border_color(C::border())
                                                .rounded(surface::css(36.))
                                                .p(surface::css(5.))
                                                .gap(surface::css(5.))
                                                .children([48_000u32, 96_000].into_iter().map(
                                                    |option| {
                                                        button(
                                                            ("seiren-mic-rate", option),
                                                            rate == u64::from(option),
                                                        )
                                                        .min_w_0()
                                                        .flex_1()
                                                        .h_full()
                                                        .rounded(surface::css(36.))
                                                        .selected(rate == u64::from(option))
                                                        .child(format!("{}kHz", option / 1000))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.seiren_sampling_rate(
                                                                    option, cx,
                                                                )
                                                            },
                                                        ))
                                                    },
                                                )),
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap(surface::css(12.))
                                .flex_shrink_0()
                                .child(
                                    div()
                                        .text_size(surface::css(14.))
                                        .text_color(C::text())
                                        .child(text(pid, "eQC")),
                                )
                                .child(
                                    button("seiren-mic-record", false)
                                        .min_w(surface::css(30.))
                                        .w(surface::css(30.))
                                        .px_0()
                                        .bg(C::black())
                                        .selected(recording)
                                        .disabled(playing)
                                        .accessibility_label(text(
                                            pid,
                                            if recording { "$pN" } else { "RLI" },
                                        ))
                                        .child(
                                            img(asset(pid, "record"))
                                                .size(surface::css(20.))
                                                .when(recording, |image| image.opacity(0.7)),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_seiren_recording(cx)
                                        })),
                                )
                                .child(
                                    button("seiren-mic-play", false)
                                        .min_w(surface::css(30.))
                                        .w(surface::css(30.))
                                        .px_0()
                                        .bg(C::black())
                                        .disabled(
                                            self.seiren_mic.clip.is_none()
                                                || recording
                                                || (pending && !playing),
                                        )
                                        .selected(playing)
                                        .accessibility_label(text(
                                            pid,
                                            if playing { "VWS" } else { "FoH" },
                                        ))
                                        .child(if pending && !playing {
                                            gpui_kit::component::spinner::Spinner::new()
                                                .small()
                                                .into_any_element()
                                        } else {
                                            img(asset(pid, if playing { "stop" } else { "play" }))
                                                .size(surface::css(20.))
                                                .into_any_element()
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_seiren_playback(cx)
                                        })),
                                ),
                        ),
                )
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
                        .text_color(C::timer())
                        .child(
                            measurement
                                .started
                                .map(|start| format!("{}s", start.elapsed().as_secs()))
                                .unwrap_or_else(|| text(pid, "xoD").to_string()),
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
                .popup(
                    razer_assets::resource_scope(v_flex()
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
                                    button("seiren-wizard-close", false)
                                        .absolute()
                                        .right(surface::css(5.))
                                        .min_w(surface::css(24.))
                                        .w(surface::css(24.))
                                        .bg(C::knob())
                                        .accessibility_label(t("CLOSE"))
                                        .child(
                                            img("synapse/seiren-eq-close.svg")
                                                .size(surface::css(16.)),
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
                        )),
                )
                .into_any_element(),
        )
    }
}

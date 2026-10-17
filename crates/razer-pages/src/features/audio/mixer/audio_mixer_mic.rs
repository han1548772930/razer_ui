//! PID1342 mounted MU/useUU/AM/lU from current main.2623357c.js.
//! Layout and presentation: main.07a2c4ed.css, resources manifest in products/1342.
use super::audio_mixer_number::{Number, Props};
use super::audio_mixer_theme::Colors as C;
use super::*;
use gpui_kit::base::Button as BaseButton;
use razer_widgets::source_slider::SourceSlider;

pub(super) struct State {
    sliders: Vec<Entity<SliderState>>,
    number: Entity<Number>,
    pub preview_popup: bool,
    /// Only real observations populate this. Zero before observation is the
    /// original audioMeterReducer's initial presentation, not a read receipt.
    pub meter: Option<[f32; 2]>,
}

fn gain_range(mode: usize) -> (f32, f32) {
    if mode == 0 { (68., 0.01) } else { (40., 0.1) }
}

impl AudioProductWorkspace {
    /// Current mounted useMg lives beside the profile bar, including while
    /// Effects disables profile selection. It never toggles preview on page exit.
    pub fn mixer_preview_indicator(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.spec.product_id != 1342 || self.draft["device"]["micPreview"] != true {
            return None;
        }
        let state = self.mixer_mic.as_ref()?;
        let mode = if self.draft["device"]["micSetup"]["useMode"] == 0 {
            "xlr"
        } else {
            "mic"
        };
        let gain = &self.draft["device"]["micSetup"][mode]["value"];
        let show_levels = self.draft["device"]["micMonitor"]["value"] == 80
            && gain.as_f64().is_some_and(|gain| gain != 0.)
            && gain == &self.draft["device"]["outputMixerReducer"][mode]["value"];
        Some(
            div()
                .relative()
                .ml(surface::css(10.))
                .child(
                    BaseButton::new("audio-mic-preview-status")
                        .w(surface::css(26.))
                        .h(surface::css(26.))
                        .rounded(surface::css(3.))
                        .bg(C::preview_warning())
                        .flex()
                        .items_center()
                        .justify_center()
                        .accessibility_label(t("MIC_SETUP_MODE"))
                        .child(
                            svg()
                                .path("synapse/audio-mixer-mic-preview.svg")
                                .w(surface::css(12.))
                                .h(surface::css(19.))
                                .text_color(C::surface()),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(mic) = &mut this.mixer_mic {
                                mic.preview_popup = true;
                            }
                            cx.notify();
                        })),
                )
                .when(state.preview_popup, |indicator| {
                    indicator.child(
                        razer_assets::deferred_resource(
                            v_flex()
                                .id("audio-mic-preview-popup")
                                .absolute()
                                .left(surface::css(-237.))
                                .top(surface::css(51.))
                                .w(surface::css(500.))
                                .p(surface::css(20.))
                                .gap(surface::css(20.))
                                .bg(C::black())
                                .border_1()
                                .border_color(C::preview_warning())
                                .rounded(surface::css(3.))
                                .text_size(surface::css(14.))
                                .items_center()
                                .occlude()
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    if let Some(mic) = &mut this.mixer_mic {
                                        mic.preview_popup = false;
                                    }
                                    cx.notify();
                                }))
                                .child(
                                    BaseButton::new("audio-mic-preview-popup-close")
                                        .absolute()
                                        .right(surface::css(10.))
                                        .top(surface::css(10.))
                                        .w(surface::css(15.))
                                        .h(surface::css(15.))
                                        .text_color(C::text())
                                        .accessibility_label(t("CLOSE"))
                                        .child(
                                            svg()
                                                .path("synapse/audio-mixer-mic-preview-close.svg")
                                                .size(surface::css(15.))
                                                .text_color(C::text()),
                                        )
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(mic) = &mut this.mixer_mic {
                                                mic.preview_popup = false;
                                            }
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .text_color(C::preview_warning())
                                        .child(t("MIC_SETUP_MODE").to_uppercase()),
                                )
                                .child(
                                    v_flex()
                                        .items_center()
                                        .text_center()
                                        .text_color(C::text())
                                        .child(t("MIC_SETUP_DESC"))
                                        .when(show_levels, |description| {
                                            description.child(t("MIC_OUT100")).child(t("MIC_OUT80"))
                                        }),
                                )
                                .child(
                                    BaseButton::new("audio-mic-preview-disable")
                                        .w(surface::css(90.))
                                        .h(surface::css(26.))
                                        .rounded(surface::css(3.))
                                        .bg(C::preview_warning())
                                        .text_color(C::black())
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(t("DISABLE").to_uppercase())
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.edit(
                                                "/device/micPreview",
                                                json!(false),
                                                window,
                                                cx,
                                            )
                                        })),
                                ),
                        )
                        .priority(12),
                    )
                })
                .into_any_element(),
        )
    }

    pub(super) fn initialize_mixer_mic_draft(&mut self) {
        if self.spec.product_id != 1342 {
            return;
        }
        self.draft["device"]["micSetup"] = json!({"useMode":0,
            "xlr":{"isEnabled":false,"value":2.04,"min":0,"max":68,"step":0.01},
            "mic":{"value":0,"min":0,"max":40,"step":0.1}});
        self.draft["device"]["micSetupShowPercentage"] = json!(true);
        self.draft["device"]["micPreview"] = json!(false);
        self.draft["device"]["micMonitor"] = json!({"isEnabled":true,"value":80});
        // Current MW main.6861 module26061 supplies the persisted initial
        // values; the UI reducer zeroes are only its pre-load presentation.
        self.draft["device"]["outputMixerReducer"]["xlr"] =
            json!({"value":0.02,"min":0,"max":68,"step":0.01});
        self.draft["device"]["outputMixerReducer"]["mic"] =
            json!({"value":0.01,"min":0,"max":40,"step":0.1});
    }

    pub(super) fn initialize_mixer_mic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id != 1342 {
            return;
        }
        let mut sliders = Vec::new();
        for mode in 0..2 {
            for percent in [false, true] {
                let (max, step) = if percent { (100., 1.) } else { gain_range(mode) };
                let slider = cx.new(|_| {
                    SliderState::new()
                        .min(0.)
                        .max(max)
                        .step(step)
                        .default_value(0.)
                });
                self.subscriptions.push(cx.subscribe_in(
                    &slider,
                    window,
                    move |this, _, event, window, cx| {
                        if this.syncing || this.mixer_mic_mode() != (mode, percent) {
                            return;
                        }
                        if let SliderEvent::Change(value) = event {
                            this.change_mixer_mic_gain(value.start(), true, window, cx);
                        }
                    },
                ));
                sliders.push(slider);
            }
        }
        let props = self.mixer_mic_number_props();
        let number = cx.new(|cx| Number::new(props, window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &number,
            window,
            |this, _, value: &f32, window, cx| {
                if !this.syncing {
                    this.change_mixer_mic_gain(*value, false, window, cx);
                }
            },
        ));
        self.mixer_mic = Some(State {
            sliders,
            number,
            preview_popup: false,
            meter: None,
        });
        self.add_slider("/device/micMonitor/value", 0., 100., 1., window, cx);
    }

    fn mixer_mic_mode(&self) -> (usize, bool) {
        (
            usize::from(self.draft["device"]["micSetup"]["useMode"] == 1),
            self.draft["device"]["micSetupShowPercentage"]
                .as_bool()
                .unwrap_or(true),
        )
    }

    fn mixer_mic_gain(&self) -> f32 {
        let (mode, percent) = self.mixer_mic_mode();
        let key = if mode == 0 { "xlr" } else { "mic" };
        let gain = self.draft["device"]["micSetup"][key]["value"]
            .as_f64()
            .unwrap_or(0.) as f32;
        if percent {
            (gain / gain_range(mode).0 * 100.).round()
        } else {
            gain
        }
    }

    fn change_mixer_mic_gain(
        &mut self,
        displayed: f32,
        from_slider: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (mode, percent) = self.mixer_mic_mode();
        let (max, step) = gain_range(mode);
        let factor = if step < 0.1 { 100. } else { 10. };
        let gain = if percent {
            (displayed * max / 100. * factor).round() / factor
        } else {
            displayed
        };
        let key = if mode == 0 { "xlr" } else { "mic" };
        let previous_output = self.draft["device"]["outputMixerReducer"][key]["value"].as_f64();
        self.edit(
            &format!("/device/micSetup/{key}/value"),
            json!(gain),
            window,
            cx,
        );
        // useUU clamps the output gain only from the range handler, not EU.
        if from_slider && previous_output.is_some_and(|value| value > f64::from(gain)) {
            self.edit(
                &format!("/device/outputMixerReducer/{key}/value"),
                json!(gain),
                window,
                cx,
            );
        }
    }

    fn mixer_mic_number_props(&self) -> Props {
        let (mode, percent) = self.mixer_mic_mode();
        Props {
            value: self.mixer_mic_gain(),
            min: 0.,
            max: if percent { 100. } else { gain_range(mode).0 },
            // useUU supplies allowDecimals=true in both display modes. Current
            // CSS hides the input; only EU's original spinner controls remain.
            decimals: true,
            max_len: if percent { 3 } else { 2 },
            tag: if percent { "%" } else { "" },
            input_visible: false,
        }
    }

    pub(super) fn sync_mixer_mic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (mode, percent) = self.mixer_mic_mode();
        let value = self.mixer_mic_gain();
        let props = self.mixer_mic_number_props();
        let Some(state) = &mut self.mixer_mic else {
            return;
        };
        state.sliders[mode * 2 + usize::from(percent)]
            .update(cx, |slider, cx| slider.set_value(value, window, cx));
        state
            .number
            .update(cx, |number, cx| number.observe(props, window, cx));
    }

    pub(super) fn mixer_mic_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let sections = &self
            .spec
            .pages
            .iter()
            .find(|page| page.key == "TAB_MIC")
            .unwrap()
            .sections;
        let noise = sections
            .iter()
            .find(|section| section.title == "NOISE_GATE")
            .unwrap();
        let compressor = sections
            .iter()
            .find(|section| section.title == "COMPRESSOR")
            .unwrap();
        surface::page_columns()
            .child(surface::page_column(
                v_flex()
                    .gap_0()
                    .child(self.mixer_mic_setup(cx))
                    .child(self.mixer_mic_monitor(cx))
                    .child(self.mixer_dynamics_panel(noise, cx)),
            ))
            .child(surface::page_column(
                v_flex()
                    .gap_0()
                    .child(self.mixer_mic_equalizer(cx))
                    .child(self.mixer_dynamics_panel(compressor, cx)),
            ))
            .into_any_element()
    }

    fn mixer_mic_monitor(&self, cx: &mut Context<Self>) -> AnyElement {
        let active = self.draft["device"]["playbackMixReducer"]["mic"]["isEnabled"]
            .as_bool()
            .unwrap_or(true);
        let value = self.draft["device"]["micMonitor"]["value"]
            .as_f64()
            .unwrap_or(0.);
        surface::panel_with_title_switch(
            t("MIC_MONITOR"),
            surface::SynapseSwitch::new("mixer-mic-monitor")
                .accessibility_label(t("MIC_MONITOR"))
                .checked(active)
                .on_change(cx.listener(|this, active: &bool, window, cx| {
                    this.edit(
                        "/device/playbackMixReducer/mic/isEnabled",
                        json!(*active),
                        window,
                        cx,
                    )
                })),
            surface::help_control(
                "mixer-mic-monitor-help",
                t("MIC_MONITOR_TOOLTIP").to_string(),
            ),
            cx,
        )
        .when_some(
            self.sliders.get("/device/micMonitor/value"),
            |panel, slider| {
                panel.child(
                    SourceSlider::new(slider, value as f32 / 100.)
                        .tip(Some(format!("{value:.0}")))
                        .enabled(active),
                )
            },
        )
        .into_any_element()
    }

    fn mixer_mic_setup(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.mixer_mic.as_ref().expect("current Mic Setup state");
        let (mode, percent) = self.mixer_mic_mode();
        let active = self.draft["device"]["micSetup"]["xlr"]["isEnabled"]
            .as_bool()
            .unwrap_or(false);
        let max = if percent { 100. } else { gain_range(mode).0 };
        let value = self.mixer_mic_gain();
        let preview = self.draft["device"]["micPreview"]
            .as_bool()
            .unwrap_or(false);
        let owner = cx.entity();
        let ports = h_flex()
            .gap(surface::css(10.))
            .mb(surface::css(23.))
            .children(
                [
                    (0_usize, "XLR_IN", "synapse/audio-mixer-mic-xlr.svg"),
                    (1, "MIC_IN", "synapse/audio-mixer-mic-jack.svg"),
                ]
                .into_iter()
                .map(|(index, label, icon)| {
                    let owner = owner.clone();
                    gpui_kit::base::Radio::new(("mixer-mic-port", index))
                        .accessibility_label(t(label))
                        .checked(mode == index)
                        .w(surface::css(120.))
                        .h(surface::css(62.))
                        .rounded(surface::css(5.))
                        .border_1()
                        .border_color(if mode == index { C::selected() } else { C::text() })
                        .bg(if mode == index {
                            rgb(0x292929).into()
                        } else {
                            C::surface()
                        })
                        .hover(|style| style.bg(rgb(0x292929)))
                        .child(
                            v_flex()
                                .items_center()
                                .w_full()
                                .px(surface::css(20.))
                                .child(img(icon).w(surface::css(30.)).mb(surface::css(10.)))
                                .child(
                                    div()
                                        .text_size(surface::css(12.))
                                        .truncate()
                                        .child(t(label).to_uppercase()),
                                ),
                        )
                        .on_change(move |_, _, window, cx| {
                            owner.update(cx, |this, cx| {
                                this.edit("/device/micSetup/useMode", json!(index), window, cx)
                            })
                        })
                }),
            );
        let number = div()
            .ml(surface::css(20.))
            .mr(surface::css(5.))
            .child(state.number.clone());
        let units = h_flex()
            .p(surface::css(4.))
            .rounded(surface::css(16.))
            .border_1()
            .border_color(C::border())
            .hover(|style| style.border_color(C::selected()))
            .children(
                [(true, "%"), (false, "dB")]
                    .into_iter()
                    .map(|(selected, label)| {
                        BaseButton::new(("mixer-mic-unit", usize::from(selected)))
                            .w(surface::css(30.))
                            .h(surface::css(22.))
                            .rounded(surface::css(11.))
                            .bg(if percent == selected {
                                C::selected()
                            } else {
                                C::surface()
                            })
                            .text_color(if percent == selected {
                                C::surface()
                            } else {
                                C::value()
                            })
                            .child(label)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit(
                                    "/device/micSetupShowPercentage",
                                    json!(!percent),
                                    window,
                                    cx,
                                )
                            }))
                    }),
            );
        let gain = h_flex()
            .justify_between()
            .child(
                h_flex()
                    .child(t("MIC_GAIN"))
                    .child(number)
                    .when(!percent, |row| row.child("DB")),
            )
            .child(units);
        let meter = v_flex().mt(surface::css(23.)).children([1, 0].map(|index| {
            let level = state
                .meter
                .map(|meter| meter[index])
                .unwrap_or(0.)
                .clamp(0., 100.);
            h_flex()
                .relative()
                .mb(surface::css(3.))
                .h(surface::css(4.))
                .child(
                    div()
                        .flex_basis(relative(348. / 520.))
                        .h_full()
                        .bg(C::selected()),
                )
                .child(
                    div()
                        .flex_basis(relative(94. / 520.))
                        .h_full()
                        .bg(C::meter_yellow()),
                )
                .child(
                    div()
                        .flex_basis(relative(78. / 520.))
                        .h_full()
                        .bg(C::meter_red()),
                )
                .child(
                    div()
                        .absolute()
                        .right_0()
                        .top_0()
                        .h_full()
                        .w(relative((100. - level) / 100.))
                        .bg(C::surface())
                        .opacity(0.8),
                )
        }));
        let preview_button = BaseButton::new("mixer-mic-preview")
            .mt(surface::css(20.))
            .h(surface::css(27.))
            .px(surface::css(16.))
            .py(surface::css(4.))
            .rounded(surface::css(3.))
            .border_1()
            .border_color(rgb(if preview { 0xfd8611 } else { 0x969696 }))
            .bg(rgb(if preview { 0xfd8611 } else { 0x292929 }))
            .child(
                h_flex()
                    .gap(surface::css(8.))
                    .child(
                        img("synapse/audio-mixer-mic-preview.svg")
                            .w(surface::css(12.))
                            .h(surface::css(19.)),
                    )
                    .child(
                        div()
                            .text_size(surface::css(12.))
                            .text_color(rgb(if preview { 0x212121 } else { 0xffffff }))
                            .child(t("MIC_SETUP").to_uppercase()),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.edit("/device/micPreview", json!(!preview), window, cx)
            }));
        surface::panel_with_help(
            t("MIC_SETUP"),
            surface::help_control(
                "mixer-mic-setup-help",
                format!(
                    "{}\n{}",
                    t("MIC_SETUP_TOOLTIP"),
                    t("MIC_SETUP_TOOLTIP_WARNING")
                ),
            ),
            cx,
        )
        .child(ports)
        .when(mode == 0, |panel| {
            panel.child(
                h_flex()
                    .my(surface::css(23.))
                    .gap(surface::css(10.))
                    .child(t("PHANTOM_POWER_48V"))
                    .child(
                        surface::SynapseSwitch::new("mixer-mic-phantom")
                            .accessibility_label(t("PHANTOM_POWER_48V"))
                            .checked(active)
                            .on_change(cx.listener(|this, active: &bool, window, cx| {
                                this.edit(
                                    "/device/micSetup/xlr/isEnabled",
                                    json!(*active),
                                    window,
                                    cx,
                                )
                            })),
                    ),
            )
        })
        .child(gain)
        .child(
            SourceSlider::new(&state.sliders[mode * 2 + usize::from(percent)], value / max)
                .tip(None::<String>)
                .range_labels(
                    if percent { "0" } else { "0 DB" },
                    format!("{max} {}", if percent { "" } else { "DB" }),
                    C::muted(),
                ),
        )
        .child(meter)
        .child(preview_button)
        .into_any_element()
    }
}

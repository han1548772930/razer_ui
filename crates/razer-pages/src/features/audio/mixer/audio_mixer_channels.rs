//! Current 1342 mounted Ag/useG/useTg/$U mixer page, main.2623357c.js.
//! Three-column output knobs, four destination cards, source 500 ms shared
//! output timer and immediate destination volume/routing actions.
use super::audio_mixer_knob::MixerKnob;
use super::audio_mixer_number::{Number, Props};
use super::audio_mixer_theme::Colors as C;
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::source_slider::SourceSlider;

pub(super) const CHANNELS: [(&str, &str); 7] = [
    ("mic", "MIC"),
    ("chat", "CHAT"),
    ("music", "MUSIC"),
    ("game", "GAME"),
    ("optical", "OPTICAL"),
    ("lineIn", "LINE_IN"),
    ("system", "SYSTEM"),
];
const BUSES: [(&str, &str, &str, usize); 4] = [
    ("playbackMixReducer", "PLAYBACK_MIX", "playback-mix", 1),
    ("streamMixReducer", "STREAM_MIX_HEADER", "stream-mix", 2),
    ("lineOutReducer", "LINE_OUT", "line-out", 3),
    ("voiceChatReducer", "VOICE_CHAT", "voice-chat", 4),
];
#[derive(Default)]
pub(super) struct State {
    sliders: BTreeMap<String, Entity<SliderState>>,
    numbers: BTreeMap<String, Entity<Number>>,
    output_preview: Option<Value>,
    output_timer: Option<Task<()>>,
    warning_active: bool,
    warning_dismissed: bool,
}
impl State {
    pub fn cancel(&mut self) {
        self.output_preview = None;
        self.output_timer = None;
        self.warning_active = false;
        self.warning_dismissed = false;
    }
    pub fn observe_output(&mut self) {
        self.output_preview = None;
    }
    pub fn remount(&mut self) {
        self.output_preview = None;
        self.warning_active = false;
        self.warning_dismissed = false;
    }
}

fn percent(value: f32, min: f32, max: f32) -> f32 {
    if min == max {
        0.
    } else {
        ((value.clamp(min, max) - min) / (max - min) * 100.).round()
    }
}
fn raw_value(percent: f32, min: f32, max: f32, step: f32) -> f32 {
    let decimals = step
        .to_string()
        .split_once('.')
        .map_or(0, |(_, tail)| tail.len());
    let factor = 10_f32.powi(decimals as i32);
    ((min + (max - min) * percent / 100.) * factor).round() / factor
}

impl AudioProductWorkspace {
    fn mixer_channel_range(&self, reducer: &str, channel: &str) -> (f32, f32, f32) {
        let value = &self.draft["device"][reducer][channel];
        let min = value["min"].as_f64().unwrap_or(0.) as f32;
        let max = if reducer == "outputMixerReducer" && matches!(channel, "mic" | "xlr") {
            self.draft["device"]["micSetup"][channel]["value"]
                .as_f64()
                .unwrap_or(0.) as f32
        } else {
            value["max"].as_f64().unwrap_or(100.) as f32
        };
        (min, max, value["step"].as_f64().unwrap_or(1.) as f32)
    }
    fn mixer_bus_props(&self, reducer: &str) -> Props {
        let (min, max, _) = self.mixer_channel_range(reducer, "volume");
        let percent_mode = self.draft["device"][reducer]["showPercentage"] == true;
        let value = self.draft["device"][reducer]["volume"]["value"]
            .as_f64()
            .unwrap_or(0.) as f32;
        Props {
            value: if percent_mode {
                percent(value, min, max)
            } else {
                value
            },
            min: if percent_mode { 0. } else { min },
            max: if percent_mode { 100. } else { max },
            decimals: !percent_mode,
            max_len: if percent_mode { 3 } else { 6 },
            tag: if percent_mode { "%" } else { "dB" },
            input_visible: true,
        }
    }
    pub(super) fn initialize_mixer_channels(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 1342 {
            return;
        }
        let rows = std::iter::once(("outputMixerReducer", "xlr"))
            .chain(CHANNELS.map(|(channel, _)| ("outputMixerReducer", channel)))
            .chain(BUSES.map(|(reducer, _, _, _)| (reducer, "volume")));
        for (reducer, channel) in rows {
            let path = format!("/device/{reducer}/{channel}/value");
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(100.)
                    .step(1.)
                    .default_value(0.)
            });
            self.subscriptions.push(cx.subscribe_in(
                &slider,
                window,
                move |this, _, event, window, cx| {
                    if this.syncing {
                        return;
                    }
                    if let SliderEvent::Change(value) = event {
                        let (min, max, step) = this.mixer_channel_range(reducer, channel);
                        let value = if this.draft["device"][reducer]["showPercentage"] == true {
                            raw_value(value.start(), min, max, step)
                        } else {
                            value.start()
                        };
                        if reducer == "outputMixerReducer" {
                            this.preview_mixer_output(channel, value, window, cx);
                        } else {
                            this.edit(
                                &format!("/device/{reducer}/volume/value"),
                                json!(value),
                                window,
                                cx,
                            );
                        }
                    }
                },
            ));
            self.mixer_channels.sliders.insert(path, slider);
        }
        for (reducer, _, _, _) in BUSES {
            let props = self.mixer_bus_props(reducer);
            let number = cx.new(|cx| Number::new(props, window, cx));
            self.subscriptions.push(cx.subscribe_in(
                &number,
                window,
                move |this, _, value: &f32, window, cx| {
                    if this.syncing {
                        return;
                    }
                    let (min, max, step) = this.mixer_channel_range(reducer, "volume");
                    let value = if this.draft["device"][reducer]["showPercentage"] == true {
                        raw_value(*value, min, max, step)
                    } else {
                        *value
                    };
                    this.edit(
                        &format!("/device/{reducer}/volume/value"),
                        json!(value),
                        window,
                        cx,
                    );
                },
            ));
            self.mixer_channels.numbers.insert(reducer.into(), number);
        }
    }
    pub(super) fn sync_mixer_channels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id != 1342 {
            return;
        }
        for (path, slider) in &self.mixer_channels.sliders {
            let mut parts = path.split('/').skip(2);
            let reducer = parts.next().unwrap();
            let channel = parts.next().unwrap();
            let value = if reducer == "outputMixerReducer" {
                self.mixer_channels
                    .output_preview
                    .as_ref()
                    .unwrap_or(&self.draft["device"][reducer])[channel]["value"]
                    .as_f64()
                    .unwrap_or(0.) as f32
            } else {
                self.draft["device"][reducer][channel]["value"]
                    .as_f64()
                    .unwrap_or(0.) as f32
            };
            let (min, max, step) = self.mixer_channel_range(reducer, channel);
            let (shown, min, max, step) = if self.draft["device"][reducer]["showPercentage"] == true
            {
                (percent(value, min, max), 0., 100., 1.)
            } else {
                (value, min, max, step)
            };
            slider.update(cx, |slider, cx| {
                // 0.7.3 exposes range builders, not mutable range setters.
                // Preserve the entity/subscription and replace only on a source
                // unit/range change (which remounts CU in the original UI).
                if (slider.min_value(), slider.max_value(), slider.step_value()) != (min, max, step)
                {
                    *slider = SliderState::new().min(min).max(max).step(step);
                }
                slider.set_value(shown, window, cx);
            });
        }
        for (reducer, number) in &self.mixer_channels.numbers {
            let props = self.mixer_bus_props(reducer);
            number.update(cx, |number, cx| number.observe(props, window, cx));
        }
        let output = self
            .mixer_channels
            .output_preview
            .as_ref()
            .unwrap_or(&self.draft["device"]["outputMixerReducer"]);
        let selected = if self.draft["device"]["micSetup"]["useMode"] == 0 {
            "xlr"
        } else {
            "mic"
        };
        let mic_gain = self.draft["device"]["micSetup"][selected]["value"]
            .as_f64()
            .unwrap_or(0.) as f32;
        let warning = output.as_object().is_some_and(|fields| {
            fields.iter().any(|(channel, value)| {
                let Some(min) = value["min"].as_f64() else {
                    return false;
                };
                let max = if matches!(channel.as_str(), "xlr" | "mic") {
                    mic_gain
                } else {
                    value["max"].as_f64().unwrap_or(0.) as f32
                };
                percent(
                    value["value"].as_f64().unwrap_or(0.) as f32,
                    min as f32,
                    max,
                ) >= 50.
            })
        });
        if warning && !self.mixer_channels.warning_active {
            self.mixer_channels.warning_dismissed = false;
        }
        self.mixer_channels.warning_active = warning;
    }
    fn preview_mixer_output(
        &mut self,
        channel: &str,
        value: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preview = self
            .mixer_channels
            .output_preview
            .get_or_insert_with(|| self.draft["device"]["outputMixerReducer"].clone());
        preview[channel] = self.draft["device"]["outputMixerReducer"][channel].clone();
        preview[channel]["value"] = json!(value);
        let path = format!("/device/outputMixerReducer/{channel}/value");
        let epoch = self.mixer_io.epoch;
        self.mixer_channels.output_timer = Some(cx.spawn_in(window, async move |owner, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            let _ = owner.update_in(cx, |this, window, cx| {
                if this.mixer_io.epoch != epoch {
                    return;
                }
                this.mixer_channels.output_preview = None;
                this.edit(&path, json!(value), window, cx);
                this.sync(window, cx);
            });
        }));
        self.sync(window, cx);
        cx.notify();
    }
    fn mixer_percentage_toggle(&self, reducer: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let percent = self.draft["device"][reducer]["showPercentage"] == true;
        h_flex()
            .p(surface::css(4.))
            .border_1()
            .border_color(C::border())
            .rounded(surface::css(16.))
            .hover(|style| style.border_color(C::selected()))
            .children(
                [("%", true), ("dB", false)]
                    .into_iter()
                    .map(|(label, selected)| {
                        BaseButton::new(SharedString::from(format!("{reducer}-unit-{label}")))
                            .w(surface::css(30.))
                            .h(surface::css(22.))
                            .rounded(surface::css(11.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(surface::css(14.))
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
                                    &format!("/device/{reducer}/showPercentage"),
                                    json!(!percent),
                                    window,
                                    cx,
                                )
                            }))
                    }),
            )
            .into_any_element()
    }
    pub(super) fn mixer_channels_page(&self, cx: &mut Context<Self>) -> AnyElement {
        surface::page_columns()
            .child(surface::page_column(
                v_flex()
                    .gap_0()
                    .child(self.mixer_output_panel(cx))
                    .child(self.mixer_bus_panel(BUSES[0], cx)),
            ))
            .child(surface::page_column(v_flex().gap_0().children(
                BUSES[1..].iter().map(|bus| self.mixer_bus_panel(*bus, cx)),
            )))
            .into_any_element()
    }
    fn mixer_output_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let output = self
            .mixer_channels
            .output_preview
            .as_ref()
            .unwrap_or(&self.draft["device"]["outputMixerReducer"]);
        let percent = self.draft["device"]["outputMixerReducer"]["showPercentage"] == true;
        let knobs = CHANNELS
            .iter()
            .map(|(source_channel, label)| {
                let mic = *source_channel == "mic";
                let channel = if mic && self.draft["device"]["micSetup"]["useMode"] == 0 {
                    "xlr"
                } else {
                    source_channel
                };
                let label = if mic {
                    if channel == "xlr" { "XLR_IN" } else { "MIC_IN" }
                } else {
                    label
                };
                let value = output[channel]["value"].as_f64().unwrap_or(0.) as f32;
                let (_, gain, _) = self.mixer_channel_range("outputMixerReducer", channel);
                let path = format!("/device/outputMixerReducer/{channel}/value");
                let slider = &self.mixer_channels.sliders[&path];
                let shown = if percent {
                    slider.read(cx).value().start()
                } else {
                    value
                };
                let mut knob =
                    MixerKnob::new(slider, t(label), if percent { "%" } else { "db" }, true)
                        .spatial_disabled(channel == "mic" && gain == 0.)
                        .display(shown.to_string());
                if mic {
                    if percent {
                        knob = knob.secondary(format!("{value} DB"));
                    }
                    knob =
                        knob.footer(
                            h_flex()
                                .mt(surface::css(10.))
                                .justify_center()
                                .gap(surface::css(5.))
                                .child(
                                    BaseButton::new(SharedString::from(format!(
                                        "mixer-output-{channel}-mic-page"
                                    )))
                                    .text_size(surface::css(14.))
                                    .text_color(C::text())
                                    .child(t(label).to_uppercase())
                                    .on_click(cx.listener(|_, _, _, cx| {
                                        if let Some(page) = razer_catalog::registered(1342)
                                            .and_then(|product| product.primary_navigation())
                                            .and_then(|nav| {
                                                nav.pages()
                                                    .iter()
                                                    .find(|page| page.kind().key() == "TAB_MIC")
                                            })
                                        {
                                            cx.emit(AudioNavigation::Page(page.id()));
                                        }
                                    })),
                                )
                                .child(surface::help_control(
                                    SharedString::from(format!("mixer-output-{channel}-help")),
                                    format!(
                                        "{}\n{}: {shown}{}\n{}: {}{} ({} {}{})",
                                        t("OUTPUT_MIXER_MIC_SETUP_TOOLTIP"),
                                        t("OUTPUT_VOLUME"),
                                        if percent { "%" } else { "db" },
                                        t("MIC_GAIN"),
                                        if percent { 100. } else { gain },
                                        if percent { "%" } else { "db" },
                                        t("MAX"),
                                        if percent { 100. } else { gain },
                                        if percent { "%" } else { "db" }
                                    ),
                                ))
                                .into_any_element(),
                        );
                }
                div()
                    .flex_basis(relative(1. / 3.))
                    .flex()
                    .justify_center()
                    .mt(surface::css(25.))
                    .child(knob)
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        mixer_card()
            .child(
                div()
                    .absolute()
                    .top(surface::css(10.))
                    .right(surface::css(10.))
                    .child(surface::help_control(
                        "mixer-output-help",
                        t("OUTPUT_MIXER_TOOLTIP").to_string(),
                    )),
            )
            .child(
                h_flex()
                    .gap(surface::css(20.))
                    .child(source_title(t("OUTPUT_MIXER")))
                    .child(self.mixer_percentage_toggle("outputMixerReducer", cx)),
            )
            .child(h_flex().flex_wrap().justify_between().children(knobs))
            .when(
                self.mixer_channels.warning_active && !self.mixer_channels.warning_dismissed,
                |panel| {
                    panel.child(
                        h_flex()
                            .relative()
                            .mt(surface::css(15.))
                            .p(surface::css(16.))
                            .gap(surface::css(8.))
                            .border_1()
                            .border_color(C::border())
                            .rounded(surface::css(5.))
                            .child(
                                razer_assets::resource_svg()
                                    .path("synapse/audio-mixer-output-info.svg")
                                    .size(surface::css(20.))
                                    .text_color(C::text()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(surface::css(14.))
                                    .child(t("OUTPUT_MIXER_TOOLTIP_INFO")),
                            )
                            .child(
                                BaseButton::new("mixer-output-alert-close")
                                    .absolute()
                                    .right(surface::css(5.))
                                    .top(surface::css(5.))
                                    .accessibility_label("Close alert")
                                    .child(
                                        razer_assets::resource_svg()
                                            .path("synapse/audio-mixer-mic-preview-close.svg")
                                            .size(surface::css(18.)),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mixer_channels.warning_dismissed = true;
                                        cx.notify();
                                    })),
                            ),
                    )
                },
            )
            .into_any_element()
    }
    fn mixer_bus_panel(
        &self,
        (reducer, title, icon, meter): (&'static str, &'static str, &'static str, usize),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let volume = &self.draft["device"][reducer]["volume"];
        let enabled = volume["isEnabled"] == true;
        let slider = &self.mixer_channels.sliders[&format!("/device/{reducer}/volume/value")];
        let help = match reducer {
            "playbackMixReducer" => t("PLAYBACK_MIX_TOOLTIP").to_string(),
            "streamMixReducer" => t("STREAM_MIX_TOOLTIP").to_string(),
            "lineOutReducer" => {
                format!("{}\n{}", t("LINE_OUT_TOOLTIP"), t("LINE_OUT_TOOLTIP_NOTE"))
            }
            _ => t("VOICE_CHAT_TOOLTIP").to_string(),
        };
        let mut panel = mixer_card()
            .child(
                div()
                    .absolute()
                    .top(surface::css(10.))
                    .right(surface::css(10.))
                    .child(surface::help_control(
                        SharedString::from(format!("{reducer}-help")),
                        help,
                    )),
            )
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .child(
                                img(SharedString::from(format!(
                                    "synapse/audio-mixer-{icon}.svg"
                                )))
                                .size(surface::css(24.)),
                            )
                            .child(source_title(t(title)))
                            .child(self.mixer_percentage_toggle(reducer, cx)),
                    )
                    .child(
                        BaseButton::new(SharedString::from(format!("{reducer}-volume-mute")))
                            .border_1()
                            .border_color(C::text())
                            .rounded(surface::css(3.))
                            .py(surface::css(5.))
                            .px(surface::css(20.))
                            .accessibility_label(t(title))
                            .child(
                                img(if enabled {
                                    "synapse/audio-mixer-volume-high.svg"
                                } else {
                                    "synapse/audio-mixer-volume-mute.svg"
                                })
                                .size(surface::css(20.)),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit(
                                    &format!("/device/{reducer}/volume/isEnabled"),
                                    json!(!enabled),
                                    window,
                                    cx,
                                )
                            })),
                    ),
            )
            .child(meter_bars(if enabled {
                self.mixer_meter.values[meter]
            } else {
                [0.; 2]
            }))
            .child(
                h_flex()
                    .gap(surface::css(20.))
                    .child(self.mixer_channels.numbers[reducer].clone())
                    .child(
                        div().flex_1().child(
                            SourceSlider::new(slider, {
                                let state = slider.read(cx);
                                (state.value().start() - state.min_value())
                                    / (state.max_value() - state.min_value())
                            })
                            .tip(None::<String>)
                            .container_height(55.),
                        ),
                    ),
            )
            .child(
                div()
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .text_size(surface::css(14.))
                    .child(t("ROUTING").to_uppercase()),
            );
        let mut table = h_flex().w_full().bg(C::knob());
        for (channel, label) in CHANNELS {
            let can_change = reducer != "voiceChatReducer" || channel != "chat";
            let row = &self.draft["device"][reducer][channel];
            let checked = row["isEnabled"] == true;
            let muted = row["isMuted"] == true;
            let mut column = v_flex().flex_1().min_w_0().items_center().child(
                div()
                    .p(surface::css(5.))
                    .text_size(surface::css(12.))
                    .child(t(label).to_uppercase()),
            );
            if can_change {
                column = column
                    .child(
                        div().py(surface::css(10.)).child(
                            Checkbox::new(SharedString::from(format!(
                                "{reducer}-{channel}-enabled"
                            )))
                            .checked(checked)
                            .on_click(cx.listener(
                                move |this, value: &bool, window, cx| {
                                    this.edit(
                                        &format!("/device/{reducer}/{channel}/isEnabled"),
                                        json!(*value),
                                        window,
                                        cx,
                                    )
                                },
                            )),
                        ),
                    )
                    .child(
                        BaseButton::new(SharedString::from(format!("{reducer}-{channel}-mute")))
                            .py(surface::css(10.))
                            .w_full()
                            .accessibility_label(t(label))
                            .child(
                                razer_assets::resource_svg()
                                    .path(if muted {
                                        "synapse/audio-mixer-route-mute.svg"
                                    } else {
                                        "synapse/audio-mixer-route-high.svg"
                                    })
                                    .size(surface::css(20.))
                                    .text_color(if muted { C::danger() } else { C::text() }),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit(
                                    &format!("/device/{reducer}/{channel}/isMuted"),
                                    json!(!muted),
                                    window,
                                    cx,
                                )
                            })),
                    );
            }
            table = table.child(column);
        }
        panel = panel.child(table);
        panel.into_any_element()
    }
}

fn source_title(title: impl Into<SharedString>) -> Div {
    let title = title.into();
    div()
        .font_family("RazerF5")
        .text_size(surface::css(16.))
        .text_color(C::selected())
        .child(title.to_uppercase())
}
fn mixer_card() -> Div {
    v_flex()
        .relative()
        .w_full()
        .flex_shrink_0()
        .my(surface::css(10.))
        .py(surface::css(30.))
        .px(surface::css(40.))
        .bg(C::surface())
        .rounded(surface::css(5.))
        .text_size(surface::css(14.))
        .text_color(C::text())
}
fn meter_bars(values: [f32; 2]) -> AnyElement {
    v_flex()
        .my(surface::css(10.))
        .children([values[1], values[0]].map(|level| {
            h_flex()
                .relative()
                .w_full()
                .h(surface::css(4.))
                .mb(surface::css(3.))
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
                        .w(relative((100. - level.clamp(0., 100.)) / 100.))
                        .bg(C::surface())
                        .opacity(0.8),
                )
        }))
        .into_any_element()
}

//! PID1342 LM/useRM/useUM Mic EQ, including its basic and advanced branches.
//! Current main.2623357c.js and main.07a2c4ed.css, not generic audio EQ layout.
use super::audio_mixer_theme::Colors as C;
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderThumb, SliderTrack};
use razer_widgets::keyboard_navigation::Button as BaseButton;

#[derive(IntoElement)]
struct Band {
    slider: Entity<SliderState>,
    label: SharedString,
    basic: bool,
}
impl RenderOnce for Band {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let value = self.slider.read(cx).value().start();
        let progress = (value + 12.) / 24.;
        let height = if self.basic { 140. } else { 300. };
        let group: SharedString = format!("mixer-mic-eq-band-{}", self.slider.entity_id()).into();
        let slider = BaseSlider::new(&self.slider)
            .vertical()
            .w(surface::css(16.))
            .h(surface::css(height))
            .relative()
            .child(
                div()
                    .absolute()
                    .left(surface::css(5.))
                    .top(surface::css(8.))
                    .bottom(surface::css(8.))
                    .w(surface::css(6.))
                    .rounded(surface::css(3.))
                    .bg(rgb(0x204d19))
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .w_full()
                            .h(relative(progress))
                            .rounded(surface::css(3.))
                            .bg(C::selected()),
                    ),
            )
            .child(
                SliderTrack::new(&self.slider)
                    .axis(Axis::Vertical)
                    .size_full()
                    .child(
                        SliderIndicator::new(&self.slider)
                            .absolute()
                            .top(surface::css(8.))
                            .bottom(surface::css(8.))
                            .w_full()
                            .child(
                                SliderThumb::new(&self.slider)
                                    .axis(Axis::Vertical)
                                    .absolute()
                                    .top(relative(1. - progress))
                                    .mt(surface::css(-8.))
                                    .size(surface::css(16.))
                                    .rounded_full()
                                    .bg(C::selected())
                                    .hover(|style| style.bg(rgb(0x707070)))
                                    .active(|style| style.bg(rgb(0x383838))),
                            ),
                    ),
            );
        let mut band = v_flex()
            .items_center()
            .relative()
            .id(("mixer-mic-eq-band", self.slider.entity_id()))
            .group(group.clone())
            .px(surface::css(15.));
        if self.basic {
            band = band
                .child(
                    div()
                        .text_size(surface::css(14.))
                        .text_color(C::text())
                        .child(self.label.to_uppercase()),
                )
                .child(
                    div()
                        .mt(surface::css(10.))
                        .mb(surface::css(50.))
                        .child(slider),
                )
                .child(
                    div()
                        .w(surface::css(60.))
                        .h(surface::css(27.))
                        .border_1()
                        .border_color(C::border())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(format!("{value:.0}")),
                )
                .child(
                    div()
                        .w(surface::css(60.))
                        .h(surface::css(27.))
                        .text_center()
                        .child("dB"),
                );
        } else {
            band = band
                .child(slider)
                .child(
                    div()
                        .absolute()
                        .top(surface::css((1. - progress) * 284. - 2.))
                        .left(surface::css(-13.))
                        .invisible()
                        .group_hover(group, |style| style.visible())
                        .w(surface::css(26.))
                        .h(surface::css(20.))
                        .rounded(surface::css(3.))
                        .bg(C::selected())
                        .text_color(rgb(0x000000))
                        .text_size(surface::css(12.))
                        .text_center()
                        .child(format!("{value:.0}")),
                )
                .child(
                    div()
                        .mt(surface::css(10.))
                        .text_size(surface::css(12.))
                        .text_color(C::text())
                        .child(self.label),
                );
        }
        band
    }
}

impl AudioProductWorkspace {
    pub(super) fn mixer_mic_equalizer(&self, cx: &mut Context<Self>) -> AnyElement {
        let basic = self.draft["equalizers"]["mic"]["useMode"] == 1;
        let key = if basic { "mic_basic" } else { "mic" };
        let eq = self
            .spec
            .equalizers
            .iter()
            .find(|eq| eq.key == key)
            .expect("current Mic EQ recipe");
        let state = &self.draft["equalizers"][key];
        let tabs = h_flex()
            .flex_wrap()
            .gap(surface::css(12.))
            .mb(surface::css(20.))
            .children(eq.presets.iter().map(|preset| {
                let name = preset.key.clone();
                let selected = state["selected"].as_str() == Some(&name);
                BaseButton::new(SharedString::from(format!("mixer-eq-tab-{key}-{name}")))
                    .accessibility_label(t(&preset.label))
                    .selected(selected)
                    .h(surface::css(27.))
                    .min_w(surface::css(90.))
                    .px(surface::css(16.))
                    .pt(surface::css(6.))
                    .pb(surface::css(7.))
                    .rounded(surface::css(3.))
                    .border_1()
                    .border_color(if selected { C::selected() } else { C::border() })
                    .bg(if selected { C::pressed() } else { C::surface() })
                    .text_color(C::text())
                    .text_size(surface::css(12.))
                    .line_height(surface::css(14.))
                    .hover(|style| style.bg(C::hover()).border_color(C::selected()))
                    .active(|style| style.bg(C::pressed()).border_color(C::selected()))
                    .child(t(&preset.label).to_uppercase())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_preset(key, &name, window, cx)
                    }))
            }));
        let mut chart = h_flex().items_start().mb(surface::css(20.));
        for (index, frequency) in eq.frequencies.iter().enumerate() {
            let path = format!("/equalizers/{key}/bands/{index}");
            let slider = self.sliders.get(&path).expect("current Mic EQ slider");
            let label: SharedString = if basic {
                t(frequency).into()
            } else {
                format!("{}Hz", frequency.replace('k', "K")).into()
            };
            chart = chart.child(Band {
                slider: slider.clone(),
                label,
                basic,
            });
        }
        chart = chart.child(
            v_flex()
                .flex_1()
                .min_w(surface::css(45.))
                .mr(surface::css(15.))
                .h(surface::css(300.))
                .items_end()
                .justify_between()
                .when(!basic, |axis| {
                    axis.children(["+12dB", "+6dB", "0dB", "-6dB", "-12dB"].map(|value| {
                        div()
                            .text_size(surface::css(12.))
                            .line_height(surface::css(12.))
                            .text_color(C::text())
                            .child(value)
                    }))
                }),
        );
        if !basic {
            chart = chart.child(
                BaseButton::new("mixer-mic-eq-reset")
                    .group("mixer-mic-eq-reset")
                    .accessibility_label(t("RESET"))
                    .w(surface::css(20.))
                    .h(surface::css(300.))
                    .opacity(0.85)
                    .child(
                        div()
                            .relative()
                            .size(surface::css(20.))
                            .child(img("synapse/audio-mixer-eq-reset.svg").size_full())
                            .child(
                                img("synapse/audio-mixer-eq-reset-hover.svg")
                                    .absolute()
                                    .inset_0()
                                    .size_full()
                                    .invisible()
                                    .group_hover("mixer-mic-eq-reset", |style| style.visible()),
                            )
                            .child(
                                img("synapse/audio-mixer-eq-reset-active.svg")
                                    .absolute()
                                    .inset_0()
                                    .size_full()
                                    .invisible()
                                    .group_active("mixer-mic-eq-reset", |style| style.visible()),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.reset_equalizer(key, window, cx)
                    })),
            );
        }
        surface::panel_with_help(
            t("MIC_EQUALIZER"),
            surface::help_control("mixer-mic-eq-help", t("MIC_EQUALIZER_TOOLTIP").to_string()),
            cx,
        )
        .child(tabs)
        .child(chart)
        .child(
            BaseButton::new("mixer-mic-eq-mode")
                .mt(surface::css(25.))
                .text_color(C::text())
                .text_size(surface::css(14.))
                .underline()
                .child(t(if basic { "SHOW_MORE" } else { "SHOW_LESS" }))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.edit(
                        "/equalizers/mic/useMode",
                        json!(if basic { 0 } else { 1 }),
                        window,
                        cx,
                    )
                })),
        )
        .into_any_element()
    }
}

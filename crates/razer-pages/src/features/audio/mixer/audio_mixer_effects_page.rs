//! Current 1342 kU column/card mount and YU/bU/VU controls.
use super::audio_mixer_knob::MixerKnob;
use super::audio_mixer_theme::Colors as C;
use super::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use razer_widgets::keyboard_navigation::Button as BaseButton;
use razer_widgets::source_slider::SourceSlider;

const ROOM_PATH: &str = "/device/echoReverb/modeValues/0";

impl AudioProductWorkspace {
    pub(super) fn initialize_mixer_effect_inputs(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 1342 {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).default_value("0"));
        self.subscriptions.push(cx.subscribe_in(
            &input,
            window,
            |this, input, event, window, cx| {
                if this.syncing || !matches!(event, InputEvent::Change) {
                    return;
                }
                let text = input.read(cx).value().to_string();
                if let Some(value) = room_size_value(&text) {
                    this.edit(ROOM_PATH, json!(value), window, cx);
                }
                // The original input is controlled; an out-of-range edit does not
                // replace I.currentValues[0], and an empty number input becomes 0.
                this.syncing = true;
                this.sync_mixer_effect_inputs(window, cx);
                this.syncing = false;
            },
        ));
        self.mixer_room_input = Some(input);
    }
    pub(super) fn sync_mixer_effect_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.mixer_room_input {
            let text = self
                .draft
                .pointer(ROOM_PATH)
                .and_then(Value::as_f64)
                .unwrap_or(0.)
                .to_string();
            if input.read(cx).value().to_string() != text {
                input.update(cx, |input, cx| input.set_value(text, window, cx));
            }
        }
    }
    pub(super) fn mixer_effects_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = self
            .spec
            .pages
            .iter()
            .find(|page| page.key == "EFFECTS")
            .expect("current Effects descriptor");
        let card = |title: &str, window: &mut Window, cx: &mut Context<Self>| {
            let section = page
                .sections
                .iter()
                .find(|section| section.title == title)
                .expect("current Effects card");
            self.mixer_effect_card(section, window, cx)
        };
        let heading = |key: &str| {
            div()
                .w_full()
                .mt(surface::css(20.))
                .text_size(surface::css(14.))
                .child(t(key).to_uppercase())
        };
        v_flex()
            .gap_0()
            .child(self.render_effect_presets(cx))
            .child(
                surface::page_columns()
                    .child(surface::page_column(
                        v_flex()
                            .gap_0()
                            .child(heading("MICROPHONE_EFFECTS"))
                            .child(card("VOICE_CHANGER", window, cx))
                            .child(card("ECHO_REVERB", window, cx)),
                    ))
                    .child(surface::page_column(
                        v_flex()
                            .gap_0()
                            .child(heading("LINE_IN_EFFECTS"))
                            .child(card("KEY_SHIFTER", window, cx))
                            .child(card("VOCAL_FADING", window, cx)),
                    )),
            )
            .into_any_element()
    }
    fn mixer_effect_card(
        &self,
        section: &AudioSection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let toggle = section
            .controls
            .iter()
            .find(|control| control.kind == "toggle")
            .expect("Effects title switch");
        let active = self
            .draft
            .pointer(&toggle.path)
            .and_then(Value::as_bool)
            .unwrap_or(section.title == "VOICE_CHANGER");
        let path = toggle.path.clone();
        let mut panel = surface::panel_with_title_switch(
            t(&section.title),
            surface::SynapseSwitch::new(SharedString::from(format!("audio-effect-switch-{path}")))
                .accessibility_label(t(&section.title))
                .checked(active)
                .on_change(cx.listener(move |this, next: &bool, window, cx| {
                    let next = if path == "/device/echoReverb/isEnabled" {
                        !this
                            .mixer_echo_committed
                            .as_ref()
                            .unwrap_or(&this.draft["device"]["echoReverb"])["isEnabled"]
                            .as_bool()
                            .unwrap_or(false)
                    } else {
                        *next
                    };
                    this.edit(&path, json!(next), window, cx)
                })),
            surface::help_control(
                SharedString::from(format!("audio-effects-help-{}", section.title)),
                match section.title.as_str() {
                    "VOICE_CHANGER" => t("VOICE_CHANGER_TOOLTIP").to_string(),
                    "KEY_SHIFTER" => t("KEY_SHIFTER_TOOLTIP").to_string(),
                    "VOCAL_FADING" => t("VOCAL_FADING_TOOLTIP").to_string(),
                    _ => format!(
                        "{}\n\n{}\n{}\n\n{}\n{}\n\n{}\n{}",
                        t("ECHO_REVERB_TOOLTIP"),
                        t("DECAY_TIME"),
                        t("ECHO_REVERB_DECAY_TIME_TOOLTIP"),
                        t("GAIN"),
                        t("ECHO_REVERB_GAIN_TOOLTIP"),
                        t("DELAY"),
                        t("ECHO_REVERB_DELAY_TOOLTIP")
                    ),
                },
            ),
            cx,
        );
        let mut body = v_flex().gap_0();
        for control in section
            .controls
            .iter()
            .filter(|control| control.kind != "toggle")
        {
            if control.kind == "presets" {
                let path = control.path.clone();
                if section.title == "ECHO_REVERB" {
                    body = body.child(
                        div()
                            .mb(surface::css(10.))
                            .child(t(&control.label).to_uppercase()),
                    );
                }
                body = body.child(
                    h_flex()
                        .gap(surface::css(12.))
                        .flex_wrap()
                        .mb(surface::css(20.))
                        .children(control.options.iter().map(|option| {
                            let path = path.clone();
                            let value = option.value.clone();
                            let selected = self.draft.pointer(&path) == Some(&value);
                            BaseButton::new(SharedString::from(format!(
                                "audio-effect-tab-{path}-{value}"
                            )))
                            .accessibility_label(t(&option.label))
                            .selected(selected)
                            .disabled(!active)
                            .min_w(surface::css(90.))
                            .h(surface::css(27.))
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
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit(&path, value.clone(), window, cx)
                            }))
                            .child(t(&option.label))
                        })),
                );
            } else if section.title == "KEY_SHIFTER" || section.title == "VOCAL_FADING" {
                if let Some(state) = self.sliders.get(&control.path) {
                    let value = self
                        .draft
                        .pointer(&control.path)
                        .and_then(Value::as_f64)
                        .unwrap_or(f64::from(control.min));
                    body = body.child(
                        div()
                            .relative()
                            .w_full()
                            .h(surface::css(64.))
                            .child(
                                SourceSlider::new(
                                    state,
                                    ((value - f64::from(control.min))
                                        / f64::from(control.max - control.min))
                                        as f32,
                                )
                                .tip(Some(format!("{value:.0}")))
                                .enabled(active),
                            )
                            .child(
                                h_flex()
                                    .absolute()
                                    .bottom(surface::css(-2.))
                                    .w_full()
                                    .justify_between()
                                    .text_color(C::muted())
                                    .text_size(surface::css(14.))
                                    .child(format!("{}", control.min))
                                    .child(format!("{}", control.max)),
                            ),
                    );
                }
            } else if control.path == ROOM_PATH {
                let state = self
                    .sliders
                    .get(ROOM_PATH)
                    .expect("current room-size state");
                let value = self
                    .draft
                    .pointer(ROOM_PATH)
                    .and_then(Value::as_f64)
                    .unwrap_or(0.);
                body = body.child(
                    h_flex()
                        .gap_0()
                        .items_start()
                        .child(div().line_height(surface::css(27.)).child(t("ROOM_SIZE")))
                        .when_some(self.mixer_room_input.as_ref(), |row, input| {
                            row.child(razer_widgets::keyboard_navigation::focus_input(
                                Input::new(input)
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .disabled(!active)
                                    .w(surface::css(50.))
                                    .h(surface::css(27.))
                                    .mx(surface::css(10.))
                                    .mb(surface::css(20.))
                                    .px(surface::css(10.))
                                    .py(surface::css(5.))
                                    .border_1()
                                    .border_color(C::border())
                                    .rounded_none()
                                    .text_size(surface::css(14.))
                                    .text_color(C::value()),
                                input,
                                cx,
                            ))
                        }),
                );
                body = body.child(
                    SourceSlider::new(state, value as f32 / 100.)
                        .tip(None::<String>)
                        .enabled(true)
                        .interaction_enabled(active)
                        .range_labels("0", "100", C::muted()),
                );
            }
        }
        if section.title == "ECHO_REVERB" {
            // Current useWU mounts room-size as a number/range pair, followed
            // by exactly three CU knobs. All three share the 500ms O handler.
            body = body.child(
                h_flex().items_start().justify_between().children(
                    section
                        .controls
                        .iter()
                        .filter(|control| {
                            control.path.starts_with("/device/echoReverb/modeValues/")
                                && control.path != ROOM_PATH
                        })
                        .filter_map(|control| {
                            self.sliders.get(&control.path).map(|state| {
                                div().mt(surface::css(25.)).child(MixerKnob::new(
                                    state,
                                    t(&control.label),
                                    control.unit.clone(),
                                    active,
                                ))
                            })
                        }),
                ),
            );
        }
        if section.title == "VOICE_CHANGER" || section.title == "ECHO_REVERB" {
            body = body.when(!active, |body| body.opacity(0.3));
        }
        panel = panel.child(body);
        let _ = window;
        panel.into_any_element()
    }
}

fn room_size_value(text: &str) -> Option<u32> {
    let text = text.trim();
    // HTML number input exposes invalid intermediate text as an empty value.
    if text.is_empty() || text.parse::<f64>().is_err() {
        return Some(0);
    }
    let negative = text.starts_with('-');
    let digits = text
        .trim_start_matches(['+', '-'])
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    let value = digits.parse::<u32>().ok()?;
    if value > 100 || (negative && value != 0) {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::room_size_value;
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn room_size_follows_html_number_and_source_parse_int() {
        assert_eq!(room_size_value(""), Some(0));
        assert_eq!(room_size_value("-"), Some(0));
        assert_eq!(room_size_value("42.9"), Some(42));
        assert_eq!(room_size_value("1e2"), Some(1));
        assert_eq!(room_size_value("100"), Some(100));
        assert_eq!(room_size_value("101"), None);
        assert_eq!(room_size_value("-1"), None);
    }
}

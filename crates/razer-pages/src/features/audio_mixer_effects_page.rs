//! Current 1342 kU column/card mount and YU/bU/VU controls.
use super::*;
use gpui_kit::base::Button as BaseButton;
use razer_widgets::source_slider::SourceSlider;
impl AudioProductWorkspace {
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
            .unwrap_or(false);
        let path = toggle.path.clone();
        let mut panel = surface::panel_with_title_switch(
            t(&section.title),
            surface::SynapseSwitch::new(SharedString::from(format!("audio-effect-switch-{path}")))
                .accessibility_label(t(&section.title))
                .checked(active)
                .on_change(cx.listener(move |this, next: &bool, window, cx| {
                    this.edit(&path, json!(*next), window, cx)
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
                            .border_color(rgb(if selected { 0x44d62c } else { 0x5d5d5d }))
                            .bg(if selected {
                                rgba(0xffffff1a)
                            } else {
                                rgba(0x111111ff)
                            })
                            .text_color(rgb(0xcccccc))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .hover(|style| style.bg(rgba(0x0000001a)).border_color(rgb(0x44d62c)))
                            .active(|style| style.bg(rgba(0xffffff1a)).border_color(rgb(0x44d62c)))
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
                                    .text_color(rgb(0x999999))
                                    .text_size(surface::css(14.))
                                    .child(format!("{}", control.min))
                                    .child(format!("{}", control.max)),
                            ),
                    );
                }
            } else {
                // Echo CU rotary widgets still require their separate source
                // canvas/drag implementation; keep their real value editors.
                body = body.child(self.render_control(control, cx));
            }
        }
        if section.title == "VOICE_CHANGER" || section.title == "ECHO_REVERB" {
            body = body.when(!active, |body| body.opacity(0.3));
        }
        panel = panel.child(body);
        let _ = window;
        panel.into_any_element()
    }
}

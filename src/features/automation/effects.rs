//! Mounted 3946 quick-effect branches: LP → NP/uP, Jl/Ql, dI/AI.
use super::*;
use gpui_kit::base::{Button as BaseButton, Slider, SliderIndicator, SliderThumb, SliderTrack};

impl AutomationEditor {
    pub(super) fn selected_effect(&self, down: bool) -> u64 {
        self.draft
            .lane(down)
            .data
            .first()
            .and_then(|value| value["selectedEffectId"].as_u64())
            .unwrap_or(4)
    }

    pub(super) fn render_effect_parameters(
        &self,
        down: bool,
        value: &Value,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        match value["selectedEffectId"].as_u64().unwrap_or(4) {
            1 | 2 => self.render_color_parameters(down, value, disabled, cx),
            7 => v_flex()
                .child(self.render_color_parameters(down, value, disabled, cx))
                .child(self.duration(down, disabled, cx))
                .into_any_element(),
            4 => self.wave_direction(down, value, disabled, cx),
            12 => v_flex()
                .mt(surface::css(20.))
                .items_start()
                .child(text("TEXT_COLOR_BOOST"))
                .child(
                    div()
                        .mb(surface::css(10.))
                        .child(self.boosts[down as usize].clone()),
                )
                .into_any_element(),
            // Spectrum and Fire mount no parameter area in LP.
            _ => div().into_any_element(),
        }
    }

    fn wave_direction(
        &self,
        down: bool,
        value: &Value,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let current = value["setting"]["direction"].as_u64().unwrap_or(4);
        v_flex()
            .mt(surface::css(20.))
            .items_start()
            .child(text("TEXT_DIRECTION"))
            .child(
                h_flex()
                    .id(SharedString::from(format!("automation-wave-toggle-{down}")))
                    .mt(surface::css(5.))
                    .p(surface::css(5.))
                    .gap(surface::css(5.))
                    .h(surface::css(42.))
                    .bg(Colors::panel())
                    .border_1()
                    .border_color(Colors::input_border())
                    .rounded(surface::css(20.))
                    .when(!disabled, |view| {
                        view.hover(|style| style.border_color(Colors::primary()))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_wave(down, cx)))
                    .children([(3, "up"), (4, "down")].map(|(direction, name)| {
                        let selected = current == direction;
                        BaseButton::new(SharedString::from(format!(
                            "automation-wave-{down}-{name}"
                        )))
                        .accessibility_label(crate::i18n::t(if direction == 3 {
                            "UP"
                        } else {
                            "DOWN"
                        }))
                        .selected(selected)
                        .disabled(disabled)
                        .w(surface::css(40.))
                        .h(surface::css(30.))
                        .rounded(surface::css(15.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if selected {
                            Colors::primary()
                        } else {
                            Colors::panel()
                        })
                        .active(move |style| {
                            style.bg(if selected {
                                Colors::primary()
                            } else {
                                Colors::close_hover()
                            })
                        })
                        .child(
                            img(SharedString::from(format!(
                                "synapse/automation-icon_direction_{name}{}.svg",
                                if selected { "" } else { "_999" },
                            )))
                            .size(surface::css(20.)),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            // GT/yT toggles from either half and stops bubbling.
                            cx.stop_propagation();
                            this.toggle_wave(down, cx);
                        }))
                    })),
            )
            .into_any_element()
    }

    fn toggle_wave(&mut self, down: bool, cx: &mut Context<Self>) {
        let current = self
            .draft
            .lane(down)
            .data
            .first()
            .and_then(|v| v["setting"]["direction"].as_u64());
        self.update_chroma_setting(
            down,
            "direction",
            json!(if current == Some(3) { 4 } else { 3 }),
            cx,
        );
    }

    fn duration(&self, down: bool, disabled: bool, cx: &Context<Self>) -> AnyElement {
        let state = &self.durations[down as usize];
        let position = state.read(cx).percentage().end;
        let focus = self.duration_focus[down as usize].clone();
        let click_focus = focus.clone();
        v_flex()
            .mt(surface::css(20.))
            .child(text("TEXT_DURATION"))
            .child(
                div()
                    .id(SharedString::from(format!("automation-duration-{down}")))
                    .track_focus(&focus)
                    .relative()
                    .w_full()
                    .h(surface::css(36.))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        if !disabled {
                            click_focus.focus(window, cx);
                        }
                    })
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if !this.chroma_parameters_editable(down) {
                            return;
                        }
                        let state = &this.durations[down as usize];
                        let value = state.read(cx).value().start();
                        let next = match event.keystroke.key.as_str() {
                            "left" | "down" => value - 1.,
                            "right" | "up" => value + 1.,
                            "home" => 1.,
                            "end" => 3.,
                            _ => return,
                        }
                        .clamp(1., 3.);
                        state.update(cx, |slider, cx| slider.set_value(next, window, cx));
                        this.update_chroma_setting(down, "duration", json!(next as u32), cx);
                        cx.stop_propagation();
                    }))
                    .child(
                        Slider::new(state)
                            .disabled(disabled)
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h(surface::css(16.))
                            .child(
                                SliderTrack::new(state)
                                    .disabled(disabled)
                                    .relative()
                                    .w_full()
                                    .h_full()
                                    .child(
                                        div()
                                            .absolute()
                                            .top(surface::css(5.))
                                            .w_full()
                                            .h(surface::css(6.))
                                            .rounded(surface::css(3.))
                                            .bg(Colors::slider_track()),
                                    )
                                    .child(
                                        SliderIndicator::new(state)
                                            .absolute()
                                            .top(surface::css(5.))
                                            .left_0()
                                            .w(relative(position))
                                            .h(surface::css(6.))
                                            .rounded(surface::css(3.))
                                            .bg(Colors::primary()),
                                    )
                                    // Native range thumb center travels width minus 16px.
                                    .child(
                                        div()
                                            .absolute()
                                            .left(surface::css(8.))
                                            .right(surface::css(8.))
                                            .h_full()
                                            .child(
                                                SliderThumb::new(state)
                                                    .disabled(disabled)
                                                    .absolute()
                                                    .left(relative(position))
                                                    .ml(surface::css(-8.))
                                                    .size(surface::css(16.))
                                                    .rounded_full()
                                                    .bg(Colors::primary())
                                                    .border_2()
                                                    .border_color(Colors::primary())
                                                    .hover(|style| style.bg(Colors::input_border()))
                                                    .active(|style| {
                                                        style.bg(Colors::slider_pressed())
                                                    }),
                                            ),
                                    ),
                            ),
                    )
                    .children(
                        [
                            "TEXT_SLIDER_SHORT",
                            "TEXT_SLIDER_MEDIUM",
                            "TEXT_SLIDER_LONG",
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(index, key)| {
                            div()
                                .absolute()
                                .bottom(surface::css(-2.))
                                .when(index == 0, |v| v.left_0())
                                .when(index == 1, |v| v.left_0().w_full().text_center())
                                .when(index == 2, |v| v.right_0())
                                .child(text(key).to_uppercase())
                        }),
                    ),
            )
            .into_any_element()
    }
}

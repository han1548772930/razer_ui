//! Independently mounted 678/679/688 Analog Gamepad callers; 679 CU -> DU -> jm.
//! Current source caller receipts: tools/audit_keyboard_analog_callers.cjs. One pair with an
//! explicit Create command and observed READY/ACTIVE/SNAPPED key states.
//! This is separate from the ordinary Customize multi-pair widget.
use super::*;

impl State {
    pub(super) fn accept_analog(&mut self, input: &str) -> bool {
        if !self.config.enabled || self.phase == Phase::Ready {
            return false;
        }
        let Some(pair) = self.staged.first_mut() else {
            return false;
        };
        match self.phase {
            Phase::Key1 => pair.key1 = input.into(),
            Phase::Key2 => pair.key2 = input.into(),
            Phase::Ready => return false,
        }
        // jm deliberately retains the rejected preview. Only these two keys
        // and a duplicate pair are forbidden; do not reuse ordinary xl.
        if matches!(input, "KEY_APPLICATION" | "KEY_LEFT_GUI") || pair.key1 == pair.key2 {
            self.message = Message::AnalogWarning;
            return false;
        }
        if self.phase == Phase::Key1 {
            self.phase = Phase::Key2;
            self.message = Message::None;
            self.capture_generation = self.capture_generation.wrapping_add(1);
            false
        } else {
            self.commit(self.staged.clone());
            self.finish();
            self.message = Message::Success;
            true
        }
    }
}

impl KeyboardProductWorkspace {
    pub(super) fn submit_analog_snap(&mut self, cx: &mut Context<Self>) {
        let Some(state) = &self.snap_tap else {
            return;
        };
        let payload = json!({"type":"ON_SET_SNAP_TAP","payload":{
            "isEnabled":state.config.enabled,"keyList":state.config.pairs,"pressedKeys":state.observed_pressed}});
        // Source reducer Gno/K25 is a real write intent. The root transport
        // reports unsupported until its native ABI is implemented, never save
        // success. Analytics Fm.z6 is deliberately not used as write evidence.
        self.request_actuation_message(payload, cx);
    }

    fn create_analog_snap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.factory_default_profile {
            return;
        }
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        if state.adjustment == Some(true) {
            state.return_focus = window.focused(cx);
            state.prompt = true;
            state.prompt_focus.focus(window, cx);
        } else if state.config.enabled && state.phase == Phase::Ready {
            state.staged = state.config.pairs.clone();
            state.begin(1, Phase::Key1, window, cx);
            state.message = Message::None;
            state.pressed = [0; 2];
        }
        cx.notify();
    }

    fn toggle_analog_snap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        state.success_timer = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.apply_toggle_analog_snap(window, cx)
            });
        }));
    }

    fn apply_toggle_analog_snap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.factory_default_profile {
            return;
        }
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        if state.adjustment == Some(true) {
            state.return_focus = window.focused(cx);
            state.prompt = true;
            state.prompt_focus.focus(window, cx);
            cx.notify();
            return;
        }
        state.config.enabled = !state.config.enabled;
        if !state.config.enabled && state.phase != Phase::Ready {
            // jm's enable effect retains the untouched config key and the
            // staged opposite key when a recording is interrupted.
            let mut pair = state.config.pairs[0].clone();
            if let Some(preview) = state.staged.first() {
                if state.phase == Phase::Key1 {
                    pair.key2 = preview.key2.clone();
                } else {
                    pair.key1 = preview.key1.clone();
                }
            }
            state.commit(vec![pair]);
            state.finish();
            state.message = Message::Success;
        }
        state.local = true;
        self.publish_snap_tap(cx);
        self.submit_analog_snap(cx);
    }

    fn outside_analog_snap(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(state) = &mut self.snap_tap else {
            return;
        };
        if state.pair_bounds.get().contains(&position)
            || state.phase == Phase::Ready
            || state.message == Message::AnalogWarning
        {
            return;
        }
        state.commit(state.staged.clone());
        state.finish();
        self.publish_snap_tap(cx);
        self.submit_analog_snap(cx);
    }

    pub(crate) fn analog_snap_panel(&self, cx: &Context<Self>) -> Option<AnyElement> {
        if !self.spec.analog_gamepad_layout() || self.page != "ACTUATION" {
            return None;
        }
        let state = self.snap_tap.as_ref()?;
        let pair = state.staged.first()?;
        let enabled = state.config.enabled;
        let locked = self.factory_default_profile;
        let bounds = state.pair_bounds.clone();
        let rows = h_flex()
            .gap(surface::css(10.))
            .relative()
            .children(
                [&pair.key1, &pair.key2]
                    .into_iter()
                    .enumerate()
                    .map(|(index, key)| {
                        let recording =
                            state.phase == if index == 0 { Phase::Key1 } else { Phase::Key2 };
                        let warning = state.message == Message::AnalogWarning;
                        let pressed = state.pressed[index];
                        let color = if warning {
                            Colors::warning()
                        } else {
                            Colors::accent()
                        };
                        let text = div()
                            .mx(surface::css(10.))
                            .min_w(surface::css(38.))
                            .text_size(surface::css(11.))
                            .child(state.name(key, &[]));
                        let text = if recording && enabled {
                            text.h(surface::css(25.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .whitespace_nowrap()
                                .with_animation(
                                    SharedString::from(format!(
                                        "679-snap-blink-{index}-{}-{}",
                                        warning, state.capture_generation
                                    )),
                                    Animation::new(Duration::from_secs(1)).repeat(),
                                    move |text, delta| {
                                        let strength = (delta * 2. - 1.).abs();
                                        text.bg(color.opacity(strength)).text_color(
                                            Colors::text().blend(Colors::panel().opacity(strength)),
                                        )
                                    },
                                )
                                .into_any_element()
                        } else {
                            text.into_any_element()
                        };
                        div()
                            .min_w(surface::css(64.))
                            .h(surface::css(44.))
                            .border(surface::css(2.))
                            .rounded(surface::css(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .border_color(if recording || warning {
                                Hsla::from(color)
                            } else if pressed == 1 {
                                Colors::accent()
                            } else if pressed == 2 {
                                Hsla::from(rgb(0x888888))
                            } else {
                                Colors::border()
                            })
                            .text_color(if pressed > 0 && !recording {
                                Hsla::from(rgb(0))
                            } else {
                                Colors::text()
                            })
                            .when(pressed > 0 && !recording, |row| {
                                row.bg(if pressed == 1 {
                                    Colors::accent()
                                } else {
                                    Hsla::from(rgb(0x888888))
                                })
                            })
                            .child(text)
                    }),
            )
            .child(
                canvas(move |rect, _, _| bounds.set(rect), |_, _, _, _| {})
                    .absolute()
                    .inset_0(),
            );
        Some(
            surface::panel_with_title_switch(
                t("SNAP_TAP_HEADER"),
                surface::SynapseSwitch::new("679-snap-enable")
                    .checked(enabled)
                    .disabled(locked)
                    .on_change(
                        cx.listener(|this, _, window, cx| this.toggle_analog_snap(window, cx)),
                    ),
                BaseButton::new("679-snap-shortcut-tooltip")
                    .tooltip(|window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(t("SNAP_TAP_SHORTCUT_TOOLTIP"))
                            .build(window, cx)
                    })
                    .child(
                        h_flex()
                            .gap(surface::css(4.))
                            .child(
                                div()
                                    .px(surface::css(10.))
                                    .py(surface::css(5.))
                                    .border_1()
                                    .border_color(rgb(0x5d5d5d))
                                    .rounded(surface::css(3.))
                                    .child("FN"),
                            )
                            .child("+")
                            .child(
                                div()
                                    .px(surface::css(10.))
                                    .py(surface::css(5.))
                                    .border_1()
                                    .border_color(rgb(0x5d5d5d))
                                    .rounded(surface::css(3.))
                                    .child("L SHIFT"),
                            ),
                    ),
                cx,
            )
            .mb_0()
            .track_focus(&state.focus)
            .capture_key_up(cx.listener(|this, event, _, cx| this.snap_key_up(event, cx)))
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.outside_analog_snap(event.position, cx)
            }))
            .child(div().child(t("SNAP_TAP_DESC")))
            .child(
                v_flex()
                    .mt(surface::css(10.))
                    .gap(surface::css(10.))
                    .opacity(if enabled && !locked { 1. } else { 0.3 })
                    .child(
                        h_flex()
                            .gap(surface::css(7.))
                            .child(
                                BaseButton::new("679-snap-create")
                                    .disabled(!enabled || locked || state.phase != Phase::Ready)
                                    .px(surface::css(16.))
                                    .py(surface::css(6.))
                                    .rounded(surface::css(3.))
                                    .bg(if enabled && state.phase == Phase::Ready {
                                        Colors::accent()
                                    } else {
                                        Hsla::from(rgb(0x30961f))
                                    })
                                    .text_color(rgb(0))
                                    .text_size(surface::css(12.))
                                    .child(t("CREATE_SNAP_TAP_TEXT").to_uppercase())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.create_analog_snap(window, cx)
                                    })),
                            )
                            .child(surface::help_control(
                                "679-snap-create-help",
                                t("SNAP_TAP_TOOLTIP_MENU"),
                            )),
                    )
                    .child(rows),
            )
            .when(state.message != Message::None, |panel| {
                panel.child(
                    div()
                        .mt(surface::css(10.))
                        .text_size(surface::css(14.))
                        .text_color(if state.message == Message::AnalogWarning {
                            Colors::warning()
                        } else if state.message == Message::Success {
                            Colors::success()
                        } else {
                            Colors::muted()
                        })
                        .child(t(state.message.label())),
                )
            })
            .into_any_element(),
        )
    }
}

//! 2676 hm/Mm and 2684 lc/cc: separately mounted thumbstick popup.
use super::*;

impl GamepadProductWorkspace {
    pub(in super::super) fn has_popup_calibration(&self) -> bool {
        matches!(self.spec.product_id, 2676 | 2684)
    }

    pub(in super::super) fn open_calibration_popup(
        &mut self,
        part: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.has_popup_calibration()
            || !(1..=2).contains(&part)
            || self.calibration_popup.is_some()
        {
            return;
        }
        self.calibration_state
            .set_reducer_valid(self.trigger_calibration_state.valid);
        let return_focus = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.calibration_popup = Some(CalibrationDialog {
            focus,
            return_focus,
        });
        self.request_calibration(CalibrationAction::Start, part, window, cx);
    }

    pub(super) fn close_calibration_popup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.calibration_popup.is_none() {
            return;
        }
        self.request_calibration(
            CalibrationAction::Stop,
            self.calibration_state.part,
            window,
            cx,
        );
        if let Some(popup) = self.calibration_popup.take() {
            if let Some(focus) = popup.return_focus {
                focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    pub(in super::super) fn render_calibration_popup(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(popup) = &self.calibration_popup else {
            return div().into_any_element();
        };
        let state = &self.calibration_state;
        let pid = self.spec.product_id;
        let title = if state.step < 1 {
            "CALIBRATION_TITLE"
        } else if state.step == 6 {
            "CALIBRATION_SUCCESS"
        } else {
            ""
        };
        let description = match state.step {
            0 => "CALIBRATION_STEP0",
            1 => "CALIBRATION_STEP1",
            2 => "CALIBRATION_STEP2",
            3 => "CALIBRATION_STEP3",
            4 => "CALIBRATION_STEP4",
            5 => "CALIBRATION_V2_STEP5",
            _ => "",
        };
        let mut product = div()
            .relative()
            .w(surface::css(371.))
            .h(surface::css(250.))
            .child(
                img(calibration_svg(pid, state.step, state.part, state.valid))
                    .w_full()
                    .h_full()
                    .object_fit(ObjectFit::Contain),
            );
        if (1..=5).contains(&state.step) && (!state.valid || state.step == 5) {
            product = product.child(
                div()
                    .absolute()
                    .left(surface::css(if state.part == 1 { 8.25 } else { 165.5 }))
                    .top(surface::css(if state.part == 1 { 11. } else { 68. }))
                    .child(direction(state, 1.)),
            );
        }
        let close = |id, label, style, cx: &Context<Self>| {
            action(id, label, style).on_click(
                cx.listener(|this, _, window, cx| this.close_calibration_popup(window, cx)),
            )
        };
        let content = if state.step == -1 {
            v_flex()
                .items_center()
                .pt(surface::css(24.))
                .pb(surface::css(8.))
                .child(img("synapse/gamepad-2636-calibration-warning.svg").size(surface::css(20.)))
                .child(
                    div()
                        .mt(surface::css(10.))
                        .font_family("RazerF5")
                        .text_size(surface::css(16.))
                        .line_height(surface::css(16.))
                        .text_color(Colors::warning())
                        .child(t("FAILED_CALIBRATION_SHORT").to_uppercase()),
                )
                .child(
                    h_flex()
                        .mt(surface::css(10.))
                        .justify_center()
                        .gap(surface::css(10.))
                        .child(close(
                            "gamepad-popup-calibration-error-cancel",
                            "CANCEL",
                            ActionStyle::Secondary,
                            cx,
                        ))
                        .child(
                            action(
                                "gamepad-popup-calibration-error-retry",
                                "CALIBRATION_RECALIBRATE",
                                ActionStyle::Primary,
                            )
                            .on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.request_calibration(
                                        CalibrationAction::Start,
                                        this.calibration_state.part,
                                        window,
                                        cx,
                                    )
                                },
                            )),
                        ),
                )
        } else {
            v_flex()
                .items_center()
                .w(surface::css(601.))
                .min_h(surface::css(352.))
                .child(div().w(surface::css(601.)).h(surface::css(21.)).when(
                    state.step >= 1,
                    |d| {
                        d.child(
                            img(SharedString::from(format!(
                                "synapse/gamepad-2636-calibration-steps-{}.svg",
                                state.step
                            )))
                            .w_full()
                            .h_full(),
                        )
                    },
                ))
                .child(
                    v_flex()
                        .items_center()
                        .mt(surface::css(50.))
                        .w(surface::css(600.))
                        .min_h(surface::css(34.))
                        .when(!title.is_empty(), |d| {
                            d.child(
                                div()
                                    .min_h(surface::css(16.))
                                    .font_family("RazerF5")
                                    .text_size(surface::css(16.))
                                    .text_color(Colors::primary())
                                    .child(t(title).to_uppercase()),
                            )
                        })
                        .when(!description.is_empty(), |d| {
                            d.child(
                                div()
                                    .w_full()
                                    .min_h(surface::css(34.))
                                    .when(!title.is_empty(), |d| d.mt(surface::css(12.)))
                                    .line_height(surface::css(17.))
                                    .child(t(description)),
                            )
                        }),
                )
                .child(
                    h_flex()
                        .w(surface::css(601.))
                        .justify_center()
                        .mt(surface::css(20.))
                        .child(product),
                )
                .when(state.step > 0, |d| {
                    d.child(h_flex().justify_center().mt(surface::css(30.)).child(close(
                        "gamepad-popup-calibration-stop",
                        if state.step == 6 { "DONE" } else { "CANCEL" },
                        if state.step == 6 {
                            ActionStyle::Primary
                        } else {
                            ActionStyle::Secondary
                        },
                        cx,
                    )))
                })
        };
        let panel = v_flex()
            .occlude()
            .w(surface::css(850.))
            .h(window.viewport_size().height)
            .bg(Colors::detail())
            .text_color(Colors::text())
            .text_size(surface::css(14.))
            .child(
                h_flex()
                    .relative()
                    .h(surface::css(36.))
                    .px(surface::css(36.))
                    .justify_center()
                    .items_center()
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .text_color(razer_widgets::theme::FeedbackColors::secondary())
                    .border_b_1()
                    .border_color(razer_widgets::theme::FeedbackColors::border())
                    .child(t(if state.part == 1 {
                        "LEFT_THUMBSTICK_CALIBRATION"
                    } else {
                        "RIGHT_THUMBSTICK_CALIBRATION"
                    }))
                    .child(
                        BaseButton::new("gamepad-popup-calibration-close")
                            .absolute()
                            .right_0()
                            .top_0()
                            .size(surface::css(36.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .accessibility_label(t("CLOSE"))
                            .child(
                                img(SharedString::from(format!(
                                    "synapse/gamepad-{pid}-calibration-close.svg"
                                )))
                                .size(surface::css(20.)),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_calibration_popup(window, cx)
                            })),
                    ),
            )
            .child(
                h_flex()
                    .justify_center()
                    .pt(surface::css(20.))
                    .px(surface::css(25.))
                    .pb(surface::css(30.))
                    .text_center()
                    .child(content),
            );
        // Im/$l closes only from the explicit close control / Cancel / Done.
        Dialog::new(cx)
            .focus_handle(popup.focus.clone())
            .close_on_backdrop_press(false)
            .close_on_escape(false)
            .on_cancel(|_, _, _| false)
            .on_ok(|_, _, _| false)
            .backdrop(div())
            .popup(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .justify_center()
                    .child(panel),
            )
            .into_any_element()
    }
}

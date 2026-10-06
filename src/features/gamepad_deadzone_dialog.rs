//! Current 2636 warning: module 99661 and the mounted thumbstick callbacks.
//! UI draft decisions only. Recalibrate requests navigation, never a device command.
use super::*;
use crate::ui::theme::GamepadDialogColors as Colors;
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::base::{Button as BaseButton, Dialog};
use std::time::Duration;

pub(super) struct DialogState {
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    expanded: bool,
}

impl DialogState {
    pub(super) fn new(window: &mut Window, cx: &mut App) -> Self {
        let return_focus = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            focus,
            return_focus,
            expanded: false,
        }
    }
}

impl GamepadProductWorkspace {
    pub(super) fn dismiss_deadzone(
        &mut self,
        rollback: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((path, previous)) = self.low_deadzone.take() {
            if rollback {
                self.write(&path, previous, cx);
            }
        }
        if let Some(state) = self.deadzone_dialog.take() {
            if let Some(focus) = state.return_focus {
                focus.focus(window, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn render_deadzone_dialog(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(state) = &self.deadzone_dialog else {
            return div().into_any_element();
        };
        let zero = self
            .low_deadzone
            .as_ref()
            .is_some_and(|(path, _)| self.number(path) == 0);
        let title = t(if zero {
            "ZERO_DEADZONES_WARNING"
        } else {
            "LOW_DEADZONES_WARNING"
        });
        let description = t(if zero {
            "ZERO_DEADZONES_WARNING_DESC_1"
        } else {
            "LOW_DEADZONES_WARNING_DESC_1"
        });
        let transition = |ms| Transition::new(Duration::from_millis(ms)).easing(Easing::Ease);
        let height = motion::transition(
            "gamepad-deadzone-detail-height",
            if state.expanded { 200_f32 } else { 0. },
            transition(250),
            window,
            cx,
        );
        let opacity = motion::transition(
            "gamepad-deadzone-detail-opacity",
            if state.expanded { 1_f32 } else { 0. },
            transition(200),
            window,
            cx,
        );
        let angle = motion::transition(
            "gamepad-deadzone-detail-arrow",
            if state.expanded { 90_f32 } else { -90. },
            transition(200),
            window,
            cx,
        );
        let detail = v_flex()
            .w_full()
            .max_w(surface::css(360.))
            .bg(Colors::detail())
            .rounded(surface::css(3.))
            .line_height(surface::css(17.))
            .child(
                BaseButton::new("gamepad-deadzone-expand")
                    .accessibility_label(t("LOW_DEADZONE_INFO_TITLE"))
                    .w_full()
                    .p(surface::css(10.))
                    .cursor_pointer()
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap(surface::css(5.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(
                                        img("synapse/gamepad-2636-info.svg")
                                            .size(surface::css(20.)),
                                    )
                                    .child(t("LOW_DEADZONE_INFO_TITLE")),
                            )
                            .child(
                                svg()
                                    .path("synapse/history-back.svg")
                                    .size(surface::css(20.))
                                    .text_color(Colors::text())
                                    .with_transformation(Transformation::rotate(radians(
                                        angle.to_radians(),
                                    ))),
                            ),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(state) = &mut this.deadzone_dialog {
                            state.expanded = !state.expanded;
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .max_h(surface::css(height))
                    .overflow_hidden()
                    .opacity(opacity)
                    .child(
                        div()
                            .pl(surface::css(35.))
                            .pr(surface::css(30.))
                            .pb(surface::css(10.))
                            .child(t("LOW_DEADZONE_INFO_DETAIL")),
                    ),
            );
        let panel = v_flex()
            .relative()
            .occlude()
            .w(surface::css(402.))
            .p(surface::css(20.))
            .gap(surface::css(20.))
            .bg(Colors::panel())
            .border_1()
            .border_color(Colors::warning())
            .rounded(surface::css(3.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(Colors::text())
            .shadow(vec![BoxShadow {
                color: Colors::shadow(),
                offset: point(px(0.), window.rem_size() * (6. / 16.)),
                blur_radius: window.rem_size() * (10. / 16.),
                spread_radius: px(0.),
                inset: false,
            }])
            .child(
                h_flex()
                    .justify_center()
                    .h(surface::css(20.))
                    .gap(surface::css(10.))
                    .text_size(surface::css(16.))
                    .text_color(Colors::warning())
                    .child(
                        img("synapse/gamepad-2636-warning.svg")
                            .w(surface::css(20.))
                            .h(surface::css(27.)),
                    )
                    .child(title.to_uppercase()),
            )
            .child(
                BaseButton::new("gamepad-deadzone-close")
                    .accessibility_label(t("CLOSE"))
                    .absolute()
                    .top_0()
                    .right_0()
                    .size(surface::css(36.))
                    .child(img("synapse/mapping-close.svg").size(surface::css(20.)))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.dismiss_deadzone(true, window, cx)),
                    ),
            )
            .child(div().child(format!(
                "{description}\n\n{}",
                t("LOW_DEADZONES_WARNING_DESC_2")
            )))
            .child(detail)
            .child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .child(
                        action("gamepad-deadzone-continue", "CONTINUE", false).on_click(
                            cx.listener(|this, _, window, cx| {
                                this.dismiss_deadzone(false, window, cx)
                            }),
                        ),
                    )
                    .child(
                        action(
                            "gamepad-deadzone-recalibrate",
                            "CALIBRATION_RECALIBRATE",
                            true,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.dismiss_deadzone(false, window, cx);
                            cx.emit(GamepadCalibrationRequested);
                        })),
                    ),
            );
        // The source mounts the alert first in sensitivity-container, then
        // applies margin -40px -20px and a 110px panel margin inside it.
        let origin = self.thumbstick_bounds.get().origin;
        let left = origin.x - window.rem_size() * (20. / 16.);
        let top = origin.y - window.rem_size() * (40. / 16.);
        Dialog::new(cx)
            .focus_handle(state.focus.clone())
            // Current module 99661 has neither backdrop nor keyboard dismissal.
            .close_on_backdrop_press(false)
            .close_on_escape(false)
            .on_cancel(|_, _, _| false)
            .on_ok(|_, _, _| false)
            .backdrop(
                div()
                    .absolute()
                    .left(left)
                    .top(top)
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height)
                    .bg(Colors::backdrop()),
            )
            .popup(
                div()
                    .absolute()
                    .top(top + window.rem_size() * (110. / 16.))
                    .left(left)
                    .w(window.viewport_size().width)
                    .flex()
                    .justify_center()
                    .child(panel),
            )
            .into_any_element()
    }
}

fn action(id: &'static str, key: &'static str, primary: bool) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(t(key))
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .px(surface::css(16.))
        .pt(surface::css(7.))
        .pb(surface::css(6.))
        .rounded(surface::css(3.))
        .text_size(surface::css(12.))
        .bg(if primary {
            Colors::primary()
        } else {
            Colors::secondary()
        })
        .text_color(if primary {
            Colors::detail()
        } else {
            Colors::secondary_text()
        })
        .when(primary, |b| {
            b.border_1().border_color(Colors::primary_border())
        })
        .active(|s| s.opacity(0.3))
        .child(t(key).to_uppercase())
}

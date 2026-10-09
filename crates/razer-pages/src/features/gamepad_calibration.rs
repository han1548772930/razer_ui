//! Current 2636 calibration presentation. Artwork, CSS and canvas geometry
//! are recorded in gamepad-2636-calibration-current-evidence.json.
use super::*;
use gpui_kit::base::Dialog;
use razer_widgets::theme::GamepadDialogColors as Colors;
use state::{CalibrationAction, CalibrationState};

#[path = "gamepad_calibration_state.rs"]
pub mod state;
#[cfg(test)]
#[path = "gamepad_calibration_tests.rs"]
mod tests;

pub(super) struct CalibrationDialog {
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
}

impl GamepadProductWorkspace {
    pub fn set_edition_id(&mut self, edition_id: u32, cx: &mut Context<Self>) {
        self.edition_id = edition_id;
        cx.notify();
    }
    pub fn calibration_generation(&self) -> Option<u64> {
        (self.spec.product_id == 2636).then(|| self.calibration_state.generation())
    }
    pub fn calibration_intent(&self) -> Option<&CalibrationIntent> {
        self.calibration_state.intent()
    }
    pub fn observe_calibration(
        &mut self,
        observation: CalibrationObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 2636
            || self.page != "TAB_CALIBRATION"
            || !self.calibration_state.observe(observation)
        {
            return false;
        }
        if self.calibration_state.step == -1 && self.calibration_dialog.is_none() {
            let return_focus = window.focused(cx);
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            self.calibration_dialog = Some(CalibrationDialog {
                focus,
                return_focus,
            });
        } else if self.calibration_state.step != -1 {
            self.dismiss_calibration_error(window, cx);
        }
        cx.notify();
        true
    }
    fn dismiss_calibration_error(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(state) = self.calibration_dialog.take() {
            if let Some(focus) = state.return_focus {
                focus.focus(window, cx);
            }
        }
    }
    fn request_calibration(
        &mut self,
        action: CalibrationAction,
        part: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 2636 || self.page != "TAB_CALIBRATION" {
            return;
        }
        if let Some(intent) = self.calibration_state.request(action, part) {
            self.dismiss_calibration_error(window, cx);
            cx.emit(intent);
            cx.notify();
        }
    }
    pub fn leave_calibration(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id == 2636 {
            if self.calibration_state.step != 0 {
                if let Some(intent) = self
                    .calibration_state
                    .request(CalibrationAction::Stop, self.calibration_state.part)
                {
                    cx.emit(intent);
                }
            } else {
                // Invalidate tester samples even if calibration was never started.
                let generation = self.calibration_state.generation();
                self.calibration_state
                    .observe(CalibrationObservation::unavailable(generation, u64::MAX));
            }
            self.dismiss_calibration_error(window, cx);
        }
    }
    pub(super) fn render_calibration(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let calibration_bounds = self.calibration_bounds.clone();
        // The source's media queries scale only the artwork/tester region.
        // Preserve its unscaled layout box and centre the scaled contents.
        let viewport_css_width = window.viewport_size().width / window.rem_size() * 16.;
        let scale = if viewport_css_width <= 400. {
            0.5
        } else if viewport_css_width <= 650. {
            0.6
        } else if viewport_css_width <= 850. {
            0.8
        } else {
            1.
        };
        let area_length = |value| surface::css(value * scale);
        let state = &self.calibration_state;
        let title = if state.step < 1 {
            t("CALIBRATION_TITLE")
        } else if state.is_complete() {
            t("CALIBRATION_SUCCESS")
        } else {
            String::new()
        };
        let edition = match self.edition_id {
            128 => 128,
            129 => 129,
            _ => 0,
        };
        let mut product = div()
            .relative()
            .w(area_length(372.))
            // Original 1116 × 753 artwork at CSS max-width 372.
            .h(area_length(251.))
            .child(
                img(SharedString::from(format!(
                    "synapse/gamepad-2636-calibration-edition-{edition}.png"
                )))
                .w_full()
                .h_full()
                .object_fit(ObjectFit::Contain),
            );
        if state.step == 0 {
            for (part, left, top) in [(1_u32, 62.75, 69.), (2_u32, 221., 126.)] {
                product = product.child(
                    div()
                        .absolute()
                        .left(area_length(left))
                        .top(area_length(top))
                        .child(
                            img("synapse/gamepad-2636-calibration-thumbstick.svg")
                                .w(area_length(38.))
                                .h(area_length(38.)),
                        )
                        .id(("calibration-stick-target", part)),
                );
            }
        } else if (1..=5).contains(&state.step) {
            if state.valid && state.step != 5 {
                product = product.child(
                    div()
                        .absolute()
                        .left(area_length(56.))
                        .top(area_length(5.))
                        .child(
                            img("synapse/gamepad-2636-calibration-bumper.svg")
                                .w(area_length(64.))
                                .h(area_length(29.)),
                        ),
                );
            } else {
                let (left, top) = if state.part == 1 {
                    (8.75, 11.)
                } else {
                    (166., 68.)
                };
                product = product.child(
                    div()
                        .absolute()
                        .left(area_length(left))
                        .top(area_length(top))
                        .child(direction(state, scale)),
                );
            }
        }
        let description = match state.step {
            0 => "CALIBRATION_STEP0",
            1 => "CALIBRATION_STEP1",
            2 => "CALIBRATION_STEP2",
            3 => "CALIBRATION_STEP3",
            4 => "CALIBRATION_STEP4",
            5 => "CALIBRATION_V2_STEP5",
            _ => "",
        };
        let footer = if state.step == 0 {
            h_flex()
                .justify_center()
                .gap(surface::css(10.))
                .child(
                    action(
                        "gamepad-calibrate-left",
                        "CALIBBRTION_LEFT_THUMBSITCK",
                        ActionStyle::Outline,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_calibration(CalibrationAction::Start, 1, window, cx)
                    })),
                )
                .child(
                    action(
                        "gamepad-calibrate-right",
                        "CALIBBRTION_RIGHT_THUMBSITCK",
                        ActionStyle::Outline,
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_calibration(CalibrationAction::Start, 2, window, cx)
                    })),
                )
        } else if state.step > 0 {
            let complete = state.is_complete();
            h_flex().justify_center().child(
                action(
                    "gamepad-calibration-stop",
                    if complete { "DONE" } else { "CANCEL" },
                    if complete {
                        ActionStyle::Primary
                    } else {
                        ActionStyle::Secondary
                    },
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.request_calibration(
                        CalibrationAction::Stop,
                        this.calibration_state.part,
                        window,
                        cx,
                    )
                })),
            )
        } else {
            h_flex()
        };
        v_flex()
            .id("gamepad-calibration-2636")
            .test_support()
            .relative()
            .w_full()
            .items_center()
            .text_center()
            .text_color(Colors::text())
            .text_size(surface::css(14.))
            .child(
                canvas(
                    move |bounds, _, _| calibration_bounds.set(bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .m(surface::css(10.))
                    .w(surface::css(601.))
                    .h(surface::css(21.))
                    .when(state.step >= 1, |d| {
                        d.child(
                            img(SharedString::from(format!(
                                "synapse/gamepad-2636-calibration-steps-{}.svg",
                                state.step
                            )))
                            .w_full()
                            .h_full(),
                        )
                    }),
            )
            .child(
                div()
                    .id("gamepad-calibration-title")
                    .test_support()
                    .role(Role::Status)
                    .aria_label(title.clone())
                    .min_h(surface::css(22.))
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .text_color(Colors::primary())
                    .child(title.to_uppercase()),
            )
            .child(
                div()
                    .relative()
                    .mt(surface::css(28.))
                    .mb(surface::css(30.))
                    .w(surface::css(850.))
                    .h(surface::css(251.))
                    .child(
                        h_flex()
                            .absolute()
                            .left(surface::css(425. * (1. - scale)))
                            .top(surface::css(125.5 * (1. - scale)))
                            .w(area_length(850.))
                            .h(area_length(251.))
                            .items_end()
                            .justify_between()
                            .child(simulator(1, state.positions[0], scale, state.step == 0))
                            .child(product)
                            .child(simulator(2, state.positions[1], scale, state.step == 0)),
                    ),
            )
            .child(
                div()
                    .id("gamepad-calibration-description")
                    .test_support()
                    .w(surface::css(460.))
                    .min_h(surface::css(44.))
                    .child(if description.is_empty() {
                        String::new()
                    } else {
                        t(description)
                    }),
            )
            .child(div().mt(surface::css(10.)).child(footer))
            .child(
                div()
                    .id("gamepad-calibration-observation-status")
                    .test_support()
                    .role(Role::Status)
                    .mt_3()
                    .text_size(surface::css(12.))
                    .text_color(Colors::secondary())
                    .child(if state.intent().is_some() {
                        "校准操作仅保留在本地，尚未发送至设备。"
                    } else if state.positions.iter().all(Option::is_none) {
                        "尚未收到手柄位置。"
                    } else {
                        ""
                    }),
            )
            .into_any_element()
    }
    pub(super) fn render_calibration_error(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(state) = &self.calibration_dialog else {
            return div().into_any_element();
        };
        let panel = v_flex()
            .id("gamepad-calibration-error")
            .test_support()
            .occlude()
            .w(surface::css(400.))
            .py(surface::css(20.))
            .px(surface::css(30.))
            .gap(surface::css(10.))
            .text_center()
            .bg(Colors::panel())
            .border_1()
            .border_color(Colors::warning())
            .rounded(surface::css(3.))
            .text_color(Colors::text())
            .child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .text_color(Colors::warning())
                    .text_size(surface::css(16.))
                    .font_family("RazerF5")
                    .child(
                        img("synapse/gamepad-2636-calibration-warning.svg").size(surface::css(20.)),
                    )
                    .child(t("CALIBRATION_ERROR")),
            )
            .child(
                div()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .child(t("CALIBRATION_ERROR_DESC")),
            )
            .child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .child(
                        action(
                            "gamepad-calibration-error-cancel",
                            "CANCEL",
                            ActionStyle::Secondary,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.request_calibration(
                                CalibrationAction::Stop,
                                this.calibration_state.part,
                                window,
                                cx,
                            )
                        })),
                    )
                    .child(
                        action(
                            "gamepad-calibration-error-retry",
                            "CALIBRATION_RECALIBRATE",
                            ActionStyle::Primary,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.request_calibration(
                                CalibrationAction::Start,
                                this.calibration_state.part,
                                window,
                                cx,
                            )
                        })),
                    ),
            );
        // XM has only explicit Cancel/Recalibrate, no backdrop/keyboard submit.
        Dialog::new(cx)
            .focus_handle(state.focus.clone())
            .close_on_backdrop_press(false)
            .close_on_escape(false)
            .on_cancel(|_, _, _| false)
            .on_ok(|_, _, _| false)
            .backdrop(div())
            .popup(
                div()
                    .absolute()
                    .top(self.calibration_bounds.get().origin.y + window.rem_size() * (100. / 16.))
                    .left_0()
                    .w(window.viewport_size().width)
                    .flex()
                    .justify_center()
                    .child(panel),
            )
            .into_any_element()
    }
}

enum ActionStyle {
    Outline,
    Secondary,
    Primary,
}
fn action(id: &'static str, key: &'static str, style: ActionStyle) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(t(key))
        .h(surface::css(27.))
        .px(surface::css(14.))
        .rounded(surface::css(3.))
        .text_size(surface::css(12.))
        .flex()
        .items_center()
        .justify_center()
        .when(matches!(style, ActionStyle::Outline), |b| {
            b.border_1()
                .border_color(Colors::text())
                .text_color(Colors::text())
                .hover(|s| s.bg(Colors::panel()))
        })
        .when(matches!(style, ActionStyle::Secondary), |b| {
            b.min_w(surface::css(90.))
                .border_1()
                .border_color(Colors::primary_border())
                .bg(Colors::secondary())
                .text_color(Colors::secondary_text())
        })
        .when(matches!(style, ActionStyle::Primary), |b| {
            b.min_w(surface::css(90.))
                .bg(Colors::primary())
                .text_color(Colors::primary_text())
                .border_1()
                .border_color(Colors::primary_border())
        })
        .active(|s| s.opacity(0.3))
        .child(if matches!(style, ActionStyle::Secondary) {
            t(key).to_uppercase()
        } else {
            t(key)
        })
}

fn simulator(part: u32, position: Option<(f64, f64)>, scale: f32, visible: bool) -> AnyElement {
    let length = |value| surface::css(value * scale);
    let (value_x, value_y) = position.map_or_else(
        || ("X: —".to_owned(), "Y: —".to_owned()),
        |(x, y)| {
            (
                format!("X: {:.1}%", x / 10.),
                format!("Y: {:.1}%", -y / 10.),
            )
        },
    );
    let mut chart = div()
        .relative()
        .w(length(162.))
        .h(length(160.))
        .child(img("synapse/gamepad-2636-calibration-simulator.svg").size(length(162.)));
    if let Some((x, y)) = position {
        let (x, y) = project(x, y, 153., 78.);
        chart = chart.child(sample(x + 81., y + 81., 162., true, scale));
    }
    div()
        .id(("gamepad-calibration-simulator", part))
        .test_support()
        .w(length(162.))
        .mb(length(-38.5))
        .when(!visible, |d| d.invisible())
        .child(chart)
        .child(
            h_flex()
                .mt(length(20.))
                .gap(length(20.))
                .justify_center()
                .text_size(length(14.))
                .child(value_x)
                .child(value_y),
        )
        .into_any_element()
}
fn project(x: f64, y: f64, span: f64, radius: f64) -> (f64, f64) {
    let x = x * span / 2000.;
    let y = y * span / 2000.;
    let length = x.hypot(y);
    if length > radius {
        (x * radius / length, y * radius / length)
    } else {
        (x, y)
    }
}
fn direction(state: &CalibrationState, scale: f32) -> Div {
    let length = |value| surface::css(value * scale);
    let position = state
        .part
        .checked_sub(1)
        .and_then(|p| state.positions.get(usize::from(p)))
        .copied()
        .flatten();
    let edge = position.is_some_and(|(x, y)| (x * 141. / 2000.).hypot(y * 141. / 2000.) >= 68.5);
    let mut chart = div()
        .relative()
        .w(length(150.))
        .h(length(157.))
        .text_size(length(14.))
        .child(
            img(SharedString::from(format!(
                "synapse/gamepad-2636-calibration-direction-{}-{}.svg",
                state.step,
                if edge { "edge" } else { "rest" }
            )))
            .w(length(150.))
            .h(length(150.)),
        );
    if let Some((x, y)) = position {
        let (x, y) = project(x, y, 141., 73.);
        chart = chart.child(sample(x + 75., y + 75., 150., false, scale));
    }
    chart.when(state.step == 5, |d| {
        d.child(
            div()
                .id("gamepad-calibration-rotations")
                .test_support()
                .absolute()
                .top(length(157.))
                .w_full()
                .text_center()
                .text_color(Colors::secondary_text())
                .child(format!("{}/3", state.rotations)),
        )
    })
}
fn sample(x: f64, y: f64, extent: f64, line: bool, scale: f32) -> AnyElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let at = |x: f64, y: f64| {
                point(
                    bounds.left() + bounds.size.width * (x / extent) as f32,
                    bounds.top() + bounds.size.height * (y / extent) as f32,
                )
            };
            if line {
                let mut p = PathBuilder::stroke(bounds.size.width * (2. / extent) as f32);
                p.move_to(at(extent / 2., extent / 2.));
                p.line_to(at(x, y));
                if let Ok(path) = p.build() {
                    window.paint_path(path, Colors::primary());
                }
            }
            let mut dot = PathBuilder::fill();
            for i in 0..=32 {
                let a = f64::from(i) * std::f64::consts::TAU / 32.;
                let p = at(x + 5. * a.cos(), y + 5. * a.sin());
                if i == 0 {
                    dot.move_to(p);
                } else {
                    dot.line_to(p);
                }
            }
            dot.close();
            if let Ok(path) = dot.build() {
                window.paint_path(path, Colors::primary());
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .w(surface::css(extent as f32 * scale))
    .h(surface::css(extent as f32 * scale))
    .into_any_element()
}

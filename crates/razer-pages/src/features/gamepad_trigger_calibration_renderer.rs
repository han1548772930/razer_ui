//! 2676 ap/tp and 2684 Oc/Rc, rendered from each current source independently.
//! The owner supplies service progress and independent visual animation values.
use super::*;
use razer_widgets::theme::GamepadDialogColors as Colors;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(super) struct TriggerCalibrationView {
    pub part: u8,
    pub step: i8,
    pub valid: bool,
    pub raw_trigger_percent: Option<f64>,
    pub timer_progress: f64,
    pub marker_progress: f64,
    pub prompt_progress: f64,
}

pub(super) fn render(
    pid: u32,
    edition: u32,
    view: TriggerCalibrationView,
    cx: &Context<GamepadProductWorkspace>,
) -> AnyElement {
    let error = view.step == -1;
    let complete = view.step == 12;
    let right = view.part == 4;
    let mut content = v_flex()
        .w(surface::css(601.))
        .min_h(surface::css(352.))
        .items_center()
        .text_center()
        .font_family("Roboto")
        .text_size(surface::css(14.))
        .text_color(Colors::text())
        .child(
            h_flex()
                .w(surface::css(601.))
                .h(surface::css(21.))
                .justify_center()
                .child(
                    img(SharedString::from(format!(
                        "synapse/gamepad-{pid}-trigger-steps-{}.svg",
                        if !error && view.step >= 11 { 2 } else { 1 }
                    )))
                    .w(surface::css(90.25))
                    .h(surface::css(21.)),
                ),
        );
    if error {
        content = content.child(
            v_flex()
                .items_center()
                .w(surface::css(601.))
                .mt(surface::css(50.))
                .flex_1()
                .child(
                    h_flex()
                        .items_start()
                        .justify_center()
                        .gap(surface::css(10.))
                        .h(surface::css(34.))
                        .font_family("RazerF5")
                        .text_size(surface::css(16.))
                        .line_height(surface::css(16.))
                        .text_color(Colors::secondary_text())
                        .child(
                            h_flex()
                                .justify_center()
                                .items_center()
                                .size(surface::css(24.))
                                .flex_shrink_0()
                                .child(
                                    img(SharedString::from(format!(
                                        "synapse/gamepad-{pid}-trigger-warning.svg"
                                    )))
                                    .size(surface::css(20.)),
                                ),
                        )
                        .child(
                            div()
                                .mt(surface::css(4.))
                                .child(t("FAILED_CALIBRATION_SHORT").to_uppercase()),
                        ),
                )
                .child(visualization(pid, edition, view))
                .child(actions(view, true, cx)),
        );
    } else {
        let key = if complete {
            "CALIBRATION_SUCCESS"
        } else if view.step >= 11 {
            "TRIGGER_CALIBRATION_STEP2"
        } else {
            "TRIGGER_CALIBRATION_STEP1"
        };
        let text = t(key).replace(
            "{{trigger}}",
            &t(if right {
                "RIGHT_TRIGGER"
            } else {
                "LEFT_TRIGGER"
            }),
        );
        content = content
            .child(
                div()
                    .mt(surface::css(50.))
                    .min_h(surface::css(34.))
                    .w(surface::css(if !complete && view.step > 10 {
                        450.
                    } else {
                        400.
                    }))
                    .line_height(surface::css(17.))
                    .when(complete, |d| {
                        d.font_family("RazerF5")
                            .text_size(surface::css(16.))
                            .line_height(surface::css(16.))
                            .text_color(Colors::primary())
                    })
                    .child(if complete { text.to_uppercase() } else { text }),
            )
            .child(visualization(pid, edition, view))
            .child(actions(view, false, cx));
    }
    content.into_any_element()
}

fn actions(
    view: TriggerCalibrationView,
    error: bool,
    cx: &Context<GamepadProductWorkspace>,
) -> AnyElement {
    let complete = view.step == 12;
    h_flex()
        .justify_center()
        .mt(surface::css(30.))
        .child(
            button(
                "gamepad-trigger-calibration-stop",
                if complete { "DONE" } else { "CANCEL" },
                complete,
            )
            .on_click(
                cx.listener(|this, _, window, cx| this.close_trigger_calibration_popup(window, cx)),
            ),
        )
        .when(error, |d| {
            d.child(
                button(
                    "gamepad-trigger-calibration-retry",
                    "CALIBRATION_RECALIBRATE",
                    true,
                )
                .on_click(
                    cx.listener(|this, _, window, cx| this.restart_trigger_calibration(window, cx)),
                ),
            )
        })
        .into_any_element()
}

fn button(id: &'static str, key: &'static str, primary: bool) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(t(key))
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .mt(surface::css(10.))
        .mx(surface::css(5.))
        .px(surface::css(14.))
        .rounded(surface::css(3.))
        .border_1()
        .border_color(Colors::primary_border())
        .text_size(surface::css(12.))
        .bg(if primary {
            Colors::primary()
        } else {
            Colors::secondary()
        })
        .text_color(if primary {
            Colors::primary_text()
        } else {
            Colors::secondary_text()
        })
        .flex()
        .justify_center()
        .items_center()
        .child(t(key).to_uppercase())
}

fn visualization(pid: u32, _edition: u32, view: TriggerCalibrationView) -> AnyElement {
    let mut area = div()
        .relative()
        .w(surface::css(601.))
        .h(surface::css(158.))
        .mt(surface::css(20.))
        .child(
            div()
                .absolute()
                .left(surface::css(114.5))
                .top_0()
                .w(surface::css(372.))
                .h(surface::css(158.))
                .overflow_hidden()
                .child(
                    img(artwork(pid, view.part, view.step))
                        .w_full()
                        .h_full()
                        .object_fit(ObjectFit::Contain),
                ),
        );
    if matches!(view.step, 10 | 11) {
        let right = view.part == 4;
        let mut meter = div()
            .absolute()
            .top(surface::css(3.))
            .when(right, |d| d.right(surface::css(39.56)))
            .when(!right, |d| d.left(surface::css(39.56)))
            .w(surface::css(74.94))
            .h(surface::css(138.91));
        // The timer sits under the transparent meter. Neither of these values
        // can advance the externally supplied SDK progress step.
        if view.valid {
            meter = meter.child(
                div()
                    .absolute()
                    .top(surface::css(19.455))
                    .when(right, |d| d.right(surface::css(19.455)))
                    .when(!right, |d| d.left(surface::css(19.455)))
                    .size(surface::css(100.))
                    .when(view.timer_progress > 0., |d| {
                        d.child(img(timer(pid, view.timer_progress)).size_full())
                    }),
            );
        }
        meter = meter.child(
            div()
                .absolute()
                .inset_0()
                .child(img(meter_svg(pid, view)).w_full().h_full()),
        );
        area = area.child(meter);
    }
    area.into_any_element()
}

fn clamp_percentage(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0., 100.)
    } else {
        0.
    }
}

fn meter_svg(pid: u32, view: TriggerCalibrationView) -> Arc<Image> {
    let source = match pid {
        2676 => include_str!("../../../../assets/synapse/gamepad-2676-trigger-meter-template.svg"),
        2684 => include_str!("../../../../assets/synapse/gamepad-2684-trigger-meter-template.svg"),
        _ => unreachable!("current trigger calibration product"),
    };
    let percent = 100. - clamp_percentage(view.raw_trigger_percent.unwrap_or(100.));
    // Parent already performs the source's 50ms cubic marker interpolation.
    let angle = -std::f64::consts::FRAC_PI_2
        - clamp_percentage(view.marker_progress) / 100. * std::f64::consts::PI;
    let x = 70.4707 + 63.9707 * angle.cos();
    let y = 70.4707 + 63.9707 * angle.sin();
    let (prompt_angle, prompt_opacity) = if view.step == 10 && percent <= 5. {
        prompt_style(view.prompt_progress)
    } else {
        (0., 0.)
    };
    let white = if view.step == 11 {
        percent > 95.
    } else {
        view.valid
    };
    let svg = source
        .replace(
            "{{mirror}}",
            if view.part == 4 {
                "translate(77 0) scale(-1 1)"
            } else {
                ""
            },
        )
        .replace("{{track}}", if white { "#FFFFFF" } else { "#555555" })
        .replace("{{x}}", &x.to_string())
        .replace("{{y}}", &y.to_string())
        .replace("{{prompt_angle}}", &prompt_angle.to_string())
        .replace("{{prompt_opacity}}", &prompt_opacity.to_string());
    Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
}

/// Verbatim sweep/fade keyframes, including their cubic-bezier(.5,0,.5,1)
/// interval. Source percentages are preserved instead of a uniform spin.
fn prompt_style(progress: f64) -> (f64, f64) {
    let p = if progress.is_finite() {
        progress.clamp(0., 1.)
    } else {
        0.
    };
    let angle = if p <= 0.0744 {
        0.
    } else if p >= 0.3509 {
        -90.
    } else {
        -90. * bezier_half((p - 0.0744) / (0.3509 - 0.0744))
    };
    let opacity = if p <= 0.25 {
        p / 0.25
    } else if p <= 0.5657 {
        1.
    } else if p < 0.7218 {
        (0.7218 - p) / (0.7218 - 0.5657)
    } else {
        0.
    };
    (angle, opacity)
}

fn bezier_half(x: f64) -> f64 {
    let mut t = x;
    for _ in 0..8 {
        let actual = 1.5 * t - 1.5 * t * t + t * t * t;
        let slope = 1.5 - 3. * t + 3. * t * t;
        t = (t - (actual - x) / slope).clamp(0., 1.);
    }
    3. * t * t - 2. * t * t * t
}

fn timer(pid: u32, progress: f64) -> Arc<Image> {
    let p = clamp_percentage(progress);
    let svg = if p >= 100. {
        "<svg width=\"100\" height=\"100\" viewBox=\"0 0 100 100\" xmlns=\"http://www.w3.org/2000/svg\"><circle cx=\"50\" cy=\"50\" r=\"50\" fill=\"#44D62C\" opacity=\"0.3\"/></svg>".to_owned()
    } else {
        let source = match pid {
            2676 => {
                include_str!("../../../../assets/synapse/gamepad-2676-trigger-timer-template.svg")
            }
            2684 => {
                include_str!("../../../../assets/synapse/gamepad-2684-trigger-timer-template.svg")
            }
            _ => unreachable!("current trigger calibration product"),
        };
        let angle = p / 100. * std::f64::consts::TAU;
        source
            .replace("{{large_arc}}", if p > 50. { "1" } else { "0" })
            .replace("{{x}}", &(50. + 50. * angle.sin()).to_string())
            .replace("{{y}}", &(50. - 50. * angle.cos()).to_string())
    };
    Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
}

fn artwork(pid: u32, part: u8, step: i8) -> Arc<Image> {
    static FIRST: OnceLock<[Arc<Image>; 5]> = OnceLock::new();
    static SECOND: OnceLock<[Arc<Image>; 5]> = OnceLock::new();
    let (cache, source) = match pid {
        2676 => (
            &FIRST,
            include_str!(
                "../../../../assets/synapse/gamepad-2676-calibration-trigger-artwork-edition-0.svg"
            ),
        ),
        2684 => (
            &SECOND,
            include_str!(
                "../../../../assets/synapse/gamepad-2684-calibration-trigger-artwork-edition-0.svg"
            ),
        ),
        _ => unreachable!("current trigger calibration product"),
    };
    let index = if step == 11 {
        0
    } else if step == 12 {
        if part == 4 { 4 } else { 3 }
    } else if part == 4 {
        2
    } else {
        1
    };
    cache.get_or_init(|| std::array::from_fn(|variant| {
        let mut svg = source.to_owned();
        for (class, active, successful) in [
            ("leftTrigger", matches!(variant, 1 | 3), variant == 3),
            ("rightTrigger", matches!(variant, 2 | 4), variant == 4),
        ] {
            svg = set_source_opacity(&svg, class, if active {0.3} else {0.});
            if successful {
                svg = svg.replace(&format!("class=\"{class}\""),
                    &format!("class=\"{class}\" style=\"fill-opacity:0.2;stroke:#44D62C;stroke-width:1;opacity:1\""));
            }
        }
        Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
    }))[index].clone()
}

/// wL/vl changes opacity only if it follows the exact class inside the same
/// original start tag. Path geometry and every other source byte remain intact.
fn set_source_opacity(source: &str, class: &str, value: f64) -> String {
    let marker = format!("class=\"{class}\"");
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    while let Some(relative) = source[cursor..].find(&marker) {
        let class_end = cursor + relative + marker.len();
        let Some(tag_end_relative) = source[class_end..].find('>') else {
            break;
        };
        let tag_end = class_end + tag_end_relative;
        let Some(opacity_relative) = source[class_end..tag_end].find(" opacity=\"") else {
            output.push_str(&source[cursor..class_end]);
            cursor = class_end;
            continue;
        };
        let start = class_end + opacity_relative + " opacity=\"".len();
        let Some(end_relative) = source[start..tag_end].find('"') else {
            break;
        };
        output.push_str(&source[cursor..start]);
        output.push_str(&value.to_string());
        cursor = start + end_relative;
    }
    output.push_str(&source[cursor..]);
    output
}

//! Current Seiren1422/1446 EQ6007 ye rotary pointer and 4561 base geometry.
//! The source uses floor for ring indices and a 120 x 110 viewport.
use super::colors::Colors as C;
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_widgets::surface;

const START: f32 = 142.5;
const END: f32 = 397.5;
const GAP: f32 = 2.55;

fn percent(value: f32, min: f32, max: f32) -> f32 {
    if min == max {
        0.
    } else {
        ((value.clamp(min, max) - min) / (max - min) * 100.).round()
    }
}

fn source_angle(x: f32, y: f32) -> f32 {
    // Match module4024 Lg's quadrant handling, including both branches at y == 0.
    let mut angle = if x != 0. {
        (y / x).atan().to_degrees().floor()
    } else if y < 0. {
        -90.
    } else {
        90.
    };
    if x < 0. && y >= 0. {
        angle -= 180.;
    }
    if x < 0. && y <= 0. {
        angle += 180.;
    }
    if angle < 0. {
        angle += 360.;
    }
    angle
}

fn index_from_angle(mut angle: f32) -> (f32, f32) {
    if (90. ..=START).contains(&angle) {
        angle = START;
    } else if (37.5..=90.).contains(&angle) {
        angle = 37.5;
    }
    if (0. ..=37.5).contains(&angle) {
        angle += 360.;
    }
    (((angle - START) / GAP).floor().min(100.), angle)
}

fn source_value(index: f32, min: f32, max: f32, step: f32) -> f32 {
    // ua(false, index, min, max, step) rounds decimal precision, not step count.
    let digits = step
        .to_string()
        .split_once('.')
        .map_or(0, |(_, tail)| tail.len());
    let factor = 10_f32.powi(digits as i32);
    (((max - min) * index / 100. + min) * factor).round() / factor
}

#[derive(Default)]
struct Pointer {
    down: bool,
    ready: bool,
    selected: bool,
    vertical: bool,
    previous_y: f32,
    index: f32,
    degree: f32,
    observed: Option<f32>,
}
impl Pointer {
    fn observe(&mut self, value: f32, min: f32, max: f32) {
        if self.observed != Some(value) {
            self.observed = Some(value);
            self.index = percent(value, min, max);
            self.degree = START + self.index * GAP;
        }
    }
    fn ring(x: f32, y: f32) -> bool {
        let radius = x * x + y * y;
        radius <= 60_f32.powi(2) && radius > 48.5_f32.powi(2)
    }
    fn press(&mut self, x: f32, y: f32) {
        let radius = x * x + y * y;
        self.down = true;
        self.ready = radius <= 60.5_f32.powi(2) && radius > 25_f32.powi(2);
        self.selected =
            (-60. ..=60.).contains(&x) && (-60. ..=55.).contains(&y) && !Self::ring(x, y);
    }
    fn turn(&mut self, x: f32, y: f32) -> Option<f32> {
        let angle = source_angle(x, y) - 0.5;
        if (self.index == 0. && (angle < 90. || (180. ..=360.).contains(&angle)))
            || (self.index == 100. && angle > 90.)
            || !((120. ..=360.).contains(&angle) || (0. ..=60.).contains(&angle))
        {
            return None;
        }
        (self.index, self.degree) = index_from_angle(angle);
        Some(self.index)
    }
    fn motion(&mut self, x: f32, y: f32) -> Option<f32> {
        if self.down && !self.ready && (-60. ..=60.).contains(&x) && self.selected {
            self.vertical = true;
            if self.previous_y == y {
                return None;
            }
            let degree = self.degree + if self.previous_y < y { -7. } else { 7. };
            let index = ((degree - START) / GAP).floor().min(100.);
            if index < 0. || degree >= END {
                if degree >= END {
                    self.index = 100.;
                    self.degree = END;
                } else if self.index > 0. && self.index < 100. {
                    self.index = 0.;
                    self.degree = degree;
                }
                return None;
            }
            self.index = index;
            self.degree = degree;
            // ye's vertical branch changes its canvas state only. It does not
            // call onChange here; mouseup still follows its clickTurn branch.
        }
        let next = if self.ready && self.down {
            self.turn(x, y)
        } else {
            None
        };
        self.previous_y = y;
        next
    }
    fn release(&mut self, x: f32, y: f32) -> Option<f32> {
        let angle = source_angle(x, y);
        let next = if ((self.down && self.ready) || self.vertical) && Self::ring(x, y) && angle > 0.
        {
            (self.index, self.degree) = index_from_angle(angle - 0.5);
            Some(self.index)
        } else {
            None
        };
        self.down = false;
        self.selected = false;
        self.vertical = false;
        next
    }
}

fn submit(state: &Entity<SliderState>, value: f32, window: &mut Window, cx: &mut App) {
    state.update(cx, |state, cx| {
        state.set_value(value, window, cx);
        cx.emit(SliderEvent::Change(value.into()));
    });
}

#[derive(IntoElement)]
pub(super) struct SeirenKnob {
    state: Entity<SliderState>,
    label: SharedString,
    unit: SharedString,
    enabled: bool,
    footer: Option<AnyElement>,
}
impl SeirenKnob {
    pub fn new(
        state: &Entity<SliderState>,
        label: impl Into<SharedString>,
        unit: impl Into<SharedString>,
        enabled: bool,
    ) -> Self {
        Self {
            state: state.clone(),
            label: label.into(),
            unit: unit.into(),
            enabled,
            footer: None,
        }
    }
    pub fn footer(mut self, footer: AnyElement) -> Self {
        self.footer = Some(footer);
        self
    }
}

impl RenderOnce for SeirenKnob {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let value = self.state.read(cx).value().start();
        let min = self.state.read(cx).min_value();
        let max = self.state.read(cx).max_value();
        let step = self.state.read(cx).step_value();
        let pointer =
            window.use_keyed_state(("seiren-eq-knob", self.state.entity_id()), cx, |_, _| {
                let mut pointer = Pointer::default();
                pointer.observe(value, min, max);
                pointer
            });
        let gesture = pointer.read(cx);
        let (index, degree) = if gesture.observed == Some(value) {
            (gesture.index, gesture.degree)
        } else {
            let index = percent(value, min, max);
            (index, START + index * GAP)
        };
        let enabled = self.enabled;
        let percent_unit = self.unit.as_ref() == "%";
        let state = self.state.clone();
        let increase = state.clone();
        let decrease = state.clone();
        let drawing = canvas(
            |_, _, _| (),
            move |bounds, _, window, cx| {
                let scale = f32::from(bounds.size.width) / 120.;
                let centre = bounds.origin + point(px(60. * scale), px(60. * scale));
                paint_knob(centre, scale, index, degree, window);
                if !enabled {
                    pointer.update(cx, |pointer, _| {
                        pointer.down = false;
                        pointer.ready = false;
                        pointer.selected = false;
                        pointer.vertical = false;
                    });
                    return;
                }
                let down = pointer.clone();
                let movement = pointer.clone();
                let up = pointer.clone();
                let pressed_state = state.clone();
                let moved_state = state.clone();
                let released_state = state.clone();
                let local = move |position: Point<Pixels>| {
                    (
                        f32::from(position.x - centre.x) / scale,
                        f32::from(position.y - centre.y) / scale,
                    )
                };
                window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                    if !phase.capture() || event.button != MouseButton::Left {
                        return;
                    }
                    let (x, y) = local(event.position);
                    let value = pressed_state.read(cx).value().start();
                    down.update(cx, |pointer, cx| {
                        pointer.observe(value, min, max);
                        pointer.press(x, y);
                        cx.notify();
                    });
                });
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if !phase.capture() {
                        return;
                    }
                    let (x, y) = local(event.position);
                    let value = moved_state.read(cx).value().start();
                    let next = movement.update(cx, |pointer, cx| {
                        pointer.observe(value, min, max);
                        if !event.dragging() {
                            pointer.down = false;
                        }
                        let next = pointer.motion(x, y);
                        if pointer.down {
                            cx.notify();
                        }
                        next
                    });
                    if let Some(index) = next {
                        submit(
                            &moved_state,
                            source_value(index, min, max, step),
                            window,
                            cx,
                        );
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if !phase.capture() || event.button != MouseButton::Left {
                        return;
                    }
                    let (x, y) = local(event.position);
                    let value = released_state.read(cx).value().start();
                    let next = up.update(cx, |pointer, cx| {
                        pointer.observe(value, min, max);
                        let next = pointer.release(x, y);
                        cx.notify();
                        next
                    });
                    if let Some(index) = next {
                        submit(
                            &released_state,
                            source_value(index, min, max, step),
                            window,
                            cx,
                        );
                    }
                });
            },
        )
        .w(surface::css(120.))
        .h(surface::css(110.));
        div()
            .id(("audio-seiren-eq-knob", self.state.entity_id()))
            .role(Role::Slider)
            .aria_label(self.label.clone())
            .aria_numeric_value(f64::from(value))
            .aria_min_numeric_value(f64::from(min))
            .aria_max_numeric_value(f64::from(max))
            .aria_numeric_value_step(f64::from(step))
            .when(enabled, |element| {
                element
                    .on_a11y_action(AccessibleAction::Increment, move |_, window, cx| {
                        let value = (increase.read(cx).value().start() + step).min(max);
                        submit(&increase, value, window, cx);
                    })
                    .on_a11y_action(AccessibleAction::Decrement, move |_, window, cx| {
                        let value = (decrease.read(cx).value().start() - step).max(min);
                        submit(&decrease, value, window, cx);
                    })
            })
            .flex()
            .flex_col()
            .items_center()
            .w(surface::css(120.))
            .child(
                div()
                    .relative()
                    .w(surface::css(120.))
                    .h(surface::css(110.))
                    .child(drawing)
                    .child(
                        div()
                            .absolute()
                            .top(surface::css(58.))
                            .left_0()
                            .w_full()
                            .flex()
                            .when(!percent_unit, |el| el.flex_col())
                            .items_center()
                            .justify_center()
                            .mt(surface::css(if self.unit.is_empty() || percent_unit {
                                -8.
                            } else {
                                -16.
                            }))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(16.))
                            .text_color(C::text())
                            .child(value.to_string())
                            .when(!self.unit.is_empty(), |el| el.child(self.unit)),
                    ),
            )
            .child(self.footer.unwrap_or_else(|| {
                div()
                    .mt(surface::css(10.))
                    .w_full()
                    .text_center()
                    .text_size(surface::css(14.))
                    .text_color(C::text())
                    .child(self.label.to_uppercase())
                    .into_any_element()
            }))
    }
}

fn arc(
    centre: Point<Pixels>,
    radius: f32,
    from: f32,
    to: f32,
    width: f32,
    color: Hsla,
    window: &mut Window,
) {
    if to <= from {
        return;
    }
    let project = |angle: f32| {
        centre
            + point(
                px(angle.to_radians().cos() * radius),
                px(angle.to_radians().sin() * radius),
            )
    };
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(project(from));
    let segments = ((to - from) / 170.).ceil() as usize;
    for i in 1..=segments {
        path.arc_to(
            point(px(radius), px(radius)),
            px(0.),
            false,
            true,
            project(from + (to - from) * i as f32 / segments as f32),
        );
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
fn circle(centre: Point<Pixels>, radius: f32, color: Hsla, window: &mut Window) {
    let mut path = PathBuilder::fill();
    path.move_to(centre + point(px(radius), px(0.)));
    path.arc_to(
        point(px(radius), px(radius)),
        px(0.),
        false,
        true,
        centre - point(px(radius), px(0.)),
    );
    path.arc_to(
        point(px(radius), px(radius)),
        px(0.),
        false,
        true,
        centre + point(px(radius), px(0.)),
    );
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
fn paint_knob(centre: Point<Pixels>, scale: f32, index: f32, degree: f32, window: &mut Window) {
    circle(centre, 50. * scale, C::knob(), window);
    arc(
        centre,
        47. * scale,
        0.,
        360.,
        6. * scale,
        C::surface(),
        window,
    );
    arc(centre, 49.5 * scale, 0., 360., scale, C::border(), window);
    circle(centre, 25. * scale, C::surface(), window);
    arc(
        centre,
        47. * scale,
        0.,
        360.,
        5.5 * scale,
        C::surface(),
        window,
    );
    // ye's rotated linear-gradient highlight on the right half of the inner ring.
    for segment in 0..90 {
        let from = -90. + segment as f32 * 2.;
        let y = (from + 1.).to_radians().sin() * 47.;
        let alpha = (1. - y.abs() / 20.).max(0.) * 0.5;
        if alpha > 0. {
            arc(
                centre,
                47. * scale,
                degree + from,
                degree + from + 2.,
                5.5 * scale,
                C::selected().opacity(alpha),
                window,
            );
        }
    }
    let project = |radius: f32, angle: f32| {
        centre
            + point(
                px(radius * scale * angle.to_radians().cos()),
                px(radius * scale * angle.to_radians().sin()),
            )
    };
    circle(project(39., degree), 2. * scale, C::selected(), window);
    let active_end = START + index * GAP;
    arc(
        centre,
        58. * scale,
        START,
        active_end,
        4. * scale,
        C::selected(),
        window,
    );
    arc(
        centre,
        58. * scale,
        active_end,
        END,
        4. * scale,
        C::remainder(),
        window,
    );
    circle(project(58., START), 2. * scale, C::selected(), window);
    circle(project(58., active_end), 2. * scale, C::selected(), window);
    circle(
        project(58., END),
        2. * scale,
        if index == 100. {
            C::selected()
        } else {
            C::remainder()
        },
        window,
    );
}

#[cfg(test)]
mod tests {
    use super::{Pointer, index_from_angle, percent, source_angle, source_value};
    #[core::prelude::v1::test]
    fn seiren_eq_source_knob_ranges_and_precision() {
        assert_eq!(percent(1.6, 0.6, 2.7), 48.);
        assert_eq!(source_value(0., 0.6, 2.7, 0.1), 0.6);
        assert_eq!(source_value(100., 0.6, 2.7, 0.1), 2.7);
        assert_eq!(source_value(48., 0.6, 2.7, 0.1), 1.6);
        assert_eq!(source_value(50., 110., 200., 1.), 155.);
        assert_eq!(index_from_angle(90.), (0., 142.5));
        assert_eq!(index_from_angle(37.5), (100., 397.5));
        assert_eq!(source_angle(-10., 0.), 0.);
    }
    #[core::prelude::v1::test]
    fn seiren_eq_inactive_arc_and_source_vertical_motion_do_not_emit_writes() {
        let mut pointer = Pointer::default();
        pointer.observe(0.5, 0., 1.);
        pointer.press(0., 0.);
        assert_eq!(pointer.motion(0., -10.), None);
        assert_eq!(pointer.release(0., -10.), None);
        assert!(!pointer.down);
        pointer.press(0., -58.);
        assert!(pointer.motion(1., -58.).is_some());
        assert!(pointer.release(1., -58.).is_some());
    }
}

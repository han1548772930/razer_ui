//! CSS/SMIL presentation from the current 784 bundle; Base retains behavior.
use super::*;
use gpui_kit::base::motion::{self, Easing, Transition};
use std::time::Duration;

#[derive(IntoElement)]
pub(super) struct ShapeChoice {
    pub(super) sides: u32,
    pub(super) selected: bool,
    pub(super) enabled: bool,
    pub(super) owner: WeakEntity<AetherStrip>,
}
impl RenderOnce for ShapeChoice {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = ElementId::from(("aether-shape", self.sides));
        let transition = Transition::new(Duration::from_millis(300)).easing(Easing::EaseInOut);
        let border = motion::transition(
            (id.clone(), "border"),
            if self.selected {
                cx.theme().primary
            } else {
                Colors::border()
            },
            transition.clone(),
            window,
            cx,
        );
        let background = motion::transition(
            (id.clone(), "background"),
            if self.selected {
                Colors::selected()
            } else {
                cx.theme().transparent
            },
            transition,
            window,
            cx,
        );
        gpui_kit::base::Button::new(id)
            .disabled(!self.enabled)
            .accessibility_label(format!("{} · {}", text("DEVICE_LAYOUT"), self.sides))
            .selected(self.selected)
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(border)
            .bg(background)
            .rounded(surface::css(3.))
            .py(surface::css(9.))
            .px(surface::css(15.))
            .focus_visible(|v| v.border_color(cx.theme().primary))
            .child(img(asset(&format!("shape-{}", self.sides))).size(surface::css(40.)))
            .on_click(move |_, window, cx| {
                let _ = self
                    .owner
                    .update(cx, |view, cx| view.choose_shape(self.sides, window, cx));
            })
    }
}

#[derive(IntoElement)]
pub(super) struct ActionIcon {
    pub(super) id: ElementId,
    pub(super) name: &'static str,
    pub(super) disabled: bool,
}
impl RenderOnce for ActionIcon {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state((self.id.clone(), "hover"), cx, |_, _| false);
        let hover = *state.read(cx) && !self.disabled;
        let fill = motion::transition(
            (self.id.clone(), "fill"),
            if hover {
                cx.theme().primary
            } else {
                cx.theme().foreground
            },
            Transition::new(Duration::from_millis(300)).easing(Easing::EaseInOut),
            window,
            cx,
        );
        div()
            .id((self.id, "icon"))
            .size_full()
            .on_hover(window.listener_for(&state, |hovered, next, _, cx| {
                *hovered = *next;
                cx.notify();
            }))
            .child(svg().path(asset(self.name)).size_full().text_color(fill))
    }
}

#[derive(IntoElement)]
pub(super) struct SourceSpinner;
impl RenderOnce for SourceSpinner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = cx.theme().primary;
        if cx.reduce_motion() {
            spinner_frame(0.25, color).into_any_element()
        } else {
            div()
                .size(surface::css(26.))
                .with_animation(
                    "aether-refresh-spinner",
                    Animation::new(Duration::from_secs(2)).repeat(),
                    move |view, phase| view.child(spinner_frame(phase, color)),
                )
                .into_any_element()
        }
    }
}
fn spinner_frame(phase: f32, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            // spinner.ef2d0235.svg: 100-unit viewBox; r=30; stroke=10;
            // two-second linear keyframes: rotate 0/180/720 and dash 10%/50%/10%.
            let scale = f32::from(bounds.size.width) / 100.;
            let radius = 30. * scale;
            let center = bounds.center();
            let rotation = if phase <= 0.5 {
                phase * 360.
            } else {
                180. + (phase - 0.5) * 1080.
            };
            let fraction = if phase <= 0.5 {
                0.1 + phase * 0.8
            } else {
                0.5 - (phase - 0.5) * 0.8
            };
            for (start, length, alpha) in [
                (0., std::f32::consts::TAU, 0.3),
                (rotation.to_radians(), fraction * std::f32::consts::TAU, 1.),
            ] {
                let project =
                    |angle: f32| center + point(px(angle.cos() * radius), px(angle.sin() * radius));
                let mut path = PathBuilder::stroke(px(10. * scale));
                path.move_to(project(start));
                // Two arcs also describe the full background circle without the
                // degenerate same-start/end case in SVG arc geometry.
                path.arc_to(
                    point(px(radius), px(radius)),
                    px(0.),
                    false,
                    true,
                    project(start + length / 2.),
                );
                path.arc_to(
                    point(px(radius), px(radius)),
                    px(0.),
                    false,
                    true,
                    project(start + length),
                );
                if let Ok(path) = path.build() {
                    window.paint_path(path, color.opacity(alpha));
                }
                if alpha == 1. {
                    // The foreground SVG uses square line caps. The exported
                    // stroke builder defaults to butt caps, so add exactly the
                    // half-stroke tangent extension at each end.
                    for (angle, direction) in [(start, -1.), (start + length, 1.)] {
                        let endpoint = project(angle);
                        let normal =
                            point(px(angle.cos() * 5. * scale), px(angle.sin() * 5. * scale));
                        let extension = point(
                            px(-angle.sin() * 5. * scale * direction),
                            px(angle.cos() * 5. * scale * direction),
                        );
                        let mut cap = PathBuilder::fill();
                        cap.move_to(endpoint + normal);
                        cap.line_to(endpoint - normal);
                        cap.line_to(endpoint - normal + extension);
                        cap.line_to(endpoint + normal + extension);
                        cap.close();
                        if let Ok(cap) = cap.build() {
                            window.paint_path(cap, color);
                        }
                    }
                }
            }
        },
    )
    .size(surface::css(26.))
}

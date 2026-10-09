//! Current 9741 checkbox; dimensions and tick keyframes from EditorCanvas CSS.
use super::*;
use gpui_kit::base::{
    Checkbox,
    motion::{Easing, Presence, Transition},
};
use std::time::Duration;

pub(super) fn checkbox(
    id: &'static str,
    checked: bool,
    enabled: bool,
    title: String,
    window: &mut Window,
    cx: &mut App,
) -> Checkbox {
    // tickTop holds height zero through 50%; tickBottom ends at 9px, overriding
    // the checked declaration's 9.6px because the animation uses forwards fill.
    let top = Presence::new((ElementId::from(id), "top"), checked)
        .transition(
            Transition::new(Duration::from_millis(100))
                .delay(Duration::from_millis(100))
                .easing(Easing::Ease),
        )
        .sample(window, cx)
        .progress;
    let bottom = Presence::new((ElementId::from(id), "bottom"), checked)
        .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
    Checkbox::new(id)
        .checked(checked)
        .disabled(!enabled)
        .accessibility_label(title.clone())
        .group(id)
        .flex()
        .items_center()
        .child(
            div()
                .id((ElementId::from(id), "square"))
                .relative()
                .size(surface::css(20.))
                .flex_shrink_0()
                .mr(surface::css(5.))
                .border_1()
                .rounded(surface::css(2.))
                .border_color(if checked {
                    Colors::selected()
                } else {
                    Colors::checkbox_border()
                })
                .bg(if checked {
                    Colors::selected()
                } else {
                    transparent_black().into()
                })
                .when(enabled, |view| {
                    view.group_hover(id, move |style| {
                        let style = style.border_color(if checked {
                            Colors::checkbox_hover()
                        } else {
                            Colors::selected()
                        });
                        if checked {
                            style.bg(Colors::checkbox_hover())
                        } else {
                            style
                        }
                    })
                    .group_active(id, |style| {
                        style
                            .bg(Colors::checkbox_pressed())
                            .border_color(Colors::checkbox_pressed())
                    })
                })
                .when(checked, |view| {
                    view.child(
                        canvas(
                            move |_, _, _| (),
                            move |bounds, _, window, _| {
                                let scale = f32::from(bounds.size.width) / 18.;
                                for (x, y, degrees, height) in [
                                    (8.6_f32, 16.4_f32, -145_f32, 15.4 * top),
                                    (0.6, 10., -50., 9. * bottom),
                                ] {
                                    if height <= 0. {
                                        continue;
                                    }
                                    let angle = degrees.to_radians();
                                    let point_at = |u: f32, v: f32| {
                                        bounds.origin
                                            + point(
                                                px((x + u * angle.cos() - v * angle.sin()) * scale),
                                                px((y + u * angle.sin() + v * angle.cos()) * scale),
                                            )
                                    };
                                    // CSS border-radius is reduced proportionally when its
                                    // diameter exceeds the 3px bar width or animated height.
                                    let radius = 1.5_f32.min(height / 2.);
                                    let k = 0.5522848 * radius;
                                    let mut path = PathBuilder::fill();
                                    path.move_to(point_at(radius, 0.));
                                    path.line_to(point_at(3. - radius, 0.));
                                    path.cubic_bezier_to(
                                        point_at(3., radius),
                                        point_at(3. - radius + k, 0.),
                                        point_at(3., radius - k),
                                    );
                                    path.line_to(point_at(3., height - radius));
                                    path.cubic_bezier_to(
                                        point_at(3. - radius, height),
                                        point_at(3., height - radius + k),
                                        point_at(3. - radius + k, height),
                                    );
                                    path.line_to(point_at(radius, height));
                                    path.cubic_bezier_to(
                                        point_at(0., height - radius),
                                        point_at(radius - k, height),
                                        point_at(0., height - radius + k),
                                    );
                                    path.line_to(point_at(0., radius));
                                    path.cubic_bezier_to(
                                        point_at(radius, 0.),
                                        point_at(0., radius - k),
                                        point_at(radius - k, 0.),
                                    );
                                    path.close();
                                    if let Ok(path) = path.build() {
                                        window.paint_path(path, Colors::region_background());
                                    }
                                }
                            },
                        )
                        .absolute()
                        .inset_0(),
                    )
                }),
        )
        .child(
            div()
                .min_w(surface::css(50.))
                .pt(surface::css(1.))
                .text_size(surface::css(14.))
                .text_color(Colors::text())
                .child(title),
        )
}

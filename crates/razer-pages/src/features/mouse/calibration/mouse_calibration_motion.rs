//! /162 check-circle keyframes, copied from its current CSS geometry/timing.
use super::*;
use gpui_kit::base::Presence;
use gpui_kit::base::motion::{Easing, Transition};
use std::time::Duration;

pub(super) fn card_hover(
    index: usize,
    hovered: bool,
    window: &mut Window,
    cx: &mut Context<MouseProductWorkspace>,
) -> f32 {
    Presence::new(
        (
            ElementId::from(("mouse-calibration-162-hover", index)),
            "top",
        ),
        hovered,
    )
    .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
    .sample(window, cx)
    .progress
}

pub(super) fn hover_color(base: u32, phase: f32) -> Hsla {
    let channel = |shift: u32| {
        let from = ((base >> shift) & 255) as f32;
        let to = ((0x44d62c_u32 >> shift) & 255) as f32;
        (from + (to - from) * phase).round() as u32
    };
    rgb((channel(16) << 16) | (channel(8) << 8) | channel(0)).into()
}

pub(super) fn selected_check(
    product_id: u32,
    index: usize,
    selected: bool,
    window: &mut Window,
    cx: &mut Context<MouseProductWorkspace>,
) -> AnyElement {
    if product_id != 162 {
        return div()
            .size_full()
            .children(
                selected
                    .then(|| img("synapse/mouse-calibration-70/selected-check.svg").size_full()),
            )
            .into_any_element();
    }
    let id = ElementId::from(("mouse-calibration-162-check", index));
    // ticktop holds height zero for its first 50%, then grows over 100 ms.
    let top_phase = Presence::new((id.clone(), "top"), selected)
        .transition(
            Transition::new(Duration::from_millis(if selected { 200 } else { 0 }))
                .easing(Easing::Linear),
        )
        .sample(window, cx)
        .progress;
    let bottom = Presence::new((id.clone(), "bottom"), selected)
        .transition(
            Transition::new(Duration::from_millis(if selected { 100 } else { 0 }))
                .easing(Easing::Ease),
        )
        .sample(window, cx)
        .progress;
    let background = Presence::new((id, "background"), selected)
        .transition(Transition::new(Duration::from_millis(200)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
    let top = Easing::Ease.sample(((top_phase - 0.5) * 2.).clamp(0., 1.));
    div()
        .size_full()
        .rounded(surface::css(10.))
        .bg(rgb(0x44d62c).opacity(background))
        .children(selected.then(|| {
            canvas(
                move |_, _, _| (),
                move |bounds, _, window, _| {
                    let scale = f32::from(bounds.size.width) / 20.;
                    for (x, y, angle, height) in [
                        (9., 16.8, -145_f32, 14.4 * top),
                        (2.4, 11.6, -50., 8. * bottom),
                    ] {
                        if height <= 0. {
                            continue;
                        }
                        let angle = angle.to_radians();
                        let at = |a: f32, b: f32| {
                            bounds.origin
                                + point(
                                    px((x + a * angle.cos() - b * angle.sin()) * scale),
                                    px((y + a * angle.sin() + b * angle.cos()) * scale),
                                )
                        };
                        // border-radius:2.4px clamps to half the 3px width/height.
                        let radius = 1.5_f32.min(height / 2.);
                        let corner = radius * 0.552_284_8;
                        let mut path = PathBuilder::fill();
                        path.move_to(at(radius, 0.));
                        path.line_to(at(3. - radius, 0.));
                        path.cubic_bezier_to(
                            at(3., radius),
                            at(3. - radius + corner, 0.),
                            at(3., radius - corner),
                        );
                        path.line_to(at(3., height - radius));
                        path.cubic_bezier_to(
                            at(3. - radius, height),
                            at(3., height - radius + corner),
                            at(3. - radius + corner, height),
                        );
                        path.line_to(at(radius, height));
                        path.cubic_bezier_to(
                            at(0., height - radius),
                            at(radius - corner, height),
                            at(0., height - radius + corner),
                        );
                        path.line_to(at(0., radius));
                        path.cubic_bezier_to(
                            at(radius, 0.),
                            at(0., radius - corner),
                            at(radius - corner, 0.),
                        );
                        path.close();
                        if let Ok(path) = path.build() {
                            window.paint_path(path, rgb(0x111111));
                        }
                    }
                },
            )
            .size_full()
        }))
        .into_any_element()
}

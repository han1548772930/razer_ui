//! Current 231 `.side-view-svg [data-btn]` presentation and native paths.
//! The 150 ms ease curve belongs to the source CSS; stroke width changes at once.
use crate::keyboard_geometry::KeyboardKeyExt as _;
use gpui_kit::base::motion::{Easing, Interpolate, Transition, transition};
use gpui_kit::{App, Bounds, ElementId, Hsla, PathStyle, Pixels, Rgba, StrokeOptions, Window};
use razer_assets::KeyboardKey;
use razer_widgets::theme::MouseCustomizeColors as Colors;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
pub(super) struct SvgButtonPresentation {
    pub(super) stroke_width: f32,
    pub(super) miter_limit: f32,
}

/// Interpolate premultiplied sRGB channels, including transparent source colors.
/// HSL interpolation would change the intermediate gray/green/orange strokes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SvgColor(Rgba);

impl From<Hsla> for SvgColor {
    fn from(color: Hsla) -> Self {
        Self(color.into())
    }
}

impl Interpolate for SvgColor {
    fn interpolate(&self, target: &Self, progress: f32) -> Self {
        let from = self.0;
        let to = target.0;
        let alpha = from.a + (to.a - from.a) * progress;
        let channel = |a: f32, b: f32| {
            if alpha <= 0. {
                0.
            } else {
                (a * from.a + (b * to.a - a * from.a) * progress) / alpha
            }
        };
        Self(Rgba {
            r: channel(from.r, to.r),
            g: channel(from.g, to.g),
            b: channel(from.b, to.b),
            a: alpha,
        })
    }
}

pub(super) struct SvgButtonPaint {
    fill: Hsla,
    stroke: Hsla,
    opacity: f32,
    stroke_width: f32,
    miter_limit: f32,
}

impl SvgButtonPresentation {
    pub(super) fn sample(
        &self,
        id: ElementId,
        active: bool,
        hovered: bool,
        hyper: bool,
        disabled: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> SvgButtonPaint {
        let color = if hyper {
            Colors::hypershift()
        } else {
            Colors::selected()
        };
        let policy = Transition::new(Duration::from_millis(150)).easing(Easing::Ease);
        let fill = transition(
            (id.clone(), "fill"),
            SvgColor::from(if active { color } else { Colors::transparent() }),
            policy.clone(),
            window,
            cx,
        );
        let fill_opacity = transition(
            (id.clone(), "fill-opacity"),
            if active { 0.55_f32 } else { 0. },
            policy.clone(),
            window,
            cx,
        );
        let stroke = transition(
            (id.clone(), "stroke"),
            SvgColor::from(if active || hovered {
                color
            } else {
                Colors::side_button_line()
            }),
            policy.clone(),
            window,
            cx,
        );
        let opacity = transition(
            (id, "opacity"),
            if disabled { 0.25_f32 } else { 1. },
            policy,
            window,
            cx,
        );
        let mut fill: Hsla = fill.0.into();
        fill.a *= fill_opacity;
        SvgButtonPaint {
            fill,
            stroke: stroke.0.into(),
            opacity,
            stroke_width: if active || hovered { 2. } else { self.stroke_width },
            miter_limit: self.miter_limit,
        }
    }
}

impl SvgButtonPaint {
    pub(super) fn opacity(&self) -> f32 {
        self.opacity
    }

    pub(super) fn paint(&self, key: &KeyboardKey, bounds: Bounds<Pixels>, window: &mut Window) {
        if self.fill.a > 0.
            && let Ok(path) = key.path(bounds, false).build()
        {
            window.paint_path(path, self.fill);
        }
        if self.stroke.a > 0. {
            let scale = f32::from(bounds.size.width) / key.bounds[2];
            // Keep the original SVG miter limit and default butt caps/miter
            // joins. The shared keyboard helper has a different width policy.
            let options = StrokeOptions::default()
                .with_line_width(self.stroke_width * scale)
                .with_miter_limit(self.miter_limit);
            if let Ok(path) = key
                .path(bounds, true)
                .with_style(PathStyle::Stroke(options))
                .build()
            {
                window.paint_path(path, self.stroke);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn transparent_fill_keeps_premultiplied_source_color() {
        let clear = SvgColor::from(Colors::transparent());
        let green = SvgColor::from(Colors::selected());
        let halfway = clear.interpolate(&green, 0.5).0;
        assert!((halfway.a - 0.5).abs() < 1e-6);
        assert!((halfway.r - green.0.r).abs() < 1e-6);
        assert!((halfway.g - green.0.g).abs() < 1e-6);
        assert!((halfway.b - green.0.b).abs() < 1e-6);
        // fill and fill-opacity are independent source transition properties.
        assert!((halfway.a * (0.55 * 0.5) - 0.1375).abs() < 1e-6);
        assert_eq!(green.interpolate(&clear, 1.).0.a, 0.);
    }

    #[test]
    fn stroke_color_interpolates_srgb_instead_of_hue() {
        let normal = SvgColor::from(Colors::side_button_line());
        let green = SvgColor::from(Colors::selected());
        let half = normal.interpolate(&green, 0.5).0;
        assert!((half.r - (normal.0.r + green.0.r) / 2.).abs() < 1e-6);
        assert!((half.g - (normal.0.g + green.0.g) / 2.).abs() < 1e-6);
        assert!((half.b - (normal.0.b + green.0.b) / 2.).abs() < 1e-6);
        assert_eq!(half.a, 1.);
    }
}

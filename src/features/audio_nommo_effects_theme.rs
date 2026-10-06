//! Nommo 1303/1304 main.css `.twoway-lighting`, `.modes-tab`, `.toggle-btn`.
use gpui_kit::base::motion::Interpolate;
use gpui_kit::{Hsla, Rgba, rgb, rgba};

pub(super) struct Colors;
impl Colors {
    pub(super) fn surface() -> Hsla {
        rgb(0x111111).into()
    }
    pub(super) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(super) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(super) fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(super) fn pressed() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub(super) fn pill_pressed() -> Hsla {
        rgba(0x0000001a).into()
    }
    pub(super) fn launch_hover() -> Hsla {
        rgb(0x000000).into()
    }
}

/// CSS interpolates color channels in sRGB, including the green active tab.
#[derive(Clone, PartialEq)]
pub(super) struct CssColor(pub(super) Rgba);
impl Interpolate for CssColor {
    fn interpolate(&self, target: &Self, progress: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * progress;
        Self(Rgba {
            r: mix(self.0.r, target.0.r),
            g: mix(self.0.g, target.0.g),
            b: mix(self.0.b, target.0.b),
            a: mix(self.0.a, target.0.a),
        })
    }
}

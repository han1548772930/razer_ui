//! Current 1383 DisplayWidget, HomeScreenDisplay and CustomizeMedia CSS tokens.
use gpui_kit::base::motion::Interpolate;
use gpui_kit::*;
pub(super) struct Colors;
impl Colors {
    pub(super) fn black() -> Hsla {
        rgb(0).into()
    }
    pub(super) fn surface() -> Hsla {
        rgb(0x222222).into()
    }
    pub(super) fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub(super) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(super) fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub(super) fn white() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(super) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(super) fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(super) fn hovered() -> Hsla {
        rgba(0x44d62c4d).into()
    }
    pub(super) fn disabled() -> Hsla {
        rgb(0x707070).into()
    }
    pub(super) fn overlay() -> Hsla {
        rgba(0x000000cc).into()
    }
    pub(super) fn disabled_visualizer() -> Hsla {
        rgba(0x2cd62c4d).into()
    }
    pub(super) fn radio_border() -> Hsla {
        rgb(0x737373).into()
    }
    pub(super) fn close_hover() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub(super) fn close_pressed() -> Hsla {
        rgba(0x0000001a).into()
    }
    pub(super) fn transparent() -> Hsla {
        rgba(0).into()
    }
}

/// CSS color interpolation preserves premultiplied alpha on transparent ends.
#[derive(Clone, PartialEq)]
pub(super) struct CssColor(pub(super) Rgba);
impl Interpolate for CssColor {
    fn interpolate(&self, target: &Self, progress: f32) -> Self {
        let alpha = self.0.a + (target.0.a - self.0.a) * progress;
        let channel = |a: f32, b: f32| {
            if alpha > 0. {
                (a * self.0.a * (1. - progress) + b * target.0.a * progress) / alpha
            } else {
                0.
            }
        };
        Self(Rgba {
            r: channel(self.0.r, target.0.r),
            g: channel(self.0.g, target.0.g),
            b: channel(self.0.b, target.0.b),
            a: alpha,
        })
    }
}

//! Exact roles from current #multipleBrightness, .widget and .stepper CSS.
use gpui_kit::{Hsla, rgb, rgba};
pub(super) struct Colors;
impl Colors {
    pub(super) fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub(super) fn stepper_hover() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub(super) fn stepper_active() -> Hsla {
        rgba(0x0000001a).into()
    }
}

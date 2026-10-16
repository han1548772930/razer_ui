//! Product-owned tokens from current Macro 21700 and main CSS.
use gpui_kit::{Hsla, rgb, rgba};
pub struct BindingColors;
impl BindingColors {
    pub fn surface() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn card() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn hover() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub fn pressed() -> Hsla {
        rgba(0x0000001a).into()
    }
    pub fn scrim() -> Hsla {
        rgba(0x00000080).into()
    }
    pub fn selected_menu() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn disabled_line() -> Hsla {
        rgba(0xcccccc1a).into()
    }
}

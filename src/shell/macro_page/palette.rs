//! Product-owned tokens from current Macro 21700 and main CSS.
use gpui_kit::{Hsla, rgb, rgba};
pub(super) struct BindingColors;
impl BindingColors {
    pub(super) fn surface() -> Hsla {
        rgb(0x222222).into()
    }
    pub(super) fn card() -> Hsla {
        rgb(0x111111).into()
    }
    pub(super) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(super) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(super) fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub(super) fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(super) fn hover() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub(super) fn pressed() -> Hsla {
        rgba(0x0000001a).into()
    }
    pub(super) fn scrim() -> Hsla {
        rgba(0x00000080).into()
    }
    pub(super) fn selected_menu() -> Hsla {
        rgb(0x000000).into()
    }
    pub(super) fn disabled_line() -> Hsla {
        rgba(0xcccccc1a).into()
    }
}

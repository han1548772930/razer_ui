//! Current 1383 CustomizeSystemInfo / DisplayWidget / tooltip CSS colors.
use gpui_kit::*;
pub(super) struct SystemColors;
impl SystemColors {
    pub(super) fn black() -> Hsla {
        rgb(0).into()
    }
    pub(super) fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub(super) fn surface() -> Hsla {
        rgb(0x222222).into()
    }
    pub(super) fn tag() -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub(super) fn text() -> Hsla {
        rgb(0xcccccc).into()
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
    pub(super) fn delete() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub(super) fn option_hover() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub(super) fn help() -> Hsla {
        rgb(0x6c6c6c).into()
    }
    pub(super) fn help_hover() -> Hsla {
        rgba(0xffffff4d).into()
    }
}

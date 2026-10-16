//! Current 1383 CustomizeAnimation/Image/Emote CSS palette.
use gpui_kit::{Hsla, rgb, rgba};
pub(super) struct Colors;
impl Colors {
    pub(super) fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub(super) fn border() -> Hsla {
        rgb(0x5f5f5f).into()
    }
    pub(super) fn emote_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(super) fn disabled_border() -> Hsla {
        rgba(0x5f5f5f4d).into()
    }
    pub(super) fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(super) fn hover() -> Hsla {
        rgba(0x44d62c4d).into()
    }
    pub(super) fn backdrop() -> Hsla {
        rgba(0x00000080).into()
    }
    pub(super) fn black() -> Hsla {
        rgb(0x000000).into()
    }
    pub(super) fn emote() -> Hsla {
        rgb(0x101010).into()
    }
    pub(super) fn search() -> Hsla {
        rgb(0x111111).into()
    }
    pub(super) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(super) fn white() -> Hsla {
        rgb(0xffffff).into()
    }
}

//! 1383 CustomizeBanner CSS colors; receipts: audio-oled-banner-current-evidence.json.
use gpui_kit::*;
pub(super) struct Colors;
impl Colors {
    pub(super) fn black() -> Hsla {
        rgb(0).into()
    }
    pub(super) fn input() -> Hsla {
        rgb(0x111111).into()
    }
    pub(super) fn selected_surface() -> Hsla {
        rgb(0x292929).into()
    }
    pub(super) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(super) fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub(super) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(super) fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(super) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(super) fn radio_border() -> Hsla {
        rgb(0x737373).into()
    }
}

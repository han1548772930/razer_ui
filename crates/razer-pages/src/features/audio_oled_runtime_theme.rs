//! Current 1383 WarningAlert/ProgressLoading/SimpleLoading CSS tokens.
use gpui_kit::*;
pub(super) struct RuntimeColors;
impl RuntimeColors {
    pub(super) fn backdrop() -> Hsla {
        rgba(0x111111b3).into()
    }
    pub(super) fn body() -> Hsla {
        rgb(0x111111).into()
    }
    pub(super) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(super) fn white() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(super) fn border() -> Hsla {
        rgb(0x707070).into()
    }
    pub(super) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(super) fn green() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(super) fn progress_track() -> Hsla {
        rgb(0x2c5824).into()
    }
    pub(super) fn button() -> Hsla {
        rgb(0x707070).into()
    }
    pub(super) fn confirm_text() -> Hsla {
        rgb(0x222222).into()
    }
    pub(super) fn confirm_border() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub(super) fn shadow() -> Hsla {
        rgba(0x00000033).into()
    }
}

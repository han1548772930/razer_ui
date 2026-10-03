//! Exact roles from current 3946 `.automation-*` CSS; 16px rem reference.
use gpui_kit::{Hsla, rgb, rgba};
pub(super) struct Colors;
impl Colors {
    pub(super) fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub(super) fn row() -> Hsla {
        rgb(0x222222).into()
    }
    pub(super) fn hover() -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub(super) fn pressed() -> Hsla {
        rgb(0x1f1f1f).into()
    }
    pub(super) fn editor() -> Hsla {
        rgb(0x2b2b2b).into()
    }
    pub(super) fn foreground() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(super) fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub(super) fn primary() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(super) fn border() -> Hsla {
        rgb(0x444444).into()
    }
    pub(super) fn divider() -> Hsla {
        rgba(0xffffff14).into()
    }
    pub(super) fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub(super) fn white() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(super) fn danger() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub(super) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
}

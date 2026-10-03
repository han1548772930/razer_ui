//! Current `#multipleBrightness` CSS colors; artwork has its own extracted palette.
use gpui_kit::{Hsla, rgb};
pub(super) struct Colors;
impl Colors {
    pub(super) fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub(super) fn foreground() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(super) fn primary() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(super) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(super) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(super) fn message_border() -> Hsla {
        rgb(0x5a5a5a).into()
    }
    pub(super) fn notice_border() -> Hsla {
        rgb(0x707070).into()
    }
}

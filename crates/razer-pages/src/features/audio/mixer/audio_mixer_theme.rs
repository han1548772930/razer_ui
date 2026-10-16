//! PID1342 current main.07a2c4ed.css and DU canvas palette.
//! CSS SHA256 23582a2533f63335b3a2b40f0849f860e1f728bab81f87cc1546ed9c56703752.
use gpui_kit::{Hsla, rgb, rgba};

pub(super) struct Colors;
impl Colors {
    pub fn surface() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn knob() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn value() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn danger() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub fn preview_warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn black() -> Hsla {
        rgb(0).into()
    }
    pub fn remainder() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn meter_yellow() -> Hsla {
        rgb(0xffff4c).into()
    }
    pub fn meter_red() -> Hsla {
        rgb(0xc8323c).into()
    }
    pub fn transparent() -> Hsla {
        rgba(0x00000000).into()
    }
    pub fn pressed() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub fn hover() -> Hsla {
        rgba(0x0000001a).into()
    }
}

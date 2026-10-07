//! Palette and geometry from current Studio main.8ec0cda4.css.
use gpui_kit::*;

pub(super) struct Colors;
impl Colors {
    pub(super) fn checkbox_border() -> Hsla {
        rgb(0x737373).into()
    }
    pub(super) fn checkbox_hover() -> Hsla {
        rgb(0x7ce26b).into()
    }
    pub(super) fn checkbox_pressed() -> Hsla {
        rgb(0x2f951e).into()
    }
    pub(super) fn spinner_hover() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub(super) fn spinner_pressed() -> Hsla {
        rgba(0x0000001a).into()
    }
    pub(super) fn menu_hover() -> Hsla {
        rgb(0x1a1a1a).into()
    }
    pub(super) fn dropdown_arrow() -> Hsla {
        rgb(0x999999).into()
    }
    pub(super) fn black() -> Hsla {
        rgb(0x000000).into()
    }
    pub(super) fn input_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(super) fn gradient_border() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub(super) fn helper() -> Hsla {
        rgb(0x707070).into()
    }
    pub(super) fn region_background() -> Hsla {
        rgb(0x111111).into()
    }
    pub(super) fn white() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(super) fn help_background() -> Hsla {
        rgb(0x4a4a4a).into()
    }
    pub(super) fn selected_pressed() -> Hsla {
        rgba(0x44d62cb3).into()
    }
    pub(super) fn panel() -> Hsla {
        rgb(0x222222).into()
    }
    pub(super) fn canvas() -> Hsla {
        rgb(0x383838).into()
    }
    pub(super) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(super) fn grid() -> Hsla {
        rgb(0x333333).into()
    }
    pub(super) fn major_grid() -> Hsla {
        rgb(0x4e4e4e).into()
    }
    pub(super) fn center_line() -> Hsla {
        rgb(0x541e1e).into()
    }
    pub(super) fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(super) fn border() -> Hsla {
        rgb(0x383838).into()
    }
    pub(super) fn hover() -> Hsla {
        rgba(0x0000004d).into()
    }
}

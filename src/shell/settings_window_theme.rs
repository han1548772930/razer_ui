//! Current Settings inline SVG styles and toolbar/tooltip CSS color roles.
use gpui_kit::{Hsla, rgb};
pub(super) fn social() -> Hsla {
    rgb(0x999999).into()
}
pub(super) fn accent() -> Hsla {
    rgb(0x44d62c).into()
}
pub(super) fn insider_text() -> Hsla {
    rgb(0x8e8e8e).into()
}
pub(super) fn tooltip_surface() -> Hsla {
    rgb(0x000000).into()
}
pub(super) fn tooltip_border() -> Hsla {
    rgb(0x5d5d5d).into()
}
pub(super) fn tooltip_text() -> Hsla {
    rgb(0xcccccc).into()
}
pub(super) fn toolbar_hover() -> Hsla {
    rgb(0x2d2d2d).into()
}

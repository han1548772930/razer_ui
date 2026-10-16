//! Current Settings inline SVG styles and toolbar/tooltip CSS color roles.
use gpui_kit::{Hsla, rgb};
pub fn tooltip_surface() -> Hsla {
    rgb(0x000000).into()
}
pub fn tooltip_border() -> Hsla {
    rgb(0x5d5d5d).into()
}
pub fn tooltip_text() -> Hsla {
    rgb(0xcccccc).into()
}
pub fn toolbar_hover() -> Hsla {
    rgb(0x2d2d2d).into()
}

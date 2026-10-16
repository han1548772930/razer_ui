//! Current camera `.tooltip_parent .help` palette; see the four CSS receipts in
//! `.analysis/results/frontend/accessories-camera-local-pages-audit.json`.
use gpui_kit::{Hsla, rgb, rgba};

pub(super) fn help() -> Hsla {
    rgb(0x6c6c6c).into()
}

pub(super) fn help_hover() -> Hsla {
    rgba(0xffffff4d).into()
}

// Current 3594/3595/3596 main CSS: camera-container .foot and
// .gradient_slider .track. These colors encode the white-balance range.
pub(super) fn range_label() -> Hsla {
    rgb(0xbdbdbd).into()
}

pub(super) fn white_balance_cool() -> Hsla {
    rgb(0x297fff).into()
}

pub(super) fn white_balance_warm() -> Hsla {
    rgb(0xfd5911).into()
}

// Current camera #video-preview / loading overlay is a black media surface.
pub(super) fn preview_background() -> Hsla {
    rgb(0x000000).into()
}

// Current 3594 .save-settings-container .save-button:hover.
pub(super) fn save_hover() -> Hsla {
    rgba(0x44d62cb3).into()
}

// Current .modal-backdrop.show opacity .7 over black.
pub(super) fn modal_backdrop() -> Hsla {
    rgba(0x000000b3).into()
}

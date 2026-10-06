//! Current camera `.tooltip_parent .help` palette; see the four CSS receipts in
//! `docs/re/camera-presentation-current-evidence.json`.
use gpui_kit::{Hsla, rgb, rgba};

pub(super) fn help() -> Hsla {
    rgb(0x6c6c6c).into()
}

pub(super) fn help_hover() -> Hsla {
    rgba(0xffffff4d).into()
}

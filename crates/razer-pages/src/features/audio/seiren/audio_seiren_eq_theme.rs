//! Product tokens from both current 684 page styles and EQ6007 band palette.
use gpui_kit::{Hsla, rgb, rgba};
/// Shared EQ palette. The microphone and stream-mixer pages read these tokens
/// from sibling modules, so the struct is visible across the audio feature.
pub(crate) struct Colors;
impl Colors {
    pub fn surface() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn inset() -> Hsla {
        rgb(0x1f1f1f).into()
    }
    /// Current UI7405 Zn canvas fill and peaking dots (1422/1446).
    pub fn waveform() -> Hsla {
        rgb(0x1a1a1a).into()
    }
    pub fn waveform_peak() -> Hsla {
        rgb(0xff6464).into()
    }
    pub fn active() -> Hsla {
        rgb(0x292929).into()
    }
    pub fn knob() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn remainder() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn hover_border() -> Hsla {
        rgb(0x8a8a8a).into()
    }
    pub fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn focus() -> Hsla {
        rgb(0x00ff41).into()
    }
    pub fn black() -> Hsla {
        rgb(0).into()
    }
    pub fn grid() -> Hsla {
        rgb(0x333333).into()
    }
    pub fn response() -> Hsla {
        rgb(0x00ff00).into()
    }
    pub fn white() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn thumb() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn thumb_border() -> Hsla {
        rgb(0x2b2b2b).into()
    }
    pub fn track() -> Hsla {
        rgb(0x444444).into()
    }
    /// Current UI5235 and 684 stream-mixer channel, routing and fader styles.
    pub fn stream_green() -> Hsla {
        rgb(0x2cd644).into()
    }
    pub fn stream_yellow() -> Hsla {
        rgb(0xd6c42c).into()
    }
    pub fn stream_red() -> Hsla {
        rgb(0xd64444).into()
    }
    pub fn route_border() -> Hsla {
        rgb(0x737373).into()
    }
    pub fn shortcut_border() -> Hsla {
        rgb(0x515151).into()
    }
    pub fn thumb_hover() -> Hsla {
        rgb(0xaaaaaa).into()
    }
    pub fn thumb_pressed() -> Hsla {
        rgb(0xbbbbbb).into()
    }
    pub fn meter_yellow() -> Hsla {
        rgb(0xffff00).into()
    }
    pub fn meter_red() -> Hsla {
        rgb(0xff0000).into()
    }
    pub fn peak() -> Hsla {
        rgb(0xc8323c).into()
    }
    /// UI7405 recording status/error, current 684 CSS and source inline style.
    pub fn timer() -> Hsla {
        rgb(0x888888).into()
    }
    pub fn recording_error() -> Hsla {
        rgb(0xe57373).into()
    }
    /// Current main CSS profile-del and warning-alert.
    pub fn destructive() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn spectrum_fill() -> Hsla {
        rgba(0x36363680).into()
    }
    pub fn spectrum_line() -> Hsla {
        rgb(0x434343).into()
    }
    pub fn overlay() -> Hsla {
        rgba(0x00000080).into()
    }
    /// Current 684 `.backdrop` and `.modal` / `.intro-button.begin:hover`.
    pub fn wizard_overlay() -> Hsla {
        rgba(0x000000cc).into()
    }
    pub fn wizard_border() -> Hsla {
        rgb(0x515151).into()
    }
    pub fn wizard_primary_hover() -> Hsla {
        rgb(0x6ade57).into()
    }
    pub fn wizard_stop_hover() -> Hsla {
        rgb(0x3d3d3d).into()
    }
    pub fn wizard_stop_pressed() -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub fn band(id: u8) -> Hsla {
        rgb([
            0xff0000, 0xfd8611, 0xffff00, 0x44d62c, 0x47e3ff, 0x007ef5, 0x9747ff, 0xff00dd,
        ][usize::from(id.min(7))])
        .into()
    }
}

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
    /// `.port-add-bend{color:#707070}` and its `.underline:after`
    /// `{background-color:#707070}` rule.
    pub(super) fn add_bend() -> Hsla {
        rgb(0x707070).into()
    }
    /// `.icon-detection:active .detect-b`, `.icon-refreshing:active path` and
    /// `.icon-detection--active:active .detect-a`: the shared pressed color.
    pub(super) fn icon_pressed() -> Hsla {
        rgb(0x39a029).into()
    }
    /// `.icon-detection--active:hover .detect-a`.
    pub(super) fn ring_hover() -> Hsla {
        rgb(0x96ef89).into()
    }
    /// `.port-item:hover .icon-close-glitter:hover path`.
    pub(super) fn remove_hover() -> Hsla {
        rgb(0xc8323c).into()
    }
    /// `.icon-power:hover rect`.
    pub(super) fn power_hover() -> Hsla {
        rgb(0x7de36c).into()
    }
    /// `.icon-power--power-off rect` and its hover color.
    pub(super) fn power_off() -> Hsla {
        rgb(0xc8323c).into()
    }
    pub(super) fn power_off_hover() -> Hsla {
        rgb(0xd97077).into()
    }
    /// `.icon-warning:hover path` and `:active`.
    pub(super) fn warning_hover() -> Hsla {
        rgb(0xfeab59).into()
    }
    pub(super) fn warning_pressed() -> Hsla {
        rgb(0xb15e0c).into()
    }
}

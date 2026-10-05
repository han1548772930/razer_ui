//! The current product `.body-wrapper`, shared by the six family renderers.
//! The audit reads each product's own manifest, CSS and body class receipt.
use crate::ui::surface;
use gpui_kit::*;

pub(super) fn body() -> Div {
    div()
        .w_full()
        .min_w(surface::css(600.))
        .pt(surface::css(10.))
        .px(surface::css(20.))
        .pb(surface::css(20.))
        .bg(rgb(0x222222))
        .font_family("Roboto")
        .font_weight(FontWeight::NORMAL)
        .text_size(surface::css(16.))
        .text_color(rgb(0xcccccc))
}

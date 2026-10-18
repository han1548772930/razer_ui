//! The current product `.body-wrapper`, shared by the six family renderers.
//! The audit reads each product's own manifest, CSS and body class receipt.
use gpui_kit::*;
use razer_widgets::surface;
use razer_widgets::theme::ProductSurfaceColors;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BodyLayout {
    Document,
    SplitViewport,
}

pub(super) fn body() -> Div {
    body_for(BodyLayout::Document)
}

pub(super) fn body_for(layout: BodyLayout) -> Div {
    let body = div()
        .w_full()
        .bg(ProductSurfaceColors::background())
        .font_family("Roboto")
        .font_weight(FontWeight::NORMAL)
        .text_size(surface::css(16.))
        .text_color(ProductSurfaceColors::text());
    match layout {
        BodyLayout::Document => body_inset(body).min_w(surface::css(600.)),
        BodyLayout::SplitViewport => body.flex().flex_col().flex_1().h_full().min_h_0().min_w_0(),
    }
}

/// Source BodyWrapper inset belongs to the scrolled body, beside its drawer.
pub(super) fn body_inset(body: Div) -> Div {
    body.pt(surface::css(10.))
        .px(surface::css(20.))
        .pb(surface::css(20.))
}

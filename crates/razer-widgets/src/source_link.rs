//! Source-styled accessible links shared across host and application pages.
use crate::surface;
use gpui_kit::{component::*, *};

pub fn source_link(
    id: &'static str,
    label: impl Into<SharedString>,
    url: &'static str,
    cx: &App,
) -> gpui_kit::base::Link {
    let label = label.into();
    gpui_kit::base::Link::new(id)
        .href(url)
        .accessibility_label(label.clone())
        .open_with(|url, _, _, cx| cx.open_url(url))
        .cursor_pointer()
        .text_size(surface::css(14.))
        .text_color(cx.theme().foreground)
        .underline()
        .hover(|view| view.text_color(cx.theme().primary))
        .focus_visible(|view| {
            view.bg(cx.theme().secondary_hover)
                .text_color(cx.theme().primary)
        })
        .child(label)
}

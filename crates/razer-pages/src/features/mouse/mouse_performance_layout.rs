//! Current mouse `.body-widgets.flex` / `.widget-col` geometry.
//! Complete source branches and CSS are retained in mouse-main-audit.json.
use super::*;

pub(super) fn widgets() -> Div {
    // Source BodyWidgets is a wrapping row. Direct .widget children retain
    // their own fixed width and auto margins without .widget-col's media rule.
    h_flex()
        .w_full()
        .max_w(surface::css(1240.))
        .mx_auto()
        .items_start()
        .justify_center()
        .flex_wrap()
}

pub(super) fn direct_widget(panel: Div) -> Div {
    panel
        .w(surface::css(600.))
        .min_w(surface::css(600.))
        .max_w(surface::css(600.))
        .mx_auto()
}

pub(super) fn columns(
    product_id: u32,
    page: &str,
    left: impl IntoElement,
    right: impl IntoElement,
) -> PerformanceColumns {
    PerformanceColumns {
        id: SharedString::from(format!("mouse-{product_id}-{page}-columns")),
        left: left.into_any_element(),
        right: right.into_any_element(),
    }
}

#[derive(IntoElement)]
pub(super) struct PerformanceColumns {
    id: SharedString,
    left: AnyElement,
    right: AnyElement,
}

impl RenderOnce for PerformanceColumns {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let compact = surface::stacked_device_columns(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
        );
        let column = |content: AnyElement| {
            // `div.flex > div { flex:auto }` grows each 600px column to
            // 620px in a 1240px body. Fixed cards retain their auto margins.
            // Keep the outer row direction when narrow: the source wraps
            // columns and adds side margins, rather than stretching cards.
            div()
                .w(surface::css(600.))
                .min_w(surface::css(600.))
                .flex_grow(1.)
                .flex_shrink(1.)
                .when(compact, |column| column.mx(surface::css(30.)))
                .child(div().w(surface::css(600.)).mx_auto().child(content))
        };
        widgets()
            .id(self.id)
            .test_support()
            .child(column(self.left))
            .child(column(self.right))
    }
}

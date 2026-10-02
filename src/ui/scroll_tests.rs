use super::SourceScrollable as _;
use gpui_kit::component::{Root, v_flex};
use gpui_kit::test::{TestSupportExt as _, TestWindowExt as _};
use gpui_kit::{
    AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render, ScrollDelta,
    Styled, TestAppContext, Window, div, point, px, size,
};

struct HeightLimitedPanel;
impl Render for HeightLimitedPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                div()
                    .id("fixed-header")
                    .test_support()
                    .h(px(40.))
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .id("limited-scroll")
                    .test_support()
                    .max_h(px(180.))
                    .scrollable_y()
                    .child(
                        v_flex()
                            .h(px(600.))
                            .flex_shrink_0()
                            .child(
                                div()
                                    .id("first-row")
                                    .test_support()
                                    .h(px(50.))
                                    .flex_shrink_0(),
                            )
                            .child(div().h(px(500.)).flex_shrink_0())
                            .child(
                                div()
                                    .id("last-row")
                                    .test_support()
                                    .h(px(50.))
                                    .flex_shrink_0(),
                            ),
                    ),
            )
            .child(
                div()
                    .id("fixed-footer")
                    .test_support()
                    .h(px(40.))
                    .flex_shrink_0(),
            )
    }
}

#[gpui_kit::test]
fn height_limited_content_scrolls_to_bottom_without_moving_the_footer(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let panel = cx.new(|_| HeightLimitedPanel);
        Root::new(panel, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let viewport = window.find("limited-scroll").bounds();
        let footer = window.find("fixed-footer").bounds();
        assert_eq!(viewport.size.height, px(180.));
        assert!(window.find("last-row").bounds().bottom() > viewport.bottom());
        window.scroll(
            "limited-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-900.))),
            cx,
        );
        window.render_frame(cx);
        assert!(window.find("first-row").bounds().top() < viewport.top());
        assert!((window.find("last-row").bounds().bottom() - viewport.bottom()).abs() <= px(1.));
        assert_eq!(window.find("fixed-footer").bounds(), footer);
        window.scroll(
            "limited-scroll",
            ScrollDelta::Pixels(point(px(0.), px(900.))),
            cx,
        );
        window.render_frame(cx);
        assert_eq!(window.find("first-row").bounds().top(), viewport.top());
    })
    .unwrap();
}

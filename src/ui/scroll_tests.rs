use super::SourceScrollable as _;
use gpui_kit::component::{Root, Theme, scroll::ScrollbarMode, v_flex};
use gpui_kit::test::{TestSupportExt as _, TestWindowExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render, ScrollDelta,
    ScrollHandle, Styled, TestAppContext, Window, div, point, px, size,
};

struct HeightLimitedPanel {
    scroll: Option<ScrollHandle>,
}
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
                    .scrollable_both()
                    .when_some(self.scroll.as_ref(), |area, scroll| area.track_scroll(scroll))
                    .child(
                        v_flex()
                            .w(px(800.))
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
        let panel = cx.new(|_| HeightLimitedPanel { scroll: None });
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

#[gpui_kit::test]
fn scrollbar_tracks_stay_at_the_viewport_edges_after_scrolling(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::set_scrollbar_mode(ScrollbarMode::Always, cx);
    });
    let scroll = ScrollHandle::new();
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let panel = cx.new(|_| HeightLimitedPanel {
            scroll: Some(scroll.clone()),
        });
        Root::new(panel, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let viewport = window.find("limited-scroll").bounds();
        let header = window.find("fixed-header").bounds();
        let footer = window.find("fixed-footer").bounds();
        assert_eq!(viewport.top(), header.bottom());
        assert_eq!(viewport.bottom(), footer.top());
        window.scroll(
            "limited-scroll",
            ScrollDelta::Pixels(point(px(-120.), px(-120.))),
            cx,
        );
        assert!(scroll.offset().x < px(0.) && scroll.offset().y < px(0.));

        // These are the fixed viewport edges, not the shifted content edges.
        let before = scroll.offset();
        window.click_at(
            "limited-scroll",
            point(viewport.size.width - px(3.), viewport.size.height * 0.8),
            cx,
        );
        assert!(scroll.offset().y < before.y);
        assert_eq!(scroll.offset().x, before.x);
        let before = scroll.offset();
        window.click_at(
            "limited-scroll",
            point(viewport.size.width * 0.8, viewport.size.height - px(3.)),
            cx,
        );
        assert!(scroll.offset().x < before.x);
        assert_eq!(scroll.offset().y, before.y);

        let before = scroll.offset();
        window.click_at("fixed-header", point(header.size.width - px(3.), px(20.)), cx);
        window.click_at("fixed-footer", point(footer.size.width - px(3.), px(20.)), cx);
        assert_eq!(scroll.offset(), before);
        assert_eq!(window.find("limited-scroll").bounds(), viewport);
        assert_eq!(window.find("fixed-header").bounds(), header);
        assert_eq!(window.find("fixed-footer").bounds(), footer);

        window.scroll(
            "limited-scroll",
            ScrollDelta::Pixels(point(px(2000.), px(2000.))),
            cx,
        );
        assert_eq!(scroll.offset(), point(px(0.), px(0.)));
        let grab = point(viewport.right() - px(3.), viewport.top() + px(8.));
        window.drag(grab, grab + point(px(0.), px(70.)), cx);
        assert!(scroll.offset().y < px(0.));
        assert_eq!(scroll.offset().x, px(0.));
    })
    .unwrap();
}

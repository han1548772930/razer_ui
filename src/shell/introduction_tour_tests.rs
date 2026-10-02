use super::{CloseRequested, IntroductionTour};
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext, Entity, ScrollDelta, TestAppContext, WindowHandle, point, px, size};
use std::{cell::Cell, rc::Rc};

fn open(
    cx: &mut TestAppContext,
    width: f32,
    height: f32,
) -> (Entity<IntroductionTour>, WindowHandle<Root>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut tour = None;
    let handle = cx.open_window(size(px(width), px(height)), |window, cx| {
        let view = cx.new(IntroductionTour::new);
        view.update(cx, |tour, cx| tour.focus(window, cx));
        tour = Some(view.clone());
        Root::new(view, window, cx)
    });
    (tour.unwrap(), handle)
}

#[gpui_kit::test]
fn tutorial_navigation_uses_source_order_and_closes_only_on_skip_or_finish(
    cx: &mut TestAppContext,
) {
    let (tour, handle) = open(cx, 1280., 760.);
    let closed = Rc::new(Cell::new(0));
    let counter = closed.clone();
    let _subscription = cx.update(|cx| {
        cx.subscribe(&tour, move |_, _: &CloseRequested, _| {
            counter.set(counter.get() + 1);
        })
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("introduction-tour-back").disabled(), Some(true));
        assert_eq!(window.find("introduction-tour-next").focused(), Some(true));
        window.press("enter", cx);
        assert_eq!(tour.read(cx).selected_ix, 1);
        window.click("introduction-tour-back", cx);
        assert_eq!(tour.read(cx).selected_ix, 0);
        window.click("introduction-tour-next", cx);
        window.click("introduction-tour-next", cx);
        assert_eq!(tour.read(cx).selected_ix, 2);
        window.click("introduction-tour-skip", cx);
        assert_eq!(closed.get(), 1);
        window.click("introduction-tour-next", cx);
        window.click("introduction-tour-next", cx);
        assert_eq!(tour.read(cx).selected_ix, 4);
        assert_eq!(window.find("introduction-tour-skip").disabled(), Some(true));
        assert_eq!(
            window.find("introduction-tour-next").label(),
            Some(crate::i18n::t("GET_STARTED").to_uppercase().as_str())
        );
        assert_eq!(closed.get(), 1);
        window.click("introduction-tour-next", cx);
        assert_eq!(closed.get(), 2);
    })
    .unwrap();
}

#[gpui_kit::test]
fn short_tour_viewport_scrolls_to_controls_and_restores_the_heading(cx: &mut TestAppContext) {
    let (_, handle) = open(cx, 760., 260.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let viewport = window.find("introduction-tour-scroll").bounds();
        let heading = window.find("introduction-tour-heading").bounds();
        assert!(window.find("introduction-tour-controls").bounds().bottom() > viewport.bottom());
        window.scroll(
            "introduction-tour-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-320.))),
            cx,
        );
        window.render_frame(cx);
        assert!(window.find("introduction-tour-heading").bounds().top() < heading.top());
        assert!(window.find("introduction-tour-controls").bounds().bottom() <= viewport.bottom());
        window.scroll(
            "introduction-tour-scroll",
            ScrollDelta::Pixels(point(px(-1500.), px(0.))),
            cx,
        );
        window.render_frame(cx);
        assert!(window.find("introduction-tour-heading").bounds().left() < heading.left());
        window.scroll(
            "introduction-tour-scroll",
            ScrollDelta::Pixels(point(px(1500.), px(1500.))),
            cx,
        );
        window.render_frame(cx);
        assert_eq!(window.find("introduction-tour-heading").bounds(), heading);
    })
    .unwrap();
}

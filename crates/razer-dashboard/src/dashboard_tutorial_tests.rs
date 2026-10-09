use super::{DashboardTutorial, DashboardTutorialEvent};
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    Subscription, TestAppContext, Window, div, px, size,
};
use razer_widgets::surface;
use std::time::Duration;

struct NavigationFixture {
    tutorial: Entity<DashboardTutorial>,
    completions: usize,
    _subscription: Subscription,
}
impl Render for NavigationFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Host42 + frontend toolbar38. This is the source tutorial's real
        // containing block: a full-height wrapper in the 48px navigation row.
        div().size_full().pt(surface::css(80.)).child(
            div()
                .w_full()
                .h(surface::css(48.))
                .flex()
                .justify_center()
                .child(
                    div()
                        .id("navigation-target")
                        .test_support()
                        .relative()
                        .w(surface::css(100.))
                        .h_full()
                        .child(self.tutorial.clone()),
                ),
        )
    }
}

#[gpui_kit::test]
fn dashboard_marker_tracks_navigation_when_panel_clamps_and_close_dismisses_both(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut owner = None;
    let handle = cx.open_window(size(px(1000.), px(900.)), |window, cx| {
        let tutorial = cx.new(|_| DashboardTutorial::new(false));
        let view = cx.new(|cx: &mut Context<NavigationFixture>| NavigationFixture {
            _subscription: cx.subscribe(&tutorial, |this, _, _: &DashboardTutorialEvent, _| {
                this.completions += 1;
            }),
            tutorial,
            completions: 0,
        });
        owner = Some(view.clone());
        Root::new(view, window, cx)
    });
    let owner = owner.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("dashboard-tutorial-content").is_none());
        assert!(window.try_find("dashboard-tutorial-indicator").is_none());
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(99));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("dashboard-tutorial-indicator").is_none());
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(1));
    for (width, height) in [(1000., 900.), (310., 300.), (700., 600.)] {
        cx.simulate_window_resize(handle.into(), size(px(width), px(height)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let nav = window.find("navigation-target").bounds();
            let indicator = window.find("dashboard-tutorial-indicator").bounds();
            let panel = window.find("dashboard-tutorial-content").bounds();
            assert!((indicator.center().x - nav.left() - px(50.)).abs() < px(0.1));
            assert!((indicator.top() - px(112.)).abs() < px(0.1));
            assert!(panel.left() >= px(0.) && panel.right() <= px(width));
            assert!(panel.top() >= px(0.) && panel.bottom() <= px(height));
            if width == 1000. {
                assert!((panel.left() - nav.left() + px(92.)).abs() < px(0.1));
                assert!((panel.top() - px(150.)).abs() < px(0.1));
            }
        })
        .unwrap();
    }
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(
            window.find("dashboard-tutorial-close").bounds().size,
            size(px(100.), px(27.))
        );
        window.click("dashboard-tutorial-close", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("dashboard-tutorial-indicator").is_none());
        assert!(window.try_find("dashboard-tutorial-content").is_none());
        assert_eq!(owner.read(cx).completions, 1);
        window.press("escape", cx);
        assert_eq!(owner.read(cx).completions, 1);
    })
    .unwrap();
}

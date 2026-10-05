use super::DashboardGroupContent;
use gpui_kit::component::{Root, Theme, v_flex};
use gpui_kit::test::{TestSupportExt as _, TestWindowExt as _};
use gpui_kit::{
    AppContext, Context, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    TestAppContext, Window, div, px, size,
};
use std::time::Duration;

struct Fixture {
    collapsed: bool,
}
impl Render for Fixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w(px(620.))
            .child(
                gpui_kit::base::Button::new("group-toggle")
                    .h(px(30.))
                    .child("Toggle")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.collapsed = !this.collapsed;
                        cx.notify();
                    })),
            )
            .child(DashboardGroupContent::new(
                "group",
                self.collapsed,
                620.,
                4,
                div().id("group-card").test_support().h(px(460.)),
            ))
            .child(div().id("following-group").test_support().h(px(30.)))
    }
}

#[gpui_kit::test]
fn dashboard_collapse_preserves_content_during_motion_and_reverses_smoothly(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let handle = cx.open_window(size(px(640.), px(780.)), |window, cx| {
        let view = cx.new(|_| Fixture { collapsed: false });
        Root::new(view, window, cx)
    });
    let start = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let top = window.find("group-card").bounds().top();
            assert_eq!(window.find("group-content").bounds().size.height, px(460.));
            window.click("group-toggle", cx);
            assert_eq!(window.find("group-card").bounds().top(), top);
            top
        })
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        // max-height is still above natural height while translation is halfway.
        assert_eq!(window.find("group-content").bounds().size.height, px(460.));
        let moved = window.find("group-card").bounds().top();
        assert!((moved - (start - px(230.))).abs() < px(1.));
        window.click("group-toggle", cx);
        assert!((window.find("group-card").bounds().top() - moved).abs() < px(1.));
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!((window.find("group-card").bounds().top() - start).abs() < px(1.));
        window.click("group-toggle", cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("group-content").bounds().size.height, px(0.));
        assert_eq!(window.find("following-group").bounds().top(), start);
        assert!(
            window.try_find("group-card").is_some(),
            "source cards remain mounted"
        );
    })
    .unwrap();
}

use super::{SourceTooltip, SourceTooltipKind};
use gpui_kit::base::Button;
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext, Context, ElementId, InputEvent as _, IntoElement, MouseMoveEvent, ParentElement,
    Render, Styled, TestAppContext, Window, div, point, px, size,
};
use std::time::Duration;

struct TipFixture {
    kind: SourceTooltipKind,
    at_edge: bool,
}

fn popup() -> ElementId {
    (ElementId::from("explanation"), "tip-popup").into()
}

impl Render for TipFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(Button::new("outside").size(px(20.)))
            .child(
                div()
                    .absolute()
                    .left(px(if self.at_edge { 600. } else { 100. }))
                    .top(px(if self.at_edge { 370. } else { 100. }))
                    .size(px(20.))
                    // Tooltip must escape its trigger's scroll/content clip.
                    .overflow_hidden()
                    .child(
                        SourceTooltip::new("explanation", "Information", 210.)
                            .kind(self.kind)
                            .trigger(|_, _, _| {
                                Button::new("explanation").size(px(20.)).into_any_element()
                            }),
                    ),
            )
    }
}

#[gpui_kit::test]
fn portal_anchors_to_icon_without_delay_and_hides_after_its_visibility_transition(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let handle = cx.open_window(size(px(640.), px(400.)), |window, cx| {
        let view = cx.new(|_| TipFixture {
            kind: SourceTooltipKind::DropTips,
            at_edge: false,
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(!window.find(popup()).visible());
        window.hover("explanation", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.find(popup()).visible(),
            "no native tooltip show delay"
        );
        let bounds = window.find(popup()).bounds();
        assert_eq!(bounds.origin, point(px(120.), px(120.)));
        assert_eq!(
            bounds.size.width,
            px(208.),
            "portal width comes from clientWidth"
        );
        window.dispatch_event(
            MouseMoveEvent {
                position: point(px(101.), px(101.)),
                pressed_button: None,
                modifiers: Default::default(),
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
        assert_eq!(
            window.find(popup()).bounds(),
            bounds,
            "moving inside the icon must not move its tip"
        );
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.hover("outside", cx);
        window.render_frame(cx);
        assert!(window.find(popup()).visible());
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(popup()).visible());
        window.hover("explanation", cx);
        window.render_frame(cx);
        assert!(
            window.find(popup()).visible(),
            "reenter must retain the current fade"
        );
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.hover("outside", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(201));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            !window.find(popup()).visible(),
            "visibility ends before the 300ms opacity fade"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn portal_uses_source_left_and_bottom_aligned_fallback_at_window_edges(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
        cx.set_reduce_motion(true);
    });
    let handle = cx.open_window(size(px(640.), px(400.)), |window, cx| {
        let view = cx.new(|_| TipFixture {
            kind: SourceTooltipKind::DropTips,
            at_edge: true,
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.hover("explanation", cx);
        window.render_frame(cx);
        let trigger = window.find("explanation").bounds();
        let tip = window.find(popup());
        assert!(tip.visible());
        assert_eq!(tip.bounds().right(), trigger.left());
        // The source measures clientHeight, then restores height:auto on the
        // visible border box. Thus its bottom is two border pixels below the icon.
        assert_eq!(tip.bounds().bottom(), trigger.bottom() + px(2.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn widget_tip_anchors_below_the_help_control_and_caps_at_300px(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
        cx.set_reduce_motion(true);
    });
    let handle = cx.open_window(size(px(640.), px(400.)), |window, cx| {
        let view = cx.new(|_| TipFixture {
            kind: SourceTooltipKind::WidgetTip,
            at_edge: false,
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.hover("explanation", cx);
        window.render_frame(cx);
        let trigger = window.find("explanation").bounds();
        let tip = window.find(popup());
        assert!(tip.visible());
        // `.widget .tip{right:14px;top:34px}` against a `.widget .help` control
        // that itself sits at `right:10px;top:10px` inside the widget.
        assert_eq!(tip.bounds().right(), trigger.right() - px(4.));
        assert_eq!(tip.bounds().origin.y, trigger.origin.y + px(24.));
        assert!(
            tip.bounds().size.width <= px(300.),
            "width:max-content stays under the 300px cap"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn profile_warning_includes_tip_hover_but_lock_warning_only_includes_icon(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
        cx.set_reduce_motion(true);
    });
    for kind in [
        SourceTooltipKind::ProfileWarning,
        SourceTooltipKind::LockedProfile,
    ] {
        let handle = cx.open_window(size(px(640.), px(400.)), |window, cx| {
            let view = cx.new(|_| TipFixture {
                kind,
                at_edge: false,
            });
            Root::new(view, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.hover("explanation", cx);
            window.render_frame(cx);
            let tip = window.find(popup());
            assert!(tip.visible());
            assert_eq!(
                tip.bounds().origin,
                point(
                    px(if kind == SourceTooltipKind::ProfileWarning {
                        100.
                    } else {
                        120.
                    }),
                    px(120.)
                )
            );
            window.hover(popup(), cx);
            window.render_frame(cx);
            assert_eq!(
                window.find(popup()).visible(),
                kind == SourceTooltipKind::ProfileWarning
            );
            window.hover("outside", cx);
            window.render_frame(cx);
            assert!(!window.find(popup()).visible());
        })
        .unwrap();
    }
}

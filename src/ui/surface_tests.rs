use super::SynapseSwitch;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::{Root, Theme, h_flex, v_flex};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext, Context, Hsla, InputEvent as _, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement, Render, Styled, TestAppContext, Window, div, px,
    size,
};
use std::{cell::Cell, rc::Rc, time::Duration};

struct Switches {
    checked: bool,
    changes: usize,
}

struct CloseControls {
    color: Rc<Cell<Hsla>>,
    calls: Rc<Cell<usize>>,
}
impl Render for CloseControls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let calls = self.calls.clone();
        let mut keymap = super::keymap_close_button("keymap-close", "Close mapping", window, cx)
            .on_click(move |_, _, _| calls.set(calls.get() + 1));
        // Capture the very style applied to this production Button instance;
        // snapshots expose geometry but not color. No duplicate motion model.
        self.color.set(
            keymap
                .style()
                .background
                .as_ref()
                .unwrap()
                .color()
                .unwrap()
                .as_solid()
                .unwrap(),
        );
        h_flex()
            .p(px(20.))
            .gap(px(20.))
            .child(keymap)
            .child(div().id("outside-close").test_support().size(px(40.)))
    }
}

#[gpui_kit::test]
fn keymap_close_preserves_hover_press_timing_and_cancels_release_outside(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let color = Rc::new(Cell::new(Hsla::default()));
    let calls = Rc::new(Cell::new(0));
    let handle = cx.open_window(size(px(400.), px(200.)), |window, cx| {
        let view = cx.new(|_| CloseControls {
            color: color.clone(),
            calls: calls.clone(),
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.hover("keymap-close", cx);
        window.render_frame(cx);
        assert_eq!(color.get().a, 0.);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        // 200ms CSS ease has reached about 80% at its midpoint. White stays
        // white throughout the transparent fade (no dark HSL interpolation).
        assert!(color.get().a > 0.07 && color.get().a < 0.09);
        assert!((color.get().l - 1.).abs() < 0.001);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!((color.get().a - 26. / 255.).abs() < 0.001);
        let position = window.find("keymap-close").bounds().center();
        window.dispatch_event(
            MouseDownEvent {
                button: MouseButton::Left,
                position,
                click_count: 1,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!((color.get().a - 26. / 255.).abs() < 0.001);
        assert_eq!(color.get().l, 0.);
        assert_eq!(calls.get(), 0, "press alone does not close");
        let position = window.find("outside-close").bounds().center();
        window.dispatch_event(
            MouseUpEvent {
                button: MouseButton::Left,
                position,
                click_count: 1,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.hover("outside-close", cx);
        window.render_frame(cx);
        assert_eq!(calls.get(), 0, "releasing outside cancels the close");
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(color.get().a, 0.);
        window.click("keymap-close", cx);
        assert_eq!(calls.get(), 1);
        window.press("space", cx);
        assert_eq!(calls.get(), 2);
    })
    .unwrap();
}

impl Render for Switches {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .child(
                SynapseSwitch::new("first")
                    .label("First")
                    .checked(self.checked)
                    .on_change(cx.listener(|this, checked, _, cx| {
                        this.checked = *checked;
                        this.changes += 1;
                        cx.notify();
                    })),
            )
            .child(SynapseSwitch::new("second").label("Second").checked(true))
    }
}

#[gpui_kit::test]
fn switch_updates_immediately_but_moves_smoothly_and_reverses_without_affecting_its_neighbor(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut switches = None;
    let handle = cx.open_window(size(px(320.), px(200.)), |window, cx| {
        let view = cx.new(|_| Switches {
            checked: false,
            changes: 0,
        });
        switches = Some(view.clone());
        Root::new(view, window, cx)
    });
    let switches = switches.unwrap();
    let (start, neighbor) = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let start = window.within("first").find("switch-handle").bounds();
            let neighbor = window.within("second").find("switch-handle").bounds();
            assert_eq!(neighbor.left() - start.left(), px(14.));
            window.click("first", cx);
            assert!(switches.read(cx).checked);
            assert_eq!(switches.read(cx).changes, 1);
            assert_eq!(window.within("first").find("switch-handle").bounds(), start);
            (start, neighbor)
        })
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let halfway = window.within("first").find("switch-handle").bounds();
        assert!(halfway.left() > start.left() && halfway.left() < start.left() + px(14.));
        assert_eq!(
            window.within("second").find("switch-handle").bounds(),
            neighbor
        );
        window.click("first", cx);
        assert!(!switches.read(cx).checked);
        assert_eq!(switches.read(cx).changes, 2);
        assert!(
            (window.within("first").find("switch-handle").bounds().left() - halfway.left()).abs()
                < px(1.)
        );
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.within("first").find("switch-handle").bounds(), start);
        cx.set_reduce_motion(true);
        window.click("first", cx);
        assert_eq!(
            window.within("first").find("switch-handle").bounds().left(),
            start.left() + px(14.)
        );
        assert_eq!(switches.read(cx).changes, 3);
    })
    .unwrap();
}

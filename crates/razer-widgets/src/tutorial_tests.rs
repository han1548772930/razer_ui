use super::{TutorialIndicator, tutorial_button};
use gpui_kit::component::{Root, Theme, h_flex, v_flex};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext, Context, IntoElement, ParentElement, Render, Styled, TestAppContext, Window, px,
    size,
};
use std::{cell::Cell, rc::Rc, time::Duration};

struct TutorialControls {
    back: Rc<Cell<usize>>,
    next: Rc<Cell<usize>>,
    back_enabled: bool,
    back_opacity: Rc<Cell<f32>>,
}

impl Render for TutorialControls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let back = self.back.clone();
        let next = self.next.clone();
        let mut back_button =
            tutorial_button("back", "Back", false, !self.back_enabled, window, cx).on_click(
                cx.listener(move |this, _, _, cx| {
                    back.set(back.get() + 1);
                    this.back_enabled = false;
                    cx.notify();
                }),
            );
        self.back_opacity.set(back_button.style().opacity.unwrap());
        v_flex().child(TutorialIndicator::new("indicator")).child(
            // Narrower than the two fixed source buttons: they must not
            // shrink silently with the default flex-shrink behavior.
            h_flex().w(px(150.)).child(back_button).child(
                tutorial_button("next", "下一步", true, false, window, cx).on_click(cx.listener(
                    move |this, _, _, cx| {
                        next.set(next.get() + 1);
                        this.back_enabled = true;
                        cx.notify();
                    },
                )),
            ),
        )
    }
}

#[gpui_kit::test]
fn indicator_reproduces_svg_startup_stagger_and_repeats_without_a_solid_disk(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let view = cx.new(|_| TutorialControls {
            back: Rc::new(Cell::new(0)),
            next: Rc::new(Cell::new(0)),
            back_enabled: false,
            back_opacity: Rc::new(Cell::new(0.)),
        });
        Root::new(view, window, cx)
    });
    // Source viewBox50, center diameter10, rendered36. The 2s linear
    // scale/opacity pairs start at (0,1,2)s and restart at (3,4,5)s.
    let samples = [
        (0, [0., 0., 0.], [false, false, false]),
        (500, [9., 0., 0.], [true, false, false]),
        (1500, [27., 9., 0.], [true, true, false]),
        (2500, [36., 27., 9.], [false, true, true]),
        (3500, [9., 36., 27.], [true, false, true]),
        (4500, [27., 9., 36.], [true, true, false]),
    ];
    let mut previous = 0;
    for (millis, diameters, visible) in samples {
        cx.executor()
            .advance_clock(Duration::from_millis(millis - previous));
        previous = millis;
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let center = window.find("tutorial-indicator-center").bounds();
            assert!((center.size.width - px(7.2)).abs() < px(0.1));
            for circle in 0..3 {
                let ring = window
                    .find(["tutorial-pulse-1", "tutorial-pulse-2", "tutorial-pulse-3"][circle]);
                assert!((ring.bounds().size.width - px(diameters[circle])).abs() < px(0.1));
                assert!((ring.bounds().center().x - center.center().x).abs() < px(0.1));
                assert!((ring.bounds().center().y - center.center().y).abs() < px(0.1));
                assert_eq!(
                    ring.visible(),
                    visible[circle],
                    "at {millis}ms, circle {circle}"
                );
            }
        })
        .unwrap();
    }
    cx.update_window(handle.into(), |_, window, cx| {
        cx.set_reduce_motion(true);
        window.render_frame(cx);
        assert!(window.find("tutorial-indicator-center").visible());
        for id in ["tutorial-pulse-1", "tutorial-pulse-2", "tutorial-pulse-3"] {
            assert!(!window.find(id).visible());
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn tutorial_buttons_keep_source_border_box_and_disabled_back_rejects_input(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let back = Rc::new(Cell::new(0));
    let next = Rc::new(Cell::new(0));
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let view = cx.new(|_| TutorialControls {
            back: back.clone(),
            next: next.clone(),
            back_enabled: false,
            back_opacity: Rc::new(Cell::new(0.)),
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        for id in ["back", "next"] {
            assert_eq!(window.find(id).bounds().size, size(px(100.), px(27.)));
        }
        window.click("back", cx);
        assert_eq!(back.get(), 0);
        window.click("next", cx);
        assert_eq!(next.get(), 1);
    })
    .unwrap();
}

#[gpui_kit::test]
fn back_button_fades_for_300ms_but_disabled_interaction_changes_immediately(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let back = Rc::new(Cell::new(0));
    let opacity = Rc::new(Cell::new(0.));
    let handle = cx.open_window(size(px(400.), px(300.)), |window, cx| {
        let view = cx.new(|_| TutorialControls {
            back: back.clone(),
            next: Rc::new(Cell::new(0)),
            back_enabled: false,
            back_opacity: opacity.clone(),
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(opacity.get(), 0.5);
        window.click("back", cx);
        assert_eq!(back.get(), 0);
        window.click("next", cx);
        window.render_frame(cx);
        assert_eq!(opacity.get(), 0.5);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            opacity.get() > 0.89 && opacity.get() < 0.92,
            "300ms CSS ease midpoint"
        );
        window.click("back", cx);
        assert_eq!(back.get(), 1);
        window.click("back", cx);
        assert_eq!(
            back.get(),
            1,
            "disabled already, even during the fade to .5"
        );
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(opacity.get(), 0.5);
        assert_eq!(window.find("back").bounds().size, size(px(100.), px(27.)));
    })
    .unwrap();
}

use super::DeviceWorkspace;
use crate::features::settings::ProfileSettings;
use crate::{model::DeviceCategory, nav::Tab};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Entity, InputEvent as _, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ScrollDelta, TestAppContext, WindowHandle, point, px, size,
};
use std::time::Duration;

fn open_mouse(
    cx: &mut TestAppContext,
    scale: f32,
) -> (WindowHandle<Root>, Entity<DeviceWorkspace>) {
    cx.update(|cx| {
        gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(16. * scale))
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280. * scale), px(1100. * scale)), |window, cx| {
        let mut device = crate::demo::demo_keyboard();
        device.product_id = 182;
        device.category = DeviceCategory::Mouse;
        device.profiles.truncate(1);
        device.profiles[0].settings = Some(ProfileSettings::default());
        let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
        view.update(cx, |view, cx| view.set_page(Tab::Performance, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    (handle, workspace.unwrap())
}

#[gpui_kit::test]
fn dpi_slot_controls_reject_disabled_edits_and_keep_each_axis(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, view) = open_mouse(cx, 1.);
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.hover("sensitivity-slot-2", cx);
        w.click("dpi-slot-2-xy", cx);
        w.click("dpi-slot-2-input-0", cx);
        w.press("secondary-a", cx);
        w.input("101", cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("dpi-slot-2-input-1", cx);
        w.press("secondary-a", cx);
        w.input("1250", cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let edited = cx.update(|cx| {
        let state = &view.read(cx).settings().sensitivity;
        assert_eq!(state.stages[1], [150, 1250]);
        assert!(state.slots[1].independent);
        assert!(!state.slots[0].independent);
        assert_eq!(state.stages[0], [400, 400]);
        state.clone()
    });
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("dpi-stage-2", cx);
        w.click("dpi-slot-2-enabled", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(w.find("dpi-slot-2-input-0").disabled(), Some(true));
        assert_eq!(w.find("dpi-slot-2-input-1").disabled(), Some(true));
        assert_eq!(w.find("dpi-slot-2-xy").disabled(), Some(true));
        w.within("dpi-slot-2-stepper-0").click("increment", cx);
        w.scroll("dpi-slot-2-input-0", ScrollDelta::Lines(point(0., 1.)), cx);
        w.click("dpi-slot-2-xy", cx);
        w.click("dpi-stage-2", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let state = &view.read(cx).settings().sensitivity;
        assert_eq!(state.stages, edited.stages);
        assert!(state.slots[1].independent);
        assert!(!state.slots[1].enabled);
        assert_eq!(state.slots[state.active].id, 3);
    });
    cx.update_window(handle.into(), |_, w, cx| {
        w.hover("sensitivity-slot-2", cx);
        w.click("dpi-slot-2-enabled", cx);
        w.hover("sensitivity-slot-3", cx);
        w.click("dpi-slot-3-enabled", cx);
        w.hover("sensitivity-slot-4", cx);
        w.click("dpi-slot-4-enabled", cx);
        w.hover("sensitivity-slot-5", cx);
        w.click("dpi-slot-5-enabled", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.hover("sensitivity-slot-1", cx);
        assert_eq!(w.find("dpi-slot-1-enabled").disabled(), Some(true));
        w.click("dpi-slot-1-enabled", cx);
        w.hover("sensitivity-slot-2", cx);
        assert_eq!(w.find("dpi-slot-2-enabled").disabled(), Some(true));
        assert_eq!(w.find("dpi-slot-2-input-1").value(), Some("1250"));
        w.click("dpi-slot-2-enabled", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.enabled_count(), 2));
}

#[gpui_kit::test]
fn dpi_stepper_commits_drafts_and_handles_buttons_keys_and_wheel(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, view) = open_mouse(cx, 1.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("dpi-slot-2-stepper-0").bounds().size,
            size(px(62.), px(26.))
        );
        // Merely hovering the numeric field does not take over page scrolling.
        window.scroll("dpi-slot-2-input-0", ScrollDelta::Lines(point(0., 1.)), cx);
    })
    .unwrap();
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.stages[1], [800, 800]));
    cx.update_window(handle.into(), |_, window, cx| {
        window.hover("sensitivity-slot-2", cx);
        window.within("dpi-slot-2-stepper-0").click("increment", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let state = &view.read(cx).settings().sensitivity;
        assert_eq!(state.stages[1], [850, 850]);
        assert_eq!(state.slots[state.active].id, 2);
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("up", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.stages[1], [900, 900]));
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("down", cx);
        window.within("dpi-slot-2-stepper-0").click("decrement", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.stages[1], [800, 800]));
    cx.update_window(handle.into(), |_, window, cx| {
        window.scroll("dpi-slot-2-input-0", ScrollDelta::Lines(point(0., 1.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.stages[1], [850, 850]));
    cx.update_window(handle.into(), |_, window, cx| {
        window.scroll("dpi-slot-2-input-0", ScrollDelta::Lines(point(0., -1.)), cx);
        window.click("dpi-slot-2-input-0", cx);
        window.press("secondary-a", cx);
        window.input("101", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.stages[1], [800, 800]));
    cx.update_window(handle.into(), |_, window, cx| window.press("enter", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.stages[1], [150, 150]));
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("dpi-slot-2-input-0").value(), Some("150"));
        // Enter releases focus, so later wheel events do not change the value.
        window.scroll("dpi-slot-2-input-0", ScrollDelta::Lines(point(0., 1.)), cx);
    })
    .unwrap();
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.stages[1], [150, 150]));
    for (draft, commit, expected) in [("-", "escape", 100), ("999999", "tab", 30000)] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("dpi-slot-2-input-0", cx);
            window.press("secondary-a", cx);
            window.input(draft, cx);
            window.press(commit, cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx).settings().sensitivity.stages[1],
                [expected, expected]
            );
        });
    }
    cx.update_window(handle.into(), |_, window, cx| {
        window.hover("sensitivity-slot-2", cx);
        window.within("dpi-slot-2-stepper-0").click("increment", cx);
        window.press("up", cx);
        window.scroll("dpi-slot-2-input-0", ScrollDelta::Lines(point(0., 1.)), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).settings().sensitivity.stages[1],
            [30000, 30000]
        )
    });
}

#[gpui_kit::test]
fn dpi_pointer_hold_repeats_at_300ms_and_stops_on_release_or_blur(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, view) = open_mouse(cx, 1.);
    cx.update_window(handle.into(), |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    let position = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.hover("sensitivity-slot-2", cx);
            let position = window
                .within("dpi-slot-2-stepper-0")
                .find("increment")
                .bounds()
                .center();
            window.dispatch_event(
                MouseMoveEvent {
                    position,
                    modifiers: Default::default(),
                    pressed_button: None,
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            position
        })
        .unwrap();
    let press = |cx: &mut TestAppContext| {
        cx.update_window(handle.into(), |_, window, cx| {
            window.dispatch_event(
                MouseDownEvent {
                    button: MouseButton::Left,
                    position,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    let release = |cx: &mut TestAppContext| {
        cx.update_window(handle.into(), |_, window, cx| {
            window.dispatch_event(
                MouseUpEvent {
                    button: MouseButton::Left,
                    position,
                    modifiers: Default::default(),
                    click_count: 1,
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
    };
    press(cx);
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.stages[1], [850, 850]));
    for (milliseconds, expected) in [(299, 850), (1, 900), (300, 950)] {
        cx.executor()
            .advance_clock(Duration::from_millis(milliseconds));
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx).settings().sensitivity.stages[1],
                [expected, expected]
            );
        });
    }
    release(cx);
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    // The semantic click following mouse-up must not add another step.
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.stages[1], [950, 950]));
    press(cx);
    cx.update_window(handle.into(), |_, window, cx| window.blur(cx))
        .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    release(cx);
    cx.update(|cx| assert_eq!(view.read(cx).settings().sensitivity.stages[1], [1000, 1000]));
}

#[gpui_kit::test]
fn dpi_reorder_and_resizing_preserve_selection_and_profile_values(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    for scale in [1., 1.25] {
        let (handle, view) = open_mouse(cx, scale);
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            let first = w.find("sensitivity-slot-1").bounds();
            let second = w.find("sensitivity-slot-2").bounds();
            assert_eq!(first.size.height, px(68. * scale));
            assert!((second.top() - first.bottom() - px(4. * scale)).abs() <= px(1.));
            w.hover("sensitivity-slot-3", cx);
            w.click("dpi-slot-3-xy", cx);
            w.click("dpi-slot-3-move", cx);
            w.press("alt-up", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let state = &view.read(cx).settings().sensitivity;
            assert_eq!(
                state.slots.iter().map(|slot| slot.id).collect::<Vec<_>>(),
                [1, 3, 2, 4, 5]
            );
            assert_eq!(state.slots[state.active].id, 3);
            assert_eq!(state.stages[state.active], [1600, 1600]);
            assert!(state.slots[state.active].independent);
        });
        for width in [1279., 700., 1280.] {
            cx.simulate_window_resize(handle.into(), size(px(width * scale), px(1100. * scale)));
            cx.update_window(handle.into(), |_, w, cx| {
                w.render_frame(cx);
                let row = w.find("sensitivity-slot-3").bounds();
                let input = w.find("dpi-slot-3-input-1").bounds();
                let slider = w
                    .within("dpi-slot-3-slider-1")
                    .find("slider-bar-container")
                    .bounds();
                let toggle = w.find("dpi-slot-3-enabled").bounds();
                assert_eq!(row.size.height, px(114. * scale));
                assert_eq!(slider.size.width, px(250. * scale));
                assert!(input.left() >= row.left());
                assert!(toggle.right() <= row.right());
                assert_eq!(w.find("dpi-slot-3-input-1").value(), Some("1600"));
            })
            .unwrap();
        }
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("dpi-stages-visible", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.render_frame(cx);
            assert!(w.try_find("sensitivity-slot-1").is_none());
            assert!(w.try_find("sensitivity-slot-3").is_some());
            assert!(w.try_find("dpi-slot-3-enabled").is_none());
            w.click("dpi-stages-visible", cx);
            w.click("profile-more", cx);
            w.click("profile-duplicate", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.click("dpi-slot-3-input-0", cx);
            w.press("secondary-a", cx);
            w.input("2400", cx);
            w.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.within("profile-select").click("input", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, w, cx| {
            w.press("up", cx);
            w.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let state = view.read(cx);
            let first = &state.settings().sensitivity;
            assert_eq!(state.device().active_profile, "demo-profile-kb");
            assert_eq!(first.stages[first.active], [1600, 1600]);
            let second = &state.device().profiles[1]
                .settings
                .as_ref()
                .unwrap()
                .sensitivity;
            assert_eq!(second.stages[second.active], [2400, 1600]);
            assert_eq!(second.slots[second.active].id, 3);
        });
    }
}

#[gpui_kit::test]
fn polling_buttons_render_and_change_selection_without_duplicate_hover_styles(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, workspace) = open_mouse(cx, 1.);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        for (rate, id) in [
            (500, "polling-500"),
            (1000, "polling-1000"),
            (125, "polling-125"),
        ] {
            window.hover(id, cx);
            window.click(id, cx);
            assert_eq!(workspace.read(cx).settings().polling, rate);
            window.render_frame(cx);
        }
        window.hover("mouse-properties", cx);
    })
    .unwrap();
}

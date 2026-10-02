use super::{
    DashboardCard, DashboardChanged, DashboardGrid, DashboardState, clamp_position, target_index,
};
use crate::{preferences::DashboardPreferences, ui::scroll::SourceScrollable as _};
use gpui_kit::component::{Root, Theme, v_flex};
use gpui_kit::test::{TestSupportExt as _, TestWindowExt as _};
use gpui_kit::{
    App, AppContext, Context, Entity, InputEvent as _, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point,
    Render, ScrollDelta, ScrollHandle, Styled, Subscription, TestAppContext, Window, WindowHandle,
    div, point, px, size,
};
use std::{cell::RefCell, rc::Rc, time::Duration};

struct Fixture {
    state: Entity<DashboardState>,
    calls: Rc<RefCell<Vec<String>>>,
    changed: usize,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}
impl Render for Fixture {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = super::super::MainLayout::new(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
            5,
        )
        .dashboard_width;
        let cards = ["a", "b", "c", "d", "link"]
            .map(|id| {
                if id == "link" {
                    DashboardCard::link(
                        id,
                        "Support",
                        "https://support.razer.com/",
                        div().size_full().child(id),
                    )
                } else {
                    let calls = self.calls.clone();
                    DashboardCard::button(id, id, div().size_full().child(id), move |_, _| {
                        calls.borrow_mut().push(id.into())
                    })
                }
            })
            .into_iter()
            .collect();
        let collapsed = self.state.read(cx).collapsed("devices");
        let toggle = self.state.clone();
        div()
            .size_full()
            .id("dashboard-scroll")
            .test_support()
            .scrollable_y()
            .track_scroll(&self.scroll)
            .child(
                v_flex()
                    .p(px(20.))
                    .w(px(width + 40.))
                    .child(
                        super::super::dashboard_group_toggle(
                            "fixture-devices",
                            "DEVICES_HEADER",
                            collapsed,
                            cx,
                        )
                        .on_click(move |_, _, cx| {
                            toggle.update(cx, |state, cx| state.toggle("devices", cx))
                        }),
                    )
                    .child(super::super::DashboardGroupContent::new(
                        "fixture-devices",
                        collapsed,
                        width,
                        5,
                        DashboardGrid::new("devices", &self.state, width, cards),
                    ))
                    .child(div().h(px(30.)))
                    .child(DashboardGrid::new(
                        "module",
                        &self.state,
                        width,
                        vec![
                            DashboardCard::button("wifi", "Wi-Fi", div(), |_, _| {}),
                            DashboardCard::button("tour", "Tour", div(), |_, _| {}),
                        ],
                    )),
            )
    }
}

#[gpui_kit::test]
fn collapsed_groups_restore_and_persist_independently_from_card_order(cx: &mut TestAppContext) {
    let mut preferences = DashboardPreferences::default();
    preferences.groups_collapsed.insert("devices".into(), true);
    preferences
        .items_order
        .insert("devices".into(), vec!["c".into(), "a".into()]);
    let (view, handle) = open(cx, true, preferences);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("fixture-devices-content").bounds().size.height,
            px(0.)
        );
        assert!(
            !window.find("a").visible(),
            "deferred cards retain the collapse clip"
        );
        let id = (gpui_kit::ElementId::from("fixture-devices"), "toggle");
        window.click(id, cx);
        assert_eq!(
            window.find("fixture-devices-content").bounds().size.height,
            px(460.)
        );
        let state = view.read(cx).state.clone();
        assert!(!state.read(cx).collapsed("devices"));
        assert!(state.read(cx).pending());
        assert_eq!(state.read(cx).snapshot().items_order["devices"], ["c", "a"]);
        let captured = state.read(cx).snapshot();
        state.update(cx, |state, _| state.mark_saved(captured));
        assert!(!state.read(cx).pending());
    })
    .unwrap();
}

fn open(
    cx: &mut TestAppContext,
    reduced: bool,
    preferences: DashboardPreferences,
) -> (Entity<Fixture>, WindowHandle<Root>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(reduced);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut fixture = None;
    let handle = cx.open_window(size(px(1000.), px(900.)), |window, cx| {
        let state = cx.new(|_| DashboardState::new(preferences));
        let view = cx.new(|cx: &mut Context<Fixture>| Fixture {
            _subscriptions: vec![
                cx.observe(&state, |_, _, cx| cx.notify()),
                cx.subscribe(&state, |this, _, _: &DashboardChanged, _| this.changed += 1),
            ],
            state,
            calls: Rc::new(RefCell::new(vec![])),
            changed: 0,
            scroll: ScrollHandle::new(),
        });
        fixture = Some(view.clone());
        Root::new(view, window, cx)
    });
    (fixture.unwrap(), handle)
}
fn down(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        MouseMoveEvent {
            position,
            pressed_button: None,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.dispatch_event(
        MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}
fn move_held(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        MouseMoveEvent {
            position,
            pressed_button: Some(MouseButton::Left),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}
fn up(position: Point<Pixels>, window: &mut Window, cx: &mut App) {
    window.dispatch_event(
        MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
}

#[test]
fn source_grid_uses_card_midpoints_and_clamps_the_incomplete_last_row() {
    assert_eq!(target_index(point(144., 0.), 3, 5), 0);
    assert_eq!(target_index(point(145., 0.), 3, 5), 1);
    assert_eq!(target_index(point(0., 109.), 3, 5), 0);
    assert_eq!(target_index(point(0., 110.), 3, 5), 3);
    assert_eq!(target_index(point(620., 240.), 3, 5), 4);
    assert_eq!(clamp_position(point(-50., -50.), 3, 5), point(0., 0.));
    assert_eq!(clamp_position(point(2000., 2000.), 3, 5), point(620., 240.));
}

#[gpui_kit::test]
fn overlapping_cards_use_source_sort_rank_for_hit_testing_during_motion(cx: &mut TestAppContext) {
    let (view, handle) = open(cx, false, DashboardPreferences::default());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let from = window.find("a").bounds().center();
        window.drag(from, from + point(px(620.), px(240.)), cx);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let origin = window.find("devices-grid").bounds().origin;
        let overlap = origin + point(px(540.), px(100.));
        assert!(window.find("c").bounds().contains(&overlap));
        assert!(window.find("d").bounds().contains(&overlap));
        // c precedes d in the new order and has the higher source z-index,
        // even though d comes later in the original source/element list.
        down(overlap, window, cx);
        up(overlap, window, cx);
        assert_eq!(&*view.read(cx).calls.borrow(), &["c"]);
    })
    .unwrap();
}

#[gpui_kit::test]
fn pointer_click_short_native_drag_and_long_drag_each_have_one_correct_outcome(
    cx: &mut TestAppContext,
) {
    let (view, handle) = open(cx, true, DashboardPreferences::default());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let calls = view.read(cx).calls.clone();
        window.click("a", cx);
        assert_eq!(&*calls.borrow(), &["a"]);
        calls.borrow_mut().clear();
        for distance in [1., 3., 10.] {
            let from = window.find("a").bounds().center();
            window.drag(from, from + point(px(distance), px(0.)), cx);
            assert_eq!(
                &*calls.borrow(),
                &["a"],
                "a {distance}px movement remains a click"
            );
            calls.borrow_mut().clear();
        }
        let from = window.find("a").bounds().center();
        down(from, window, cx);
        move_held(from + point(px(11.), px(0.)), window, cx);
        move_held(from, window, cx);
        up(from, window, cx);
        assert!(
            calls.borrow().is_empty(),
            "crossing 10px stays a drag even after returning"
        );
        assert!(view.read(cx).state.read(cx).press.is_none());
        assert!(!view.read(cx).state.read(cx).pending());
        window.press("enter", cx);
        assert_eq!(
            &*calls.borrow(),
            &["a"],
            "keyboard activation remains available"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn cross_row_reorder_keeps_other_groups_and_late_save_cannot_acknowledge_newer_order(
    cx: &mut TestAppContext,
) {
    let (view, handle) = open(cx, true, DashboardPreferences::default());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let a = window.find("a").bounds();
        let modules = window.find("wifi").bounds();
        window.drag(a.center(), a.center() + point(px(620.), px(240.)), cx);
        let state = view.read(cx).state.clone();
        assert_eq!(
            state.read(cx).snapshot().items_order["devices"],
            ["b", "c", "d", "link", "a"]
        );
        assert_eq!(
            window.find("a").bounds().origin,
            a.origin + point(px(310.), px(240.))
        );
        assert_eq!(window.find("wifi").bounds(), modules);
        assert!(view.read(cx).calls.borrow().is_empty());
        let captured = state.read(cx).snapshot();
        let b = window.find("b").bounds().center();
        window.drag(b, b + point(px(310.), px(0.)), cx);
        state.update(cx, |state, _| state.mark_saved(captured));
        assert!(state.read(cx).pending());
        let latest = state.read(cx).snapshot();
        assert_eq!(latest.items_order["devices"], ["c", "b", "d", "link", "a"]);
        state.update(cx, |state, _| state.mark_saved(latest));
        assert!(!state.read(cx).pending());
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).changed, 2));
}

#[gpui_kit::test]
fn neighboring_cards_and_release_snap_take_300ms_while_dragged_card_tracks_immediately(
    cx: &mut TestAppContext,
) {
    let (view, handle) = open(cx, false, DashboardPreferences::default());
    let (a, b) = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let a = window.find("a").bounds();
            let b = window.find("b").bounds();
            down(a.center(), window, cx);
            move_held(a.center() + point(px(160.), px(0.)), window, cx);
            assert_eq!(window.find("a").bounds().left(), a.left() + px(160.));
            assert_eq!(window.find("b").bounds(), b);
            (a, b)
        })
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let moving = window.find("b").bounds().left();
        assert!(moving > a.left() && moving < b.left());
        up(a.center() + point(px(160.), px(0.)), window, cx);
        assert_eq!(window.find("a").bounds().left(), a.left() + px(160.));
        assert!(view.read(cx).calls.borrow().is_empty());
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("a").bounds().origin, b.origin);
        assert_eq!(window.find("b").bounds().origin, a.origin);
    })
    .unwrap();
}

#[gpui_kit::test]
fn wheel_during_press_moves_card_in_source_direction_without_scrolling_page(
    cx: &mut TestAppContext,
) {
    let (view, handle) = open(cx, true, DashboardPreferences::default());
    cx.simulate_window_resize(handle.into(), size(px(1000.), px(600.)));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let a = window.find("a").bounds();
        down(a.center(), window, cx);
        window.scroll("a", ScrollDelta::Pixels(point(px(0.), px(-240.))), cx);
        assert_eq!(window.find("a").bounds().top(), a.top() + px(240.));
        assert_eq!(view.read(cx).scroll.offset().y, px(0.));
        up(a.center(), window, cx);
        assert!(view.read(cx).calls.borrow().is_empty());
        assert_eq!(
            view.read(cx).state.read(cx).snapshot().items_order["devices"],
            ["b", "c", "d", "a", "link"]
        );
        window.scroll(
            "dashboard-scroll",
            ScrollDelta::Pixels(point(px(0.), px(-120.))),
            cx,
        );
        assert!(
            view.read(cx).scroll.offset().y < px(0.),
            "normal wheel scrolling resumes after release"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn dragging_service_links_never_opens_them_but_clicking_does(cx: &mut TestAppContext) {
    let (_, handle) = open(cx, true, DashboardPreferences::default());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let from = window
            .within("link")
            .find("card-highlight")
            .bounds()
            .center();
        window.drag(from, from - point(px(200.), px(0.)), cx);
    })
    .unwrap();
    assert!(cx.opened_url().is_none());
    cx.update_window(handle.into(), |_, window, cx| {
        window.within("link").click("card-highlight", cx)
    })
    .unwrap();
    assert_eq!(
        cx.opened_url().as_deref(),
        Some("https://support.razer.com/")
    );
}

#[gpui_kit::test]
fn resize_reflows_saved_order_without_changing_the_source_card_dimensions(cx: &mut TestAppContext) {
    let mut preferences = DashboardPreferences::default();
    preferences.items_order.insert(
        "devices".into(),
        ["missing", "c", "a"].map(String::from).to_vec(),
    );
    preferences
        .items_order
        .insert("module".into(), ["tour", "wifi"].map(String::from).to_vec());
    let (view, handle) = open(cx, true, preferences);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("c").bounds().left(), px(20.));
        assert_eq!(window.find("a").bounds().left(), px(330.));
        assert_eq!(window.find("tour").bounds().left(), px(20.));
    })
    .unwrap();
    cx.simulate_window_resize(handle.into(), size(px(660.), px(1000.)));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let origin = window.find("devices-grid").bounds().origin;
        for (id, offset) in [
            ("c", point(0., 0.)),
            ("a", point(310., 0.)),
            ("b", point(0., 240.)),
            ("link", point(0., 480.)),
        ] {
            let bounds = if id == "link" {
                window.within("link").find("card-highlight").bounds()
            } else {
                window.find(id).bounds()
            };
            assert_eq!(bounds.size, size(px(290.), px(220.)));
            assert_eq!(bounds.origin, origin + point(px(offset.x), px(offset.y)));
        }
        assert!(
            !view.read(cx).state.read(cx).pending(),
            "reflow alone is not a layout edit"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn native_drag_cancellation_restores_order_and_never_activates_the_card(cx: &mut TestAppContext) {
    let (view, handle) = open(cx, true, DashboardPreferences::default());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let from = window.find("a").bounds().center();
        down(from, window, cx);
        move_held(from + point(px(310.), px(0.)), window, cx);
        assert!(view.read(cx).state.read(cx).pending());
        // Native lifecycle event, not a direct call into DashboardState.
        assert!(cx.stop_active_drag(window));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(view.read(cx).state.read(cx).press.is_none());
        assert!(!view.read(cx).state.read(cx).pending());
        assert!(view.read(cx).calls.borrow().is_empty());
        assert_eq!(window.find("a").bounds().left(), px(20.));
    })
    .unwrap();
}

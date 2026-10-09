use super::{DeviceKind, open};
use gpui_kit::base::{Button, Popover};
use gpui_kit::component::v_flex;
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext, TestAppContext, point, px, size};
use gpui_kit::{
    Context, DismissEvent, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, ScrollDelta, Styled, Subscription, Window, div,
};
use std::{cell::Cell, rc::Rc, time::Duration};

struct ModalFocus {
    popup: Option<Entity<super::IotPopup>>,
    trigger_focus: FocusHandle,
    dismissals: usize,
    subscription: Option<Subscription>,
}
impl Render for ModalFocus {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                Button::new("open-iot")
                    .track_focus(&self.trigger_focus)
                    .w(px(160.))
                    .h(px(36.))
                    .child("Add Wi-Fi device")
                    .on_click(cx.listener(|this, _, window, cx| {
                        let popup = open(DeviceKind::General, window, cx);
                        this.subscription =
                            Some(cx.subscribe(&popup, |this, _, _: &DismissEvent, cx| {
                                this.dismissals += 1;
                                cx.notify();
                            }));
                        this.popup = Some(popup);
                        cx.notify();
                    })),
            )
            .children(self.popup.clone())
    }
}

#[gpui_kit::test]
fn escape_closes_the_dropdown_before_the_modal_and_restores_the_trigger(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut fixture = None;
    let handle = cx.open_window(size(px(900.), px(700.)), |window, cx| {
        let view = cx.new(|cx| ModalFocus {
            popup: None,
            trigger_focus: cx.focus_handle().tab_stop(true),
            dismissals: 0,
            subscription: None,
        });
        fixture = Some(view.clone());
        Root::new(view, window, cx)
    });
    let fixture = fixture.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("open-iot", cx);
        let popup = fixture.read(cx).popup.clone().unwrap();
        window.press("enter", cx);
        assert!(popup.read(cx).is_open());
        // Cycle farther than the number of controls in either direction.
        for key in ["tab", "shift-tab"] {
            for _ in 0..12 {
                window.press(key, cx);
                assert!(popup.read(cx).focus.contains_focused(window, cx));
            }
        }
        window.within("iot-preview-scene").click("input", cx);
        assert_eq!(window.find("iot-preview-scene").expanded(), Some(true));
        window.press("escape", cx);
        assert_eq!(window.find("iot-preview-scene").expanded(), Some(false));
        assert!(popup.read(cx).is_open());
        assert!(popup.read(cx).scenes.focus_handle(cx).is_focused(window));
        window.press("escape", cx);
        assert!(!popup.read(cx).is_open());
        assert_eq!(window.find("open-iot").focused(), Some(true));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(fixture.read(cx).dismissals, 1));
}

#[gpui_kit::test]
fn outer_host_delays_entrance_and_retains_the_modal_until_removal_deadline(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut fixture = None;
    let handle = cx.open_window(size(px(900.), px(700.)), |window, cx| {
        let view = cx.new(|cx| ModalFocus {
            popup: None,
            trigger_focus: cx.focus_handle().tab_stop(true),
            dismissals: 0,
            subscription: None,
        });
        fixture = Some(view.clone());
        Root::new(view, window, cx)
    });
    let fixture = fixture.unwrap();
    let popup = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("open-iot", cx);
            let popup = fixture.read(cx).popup.clone().unwrap();
            assert!(!popup.read(cx).shown);
            popup
        })
        .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(99));
    cx.update(|cx| assert!(!popup.read(cx).shown));
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(popup.read(cx).shown);
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(150));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("gr-add-close", cx);
        assert!(!popup.read(cx).shown);
        assert!(popup.read(cx).is_open());
        assert!(window.try_find("gamer-room-add-dialog").is_some());
        assert!(popup.read(cx).focus.contains_focused(window, cx));
        // A repeated dismissal must not restart the removal deadline.
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(299));
    cx.update(|cx| {
        assert!(popup.read(cx).is_open());
        assert_eq!(fixture.read(cx).dismissals, 0);
    });
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(!popup.read(cx).is_open());
        assert!(window.try_find("gamer-room-add-dialog").is_none());
        assert_eq!(window.find("open-iot").focused(), Some(true));
        assert_eq!(fixture.read(cx).dismissals, 1);
    })
    .unwrap();
    let reopened = cx
        .update_window(handle.into(), |_, window, cx| {
            window.click("open-iot", cx);
            let reopened = fixture.read(cx).popup.clone().unwrap();
            assert!(!reopened.read(cx).shown);
            window.press("escape", cx);
            reopened
        })
        .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update(|cx| {
        assert!(!reopened.read(cx).shown, "closing cancels delayed entrance");
        assert!(reopened.read(cx).is_open());
    });
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!reopened.read(cx).is_open());
        assert_eq!(fixture.read(cx).dismissals, 2);
    });
}

struct ModalLayers {
    popup: Entity<super::IotPopup>,
    background_clicks: Rc<Cell<usize>>,
    background_scrolls: Rc<Cell<usize>>,
}
impl Render for ModalLayers {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let clicks = self.background_clicks.clone();
        let scrolls = self.background_scrolls.clone();
        v_flex()
            .size_full()
            .child(
                Popover::new("background-popover")
                    .open(true)
                    .overlay_closable(false)
                    .trigger_with(|_, _, _| div().size(px(1.)).into_any_element())
                    .content(move |_, _, _| {
                        let clicks = clicks.clone();
                        let scrolls = scrolls.clone();
                        Button::new("background-popup-button")
                            .w(px(650.))
                            .h(px(500.))
                            .on_click(move |_, _, _| clicks.set(clicks.get() + 1))
                            .on_scroll_wheel(move |_, _, _| scrolls.set(scrolls.get() + 1))
                            .child("Background popup")
                            .into_any_element()
                    }),
            )
            .child(self.popup.clone())
    }
}

#[gpui_kit::test]
fn modal_covers_existing_popups_but_keeps_its_dropdown_clickable(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let clicks = Rc::new(Cell::new(0));
    let scrolls = Rc::new(Cell::new(0));
    let mut popup = None;
    let handle = cx.open_window(size(px(900.), px(700.)), |window, cx| {
        let modal = open(DeviceKind::General, window, cx);
        popup = Some(modal.clone());
        let fixture = cx.new(|_| ModalLayers {
            popup: modal,
            background_clicks: clicks.clone(),
            background_scrolls: scrolls.clone(),
        });
        Root::new(fixture, window, cx)
    });
    let popup = popup.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        // This location is above the modal's 106px top, within its backdrop
        // and within the earlier background popup.
        window.click_at("iot-modal-backdrop", point(px(50.), px(50.)), cx);
        window.scroll(
            "iot-modal-backdrop",
            ScrollDelta::Pixels(point(px(0.), px(-80.))),
            cx,
        );
        assert_eq!(clicks.get(), 0);
        assert_eq!(scrolls.get(), 0);
        assert!(popup.read(cx).is_open());
        window.within("iot-preview-scene").click("input", cx);
        window.click("option-gr-prepare", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("iot-preview-scene").expanded(), Some(false));
        assert!(popup.read(cx).kind == DeviceKind::GamerRoom);
        assert_eq!(clicks.get(), 0);
        window.click("gr-add-close", cx);
        assert!(!popup.read(cx).is_open());
    })
    .unwrap();
}

#[gpui_kit::test]
fn upper_right_close_dismisses_each_wifi_flow_at_small_window_size(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    for kind in [
        DeviceKind::General,
        DeviceKind::GamerRoom,
        DeviceKind::KeyLight,
    ] {
        let mut popup = None;
        let handle = cx.open_window(size(px(700.), px(480.)), |window, cx| {
            let view = open(kind, window, cx);
            popup = Some(view.clone());
            Root::new(view, window, cx)
        });
        let popup = popup.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let panel = window.find("gamer-room-add-dialog").bounds();
            let close = window.find("gr-add-close").bounds();
            assert_eq!(close.size, size(px(36.), px(36.)));
            assert_eq!(close.right(), panel.right());
            assert_eq!(close.top(), panel.top());
            assert!(close.bottom() <= px(480.));
            window.click_at("gr-add-close", point(px(34.), px(2.)), cx);
            assert!(!popup.read(cx).is_open());
            assert!(window.try_find("gamer-room-add-dialog").is_none());
        })
        .unwrap();
    }
}

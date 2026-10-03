use super::{HostTab, HostTabs};
use crate::{nav::Tab, shell::Location};
use gpui_kit::TestAppContext;

use gpui_kit::component::{Root, Theme, h_flex};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Render,
    ScrollDelta, ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Window, div,
    point, px, size,
};
use std::{cell::Cell, rc::Rc};

struct ChromeFixture {
    scroll: ScrollHandle,
    blank_presses: Rc<Cell<usize>>,
    tab_presses: Rc<Cell<usize>>,
    maximized: bool,
    toggles: usize,
}
impl Render for ChromeFixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let blank_presses = self.blank_presses.clone();
        super::titlebar_frame(false)
            .w_full()
            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                blank_presses.set(blank_presses.get() + 1);
            })
            .child(
                h_flex()
                    .id("fixture-tabs")
                    .w(px(200.))
                    .h_full()
                    .items_end()
                    .gap(px(4.))
                    .overflow_x_scroll()
                    .track_scroll(&self.scroll)
                    .children((0..3).map(|i| {
                        let scroll = self.scroll.clone();
                        let tab_presses = self.tab_presses.clone();
                        let id = SharedString::from(format!("fixture-tab-{i}"));
                        super::tab_frame(id.clone(), i == 0)
                            .w(px(140.))
                            .on_scroll_wheel(move |event, window, _| {
                                super::scroll_tabs(&scroll, event, window);
                            })
                            .child(
                                gpui_kit::base::Button::new(id)
                                    .size_full()
                                    .child("Tab")
                                    .on_click(move |_, _, _| {
                                        tab_presses.set(tab_presses.get() + 1);
                                    }),
                            )
                    })),
            )
            .child(div().flex_1())
            .child(super::maximize_button(self.maximized).on_click(cx.listener(
                |this, _, _, cx| {
                    this.maximized = !this.maximized;
                    this.toggles += 1;
                    cx.notify();
                },
            )))
            .child(
                super::window_button("fixture-close", "synapse/host-close.svg", "Close")
                    .on_click(|_, _, _| {}),
            )
    }
}

#[gpui_kit::test]
fn titlebar_blank_space_keeps_native_drag_hitbox_but_tabs_and_controls_occlude_it(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let blank = Rc::new(Cell::new(0));
    let tabs = Rc::new(Cell::new(0));
    let scroll = ScrollHandle::new();
    let handle = cx.open_window(size(px(640.), px(240.)), |window, cx| {
        let view = cx.new(|_| ChromeFixture {
            scroll: scroll.clone(),
            blank_presses: blank.clone(),
            tab_presses: tabs.clone(),
            maximized: false,
            toggles: 0,
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        // This listener uses the SAME hitbox as WindowControlArea::Drag. Native
        // WM_NCHITTEST must see it only in empty space, including above the tabs.
        use gpui_kit::{InputEvent as _, MouseDownEvent, MouseMoveEvent, MouseUpEvent};
        for position in [point(px(10.), px(4.)), point(px(350.), px(20.))] {
            window.dispatch_event(
                MouseMoveEvent {
                    position,
                    pressed_button: None,
                    modifiers: Default::default(),
                }
                .to_platform_input(),
                cx,
            );
            window.dispatch_event(
                MouseDownEvent {
                    position,
                    button: MouseButton::Left,
                    click_count: 1,
                    first_mouse: false,
                    modifiers: Default::default(),
                }
                .to_platform_input(),
                cx,
            );
            window.dispatch_event(
                MouseUpEvent {
                    position,
                    button: MouseButton::Left,
                    click_count: 1,
                    modifiers: Default::default(),
                }
                .to_platform_input(),
                cx,
            );
        }
        assert_eq!(blank.get(), 2);
        window.click("fixture-tab-0", cx);
        assert_eq!(tabs.get(), 1);
        window.click("window-maximize", cx);
        window.click_at("fixture-close", point(px(46.), px(2.)), cx);
        assert_eq!(
            blank.get(),
            2,
            "controls must never initiate an OS window drag"
        );
        // Occluding the native drag area must not break horizontal tab scrolling.
        window.scroll(
            "fixture-tab-0",
            ScrollDelta::Pixels(point(px(0.), px(-80.))),
            cx,
        );
        assert!(scroll.offset().x < px(0.));
        window.scroll(
            "fixture-tab-1",
            ScrollDelta::Pixels(point(px(0.), px(80.))),
            cx,
        );
        assert_eq!(scroll.offset().x, px(0.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn maximize_control_changes_icon_and_accepts_mouse_and_keyboard_once(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut owner = None;
    let handle = cx.open_window(size(px(640.), px(240.)), |window, cx| {
        let view = cx.new(|_| ChromeFixture {
            scroll: ScrollHandle::new(),
            blank_presses: Default::default(),
            tab_presses: Default::default(),
            maximized: false,
            toggles: 0,
        });
        owner = Some(view.clone());
        Root::new(view, window, cx)
    });
    let owner = owner.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("window-maximize").label(), Some("最大化窗口"));
        window.click("window-maximize", cx);
        assert_eq!(owner.read(cx).toggles, 1);
        assert_eq!(window.find("window-maximize").label(), Some("还原窗口"));
        assert_eq!(
            window
                .within("window-maximize")
                .find("window-control-image")
                .bounds()
                .size,
            size(px(13.), px(13.))
        );
        window.press("enter", cx);
        assert_eq!(owner.read(cx).toggles, 2);
        assert!(!owner.read(cx).maximized);
        window.press("space", cx);
        assert_eq!(owner.read(cx).toggles, 3);
        assert!(owner.read(cx).maximized);
        assert_eq!(
            window.find("window-maximize").bounds().size,
            size(px(48.), px(42.))
        );
    })
    .unwrap();
}

struct WindowControls {
    closes: Rc<Cell<usize>>,
}
impl Render for WindowControls {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let closes = self.closes.clone();
        h_flex()
            .w_full()
            .justify_end()
            .child(super::window_button(
                "minimize",
                "synapse/host-minimize.svg",
                "Minimize",
            ))
            .child(super::window_button(
                "maximize",
                "synapse/host-maximize.svg",
                "Maximize",
            ))
            .child(super::window_button(
                "restore",
                "synapse/host-restore.svg",
                "Restore",
            ))
            .child(
                super::window_button("close", "synapse/host-close.svg", "Close")
                    .on_click(move |_, _, _| closes.set(closes.get() + 1)),
            )
    }
}

#[gpui_kit::test]
fn host_controls_keep_intrinsic_svg_sizes_and_full_close_hit_target(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let closes = Rc::new(Cell::new(0));
    let handle = cx.open_window(size(px(640.), px(200.)), |window, cx| {
        let view = cx.new(|_| WindowControls {
            closes: closes.clone(),
        });
        Root::new(view, window, cx)
    });
    for viewport_width in [640., 360.] {
        cx.simulate_window_resize(handle.into(), size(px(viewport_width), px(200.)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            for (id, width, height) in [
                ("minimize", 48., 32.),
                ("maximize", 48., 32.),
                ("restore", 13., 13.),
                ("close", 12.7, 12.7),
            ] {
                let control = window.find(id).bounds();
                let glyph = window.within(id).find("window-control-image").bounds();
                assert_eq!(control.size, size(px(48.), px(42.)));
                assert!((glyph.size.width - px(width)).abs() < px(0.1));
                assert!((glyph.size.height - px(height)).abs() < px(0.1));
                assert!((glyph.center().x - control.center().x).abs() < px(0.1));
                assert!((glyph.center().y - control.center().y).abs() < px(0.1));
            }
            assert_eq!(window.find("close").bounds().right(), px(viewport_width));
            let before = closes.get();
            // A corner far outside the 12.7px icon still activates the control once.
            window.click_at("close", point(px(46.), px(2.)), cx);
            assert_eq!(closes.get(), before + 1);
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn migration_entry_reuses_its_tab_and_can_reopen_after_close(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let mut tabs = HostTabs::new(cx);
        tabs.visit(&Location::Main(Tab::Setting), cx);
        tabs.visit(&Location::ProfileMigration, cx);
        tabs.visit(&Location::ProfileMigration, cx);
        assert_eq!(tabs.order(), ["syn3-profile-migration"]);
        assert!(
            tabs.remove(&HostTab::ProfileMigration, &Location::ProfileMigration)
                == Some(Location::Main(Tab::Setting))
        );
        assert_eq!(tabs.closed, [HostTab::ProfileMigration]);
        tabs.visit(&Location::ProfileMigration, cx);
        assert_eq!(tabs.order(), ["syn3-profile-migration"]);
        assert!(tabs.closed.is_empty());
    });
}

#[gpui_kit::test]
fn closing_tabs_selects_neighbors_and_reopening_preserves_order(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let mut tabs = HostTabs::new(cx);
        let first = HostTab::Device("first".into());
        let second = HostTab::Device("second".into());
        tabs.visit(&Location::Main(Tab::Setting), cx);
        tabs.open(first.clone(), cx);
        tabs.open(second.clone(), cx);
        tabs.open(HostTab::Tour(crate::shell::TourKind::Synapse), cx);
        // Inactive close preserves the active tab; reopening appends exactly once.
        assert!(tabs.remove(&first, &second.location()).is_none());
        tabs.visit(&first.location(), cx);
        tabs.visit(&first.location(), cx);
        assert_eq!(
            tabs.open.iter().map(|entry| &entry.tab).collect::<Vec<_>>(),
            vec![
                &second,
                &HostTab::Tour(crate::shell::TourKind::Synapse),
                &first
            ]
        );
        // Active middle closes toward the right; active last closes toward the left.
        assert!(
            tabs.remove(
                &HostTab::Tour(crate::shell::TourKind::Synapse),
                &Location::Tour(crate::shell::TourKind::Synapse)
            ) == Some(first.location())
        );
        assert!(tabs.remove(&first, &first.location()) == Some(second.location()));
        assert!(tabs.remove(&second, &second.location()) == Some(Location::Main(Tab::Setting)));
        // Closing an already closed tab cannot change the undo stack.
        assert!(
            tabs.remove(&second, &Location::Main(Tab::Setting))
                .is_none()
        );
        assert_eq!(
            tabs.closed,
            vec![
                HostTab::Tour(crate::shell::TourKind::Synapse),
                first,
                second
            ]
        );
    });
}

#[gpui_kit::test]
fn synapse_and_chroma_tours_keep_distinct_tabs_and_close_independently(cx: &mut TestAppContext) {
    cx.update(|cx| {
        let mut tabs = HostTabs::new(cx);
        let synapse = HostTab::Tour(crate::shell::TourKind::Synapse);
        let chroma = HostTab::Tour(crate::shell::TourKind::Chroma);
        tabs.visit(&Location::Main(Tab::Setting), cx);
        tabs.visit(&synapse.location(), cx);
        tabs.visit(&chroma.location(), cx);
        tabs.visit(&synapse.location(), cx);
        assert_eq!(tabs.order(), ["host-tour", "host-chroma-tour"]);
        assert!(tabs.remove(&chroma, &synapse.location()).is_none());
        assert_eq!(tabs.order(), ["host-tour"]);
        tabs.visit(&chroma.location(), cx);
        assert_eq!(tabs.order(), ["host-tour", "host-chroma-tour"]);
        assert!(tabs.remove(&synapse, &synapse.location()) == Some(chroma.location()));
        assert!(tabs.remove(&chroma, &chroma.location()) == Some(Location::Main(Tab::Setting)));
    });
}

#[gpui_kit::test]
fn restored_positions_ignore_missing_tabs_and_save_completion_keeps_newer_order_pending(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        let mut tabs = HostTabs::new(cx);
        let first = HostTab::Device("first".into());
        let second = HostTab::Device("second".into());
        tabs.open(first.clone(), cx);
        tabs.open(second.clone(), cx);
        tabs.open(HostTab::Tour(crate::shell::TourKind::Synapse), cx);
        tabs.restore_order(&[
            "host-missing".into(),
            "host-second".into(),
            "host-first".into(),
        ]);
        assert_eq!(tabs.order(), ["host-second", "host-first", "host-tour"]);
        assert!(!tabs.order_pending());
        tabs.open.swap(0, 1);
        tabs.order_changed = true;
        let captured = tabs.order();
        tabs.open.swap(1, 2);
        tabs.mark_order_saved(captured);
        assert!(tabs.order_pending());
        tabs.mark_order_saved(tabs.order());
        assert!(!tabs.order_pending());
        tabs.remove(
            &HostTab::Tour(crate::shell::TourKind::Synapse),
            &first.location(),
        );
        assert!(
            !tabs.order_pending(),
            "closing a tab alone is not an unsaved layout edit"
        );
    });
}

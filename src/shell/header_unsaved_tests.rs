use super::unsaved_profiles;
use gpui_kit::component::{Root, h_flex};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Context, IntoElement, ParentElement, Render, Styled, TestAppContext, Window, px,
    size,
};
use std::time::Duration;
use std::{cell::Cell, rc::Rc};

struct HeaderFixture {
    items: Vec<(String, String)>,
    saving: bool,
    saves: Rc<Cell<usize>>,
    discards: Rc<Cell<usize>>,
}

impl Render for HeaderFixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let saves = self.saves.clone();
        let discards = self.discards.clone();
        h_flex().w_full().justify_end().child(unsaved_profiles(
            self.items.clone(),
            self.saving,
            move |_, _| saves.set(saves.get() + 1),
            move |_, _| discards.set(discards.get() + 1),
            cx,
        ))
    }
}

#[gpui_kit::test]
fn unsaved_header_requires_profiles_and_dismisses_without_saving_from_a_row(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let saves = Rc::new(Cell::new(0));
    let discards = Rc::new(Cell::new(0));
    let mut fixture = None;
    let handle = cx.open_window(size(px(800.), px(600.)), |window, cx| {
        let view = cx.new(|_| HeaderFixture {
            items: vec![],
            saving: false,
            saves: saves.clone(),
            discards: discards.clone(),
        });
        fixture = Some(view.clone());
        Root::new(view, window, cx)
    });
    let fixture = fixture.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("header-unsaved-trigger").is_none());
        fixture.update(cx, |fixture, cx| {
            fixture.items.push(("mouse".into(), "Razer Mouse".into()));
            cx.notify();
        });
        window.render_frame(cx);
        window.click("header-unsaved-trigger", cx);
        assert!(window.try_find("header-unsaved-menu").is_some());
        window.click("header-unsaved-item-mouse", cx);
        assert!(window.try_find("header-unsaved-menu").is_none());
        assert_eq!(saves.get(), 0);
        assert_eq!(discards.get(), 0);
        window.click("header-unsaved-trigger", cx);
        window.press("escape", cx);
        assert!(window.try_find("header-unsaved-menu").is_none());
        window.click("header-unsaved-trigger", cx);
        window.click("header-unsaved-save", cx);
        assert_eq!(saves.get(), 1);
        window.click("header-unsaved-trigger", cx);
        window.click("header-unsaved-discard", cx);
        assert_eq!(discards.get(), 1);
        fixture.update(cx, |fixture, cx| {
            fixture.saving = true;
            cx.notify();
        });
        window.click("header-unsaved-trigger", cx);
        assert_eq!(window.find("header-unsaved-save").disabled(), Some(true));
        assert_eq!(window.find("header-unsaved-discard").disabled(), Some(true));
        window.press("escape", cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn unsaved_dropdown_delays_show_and_removes_inert_content_after_hide(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(false);
        gpui_kit::component::Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let handle = cx.open_window(size(px(800.), px(600.)), |window, cx| {
        let view = cx.new(|_| HeaderFixture {
            items: vec![("mouse".into(), "Razer Mouse".into())],
            saving: false,
            saves: Rc::new(Cell::new(0)),
            discards: Rc::new(Cell::new(0)),
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("header-unsaved-trigger", cx);
        assert!(window.try_find("header-unsaved-menu").is_some());
        assert!(window.try_find("header-unsaved-save").is_none());
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    let initial_top = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("header-unsaved-save").is_some());
            window.find("header-unsaved-menu").bounds().top()
        })
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            (window.find("header-unsaved-menu").bounds().top() - initial_top - px(7.)).abs()
                <= px(1.)
        );
        window.press("escape", cx);
        window.render_frame(cx);
        assert!(window.try_find("header-unsaved-menu").is_some());
        assert!(window.try_find("header-unsaved-save").is_none());
        assert_eq!(window.find("header-unsaved-trigger").focused(), Some(true));
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("header-unsaved-menu").is_none());
    })
    .unwrap();
}

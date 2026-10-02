use super::AccountMenu;
use gpui_kit::component::{Root, Theme, h_flex};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, TestAppContext,
    Window, div, px, size,
};
use std::time::Duration;

struct MenuFixture {
    menu: Entity<AccountMenu>,
    page_clicks: usize,
}
impl Render for MenuFixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(260.))
            .child(h_flex().justify_end().child(self.menu.clone()))
            .child(
                gpui_kit::base::Button::new("under-menu")
                    .w(px(223.))
                    .h(px(60.))
                    .child("Underlying page")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page_clicks += 1;
                        cx.notify();
                    })),
            )
    }
}

#[gpui_kit::test]
fn account_hidden_phase_does_not_intercept_page_clicks(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut fixture = None;
    let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
        let menu = cx.new(|cx| AccountMenu::new(window, cx));
        let view = cx.new(|_| MenuFixture {
            menu,
            page_clicks: 0,
        });
        fixture = Some(view.clone());
        Root::new(view, window, cx)
    });
    let fixture = fixture.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("account-menu-trigger", cx);
        window.click("under-menu", cx);
        assert_eq!(fixture.read(cx).page_clicks, 1);
        assert!(!fixture.read(cx).menu.read(cx).popup.read(cx).is_open());
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("account-menu").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn account_delay_escape_and_reopen_preserve_focus_and_cancel_old_timers(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16.)));
    let mut menu = None;
    let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
        let account = cx.new(|cx| AccountMenu::new(window, cx));
        menu = Some(account.clone());
        let view = cx.new(|_| MenuFixture {
            menu: account,
            page_clicks: 0,
        });
        Root::new(view, window, cx)
    });
    let menu = menu.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("account-menu-trigger", cx);
        assert!(!menu.read(cx).shown);
        // Escape also works during the transparent 100ms mounting phase.
        window.press("escape", cx);
        window.render_frame(cx);
        assert!(!menu.read(cx).popup.read(cx).is_open());
        assert_eq!(window.find("account-menu-trigger").focused(), Some(true));
        window.click("account-menu-trigger", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    let initial_top = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(menu.read(cx).shown);
            window.find("account-menu").bounds().top()
        })
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            (window.find("account-menu").bounds().top() - initial_top - px(7.)).abs() <= px(1.)
        );
        // A second click on the trigger closes instead of re-opening on mouse-up.
        window.click("account-menu-trigger", cx);
        assert!(!menu.read(cx).popup.read(cx).is_open());
        assert!(menu.read(cx).rendered);
        assert_eq!(window.find("account-menu-trigger").focused(), Some(true));
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("account-menu").is_none());
        assert!(!menu.read(cx).rendered);
    })
    .unwrap();
}

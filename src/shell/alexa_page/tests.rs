//! Interaction regressions; compile only under the current no-execution constraint.
use super::{Account, AlexaPage, InstallState, Page};
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext, Entity, TestAppContext, WindowHandle, px, size};
use std::time::Duration;

fn open(
    cx: &mut TestAppContext,
    scene: &str,
    reduced: bool,
) -> (Entity<AlexaPage>, WindowHandle<Root>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(reduced);
        Theme::update(cx, |theme| theme.font_size = px(16.));
    });
    let mut page = None;
    let handle = cx.open_window(size(px(1100.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| {
            let mut view = AlexaPage::new(window, cx);
            view.choose_scene(scene, window, cx);
            view
        });
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    (page.unwrap(), handle)
}

#[gpui_kit::test]
fn unknown_and_guest_scenes_do_not_grant_authenticated_navigation(cx: &mut TestAppContext) {
    let (page, handle) = open(cx, "unknown", true);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(page.read(cx).account, Account::Unknown);
        assert!(window.try_find("alexa-install-action").is_none());
        assert!(window.try_find("alexa-skills").is_none());
        page.update(cx, |page, cx| page.choose_scene("guest", window, cx));
        window.render_frame(cx);
        assert!(window.try_find("alexa-skills").is_none());
        assert!(window.try_find("alexa-settings").is_none());
        window.click("alexa-razer-login", cx);
        assert_eq!(page.read(cx).account, Account::Guest);
        assert!(!page.read(cx).notice.is_empty());
        window.click("alexa-help", cx);
        assert_eq!(page.read(cx).page, Page::Help);
        assert!(page.read(cx).has_previous_page());
    })
    .unwrap();
}

#[gpui_kit::test]
fn skills_keep_one_expansion_and_remain_usable_when_the_skill_switch_is_off(
    cx: &mut TestAppContext,
) {
    let (page, handle) = open(cx, "ready", true);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("alexa-skills", cx);
        window.click("basic-lighting", cx);
        assert_eq!(page.read(cx).expanded, Some("basic-lighting"));
        window.click("chroma-lighting", cx);
        assert_eq!(page.read(cx).expanded, Some("chroma-lighting"));
        window.click("alexa-synapse-skills", cx);
        assert!(!page.read(cx).synapse_skills);
        window.click("launch-application", cx);
        assert_eq!(page.read(cx).expanded, Some("launch-application"));
        window.click("launch-application", cx);
        assert_eq!(page.read(cx).expanded, None);
        assert!(window.try_find("power").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn disabled_settings_reject_changes_and_logout_cancel_restores_focus(cx: &mut TestAppContext) {
    let (page, handle) = open(cx, "ready", true);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("alexa-settings", cx);
        window.click("alexa-shortcut", cx);
        window.press("q", cx);
        assert_eq!(page.read(cx).shortcut, "Ctrl + Shift + Q");
        window.click("alexa-enabled", cx);
        assert!(!page.read(cx).enabled);
        window.click("alexa-sounds", cx);
        assert!(page.read(cx).sounds);
        assert_eq!(window.find("alexa-shortcut").disabled(), Some(true));
        window.click("alexa-logout", cx);
        assert!(page.read(cx).modal.is_some());
        for _ in 0..7 {
            window.press("tab", cx);
            assert!(page.read(cx).modal_focus.contains_focused(window, cx));
        }
        window.click("alexa-logout-cancel", cx);
        assert!(page.read(cx).modal.is_none());
        assert_eq!(page.read(cx).account, Account::Ready);
        assert_eq!(window.find("alexa-logout").focused(), Some(true));
        window.click("alexa-logout", cx);
        window.click("alexa-logout-confirm", cx);
        assert_eq!(page.read(cx).account, Account::Razer);
        assert_eq!(page.read(cx).page, Page::Home);
        assert!(!page.read(cx).has_previous_page());
        assert!(!page.read(cx).has_next_page());
        assert!(window.try_find("alexa-settings").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn installation_preview_cancels_waiting_but_cannot_cancel_saving(cx: &mut TestAppContext) {
    let (page, handle) = open(cx, "available", true);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("alexa-install-action", cx);
        assert_eq!(page.read(cx).installer, Some(InstallState::Waiting));
        window.click("alexa-install-action", cx);
        assert_eq!(page.read(cx).installer, Some(InstallState::Available));
        page.update(cx, |page, cx| page.choose_scene("saving", window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("alexa-install-action").disabled(), Some(true));
        window.click("alexa-install-action", cx);
        assert_eq!(page.read(cx).installer, Some(InstallState::Saving));
    })
    .unwrap();
}

#[gpui_kit::test]
fn escape_cancels_patch_entrance_and_retains_its_modal_until_the_source_deadline(
    cx: &mut TestAppContext,
) {
    let (page, handle) = open(cx, "notes", false);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(page.read(cx).modal.is_some());
        window.press("escape", cx);
        assert!(page.read(cx).modal.is_some());
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(299));
    cx.run_until_parked();
    cx.update(|cx| assert!(page.read(cx).modal.is_some()));
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update(|cx| assert!(page.read(cx).modal.is_none()));
}

use super::{super::runtime_page::RuntimePanel, SettingsPage};
use crate::preferences::AppPreferences;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Entity, TestAppContext, WindowHandle, px, size};

fn open(
    cx: &mut TestAppContext,
    mut preferences: AppPreferences,
) -> (Entity<SettingsPage>, WindowHandle<Root>) {
    cx.update(gpui_kit::init);
    preferences.language = crate::preferences::LANGUAGES
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(&crate::i18n::locale()))
        .map(|(code, _)| (*code).to_string())
        .unwrap_or_else(|| "en".into());
    let mut page = None;
    let window = cx.open_window(size(px(1500.), px(1600.)), |window, cx| {
        let runtime = cx.new(|_| RuntimePanel::new());
        let entity = cx.new(|cx| SettingsPage::new(preferences, runtime, window, cx));
        page = Some(entity.clone());
        Root::new(entity, window, cx)
    });
    (page.unwrap(), window)
}

#[gpui_kit::test]
fn recommendation_reset_clears_products_and_ownership_but_preserves_categories(
    cx: &mut TestAppContext,
) {
    let preferences = AppPreferences {
        ignored_categories: vec!["mouse".into()],
        ignored_products: vec!["182".into()],
        owned_products: vec!["653".into()],
        ..Default::default()
    };
    let (page, handle) = open(cx, preferences);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-reset-categories", cx);
        let values = page.read(cx).snapshot();
        assert!(values.ignored_products.is_empty());
        assert!(values.owned_products.is_empty());
        assert_eq!(values.ignored_categories, ["mouse"]);
        window.click("settings-recommendations", cx);
        assert_eq!(
            window.find("settings-category-mouse").disabled(),
            Some(true)
        );
        window.click("settings-recommendations", cx);
        assert_eq!(page.read(cx).snapshot().ignored_categories, ["mouse"]);
        window.click("settings-discard", cx);
        assert_eq!(page.read(cx).snapshot().owned_products, ["653"]);
        assert_eq!(page.read(cx).snapshot().ignored_products, ["182"]);
    })
    .unwrap();
}

#[gpui_kit::test]
fn tutorial_persistence_and_discard_leave_unrelated_drafts_independent(cx: &mut TestAppContext) {
    let (page, handle) = open(cx, AppPreferences::default());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-notifications", cx);
        page.update(cx, |page, cx| page.tutorial_seen(true, cx));
        page.update(cx, |page, cx| page.dashboard_tutorial_seen(true, cx));
        let tutorial = page.read(cx).tutorial_snapshot();
        assert!(
            tutorial.notifications,
            "tutorial writes use saved preferences"
        );
        assert!(tutorial.gamer_room_tutorial_seen);
        assert!(tutorial.dashboard_tutorial_seen);
        page.update(cx, |page, cx| page.mark_tutorial_saved(false, false, cx));
        assert!(
            page.read(cx).tutorial_pending(),
            "an older completion must not erase a newer dismissal"
        );
        window.click("settings-discard", cx);
        assert!(page.read(cx).snapshot().notifications);
        assert!(page.read(cx).snapshot().gamer_room_tutorial_seen);
        assert!(page.read(cx).snapshot().dashboard_tutorial_seen);
        page.update(cx, |page, cx| page.mark_tutorial_saved(true, false, cx));
        assert!(
            page.read(cx).tutorial_pending(),
            "the Dashboard flag has its own captured revision"
        );
        page.update(cx, |page, cx| page.mark_tutorial_saved(true, true, cx));
        assert!(!page.read(cx).dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn delayed_save_preserves_newer_edits_and_busy_buttons_reject_repeat_submission(
    cx: &mut TestAppContext,
) {
    let (page, handle) = open(cx, AppPreferences::default());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-notifications", cx);
        let captured = page.read(cx).snapshot();
        page.update(cx, |page, cx| page.set_persistence_state(true, None, cx));
        window.render_frame(cx);
        assert_eq!(window.find("settings-save").disabled(), Some(true));
        assert_eq!(window.find("settings-discard").disabled(), Some(true));
        window.click("settings-notifications", cx);
        page.update(cx, |page, cx| {
            page.mark_saved(captured, cx);
            page.set_persistence_state(false, None, cx);
        });
        assert!(page.read(cx).dirty());
        window.click("settings-discard", cx);
        assert!(!page.read(cx).snapshot().notifications);
        assert!(!page.read(cx).dirty());
    })
    .unwrap();
}

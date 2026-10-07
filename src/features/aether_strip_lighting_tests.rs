//! Compiled by `cargo check --all-targets`; not executed under the source-audit policy.
use super::AetherLightingPage;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};

#[gpui_kit::test]
fn lighting_preview_keeps_source_controls_in_the_tab_body(cx: &mut TestAppContext) {
    cx.update(|cx| gpui_kit::init(cx));
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let page = cx.new(|cx| AetherLightingPage::new(window, cx));
        Root::new(page, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        for id in [
            "aether-lighting-page",
            "aether-lighting-override",
            "aether-lighting-override-switch",
            "aether-lighting-brightness",
            "aether-lighting-brightness-control",
            "aether-lighting-effects",
            "aether-lighting-effect-tabs",
            "aether-lighting-quick-effects",
            "aether-lighting-effect-selector",
        ] {
            assert!(
                window.try_find(id).is_some(),
                "missing Aether lighting control {id}"
            );
        }
        assert!(
            window
                .try_find("aether-lighting-advanced-effects")
                .is_none()
        );
        window.click("aether-lighting-advanced-tab", cx);
        assert!(window.try_find("aether-lighting-quick-effects").is_none());
        for id in [
            "aether-lighting-advanced-effects",
            "aether-lighting-profile-selector",
            "aether-lighting-chroma-studio",
        ] {
            assert!(window.try_find(id).is_some(), "missing Aether control {id}");
        }
    })
    .unwrap();
}

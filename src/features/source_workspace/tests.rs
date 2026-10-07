//! Compiled by cargo check --all-targets; not executed under the source-audit policy.
use super::{SourceProductWorkspace, active_profile_settings_dirty};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};
use serde_json::json;

#[gpui_kit::test]
fn receiver_never_mounts_a_profile_selector(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let view = cx.new(|cx| {
            SourceProductWorkspace::new(crate::demo::registered_preview(179).unwrap(), window, cx)
        });
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("source-profile-select").is_none());
        assert!(window.try_find("source-product-navigation").is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn registered_preview_uses_the_product_navigation_and_stable_tab_ids(cx: &mut TestAppContext) {
    cx.update(|cx| gpui_kit::init(cx));
    let device = crate::demo::registered_preview(653).unwrap();
    let expected_pages: Vec<_> = crate::product::registered(device.product_id)
        .unwrap()
        .primary_navigation()
        .unwrap()
        .pages()
        .iter()
        .filter(|page| page.role() != crate::product::ProductPageRole::Help)
        .map(|page| (page.id(), format!("product-page-{}", page.id().key())))
        .collect();
    let first_page = expected_pages.first().unwrap().0;
    let second_tab = expected_pages.get(1).unwrap().1.clone();
    let second_page = expected_pages.get(1).unwrap().0;
    let mut workspace = None;
    let handle = cx.open_window(size(px(2560.), px(1200.)), |window, cx| {
        let view = cx.new(|cx| SourceProductWorkspace::new(device, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        for (_, tab_id) in &expected_pages {
            assert!(
                window.try_find(tab_id.clone()).is_some(),
                "missing source tab {tab_id}"
            );
        }
        window.click(second_tab.clone(), cx);
        assert_ne!(second_page, first_page);
        assert_eq!(workspace.as_ref().unwrap().read(cx).page, Some(second_page));
        assert!(window.try_find(second_tab.clone()).is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn accessory_and_monitor_previews_keep_the_registered_product_tabs(cx: &mut TestAppContext) {
    cx.update(|cx| gpui_kit::init(cx));

    for pid in [164, 179, 241, 784, 3858, 3880] {
        let device = crate::demo::registered_preview(pid).expect("registered source preview");
        let navigation = crate::product::registered(pid)
            .and_then(|product| product.primary_navigation())
            .expect("primary source navigation");
        let pages: Vec<_> = navigation.pages().iter().collect();
        let page_ids: Vec<_> = pages
            .iter()
            .filter(|page| page.role() != crate::product::ProductPageRole::Help)
            .map(|page| format!("product-page-{}", page.id().key()))
            .collect();
        let help_ids: Vec<_> = pages
            .iter()
            .filter(|page| page.role() == crate::product::ProductPageRole::Help)
            .map(|page| format!("source-help-{}", page.id().key()))
            .collect();

        let handle = cx.open_window(size(px(1600.), px(1000.)), |window, cx| {
            Root::new(
                cx.new(|cx| SourceProductWorkspace::new(device, window, cx)),
                window,
                cx,
            )
        });
        let page_ids = page_ids.clone();
        let help_ids = help_ids.clone();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            for id in page_ids {
                assert!(
                    window.try_find(id.clone()).is_some(),
                    "missing {pid} tab {id}"
                );
            }
            for id in help_ids {
                assert!(
                    window.try_find(id.clone()).is_some(),
                    "missing {pid} help {id}"
                );
            }
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn aether_lighting_preview_uses_the_registered_tab_and_switches_effect_modes(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| gpui_kit::init(cx));
    let device = crate::demo::registered_preview(784).unwrap();
    let lighting_tab = crate::product::registered(784)
        .unwrap()
        .primary_navigation()
        .unwrap()
        .pages()
        .iter()
        .find(|page| page.kind().key() == "TAB_LIGHTING")
        .map(|page| format!("product-page-{}", page.id().key()))
        .unwrap();
    let handle = cx.open_window(size(px(1600.), px(1000.)), |window, cx| {
        Root::new(
            cx.new(|cx| SourceProductWorkspace::new(device, window, cx)),
            window,
            cx,
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(lighting_tab, cx);
        assert!(window.try_find("aether-lighting-override").is_some());
        assert!(window.try_find("aether-lighting-quick-effects").is_some());
        assert!(
            window
                .try_find("aether-lighting-advanced-effects")
                .is_none()
        );

        window.click("aether-lighting-advanced-tab", cx);
        assert!(window.try_find("aether-lighting-quick-effects").is_none());
        assert!(
            window
                .try_find("aether-lighting-advanced-effects")
                .is_some()
        );

        window.click("aether-lighting-quick-tab", cx);
        assert!(window.try_find("aether-lighting-quick-effects").is_some());
        assert!(
            window
                .try_find("aether-lighting-advanced-effects")
                .is_none()
        );
    })
    .unwrap();
}

#[test]
fn legacy_layout_moves_once_from_active_profile_and_device_value_wins() {
    let mut device = crate::demo::registered_preview(784).unwrap();
    let mut active = device.profiles[0].clone();
    active.id = "active".into();
    active.source_settings = Some(json!({"_accessory":{"bendData":[1,2]},"brightness":75}));
    let mut inactive = active.clone();
    inactive.id = "inactive".into();
    inactive.source_settings = Some(json!({"_accessory":{"bendData":[3]},"brightness":10}));
    device.profiles = vec![inactive, active];
    device.active_profile = "active".into();
    SourceProductWorkspace::migrate_device_settings(&mut device);
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["_accessory"],
        json!({"bendData":[1,2]})
    );
    assert!(device.profiles.iter().all(|p| {
        p.source_settings
            .as_ref()
            .unwrap()
            .get("_accessory")
            .is_none()
    }));
    assert_eq!(
        device.profiles[0].source_settings.as_ref().unwrap()["brightness"],
        10
    );
    device.profiles[0].source_settings.as_mut().unwrap()["_accessory"] = json!({"bendData":[4]});
    device.active_profile = "inactive".into();
    SourceProductWorkspace::migrate_device_settings(&mut device);
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["_accessory"],
        json!({"bendData":[1,2]})
    );
}

#[test]
fn active_profile_product_draft_is_detected_before_deletion() {
    let mut saved = crate::demo::registered_preview(784).unwrap();
    let mut device = saved.clone();
    let active = device.active_profile.clone();
    device
        .profiles
        .iter_mut()
        .find(|profile| profile.id == active)
        .unwrap()
        .source_settings = Some(json!({"brightness": 25}));

    assert!(active_profile_settings_dirty(&device, &saved));

    saved
        .profiles
        .iter_mut()
        .find(|profile| profile.id == active)
        .unwrap()
        .source_settings = Some(json!({"brightness": 25}));
    assert!(!active_profile_settings_dirty(&device, &saved));

    saved.profiles.retain(|profile| profile.id != active);
    assert!(active_profile_settings_dirty(&device, &saved));
}

#[gpui_kit::test]
fn lighting_changes_and_profile_switches_preserve_device_layout(cx: &mut TestAppContext) {
    cx.update(|cx| gpui_kit::init(cx));
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        Root::new(
            cx.new(|cx| {
                SourceProductWorkspace::new(
                    crate::demo::registered_preview(784).unwrap(),
                    window,
                    cx,
                )
            }),
            window,
            cx,
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        let view = cx.new(|cx| {
            SourceProductWorkspace::new(crate::demo::registered_preview(784).unwrap(), window, cx)
        });
        view.update(cx, |workspace, cx| {
            let layout =
                workspace.device.source_device_settings.as_ref().unwrap()["_accessory"].clone();
            let mut second = workspace.device.profiles[0].clone();
            second.id = "second".into();
            second.source_settings = None;
            workspace.device.profiles.push(second);
            workspace.capture(json!({"brightness":25}), cx);
            workspace.device.active_profile = "second".into();
            workspace.restore_active(window, cx);
            assert_eq!(
                workspace.device.source_device_settings.as_ref().unwrap()["_accessory"],
                layout
            );
            assert!(workspace.device.profiles.iter().all(|p| {
                p.source_settings
                    .as_ref()
                    .is_none_or(|s| s.get("_accessory").is_none())
            }));
        });
    })
    .unwrap();
}

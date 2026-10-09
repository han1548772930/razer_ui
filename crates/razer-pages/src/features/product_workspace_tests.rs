//! UI integration coverage compiled by cargo check; execution remains prohibited.
use super::ProductWorkspace;
use crate::features::display_mode_roots::{DisplayModeRoot, has_root_branch};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};

#[gpui_kit::test]
fn receiver_pairing_keeps_the_product_owners_local_indicator_draft(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1600.), px(1100.)), |window, cx| {
        let view = cx.new(|cx| {
            ProductWorkspace::new(
                razer_model::demo::registered_preview(179).unwrap(),
                false,
                window,
                cx,
            )
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    let saved = cx.update(|cx| view.read(cx).saved_snapshot(cx));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("source-profile-select").is_none());
        assert!(window.try_find("receiver-pairing-modal").is_none());
        assert_eq!(window.find("receiver-indicator-1").checked(), Some(true));
        window.click("receiver-indicator-2", cx);
        assert_eq!(window.find("receiver-indicator-2").checked(), Some(true));
    })
    .unwrap();
    cx.run_until_parked();
    let draft = cx.update(|cx| {
        let workspace = view.read(cx);
        let draft = workspace.snapshot(cx);
        assert!(workspace.committed_pending(cx));
        let profile = draft
            .profiles
            .iter()
            .find(|profile| profile.id == draft.active_profile)
            .unwrap();
        assert_eq!(
            profile
                .source_settings
                .as_ref()
                .unwrap()
                .pointer("/runtime/indicatorLedStatus"),
            Some(&serde_json::json!(2))
        );
        assert_eq!(
            serde_json::to_value(workspace.saved_snapshot(cx)).unwrap(),
            serde_json::to_value(&saved).unwrap()
        );
        draft
    });

    cx.update_window(handle.into(), |_, window, cx| {
        window.click("receiver-open-pairing", cx);
        assert!(window.find("receiver-pairing-modal").visible());
        assert!(window.try_find("receiver-pairing-loading").is_some());
        // Merely opening the utility cannot acknowledge a device operation.
        assert!(window.try_find("receiver-pairing-completion").is_none());
        window
            .within("receiver-pairing-modal")
            .click("receiver-close-pairing", cx);
        assert!(window.try_find("receiver-pairing-modal").is_none());
        assert_eq!(window.find("receiver-indicator-2").checked(), Some(true));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            serde_json::to_value(view.read(cx).snapshot(cx)).unwrap(),
            serde_json::to_value(&draft).unwrap()
        );
        assert!(view.read(cx).committed_pending(cx));
    });

    // Marking a completed local save must not erase edits made after its snapshot.
    cx.update(|cx| {
        view.update(cx, |workspace, cx| workspace.mark_saved(draft.clone(), cx));
    });
    cx.update(|cx| assert!(!view.read(cx).committed_pending(cx)));
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("receiver-indicator-3", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        view.update(cx, |workspace, cx| workspace.mark_saved(draft.clone(), cx));
    });
    cx.update(|cx| assert!(view.read(cx).committed_pending(cx)));
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |workspace, cx| workspace.discard(window, cx));
        window.render_frame(cx);
        assert_eq!(window.find("receiver-indicator-2").checked(), Some(true));
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(!view.read(cx).committed_pending(cx));
        assert_eq!(
            serde_json::to_value(view.read(cx).snapshot(cx)).unwrap(),
            serde_json::to_value(draft).unwrap()
        );
    });
}

#[gpui_kit::test]
fn display_mode_pages_require_both_current_root_and_local_content(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    // 653 exercises the original adapter; 70 exercises the source workspace.
    // 3080 has a normal Lighting page but no audited root; 691 has no roots.
    for (pid, expected_chroma, expected_armory) in [
        (653, true, true),
        (70, true, true),
        (3080, false, false),
        (691, false, false),
    ] {
        cx.open_window(size(px(1600.), px(1100.)), |window, cx| {
            let view = cx.new(|cx| {
                ProductWorkspace::new(
                    razer_model::demo::registered_preview(pid).unwrap(),
                    false,
                    window,
                    cx,
                )
            });
            view.update(cx, |workspace, cx| {
                assert!(
                    workspace.has_local_page(cx),
                    "{pid} has a normal product page"
                );
                assert_eq!(
                    workspace.has_chroma_device_page(cx),
                    expected_chroma,
                    "{pid} Chroma capability"
                );
                assert_eq!(
                    workspace.has_armory_device_page(cx),
                    expected_armory,
                    "{pid} Armory capability"
                );
                assert_eq!(
                    workspace.chroma_device_page(window, cx).is_some(),
                    expected_chroma,
                    "{pid} Chroma content"
                );
                assert_eq!(
                    workspace.armory_device_page(window, cx).is_some(),
                    expected_armory,
                    "{pid} Armory content"
                );
                if expected_chroma {
                    assert!(has_root_branch(DisplayModeRoot::ChromaApp, pid));
                }
                if expected_armory {
                    assert!(has_root_branch(DisplayModeRoot::Armory, pid));
                }
            });
            Root::new(view, window, cx)
        });
    }
}

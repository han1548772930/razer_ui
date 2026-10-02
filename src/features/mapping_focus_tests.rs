use super::DeviceWorkspace;
use crate::nav::Tab;
use gpui_kit::component::{Root, WindowExt};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Entity, TestAppContext, WindowHandle, px, size};

fn open_mouse(cx: &mut TestAppContext) -> (WindowHandle<Root>, Entity<DeviceWorkspace>) {
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| {
            DeviceWorkspace::new(crate::model::measured_devices().remove(0), true, window, cx)
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    (handle, workspace.unwrap())
}

#[gpui_kit::test]
fn rejected_mapping_switch_keeps_the_original_return_target(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, view) = open_mouse(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("customize-drawer-toggle", cx);
        window.click("drawer-input-RightButton", cx);
        window.click("mapping-category-disable", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let original_focus = cx.update(|cx| view.read(cx).mapping_return_focus.clone());
    assert!(original_focus.is_some());
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("drawer-input-Button4", cx);
        window.click("mapping-keep-editing", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.mapping.as_ref().unwrap().input, "RightButton");
        assert_eq!(state.mapping_return_focus, original_focus);
        assert!(state.mapping_dirty());
        assert!(!state.committed_pending());
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("mapping-cancel", cx);
        window.click("mapping-discard", cx);
        assert!(window.try_find("mapping-overlay").is_none());
        assert_eq!(
            window.find("drawer-input-RightButton").focused(),
            Some(true)
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn discarding_committed_profiles_keeps_mapping_drafts_or_requires_a_decision(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, view) = open_mouse(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |state, cx| {
            state.edit(window, cx, |settings| settings.lighting.brightness = 12);
        });
        window.render_frame(cx);
        window.click("mouse-input-RightButton", cx);
        window.click("mapping-category-disable", cx);
        let return_focus = view.read(cx).mapping_return_focus.clone();
        assert!(view.read(cx).committed_pending());
        assert!(view.read(cx).mapping_dirty());
        view.update(cx, |state, cx| assert!(state.discard_committed(window, cx)));
        assert!(!view.read(cx).committed_pending());
        assert!(view.read(cx).mapping_dirty());
        assert_eq!(view.read(cx).mapping.as_ref().unwrap().value, "disable");
        assert_eq!(view.read(cx).mapping_return_focus, return_focus);

        // A rollback must not retarget a draft from an unsaved profile.
        view.update(cx, |state, _| {
            let mut profile = state.device.profiles[0].clone();
            profile.id = "unsaved-profile".into();
            state.device.active_profile = profile.id.clone();
            state.device.profiles.push(profile);
        });
        assert!(view.read(cx).discard_would_remove_mapping());
        view.update(cx, |state, cx| {
            assert!(!state.discard_committed(window, cx))
        });
        assert_eq!(view.read(cx).device.active_profile, "unsaved-profile");
        assert_eq!(view.read(cx).mapping.as_ref().unwrap().value, "disable");
    })
    .unwrap();
}

#[gpui_kit::test]
fn closing_a_drawer_mapping_returns_focus_to_the_visible_toggle(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, view) = open_mouse(cx);
    for dirty in [false, true] {
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("customize-drawer-toggle", cx);
            window.click("drawer-input-RightButton", cx);
            if dirty {
                window.click("mapping-category-disable", cx);
            }
            window.click("customize-drawer-toggle", cx);
            if dirty {
                window.click("mapping-discard", cx);
            }
            assert!(window.try_find("mapping-overlay").is_none());
            assert_eq!(window.find("customize-drawer-toggle").focused(), Some(true));
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let state = view.read(cx);
            assert!(!state.customize_drawer.open);
            assert!(state.customize_drawer.source_input.is_none());
            assert!(state.mapping_return_focus.is_none());
            assert!(!state.mapping_recording);
        });
    }
}

#[gpui_kit::test]
fn mapping_navigation_and_global_save_release_editor_focus(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, view) = open_mouse(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("mouse-input-RightButton", cx);
        window.click("device-tab-performance", cx);
        assert!(window.try_find("mapping-overlay").is_none());
        assert_eq!(window.find("device-tab-performance").focused(), Some(true));
        window.click("device-tab-customize", cx);
        window.click("mouse-input-RightButton", cx);
        view.update(cx, |state, cx| state.set_page(Tab::Performance, window, cx));
        window.render_frame(cx);
        assert!(view.read(cx).workspace_focus.is_focused(window));
        assert!(view.read(cx).mapping_return_focus.is_none());
        window.click("device-tab-customize", cx);
        window.click("mouse-input-RightButton", cx);
        window.click("mapping-category-disable", cx);
        view.update(cx, |state, cx| assert!(state.prepare_save(window, cx)));
        window.render_frame(cx);
        assert!(window.try_find("mapping-overlay").is_none());
        assert_eq!(window.find("mouse-input-RightButton").focused(), Some(true));
        assert!(
            view.read(cx)
                .settings()
                .bindings
                .contains_key("RightButton")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn global_save_does_not_steal_focus_from_a_separate_surface(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, view) = open_mouse(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("mouse-input-RightButton", cx);
        window.click("mapping-category-disable", cx);
        window.open_dialog(cx, |dialog, _, _| dialog.title("另一项操作"));
        window.render_frame(cx);
        let focus = window.focused(cx);
        assert!(focus.is_some());
        assert!(!view.read(cx).workspace_focus.contains_focused(window, cx));
        view.update(cx, |state, cx| assert!(state.prepare_save(window, cx)));
        window.render_frame(cx);
        assert_eq!(window.focused(cx), focus);
        assert!(view.read(cx).mapping.is_none());
        assert!(view.read(cx).mapping_return_focus.is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn saving_from_the_close_dialog_returns_focus_when_a_filtered_row_disappears(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let (handle, view) = open_mouse(cx);
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |state, _| {
            state
                .settings_mut()
                .bindings
                .insert("RightButton".into(), "disable".into());
        });
        window.render_frame(cx);
        window.click("customize-drawer-toggle", cx);
        window.within("drawer-filter").click("input", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("drawer-input-RightButton", cx);
        window.click("mapping-category-default", cx);
        window.click("mapping-close", cx);
        window.click("mapping-save", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("mapping-overlay").is_none());
        assert!(window.try_find("drawer-input-RightButton").is_none());
        assert_eq!(
            window.within("drawer-filter").find("input").focused(),
            Some(true)
        );
        assert_eq!(view.read(cx).settings().bindings["RightButton"], "default");
    })
    .unwrap();
}

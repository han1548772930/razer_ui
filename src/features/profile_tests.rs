use super::{
    create_local_profile, default_profile_name, delete_local_profile, duplicate_profile_name,
    rename_local_profile, reset_local_profile,
};
use crate::features::DeviceWorkspace;
use crate::features::settings::ProfileSettings;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};

fn device() -> crate::model::Device {
    let mut device = crate::demo::demo_keyboard();
    device.profiles[0].name = "Work".into();
    device.profiles[0].settings = Some(ProfileSettings::default());
    device
}

#[test]
fn source_names_fill_gaps_without_nested_duplicate_suffixes() {
    let mut device = device();
    assert_eq!(default_profile_name(&device.profiles, ""), "Default");
    assert_eq!(
        default_profile_name(&device.profiles, "Desktop"),
        "Desktop-Default"
    );
    let saved = device.clone();
    create_local_profile(&mut device, &saved, true);
    assert_eq!(device.profiles[1].name, "Work (1)");
    create_local_profile(&mut device, &saved, true);
    assert_eq!(device.profiles[2].name, "Work (2)");
    let first_copy = device.profiles[1].id.clone();
    assert!(delete_local_profile(&mut device, &first_copy));
    assert_eq!(
        duplicate_profile_name(&device.profiles, "Work (2)"),
        "Work (1)"
    );
}

#[test]
fn duplicates_are_independent_and_new_profiles_use_defaults() {
    let mut device = device();
    let settings = device.profiles[0].settings.as_mut().unwrap();
    settings.volume = 23;
    settings
        .bindings
        .insert("KEY_A".into(), "keyboard:Ctrl+C".into());
    settings
        .hypershift_bindings
        .insert("KEY_B".into(), "disable".into());
    let saved = device.clone();
    create_local_profile(&mut device, &saved, true);
    assert_eq!(device.profiles[0].settings, device.profiles[1].settings);
    device.profiles[1].settings.as_mut().unwrap().volume = 41;
    device.profiles[1]
        .settings
        .as_mut()
        .unwrap()
        .bindings
        .clear();
    assert_eq!(device.profiles[0].settings.as_ref().unwrap().volume, 23);
    assert_eq!(
        device.profiles[0].settings.as_ref().unwrap().bindings.len(),
        1
    );
    create_local_profile(&mut device, &saved, false);
    let new_settings = device.profiles[2].settings.as_ref().unwrap();
    assert_eq!(new_settings.volume, ProfileSettings::default().volume);
    assert!(new_settings.bindings.is_empty());
    assert!(new_settings.hypershift_bindings.is_empty());
    assert!(device.profiles[2].dpi_stages.is_none());
    assert_ne!(device.profiles[2].id, device.profiles[1].id);
}

#[test]
fn rename_trims_rejects_duplicates_and_obeys_browser_utf16_limit() {
    let mut device = device();
    let id = device.active_profile.clone();
    assert!(rename_local_profile(&mut device, &id, "  Personal  "));
    assert_eq!(device.profiles[0].name, "Personal");
    assert!(!rename_local_profile(&mut device, &id, "\t \n"));
    assert!(!rename_local_profile(&mut device, &id, &"🖱".repeat(17)));
    let saved = device.clone();
    create_local_profile(&mut device, &saved, true);
    assert!(!rename_local_profile(&mut device, &id, "Personal (1)"));
    assert_eq!(device.profiles[0].name, "Personal");
    assert!(rename_local_profile(&mut device, &id, &"🖱".repeat(16)));
}

#[test]
fn deletion_preserves_the_last_profile_and_saved_identities() {
    let mut device = device();
    let first = device.active_profile.clone();
    assert!(!delete_local_profile(&mut device, &first));
    let original = device.clone();
    create_local_profile(&mut device, &original, true);
    let second = device.active_profile.clone();
    let saved = device.clone();
    assert!(delete_local_profile(&mut device, &second));
    assert_eq!(device.active_profile, first);
    create_local_profile(&mut device, &saved, true);
    assert_ne!(device.active_profile, second);
    assert!(!delete_local_profile(&mut device, "missing"));
    assert_eq!(saved.profiles.len(), 2);
}

#[test]
fn resetting_bindings_keeps_other_settings_and_full_reset_keeps_identity() {
    let mut device = device();
    let id = device.active_profile.clone();
    let settings = device.profiles[0].settings.as_mut().unwrap();
    settings.volume = 17;
    settings.bindings.insert("KEY_A".into(), "disable".into());
    settings
        .hypershift_bindings
        .insert("KEY_A".into(), "keyboard:F1".into());
    assert!(reset_local_profile(&mut device, &id, true));
    let settings = device.profiles[0].settings.as_ref().unwrap();
    assert_eq!(settings.volume, 17);
    assert!(settings.bindings.is_empty());
    assert!(settings.hypershift_bindings.is_empty());
    assert!(reset_local_profile(&mut device, &id, false));
    assert_eq!(device.profiles[0].name, "Work");
    assert_eq!(device.profiles[0].id, id);
    assert_eq!(device.profiles[0].settings.as_ref().unwrap().volume, 70);
}

#[gpui_kit::test]
fn profile_menu_rename_enter_escape_and_delete_are_real_ui_flows(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(900.)), |window, cx| {
        let view = cx.new(|cx| DeviceWorkspace::new(device(), true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-more", cx);
        window.click("profile-delete", cx);
        assert!(window.try_find("profile-confirmation").is_none());
        window.click("profile-rename", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("profile-name-input").focused(), Some(true));
        window.input("Renamed", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.device.profiles[0].name, "Renamed");
        assert_eq!(workspace.saved.profiles[0].name, "Work");
        assert!(workspace.dirty());
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-more", cx);
        window.click("profile-rename", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.input("Cancelled", cx);
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).device.profiles[0].name, "Renamed"));
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-more", cx);
        window.click("profile-duplicate", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.device.profiles.len(), 2);
        assert_eq!(
            workspace.device.active_profile_obj().unwrap().name,
            "Renamed (1)"
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-more", cx);
        window.click("profile-delete", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("profile-confirmation").is_some());
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).device.profiles.len(), 2));
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-more", cx);
        window.click("profile-delete", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-delete-confirm", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.device.profiles.len(), 1);
        assert_eq!(
            workspace.device.active_profile_obj().unwrap().name,
            "Renamed"
        );
    });
    cx.simulate_window_resize(handle.into(), size(px(700.), px(900.)));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        let more = window.find("profile-more").bounds();
        assert!(more.right() <= window.find("device-tab-customize").bounds().left());
        assert!(window.find("profile-select").bounds().right() <= more.left());
        window.click("profile-more", cx);
        window.click("profile-rename", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.find("profile-name-input").bounds().right()
                <= window.find("profile-more").bounds().left()
        );
        window.press("escape", cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn new_profile_and_delete_preserve_pending_mapping_until_a_decision(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| DeviceWorkspace::new(device(), true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-more", cx);
        window.click("profile-add", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let added_id = cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.device.profiles.len(), 2);
        workspace.device.active_profile.clone()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("keyboard-input-KEY_A", cx);
        window.click("mapping-category-keyboard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(view.read(cx).mapping_dirty()));
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-more", cx);
        window.click("profile-delete", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-delete-confirm", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("mapping-keep-editing", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.device.profiles.len(), 2);
        assert_eq!(workspace.device.active_profile, added_id);
        assert!(workspace.mapping_dirty());
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-more", cx);
        window.click("profile-delete", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-delete-confirm", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("mapping-discard", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.device.profiles.len(), 1);
        assert!(!workspace.mapping_dirty());
        assert_eq!(workspace.device.active_profile_obj().unwrap().name, "Work");
    });
}

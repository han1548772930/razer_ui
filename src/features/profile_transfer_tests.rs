use super::{MAX_BYTES, decode_profile, encode_profile, import_name, read_profile, write_profile};
use crate::features::{
    DeviceWorkspace,
    settings::{LinkedGame, ProfileSettings},
};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn device() -> crate::model::Device {
    let mut device = crate::demo::demo_keyboard();
    device.profiles[0].name = "Work".into();
    let mut settings = ProfileSettings::default();
    settings.normalize(device.product_id);
    settings
        .bindings
        .insert("KEY_A".into(), "keyboard:Ctrl+C".into());
    settings
        .hypershift_bindings
        .insert("KEY_B".into(), "future:opaque".into());
    settings.linked_games.push(LinkedGame {
        name: "Game".into(),
        executable: "C:\\Games\\Game.exe".into(),
    });
    settings
        .keyboard
        .set_snap_pair(1, ["KEY_Q".into(), "KEY_E".into()]);
    let custom = settings.keyboard.add_dial().unwrap();
    settings.keyboard.rename_dial(&custom, "自定义拨轮");
    settings.keyboard.select_dial(&custom);
    settings
        .keyboard
        .dial_modes
        .iter_mut()
        .find(|mode| mode.uid == custom)
        .unwrap()
        .mappings
        .insert("ScrollRight".into(), "keyboard:Ctrl+C".into());
    device.profiles[0].settings = Some(settings);
    device
}

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "razer-profile-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        for name in ["source.json", "copy.json", "bad.json"] {
            let _ = std::fs::remove_file(self.path(name));
        }
        let _ = std::fs::remove_dir(&self.0);
    }
}

#[test]
fn local_profile_roundtrip_keeps_all_settings_and_regenerates_identity() {
    let device = device();
    let original = &device.profiles[0];
    let bytes = encode_profile(&device, original).unwrap();
    let imported = decode_profile(&bytes, device.product_id, device.layout_id).unwrap();
    assert_eq!(imported.name, original.name);
    assert_eq!(imported.settings, original.settings);
    assert!(imported.id.is_empty());
    assert!(imported.guid.is_empty());
    assert!(imported.dpi_stages.is_none());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["format"],
        "razer-ui-profile"
    );
}

#[test]
fn profile_transfer_keeps_a_dial_disabled_by_alt_tab_without_changing_its_identity() {
    let mut device = device();
    {
        let keyboard = &mut device.profiles[0].settings.as_mut().unwrap().keyboard;
        keyboard.select_dial("SWITCH_APPLICATIONS");
        for uid in ["KEYBOARD_BRIGHTNESS", "WINDOWS_ZOOM", "TRACK_JOGGING"] {
            keyboard.enable_dial(uid, false);
        }
    }
    for disabled in [true, false] {
        device.profiles[0]
            .settings
            .as_mut()
            .unwrap()
            .keyboard
            .set_disable_alt_tab(disabled);
        let bytes = encode_profile(&device, &device.profiles[0]).unwrap();
        let imported = decode_profile(&bytes, device.product_id, device.layout_id).unwrap();
        assert_eq!(imported.settings, device.profiles[0].settings);
        let keyboard = &imported.settings.as_ref().unwrap().keyboard;
        assert_eq!(keyboard.dial_active, "SWITCH_APPLICATIONS");
        assert!(
            keyboard
                .dial_modes
                .iter()
                .filter(|mode| !mode.is_custom)
                .all(|mode| !mode.enabled)
        );

        let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        invalid["profile"]["settings"]["keyboard"]["dial_active"] = "missing-mode".into();
        assert!(
            decode_profile(
                &serde_json::to_vec(&invalid).unwrap(),
                device.product_id,
                device.layout_id
            )
            .is_err()
        );
    }
}

#[test]
fn imports_reject_incompatible_schema_bad_values_duplicate_keys_and_large_files() {
    let device = device();
    let bytes = encode_profile(&device, &device.profiles[0]).unwrap();
    let valid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let rejects = |value: serde_json::Value| {
        assert!(
            decode_profile(
                &serde_json::to_vec(&value).unwrap(),
                device.product_id,
                device.layout_id
            )
            .is_err()
        );
    };
    for (field, value) in [
        ("version", serde_json::json!(999)),
        ("format", serde_json::json!("synapse4")),
        ("product_id", serde_json::json!(182)),
        ("layout_id", serde_json::json!(999)),
    ] {
        let mut changed = valid.clone();
        changed[field] = value;
        rejects(changed);
    }
    let mut changed = valid.clone();
    changed["profile"]["settings"]["volume"] = 101.into();
    rejects(changed);
    let mut changed = valid.clone();
    changed["profile"]["settings"]["keyboard"]["future_option"] = true.into();
    rejects(changed);
    let mut changed = valid.clone();
    changed["profile"]["settings"]
        .as_object_mut()
        .unwrap()
        .remove("lighting");
    rejects(changed);
    let mut changed = valid.clone();
    changed["profile"]["settings"]["linked_games"][0]["executable"] = "relative.exe".into();
    rejects(changed);
    let mut changed = valid.clone();
    changed["profile"]["name"] = " ".into();
    rejects(changed);
    let duplicate = String::from_utf8(bytes)
        .unwrap()
        .replace("\"volume\": 70", "\"volume\": 20, \"volume\": 70");
    assert!(decode_profile(duplicate.as_bytes(), device.product_id, device.layout_id).is_err());
    assert!(
        decode_profile(
            &vec![b' '; MAX_BYTES + 1],
            device.product_id,
            device.layout_id
        )
        .is_err()
    );
    assert!(
        decode_profile(
            br#"{"profiles":[],"productId":653}"#,
            device.product_id,
            device.layout_id
        )
        .is_err()
    );
}

#[test]
fn imported_names_are_unique_and_export_never_overwrites_an_existing_file() {
    let mut device = device();
    let name = "界".repeat(32);
    device.profiles[0].name = name.clone();
    let resolved = import_name(&device.profiles, &name);
    assert!(resolved.encode_utf16().count() <= 32);
    assert!(resolved.ends_with(" (1)"));
    let files = Files::new();
    let path = files.path("source.json");
    let bytes = encode_profile(&device, &device.profiles[0]).unwrap();
    write_profile(&path, &bytes).unwrap();
    assert!(write_profile(&path, b"replace").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(
        read_profile(&path, device.product_id, device.layout_id)
            .unwrap()
            .settings,
        device.profiles[0].settings
    );
}

#[gpui_kit::test]
fn native_file_dialogs_export_and_import_a_profile_without_overwriting_existing_profiles(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let files = Files::new();
    let path = files.path("copy.json");
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
        window.click("profile-export", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-export-save", cx)
    })
    .unwrap();
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|_| Some(path.clone()));
    cx.run_until_parked();
    let exported = std::fs::read(&path).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("profile-export-result").is_some());
        window.click("profile-export-close", cx);
        window.click("profile-more", cx);
        window.click("profile-import", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(window.find("profile-import-confirm").disabled(), Some(true));
        window.click("profile-import-browse", cx);
    })
    .unwrap();
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|_| Some(vec![path.clone()]));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("profile-import-name").value(), Some("Work (1)"));
        window.click("profile-import-confirm", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.device().profiles.len(), 2);
        assert_eq!(workspace.device().profiles[0].name, "Work");
        assert_eq!(workspace.device().profiles[1].name, "Work (1)");
        assert_eq!(
            workspace.device().profiles[0].settings,
            workspace.device().profiles[1].settings
        );
        assert_ne!(
            workspace.device().profiles[0].id,
            workspace.device().profiles[1].id
        );
        assert!(workspace.dirty());
    });
    assert_eq!(std::fs::read(&path).unwrap(), exported);
}

#[gpui_kit::test]
fn cancelling_or_changing_profile_during_import_keeps_existing_drafts(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let files = Files::new();
    let path = files.path("source.json");
    let data = device();
    write_profile(&path, &encode_profile(&data, &data.profiles[0]).unwrap()).unwrap();
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| DeviceWorkspace::new(data, true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-more", cx);
        window.click("profile-import", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-import-browse", cx)
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| None);
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("profile-import-confirm").disabled(), Some(true));
        window.click("profile-import-browse", cx);
        view.update(cx, |workspace, cx| workspace.add_profile(window, cx));
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| Some(vec![path.clone()]));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("profile-import-error").is_some());
        assert_eq!(window.find("profile-import-confirm").disabled(), Some(true));
        window.click("profile-import-cancel", cx);
    })
    .unwrap();
    cx.update(|cx| assert_eq!(view.read(cx).device().profiles.len(), 2));
}

#[gpui_kit::test]
fn confirming_an_import_preserves_a_mapping_when_the_user_keeps_editing(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let files = Files::new();
    let path = files.path("source.json");
    let data = device();
    write_profile(&path, &encode_profile(&data, &data.profiles[0]).unwrap()).unwrap();
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| DeviceWorkspace::new(data, true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("keyboard-input-KEY_B", cx);
        window.click("mapping-category-keyboard", cx);
        window.click("mapping-record", cx);
        window.press("ctrl-k", cx);
        window.click("profile-more", cx);
        window.click("profile-import", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let draft = cx.update(|cx| view.read(cx).mapping.as_ref().unwrap().value.clone());
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-import-browse", cx)
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| Some(vec![path.clone()]));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-import-confirm", cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("mapping-keep-editing").is_some());
        window.click("mapping-keep-editing", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert_eq!(workspace.device().profiles.len(), 1);
        assert_eq!(workspace.mapping.as_ref().unwrap().value, draft);
        assert!(workspace.mapping_dirty());
        assert!(!workspace.settings().bindings.contains_key("KEY_B"));
    });
}

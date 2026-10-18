use crate::features::{
    DeviceWorkspace,
    settings::{LinkedGame, ProfileSettings},
};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};
use razer_model::profile_transfer::encode_profile;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn device() -> razer_model::model::Device {
    let mut device = crate::test_devices::keyboard();
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

fn write_profile(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let operation = razer_storage::file_transfer::Transfer::new().accept()?;
    razer_storage::profile_transfer::write_local(&operation, path, bytes)
}
#[gpui_kit::test]
fn native_file_dialogs_export_and_import_a_profile_without_overwriting_existing_profiles(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
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
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
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
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
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

use super::{game_from_path, valid_game};
use crate::features::{DeviceWorkspace, settings::LinkedGame};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, SharedString, TestAppContext, px, size};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct GameFile(PathBuf);
impl GameFile {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "razer-linked-game-{}-{}.exe",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .unwrap();
        Self(path)
    }
}
impl Drop for GameFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn linked_paths_are_absolute_executables_and_never_command_lines() {
    let game = LinkedGame {
        name: "Test".into(),
        executable: "C:\\Games\\Test.exe".into(),
    };
    assert!(valid_game(&game));
    for path in [
        "Test.exe",
        "C:\\Games\\Test.exe --launch",
        "C:\\Games\\Test.bat",
        "C:\\Games\\Test\n.exe",
    ] {
        assert!(!valid_game(&LinkedGame {
            executable: path.into(),
            ..game.clone()
        }));
    }
    let file = GameFile::new();
    let saved = game_from_path(&file.0).unwrap();
    assert!(valid_game(&saved));
    assert_eq!(
        saved.executable,
        file.0.canonicalize().unwrap().to_string_lossy()
    );
}

#[gpui_kit::test]
fn adding_duplicate_removing_and_cancelling_linked_games_uses_the_real_dialog(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let file = GameFile::new();
    let game = game_from_path(&file.0).unwrap();
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::demo::demo_keyboard(), true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-more", cx);
        window.click("profile-linked-games", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.try_find("linked-games-empty").is_some());
        window.click("linked-games-add", cx);
    })
    .unwrap();
    assert!(cx.did_prompt_for_paths());
    cx.simulate_path_prompt_response(|_| Some(vec![file.0.clone()]));
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            view.read(cx).settings().linked_games.is_empty(),
            "selection stays in the dialog until Save"
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("linked-games-empty").is_none());
        window.click("linked-games-add", cx);
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| Some(vec![file.0.clone()]));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("linked-games-error").is_some());
        window.click("linked-games-save", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(view.read(cx).settings().linked_games, [game.clone()]);
        assert!(view.read(cx).dirty());
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("profile-more", cx);
        window.click("profile-linked-games", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(
            SharedString::from(format!("linked-game-remove-{}", game.executable)),
            cx,
        );
        assert!(window.try_find("linked-games-empty").is_some());
        window.click("linked-games-cancel", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).settings().linked_games, [game]));
}

#[gpui_kit::test]
fn a_late_file_selection_does_not_link_the_new_active_profile(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let file = GameFile::new();
    let mut workspace = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::demo::demo_keyboard(), true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("profile-more", cx);
        window.click("profile-linked-games", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("linked-games-add", cx);
        view.update(cx, |workspace, cx| workspace.add_profile(window, cx));
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| Some(vec![file.0.clone()]));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("linked-games-error").is_some());
        window.click("linked-games-save", cx);
        assert!(window.try_find("linked-games-dialog").is_some());
    })
    .unwrap();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert!(
            workspace.device().profiles.iter().all(|profile| profile
                .settings
                .as_ref()
                .unwrap()
                .linked_games
                .is_empty())
        );
    });
}

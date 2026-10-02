use super::pressed_key;
use crate::features::{settings::Keyboard, workspace::DeviceWorkspace};
use gpui_kit::component::{Root, Theme};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, SharedString, TestAppContext, px, size};

#[test]
fn snap_pairs_accept_source_keys_without_duplicates_or_partial_commits() {
    let mut keyboard = Keyboard::default();
    assert!(keyboard.set_snap_pair(0, ["A".into(), "D".into()]));
    assert_eq!(keyboard.snap_keys, ["KEY_A", "KEY_D"]);
    let saved = keyboard.clone();
    for pair in [
        ["KEY_A", "KEY_A"],
        ["KEY_FN", "KEY_B"],
        ["KEY_LEFT_GUI", "KEY_B"],
        ["", "KEY_C"],
    ] {
        assert!(!keyboard.set_snap_pair(0, pair.map(str::to_owned)));
        assert_eq!(keyboard, saved);
    }
    assert!(!keyboard.set_snap_pair(1, ["KEY_A".into(), "KEY_B".into()]));
    for (ix, pair) in [["Q", "E"], ["W", "S"], ["Z", "X"]].into_iter().enumerate() {
        assert!(keyboard.set_snap_pair(ix + 1, pair.map(str::to_owned)));
    }
    assert!(!keyboard.set_snap_pair(4, ["KEY_B".into(), "KEY_C".into()]));
    keyboard.remove_snap_pair(0);
    assert_eq!(keyboard.snap_key_pairs().len(), 4);
    keyboard.remove_snap_pair(2);
    assert_eq!(keyboard.snap_key_pairs()[2], ["KEY_Z", "KEY_X"]);
    assert_eq!(pressed_key("space"), Some("KEY_SPACEBAR".into()));
    assert_eq!(pressed_key("up"), Some("KEY_UP_ARROW".into()));
    assert_eq!(pressed_key("Num Enter"), Some("KEY_NUMPAD_ENTER".into()));
    assert_eq!(pressed_key("windows"), None);
    let restored: Keyboard =
        serde_json::from_str(&serde_json::to_string(&keyboard).unwrap()).unwrap();
    assert_eq!(restored, keyboard);
}

#[test]
fn command_modes_preserve_identity_cycle_forward_and_keep_one_preset() {
    let mut keyboard = Keyboard::default();
    let custom = keyboard.add_dial().unwrap();
    assert_ne!(custom, keyboard.dial_active);
    assert!(keyboard.dial_modes[0].is_custom);
    assert_eq!(keyboard.dial_modes[0].mappings.len(), 2);
    assert_eq!(keyboard.dial_modes[0].mappings["ScrollRight"], "disable");
    assert!(!keyboard.rename_dial(&custom, "  "));
    assert!(!keyboard.rename_dial(&custom, &"😀".repeat(21)));
    assert!(keyboard.rename_dial(&custom, "编辑模式"));
    keyboard.select_dial("WINDOWS_ZOOM");
    keyboard.enable_dial("WINDOWS_ZOOM", false);
    assert_eq!(keyboard.dial_active, "SWITCH_APPLICATIONS");
    keyboard.enable_dial("SWITCH_APPLICATIONS", false);
    keyboard.enable_dial("TRACK_JOGGING", false);
    keyboard.enable_dial("KEYBOARD_BRIGHTNESS", false);
    assert!(
        keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == "KEYBOARD_BRIGHTNESS")
            .unwrap()
            .enabled
    );
    // Em reuses its last-default restriction for enabled custom switches too.
    assert!(!keyboard.dial_enabled_change_allowed(&custom, false));
    keyboard.enable_dial(&custom, false);
    assert!(
        keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == custom)
            .unwrap()
            .enabled
    );
    keyboard.select_dial(&custom);
    keyboard.move_dial(&custom, 1);
    assert_eq!(keyboard.dial_active, custom);
    keyboard.delete_dial(&custom);
    assert_eq!(keyboard.dial_active, "KEYBOARD_BRIGHTNESS");
    assert!(keyboard.dial_modes.iter().all(|mode| mode.uid != custom));
    let original_preset_uid = "source-preset-uid";
    keyboard
        .dial_modes
        .iter_mut()
        .find(|mode| mode.name == "KEYBOARD_BRIGHTNESS")
        .unwrap()
        .uid = original_preset_uid.into();
    keyboard.reset_dial();
    assert_eq!(keyboard.dial_active, original_preset_uid);
    assert_eq!(keyboard.dial_modes.len(), 8);
    let restored: Keyboard =
        serde_json::from_str(&serde_json::to_string(&keyboard).unwrap()).unwrap();
    assert_eq!(restored, keyboard);
    let mut legacy: Keyboard = serde_json::from_value(serde_json::json!({
        "snap_keys": ["A", "D"],
        "dial_modes": [{"uid": "KEYBOARD_BRIGHTNESS", "enabled": false}],
        "dial_active": "missing-mode"
    }))
    .unwrap();
    legacy.normalize();
    assert_eq!(legacy.dial_modes[0].uid, "KEYBOARD_BRIGHTNESS");
    assert!(legacy.dial_modes[0].enabled);
    assert_eq!(legacy.dial_modes[0].color, [0, 255, 0]);
    assert_eq!(legacy.dial_active, "KEYBOARD_BRIGHTNESS");
    assert_eq!(legacy.snap_keys, ["KEY_A", "KEY_D"]);
}

#[test]
fn alt_tab_restriction_preserves_the_current_dial_identity_without_reenabling_it() {
    let mut keyboard = Keyboard::default();
    keyboard.select_dial("SWITCH_APPLICATIONS");
    let custom = keyboard.add_dial().unwrap();
    keyboard.rename_dial(&custom, "Switch Applications");
    keyboard.set_disable_alt_tab(true);
    assert_eq!(keyboard.dial_active, "SWITCH_APPLICATIONS");
    assert!(
        !keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == "SWITCH_APPLICATIONS")
            .unwrap()
            .enabled
    );
    assert!(
        !keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == custom)
            .unwrap()
            .enabled
    );
    assert!(!keyboard.dial_enabled_change_allowed("SWITCH_APPLICATIONS", true));
    keyboard.enable_dial("SWITCH_APPLICATIONS", true);
    keyboard.normalize();
    assert_eq!(keyboard.dial_active, "SWITCH_APPLICATIONS");

    let mut restored: Keyboard =
        serde_json::from_str(&serde_json::to_string(&keyboard).unwrap()).unwrap();
    restored.normalize();
    assert_eq!(restored, keyboard);
    keyboard.set_disable_alt_tab(false);
    keyboard.normalize();
    assert_eq!(keyboard.dial_active, "SWITCH_APPLICATIONS");
    assert!(
        !keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == "SWITCH_APPLICATIONS")
            .unwrap()
            .enabled
    );
    keyboard.enable_dial("SWITCH_APPLICATIONS", true);
    assert!(
        keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == "SWITCH_APPLICATIONS")
            .unwrap()
            .enabled
    );
    keyboard.set_disable_alt_tab(true);
    keyboard.select_dial("WINDOWS_ZOOM");
    keyboard.select_dial("SWITCH_APPLICATIONS");
    assert_eq!(keyboard.dial_active, "WINDOWS_ZOOM");
    keyboard.reset_dial();
    assert_eq!(keyboard.dial_active, "KEYBOARD_BRIGHTNESS");
    assert!(
        !keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == "SWITCH_APPLICATIONS")
            .unwrap()
            .enabled
    );
}

#[test]
fn alt_tab_can_disable_the_only_preset_and_empty_cycles_keep_a_valid_identity() {
    let mut keyboard = Keyboard::default();
    keyboard.select_dial("SWITCH_APPLICATIONS");
    for uid in ["KEYBOARD_BRIGHTNESS", "WINDOWS_ZOOM", "TRACK_JOGGING"] {
        keyboard.enable_dial(uid, false);
    }
    assert!(!keyboard.dial_enabled_change_allowed("SWITCH_APPLICATIONS", false));
    keyboard.set_disable_alt_tab(true);
    assert!(keyboard.dial_modes.iter().all(|mode| !mode.enabled));
    keyboard.normalize();
    assert_eq!(keyboard.dial_active, "SWITCH_APPLICATIONS");
    assert!(keyboard.dial_modes.iter().all(|mode| !mode.enabled));
    keyboard.set_disable_alt_tab(false);
    keyboard.normalize();
    assert!(keyboard.dial_modes.iter().all(|mode| !mode.enabled));
    let custom = keyboard.add_dial().unwrap();
    keyboard.select_dial(&custom);
    keyboard.enable_dial(&custom, false);
    assert_eq!(keyboard.dial_active, custom);
    assert!(keyboard.dial_selection_valid());
    keyboard.enable_dial("KEYBOARD_BRIGHTNESS", true);
    keyboard.normalize();
    assert_eq!(keyboard.dial_active, custom);
    assert!(keyboard.dial_selection_valid());
    keyboard.delete_dial(&custom);
    assert!(keyboard.dial_selection_valid());
}

#[gpui_kit::test]
fn gaming_alt_tab_checkbox_disables_the_source_preset_without_selecting_another(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16.)));
    let mut workspace = None;
    let handle = cx.open_window(size(px(1300.), px(1600.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::demo::demo_keyboard(), true, window, cx));
        view.update(cx, |view, cx| {
            view.edit(window, cx, |settings| {
                settings.keyboard.select_dial("SWITCH_APPLICATIONS")
            })
        });
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("gaming-windows").checked(), Some(false));
        window.click("gaming-enabled", cx);
        assert_eq!(window.find("gaming-windows").checked(), Some(true));
        window.click("gaming-alt-tab", cx);
        assert_eq!(
            window.find("dial-selected-SWITCH_APPLICATIONS").disabled(),
            Some(true)
        );
        assert_eq!(
            window.find("dial-enabled-SWITCH_APPLICATIONS").disabled(),
            Some(true)
        );
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).settings().keyboard.dial_active,
            "SWITCH_APPLICATIONS"
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("gaming-alt-tab", cx);
        assert_eq!(
            window.find("dial-selected-SWITCH_APPLICATIONS").disabled(),
            Some(true)
        );
        assert_eq!(
            window.find("dial-enabled-SWITCH_APPLICATIONS").disabled(),
            Some(false)
        );
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).settings().keyboard.dial_active,
            "SWITCH_APPLICATIONS"
        )
    });
}

#[gpui_kit::test]
fn snap_tap_captures_native_keys_and_cancels_an_incomplete_pair(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16.)));
    let mut workspace = None;
    let handle = cx.open_window(size(px(1300.), px(1600.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::demo::demo_keyboard(), true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(window.find("snap-key-0-0").disabled(), Some(true));
        window.click("snap-tap-enabled", cx);
        window.click("snap-key-0-0", cx);
        window.press("q", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).settings().keyboard.snap_keys,
            ["KEY_A", "KEY_D"]
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("q", cx);
        assert!(window.find("snap-tap-message").visible());
        window.press("e", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).settings().keyboard.snap_keys,
            ["KEY_Q", "KEY_E"]
        )
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("snap-add-pair", cx);
        window.press("w", cx);
        window.press("escape", cx);
        assert!(window.try_find("snap-key-1-0").is_none());
        window.click("snap-add-pair", cx);
        window.press("space", cx);
        window.press("tab", cx);
        assert_eq!(
            window.find("snap-key-1-0").label(),
            Some("Snap Tap 第 2 组按键 1")
        );
    })
    .unwrap();
    cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(
            state.settings().keyboard.snap_key_pairs(),
            vec![
                ["KEY_Q".to_owned(), "KEY_E".to_owned()],
                ["KEY_SPACEBAR".to_owned(), "KEY_TAB".to_owned()],
            ]
        );
        assert!(state.mapping.is_none());
    });
}

#[gpui_kit::test]
fn dial_highlight_and_mapping_edits_do_not_replace_the_current_mode(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16.)));
    let mut workspace = None;
    let handle = cx.open_window(size(px(1300.), px(1600.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::demo::demo_keyboard(), true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("dial-mode-WINDOWS_ZOOM", cx);
        assert_eq!(window.find("dial-mode-WINDOWS_ZOOM").selected(), Some(true));
        window.click("dial-add", cx);
    })
    .unwrap();
    let uid = cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.settings().keyboard.dial_active, "KEYBOARD_BRIGHTNESS");
        state.dial_highlight.clone().unwrap()
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(SharedString::from(format!("dial-mode-{uid}")), cx);
        window.click("dial-name", cx);
        window.press("ctrl-a", cx);
        window.input("Editing", cx);
        window.press("enter", cx);
        window.click(SharedString::from(format!("dial-expand-{uid}")), cx);
        window.click(
            SharedString::from(format!("dial-mapping-{uid}-ScrollRight")),
            cx,
        );
        window.click("mapping-category-mouse", cx);
        window.click("mapping-apply", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let state = view.read(cx);
        let mode = state
            .settings()
            .keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == uid)
            .unwrap();
        assert_ne!(mode.mappings["ScrollRight"], "disable");
        assert_eq!(mode.mappings["ScrollLeft"], "disable");
        assert!(!state.settings().bindings.contains_key("ScrollRight"));
        assert!(
            !state
                .settings()
                .hypershift_bindings
                .contains_key("ScrollRight")
        );
        assert_eq!(state.settings().keyboard.dial_active, "KEYBOARD_BRIGHTNESS");
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(
            SharedString::from(format!("dial-mapping-{uid}-ScrollLeft")),
            cx,
        );
        window.click("mapping-category-keyboard", cx);
    })
    .unwrap();
    cx.update(|cx| {
        let state = view.read(cx);
        let mode = state
            .settings()
            .keyboard
            .dial_modes
            .iter()
            .find(|mode| mode.uid == uid)
            .unwrap();
        assert_eq!(mode.name, "Editing");
        assert_eq!(
            state.mapping.as_ref().unwrap().dial_mode.as_deref(),
            Some(uid.as_str())
        );
        assert_eq!(state.settings().keyboard.dial_active, "KEYBOARD_BRIGHTNESS");
        assert!(!state.settings().bindings.contains_key("ScrollRight"));
        assert_eq!(mode.mappings["ScrollLeft"], "disable");
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("mapping-close", cx);
        window.click("mapping-discard", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click(SharedString::from(format!("dial-delete-{uid}")), cx);
        window.click("dial-delete-confirm", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let state = view.read(cx);
        assert!(
            state
                .settings()
                .keyboard
                .dial_modes
                .iter()
                .all(|mode| mode.uid != uid)
        );
        assert_eq!(state.settings().keyboard.dial_active, "KEYBOARD_BRIGHTNESS");
    });
}

#[gpui_kit::test]
fn snap_layout_picker_distinguishes_numpad_enter_and_filters_used_keys(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16.)));
    let mut workspace = None;
    let handle = cx.open_window(size(px(1300.), px(1600.)), |window, cx| {
        let mut device = crate::demo::demo_keyboard();
        device.layout_id = 16;
        let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("snap-tap-enabled", cx);
        window.click("snap-add-pair", cx);
        window.press("alt-down", cx);
        assert!(window.find("snap-layout-key").visible());
        window.click("snap-layout-key", cx);
        window.input("Num Enter", cx);
    })
    .unwrap();
    cx.update(|cx| {
        let choices = view.read(cx).snap_choice_items();
        for id in [
            "KEY_NUMPAD_ENTER",
            "KEY_ENTER",
            "KEY_RIGHT_CTRL",
            "KEY_LEFT_CTRL",
        ] {
            assert!(choices.iter().any(|choice| choice.id() == id));
        }
        for id in [
            "KEY_A",
            "KEY_D",
            "KEY_FN",
            "KEY_APPLICATION",
            "KEY_LEFT_GUI",
            "KEY_YEN",
        ] {
            assert!(!choices.iter().any(|choice| choice.id() == id));
        }
    });
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let state = view.read(cx);
        let capture = state.keyboard_controls.capture.as_ref().unwrap();
        assert_eq!(capture.keys[0], "KEY_NUMPAD_ENTER");
        assert_eq!(capture.key, 1);
        assert_eq!(state.settings().keyboard.snap_key_pairs().len(), 1);
        assert!(
            !state
                .snap_choice_items()
                .iter()
                .any(|choice| choice.id() == "KEY_NUMPAD_ENTER")
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("enter", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).settings().keyboard.snap_key_pairs()[1],
            ["KEY_NUMPAD_ENTER", "KEY_ENTER"]
        );
    });
}

#[gpui_kit::test]
fn dial_confirmations_follow_their_icons_and_escape_preserves_modes(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16.)));
    let mut workspace = None;
    let handle = cx.open_window(size(px(1300.), px(2000.)), |window, cx| {
        let view =
            cx.new(|cx| DeviceWorkspace::new(crate::demo::demo_keyboard(), true, window, cx));
        workspace = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = workspace.unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("dial-add", cx);
    })
    .unwrap();
    let uid = cx.update(|cx| view.read(cx).dial_highlight.clone().unwrap());
    cx.update_window(handle.into(), |_, window, cx| {
        let trigger = SharedString::from(format!("dial-delete-{uid}"));
        window.click(trigger.clone(), cx);
        let icon = window.find(trigger).bounds();
        let popup = window.find("dial-delete-confirmation").bounds();
        assert_eq!(popup.size.width, px(300.));
        assert_eq!(popup.top() - icon.top(), px(26.));
        assert_eq!(icon.right() - popup.right(), px(11.));
        assert_eq!(
            window.find("dial-delete-confirm").bounds().size,
            size(px(90.), px(27.))
        );
        assert!(window.try_find("dial-delete-cancel").is_none());
        assert!(
            view.read(cx)
                .keyboard_controls
                .dial_confirmation_focus
                .is_focused(window)
        );
        window.press("escape", cx);
        assert!(window.try_find("dial-delete-confirmation").is_none());

        window.click("dial-reset", cx);
        let icon = window.find("dial-reset").bounds();
        let popup = window.find("dial-reset-confirmation").bounds();
        assert_eq!(popup.size.width, px(300.));
        assert_eq!(popup.top() - icon.top(), px(42.));
        assert_eq!(popup.right(), icon.right());
        assert!(window.try_find("dial-reset-cancel").is_none());
        window.press("escape", cx);
        assert!(window.try_find("dial-reset-confirmation").is_none());
    })
    .unwrap();
    cx.update(|cx| {
        let workspace = view.read(cx);
        assert!(workspace.keyboard_controls.dial_confirmation.is_none());
        assert!(
            workspace
                .settings()
                .keyboard
                .dial_modes
                .iter()
                .any(|mode| mode.uid == uid)
        );
    });
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("dial-reset", cx);
        let old_target = view
            .read(cx)
            .keyboard_controls
            .dial_confirmation
            .clone()
            .unwrap();
        view.update(cx, |workspace, cx| workspace.add_profile(window, cx));
        window.render_frame(cx);
        assert!(!old_target.current(view.read(cx)));
        assert!(view.read(cx).keyboard_controls.dial_confirmation.is_none());
        assert!(window.try_find("dial-reset-confirmation").is_none());
    })
    .unwrap();
}

use super::{Shortcut, ShortcutOutput, Shortcuts, validate_shortcuts};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, px, size};

fn shortcut(id: &str) -> Shortcut {
    Shortcut {
        id: id.into(),
        input: "KEY_K".into(),
        modifiers: vec!["KEY_LEFT_CTRL".into()],
        hypershift: false,
        output: ShortcutOutput::Text {
            text: "你好\nshortcut".into(),
        },
    }
}

#[test]
fn shortcut_storage_rejects_invalid_or_duplicate_chords_without_losing_unicode() {
    let original = shortcut("first");
    let decoded: Shortcut =
        serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
    assert_eq!(decoded, original);
    assert!(validate_shortcuts(&[original.clone()]).is_ok());
    assert!(validate_shortcuts(&[original.clone(), shortcut("second")]).is_err());
    let mut invalid = original.clone();
    invalid.modifiers.push("KEY_LEFT_CTRL".into());
    assert!(invalid.validate().is_err());
    invalid = original.clone();
    invalid.input = "KEY_LEFT_SHIFT".into();
    assert!(invalid.validate().is_err());
    invalid = original;
    invalid.output = ShortcutOutput::Website {
        target: "javascript:alert(1)".into(),
    };
    assert!(invalid.validate().is_err());
    assert!(serde_json::from_str::<Shortcut>(r#"{"id":"a","input":"KEY_K","modifiers":[],"hypershift":false,"output":{"kind":"future"}}"#).is_err());
}

#[gpui_kit::test]
fn captures_saves_and_duplicates_a_global_shortcut_through_real_controls(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut shortcuts = None;
    let handle = cx.open_window(size(px(1280.), px(900.)), |w, cx| {
        let view = cx.new(|cx| Shortcuts::new(vec![], w, cx));
        shortcuts = Some(view.clone());
        Root::new(view, w, cx)
    });
    let view = shortcuts.unwrap();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("shortcut-add-card", cx);
        assert_eq!(w.find("shortcut-apply").disabled(), Some(true));
        w.click("shortcut-record", cx);
        w.press("ctrl-k", cx);
        w.within("shortcut-kind").click("input", cx);
        for _ in 0..4 {
            w.press("down", cx);
        }
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("shortcut-text", cx);
        w.input("你好快捷键", cx);
        w.click("shortcut-apply", cx);
        assert!(w.try_find("shortcut-editor").is_none());
    })
    .unwrap();
    cx.run_until_parked();
    let id = cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.items.len(), 1);
        assert_eq!(state.items[0].input, "KEY_K");
        assert_eq!(state.items[0].modifiers, ["CTRL"]);
        assert_eq!(
            state.items[0].output,
            ShortcutOutput::Text {
                text: "你好快捷键".into()
            }
        );
        state.items[0].id.clone()
    });
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("shortcut-check", cx);
        assert!(
            w.find("shortcut-encoding-status")
                .label()
                .unwrap()
                .contains("检查通过")
        );
        assert_eq!(w.find("shortcut-engine-apply").disabled(), Some(true));
        w.click(
            gpui_kit::SharedString::from(format!("shortcut-duplicate-{id}")),
            cx,
        );
        assert!(w.try_find("shortcut-encoding-status").is_none());
        assert_eq!(w.find("shortcut-check").disabled(), Some(true));
        assert_eq!(w.find("shortcut-apply").disabled(), Some(true));
        w.click("shortcut-record", cx);
        w.press("ctrl-k", cx);
        assert_eq!(w.find("shortcut-apply").disabled(), Some(true));
        assert!(
            w.find("shortcut-error")
                .label()
                .unwrap()
                .contains("已经分配")
        );
        w.click("shortcut-close", cx);
        w.click("shortcut-keep-editing", cx);
        assert!(w.try_find("shortcut-editor").is_some());
        w.click("shortcut-close", cx);
        w.click("shortcut-discard", cx);
        assert!(w.try_find("shortcut-editor").is_none());
    })
    .unwrap();
    cx.update(|cx| assert_eq!(view.read(cx).items.len(), 1));
}

#[gpui_kit::test]
fn generic_controls_preserve_legacy_storage_until_edit_and_default_empty_recordings(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let mut shortcuts = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |w, cx| {
        let view = cx.new(|cx| Shortcuts::new(vec![shortcut("existing")], w, cx));
        shortcuts = Some(view.clone());
        Root::new(view, w, cx)
    });
    let view = shortcuts.unwrap();
    cx.update(|cx| {
        assert_eq!(view.read(cx).snapshot()[0].modifiers, ["KEY_LEFT_CTRL"]);
        assert!(view.read(cx).snapshot()[0].chord().contains("左 Ctrl"));
        assert!(!view.read(cx).dirty());
    });
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("shortcut-edit-existing", cx);
        w.click("shortcut-modifier-CTRL", cx);
        w.click("shortcut-modifier-CTRL", cx);
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).draft.as_ref().unwrap().value.modifiers,
            ["CTRL"]
        )
    });
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("shortcut-record", cx);
        w.press("a", cx);
        w.click("shortcut-apply", cx);
    })
    .unwrap();
    cx.update(|cx| {
        let value = &view.read(cx).items[0];
        assert_eq!(value.input, "KEY_A");
        assert_eq!(value.modifiers, ["CTRL", "SHIFT"]);
    });
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("shortcut-edit-existing", cx);
        w.click("shortcut-modifier-CTRL", cx);
        w.click("shortcut-modifier-SHIFT", cx);
        w.within("shortcut-mouse").click("input", cx);
        w.press("down", cx);
        w.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).draft.as_ref().unwrap().value.modifiers,
            ["CTRL", "SHIFT"]
        )
    });
}

#[gpui_kit::test]
fn local_encoding_check_reports_native_dependency_without_mutating_saved_shortcuts(
    cx: &mut TestAppContext,
) {
    cx.update(gpui_kit::init);
    let mut item = shortcut("copy");
    item.output = ShortcutOutput::Windows {
        action: "Copy".into(),
    };
    let mut shortcuts = None;
    let handle = cx.open_window(size(px(1000.), px(850.)), |w, cx| {
        let view = cx.new(|cx| Shortcuts::new(vec![item.clone()], w, cx));
        shortcuts = Some(view.clone());
        Root::new(view, w, cx)
    });
    let view = shortcuts.unwrap();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("shortcut-check", cx);
        assert!(
            w.find("shortcut-encoding-status")
                .label()
                .unwrap()
                .contains("Turbo")
        );
        assert_eq!(w.find("shortcut-engine-apply").disabled(), Some(true));
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(view.read(cx).snapshot(), [item]);
        assert!(!view.read(cx).dirty());
    });
}

#[gpui_kit::test]
fn late_program_picker_cannot_change_a_reopened_draft_with_the_same_id(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut item = shortcut("program");
    item.output = ShortcutOutput::Program {
        target: r"C:\Apps\original.exe".into(),
    };
    let mut shortcuts = None;
    let handle = cx.open_window(size(px(1280.), px(900.)), |w, cx| {
        let view = cx.new(|cx| Shortcuts::new(vec![item.clone()], w, cx));
        shortcuts = Some(view.clone());
        Root::new(view, w, cx)
    });
    let view = shortcuts.unwrap();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("shortcut-edit-program", cx);
        w.click("shortcut-browse", cx);
        w.click("shortcut-cancel", cx);
        w.click("shortcut-edit-program", cx);
    })
    .unwrap();
    cx.simulate_path_prompt_response(|_| Some(vec![std::path::PathBuf::from(r"C:\Apps\late.exe")]));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(view.read(cx).draft.as_ref().unwrap().value, item));
}

#[gpui_kit::test]
fn delete_confirmation_and_discard_restore_saved_shortcuts(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let mut shortcuts = None;
    let handle = cx.open_window(size(px(700.), px(600.)), |w, cx| {
        let view = cx.new(|cx| Shortcuts::new(vec![shortcut("existing")], w, cx));
        shortcuts = Some(view.clone());
        Root::new(view, w, cx)
    });
    let view = shortcuts.unwrap();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("shortcut-delete-existing", cx);
        w.click("shortcut-delete-cancel", cx);
        assert!(w.try_find("shortcut-row-existing").is_some());
        w.click("shortcut-delete-existing", cx);
        w.click("shortcut-delete-confirm", cx);
        assert!(w.try_find("shortcut-row-existing").is_none());
        w.click("shortcuts-discard-all", cx);
        assert!(w.try_find("shortcut-row-existing").is_some());
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(view.read(cx).items, [shortcut("existing")]);
        assert!(!view.read(cx).dirty());
    });
}

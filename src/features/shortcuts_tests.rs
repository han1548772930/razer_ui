use super::{Shortcut, ShortcutOutput, Shortcuts, validate_shortcuts, validate_stored_shortcuts};
use gpui_kit::component::{Root, Theme};
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
    assert!(validate_stored_shortcuts(&[original.clone(), shortcut("second")]).is_ok());
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
    let previous = format!("{}end", "a".repeat(240));
    let changed = format!("{}{}end", "a".repeat(240), "🙂".repeat(5));
    let (range, limited) = super::limit_shortcut_text(&previous, &changed).unwrap();
    assert_eq!(range, 252..260);
    assert_eq!(limited, format!("{}{}end", "a".repeat(240), "🙂".repeat(3)));
    assert_eq!(limited.encode_utf16().count(), 249);
}

#[gpui_kit::test]
fn maps_an_output_then_records_and_duplicates_inline(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
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
        w.click("shortcut-category-TEXT_FUNCTION", cx);
        w.click("shortcut-text", cx);
        w.input("source shortcut", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, w, cx| {
        w.click("shortcut-apply", cx);
    })
    .unwrap();
    let id = cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.items.len(), 1);
        assert!(state.items[0].input.is_empty());
        assert!(validate_stored_shortcuts(&state.items).is_ok());
        assert!(validate_shortcuts(&state.items).is_err());
        state.items[0].id.clone()
    });
    cx.update_window(handle.into(), |_, w, cx| {
        w.click(
            gpui_kit::SharedString::from(format!("shortcut-record-{id}")),
            cx,
        );
        // Drive the source 100ms registration boundary explicitly in this
        // existing UI contract; no wall-clock sleep is needed.
        view.update(cx, |state, _| {
            state.recording_ready = Some(std::time::Instant::now())
        });
        w.press("ctrl-k", cx);
        assert_eq!(view.read(cx).items[0].input, "KEY_K");
        w.click(
            gpui_kit::SharedString::from(format!("shortcut-more-{id}")),
            cx,
        );
        w.click(
            gpui_kit::SharedString::from(format!("shortcut-duplicate-{id}")),
            cx,
        );
        assert!(w.try_find("shortcut-editor").is_none());
    })
    .unwrap();
    let duplicate = cx.update(|cx| {
        let state = view.read(cx);
        assert_eq!(state.items.len(), 2);
        assert_eq!(state.items[0].output, state.items[1].output);
        assert!(state.items[1].input.is_empty());
        state.items[1].id.clone()
    });
    cx.update_window(handle.into(), |_, w, cx| {
        w.click(
            gpui_kit::SharedString::from(format!("shortcut-record-{duplicate}")),
            cx,
        );
        view.update(cx, |state, _| {
            state.recording_ready = Some(std::time::Instant::now())
        });
        w.press("ctrl-k", cx);
        assert!(
            w.try_find(gpui_kit::SharedString::from(format!(
                "shortcut-warning-{duplicate}"
            )))
            .is_some()
        );
        assert!(validate_stored_shortcuts(&view.read(cx).items).is_ok());
        assert!(validate_shortcuts(&view.read(cx).items).is_err());
    })
    .unwrap();
}

#[gpui_kit::test]
fn recording_preserves_legacy_storage_until_input_and_defaults_empty_modifiers(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let mut shortcuts = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |w, cx| {
        let view = cx.new(|cx| Shortcuts::new(vec![shortcut("existing")], w, cx));
        shortcuts = Some(view.clone());
        Root::new(view, w, cx)
    });
    let view = shortcuts.unwrap();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        assert_eq!(view.read(cx).snapshot()[0].modifiers, ["KEY_LEFT_CTRL"]);
        w.click("shortcut-record-existing", cx);
        assert!(!view.read(cx).committed_pending());
        view.update(cx, |state, _| {
            state.recording_ready = Some(std::time::Instant::now())
        });
        w.press("a", cx);
        assert_eq!(view.read(cx).items[0].input, "KEY_A");
        assert_eq!(view.read(cx).items[0].modifiers, ["CTRL", "SHIFT"]);
        assert!(view.read(cx).committed_pending());
        view.update(cx, |state, cx| {
            state.record_input("ScrollButton".into(), gpui_kit::Modifiers::default(), cx)
        });
        assert_eq!(view.read(cx).items[0].input, "ScrollButton");
        assert_eq!(view.read(cx).items[0].modifiers, ["CTRL", "SHIFT"]);
    })
    .unwrap();
}

#[test]
fn pure_encoding_keeps_native_dependency_failures_out_of_the_ui() {
    let mut item = shortcut("copy");
    item.output = ShortcutOutput::Windows {
        action: "Copy".into(),
    };
    let before = item.clone();
    assert!(
        super::super::shortcut_engine::encode_shortcuts(&[item.clone()])
            .unwrap_err()
            .contains("Turbo")
    );
    assert_eq!(item, before);
}

#[gpui_kit::test]
fn late_program_picker_cannot_change_a_reopened_draft_with_the_same_id(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
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
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16.)));
    let mut shortcuts = None;
    let handle = cx.open_window(size(px(700.), px(600.)), |w, cx| {
        let view = cx.new(|cx| Shortcuts::new(vec![shortcut("existing")], w, cx));
        shortcuts = Some(view.clone());
        Root::new(view, w, cx)
    });
    let view = shortcuts.unwrap();
    cx.update_window(handle.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("shortcut-more-existing", cx);
        w.click("shortcut-delete-existing", cx);
        let row = w.find("shortcut-row-existing").bounds();
        let popup = w.find("shortcut-delete-confirmation").bounds();
        assert_eq!(popup.left() - row.left(), px(274.));
        assert_eq!(popup.top() - row.top(), px(53.));
        assert_eq!(popup.size.width, px(300.));
        assert!(w.try_find("shortcut-delete-cancel").is_none());
        w.press("escape", cx);
        assert!(w.try_find("shortcut-delete-confirmation").is_none());
        assert!(w.try_find("shortcut-row-existing").is_some());
        assert!(!view.read(cx).dirty());
        w.click("shortcut-more-existing", cx);
        w.click("shortcut-delete-existing", cx);
        w.click("shortcut-delete-confirm", cx);
        assert!(w.try_find("shortcut-row-existing").is_none());
        assert!(view.read(cx).committed_pending());
        view.update(cx, |state, cx| {
            state.items = state.saved.clone();
            state.changed(cx);
        });
        assert!(w.try_find("shortcut-row-existing").is_some());
    })
    .unwrap();
    cx.update(|cx| {
        assert_eq!(view.read(cx).items, [shortcut("existing")]);
        assert!(!view.read(cx).dirty());
    });
}

#[gpui_kit::test]
fn completing_an_older_shortcut_write_keeps_later_commits_and_drafts_pending(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let original = shortcut("existing");
    let mut shortcuts = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let view = cx.new(|cx| Shortcuts::new(vec![original.clone()], window, cx));
        shortcuts = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = shortcuts.unwrap();
    let first_write = cx
        .update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("shortcut-edit-existing", cx);
            view.update(cx, |state, cx| {
                state.set_output_value("A".into());
                state.changed(cx);
            });
            assert!(view.read(cx).draft_dirty());
            assert!(!view.read(cx).committed_pending());
            assert_eq!(view.read(cx).snapshot(), [original]);
            window.click("shortcut-apply", cx);
            assert!(view.read(cx).committed_pending());
            view.read(cx).snapshot()
        })
        .unwrap();
    let second_write = cx
        .update_window(handle.into(), |_, window, cx| {
            window.click("shortcut-edit-existing", cx);
            view.update(cx, |state, cx| {
                state.set_output_value("B".into());
                state.changed(cx);
            });
            window.click("shortcut-apply", cx);
            view.update(cx, |state, cx| state.mark_saved(first_write, cx));
            assert!(view.read(cx).committed_pending());
            assert_eq!(view.read(cx).saved_snapshot()[0].output.value(), "A");
            assert_eq!(view.read(cx).snapshot()[0].output.value(), "B");
            view.read(cx).snapshot()
        })
        .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("shortcut-edit-existing", cx);
        view.update(cx, |state, cx| {
            state.set_output_value("C".into());
            state.changed(cx);
        });
        view.update(cx, |state, cx| state.mark_saved(second_write, cx));
        assert!(!view.read(cx).committed_pending());
        assert!(view.read(cx).draft_dirty());
        assert!(view.read(cx).dirty());
        assert_eq!(view.read(cx).snapshot()[0].output.value(), "B");
        assert_eq!(
            view.read(cx).draft.as_ref().unwrap().value.output.value(),
            "C"
        );
    })
    .unwrap();
}

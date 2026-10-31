use super::*;

const PROMPT_KEY: &str = "TEXT_AI_REPHRASE_PROMPT";

fn text(value: &str) -> ActionItem {
    let mut action = ActionItem::new(ActionKind::Text);
    action.value = value.into();
    action
}

#[gpui_kit::test]
fn locale_refresh_uses_live_active_and_inactive_drafts_without_clearing_history(
    cx: &mut TestAppContext,
) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    file.entries[0].actions = vec![text("Saved first")];
    let mut second = file.entries[0].clone();
    second.id = 2;
    second.name = "Second".into();
    second.xml_guid = Some("97ff67b4-4c09-411b-a2d2-b6ec0812127e".into());
    second.actions = vec![text("Saved second")];
    file.entries.push(second);
    file.next_id = 3;
    let (page, library, handle) = open(cx, file);
    page.update(cx, |page, cx| {
        page.checkpoint_actions();
        page.actions[0].value = "Untranslated prompt".into();
        page.actions[0].translation_key = Some(PROMPT_KEY.into());
        page.selected_actions = vec![0];
        page.select_entry_now(2, cx);
        page.checkpoint_actions();
        page.actions[0].value = "Unsaved manual text".into();
        page.redo = vec![vec![text("Redo second")]];
        page.selected_actions = vec![0];
    });
    cx.update_window(handle.into(), |_, window, cx| {
        page.update(cx, |page, cx| {
            page.locale.clear();
            page.refresh_locale(window, cx);
        });
        let expected = tr(PROMPT_KEY);
        assert_ne!(expected, PROMPT_KEY);
        let current = page.read(cx);
        assert_eq!(current.actions[0].value, "Unsaved manual text");
        assert_eq!(current.saved_actions, current.actions);
        assert_eq!(current.undo, vec![vec![text("Saved second")]]);
        assert_eq!(current.redo, vec![vec![text("Redo second")]]);
        assert_eq!(current.selected_actions, vec![0]);
        assert!(current.can_save());
        let inactive = &current.inactive_drafts[&1];
        assert_eq!(inactive.actions[0].value, expected);
        assert_eq!(inactive.undo, vec![vec![text("Saved first")]]);
        assert_eq!(inactive.selected, vec![0]);
        assert!(inactive.actions_dirty);
        let snapshot = library.read(cx).snapshot();
        assert_eq!(snapshot.entries[0].actions, inactive.actions);
        assert_eq!(snapshot.entries[1].actions, current.actions);
        page.update(cx, |page, cx| page.select_entry_now(1, cx));
        assert_eq!(page.read(cx).actions[0].value, expected);
        assert_eq!(page.read(cx).undo, vec![vec![text("Saved first")]]);
        assert!(page.read(cx).can_save());
    })
    .unwrap();
}

#[gpui_kit::test]
fn translated_item_refresh_updates_an_open_text_draft_and_keeps_the_dialog(
    cx: &mut TestAppContext,
) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    file.entries[0].actions = vec![text("Untranslated prompt")];
    let (page, _, handle) = open(cx, file);
    cx.update_window(handle.into(), |_, window, cx| {
        page.update(cx, |page, cx| {
            page.actions[0].translation_key = Some(PROMPT_KEY.into());
            page.open_text_editor(0, window, cx);
            page.text_editor
                .update(cx, |input, cx| input.set_value("Dialog draft", window, cx));
            page.locale.clear();
            page.refresh_locale(window, cx);
        });
        let current = page.read(cx);
        assert_eq!(current.editing_action, Some(0));
        assert_eq!(
            current.text_editor.read(cx).value().as_ref(),
            tr(PROMPT_KEY)
        );
        assert_eq!(current.actions[0].value, tr(PROMPT_KEY));
        assert!(current.text_row(0, cx).borrow().armed);
        assert!(current.undo.is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn external_text_updates_keep_the_same_row_editor_and_replace_a_changed_source_identity(
    cx: &mut TestAppContext,
) {
    let mut file = MacroLibraryFile::default();
    file.append_source_documents(&[serde_json::json!({
        "guid":"e7d0313f-5a9e-450a-b15a-1e70b558b3fc","name":"Text macro",
        "macroPlaybackOption":"standard","macroEvents":[{"Type":"actionBar"},{"Type":4,"Id":"first","Text":"Original"}],
        "appEngine":{"events":[],"mouseMoveType":"none"}
    })]).unwrap();
    file.current = Some(1);
    file.tutorial = Tutorial::Complete;
    let (page, library, handle) = open(cx, file);
    cx.update_window(handle.into(), |_, window, cx| {
        page.update(cx, |page, cx| page.open_text_editor(0, window, cx));
    })
    .unwrap();
    library.update(cx, |library, cx| {
        let mut file = library.snapshot();
        file.entries[0].actions[0].value = "External text".into();
        file.entries[0].source_document.as_mut().unwrap()["macroEvents"][1]["Text"] =
            serde_json::json!("External text");
        library.replace(file, cx);
    });
    cx.update(|cx| {
        let current = page.read(cx);
        assert_eq!(current.editing_action, Some(0));
        assert_eq!(
            current.text_editor.read(cx).value().as_ref(),
            "External text"
        );
        assert!(current.text_row(0, cx).borrow().armed);
        current.text_row(0, cx).borrow_mut().category = 3;
    });
    library.update(cx, |library, cx| {
        let mut file = library.snapshot();
        file.entries[0].source_document.as_mut().unwrap()["macroEvents"][1]["Id"] =
            serde_json::json!("replacement");
        library.replace(file, cx);
    });
    cx.update(|cx| {
        assert!(page.read(cx).editing_action.is_none());
        assert_eq!(page.read(cx).text_row(0, cx).borrow().category, 0);
    });
}

#[gpui_kit::test]
fn saving_an_unchanged_text_value_removes_its_translation_identity(cx: &mut TestAppContext) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    let mut action = text(&tr(PROMPT_KEY));
    action.translation_key = Some(PROMPT_KEY.into());
    file.entries[0].actions = vec![action];
    let (page, _, handle) = open(cx, file);
    cx.update_window(handle.into(), |_, window, cx| {
        page.update(cx, |page, cx| page.open_text_editor(0, window, cx));
        page.update(cx, |page, cx| {
            page.text_editor.update(cx, |input, cx| {
                input.set_value("Temporary edit", window, cx)
            });
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        page.update(cx, |page, cx| {
            page.text_editor
                .update(cx, |input, cx| input.set_value(tr(PROMPT_KEY), window, cx));
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(page.read(cx).text_row(0, cx).borrow().armed);
        page.update(cx, |page, cx| page.save_text_editor(window, cx));
        assert_eq!(page.read(cx).actions[0].value, tr(PROMPT_KEY));
        assert!(page.read(cx).actions[0].translation_key.is_none());
        assert_eq!(
            page.read(cx).undo.last().unwrap()[0]
                .translation_key
                .as_deref(),
            Some(PROMPT_KEY)
        );
        assert!(page.read(cx).can_save());
        assert!(page.read(cx).editing_action.is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn emoji_loading_starts_on_open_and_cancels_when_the_panel_unmounts(cx: &mut TestAppContext) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    file.entries[0].actions = vec![text("Text")];
    let (page, _, handle) = open(cx, file);
    cx.update_window(handle.into(), |_, window, cx| {
        page.update(cx, |page, cx| page.open_text_editor(0, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(!page.read(cx).text_row(0, cx).borrow().emoji_loaded);
        page.update(cx, |page, cx| page.toggle_text_emoji(0, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(20));
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(!page.read(cx).text_row(0, cx).borrow().emoji_loaded);
        page.update(cx, |page, cx| page.toggle_text_emoji(0, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(20));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(!page.read(cx).text_row(0, cx).borrow().emoji_loaded);
        page.update(cx, |page, cx| page.toggle_text_emoji(0, window, cx));
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(29));
    cx.run_until_parked();
    cx.update(|cx| assert!(!page.read(cx).text_row(0, cx).borrow().emoji_loaded));
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(page.read(cx).text_row(0, cx).borrow().emoji_loaded);
        page.update(cx, |page, cx| {
            let row = page.text_row(0, cx);
            row.borrow_mut().query = "face".into();
            row.borrow_mut().applied_query = "face".into();
            row.borrow_mut().category = 3;
            page.toggle_text_emoji(0, window, cx);
            page.toggle_text_emoji(0, window, cx);
            let row = row.borrow();
            assert!(!row.emoji_loaded);
            assert!(row.query.is_empty());
            assert!(row.applied_query.is_empty());
            assert_eq!(row.category, 3);
            drop(row);
            page.cancel_text_editor(window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(30));
    cx.run_until_parked();
    cx.update(|cx| assert!(!page.read(cx).text_row(0, cx).borrow().emoji_loaded));
}

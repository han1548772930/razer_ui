//! Pointer regressions compiled by cargo check --all-targets; not run during source audits.
use super::{Entry, EntryKind, MacroPage, Tutorial, tr};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, ClipboardItem, Entity, TestAppContext, WindowHandle, px, size};
use razer_model::macro_library::{ActionItem, ActionKind};
use razer_pages::features::macro_library::MacroLibrary;
use razer_pages::features::macro_library::MacroLibraryFile;
use razer_pages::features::macro_library::MacroType;
use std::time::Duration;

fn open(
    cx: &mut TestAppContext,
    file: MacroLibraryFile,
) -> (Entity<MacroPage>, Entity<MacroLibrary>, WindowHandle<Root>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
    });
    let library = cx.new(|_| MacroLibrary::new(file));
    let mut page = None;
    let handle = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let entity = cx.new(|cx| MacroPage::new(library.clone(), window, cx));
        page = Some(entity.clone());
        Root::new(entity, window, cx)
    });
    (page.unwrap(), library, handle)
}

fn existing_file(kind: MacroType, tutorial: Tutorial) -> MacroLibraryFile {
    MacroLibraryFile {
        next_id: 2,
        entries: vec![Entry {
            id: 1,
            name: "Macro 1".into(),
            kind: EntryKind::Macro,
            parent: None,
            open: false,
            actions: vec![],
            macro_type: kind,
            active_phase: None,
            record_delay: 0,
            xml_guid: Some("d1ee0ca7-e747-42cc-929a-282c8cde55f2".into()),
            xml_mouse_mode: 0,
            source_document: None,
        }],
        current: Some(1),
        tutorial,
        record_shortcut: Vec::new(),
    }
}

#[gpui_kit::test]
fn tutorial_buttons_unlock_local_action_editing_and_save(cx: &mut TestAppContext) {
    let (page, library, handle) = open(cx, MacroLibraryFile::default());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("macro-new-link", cx);
        assert_eq!(page.read(cx).tutorial, Tutorial::Record);
        assert_eq!(library.read(cx).snapshot().tutorial, Tutorial::Record);

        let next = window.find("macro-onboarding-next");
        assert_eq!(next.label(), Some(tr("TEXT_MACRO_CONTENT_NEXT").as_str()));
        assert!(
            window
                .find("macro-item-list")
                .bounds()
                .contains(&next.bounds().center()),
            "the real tutorial button must overlap the disabled list"
        );
        window.click("macro-onboarding-next", cx);
        assert_eq!(page.read(cx).tutorial, Tutorial::Add);
        assert_eq!(library.read(cx).snapshot().tutorial, Tutorial::Add);
        assert_eq!(
            window.find("macro-onboarding-next").label(),
            Some(tr("TEXT_MACRO_CONTENT_DONE").as_str())
        );
        assert_eq!(window.find("macro-palette-delay").disabled(), Some(true));

        window.click("macro-onboarding-next", cx);
        assert_eq!(page.read(cx).tutorial, Tutorial::Complete);
        assert_eq!(library.read(cx).snapshot().tutorial, Tutorial::Complete);
        assert!(window.try_find("macro-onboarding").is_none());
        assert_eq!(window.find("macro-palette-delay").disabled(), None);

        window.click("macro-palette-delay", cx);
        assert_eq!(page.read(cx).actions.len(), 1);
        assert!(library.read(cx).snapshot().entries[0].actions.is_empty());
        window.click("macro-undo", cx);
        assert!(page.read(cx).actions.is_empty());
        window.click("macro-redo", cx);
        assert_eq!(page.read(cx).actions.len(), 1);
        window.click("macro-save", cx);
        assert_eq!(library.read(cx).snapshot().entries[0].actions.len(), 1);
        assert_eq!(window.find("macro-save").disabled(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn tutorial_skip_is_reachable_above_standard_and_phased_lists(cx: &mut TestAppContext) {
    for kind in [MacroType::Standard, MacroType::Phased] {
        for tutorial in [Tutorial::Record, Tutorial::Add] {
            let (page, library, handle) = open(cx, existing_file(kind, tutorial));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.click("macro-onboarding-skip", cx);
                assert_eq!(page.read(cx).tutorial, Tutorial::Complete);
                assert_eq!(library.read(cx).snapshot().tutorial, Tutorial::Complete);
                assert!(window.try_find("macro-onboarding").is_none());
            })
            .unwrap();
        }
    }
}

#[gpui_kit::test]
fn recording_settings_receive_clicks_above_the_disabled_list(cx: &mut TestAppContext) {
    let (page, library, handle) = open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("macro-record-options", cx);
        assert!(page.read(cx).record_ui.open);
        assert!(window.try_find("macro-record-settings").is_some());
        window.click(("macro-record-delay", 3usize), cx);
        assert_eq!(page.read(cx).record_delay(), 3);
        assert_eq!(library.read(cx).snapshot().entries[0].record_delay, 3);
        assert!(page.read(cx).record_ui.open);

        window.click(("macro-record-type", 1usize), cx);
        assert_eq!(page.read(cx).current_macro_type(), MacroType::Sequence);
        assert_eq!(library.read(cx).snapshot().entries[0].record_delay, 0);
        assert!(window.try_find(("macro-record-delay", 3usize)).is_none());
        window.click("macro-record-options", cx);
        assert!(!page.read(cx).record_ui.open);
        window.click("macro-palette-keyboard", cx);
        assert!(!page.read(cx).actions.is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn profile_commands_resume_after_settings_and_save_the_suspended_new_action(
    cx: &mut TestAppContext,
) {
    let (page, library, handle) = open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("macro-palette-delay", cx);
        window.click("macro-record-options", cx);
        for id in ["macro-new-link", "macro-selector", "macro-more"] {
            assert_eq!(window.find(id).disabled(), Some(true));
            window.click(id, cx);
        }
        assert_eq!(page.read(cx).current, Some(1));
        assert_eq!(page.read(cx).entries.len(), 1);
        assert!(page.read(cx).record_ui.open);
        assert!(page.read(cx).suspended_action.is_none());

        window.click("macro-record-options", cx);
        for id in ["macro-new-link", "macro-selector", "macro-more"] {
            assert_eq!(window.find(id).disabled(), None);
        }
        window.click("macro-more", cx);
        window.click("macro-menu-add", cx);
        assert!(window.try_find("macro-unsaved-dialog").is_some());
        assert_eq!(page.read(cx).current, Some(1));
        assert_eq!(page.read(cx).actions.len(), 1);
        assert!(library.read(cx).snapshot().entries[0].actions.is_empty());

        window.click("macro-unsaved-save", cx);
        assert!(window.try_find("macro-unsaved-dialog").is_none());
        assert_eq!(page.read(cx).current, Some(2));
        assert_eq!(page.read(cx).entries.len(), 2);
        assert!(page.read(cx).actions.is_empty());
        assert_eq!(library.read(cx).snapshot().entries[0].actions.len(), 1);
    })
    .unwrap();
}

#[gpui_kit::test]
fn unsaved_escape_keeps_draft_and_discard_resumes_new_without_saving(cx: &mut TestAppContext) {
    let (page, library, handle) = open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("macro-palette-delay", cx);
        window.click("macro-new-link", cx);
        assert!(window.try_find("macro-unsaved-dialog").is_some());
        window.press("escape", cx);
        assert!(window.try_find("macro-unsaved-dialog").is_none());
        assert_eq!(page.read(cx).current, Some(1));
        assert_eq!(page.read(cx).actions.len(), 1);
        assert_eq!(page.read(cx).entries.len(), 1);
        assert!(library.read(cx).snapshot().entries[0].actions.is_empty());

        window.click("macro-new-link", cx);
        window.click("macro-unsaved-discard", cx);
        assert!(window.try_find("macro-unsaved-dialog").is_none());
        assert_eq!(page.read(cx).current, Some(2));
        assert_eq!(page.read(cx).entries.len(), 2);
        assert!(page.read(cx).actions.is_empty());
        assert!(library.read(cx).snapshot().entries[0].actions.is_empty());
    })
    .unwrap();
}

#[gpui_kit::test]
fn profile_commands_stay_disabled_through_recording_lifecycle(cx: &mut TestAppContext) {
    use super::recording::Stage;
    let (page, _, handle) = open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    // Render-only state fixtures: no recorder, worker or DLL is started.
    for stage in [
        Stage::Countdown(2),
        Stage::Starting,
        Stage::Recording,
        Stage::Stopping,
    ] {
        page.update(cx, |page, _| page.recording.stage = stage);
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            for id in ["macro-new-link", "macro-selector", "macro-more"] {
                assert_eq!(window.find(id).disabled(), Some(true));
                window.click(id, cx);
            }
            assert_eq!(page.read(cx).entries.len(), 1);
            assert!(!page.read(cx).selector_open);
            assert!(!page.read(cx).more_open);
        })
        .unwrap();
    }
    page.update(cx, |page, _| page.recording.stage = Stage::Idle);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("macro-new-link", cx);
        assert_eq!(page.read(cx).entries.len(), 2);
    })
    .unwrap();
}

#[test]
fn action_input_coercion_matches_current_st_yt_and_ot() {
    for (kind, input, expected) in [
        (ActionKind::Delay, "", "0"),
        (ActionKind::Delay, "-2", "0"),
        (ActionKind::Delay, "999999", "99999.999"),
        (ActionKind::Delay, "0.12349", "0.123"),
        (ActionKind::Delay, "0.0625", "0.063"),
        (ActionKind::Delay, "1.0005", "1"),
        (ActionKind::Delay, "0.0000000000000000001", "0"),
        (ActionKind::Loop, "", "1"),
        (ActionKind::Loop, "invalid", "1"),
        (ActionKind::Loop, "0", "1"),
        (ActionKind::Loop, "100000", "99999"),
        (ActionKind::Loop, " 2.5 pasted", "2.5"),
        (ActionKind::Loop, "1e2", "100"),
        (ActionKind::Loop, "1e9999", "99999"),
        (ActionKind::Loop, "Infinity", "99999"),
        (ActionKind::Command, " echo ", " echo "),
        (ActionKind::Command, "  ", "  "),
    ] {
        assert_eq!(super::editors::committed_value(kind, input), expected);
    }
}

#[test]
fn randomized_action_draft_truncates_before_range_checks() {
    use super::DelayBound;
    for (min, max, changed, expected_min, expected_max) in [
        ("0.123", "4", DelayBound::Min, "0.12", "4"),
        ("4", "1.234", DelayBound::Max, "4", "5"),
        ("5.006", "5", DelayBound::Min, "5", "5"),
        ("", "1", DelayBound::Min, "", "1"),
        ("0x5", "1", DelayBound::Min, "0x5", "1"),
        ("0x5", "0.5", DelayBound::Max, "0x5", "5"),
    ] {
        assert!(super::editors::valid_randomized_draft(
            if changed == DelayBound::Min { min } else { max }
        ));
        assert_eq!(
            super::editors::normalize_randomized_range(min, max, changed),
            (expected_min.to_string(), expected_max.to_string())
        );
    }
    assert!(!super::editors::valid_randomized_draft("5.10"));
    assert!(!super::editors::valid_randomized_draft("0.1.2"));
}

#[gpui_kit::test]
fn macro_name_paste_truncates_utf16_and_escape_discards_the_name_draft(cx: &mut TestAppContext) {
    let original = format!("{}😀", "源".repeat(31));
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    file.entries[0].name = original.clone();
    let (page, library, handle) = open(cx, file);
    let expected = format!("{}😀", "甲".repeat(30));
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("macro-more", cx);
        window.click("macro-menu-rename", cx);
        assert_eq!(window.find("macro-rename-input").focused(), Some(true));
        assert_eq!(
            window.find("macro-rename-input").value(),
            Some(original.as_str())
        );
        cx.write_to_clipboard(ClipboardItem::new_string(format!("{expected}extra")));
        window.press("secondary-v", cx);
        assert_eq!(
            window.find("macro-rename-input").value(),
            Some(expected.as_str())
        );
        window.press("escape", cx);
        assert!(page.read(cx).rename.is_none());
        assert_eq!(library.read(cx).snapshot().entries[0].name, original);
        window.click("macro-more", cx);
        window.click("macro-menu-rename", cx);
        assert_eq!(
            window.find("macro-rename-input").value(),
            Some(original.as_str())
        );
        window.press("secondary-v", cx);
        window.press("enter", cx);
        assert!(page.read(cx).rename.is_none());
        assert_eq!(library.read(cx).snapshot().entries[0].name, expected);
    })
    .unwrap();
}

#[gpui_kit::test]
fn command_input_keeps_whitespace_through_enter_and_local_save(cx: &mut TestAppContext) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    let mut command = ActionItem::new(ActionKind::Command);
    command.value = "echo".into();
    file.entries[0].actions.push(command);
    let (page, library, handle) = open(cx, file);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(("macro-action-edit", 0usize), cx);
        window.input(" echo ", cx);
        assert_eq!(window.find("macro-save").disabled(), None);
        assert_eq!(page.read(cx).actions[0].value, "echo");
        window.press("enter", cx);
        assert!(page.read(cx).editing_action.is_none());
        assert_eq!(page.read(cx).actions[0].value, " echo ");
        window.click("macro-save", cx);
        assert_eq!(
            library.read(cx).snapshot().entries[0].actions[0].value,
            " echo "
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn whitespace_only_command_change_is_saved_from_the_pending_editor(cx: &mut TestAppContext) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    let mut command = ActionItem::new(ActionKind::Command);
    command.value = "echo".into();
    file.entries[0].actions.push(command);
    let (page, library, handle) = open(cx, file);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(("macro-action-edit", 0usize), cx);
        window.input(" echo ", cx);
        assert_eq!(page.read(cx).actions[0].value, "echo");
        window.click("macro-save", cx);
        assert_eq!(
            library.read(cx).snapshot().entries[0].actions[0].value,
            " echo "
        );
        assert!(page.read(cx).editing_action.is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn clearing_delay_keeps_the_previous_valid_timer(cx: &mut TestAppContext) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    let mut delay = ActionItem::new(ActionKind::Delay);
    delay.value = "0.25".into();
    file.entries[0].actions.push(delay);
    let (page, _, handle) = open(cx, file);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(("macro-delay-value", 0usize), cx);
        window.input("2", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("secondary-a", cx);
        window.press("backspace", cx);
        assert_eq!(
            window.find(("macro-numeric-input", 0usize)).value(),
            Some("")
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(99));
    cx.update(|cx| assert_eq!(page.read(cx).actions[0].value, "0.25"));
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(page.read(cx).actions[0].value, "2"));
}

#[gpui_kit::test]
fn command_timer_reads_current_text_at_the_first_pending_deadline(cx: &mut TestAppContext) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    let mut command = ActionItem::new(ActionKind::Command);
    command.value = "old".into();
    file.entries[0].actions.push(command);
    let (page, _, handle) = open(cx, file);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(("macro-action-edit", 0usize), cx);
        window.input("first", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("secondary-a", cx);
        window.input(" second ", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(99));
    cx.update(|cx| assert_eq!(page.read(cx).actions[0].value, "old"));
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(page.read(cx).actions[0].value, " second "));
}

#[gpui_kit::test]
fn loop_input_normalizes_and_updates_its_pair_after_150ms(cx: &mut TestAppContext) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    for state in ["start", "end"] {
        let mut item = ActionItem::new(ActionKind::Loop);
        item.state = state.into();
        item.value = "1".into();
        item.loop_pair_id = Some(7);
        file.entries[0].actions.push(item);
    }
    let (page, library, handle) = open(cx, file);
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click(("macro-loop-count", 0usize), cx);
        window.input("12", cx);
        assert_eq!(
            window.find(("macro-numeric-input", 0usize)).value(),
            Some("12")
        );
        assert_eq!(page.read(cx).actions[0].value, "1");
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.update_window(handle.into(), |_, window, cx| {
        window.press("secondary-a", cx);
        window.input("13", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(99));
    cx.update(|cx| assert_eq!(page.read(cx).actions[0].value, "1"));
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(page.read(cx).actions[0].value, "12");
        assert_eq!(page.read(cx).actions[1].value, "12");
        window.render_frame(cx);
        assert_eq!(
            window.find(("macro-numeric-input", 0usize)).value(),
            Some("12")
        );
        assert_eq!(library.read(cx).snapshot().entries[0].actions[0].value, "1");
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(50));
    cx.run_until_parked();
    cx.update_window(handle.into(), |_, window, cx| {
        assert_eq!(page.read(cx).actions[0].value, "13");
        assert_eq!(page.read(cx).actions[1].value, "13");
        window.render_frame(cx);
        assert_eq!(
            window.find(("macro-numeric-input", 0usize)).value(),
            Some("13")
        );
        window.press("escape", cx);
        assert!(page.read(cx).editing_action.is_none());
        window.click("macro-save", cx);
        assert!(
            library.read(cx).snapshot().entries[0]
                .actions
                .iter()
                .all(|item| item.value == "13")
        );
    })
    .unwrap();
}

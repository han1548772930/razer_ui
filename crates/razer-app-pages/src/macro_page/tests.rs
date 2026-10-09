//! Pointer regressions compiled by cargo check --all-targets; not run during source audits.
use super::{Entry, EntryKind, MacroPage, Tutorial, tr};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Entity, TestAppContext, WindowHandle, px, size};
use razer_pages::features::macro_library::MacroLibrary;
use razer_pages::features::macro_library::MacroLibraryFile;
use razer_pages::features::macro_library::MacroType;

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
        }],
        current: Some(1),
        tutorial,
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
        assert_eq!(window.find("macro-palette-delay").disabled(), Some(false));

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
            assert_eq!(window.find(id).disabled(), Some(false));
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

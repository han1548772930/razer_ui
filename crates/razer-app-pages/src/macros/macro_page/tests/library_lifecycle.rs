use super::super::{MacroTab, recording, recording_actor};
use super::*;

fn text(value: &str) -> ActionItem {
    let mut row = ActionItem::new(ActionKind::Text);
    row.value = value.into();
    row
}

fn two_macros(kind: MacroType) -> MacroLibraryFile {
    let mut file = existing_file(kind, Tutorial::Complete);
    file.entries[0].actions = vec![text("saved first")];
    let mut second = file.entries[0].clone();
    second.id = 2;
    second.xml_guid = Some("53b59960-d28e-487c-989b-623f84d24a0a".into());
    second.name = "Macro 2".into();
    second.actions = vec![text("saved second")];
    second.active_phase = (kind == MacroType::Phased).then_some(2);
    file.entries.push(second);
    file.next_id = 3;
    file
}

#[gpui_kit::test]
fn external_macros_replace_actions_preserve_guid_history_and_ignore_own_publications(
    cx: &mut TestAppContext,
) {
    let mut file = existing_file(MacroType::Standard, Tutorial::Complete);
    file.entries[0].actions = vec![text("saved")];
    let (page, library, _) = open(cx, file);
    page.update(cx, |page, cx| {
        page.actions = vec![text("unsaved")];
        page.undo = vec![vec![text("before")]];
        page.redo = vec![vec![text("after")]];
        page.selected_actions = vec![0];
        page.entries[0].record_delay = 1;
        page.publish_library(cx);
    });
    library.update(cx, |library, cx| library.mark_saved(library.snapshot(), cx));
    cx.update(|cx| assert_eq!(page.read(cx).actions[0].value, "unsaved"));
    library.update(cx, |library, cx| {
        let mut file = library.snapshot();
        file.entries[0].actions = vec![text("remote")];
        library.replace(file, cx);
    });
    cx.update(|cx| {
        let page = page.read(cx);
        assert_eq!(page.actions, vec![text("remote")]);
        assert_eq!(page.saved_actions, page.actions);
        assert_eq!(page.undo, vec![vec![text("before")]]);
        assert_eq!(page.redo, vec![vec![text("after")]]);
        assert!(page.selected_actions.is_empty());
        assert!(!page.can_save());
    });
    library.update(cx, |library, cx| {
        let mut file = library.snapshot();
        file.entries[0].xml_guid = Some("dc5993e7-d293-4476-a128-120884a06f32".into());
        library.replace(file, cx);
    });
    cx.update(|cx| {
        assert!(page.read(cx).undo.is_empty());
        assert!(page.read(cx).redo.is_empty());
    });
}

#[gpui_kit::test]
fn editor_only_selection_preserves_drafts_and_local_tutorial(cx: &mut TestAppContext) {
    let (page, library, _) = open(cx, two_macros(MacroType::Standard));
    page.update(cx, |page, _| {
        page.actions[0] = text("draft first");
        page.undo = vec![vec![text("saved first")]];
        page.selected_actions = vec![0];
    });
    library.update(cx, |library, cx| {
        let mut file = library.snapshot();
        file.current = Some(2);
        file.tutorial = Tutorial::Initial;
        library.replace(file, cx);
    });
    cx.update(|cx| {
        let page = page.read(cx);
        assert_eq!(page.current, Some(2));
        assert_eq!(page.tutorial, Tutorial::Complete);
        assert_eq!(page.actions[0].value, "saved second");
    });
    page.update(cx, |page, cx| page.select_entry_now(1, cx));
    cx.update(|cx| {
        let page = page.read(cx);
        assert_eq!(page.actions[0].value, "draft first");
        assert_eq!(page.undo, vec![vec![text("saved first")]]);
        assert_eq!(page.selected_actions, vec![0]);
        assert!(page.can_save());
    });
}

#[gpui_kit::test]
fn two_retained_pages_publish_from_the_latest_shared_library(cx: &mut TestAppContext) {
    let (first, library, first_window) =
        open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    let mut second = None;
    let second_window = cx.open_window(size(px(1280.), px(1000.)), |window, cx| {
        let page = cx.new(|cx| MacroPage::new(library.clone(), window, cx));
        second = Some(page.clone());
        Root::new(page, window, cx)
    });
    let second = second.unwrap();
    cx.update_window(first_window.into(), |_, window, cx| {
        first.update(cx, |page, cx| {
            page.create_entry_now(EntryKind::Macro, window, cx)
        });
    })
    .unwrap();
    cx.update_window(second_window.into(), |_, window, cx| {
        assert_eq!(second.read(cx).entries.len(), 2);
        second.update(cx, |page, cx| {
            page.create_entry_now(EntryKind::Folder, window, cx)
        });
    })
    .unwrap();
    cx.update(|cx| {
        let snapshot = library.read(cx).snapshot();
        assert_eq!(snapshot.entries.len(), 3);
        assert_eq!(first.read(cx).entries, snapshot.entries);
        assert_eq!(second.read(cx).entries, snapshot.entries);
        assert_eq!(snapshot.next_id, 4);
    });
}

#[gpui_kit::test]
fn profile_import_notifies_the_mounted_editor_before_its_next_save(cx: &mut TestAppContext) {
    let (page, library, window) = open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    page.update(cx, |page, _| {
        page.actions = vec![text("discarded by remote macroList")];
        page.undo = vec![vec![]];
    });
    let original = razer_model::product_preview::registered_preview(653).unwrap();
    let mut candidate = original.clone();
    candidate.profiles.push(razer_model::model::Profile {
        id: "imported-profile".into(),
        guid: "imported-profile".into(),
        name: "Imported".into(),
        settings: None,
        dpi_stages: None,
        source_settings: Some(serde_json::json!({"guid":"imported-profile","name":"Imported"})),
    });
    cx.update(|cx| {
        let before = library.read(cx).snapshot();
        let request = razer_model::profile_transfer::SourceProfileImport::new(&original, candidate)
            .unwrap()
            .with_source_macros(
                &before.mapping_catalog(),
                vec![serde_json::json!({
                    "guid":"d0b87d02-26f6-489f-97f4-41b6952e009d","name":"Imported macro",
                    "macroEvents":[],"macroPlaybackOption":"standard",
                    "appEngine":{"events":[],"hash":"source-engine","mouseMoveType":"none"}
                })],
            );
        let import = request.prepare_macros(&before).unwrap().unwrap();
        let merged = import.merge_receipt(&before).unwrap();
        library.update(cx, |library, cx| {
            library.publish_profile_import(&import, merged, cx)
        });
    });
    cx.update_window(window.into(), |_, window, cx| {
        assert_eq!(page.read(cx).entries.len(), 2);
        assert!(page.read(cx).actions.is_empty());
        assert_eq!(page.read(cx).undo, vec![Vec::<ActionItem>::new()]);
        page.update(cx, |page, cx| {
            page.create_entry_now(EntryKind::Folder, window, cx)
        });
        assert_eq!(
            library.read(cx).snapshot().entries[1].name,
            "Imported macro"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn phased_mount_runs_at_owner_transitions_and_render_keeps_business_state(cx: &mut TestAppContext) {
    let mut file = two_macros(MacroType::Phased);
    file.entries[0].active_phase = Some(2);
    let (page, library, window) = open(cx, file);
    cx.update(|cx| assert_eq!(library.read(cx).snapshot().entries[0].active_phase, Some(0)));
    page.update(cx, |page, cx| {
        page.phased_ui.expanded[1] = false;
        page.select_entry_now(2, cx);
        assert_eq!(page.active_phase(), Some(2));
        assert!(!page.phased_ui.expanded[1]);
        page.locale = "stale locale sentinel".into();
    });
    cx.update_window(window.into(), |_, window, cx| {
        let before = library.read(cx).snapshot();
        window.render_frame(cx);
        window.render_frame(cx);
        assert_eq!(library.read(cx).snapshot(), before);
        assert_eq!(page.read(cx).locale, "stale locale sentinel");
        page.update(cx, |page, cx| page.refresh_locale(window, cx));
        assert_eq!(page.read(cx).locale, razer_i18n::locale());
        page.update(cx, |page, cx| page.set_tab(MacroTab::Help, window, cx));
        page.update(cx, |page, cx| page.step_history(false, window, cx));
        assert_eq!(page.read(cx).active_phase(), Some(0));
        assert_eq!(page.read(cx).phased_ui.expanded, [true; 3]);
    })
    .unwrap();
}

#[gpui_kit::test]
fn recording_uses_the_external_current_document_and_keeps_commit_after_cleanup_failure(
    cx: &mut TestAppContext,
) {
    let (page, library, _) = open(cx, two_macros(MacroType::Standard));
    page.update(cx, |page, cx| {
        page.recording.stage = recording::Stage::Recording;
        page.accept_recording_update(
            recording_actor::Update::Preview {
                first_row: 0,
                rows: vec![text("preview")],
            },
            cx,
        );
    });
    library.update(cx, |library, cx| {
        let mut file = library.snapshot();
        file.current = Some(2);
        file.entries[1].actions = vec![text("remote second")];
        file.entries[1].macro_type = MacroType::Phased;
        file.entries[1].active_phase = Some(2);
        library.replace(file, cx);
    });
    library.update(cx, |library, cx| {
        let mut file = library.snapshot();
        file.entries[1].active_phase = Some(2);
        library.replace(file, cx);
    });
    page.update(cx, |page, cx| {
        assert!(page.recording_busy());
        assert_eq!(
            page.recording.preview,
            vec![text("remote second"), text("preview")]
        );
        let options = page.recording_options(cx).unwrap();
        assert_eq!(options.macro_type, MacroType::Phased);
        assert_eq!(options.phase, Some(2));
        page.undo = vec![vec![text("undo")]];
        page.redo = vec![vec![text("redo")]];
        page.selected_actions = vec![0];
        let mut row = ActionItem::new(ActionKind::Keyboard);
        row.keyboard = Some(razer_model::macro_library::KeyboardEvent {
            pair_id: Some(44),
            makecode: Some(65),
            state: Some(0),
            flag: Some(0),
            key_type: None,
        });
        row.phase = Some(1);
        assert_eq!(page.commit_recorded_actions(vec![row], cx).unwrap(), 1);
        assert_eq!(page.actions[1].phase, Some(1));
        assert_eq!(page.undo, vec![vec![text("undo")]]);
        assert_eq!(page.redo, vec![vec![text("redo")]]);
        assert_eq!(page.selected_actions, vec![0]);
        page.accept_recording_update(
            recording_actor::Update::Finished(Err("restore failed".into())),
            cx,
        );
        assert_eq!(page.actions.len(), 2);
        assert!(
            page.recording
                .error
                .as_ref()
                .unwrap()
                .contains("restore failed")
        );
        assert!(page.can_save());
    });
    cx.update(|cx| {
        assert_eq!(
            library.read(cx).snapshot().entries[1].actions,
            vec![text("remote second")]
        )
    });
}

#[gpui_kit::test]
fn empty_recording_marks_a_draft_and_cancellation_cannot_append(cx: &mut TestAppContext) {
    let (page, library, _) = open(cx, existing_file(MacroType::Standard, Tutorial::Complete));
    page.update(cx, |page, cx| {
        page.commit_recorded_actions(vec![], cx).unwrap();
        assert!(page.can_save());
        page.save_actions(cx);
        assert!(!page.can_save());
        page.cancel_recording(cx);
        assert!(
            page.commit_recorded_actions(vec![text("cancelled")], cx)
                .is_err()
        );
        assert!(page.actions.is_empty());
    });
    cx.update(|cx| assert!(library.read(cx).snapshot().entries[0].actions.is_empty()));
}

#[gpui_kit::test]
fn external_record_profile_updates_the_current_consumer_without_replacing_an_action_draft(cx: &mut TestAppContext) {
    use razer_model::macro_library::{MovementMode,recording::{DelayMode,Preferences,Seconds}};
    let (page,library,_) = open(cx,existing_file(MacroType::Standard,Tutorial::Complete));
    page.update(cx,|page,_| { page.actions=vec![text("unsaved")]; });
    let profile = Preferences {
        start_delay:4,fixed:Seconds::new(0.375).unwrap(),
        random:[Seconds::new(0.125).unwrap(),Seconds::new(0.75).unwrap()],
        movement:MovementMode::Relative,macro_type:MacroType::Sequence,delay_mode:DelayMode::Fixed,
    };
    library.update(cx,|library,cx| {
        let mut file=library.snapshot();
        file.recording_preferences=Some(profile.clone());
        library.replace(file,cx);
    });
    cx.update(|cx| {
        let page=page.read(cx);
        assert_eq!(page.actions,vec![text("unsaved")]);
        assert_eq!(page.record_preferences(),profile);
        let options=page.recording_commit_options(cx).unwrap();
        assert_eq!(options.macro_type,MacroType::Standard);
        assert_eq!(options.finalization_macro_type,MacroType::Sequence);
        assert_eq!(options.finalization_delay_mode,1);
        assert_eq!(options.movement_mode,3);
        assert_eq!(options.fixed,0.375);
        assert_eq!(options.random,[0.125,0.75]);
        assert_eq!(page.record_ui.start.read(cx).selected_value().map(String::as_str),Some("4s"));
    });
}

#[gpui_kit::test]
fn saving_and_reloading_keeps_the_commit_engine_lane_and_undo_rebuilds_from_actions(cx: &mut TestAppContext) {
    let (page,library,_) = open(cx,existing_file(MacroType::Phased,Tutorial::Complete));
    page.update(cx,|page,cx| {
        page.entries[0].active_phase=Some(2);
        let mut row=ActionItem::new(ActionKind::Keyboard);
        row.phase=Some(1);
        row.keyboard=Some(razer_model::macro_library::KeyboardEvent {
            pair_id:Some(1),makecode:Some(65),state:Some(0),flag:Some(0),key_type:None,
        });
        page.commit_recorded_actions(vec![row],cx).unwrap();
        assert!(page.save_actions(cx));
    });
    let saved=cx.update(|cx|library.read(cx).snapshot());
    let documents=saved.source_macro_documents().unwrap();
    assert_eq!(documents[0]["macroEvents"][1]["phase"],1);
    assert_eq!(documents[0]["appEngine"]["holdEvents"],serde_json::json!([]));
    assert_eq!(documents[0]["appEngine"]["releaseEvents"],serde_json::json!([{"type":"keyboard","scancode":30,"flag":0}]));
    let decoded:MacroLibraryFile=serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
    let (reloaded,reloaded_library,_) = open(cx,decoded);
    reloaded.update(cx,|page,cx| {
        assert_eq!(page.actions[0].phase,Some(1));
        let before=page.actions.clone();
        page.add_action("text",cx);
        assert_eq!(page.actions.len(),before.len()+1);
        page.undo_action(cx);
        assert_eq!(page.actions,before);
        // An actual undo dispatch rebuilds appEngine even when macroEvents
        // matches a previously saved snapshot after the history transition.
        assert!(page.can_save());
        assert!(page.save_actions(cx));
    });
    cx.update(|cx| {
        let documents=reloaded_library.read(cx).snapshot().source_macro_documents().unwrap();
        assert_eq!(documents[0]["appEngine"]["holdEvents"],serde_json::json!([{"type":"keyboard","scancode":30,"flag":0}]));
        assert_eq!(documents[0]["appEngine"]["releaseEvents"],serde_json::json!([]));
    });
}

#[gpui_kit::test]
fn stale_recording_updates_cannot_finish_or_overwrite_the_current_owner(cx: &mut TestAppContext) {
    let (page,_,_) = open(cx,existing_file(MacroType::Standard,Tutorial::Complete));
    page.update(cx,|page,cx| {
        page.recording.stage=recording::Stage::Recording;
        page.recording.count=4;
        page.receive_recording_update(u64::MAX,recording_actor::Update::Preview {first_row:0,rows:vec![text("stale")]},cx);
        page.receive_recording_update(u64::MAX,recording_actor::Update::Finished(Err("stale failure".into())),cx);
        assert!(page.recording_busy());
        assert_eq!(page.recording.count,4);
        assert!(page.recording.preview.is_empty());
        assert!(page.recording.error.is_none());
        page.receive_recording_update(0,recording_actor::Update::Progress(5),cx);
        assert_eq!(page.recording.count,5);
    });
}

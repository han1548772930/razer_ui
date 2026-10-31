use super::*;

fn document(kind: MacroType) -> Value {
    json!({
        "guid":"8b2a6c4b-80b0-4a90-b146-b54ae81ebf0e", "name":"Existing macro",
        "macroPlaybackOption":match kind {MacroType::Standard=>"standard",MacroType::Sequence=>"sequence",MacroType::Phased=>"phased"},
        "activePhase":2,"delaySetting":3,
        "macroEvents":[{"Type":"actionBar","recordProfile":{},"externalBar":true},
            {"Type":1,"Id":"source-key","KeyEvent":{"Makecode":65,"State":0},"flag":0,"ScanCode":30,"phase":0,"externalRow":"kept"}],
        "appEngine":{"events":[{"type":"keyboard","scancode":30,"flag":1,"externalEvent":17}],
            "pressEvents":[{"type":"delay","ms":31}],"holdEvents":[{"type":"delay","ms":41}],"releaseEvents":[],
            "mouseMoveType":"absolute","hash":"source-unchanged","externalEngine":{"value":9}},
        "externalDocument":["preserved"]
    })
}

fn library(kind: MacroType) -> MacroLibraryFile {
    let mut library = MacroLibraryFile::default();
    library.append_source_documents(&[document(kind)]).unwrap();
    library
}

fn options(kind: MacroType, movement: u8) -> recording::Options {
    recording::Options {
        macro_type: kind,
        phase: Some(1),
        delay_mode: 3,
        fixed: 0.25,
        random: [0.125, 0.75],
        next_pair: 2,
        movement_mode: movement,
        shortcut: vec![],
        finalization_macro_type: kind,
        finalization_delay_mode: 3,
    }
}

fn key() -> ActionItem {
    let mut key = ActionItem::new(ActionKind::Keyboard);
    key.phase = Some(1);
    key.keyboard = Some(KeyboardEvent {
        pair_id: Some(2),
        makecode: Some(66),
        state: Some(0),
        flag: Some(0),
        key_type: None,
    });
    key
}

#[test]
fn language_refresh_rebuilds_the_live_list_from_the_first_changed_macro() {
    let first = document(MacroType::Standard);
    let mut translated = document(MacroType::Standard);
    translated["guid"] = json!("f44b9998-40cb-4227-91b1-e409d3f7cf7a");
    translated["macroEvents"][1] = json!({
        "Type":4,"Text":"Original prompt","translationKey":"TEXT_AI_REPHRASE_PROMPT"
    });
    let mut following = document(MacroType::Standard);
    following["guid"] = json!("5e4f35a2-cab3-47de-8d4d-ec4e9832f738");
    let mut file = MacroLibraryFile::default();
    file.append_source_documents(&[first.clone(), translated, following])
        .unwrap();
    assert!(
        file.translate_text_actions(|key| {
            assert_eq!(key, "TEXT_AI_REPHRASE_PROMPT");
            Some("Translated prompt".into())
        })
        .unwrap()
    );
    let output = file.source_macro_documents().unwrap();
    assert_eq!(output[0]["appEngine"], first["appEngine"]);
    assert_eq!(output[0]["macroEvents"], first["macroEvents"]);
    assert_eq!(output[1]["macroEvents"][1]["Text"], "Translated prompt");
    assert_eq!(
        output[1]["macroEvents"][1]["translationKey"],
        "TEXT_AI_REPHRASE_PROMPT"
    );
    assert_eq!(output[1]["appEngine"]["events"][0]["type"], "clipboard");
    assert_eq!(
        output[1]["appEngine"]["events"][0]["text"],
        "Translated prompt"
    );
    assert_eq!(
        output[2]["appEngine"]["events"],
        json!([{"type":"keyboard","scancode":30,"flag":0}])
    );
    for document in &output[1..] {
        assert_eq!(document["appEngine"]["mouseMoveType"], "absolute");
        assert_eq!(document["macroEvents"][0]["recordProfile"]["mmtSetting"], 1);
        assert!(document["appEngine"].get("externalEngine").is_none());
        assert!(document["appEngine"].get("pressEvents").is_none());
        assert!(document["appEngine"].get("holdEvents").is_none());
        assert!(document["appEngine"].get("releaseEvents").is_none());
    }
    let unchanged = file.clone();
    assert!(
        !file
            .translate_text_actions(|_| Some("Translated prompt".into()))
            .unwrap()
    );
    assert_eq!(file, unchanged);
    assert!(
        !file
            .translate_text_actions(|_| Some(String::new()))
            .unwrap()
    );
    assert_eq!(file, unchanged);
    assert!(!file.translate_text_actions(|_| None).unwrap());
    assert_eq!(file, unchanged);
}

#[test]
fn translated_phased_engine_preserves_absent_ordinary_events_and_filters_unphased_rows() {
    let mut source = document(MacroType::Phased);
    source["appEngine"]
        .as_object_mut()
        .unwrap()
        .remove("events");
    source["macroEvents"] = json!([
        {"Type":"actionBar","recordProfile":{"mmtSetting":0}},
        {"Type":4,"Text":"Original","translationKey":"TEXT_AI_REPHRASE_PROMPT","phase":0},
        {"Type":4,"Text":"Unassigned"},
        {"Type":4,"Text":"Release","phase":2}
    ]);
    let mut file = MacroLibraryFile::default();
    file.append_source_documents(&[source]).unwrap();
    assert!(
        file.entries[0]
            .editor_execution()
            .unwrap()
            .engine()
            .ordinary_events()
            .is_err()
    );
    file.translate_text_actions(|_| Some("Translated".into()))
        .unwrap();
    let entry = &file.entries[0];
    let engine = serde_json::to_value(entry.execution.as_ref().unwrap().engine()).unwrap();
    assert!(engine.get("events").is_none());
    assert_eq!(engine["mouseMoveType"], "none");
    assert_eq!(engine["pressEvents"].as_array().unwrap().len(), 1);
    assert_eq!(engine["pressEvents"][0]["text"], "Translated");
    assert_eq!(engine["holdEvents"], json!([]));
    assert_eq!(engine["releaseEvents"].as_array().unwrap().len(), 1);
    assert_eq!(engine["releaseEvents"][0]["text"], "Release");
    let mut invalidated = entry.execution.clone().unwrap();
    invalidated.invalidate();
    let edited = file
        .execution_for_actions(entry, &entry.actions, Some(&invalidated), 0)
        .unwrap();
    assert_eq!(serde_json::to_value(edited.engine()).unwrap(), engine);
    assert!(
        file.source_macro_documents().unwrap()[0]["appEngine"]
            .get("events")
            .is_none()
    );
    let error = file
        .append_recorded_actions(
            entry,
            &entry.actions,
            entry.execution.as_ref(),
            vec![],
            &options(MacroType::Phased, 0),
        )
        .err()
        .unwrap();
    assert!(error.contains("ordinary events are absent"));
}

#[test]
fn translation_failure_keeps_all_prior_actions_and_engines_unchanged() {
    let mut file = library(MacroType::Standard);
    let mut row = ActionItem::new(ActionKind::Text);
    row.value = "Original".into();
    row.translation_key = Some("TEXT_AI_REPHRASE_PROMPT".into());
    file.entries[0].actions = vec![row];
    let mut following = file.entries[0].clone();
    following.id = 2;
    following.xml_guid = Some("8b394257-b69c-46ea-b7e0-97d13106e53e".into());
    let mut missing_physical_key = ActionItem::new(ActionKind::Keyboard);
    missing_physical_key.value = "A".into();
    following.actions = vec![missing_physical_key];
    file.entries.push(following);
    file.next_id = 3;
    let unchanged = file.clone();
    let error = file
        .translate_text_actions(|_| Some("Translated".into()))
        .unwrap_err();
    assert!(error.contains("physical key metadata"));
    assert_eq!(file, unchanged);
}

#[test]
fn text_translation_identity_roundtrips_locally_and_invalid_external_types_fail() {
    let mut source = document(MacroType::Standard);
    source["macroEvents"][1] =
        json!({"Type":4,"Text":"Prompt","translationKey":"TEXT_AI_REPHRASE_PROMPT"});
    let mut file = MacroLibraryFile::default();
    file.append_source_documents(&[source.clone()]).unwrap();
    let decoded: MacroLibraryFile =
        serde_json::from_slice(&serde_json::to_vec(&file).unwrap()).unwrap();
    assert_eq!(
        decoded.entries[0].actions[0].translation_key.as_deref(),
        Some("TEXT_AI_REPHRASE_PROMPT")
    );
    assert_eq!(
        decoded.source_macro_documents().unwrap()[0]["macroEvents"][1]["translationKey"],
        "TEXT_AI_REPHRASE_PROMPT"
    );
    let encoded = xml::encode(&decoded.entries[0], &decoded.entries).unwrap();
    assert!(encoded.contains("<translationKey>TEXT_AI_REPHRASE_PROMPT</translationKey>"));
    assert!(
        xml::decode(&encoded).unwrap().actions[0]
            .translation_key
            .is_none()
    );
    source["macroEvents"][1]["translationKey"] = json!(7);
    assert!(
        MacroLibraryFile::default()
            .append_source_documents(&[source])
            .is_err()
    );
}

#[test]
fn recording_appends_to_current_engine_phase_without_rewriting_payload_or_old_lanes() {
    let mut file = library(MacroType::Phased);
    let entry = &file.entries[0];
    let append = file
        .append_recorded_actions(
            entry,
            &entry.actions,
            None,
            vec![key()],
            &options(MacroType::Standard, 3),
        )
        .unwrap();
    assert_eq!(append.actions.last().unwrap().phase, Some(1));
    let engine = serde_json::to_value(append.execution.engine()).unwrap();
    assert_eq!(
        engine["events"],
        document(MacroType::Phased)["appEngine"]["events"]
    );
    assert_eq!(engine["pressEvents"], json!([{"type":"delay","ms":31}]));
    assert_eq!(engine["holdEvents"], json!([{"type":"delay","ms":41}]));
    assert_eq!(
        engine["releaseEvents"],
        json!([{"type":"keyboard","scancode":48,"flag":0}])
    );
    assert_eq!(engine["mouseMoveType"], "relative");
    file.entries[0].actions = append.actions;
    file.entries[0].execution = Some(append.execution);
    let saved = file.source_macro_documents().unwrap().remove(0);
    assert_eq!(saved["appEngine"], engine);
    assert_eq!(saved["macroEvents"][2]["phase"], 1);
    assert_eq!(saved["macroEvents"][1]["externalRow"], "kept");
    assert_eq!(saved["macroEvents"][0]["externalBar"], true);
    assert_eq!(saved["externalDocument"], json!(["preserved"]));
}

#[test]
fn saved_execution_survives_reload_metadata_changes_and_external_extensions() {
    let mut file = library(MacroType::Standard);
    let entry = &file.entries[0];
    let append = file
        .append_recorded_actions(
            entry,
            &entry.actions,
            None,
            vec![key()],
            &options(MacroType::Standard, 2),
        )
        .unwrap();
    let engine = serde_json::to_value(append.execution.engine()).unwrap();
    file.entries[0].actions = append.actions;
    file.entries[0].execution = Some(append.execution);
    let bytes = serde_json::to_vec(&file).unwrap();
    let mut reloaded: MacroLibraryFile = serde_json::from_slice(&bytes).unwrap();
    reloaded.validate().unwrap();
    reloaded.entries[0].name = "Renamed".into();
    reloaded.entries[0].active_phase = Some(1);
    reloaded.entries[0].record_delay = 2;
    let document = reloaded.source_macro_documents().unwrap().remove(0);
    assert_eq!(document["appEngine"], engine);
    assert_eq!(document["appEngine"]["externalEngine"], json!({"value":9}));
    assert_eq!(document["appEngine"]["events"][0]["externalEvent"], 17);
    assert_eq!(document["name"], "Renamed");
    assert_eq!(document["delaySetting"], 2);
}

#[test]
fn actual_action_edit_and_undo_rebuild_the_current_lane_even_when_rows_match_again() {
    let file = library(MacroType::Standard);
    let entry = &file.entries[0];
    let mut snapshot = entry.editor_execution().unwrap();
    let retained = file
        .execution_for_actions(entry, &entry.actions, Some(&snapshot), 0)
        .unwrap();
    assert_eq!(
        serde_json::to_value(retained.engine()).unwrap()["events"][0]["flag"],
        1
    );
    snapshot.invalidate();
    let rebuilt = file
        .execution_for_actions(entry, &entry.actions, Some(&snapshot), 0)
        .unwrap();
    let engine = serde_json::to_value(rebuilt.engine()).unwrap();
    assert_eq!(
        engine["events"],
        json!([{"type":"keyboard","scancode":30,"flag":0}])
    );
    assert_eq!(engine["holdEvents"], json!([{"type":"delay","ms":41}]));
    assert_eq!(engine["mouseMoveType"], "absolute");
}

#[test]
fn stale_execution_baseline_cannot_replace_changed_actions() {
    let mut file = library(MacroType::Standard);
    file.entries[0].execution = Some(file.entries[0].editor_execution().unwrap());
    file.entries[0].actions[0].keyboard.as_mut().unwrap().state = Some(1);
    file.entries[0].actions[0]
        .keyboard
        .as_mut()
        .unwrap()
        .makecode = Some(66);
    file.entries[0].actions[0].recorded_input = None;
    let engine = file.source_macro_documents().unwrap().remove(0)["appEngine"].clone();
    assert_eq!(
        engine["events"],
        json!([{"type":"keyboard","scancode":48,"flag":1}])
    );
}

#[test]
fn empty_phased_recording_preserves_lanes_and_applies_the_source_normal_lane_mode_rule() {
    let mut source = document(MacroType::Phased);
    source["appEngine"]["events"] = json!([]);
    let mut file = MacroLibraryFile::default();
    file.append_source_documents(&[source.clone()]).unwrap();
    let entry = &file.entries[0];
    let append = file
        .append_recorded_actions(
            entry,
            &entry.actions,
            None,
            vec![],
            &options(MacroType::Phased, 3),
        )
        .unwrap();
    assert_eq!(append.appended, 0);
    assert_eq!(append.actions, entry.actions);
    let engine = serde_json::to_value(append.execution.engine()).unwrap();
    assert_eq!(engine["mouseMoveType"], "none");
    assert_eq!(engine["pressEvents"], source["appEngine"]["pressEvents"]);
    assert_eq!(engine["holdEvents"], source["appEngine"]["holdEvents"]);
}

#[test]
fn current_movement_mode_compiles_only_new_coordinates_and_keeps_the_recorded_prefix() {
    let mut source = document(MacroType::Standard);
    source["macroEvents"] = json!([
        {"Type":"actionBar","recordProfile":{"mmtSetting":1}},
        {"Type":3,"Number":"0.001","MouseEvent":{"Buffer":[{"x":640,"y":360,"time":1}]}}
    ]);
    source["appEngine"]["events"] = json!([{"type":"mouseMove","x":32123,"y":45678}]);
    let mut file = MacroLibraryFile::default();
    file.append_source_documents(&[source]).unwrap();
    let mut row = ActionItem::new(ActionKind::Mouse);
    row.value = "0.001".into();
    row.mouse_movement = Some(MouseMovement {
        imported_xml: false,
        mode: 1,
        buffer: vec![json!({"x":20,"y":30,"time":1})],
        geometry: Some(RecordingGeometry {
            monitors_before: json!([{"isPrimaryMonitor":true,"MonitorRect":{"left":0,"top":0,"right":100,"bottom":100},"dpiScaleX":2,"dpiScaleY":2}]),
            monitors_after: json!([]),
            screen_before: json!({}),
            screen_after: json!({}),
        }),
    });
    let entry = &file.entries[0];
    let append = file
        .append_recorded_actions(
            entry,
            &entry.actions,
            None,
            vec![row.clone()],
            &options(MacroType::Standard, 3),
        )
        .unwrap();
    let engine = serde_json::to_value(append.execution.engine()).unwrap();
    assert_eq!(
        engine["events"][0],
        json!({"type":"mouseMove","x":32123,"y":45678})
    );
    assert_eq!(engine["events"][1]["x"].as_f64(), Some(10.));
    assert_eq!(engine["events"][1]["y"].as_f64(), Some(15.));
    assert_eq!(engine["mouseMoveType"], "relative");
    assert_eq!(append.actions[0].mouse_movement.as_ref().unwrap().mode, 1);
    assert_eq!(append.actions[1].mouse_movement.as_ref().unwrap().mode, 3);
    let disabled = file
        .append_recorded_actions(
            entry,
            &entry.actions,
            None,
            vec![row],
            &options(MacroType::Standard, 0),
        )
        .unwrap();
    assert_eq!(disabled.appended, 0);
    let engine = serde_json::to_value(disabled.execution.engine()).unwrap();
    assert_eq!(engine["events"].as_array().unwrap().len(), 1);
    assert_eq!(engine["mouseMoveType"], "none");
    for (append, expected_mode, expected_profile) in
        [(append, "relative", "3"), (disabled, "none", "1")]
    {
        let mut exported = entry.clone();
        exported.actions = append.actions;
        exported.execution = Some(append.execution);
        let output = xml::encode(&exported, std::slice::from_ref(&exported)).unwrap();
        let document = roxmltree::Document::parse(&output).unwrap();
        assert_eq!(
            document
                .descendants()
                .find(|node| node.has_tag_name("MouseMoveType"))
                .and_then(|node| node.text()),
            Some(expected_mode)
        );
        assert_eq!(
            document
                .descendants()
                .find(|node| node.has_tag_name("mmtSetting"))
                .and_then(|node| node.text()),
            Some(expected_profile)
        );
        assert_eq!(
            document
                .descendants()
                .find(|node| node.has_tag_name("x"))
                .and_then(|node| node.text()),
            Some("640")
        );
    }
}

#[test]
fn missing_or_unknown_engine_fields_fail_at_the_retained_boundary() {
    for engine in [
        json!({"mouseMoveType":"none"}),
        json!({"events":[],"mouseMoveType":"unknown"}),
        json!({"events":[{"type":"keyboard","flag":0}]}),
    ] {
        let mut file = library(MacroType::Standard);
        file.entries[0].source_document.as_mut().unwrap()["appEngine"] = engine;
        assert!(file.entries[0].editor_execution().is_err());
    }
}

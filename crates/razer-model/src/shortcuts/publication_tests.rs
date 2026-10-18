use super::*;
use crate::{
    macro_library::{ActionItem, ActionKind, Entry, EntryKind, MacroType},
    shortcuts::ShortcutOutput,
};
use serde_json::json;

fn shortcut(output: ShortcutOutput) -> Shortcut {
    Shortcut::new(
        "shortcut-1".into(),
        "KEY_K".into(),
        vec!["CTRL".into()],
        false,
        output,
    )
}

#[test]
fn defaults_are_complete_stable_and_reference_real_event_collections() {
    let mut compiler = Compiler::new();
    let library = MacroLibraryFile::default();
    let first = compiler
        .compile(
            &[shortcut(ShortcutOutput::Windows {
                action: "Copy".into(),
            })],
            &library,
        )
        .unwrap();
    assert_eq!(first.default_turbos().len(), 38);
    let copy = first
        .default_turbos()
        .iter()
        .find(|v| v["name"] == "Copy")
        .unwrap();
    // Dashboard 51480's D.getData, selected by createEvent("Copy").
    assert_eq!(
        copy["appEngine"]["events"],
        json!([
            {"type":"keyboard","scancode":29,"flag":0},
            {"type":"keyboard","scancode":46,"flag":0},
            {"type":"delay","ms":10},
            {"type":"keyboard","scancode":46,"flag":1},
            {"type":"keyboard","scancode":29,"flag":1}
        ])
    );
    assert_eq!(
        first.app_engine()["mappings"][0]["output"],
        json!({"type":"turbo","guid":copy["guid"],"flag":0,"repeat":1})
    );
    let second = compiler.compile(&[], &library).unwrap();
    assert_eq!(first.default_turbos(), second.default_turbos());
    assert_eq!(
        second.app_engine()["hash"],
        "de2ccb2014607a84df08306567cd96f0"
    );
}

fn library() -> MacroLibraryFile {
    let mut action = ActionItem::new(ActionKind::Text);
    action.value = "Local macro".into();
    let mut library = MacroLibraryFile::default();
    library.next_id = 2;
    library.entries.push(Entry {
        id: 1,
        name: "Local macro".into(),
        kind: EntryKind::Macro,
        parent: None,
        open: false,
        actions: vec![action],
        macro_type: MacroType::Standard,
        active_phase: None,
        record_delay: 0,
        xml_guid: Some("12345678-1234-4234-8234-123456789abc".into()),
        xml_mouse_mode: 0,
        source_document: None,
    });
    library
}

#[test]
fn macro_identity_resolves_local_id_and_preserves_consumer_contracts() {
    let library = library();
    let item = shortcut(ShortcutOutput::Macro {
        macro_id: 1,
        name: "Old display name".into(),
        playback: "NTimes".into(),
        repeat_count: 3,
    });
    let document = Compiler::new().compile(&[item], &library).unwrap();
    assert!(document.phased_macros().is_empty());
    assert_eq!(
        document.macros()[0]["appEngine"]["events"],
        json!([
            {"type":"clipboard","id":"text","text":"Local macro"},
            {"type":"delay","ms":500}
        ])
    );
    assert_eq!(
        document.app_engine()["mappings"][0]["output"],
        json!({"type":"macro","guid":"12345678-1234-4234-8234-123456789abc","flag":0,"toggle":0,"repeat":3})
    );
}

#[test]
fn invalid_dependencies_and_ipc_corruption_cannot_be_published() {
    let mut compiler = Compiler::new();
    let good = compiler.compile(&[], &library()).unwrap();
    let missing = shortcut(ShortcutOutput::Macro {
        macro_id: 2,
        name: "Deleted".into(),
        playback: "Once".into(),
        repeat_count: 1,
    });
    assert!(compiler.compile(&[missing], &library()).is_err());
    assert!(
        compiler
            .compile(
                &[shortcut(ShortcutOutput::Windows {
                    action: "LockComputer".into()
                })],
                &library()
            )
            .is_err()
    );
    assert_eq!(compiler.compile(&[], &library()).unwrap(), good);
    let mut wire = serde_json::to_value(&good).unwrap();
    wire["app_engine"]["mappings"] = json!([{"input":{"type":"keyboard","scancode":30,"flag":0},"output":{"type":"macro","guid":"absent","flag":0}}]);
    wire["app_engine"]["hash"] = source_hash(&wire["app_engine"]).unwrap().into();
    assert!(serde_json::from_value::<CompiledDocument>(wire).is_err());
    let mut wire = serde_json::to_value(&good).unwrap();
    wire["macros"][0]["appEngine"]["events"][0]["text"] = "altered".into();
    assert!(serde_json::from_value::<CompiledDocument>(wire).is_err());
    let mut wire = serde_json::to_value(&good).unwrap();
    wire.as_object_mut().unwrap().remove("phased_macros");
    assert!(serde_json::from_value::<CompiledDocument>(wire).is_err());
    let mut wire = serde_json::to_value(&good).unwrap();
    wire["phased_macros"] = wire["macros"].clone();
    assert!(serde_json::from_value::<CompiledDocument>(wire).is_err());
}

#[test]
fn mixed_macro_catalogs_publish_to_native_consumers_and_clear_deleted_definitions() {
    let mut library = library();
    let mut phased = library.entries[0].clone();
    phased.id = 2;
    phased.xml_guid = Some("23456789-1234-4234-8234-123456789abc".into());
    phased.macro_type = MacroType::Phased;
    phased.actions.clear();
    library.entries.push(phased);
    library.next_id = 3;
    let mut compiler = Compiler::new();
    let document = compiler.compile(&[], &library).unwrap();
    assert_eq!(document.macros().len(), 2);
    assert_eq!(document.phased_macros().len(), 1);
    assert_eq!(
        document.phased_macros()[0]["guid"],
        "23456789-1234-4234-8234-123456789abc"
    );
    let mut ordinary = razer_device::macro_runtime::Library::default();
    let mut phased = razer_device::macro_runtime::advanced::Library::default();
    assert_eq!(
        ordinary.publish(&json!(document.macros())).unwrap().count,
        2
    );
    assert_eq!(
        phased
            .publish_phased(&json!(document.phased_macros()))
            .unwrap()
            .count,
        1
    );
    assert!(
        razer_device::macro_runtime::advanced::Execution::phased(
            &phased,
            "23456789-1234-4234-8234-123456789abc",
            0
        )
        .is_ok()
    );

    library
        .entries
        .retain(|entry| entry.macro_type != MacroType::Phased);
    let document = compiler.compile(&[], &library).unwrap();
    ordinary.publish(&json!(document.macros())).unwrap();
    assert_eq!(
        phased
            .publish_phased(&json!(document.phased_macros()))
            .unwrap()
            .count,
        0
    );
    assert!(
        razer_device::macro_runtime::advanced::Execution::phased(
            &phased,
            "23456789-1234-4234-8234-123456789abc",
            0
        )
        .is_err()
    );
    assert!(
        ordinary
            .get("12345678-1234-4234-8234-123456789abc")
            .is_some()
    );

    library.entries.clear();
    let document = compiler.compile(&[], &library).unwrap();
    assert_eq!(
        ordinary.publish(&json!(document.macros())).unwrap().count,
        0
    );
    assert_eq!(
        phased
            .publish_phased(&json!(document.phased_macros()))
            .unwrap()
            .count,
        0
    );
    assert!(
        ordinary
            .get("12345678-1234-4234-8234-123456789abc")
            .is_none()
    );
}

#[test]
fn source_phase_and_sequence_options_keep_distinct_native_edges() {
    for (kind, playback, down, up) in [
        (
            MacroType::Sequence,
            "Sequence",
            json!({"type":"macro","guid":"12345678-1234-4234-8234-123456789abc","flag":3,"toggle":3}),
            json!({"type":"disabled"}),
        ),
        (
            MacroType::Phased,
            "Phased",
            json!({"type":"phasedMacro","guid":"12345678-1234-4234-8234-123456789abc","flag":0,"toggle":0}),
            json!({"type":"phasedMacro","guid":"12345678-1234-4234-8234-123456789abc","flag":1}),
        ),
    ] {
        let mut library = library();
        library.entries[0].macro_type = kind;
        if kind == MacroType::Phased {
            library.entries[0].actions[0].phase = Some(0);
        }
        let item = shortcut(ShortcutOutput::Macro {
            macro_id: 1,
            name: "Local macro".into(),
            playback: playback.into(),
            repeat_count: 1,
        });
        let document = Compiler::new().compile(&[item], &library).unwrap();
        assert_eq!(document.app_engine()["mappings"][0]["output"], down);
        assert_eq!(document.app_engine()["mappings"][1]["output"], up);
        if kind == MacroType::Phased {
            assert_eq!(document.macros(), document.phased_macros());
            assert_eq!(
                document.macros()[0]["appEngine"]["pressEvents"],
                json!([
                    {"type":"clipboard","id":"text","text":"Local macro"},{"type":"delay","ms":500}
                ])
            );
        } else {
            assert!(document.phased_macros().is_empty());
        }
    }
}

#[test]
fn malformed_native_fields_are_rejected_even_with_matching_hashes() {
    let document = Compiler::new()
        .compile(
            &[shortcut(ShortcutOutput::Text { text: "x".into() })],
            &library(),
        )
        .unwrap();
    for (field, invalid) in [
        ("scancode", Value::Null),
        ("scancode", json!(65536)),
        ("hypershift", json!("false")),
        ("flag", json!(8)),
    ] {
        let mut wire = serde_json::to_value(&document).unwrap();
        wire["app_engine"]["mappings"][0]["input"][field] = invalid;
        wire["app_engine"]["hash"] = source_hash(&wire["app_engine"]).unwrap().into();
        assert!(
            serde_json::from_value::<CompiledDocument>(wire).is_err(),
            "{field}"
        );
    }
    for output in [
        json!({"type":"keyboard","scancode":null,"flag":0}),
        json!({"type":"delay","ms":-1}),
        json!({"type":"multi","outputs":[{"type":"futureOutput"}]}),
    ] {
        let mut wire = serde_json::to_value(&document).unwrap();
        wire["app_engine"]["mappings"][0]["output"] = output;
        wire["app_engine"]["hash"] = source_hash(&wire["app_engine"]).unwrap().into();
        assert!(serde_json::from_value::<CompiledDocument>(wire).is_err());
    }
}

#[test]
fn unrecorded_output_rows_remain_local_without_blocking_recorded_shortcuts() {
    let active = shortcut(ShortcutOutput::Text {
        text: "recorded".into(),
    });
    let mut draft = shortcut(ShortcutOutput::Text {
        text: "not yet recorded".into(),
    });
    draft.id = "output-only".into();
    draft.input.clear();
    draft.modifiers.clear();
    let mut compiler = Compiler::new();
    let before = compiler
        .compile(&[active.clone()], &MacroLibraryFile::default())
        .unwrap();
    let combined = compiler
        .compile(&[active, draft.clone()], &MacroLibraryFile::default())
        .unwrap();
    assert_eq!(before, combined);
    assert!(draft.input.is_empty());
}

#[test]
fn imported_catalog_routes_types_and_retains_phase_lanes_through_edit_and_delete() {
    let phased_guid = "33456789-1234-4234-8234-123456789abc";
    let source = json!({
        "guid":phased_guid,"name":"Phased import","macroPlaybackOption":"phased",
        "macroEvents":[
            {"Type":"actionBar","recordProfile":{"mmtSetting":0,"macroType":2}},
            {"Type":4,"Text":"press","phase":0},
            {"Type":4,"Text":"held","phase":1},
            {"Type":4,"Text":"release","phase":2}
        ],
        "appEngine":{
            "events":[],"mouseMoveType":"none",
            "pressEvents":[{"type":"clipboard","id":"text","text":"press"},{"type":"delay","ms":500}],
            "holdEvents":[{"type":"clipboard","id":"text","text":"held"},{"type":"delay","ms":500}],
            "releaseEvents":[{"type":"clipboard","id":"text","text":"release"},{"type":"delay","ms":500}]
        }
    });
    let mut library = library();
    let mut sequence = library.entries[0].clone();
    sequence.id = 2;
    sequence.xml_guid = Some("23456789-1234-4234-8234-123456789abc".into());
    sequence.macro_type = MacroType::Sequence;
    library.entries.push(sequence);
    library.next_id = 3;
    library.append_source_documents(&[source]).unwrap();
    let mut compiler = Compiler::new();
    let initial = compiler.compile(&[], &library).unwrap();
    assert_eq!(initial.macros().len(), 3);
    assert_eq!(initial.phased_macros().len(), 1);
    assert_eq!(initial.phased_macros()[0]["guid"], phased_guid);
    let mut ordinary = razer_device::macro_runtime::Library::default();
    let mut advanced = razer_device::macro_runtime::advanced::Library::default();
    ordinary.publish(&json!(initial.macros())).unwrap();
    advanced
        .publish_phased(&json!(initial.phased_macros()))
        .unwrap();

    library.entries[2].name = "Renamed phased import".into();
    let renamed = compiler.compile(&[], &library).unwrap();
    assert_eq!(renamed.phased_macros()[0]["name"], "Renamed phased import");
    assert_eq!(
        renamed.phased_macros()[0]["appEngine"],
        initial.phased_macros()[0]["appEngine"]
    );

    // Saving an edited phase rebuilds its source lanes; the retained import
    // must not override the changed row or leave events in the former lane.
    library.entries[2].actions[0].phase = Some(2);
    library.entries[2]
        .actions
        .retain(|action| action.phase != Some(1));
    let edited = compiler.compile(&[], &library).unwrap();
    let engine = &edited.phased_macros()[0]["appEngine"];
    assert_eq!(engine["pressEvents"], json!([]));
    assert_eq!(engine["holdEvents"], json!([]));
    assert_eq!(
        engine["releaseEvents"],
        json!([
            {"type":"clipboard","id":"text","text":"press"},{"type":"delay","ms":500},
            {"type":"clipboard","id":"text","text":"release"},{"type":"delay","ms":500}
        ])
    );
    assert_eq!(engine["events"], json!([]));
    advanced
        .publish_phased(&json!(edited.phased_macros()))
        .unwrap();

    library.entries.pop();
    let deleted = compiler.compile(&[], &library).unwrap();
    assert_eq!(deleted.macros().len(), 2);
    assert!(deleted.phased_macros().is_empty());
    advanced
        .publish_phased(&json!(deleted.phased_macros()))
        .unwrap();
    assert!(
        razer_device::macro_runtime::advanced::Execution::phased(&advanced, phased_guid, 0)
            .is_err()
    );
}

#[test]
fn retained_phased_import_missing_a_lane_is_rejected_without_synthesizing_it() {
    for missing in ["pressEvents", "holdEvents", "releaseEvents"] {
        let mut source = json!({
            "guid":"33456789-1234-4234-8234-123456789abc","name":"Incomplete phase",
            "macroPlaybackOption":"phased","macroEvents":[],
            "appEngine":{"events":[],"pressEvents":[],"holdEvents":[],"releaseEvents":[],"mouseMoveType":"none"}
        });
        source["appEngine"].as_object_mut().unwrap().remove(missing);
        let mut library = MacroLibraryFile::default();
        library.append_source_documents(&[source.clone()]).unwrap();
        let error = Compiler::new().compile(&[], &library).unwrap_err();
        assert!(error.contains(missing), "{error}");
        assert_eq!(library.source_macro_documents().unwrap(), [source]);
    }
}

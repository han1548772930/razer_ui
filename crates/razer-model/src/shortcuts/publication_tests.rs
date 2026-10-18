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
fn macro_identity_resolves_local_id_and_both_consumers_receive_complete_list() {
    let library = library();
    let item = shortcut(ShortcutOutput::Macro {
        macro_id: 1,
        name: "Old display name".into(),
        playback: "NTimes".into(),
        repeat_count: 3,
    });
    let document = Compiler::new().compile(&[item], &library).unwrap();
    assert_eq!(document.macros(), document.phased_macros());
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
        assert_eq!(document.macros(), document.phased_macros());
        if kind == MacroType::Phased {
            assert_eq!(
                document.macros()[0]["appEngine"]["pressEvents"],
                json!([
                    {"type":"clipboard","id":"text","text":"Local macro"},{"type":"delay","ms":500}
                ])
            );
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

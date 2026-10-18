use super::*;

fn shortcut(id: &str) -> Shortcut {
    Shortcut::new(
        id.into(),
        "KEY_K".into(),
        vec!["KEY_LEFT_CTRL".into()],
        false,
        ShortcutOutput::Text {
            text: "你好\nshortcut".into(),
        },
    )
}

#[test]
fn shortcut_storage_rejects_invalid_or_duplicate_chords_without_losing_unicode() {
    let original = shortcut("first");
    let document = serde_json::to_value(&original).unwrap();
    assert_eq!(
        document,
        serde_json::json!({
            "id": "first",
            "input": "KEY_K",
            "modifiers": ["KEY_LEFT_CTRL"],
            "hypershift": false,
            "output": {"kind": "text", "text": "你好\nshortcut"},
        })
    );
    let decoded: Shortcut = serde_json::from_value(document).unwrap();
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
    assert!(
        serde_json::from_str::<Shortcut>(
            r#"{"id":"a","input":"KEY_K","modifiers":[],"hypershift":false,"output":{"kind":"future"}}"#
        )
        .is_err()
    );
}

#[test]
fn stored_output_drafts_preserve_the_unrecorded_state_without_becoming_registered() {
    let mut draft = shortcut("draft");
    draft.input.clear();
    draft.modifiers.clear();
    assert!(validate_stored_shortcuts(&[draft.clone()]).is_ok());
    assert!(validate_shortcuts(&[draft.clone()]).is_err());
    draft.modifiers.push("CTRL".into());
    assert!(validate_stored_shortcuts(&[draft.clone()]).is_err());
    draft.modifiers.clear();
    draft.output = ShortcutOutput::Text {
        text: "🙂".repeat(126),
    };
    assert!(validate_stored_shortcuts(&[draft]).is_err());
}

#[test]
fn windows_choices_and_validation_exclude_the_online_assistant() {
    let choices = &source_shortcuts().windows;
    assert!(choices.iter().any(|choice| choice.id == "Calculator"));
    assert!(choices.iter().any(|choice| choice.id == "LockComputer"));
    assert!(!choices.iter().any(|choice| choice.id == "MSCopilot"));
    assert!(!WINDOWS.iter().any(|(id, _)| *id == "MSCopilot"));

    let mut excluded = shortcut("online-assistant");
    excluded.output = ShortcutOutput::Windows {
        action: "MSCopilot".into(),
    };
    assert!(validate_shortcuts(&[excluded.clone()]).is_err());
    assert!(super::encode_shortcuts(&[excluded]).is_err());
}

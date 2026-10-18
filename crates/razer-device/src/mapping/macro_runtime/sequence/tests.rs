use super::*;
use serde_json::{Value, json};
fn row(guid: &str, events: Vec<Value>) -> Value {
    let mut engine = json!({"events":events});
    engine["hash"] = crate::mapping_engine_hash::source_hash(&engine)
        .unwrap()
        .into();
    json!({"guid":guid,"appEngine":engine})
}
fn key(code: u16, flag: u16) -> Value {
    json!({"type":"keyboard","scancode":code,"flag":flag})
}
fn trigger(sequences: &mut Sequences, library: &Library, guid: &str) -> Result<Vec<Action>> {
    sequences.begin(guid)?;
    for _ in 0..1000 {
        match sequences.poll(library)? {
            Poll::Progress => (),
            Poll::Actions(actions) => return Ok(actions),
            Poll::Complete => return Ok(Vec::new()),
        }
    }
    panic!("Sequence did not reach a boundary");
}
fn code(actions: &[Action]) -> u16 {
    let Action::Keyboard(key) = &actions[0] else {
        panic!("Expected key")
    };
    key.scancode
}

#[test]
fn trigger_emits_one_click_skips_recorded_releases_and_wraps() {
    let mut library = Library::default();
    library
        .publish(&json!([row(
            "s",
            vec![
                key(30, 0),
                key(30, 1),
                json!({"type":"delay","ms":999}),
                key(31, 0),
                key(31, 1)
            ]
        )]))
        .unwrap();
    let mut sequences = Sequences::default();
    for expected in [30, 31, 30] {
        let actions = trigger(&mut sequences, &library, "s").unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(code(&actions), expected);
        assert!(matches!(&actions[1],Action::Keyboard(key) if key.flag==1));
    }
}

#[test]
fn loop_cursor_survives_triggers_and_hash_change_resets_it() {
    let mut library = Library::default();
    library
        .publish(&json!([row(
            "s",
            vec![
                json!({"type":"loop","flag":0,"repeat":2}),
                key(30, 0),
                json!({"type":"loop","flag":1}),
                key(31, 0)
            ]
        )]))
        .unwrap();
    let mut sequences = Sequences::default();
    for expected in [30, 30, 31, 30] {
        assert_eq!(
            code(&trigger(&mut sequences, &library, "s").unwrap()),
            expected
        );
    }
    library
        .publish(&json!([row("s", vec![key(50, 0)])]))
        .unwrap();
    assert_eq!(code(&trigger(&mut sequences, &library, "s").unwrap()), 50);
}

#[test]
fn nested_cursor_holds_parent_until_child_exhausted() {
    let mut library = Library::default();
    library
        .publish(&json!([
            row(
                "outer",
                vec![json!({"type":"macro","guid":"inner","flag":0}), key(40, 0)]
            ),
            row(
                "inner",
                vec![key(30, 0), key(30, 1), key(31, 0), key(31, 1)]
            )
        ]))
        .unwrap();
    let mut sequences = Sequences::default();
    for expected in [30, 31, 40, 30] {
        assert_eq!(
            code(&trigger(&mut sequences, &library, "outer").unwrap()),
            expected
        );
    }
}

#[test]
fn nested_end_at_entry_preserves_source_exhausted_cursor() {
    let mut library = Library::default();
    library
        .publish(&json!([
            row(
                "outer",
                vec![json!({"type":"macro","guid":"inner","flag":0}), key(40, 0)]
            ),
            row("inner", vec![key(30, 0)])
        ]))
        .unwrap();
    let mut sequences = Sequences::default();
    for expected in [30, 40, 40] {
        assert_eq!(
            code(&trigger(&mut sequences, &library, "outer").unwrap()),
            expected
        );
    }
}

#[test]
fn ignored_events_finish_after_one_wrap_and_invalid_event_remains_failure() {
    let mut library = Library::default();
    library
        .publish(&json!([row(
            "s",
            vec![
                json!({"type":"delay","ms":9}),
                json!({"type":"audio","id":"speaker","mute":1})
            ]
        )]))
        .unwrap();
    let mut sequences = Sequences::default();
    assert!(trigger(&mut sequences, &library, "s").unwrap().is_empty());
    library
        .publish(&json!([row("s", vec![json!({"type":"bad"})])]))
        .unwrap();
    for _ in 0..2 {
        assert!(trigger(&mut sequences, &library, "s").is_err());
    }
}

#[test]
fn cancel_stops_current_trigger_without_losing_device_cursor() {
    let mut library = Library::default();
    library
        .publish(&json!([row(
            "s",
            vec![json!({"type":"delay","ms":0}), key(30, 0)]
        )]))
        .unwrap();
    let mut sequences = Sequences::default();
    sequences.begin("s").unwrap();
    assert_eq!(sequences.poll(&library).unwrap(), Poll::Progress);
    sequences.cancel();
    assert_eq!(sequences.poll(&library).unwrap(), Poll::Complete);
    assert_eq!(code(&trigger(&mut sequences, &library, "s").unwrap()), 30);
}

#[test]
fn empty_unbalanced_and_recursive_sequences_are_explicit_failures() {
    let mut library = Library::default();
    library
        .publish(&json!([
            row("empty", vec![]),
            row("loop", vec![json!({"type":"loop","flag":1})]),
            row(
                "recursive",
                vec![json!({"type":"macro","guid":"recursive","flag":0})]
            )
        ]))
        .unwrap();
    let mut sequences = Sequences::default();
    for guid in ["empty", "loop", "recursive", "missing"] {
        assert!(trigger(&mut sequences, &library, guid).is_err());
    }
}

use super::*;
use serde_json::{Value, json};

fn key(scancode: u16) -> Keyboard {
    Keyboard {
        scancode,
        flag: 0,
        modifiers: 0,
        unit: None,
        delay_ms: None,
        release_after_ms: None,
    }
}

#[test]
fn publication_accepts_empty_guid_but_nested_lookup_does_not() {
    let mut library = Library::default();
    library
        .publish(&json!([row("", vec![json!({"type":"delay","ms":0})])]))
        .unwrap();
    assert!(library.get("").is_some());
    assert!(super::events::parse(&json!({"type":"macro","guid":"","flag":0})).is_err());
}

#[test]
fn clipboard_and_audio_failures_are_retained_without_stopping_source_flow() {
    let mut library = Library::default();
    library
        .publish(&json!([row(
            "one",
            vec![
                json!({"type":"clipboard","id":"text","text":"x"}),
                json!({"type":"audio","id":"speaker","mute":2})
            ]
        )]))
        .unwrap();
    let mut execution = Execution::start(&library, "one", 1).unwrap();
    for error in ["clipboard busy", "endpoint absent"] {
        let pending = action(&mut execution, &library, 0);
        execution
            .acknowledge(pending.ticket, Err(error.into()))
            .unwrap();
    }
    let result = complete(&mut execution, &library, 0);
    assert_eq!(result.outcome, Outcome::Completed);
    assert_eq!(
        result.operation_errors,
        ["clipboard busy", "endpoint absent"]
    );
}

#[test]
fn only_middle_button_duplicate_is_suppressed() {
    for (id, emitted) in [(1, 2), (16, 1), (4, 2)] {
        let mut library = Library::default();
        library
            .publish(&json!([row(
                "one",
                vec![
                    json!({"type":"mouse","id":id}),
                    json!({"type":"mouse","id":id})
                ]
            )]))
            .unwrap();
        let mut execution = Execution::start(&library, "one", 1).unwrap();
        for _ in 0..emitted {
            let pending = action(&mut execution, &library, 0);
            assert!(!pending.cleanup);
            execution.acknowledge(pending.ticket, Ok(())).unwrap();
        }
        let release = action(&mut execution, &library, 0);
        assert!(release.cleanup);
    }
}

#[test]
fn keyboard_repeats_use_system_initial_and_repeat_intervals() {
    let mut executor =
        KeyboardExecutor::new(KeyboardTiming::from_system_parameters(Some(2), Some(31)));
    let mut sent = Vec::new();
    let mut send = |key: &Keyboard| {
        sent.push(key.clone());
        Ok(())
    };
    executor.execute(key(30), false, 0, &mut send).unwrap();
    assert_eq!(executor.next_deadline_us(), Some(750_000));
    assert!(executor.poll(749_999, &mut send).is_empty());
    assert!(executor.poll(750_000, &mut send).is_empty());
    assert_eq!(executor.next_deadline_us(), Some(783_000));
    executor
        .execute(key(30), false, 760_000, &mut send)
        .unwrap();
    assert_eq!(executor.next_deadline_us(), Some(783_000));
    assert_eq!(sent.len(), 2);
}

#[test]
fn auto_release_waits_for_negative_tracker_and_removes_repeat() {
    let mut executor = KeyboardExecutor::new(KeyboardTiming {
        initial_ms: 1000,
        repeat_ms: 30,
    });
    let mut press = key(30);
    press.release_after_ms = Some(250);
    let mut sent = Vec::new();
    let mut send = |key: &Keyboard| {
        sent.push(key.clone());
        Ok(())
    };
    executor.execute(press, false, 0, &mut send).unwrap();
    executor.poll(250_000, &mut send);
    assert_eq!(executor.next_deadline_us(), Some(500_000));
    executor.poll(500_000, &mut send);
    assert_eq!(executor.next_deadline_us(), None);
    assert_eq!(sent.iter().map(|key| key.flag).collect::<Vec<_>>(), [0, 1]);
}

#[test]
fn release_of_other_key_restarts_active_initial_delay_and_keeps_unit_identity() {
    let mut executor = KeyboardExecutor::new(KeyboardTiming {
        initial_ms: 250,
        repeat_ms: 30,
    });
    let mut sent = Vec::new();
    let mut send = |key: &Keyboard| {
        sent.push(key.clone());
        Ok(())
    };
    executor.execute(key(30), false, 0, &mut send).unwrap();
    let mut active = key(30);
    active.unit = Some(0);
    executor
        .execute(active.clone(), false, 10, &mut send)
        .unwrap();
    let mut release = key(30);
    release.flag = 1;
    executor.execute(release, false, 20, &mut send).unwrap();
    assert_eq!(executor.next_deadline_us(), Some(250020));
    active.flag = 1;
    executor.execute(active, false, 30, &mut send).unwrap();
    assert_eq!(executor.next_deadline_us(), None);
    assert_eq!(sent.len(), 4);
}

#[test]
fn smart_stop_ends_repetition_without_fabricating_release() {
    let mut executor = KeyboardExecutor::new(KeyboardTiming {
        initial_ms: 250,
        repeat_ms: 30,
    });
    let mut sent = Vec::new();
    let mut send = |key: &Keyboard| {
        sent.push(key.clone());
        Ok(())
    };
    executor.execute(key(30), true, 0, &mut send).unwrap();
    executor.poll(250_000, &mut send);
    executor.poll(280_000, &mut send);
    executor.poll(310_000, &mut send);
    assert_eq!(executor.next_deadline_us(), None);
    assert_eq!(sent.len(), 3);
    assert!(sent.iter().all(|key| key.flag == 0));
}

#[test]
fn failed_repeat_and_stop_do_not_leave_live_timers() {
    let mut executor = KeyboardExecutor::new(KeyboardTiming {
        initial_ms: 250,
        repeat_ms: 30,
    });
    executor
        .execute(key(30), false, 0, &mut |_| Ok(()))
        .unwrap();
    assert_eq!(
        executor.poll(250_000, &mut |_| Err("driver disappeared".into())),
        ["driver disappeared"]
    );
    assert_eq!(executor.next_deadline_us(), None);
    executor.stop();
    executor
        .execute(
            {
                let mut release = key(30);
                release.flag = 1;
                release
            },
            false,
            300_000,
            &mut |_| panic!("stopped owner must not send release"),
        )
        .unwrap();
}

fn row(guid: &str, events: Vec<Value>) -> Value {
    let mut engine = json!({"mouseMoveType":"none", "events":events});
    engine["hash"] = crate::mapping_engine_hash::source_hash(&engine)
        .unwrap()
        .into();
    json!({"guid":guid,"appEngine":engine})
}

fn poll(execution: &mut Execution, library: &Library, now: u64) -> Poll {
    execution.poll(now, library, &mut |min, _| Ok(min))
}

fn action(execution: &mut Execution, library: &Library, now: u64) -> PendingAction {
    for _ in 0..1000 {
        match poll(execution, library, now) {
            Poll::Action(action) => return action,
            Poll::Progress => {}
            other => panic!("Expected action, got {other:?}"),
        }
    }
    panic!("Macro made no output progress");
}

fn complete(execution: &mut Execution, library: &Library, now: u64) -> Completion {
    for _ in 0..1000 {
        match poll(execution, library, now) {
            Poll::Complete(result) => return result,
            Poll::Progress => {}
            other => panic!("Expected completion, got {other:?}"),
        }
    }
    panic!("Macro did not finish");
}

#[test]
fn publication_is_atomic_and_running_definitions_survive_replacement() {
    let mut library = Library::default();
    library
        .publish(&json!([row("one", vec![json!({"type":"delay","ms":7})])]))
        .unwrap();
    let original = library.get("one").unwrap();
    let mut invalid = row("two", vec![]);
    invalid["appEngine"]["hash"] = "wrong".into();
    assert!(
        library
            .publish(&json!([row("one", vec![]), invalid]))
            .is_err()
    );
    assert!(Arc::ptr_eq(&original, &library.get("one").unwrap()));
    library.publish(&json!([row("one", vec![])])).unwrap();
    assert_eq!(original.event_count(), 1);
    assert_eq!(library.get("one").unwrap().event_count(), 0);
}

#[test]
fn same_hash_reuses_the_live_definition_before_event_validation() {
    let mut library = Library::default();
    let first = row("one", vec![json!({"type":"delay","ms":1})]);
    library.publish(&json!([first.clone()])).unwrap();
    let original = library.get("one").unwrap();
    let mut reused = first;
    reused["appEngine"]["events"] = Value::Null;
    let publication = library.publish(&json!([reused])).unwrap();
    assert!(publication.changed.is_empty());
    assert!(Arc::ptr_eq(&original, &library.get("one").unwrap()));
}

#[test]
fn event_failure_occurs_when_reached_and_releases_accepted_outputs() {
    let mut library = Library::default();
    library
        .publish(&json!([row(
            "one",
            vec![
                json!({"type":"keyboard","scancode":30,"flag":0}),
                json!({"type":"invalid"})
            ]
        )]))
        .unwrap();
    let mut execution = Execution::start(&library, "one", 1).unwrap();
    let press = action(&mut execution, &library, 0);
    execution.acknowledge(press.ticket, Ok(())).unwrap();
    let release = action(&mut execution, &library, 0);
    assert!(release.cleanup);
    assert!(matches!(
        release.action,
        Action::Keyboard(Keyboard { flag: 1, .. })
    ));
    execution.acknowledge(release.ticket, Ok(())).unwrap();
    let result = complete(&mut execution, &library, 0);
    assert_eq!(result.outcome, Outcome::Failed);
    assert!(result.execution_error.unwrap().contains("invalid"));
}

#[test]
fn cancellation_waits_for_inflight_make_then_reports_release_failure() {
    let mut library = Library::default();
    library
        .publish(&json!([row("one", vec![json!({"type":"mouse","id":1})])]))
        .unwrap();
    let mut execution = Execution::start(&library, "one", 0).unwrap();
    let press = action(&mut execution, &library, 0);
    execution.cancel();
    assert_eq!(
        poll(&mut execution, &library, 0),
        Poll::WaitingForCompletion
    );
    execution.acknowledge(press.ticket, Ok(())).unwrap();
    let release = action(&mut execution, &library, 0);
    assert!(release.cleanup);
    assert!(matches!(release.action, Action::Mouse(Mouse { id: 2, .. })));
    execution
        .acknowledge(release.ticket, Err("driver disconnected".into()))
        .unwrap();
    let result = complete(&mut execution, &library, 0);
    assert_eq!(result.outcome, Outcome::Cancelled);
    assert_eq!(result.cleanup_errors, ["driver disconnected"]);
}

#[test]
fn stale_completion_cannot_consume_another_execution_output() {
    let mut library = Library::default();
    library
        .publish(&json!([row("one", vec![json!({"type":"mouse","id":1})])]))
        .unwrap();
    let mut first = Execution::start(&library, "one", 1).unwrap();
    let mut second = Execution::start(&library, "one", 1).unwrap();
    let old = action(&mut first, &library, 0);
    let current = action(&mut second, &library, 0);
    assert!(second.acknowledge(old.ticket, Ok(())).is_err());
    assert_eq!(poll(&mut second, &library, 0), Poll::WaitingForCompletion);
    second
        .acknowledge(current.ticket, Err("actual failure".into()))
        .unwrap();
    assert_eq!(complete(&mut second, &library, 0).outcome, Outcome::Failed);
}

#[test]
fn interrupted_keyboard_spacing_finishes_dispatched_key_then_releases_both() {
    let mut library = Library::default();
    library
        .publish(&json!([row(
            "one",
            vec![
                json!({"type":"keyboard","scancode":30,"flag":0}),
                json!({"type":"keyboard","scancode":31,"flag":0})
            ]
        )]))
        .unwrap();
    let mut execution = Execution::start(&library, "one", 1).unwrap();
    let first = action(&mut execution, &library, 10);
    execution.acknowledge(first.ticket, Ok(())).unwrap();
    assert_eq!(
        poll(&mut execution, &library, 20),
        Poll::WaitingUntil(10020)
    );
    execution.cancel();
    let delayed = action(&mut execution, &library, 21);
    assert!(matches!(
        delayed.action,
        Action::Keyboard(Keyboard {
            scancode: 31,
            flag: 0,
            ..
        })
    ));
    execution.acknowledge(delayed.ticket, Ok(())).unwrap();
    for scancode in [30, 31] {
        let release = action(&mut execution, &library, 21);
        assert!(release.cleanup);
        assert!(
            matches!(release.action,Action::Keyboard(Keyboard { scancode:actual,flag:1,.. }) if actual==scancode)
        );
        execution.acknowledge(release.ticket, Ok(())).unwrap();
    }
    assert_eq!(
        complete(&mut execution, &library, 21).outcome,
        Outcome::Cancelled
    );
}

#[test]
fn nested_macro_uses_current_publication_and_retains_outer_version() {
    let mut library = Library::default();
    library
        .publish(&json!([
            row(
                "outer",
                vec![
                    json!({"type":"delay","ms":1}),
                    json!({"type":"macro","guid":"inner","flag":0})
                ]
            ),
            row(
                "inner",
                vec![json!({"type":"clipboard","id":"text","text":"old"})]
            )
        ]))
        .unwrap();
    let mut execution = Execution::start(&library, "outer", 1).unwrap();
    assert_eq!(poll(&mut execution, &library, 0), Poll::WaitingUntil(1000));
    library
        .publish(&json!([row(
            "inner",
            vec![json!({"type":"clipboard","id":"text","text":"current"})]
        )]))
        .unwrap();
    let nested = action(&mut execution, &library, 1000);
    assert_eq!(nested.action, Action::Clipboard("current".into()));
    execution.acknowledge(nested.ticket, Ok(())).unwrap();
    assert_eq!(
        complete(&mut execution, &library, 1000).outcome,
        Outcome::Completed
    );
}

#[test]
fn loop_repeats_its_range_then_resumes_after_end() {
    let mut library = Library::default();
    library
        .publish(&json!([row(
            "one",
            vec![
                json!({"type":"loop","flag":0,"repeat":2}),
                json!({"type":"clipboard","id":"text","text":"inside"}),
                json!({"type":"loop","flag":1}),
                json!({"type":"clipboard","id":"text","text":"after"})
            ]
        )]))
        .unwrap();
    let mut execution = Execution::start(&library, "one", 1).unwrap();
    for expected in ["inside", "inside", "after"] {
        let output = action(&mut execution, &library, 0);
        assert_eq!(output.action, Action::Clipboard(expected.into()));
        execution.acknowledge(output.ticket, Ok(())).unwrap();
    }
    assert_eq!(
        complete(&mut execution, &library, 0).outcome,
        Outcome::Completed
    );
}

#[test]
fn recursive_macro_fails_at_source_limit() {
    let mut library = Library::default();
    library
        .publish(&json!([row(
            "one",
            vec![json!({"type":"macro","guid":"one","flag":0})]
        )]))
        .unwrap();
    let mut execution = Execution::start(&library, "one", 1).unwrap();
    let result = complete(&mut execution, &library, 0);
    assert_eq!(
        result.execution_error.as_deref(),
        Some("max macro recursion")
    );
}

#[test]
fn parser_preserves_word_truncation_and_signed_coordinate_bits() {
    let event =
        events::parse(&json!({"type":"keyboard","scancode":65566,"flag":-1,"unitId":0})).unwrap();
    assert!(matches!(
        event,
        Event::Keyboard(Keyboard {
            scancode: 30,
            flag: 65535,
            unit: Some(0),
            ..
        })
    ));
    assert_eq!(
        events::parse(&json!({"type":"mouseMove","x":-20,"y":7})).unwrap(),
        Event::MouseMove { x: -20, y: 7 }
    );
    assert!(events::parse(&json!({"type":"delay","ms":1.5})).is_err());
    assert!(events::parse(&json!({"type":"loop","flag":0,"repeat":0})).is_err());
}

#[test]
fn endless_ignored_output_failures_keep_bounded_diagnostics_and_exact_count() {
    let mut library = Library::default();
    library
        .publish(&json!([row(
            "one",
            vec![json!({"type":"clipboard","id":"text","text":"x"})]
        )]))
        .unwrap();
    let mut execution = Execution::start(&library, "one", 0).unwrap();
    for index in 0..200 {
        let output = action(&mut execution, &library, 0);
        execution
            .acknowledge(output.ticket, Err(format!("failure {index}")))
            .unwrap();
    }
    execution.cancel();
    let completion = complete(&mut execution, &library, 0);
    assert_eq!(completion.operation_errors.len(), 64);
    assert_eq!(completion.operation_errors[0], "failure 0");
    assert_eq!(completion.operation_errors[63], "failure 63");
    assert_eq!(completion.omitted_operation_errors, 136);
    assert_eq!(completion.outcome, Outcome::Cancelled);
}

#[test]
fn owner_failure_requires_pending_completion_then_releases_confirmed_outputs() {
    let mut library = Library::default();
    library
        .publish(&json!([row("one", vec![json!({"type":"mouse","id":1})])]))
        .unwrap();
    let mut execution = Execution::start(&library, "one", 1).unwrap();
    let output = action(&mut execution, &library, 0);
    assert!(execution.fail("owner failed".into()).is_err());
    execution.acknowledge(output.ticket, Ok(())).unwrap();
    execution.fail("owner failed".into()).unwrap();
    let release = action(&mut execution, &library, 0);
    assert!(release.cleanup);
    assert!(matches!(release.action, Action::Mouse(Mouse { id: 2, .. })));
    execution.acknowledge(release.ticket, Ok(())).unwrap();
    let completion = complete(&mut execution, &library, 0);
    assert_eq!(completion.execution_error.as_deref(), Some("owner failed"));
    assert_eq!(completion.outcome, Outcome::Failed);
}

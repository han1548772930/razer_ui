use super::*;
use serde_json::json;

fn row(guid: &str, mut engine: Value) -> Value {
    engine["hash"] = crate::mapping_engine_hash::source_hash(&engine)
        .unwrap()
        .into();
    json!({"guid":guid,"appEngine":engine})
}
fn key(scancode: u16, flag: u16) -> Value {
    json!({"type":"keyboard","scancode":scancode,"flag":flag})
}
fn next(execution: &mut Execution, now: u64) -> Poll {
    for _ in 0..100 {
        let poll = execution.poll(now);
        if poll != Poll::Progress {
            return poll;
        }
    }
    panic!("Execution did not reach a boundary");
}
fn action(execution: &mut Execution, now: u64) -> PendingAction {
    let Poll::Action(action) = next(execution, now) else {
        panic!("Expected action");
    };
    action
}
fn complete(execution: &mut Execution) -> Completion {
    let Poll::Complete(completion) = next(execution, 0) else {
        panic!("Expected completion");
    };
    completion
}

#[test]
fn phased_preserves_cross_phase_duplicate_gap_and_release_after_cancel() {
    let mut library = Library::default();
    library
        .publish_phased(&json!([row(
            "p",
            json!({"pressEvents":[key(30,0)],"holdEvents":[key(30,0)],"releaseEvents":[key(30,1)]})
        )]))
        .unwrap();
    let mut execution = Execution::phased(&library, "p", 30).unwrap();
    let press = action(&mut execution, 0);
    execution.acknowledge(press.ticket, Ok(())).unwrap();
    assert_eq!(next(&mut execution, 0), Poll::WaitingUntil(30_000));
    assert_eq!(next(&mut execution, 30_000), Poll::WaitingUntil(60_000));
    execution.cancel();
    let release = action(&mut execution, 30_001);
    assert!(!release.cleanup);
    assert!(matches!(
        release.action,
        Action::Output(super::super::Action::Keyboard(super::super::Keyboard {
            flag: 1,
            ..
        }))
    ));
    execution.acknowledge(release.ticket, Ok(())).unwrap();
    assert_eq!(complete(&mut execution).outcome, Outcome::Cancelled);
}

#[test]
fn late_phased_make_is_released_and_stale_ticket_rejected() {
    let mut library = Library::default();
    library
        .publish_phased(&json!([row(
            "p",
            json!({"pressEvents":[key(44,0)],"holdEvents":[],"releaseEvents":[]})
        )]))
        .unwrap();
    let mut execution = Execution::phased(&library, "p", 0).unwrap();
    let press = action(&mut execution, 0);
    execution.cancel();
    assert_eq!(execution.poll(0), Poll::WaitingForCompletion);
    execution.acknowledge(press.ticket, Ok(())).unwrap();
    let release = action(&mut execution, 0);
    assert!(release.cleanup);
    assert!(execution.acknowledge(press.ticket, Ok(())).is_err());
    execution
        .acknowledge(release.ticket, Err("release failed".into()))
        .unwrap();
    assert_eq!(complete(&mut execution).cleanup_errors, ["release failed"]);
}

#[test]
fn phased_parse_diagnostic_advances_but_send_failure_skips_release_phase() {
    let mut library = Library::default();
    library.publish_phased(&json!([row("p",json!({"pressEvents":[{"type":"delay","ms":1},key(30,0)],"holdEvents":[],"releaseEvents":[key(31,0)]}))])).unwrap();
    let mut execution = Execution::phased(&library, "p", 0).unwrap();
    let press = action(&mut execution, 0);
    execution
        .acknowledge(press.ticket, Err("send failed".into()))
        .unwrap();
    let result = complete(&mut execution);
    assert_eq!(result.outcome, Outcome::Failed);
    assert_eq!(result.operation_errors.len(), 1);
}

#[test]
fn empty_hold_zero_delay_waits_for_cancellation() {
    let mut library = Library::default();
    library
        .publish_phased(&json!([row(
            "p",
            json!({"pressEvents":[],"holdEvents":[],"releaseEvents":[]})
        )]))
        .unwrap();
    let mut execution = Execution::phased(&library, "p", 0).unwrap();
    assert_eq!(next(&mut execution, 0), Poll::WaitingForCancellation);
    execution.cancel();
    assert_eq!(complete(&mut execution).outcome, Outcome::Cancelled);
}

#[test]
fn turbo_default_priority_atomic_publication_and_independent_sources() {
    let mut library = Library::default();
    library
        .publish_turbos(
            TurboSource::User,
            &json!([row("t", json!({"events":[key(50,0)]}))]),
        )
        .unwrap();
    library
        .publish_turbos(
            TurboSource::Default,
            &json!([row("t", json!({"events":[key(30,0)]}))]),
        )
        .unwrap();
    let invalid = json!([{"guid":"t","appEngine":{"hash":"invalid","events":[]}}]);
    assert!(
        library
            .publish_turbos(TurboSource::Default, &invalid)
            .is_err()
    );
    let mut running = Execution::turbo(&library, "t", 1, 0).unwrap();
    library
        .publish_turbos(TurboSource::Default, &json!([]))
        .unwrap();
    for (execution, scancode) in [
        (&mut running, 30),
        (&mut Execution::turbo(&library, "t", 1, 0).unwrap(), 50),
    ] {
        let begin = action(execution, 0);
        assert_eq!(begin.action, Action::BeginTurboCycle);
        execution.acknowledge(begin.ticket, Ok(())).unwrap();
        let output = action(execution, 0);
        assert!(
            matches!(output.action,Action::Output(super::super::Action::Keyboard(ref key)) if key.scancode==scancode)
        );
    }
}

#[test]
fn turbo_finishes_first_cycle_on_cancel_and_restores_modifiers_before_cleanup() {
    let mut library = Library::default();
    library
        .publish_turbos(
            TurboSource::User,
            &json!([row(
                "t",
                json!({"events":[key(30,0),{"type":"delay","ms":10},key(31,0)]})
            )]),
        )
        .unwrap();
    let mut execution = Execution::turbo(&library, "t", 0, 1000).unwrap();
    execution.cancel();
    for expected in [0, 30, 31, 1] {
        let pending = action(&mut execution, 0);
        match expected {
            0 => assert_eq!(pending.action, Action::BeginTurboCycle),
            1 => assert_eq!(pending.action, Action::EndTurboCycle),
            code => assert!(
                matches!(pending.action,Action::Output(super::super::Action::Keyboard(ref key)) if key.scancode==code)
            ),
        }
        execution.acknowledge(pending.ticket, Ok(())).unwrap();
    }
    for _ in 0..2 {
        let release = action(&mut execution, 0);
        assert!(release.cleanup);
        execution.acknowledge(release.ticket, Ok(())).unwrap();
    }
    assert_eq!(complete(&mut execution).outcome, Outcome::Cancelled);
}

#[test]
fn turbo_preserves_intermediate_failure_and_source_last_result_flow() {
    let mut library = Library::default();
    library
        .publish_turbos(
            TurboSource::User,
            &json!([row("t", json!({"events":[key(30,0),key(30,1)]}))]),
        )
        .unwrap();
    let mut execution = Execution::turbo(&library, "t", 2, 5).unwrap();
    for result in [Ok(()), Err("make rejected".into()), Ok(()), Ok(())] {
        let pending = action(&mut execution, 0);
        execution.acknowledge(pending.ticket, result).unwrap();
    }
    assert_eq!(next(&mut execution, 0), Poll::WaitingUntil(5000));
    for _ in 0..4 {
        let pending = action(&mut execution, 5000);
        execution.acknowledge(pending.ticket, Ok(())).unwrap();
    }
    let result = complete(&mut execution);
    assert_eq!(result.outcome, Outcome::Completed);
    assert_eq!(result.operation_errors, ["make rejected"]);
}

#[test]
fn driver_brightness_turbo_uses_source_step_ten() {
    let event = parse_event(
        &json!({"type":"display","id":"driverBrightnessDown"}),
        false,
    )
    .unwrap();
    assert!(matches!(
        event,
        ParsedEvent::Action(Action::Display(DisplayAction::BrightnessDown(10)))
    ));
}

#[test]
fn cancellation_during_turbo_gap_does_not_start_another_cycle() {
    let mut library = Library::default();
    library
        .publish_turbos(
            TurboSource::User,
            &json!([row("t", json!({"events":[{"type":"delay","ms":0}]}))]),
        )
        .unwrap();
    let mut execution = Execution::turbo(&library, "t", 0, 1000).unwrap();
    for _ in 0..2 {
        let pending = action(&mut execution, 0);
        execution.acknowledge(pending.ticket, Ok(())).unwrap();
    }
    assert_eq!(next(&mut execution, 0), Poll::WaitingUntil(1_000_000));
    execution.cancel();
    assert_eq!(complete(&mut execution).outcome, Outcome::Cancelled);
}

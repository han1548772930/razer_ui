use super::*;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
};

#[derive(Default)]
struct Behavior {
    current: bool,
    fail_execute: bool,
    panic_execute: bool,
    fail_shutdown: bool,
    panic_shutdown: bool,
    fail_termination: bool,
    panic_termination: bool,
    panic_drop: bool,
    cancel_on_shutdown: bool,
}

struct Scripted {
    calls: Rc<RefCell<Vec<&'static str>>>,
    behavior: Behavior,
    valid: Arc<AtomicBool>,
}

fn report() -> Report {
    serde_json::from_value(serde_json::json!({
        "requested":{"field":"power_saving","value":9},
        "acknowledgements":[],"time_to_sleep":540,"critical_percent":null,
        "dim_before":null,"setter_sent":true
    }))
    .unwrap()
}

impl Transport for Scripted {
    fn execute(&mut self) -> anyhow::Result<Observation> {
        self.calls.borrow_mut().push("execute");
        assert!(!self.behavior.panic_execute, "scripted power failure");
        ensure!(!self.behavior.fail_execute, "device read failed");
        Ok(if self.behavior.current {
            Observation::Current(report())
        } else {
            Observation::Idle(420)
        })
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("shutdown");
        if self.behavior.cancel_on_shutdown {
            self.valid.store(false, Ordering::Release);
        }
        assert!(!self.behavior.panic_shutdown, "scripted shutdown panic");
        ensure!(!self.behavior.fail_shutdown, "shutdown failed");
        Ok(())
    }
    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("terminate");
        assert!(
            !self.behavior.panic_termination,
            "scripted termination panic"
        );
        ensure!(!self.behavior.fail_termination, "join failed");
        Ok(())
    }
}

impl Drop for Scripted {
    fn drop(&mut self) {
        self.calls.borrow_mut().push("drop");
        assert!(!self.behavior.panic_drop, "scripted drop panic");
    }
}

fn run(behavior: Behavior) -> (Completion, Vec<&'static str>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let valid = Arc::new(AtomicBool::new(true));
    let completion = run_with(&valid, || {
        Ok(Scripted {
            calls: calls.clone(),
            behavior,
            valid: valid.clone(),
        })
    });
    let log = calls.borrow().clone();
    (completion, log)
}

#[test]
fn idle_readback_is_delivered_only_after_shutdown_join_and_destruction() {
    let (completion, calls) = run(Behavior::default());
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.idle_seconds(), Some(420));
    assert!(completion.current_report().is_none());
    assert!(completion.cleanup_error().is_none());
}

#[test]
fn current_power_keeps_its_typed_report_and_confirmed_seconds() {
    let (completion, calls) = run(Behavior {
        current: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(
        completion.current_report().unwrap().time_to_sleep,
        Some(540)
    );
    assert!(completion.idle_seconds().is_none());
}

#[test]
fn failed_or_panicked_device_io_still_completes_every_cleanup_stage() {
    for behavior in [
        Behavior {
            fail_execute: true,
            ..Default::default()
        },
        Behavior {
            panic_execute: true,
            ..Default::default()
        },
    ] {
        let (completion, calls) = run(behavior);
        assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
        assert!(completion.result.is_err());
        assert!(completion.idle_seconds().is_none());
    }
}

#[test]
fn shutdown_and_join_failures_remain_separate_from_actual_device_confirmation() {
    let (completion, calls) = run(Behavior {
        fail_shutdown: true,
        fail_termination: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.idle_seconds(), Some(420));
    let cleanup = completion.cleanup_error().unwrap();
    assert!(cleanup.contains("shutdown failed"));
    assert!(cleanup.contains("join failed"));
}

#[test]
fn panicked_cleanup_stages_cannot_skip_later_cleanup_or_erase_the_confirmation() {
    let (completion, calls) = run(Behavior {
        panic_shutdown: true,
        panic_termination: true,
        panic_drop: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.idle_seconds(), Some(420));
    assert!(completion.shutdown_error.is_some());
    assert!(completion.termination_error.is_some());
    assert!(completion.destruction_error.is_some());
}

#[test]
fn device_and_cleanup_failures_preserve_each_independent_error() {
    let (completion, calls) = run(Behavior {
        fail_execute: true,
        fail_shutdown: true,
        fail_termination: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(
        completion
            .result
            .unwrap_err()
            .contains("device read failed")
    );
    assert!(
        completion
            .shutdown_error
            .unwrap()
            .contains("shutdown failed")
    );
    assert!(
        completion
            .termination_error
            .unwrap()
            .contains("join failed")
    );
}

#[test]
fn cancellation_before_poll_never_creates_a_native_service() {
    let completion = run_with::<Scripted>(&AtomicBool::new(false), || {
        panic!("factory called after cancel")
    });
    assert!(completion.result.is_err());
}

#[test]
fn cancellation_during_cleanup_rejects_the_late_observation() {
    let (completion, calls) = run(Behavior {
        cancel_on_shutdown: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.idle_seconds().is_none());
    assert!(completion.result.unwrap_err().contains("取消"));
}

#[test]
fn failed_service_creation_is_not_device_or_cleanup_success() {
    let completion = run_with::<Scripted>(&AtomicBool::new(true), || anyhow::bail!("spawn failed"));
    assert!(completion.result.unwrap_err().contains("spawn failed"));
    assert!(completion.shutdown_error.is_none());
    assert!(completion.termination_error.is_none());
}

use super::*;
use std::{cell::RefCell, rc::Rc, sync::Arc};

#[derive(Default)]
struct Behavior {
    fail_execute: bool,
    panic_execute: bool,
    fail_shutdown: bool,
    panic_shutdown: bool,
    fail_termination: bool,
    panic_termination: bool,
    panic_drop: bool,
    cancel_after_execute: bool,
    cancel_on_shutdown: bool,
}

struct Scripted {
    calls: Rc<RefCell<Vec<&'static str>>>,
    behavior: Behavior,
    valid: Arc<AtomicBool>,
}

impl Transport for Scripted {
    fn execute(&mut self) -> anyhow::Result<u32> {
        self.calls.borrow_mut().push("execute");
        assert!(!self.behavior.panic_execute, "scripted polling failure");
        ensure!(!self.behavior.fail_execute, "device readback failed");
        if self.behavior.cancel_after_execute {
            self.valid.store(false, Ordering::Release);
        }
        Ok(500)
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("shutdown");
        assert!(!self.behavior.panic_shutdown, "scripted shutdown panic");
        if self.behavior.cancel_on_shutdown {
            self.valid.store(false, Ordering::Release);
        }
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
        assert!(!self.behavior.panic_drop, "scripted transport drop panic");
    }
}

fn simulate(behavior: Behavior) -> (Completion, Vec<&'static str>) {
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
fn actual_readback_is_published_after_shutdown_termination_and_destruction() {
    let (completion, calls) = simulate(Behavior::default());
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.observed(), Some(500));
    assert!(completion.cleanup_error().is_none());
    assert!(completion.message().contains("500 Hz"));
}

#[test]
fn failed_readback_still_cleans_up_in_full_order() {
    let (completion, calls) = simulate(Behavior {
        fail_execute: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.observed().is_none());
    assert!(
        completion
            .result
            .as_ref()
            .unwrap_err()
            .contains("device readback failed")
    );
    assert!(!completion.message().contains("已回读确认"));
}

#[test]
fn panicked_operation_cannot_skip_cleanup_or_publish_success() {
    let (completion, calls) = simulate(Behavior {
        panic_execute: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.observed().is_none());
    assert!(completion.result.is_err());
}

#[test]
fn valid_observation_survives_independently_reported_cleanup_failures() {
    let (completion, calls) = simulate(Behavior {
        fail_shutdown: true,
        fail_termination: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.observed(), Some(500));
    let cleanup = completion.cleanup_error().unwrap();
    assert!(cleanup.contains("shutdown failed"));
    assert!(cleanup.contains("join failed"));
    assert!(completion.message().contains("500 Hz"));
    assert!(completion.message().contains("通信清理"));
}

#[test]
fn device_failure_and_each_cleanup_failure_are_preserved_together() {
    let (completion, calls) = simulate(Behavior {
        fail_execute: true,
        fail_shutdown: true,
        fail_termination: true,
        panic_drop: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(
        completion
            .result
            .unwrap_err()
            .contains("device readback failed")
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
    assert!(completion.destruction_error.is_some());
}

#[test]
fn shutdown_panic_does_not_skip_termination_or_destroy_valid_actual() {
    let (completion, calls) = simulate(Behavior {
        panic_shutdown: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.observed(), Some(500));
    assert!(completion.shutdown_error.is_some());
    assert!(completion.termination_error.is_none());
}

#[test]
fn termination_panic_does_not_skip_transport_destruction() {
    let (completion, calls) = simulate(Behavior {
        panic_termination: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.observed(), Some(500));
    assert!(completion.termination_error.is_some());
    assert!(completion.destruction_error.is_none());
}

#[test]
fn destruction_panic_is_separate_from_valid_actual() {
    let (completion, calls) = simulate(Behavior {
        panic_drop: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.observed(), Some(500));
    assert!(completion.destruction_error.is_some());
    assert!(
        completion
            .cleanup_error()
            .unwrap()
            .contains("通信所有者释放失败")
    );
}

#[test]
fn already_canceled_scope_never_creates_a_transport() {
    let completion = run_with::<Scripted>(&AtomicBool::new(false), || {
        panic!("factory called after cancellation")
    });
    assert!(completion.observed().is_none());
    assert!(completion.result.unwrap_err().contains("取消"));
}

#[test]
fn cancellation_after_device_operation_suppresses_old_scope_actual() {
    let (completion, calls) = simulate(Behavior {
        cancel_after_execute: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.observed().is_none());
    assert!(completion.result.unwrap_err().contains("取消"));
}

#[test]
fn cancellation_during_cleanup_suppresses_old_scope_actual() {
    let (completion, calls) = simulate(Behavior {
        cancel_on_shutdown: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.observed().is_none());
    assert!(completion.result.unwrap_err().contains("取消"));
}

#[test]
fn canceled_during_service_creation_skips_execute_but_completes_cleanup() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let valid = Arc::new(AtomicBool::new(true));
    let completion = run_with(&valid, || {
        valid.store(false, Ordering::Release);
        Ok(Scripted {
            calls: calls.clone(),
            behavior: Behavior::default(),
            valid: valid.clone(),
        })
    });
    assert_eq!(*calls.borrow(), ["shutdown", "terminate", "drop"]);
    assert!(completion.observed().is_none());
    assert!(completion.result.unwrap_err().contains("取消"));
}

#[test]
fn service_creation_failure_is_neither_device_confirmation_nor_cleanup_success() {
    let completion = run_with::<Scripted>(&AtomicBool::new(true), || anyhow::bail!("spawn failed"));
    assert!(completion.observed().is_none());
    assert!(completion.result.unwrap_err().contains("spawn failed"));
    assert!(completion.shutdown_error.is_none());
    assert!(completion.termination_error.is_none());
    assert!(completion.destruction_error.is_none());
}

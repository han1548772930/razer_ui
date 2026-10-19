use super::*;
use razer_device::mouse_dpi_stages::PackedDpiStage;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Copy, Default)]
enum Fault {
    #[default]
    None,
    Error,
    Panic,
}

impl Fault {
    fn check(self, error: &'static str) -> anyhow::Result<()> {
        match self {
            Self::None => Ok(()),
            Self::Error => anyhow::bail!(error),
            Self::Panic => panic!("{error}"),
        }
    }
}

#[derive(Default)]
struct Behavior {
    execute: Fault,
    shutdown: Fault,
    termination: Fault,
    panic_drop: bool,
    cancel_execute: bool,
    cancel_shutdown: bool,
}

struct Scripted {
    calls: Rc<RefCell<Vec<&'static str>>>,
    valid: Rc<Cell<bool>>,
    behavior: Behavior,
}

fn actual() -> DpiStagesReading {
    DpiStagesReading {
        profile_id: 1,
        active_stage: 1,
        number_of_active_stages: 1,
        records: vec![PackedDpiStage {
            index: 1,
            x: 2300,
            y: 2400,
            z: 0,
        }],
        response_bytes: 10,
    }
}

impl Transport for Scripted {
    fn execute(&mut self) -> anyhow::Result<DpiStagesReading> {
        self.calls.borrow_mut().push("execute");
        self.behavior.execute.check("table getter failed")?;
        if self.behavior.cancel_execute {
            self.valid.set(false);
        }
        Ok(actual())
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("shutdown");
        if self.behavior.cancel_shutdown {
            self.valid.set(false);
        }
        self.behavior.shutdown.check("shutdown receipt missing")
    }

    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("terminate");
        self.behavior.termination.check("pipe join failed")
    }
}

impl Drop for Scripted {
    fn drop(&mut self) {
        self.calls.borrow_mut().push("drop");
        assert!(!self.behavior.panic_drop, "transport destruction failed");
    }
}

fn simulate(behavior: Behavior) -> (Completion, Vec<&'static str>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let valid = Rc::new(Cell::new(true));
    let completion = run_with(
        || valid.get(),
        || {
            Ok(Scripted {
                calls: calls.clone(),
                valid: valid.clone(),
                behavior,
            })
        },
    );
    let log = calls.borrow().clone();
    (completion, log)
}

#[::core::prelude::v1::test]
fn dpi_stage_worker_actual_readback_is_delivered_after_all_cleanup_stages() {
    let (completion, calls) = simulate(Behavior::default());
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.reading(), Some(&actual()));
    assert!(completion.cleanup_error().is_none());
    assert!(completion.message(true, true).contains("已回读确认"));
    assert!(completion.message(false, true).contains("已读取设备"));
}

#[::core::prelude::v1::test]
fn dpi_stage_worker_cleanup_failures_keep_actual_readback_and_each_cleanup_error() {
    let (completion, calls) = simulate(Behavior {
        shutdown: Fault::Error,
        termination: Fault::Error,
        panic_drop: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.reading(), Some(&actual()));
    let cleanup = completion.cleanup_error().unwrap();
    assert!(cleanup.contains("shutdown receipt missing"));
    assert!(cleanup.contains("pipe join failed"));
    assert!(completion.destruction_error.is_some());
    let stale = completion.message(true, false);
    assert!(stale.contains("旧设备阶段表结果已丢弃"));
    assert!(stale.contains("shutdown receipt missing"));
    assert!(!stale.contains("已回读确认"));
}

#[::core::prelude::v1::test]
fn dpi_stage_worker_read_failure_does_not_hide_cleanup_failures() {
    let (completion, calls) = simulate(Behavior {
        execute: Fault::Error,
        shutdown: Fault::Error,
        termination: Fault::Error,
        panic_drop: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.reading().is_none());
    assert!(
        completion
            .result
            .as_ref()
            .unwrap_err()
            .contains("table getter failed")
    );
    assert!(completion.shutdown_error.is_some());
    assert!(completion.termination_error.is_some());
    assert!(completion.destruction_error.is_some());
    let message = completion.message(false, true);
    assert!(message.contains("table getter failed"));
    assert!(message.contains("shutdown receipt missing"));
    assert!(!message.contains("已读取设备"));
}

#[::core::prelude::v1::test]
fn dpi_stage_worker_panics_cannot_skip_remaining_cleanup() {
    let (completion, calls) = simulate(Behavior {
        execute: Fault::Panic,
        shutdown: Fault::Panic,
        termination: Fault::Panic,
        panic_drop: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.result.is_err());
    assert!(completion.shutdown_error.is_some());
    assert!(completion.termination_error.is_some());
    assert!(completion.destruction_error.is_some());
}

#[::core::prelude::v1::test]
fn dpi_stage_worker_cancel_during_transfer_or_release_never_reports_success() {
    for behavior in [
        Behavior {
            cancel_execute: true,
            ..Default::default()
        },
        Behavior {
            cancel_shutdown: true,
            ..Default::default()
        },
    ] {
        let (completion, calls) = simulate(behavior);
        assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
        assert!(completion.result.as_ref().unwrap_err().contains("取消"));
        assert!(completion.reading().is_none());
        assert!(completion.cleanup_error().is_none());
        assert!(!completion.message(true, true).contains("已回读确认"));
    }
}

#[::core::prelude::v1::test]
fn dpi_stage_worker_canceled_admission_never_creates_a_transport() {
    let completion = run_with::<Scripted>(|| false, || panic!("canceled owner must not spawn"));
    assert!(completion.result.as_ref().unwrap_err().contains("取消"));
    assert!(completion.cleanup_error().is_none());
}

#[::core::prelude::v1::test]
fn dpi_stage_worker_creation_failure_is_not_confused_with_transport_cleanup() {
    let completion = run_with::<Scripted>(|| true, || anyhow::bail!("transport creation failed"));
    assert!(
        completion
            .result
            .as_ref()
            .unwrap_err()
            .contains("transport creation failed")
    );
    assert!(completion.cleanup_error().is_none());
}

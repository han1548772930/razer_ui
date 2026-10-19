use super::*;
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

impl Transport for Scripted {
    fn execute(&mut self) -> anyhow::Result<DeviceWriteAck> {
        self.calls.borrow_mut().push("execute");
        self.behavior.execute.check("setter rejected")?;
        if self.behavior.cancel_execute {
            self.valid.set(false);
        }
        Ok(DeviceWriteAck {
            command: [7, 4, 5],
            data: vec![9, 8],
        })
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
fn legacy_dpi_worker_acknowledgement_waits_for_all_cleanup_stages() {
    let (completion, calls) = simulate(Behavior::default());
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(
        completion.result.as_ref().unwrap(),
        &DeviceWriteAck {
            command: [7, 4, 5],
            data: vec![9, 8],
        }
    );
    assert!(completion.cleanup_error().is_none());
    assert!(
        completion
            .message("DPI 已保存到本机", true, 1600, 1700)
            .contains("X 1600 / Y 1700")
    );
}

#[::core::prelude::v1::test]
fn legacy_dpi_worker_cleanup_failure_preserves_actual_acknowledgement() {
    let (completion, calls) = simulate(Behavior {
        shutdown: Fault::Error,
        termination: Fault::Error,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(completion.result.as_ref().unwrap().data, [9, 8]);
    let cleanup = completion.cleanup_error().unwrap();
    assert!(cleanup.contains("shutdown receipt missing"));
    assert!(cleanup.contains("pipe join failed"));
    let message = completion.message("临时 DPI 设置", true, 1600, 1700);
    assert!(message.contains("设备已应答"));
    assert!(message.contains("通信清理"));
    let stale = completion.message("临时 DPI 设置", false, 1600, 1700);
    assert!(stale.contains("旧设备应答已丢弃"));
    assert!(stale.contains("shutdown receipt missing"));
    assert!(!stale.contains("设备已应答"));
}

#[::core::prelude::v1::test]
fn legacy_dpi_worker_operation_failure_does_not_discard_cleanup_failures() {
    let (completion, calls) = simulate(Behavior {
        execute: Fault::Error,
        shutdown: Fault::Error,
        termination: Fault::Error,
        panic_drop: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(
        completion
            .result
            .as_ref()
            .unwrap_err()
            .contains("setter rejected")
    );
    assert!(completion.shutdown_error.is_some());
    assert!(completion.termination_error.is_some());
    assert!(completion.destruction_error.is_some());
    let message = completion.message("临时 DPI 设置", true, 1600, 1700);
    assert!(message.contains("setter rejected"));
    assert!(message.contains("shutdown receipt missing"));
    assert!(message.contains("pipe join failed"));
    assert!(!message.contains("设备已应答"));
}

#[::core::prelude::v1::test]
fn legacy_dpi_worker_panics_cannot_skip_remaining_cleanup() {
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
fn legacy_dpi_worker_cancelled_write_cleans_up_without_success() {
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
        assert!(completion.cleanup_error().is_none());
        assert!(
            !completion
                .message("临时 DPI 设置", true, 1600, 1700)
                .contains("设备已应答")
        );
    }
}

#[::core::prelude::v1::test]
fn legacy_dpi_worker_cancelled_before_creation_does_not_create_transport() {
    let completion = run_with::<Scripted>(|| false, || panic!("canceled owner must not spawn"));
    assert!(completion.result.as_ref().unwrap_err().contains("取消"));
    assert!(completion.cleanup_error().is_none());
}

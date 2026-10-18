use super::*;
use anyhow::ensure;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

struct Backend {
    calls: RefCell<Vec<&'static str>>,
    batches: VecDeque<anyhow::Result<Vec<InputEvent>>>,
    start_failure: bool,
    stop_failure: bool,
    termination_failures: usize,
}
impl Backend {
    fn new() -> Self {
        Self {
            calls: RefCell::new(vec![]),
            batches: VecDeque::new(),
            start_failure: false,
            stop_failure: false,
            termination_failures: 0,
        }
    }
}
impl Monitor for Backend {
    fn start(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("start");
        ensure!(!self.start_failure, "lost START acknowledgement");
        Ok(())
    }
    fn events(&mut self) -> anyhow::Result<Vec<InputEvent>> {
        self.calls.borrow_mut().push("poll");
        self.batches.pop_front().unwrap_or_else(|| Ok(vec![]))
    }
}
impl Cleanup for Backend {
    fn stop(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("stop");
        ensure!(!self.stop_failure, "lost STOP acknowledgement");
        Ok(())
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("shutdown");
        Ok(())
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("terminate");
        if self.termination_failures > 0 {
            self.termination_failures -= 1;
            anyhow::bail!("process still owned");
        }
        Ok(())
    }
}
fn batch(container: &str, pid: u32) -> InputEvent {
    InputEvent::from_native(&format!(r#"{{"containerId":"{container}","productId":{pid},"vendorId":5426}}"#),1,
        r#"[{"timeTick":10000,"data":"{\"type\":\"mouseMove\",\"x\":40,\"y\":40}"},{"timeTick":20000,"data":"{\"type\":\"mouseMove\",\"x\":3,\"y\":4}"}]"#.into()).unwrap()
}
#[test]
fn mouse_dynamic_monitor_cancellation_still_stops_shuts_down_and_reaps_before_release() {
    let mut backend = Backend::new();
    backend.batches.push_back(Ok(vec![batch("current", 190)]));
    let active = AtomicBool::new(true);
    let observed = RefCell::new(vec![]);
    let completion = monitor_with(
        &mut backend,
        &active,
        |event| event.container_id() == "current",
        |update| match update {
            Update::MonitoringStarted => observed.borrow_mut().push(-1.),
            Update::Speed(value) => observed.borrow_mut().push(value),
        },
        || Duration::ZERO,
        || active.store(false, Ordering::Release),
    );
    assert!(matches!(completion.result, Ok(Outcome::MonitoringStopped)));
    assert!(completion.cleanup_failure.is_none());
    assert_eq!(*observed.borrow(), vec![-1., 5.]);
    assert_eq!(
        *backend.calls.borrow(),
        vec!["start", "poll", "stop", "shutdown", "terminate"]
    );
}
#[test]
fn mouse_dynamic_partial_start_failure_retains_stop_and_shutdown_obligations() {
    let mut backend = Backend::new();
    backend.start_failure = true;
    let result = monitor_with(
        &mut backend,
        &AtomicBool::new(true),
        |_| true,
        |_| panic!("unacknowledged start cannot publish"),
        || Duration::ZERO,
        || panic!("no monitoring loop after failed start"),
    );
    assert!(result.result.is_err());
    assert_eq!(
        *backend.calls.borrow(),
        vec!["start", "stop", "shutdown", "terminate"]
    );
}
#[test]
fn mouse_dynamic_monitor_ignores_another_container_and_keeps_native_failure_cleanup() {
    let mut backend = Backend::new();
    backend.batches.push_back(Ok(vec![batch("other", 190)]));
    backend
        .batches
        .push_back(Err(anyhow::anyhow!("callback queue overflow")));
    let speeds = Cell::new(0);
    let result = monitor_with(
        &mut backend,
        &AtomicBool::new(true),
        |event| event.container_id() == "current",
        |update| {
            if matches!(update, Update::Speed(_)) {
                speeds.set(speeds.get() + 1);
            }
        },
        || Duration::ZERO,
        || {},
    );
    assert!(
        result
            .result
            .unwrap_err()
            .contains("callback queue overflow")
    );
    assert_eq!(speeds.get(), 0);
    assert_eq!(
        &backend.calls.borrow()[3..],
        &["stop", "shutdown", "terminate"]
    );
}
#[test]
fn mouse_dynamic_monitor_retains_lease_until_termination_retry_and_keeps_failures_visible() {
    let mut backend = Backend::new();
    backend.stop_failure = true;
    backend.termination_failures = 1;
    let active = AtomicBool::new(true);
    let waits = Cell::new(0);
    let result = monitor_with(
        &mut backend,
        &active,
        |_| true,
        |_| {},
        || Duration::ZERO,
        || {
            active.store(false, Ordering::Release);
            waits.set(waits.get() + 1);
        },
    );
    let failure = result.cleanup_failure.unwrap();
    assert!(failure.contains("lost STOP acknowledgement"));
    assert!(failure.contains("process still owned"));
    assert_eq!(
        &backend.calls.borrow()[2..],
        &["stop", "shutdown", "terminate", "terminate"]
    );
    assert_eq!(waits.get(), 2);
}
#[test]
fn mouse_dynamic_monitor_canceled_before_admission_never_starts_or_polls() {
    let mut backend = Backend::new();
    let result = monitor_with(
        &mut backend,
        &AtomicBool::new(false),
        |_| true,
        |_| panic!("canceled monitor cannot publish"),
        || Duration::ZERO,
        || {},
    );
    assert!(result.result.is_err());
    assert_eq!(
        *backend.calls.borrow(),
        vec!["stop", "shutdown", "terminate"]
    );
}

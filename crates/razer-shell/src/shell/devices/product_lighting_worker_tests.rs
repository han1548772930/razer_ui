use super::*;
use std::cell::RefCell;
use std::rc::Rc;

struct Fake<'a> {
    calls: Rc<RefCell<Vec<&'static str>>>,
    canceled: &'a AtomicBool,
    fail_execute: bool,
    panic_shutdown: bool,
    fail_reaping: bool,
    cancel_after_execute: bool,
}
impl Transport for Fake<'_> {
    fn execute(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("execute");
        if self.cancel_after_execute {
            self.canceled.store(true, Ordering::Release);
        }
        ensure!(!self.fail_execute, "synthetic device rejection");
        Ok(())
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("shutdown");
        assert!(!self.panic_shutdown, "synthetic shutdown panic");
        Ok(())
    }
    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("reap");
        ensure!(!self.fail_reaping, "synthetic reaping failure");
        Ok(())
    }
}
impl Drop for Fake<'_> {
    fn drop(&mut self) {
        self.calls.borrow_mut().push("drop");
    }
}
fn fake<'a>(calls: Rc<RefCell<Vec<&'static str>>>, canceled: &'a AtomicBool) -> Fake<'a> {
    Fake {
        calls,
        canceled,
        fail_execute: false,
        panic_shutdown: false,
        fail_reaping: false,
        cancel_after_execute: false,
    }
}

#[::core::prelude::v1::test]
fn brightness_worker_keeps_device_and_every_cleanup_failure() {
    let active = AtomicBool::new(true);
    let canceled = AtomicBool::new(false);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let result = run_with(&active, &canceled, || {
        let mut transport = fake(calls.clone(), &canceled);
        transport.fail_execute = true;
        transport.panic_shutdown = true;
        transport.fail_reaping = true;
        Ok(transport)
    });
    let error = format!(
        "{:#}",
        result.err().expect("failed device and cleanup must fail")
    );
    assert!(error.contains("synthetic device rejection"));
    assert!(error.contains("shutdown:"));
    assert!(error.contains("synthetic reaping failure"));
    assert_eq!(&*calls.borrow(), &["execute", "shutdown", "reap", "drop"]);
}

#[::core::prelude::v1::test]
fn brightness_worker_cancellation_after_write_still_cleans_up_and_never_completes() {
    let active = AtomicBool::new(true);
    let canceled = AtomicBool::new(false);
    let calls = Rc::new(RefCell::new(Vec::new()));
    let result = run_with(&active, &canceled, || {
        let mut transport = fake(calls.clone(), &canceled);
        transport.cancel_after_execute = true;
        Ok(transport)
    });
    assert!(result.is_err());
    assert_eq!(&*calls.borrow(), &["execute", "shutdown", "reap", "drop"]);
}

#[::core::prelude::v1::test]
fn brightness_worker_stopped_page_or_connection_does_not_create_a_transport() {
    for (active, canceled) in [(false, false), (true, true)] {
        let active = AtomicBool::new(active);
        let canceled = AtomicBool::new(canceled);
        let result = run_with::<Fake<'_>>(&active, &canceled, || {
            panic!("Canceled owners must not spawn")
        });
        assert!(result.is_err());
    }
}

#[::core::prelude::v1::test]
fn brightness_worker_success_waits_for_shutdown_reaping_and_release() {
    let active = AtomicBool::new(true);
    let canceled = AtomicBool::new(false);
    let calls = Rc::new(RefCell::new(Vec::new()));
    assert!(matches!(
        run_with(&active, &canceled, || Ok(fake(calls.clone(), &canceled))),
        Ok(Reply::Submitted)
    ));
    assert_eq!(&*calls.borrow(), &["execute", "shutdown", "reap", "drop"]);
}

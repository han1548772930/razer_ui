use super::{Completion, Transport, run_with};
use std::{
    cell::Cell,
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Read,
    Set(u8),
    Shutdown,
    Terminate,
    Release,
}

struct Gate {
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
}

impl Gate {
    fn new() -> Self {
        Self {
            entered: Arc::new(Barrier::new(2)),
            release: Arc::new(Barrier::new(2)),
        }
    }

    fn wait(&self) {
        self.entered.wait();
        self.release.wait();
    }
}

struct Mock {
    log: Arc<Mutex<Vec<Event>>>,
    operation_error: Option<&'static str>,
    shutdown_error: Option<&'static str>,
    termination_error: Option<&'static str>,
    operation_panics: bool,
    operation_gate: Option<Gate>,
    cleanup_gate: Option<Gate>,
    release_gate: Option<Gate>,
}

impl Mock {
    fn new(log: &Arc<Mutex<Vec<Event>>>) -> Self {
        Self {
            log: log.clone(),
            operation_error: None,
            shutdown_error: None,
            termination_error: None,
            operation_panics: false,
            operation_gate: None,
            cleanup_gate: None,
            release_gate: None,
        }
    }
}

impl Transport for Mock {
    fn brightness(&mut self, percent: Option<u8>) -> anyhow::Result<u8> {
        self.log
            .lock()
            .unwrap()
            .push(percent.map_or(Event::Read, Event::Set));
        if let Some(gate) = self.operation_gate.take() {
            gate.wait();
        }
        assert!(
            !self.operation_panics,
            "simulated brightness transport panic"
        );
        if let Some(error) = self.operation_error {
            anyhow::bail!(error);
        }
        Ok(percent.unwrap_or(42))
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.log.lock().unwrap().push(Event::Shutdown);
        if let Some(gate) = self.cleanup_gate.take() {
            gate.wait();
        }
        if let Some(error) = self.shutdown_error {
            anyhow::bail!(error);
        }
        Ok(())
    }

    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.log.lock().unwrap().push(Event::Terminate);
        if let Some(error) = self.termination_error {
            anyhow::bail!(error);
        }
        Ok(())
    }
}

impl Drop for Mock {
    fn drop(&mut self) {
        if let Some(gate) = self.release_gate.take() {
            gate.wait();
        }
        self.log.lock().unwrap().push(Event::Release);
    }
}

fn events(log: &Arc<Mutex<Vec<Event>>>) -> Vec<Event> {
    log.lock().unwrap().clone()
}

#[test]
fn cancellation_while_queued_does_not_create_a_service() {
    let canceled = AtomicBool::new(true);
    let created = Cell::new(false);
    let log = Arc::new(Mutex::new(Vec::new()));
    let completion = run_with(None, &canceled, || {
        created.set(true);
        Ok(Mock::new(&log))
    });
    assert!(!created.get());
    assert_eq!(completion.observed(), None);
    assert!(completion.error().unwrap().contains("已取消"));
    assert!(completion.cleanup_error().is_none());
    assert!(events(&log).is_empty());
}

#[test]
fn cancellation_during_service_creation_stops_the_first_device_request_and_cleans_up() {
    let canceled = Arc::new(AtomicBool::new(false));
    let log = Arc::new(Mutex::new(Vec::new()));
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let worker = {
        let signal = canceled.clone();
        let log = log.clone();
        let entered = entered.clone();
        let release = release.clone();
        thread::spawn(move || {
            run_with(Some(63), &signal, || {
                entered.wait();
                release.wait();
                Ok(Mock::new(&log))
            })
        })
    };
    entered.wait();
    canceled.store(true, Ordering::Release);
    release.wait();
    let completion = worker.join().unwrap();
    assert_eq!(completion.observed(), None);
    assert!(completion.error().unwrap().contains("已取消"));
    assert_eq!(
        events(&log),
        [Event::Shutdown, Event::Terminate, Event::Release]
    );
}

#[test]
fn canceled_read_rejects_the_late_response_and_still_runs_all_cleanup() {
    let canceled = Arc::new(AtomicBool::new(false));
    let log = Arc::new(Mutex::new(Vec::new()));
    let gate = Gate::new();
    let entered = gate.entered.clone();
    let release = gate.release.clone();
    let mut transport = Mock::new(&log);
    transport.operation_gate = Some(gate);
    let worker = {
        let signal = canceled.clone();
        thread::spawn(move || run_with(None, &signal, || Ok(transport)))
    };
    entered.wait();
    canceled.store(true, Ordering::Release);
    release.wait();
    let completion = worker.join().unwrap();
    assert_eq!(completion.observed(), None);
    assert!(completion.error().unwrap().contains("已取消"));
    assert_eq!(
        events(&log),
        [
            Event::Read,
            Event::Shutdown,
            Event::Terminate,
            Event::Release
        ]
    );
}

#[test]
fn accepted_setter_stays_pending_until_shutdown_and_transport_release_finish() {
    let mut state = super::super::State::default();
    let (_, accepted) = state.accept().unwrap();
    let canceled = AtomicBool::new(false);
    let log = Arc::new(Mutex::new(Vec::new()));
    let gate = Gate::new();
    let entered = gate.entered.clone();
    let release = gate.release.clone();
    let mut transport = Mock::new(&log);
    transport.release_gate = Some(gate);
    let (sender, receiver) = mpsc::channel::<Completion>();
    assert!(state.pending());
    let worker = thread::spawn(move || {
        let completion = {
            let _accepted = accepted;
            run_with(Some(63), &canceled, || Ok(transport))
        };
        sender.send(completion).unwrap();
    });
    entered.wait();
    assert!(state.pending());
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(
        events(&log),
        [Event::Set(63), Event::Shutdown, Event::Terminate]
    );
    release.wait();
    let completion = receiver.recv().unwrap();
    worker.join().unwrap();
    assert_eq!(completion.observed(), Some(63));
    assert!(completion.error().is_none());
    assert!(!state.pending());
    assert_eq!(events(&log).last(), Some(&Event::Release));
}

#[test]
fn operation_shutdown_and_termination_failures_are_all_retained() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut transport = Mock::new(&log);
    transport.operation_error = Some("actual getter failed");
    transport.shutdown_error = Some("native shutdown receipt missing");
    transport.termination_error = Some("owned process not reaped");
    let completion = run_with(None, &AtomicBool::new(false), || Ok(transport));
    assert_eq!(completion.observed(), None);
    let error = completion.error().unwrap();
    assert!(error.contains("actual getter failed"));
    assert!(error.contains("native shutdown receipt missing"));
    assert!(error.contains("owned process not reaped"));
    assert_eq!(
        events(&log),
        [
            Event::Read,
            Event::Shutdown,
            Event::Terminate,
            Event::Release
        ]
    );
}

#[test]
fn cleanup_failure_preserves_the_real_readback_as_a_separate_fact() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut transport = Mock::new(&log);
    transport.shutdown_error = Some("native shutdown failed");
    let completion = run_with(Some(72), &AtomicBool::new(false), || Ok(transport));
    assert_eq!(completion.observed(), Some(72));
    assert!(
        completion
            .cleanup_error()
            .unwrap()
            .contains("native shutdown failed")
    );
    assert_eq!(
        events(&log),
        [
            Event::Set(72),
            Event::Shutdown,
            Event::Terminate,
            Event::Release
        ]
    );
}

#[test]
fn cancellation_during_shutdown_waits_for_cleanup_and_rejects_the_retired_reading() {
    let canceled = Arc::new(AtomicBool::new(false));
    let log = Arc::new(Mutex::new(Vec::new()));
    let gate = Gate::new();
    let entered = gate.entered.clone();
    let release = gate.release.clone();
    let mut transport = Mock::new(&log);
    transport.cleanup_gate = Some(gate);
    let worker = {
        let signal = canceled.clone();
        thread::spawn(move || run_with(Some(63), &signal, || Ok(transport)))
    };
    entered.wait();
    canceled.store(true, Ordering::Release);
    release.wait();
    let completion = worker.join().unwrap();
    assert_eq!(completion.observed(), None);
    assert!(completion.error().unwrap().contains("已取消"));
    assert_eq!(
        events(&log),
        [
            Event::Set(63),
            Event::Shutdown,
            Event::Terminate,
            Event::Release
        ]
    );
}

#[test]
fn transport_panic_still_completes_shutdown_termination_and_release() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut transport = Mock::new(&log);
    transport.operation_panics = true;
    let completion = run_with(None, &AtomicBool::new(false), || Ok(transport));
    assert_eq!(completion.observed(), None);
    assert!(completion.error().unwrap().contains("panicked"));
    assert_eq!(
        events(&log),
        [
            Event::Read,
            Event::Shutdown,
            Event::Terminate,
            Event::Release
        ]
    );
}

#[test]
fn service_creation_failure_is_not_a_reading_or_a_shutdown_receipt() {
    let completion = run_with::<Mock>(None, &AtomicBool::new(false), || {
        anyhow::bail!("service process creation failed")
    });
    assert_eq!(completion.observed(), None);
    assert_eq!(
        completion.error().as_deref(),
        Some("service process creation failed")
    );
    assert!(completion.cleanup_error().is_none());
}

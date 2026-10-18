use super::{Admission, Command, Reply, Transport, run_with, verify_shutdown};
use razer_device::keyboard_submission::Intent;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

struct Mock {
    log: Arc<Mutex<Vec<String>>>,
    submit_barrier: Option<(Arc<Barrier>, Arc<Barrier>)>,
    cleanup_barrier: Option<(Arc<Barrier>, Arc<Barrier>)>,
    fail: Option<u64>,
    panic: Option<u64>,
    shutdown: Value,
}
impl Mock {
    fn new(log: &Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            log: log.clone(),
            submit_barrier: None,
            cleanup_barrier: None,
            fail: None,
            panic: None,
            shutdown: json!({"stopped":true}),
        }
    }
}
impl Transport for Mock {
    fn submit(&mut self, intent: Intent, valid: &AtomicBool) -> anyhow::Result<()> {
        let Intent::BottomDeadzone { value } = intent else {
            panic!("unexpected simulated command")
        };
        let generation = value.parse::<u64>().unwrap();
        self.log
            .lock()
            .unwrap()
            .push(format!("submit {generation}"));
        if let Some((entered, release)) = self.submit_barrier.take() {
            entered.wait();
            release.wait();
        }
        anyhow::ensure!(valid.load(Ordering::Acquire), "actual owner canceled");
        assert_ne!(self.panic, Some(generation), "simulated setter panic");
        anyhow::ensure!(self.fail != Some(generation), "actual setter failed");
        Ok(())
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.log.lock().unwrap().push("shutdown".into());
        if let Some((entered, release)) = self.cleanup_barrier.take() {
            entered.wait();
            release.wait();
        }
        verify_shutdown(&self.shutdown)
    }
}
impl Drop for Mock {
    fn drop(&mut self) {
        self.log.lock().unwrap().push("destroy transport".into());
    }
}

fn command(generation: u64) -> Command {
    Command {
        generation,
        intent: Intent::BottomDeadzone {
            value: generation.to_string(),
        },
    }
}

fn finish(reply: Reply, expected: u64) -> Result<(), String> {
    let Reply::Finished(generation, result) = reply else {
        panic!("expected setter completion")
    };
    assert_eq!(generation, expected);
    result
}

#[test]
fn graceful_close_drains_accepted_fifo_then_waits_for_shutdown_and_transport_destruction() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let cleanup_entered = Arc::new(Barrier::new(2));
    let cleanup_release = Arc::new(Barrier::new(2));
    let mut transport = Mock::new(&log);
    transport.submit_barrier = Some((entered.clone(), release.clone()));
    transport.cleanup_barrier = Some((cleanup_entered.clone(), cleanup_release.clone()));
    let valid = Arc::new(AtomicBool::new(true));
    let (commands, incoming) = mpsc::sync_channel(128);
    let (sender, replies) = mpsc::channel();
    let mut admission = Admission::new(commands);
    admission.submit(command(1), 1).unwrap();
    admission.submit(command(2), 2).unwrap();
    let worker = {
        let valid = valid.clone();
        thread::spawn(move || run_with(valid, incoming, sender, || Ok(transport)))
    };
    entered.wait();
    admission.stop_after_pending();
    admission.stop_after_pending();
    assert!(valid.load(Ordering::Acquire));
    assert!(replies.try_recv().is_err());
    release.wait();
    cleanup_entered.wait();
    for generation in [1, 2] {
        finish(replies.recv().unwrap(), generation).unwrap();
        admission.finish(generation);
    }
    assert!(replies.try_recv().is_err());
    assert!(!worker.is_finished());
    assert_eq!(*log.lock().unwrap(), ["submit 1", "submit 2", "shutdown"]);
    cleanup_release.wait();
    assert!(matches!(replies.recv().unwrap(), Reply::Stopped));
    worker.join().unwrap();
    assert_eq!(
        *log.lock().unwrap(),
        ["submit 1", "submit 2", "shutdown", "destroy transport"]
    );
    assert!(admission.retire().0.is_empty());
}

#[test]
fn canceled_exit_keeps_new_saves_for_a_successor_without_repeating_accepted_writes() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (commands, incoming) = mpsc::sync_channel(128);
    let mut admission = Admission::new(commands);
    admission.submit(command(1), 1).unwrap();
    admission.submit(command(2), 2).unwrap();
    admission.stop_after_pending();
    admission.submit(command(3), 3).unwrap();
    admission.submit(command(4), 4).unwrap();
    let (sender, replies) = mpsc::channel();
    run_with(Arc::new(AtomicBool::new(true)), incoming, sender, || {
        Ok(Mock::new(&log))
    });
    for generation in [1, 2] {
        finish(replies.recv().unwrap(), generation).unwrap();
        admission.finish(generation);
    }
    assert!(matches!(replies.recv().unwrap(), Reply::Stopped));
    let (unfinished, waiting) = admission.retire();
    assert!(unfinished.is_empty());
    assert_eq!(waiting.iter().copied().collect::<Vec<_>>(), [3, 4]);
    let (commands, incoming) = mpsc::sync_channel(128);
    let mut successor = Admission::new(commands);
    for generation in waiting {
        successor.submit(command(generation), generation).unwrap();
    }
    successor.stop_after_pending();
    let (sender, replies) = mpsc::channel();
    run_with(Arc::new(AtomicBool::new(true)), incoming, sender, || {
        Ok(Mock::new(&log))
    });
    finish(replies.recv().unwrap(), 3).unwrap();
    finish(replies.recv().unwrap(), 4).unwrap();
    assert!(matches!(replies.recv().unwrap(), Reply::Stopped));
    assert_eq!(
        *log.lock().unwrap(),
        [
            "submit 1",
            "submit 2",
            "shutdown",
            "destroy transport",
            "submit 3",
            "submit 4",
            "shutdown",
            "destroy transport",
        ]
    );
}

#[test]
fn real_scope_cancellation_rejects_queued_setters_but_finishes_cleanup() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let mut transport = Mock::new(&log);
    transport.submit_barrier = Some((entered.clone(), release.clone()));
    let valid = Arc::new(AtomicBool::new(true));
    let (commands, incoming) = mpsc::sync_channel(128);
    commands.send(command(1)).unwrap();
    commands.send(command(2)).unwrap();
    drop(commands);
    let (sender, replies) = mpsc::channel();
    let worker = {
        let valid = valid.clone();
        thread::spawn(move || run_with(valid, incoming, sender, || Ok(transport)))
    };
    entered.wait();
    valid.store(false, Ordering::Release);
    release.wait();
    assert!(
        finish(replies.recv().unwrap(), 1)
            .unwrap_err()
            .contains("actual owner canceled")
    );
    assert!(
        finish(replies.recv().unwrap(), 2)
            .unwrap_err()
            .contains("task was canceled")
    );
    assert!(matches!(replies.recv().unwrap(), Reply::Stopped));
    worker.join().unwrap();
    assert_eq!(
        *log.lock().unwrap(),
        ["submit 1", "shutdown", "destroy transport"]
    );
}

#[test]
fn setter_failure_keeps_its_actual_error_and_does_not_create_an_optimistic_success() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut transport = Mock::new(&log);
    transport.fail = Some(1);
    let (commands, incoming) = mpsc::sync_channel(128);
    commands.send(command(1)).unwrap();
    commands.send(command(2)).unwrap();
    drop(commands);
    let (sender, replies) = mpsc::channel();
    run_with(Arc::new(AtomicBool::new(true)), incoming, sender, || {
        Ok(transport)
    });
    assert!(
        finish(replies.recv().unwrap(), 1)
            .unwrap_err()
            .contains("actual setter failed")
    );
    finish(replies.recv().unwrap(), 2).unwrap();
    assert!(matches!(replies.recv().unwrap(), Reply::Stopped));
    assert!(replies.try_recv().is_err());
}

#[test]
fn startup_failure_rejects_every_accepted_generation_and_preserves_terminal_failure() {
    let (commands, incoming) = mpsc::sync_channel(128);
    commands.send(command(1)).unwrap();
    commands.send(command(2)).unwrap();
    drop(commands);
    let (sender, replies) = mpsc::channel();
    run_with::<Mock>(Arc::new(AtomicBool::new(true)), incoming, sender, || {
        anyhow::bail!("actual startup failure")
    });
    for generation in [1, 2] {
        assert!(
            finish(replies.recv().unwrap(), generation)
                .unwrap_err()
                .contains("actual startup failure")
        );
    }
    assert!(
        matches!(replies.recv().unwrap(), Reply::CleanupFailed(error) if error.contains("actual startup failure"))
    );
    assert!(matches!(replies.recv().unwrap(), Reply::Stopped));
}

#[test]
fn setter_panic_still_reports_failure_and_waits_for_transport_cleanup() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut transport = Mock::new(&log);
    transport.panic = Some(1);
    let (commands, incoming) = mpsc::sync_channel(128);
    commands.send(command(1)).unwrap();
    drop(commands);
    let (sender, replies) = mpsc::channel();
    run_with(Arc::new(AtomicBool::new(true)), incoming, sender, || {
        Ok(transport)
    });
    assert!(
        finish(replies.recv().unwrap(), 1)
            .unwrap_err()
            .contains("owner panicked")
    );
    assert!(matches!(replies.recv().unwrap(), Reply::Stopped));
    assert_eq!(
        *log.lock().unwrap(),
        ["submit 1", "shutdown", "destroy transport"]
    );
}

#[test]
fn invalid_shutdown_receipts_are_terminal_failures_after_transport_destruction() {
    for receipt in [
        Value::Null,
        json!({}),
        json!({"stopped":false}),
        json!({"stopped":"true"}),
    ] {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut transport = Mock::new(&log);
        transport.shutdown = receipt;
        let (commands, incoming) = mpsc::sync_channel(128);
        drop(commands);
        let (sender, replies) = mpsc::channel();
        run_with(Arc::new(AtomicBool::new(true)), incoming, sender, || {
            Ok(transport)
        });
        assert!(
            matches!(replies.recv().unwrap(), Reply::CleanupFailed(error) if error.contains("did not acknowledge shutdown"))
        );
        assert_eq!(*log.lock().unwrap(), ["shutdown", "destroy transport"]);
        assert!(matches!(replies.recv().unwrap(), Reply::Stopped));
    }
}

#[test]
fn active_and_successor_queues_reject_overflow_without_marking_failed_admission_pending() {
    let (commands, incoming) = mpsc::sync_channel(128);
    let mut admission = Admission::new(commands);
    for generation in 0..128 {
        admission.submit(command(generation), generation).unwrap();
    }
    assert!(
        admission
            .submit(command(128), 128)
            .unwrap_err()
            .contains("queue unavailable")
    );
    admission.stop_after_pending();
    for generation in 128..256 {
        admission.submit(command(generation), generation).unwrap();
    }
    assert!(
        admission
            .submit(command(256), 256)
            .unwrap_err()
            .contains("successor queue is full")
    );
    let (pending, waiting) = admission.retire();
    assert_eq!(pending.len(), 128);
    assert!(!pending.contains(&128));
    assert_eq!(waiting.len(), 128);
    assert_eq!(incoming.try_iter().count(), 128);
}

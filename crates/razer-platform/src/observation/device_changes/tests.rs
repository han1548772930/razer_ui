use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

fn coalesced_queue() -> (EventQueue, Receiver<DeviceChange>) {
    let (events, changes) = async_channel::bounded(1);
    (EventQueue::Coalesced(events), changes)
}

fn path() -> Vec<u16> {
    r"\\?\hid#same-interface".encode_utf16().collect()
}

fn changed(events: &EventPublisher) {
    events.interface(DeviceInterfaceAction::Arrival, path());
}

struct CallbackContext(Arc<AtomicBool>);

impl Drop for CallbackContext {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[test]
fn shutdown_joins_native_cleanup_and_closes_event_delivery() {
    let (events, changes) = coalesced_queue();
    let released = Arc::new(AtomicBool::new(false));
    let context = released.clone();
    let monitor = DeviceChangeMonitor::start_worker(events, move |stopped, events| {
        let _context = CallbackContext(context);
        wait_for_stop(stopped, &events);
        Ok(())
    })
    .unwrap();

    monitor.shutdown().unwrap();

    assert!(released.load(Ordering::Acquire));
    assert!(matches!(changes.try_recv(), Ok(DeviceChange::Ready)));
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn a_full_event_queue_cannot_block_shutdown_before_the_stop_wait() {
    let (events, changes) = coalesced_queue();
    let (enter, entered) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |stopped, events| {
        changed(&events);
        enter.send(()).unwrap();
        wait_for_stop(stopped, &events);
        Ok(())
    })
    .unwrap();
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    let (complete, completed) = mpsc::channel();
    let closing = std::thread::spawn(move || {
        complete.send(monitor.shutdown()).unwrap();
    });
    let result = completed.recv_timeout(Duration::from_secs(5));
    let event = changes.try_recv();
    let closed = changes.is_closed();
    // Release a regressed blocking publisher before joining the test thread.
    drop(changes);
    closing.join().unwrap();

    result
        .expect("shutdown must not wait for the event consumer")
        .unwrap();
    assert!(matches!(event, Ok(DeviceChange::Ready)));
    assert!(closed);
}

#[test]
fn drop_signals_stop_without_waiting_for_native_cleanup() {
    let (events, changes) = coalesced_queue();
    let (started, cleanup_started) = mpsc::channel();
    let (release, may_finish) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |stopped, _| {
        let _ = stopped.recv();
        started.send(()).unwrap();
        may_finish.recv().unwrap();
        Ok(())
    })
    .unwrap();
    let (complete, completed) = mpsc::channel();
    let dropping = std::thread::spawn(move || {
        drop(monitor);
        complete.send(()).unwrap();
    });
    let cleanup = cleanup_started.recv_timeout(Duration::from_secs(5));
    let nonblocking = completed.recv_timeout(Duration::from_secs(5));
    // Always release the simulated native cleanup, including a blocking-Drop
    // regression, so that the test can fail without leaving a stuck worker.
    release.send(()).unwrap();
    dropping.join().unwrap();

    assert!(cleanup.is_ok());
    assert!(nonblocking.is_ok(), "Drop must not wait for native cleanup");
    assert!(changes.recv_blocking().is_err());
}

#[test]
fn pending_shutdown_retains_worker_and_context_until_cleanup_is_terminal() {
    let (events, changes) = coalesced_queue();
    let released = Arc::new(AtomicBool::new(false));
    let context = released.clone();
    let (started, cleanup_started) = mpsc::channel();
    let (release, may_finish) = mpsc::channel();
    let mut monitor = DeviceChangeMonitor::start_worker(events, move |stopped, _| {
        let _context = CallbackContext(context);
        let _ = stopped.recv();
        started.send(()).unwrap();
        may_finish.recv().unwrap();
        Err("simulated first unregister failed; all resources now released".into())
    })
    .unwrap();
    monitor.request_stop();
    cleanup_started
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    let pending = monitor.try_shutdown();
    let has_worker = monitor.worker.is_some();
    let terminal = monitor.is_finished();
    let prematurely_released = released.load(Ordering::Acquire);
    let prematurely_closed = changes.is_closed();
    release.send(()).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !monitor.is_finished() && std::time::Instant::now() < deadline {
        std::thread::park_timeout(Duration::from_millis(1));
    }
    let result = monitor.try_shutdown();
    assert_eq!(pending, Ok(false));
    assert!(has_worker);
    assert!(!terminal);
    assert!(!prematurely_released);
    assert!(!prematurely_closed);
    assert_eq!(
        result.unwrap_err(),
        "simulated first unregister failed; all resources now released"
    );
    assert!(monitor.worker.is_none());
    assert!(released.load(Ordering::Acquire));
    assert!(changes.is_closed());
    assert_eq!(monitor.try_shutdown(), Ok(true));
}

#[test]
fn shutdown_preserves_cleanup_failure_and_closes_retained_callback_delivery() {
    let (events, changes) = coalesced_queue();
    let (retain, retained) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |_, events| {
        changed(&events);
        retain.send(events).unwrap();
        Err("simulated unregister failure; callback context retained".into())
    })
    .unwrap();

    assert_eq!(
        monitor.shutdown().unwrap_err(),
        "simulated unregister failure; callback context retained"
    );
    let callback_events = retained.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(changes.is_closed());
    assert!(matches!(
        changes.try_recv(),
        Ok(DeviceChange::Failed(error))
            if error == "simulated unregister failure; callback context retained"
    ));
    changed(&callback_events);
    assert!(changes.try_recv().is_err());
}

#[test]
fn a_panicked_worker_is_joined_and_cannot_keep_its_event_channel_open() {
    let (events, changes) = coalesced_queue();
    let (retain, retained) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |_, events| {
        retain.send(events).unwrap();
        panic!("simulated device-change worker panic");
    })
    .unwrap();

    assert_eq!(
        monitor.shutdown().unwrap_err(),
        "设备变化监听线程异常结束。"
    );
    let callback_events = retained.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
    changed(&callback_events);
    assert!(changes.try_recv().is_err());
}

#[test]
fn exact_remove_and_reinsert_at_the_same_path_are_not_coalesced() {
    let (events, changes) = EventQueue::exact(3);
    let (stop, stopped) = mpsc::sync_channel(1);
    let publisher = EventPublisher::new(events, stop);
    publisher.ready();
    publisher.interface(DeviceInterfaceAction::Removal, path());
    publisher.interface(DeviceInterfaceAction::Arrival, path());

    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Ready { sequence: 1 }
    );
    for (sequence, action) in [
        (2, DeviceInterfaceAction::Removal),
        (3, DeviceInterfaceAction::Arrival),
    ] {
        assert_eq!(
            changes.try_recv().unwrap(),
            ExactDeviceChange::Interface {
                sequence,
                action,
                path_utf16: path(),
            }
        );
    }
    assert!(changes.is_empty());
    assert!(!changes.is_closed());
    assert_eq!(stopped.try_recv(), Err(mpsc::TryRecvError::Empty));
}

#[test]
fn event_after_empty_observation_prevents_resuming_the_consumed_prefix() {
    let (queue, changes) = EventQueue::exact(3);
    let (stop, _stopped) = mpsc::sync_channel(1);
    let publisher = EventPublisher::new(queue, stop);
    let gate = publisher.exact_gate().unwrap();
    publisher.ready();
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Ready { sequence: 1 }
    );
    assert!(changes.is_empty());
    publisher.interface(DeviceInterfaceAction::Removal, path());
    let resumed = std::cell::Cell::new(false);
    assert!(
        !gate
            .try_resume(1, || {
                resumed.set(true);
                Ok(())
            })
            .unwrap()
    );
    assert!(!resumed.get());
    assert_eq!(gate.current_revision(), None);
    assert!(matches!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Interface { sequence: 2, .. }
    ));
    assert!(
        gate.try_resume(2, || {
            resumed.set(true);
            Ok(())
        })
        .unwrap()
    );
    assert!(resumed.get());
    assert_eq!(gate.current_revision(), Some(1));
}

#[test]
fn publishing_an_event_immediately_revokes_prior_admission_and_old_native_revisions() {
    let (queue, changes) = EventQueue::exact(4);
    let (stop, _stopped) = mpsc::sync_channel(1);
    let publisher = EventPublisher::new(queue, stop);
    let gate = publisher.exact_gate().unwrap();
    assert_eq!(gate.current_revision(), None);
    assert!(
        !gate
            .try_resume(0, || panic!("registration is not ready"))
            .unwrap()
    );
    publisher.ready();
    changes.try_recv().unwrap();
    assert!(gate.try_resume(1, || Ok(())).unwrap());
    let native_open_revision = gate.current_revision().unwrap();
    publisher.interface(DeviceInterfaceAction::Removal, path());
    assert_eq!(gate.current_revision(), None);
    assert!(!gate.is_current(native_open_revision));
    publisher.interface(DeviceInterfaceAction::Arrival, path());
    changes.try_recv().unwrap();
    changes.try_recv().unwrap();
    assert!(gate.try_resume(3, || Ok(())).unwrap());
    assert!(gate.is_current(2));
    assert!(!gate.is_current(native_open_revision));
    publisher.lost(DeviceChangeLoss::QueueOverflow);
    assert_eq!(gate.current_revision(), None);
    assert!(
        !gate
            .try_resume(3, || panic!("lost observation cannot resume"))
            .unwrap()
    );
}

#[test]
fn failed_resume_does_not_open_admission_and_stopping_permanently_revokes_the_gate() {
    let (queue, changes) = EventQueue::exact(2);
    let (stop, _stopped) = mpsc::sync_channel(1);
    let publisher = EventPublisher::new(queue, stop);
    let gate = publisher.exact_gate().unwrap();
    publisher.ready();
    changes.try_recv().unwrap();
    assert_eq!(
        gate.try_resume(1, || Err("simulated pool commit failed".into())),
        Err("simulated pool commit failed".into())
    );
    assert_eq!(gate.current_revision(), None);
    assert!(gate.try_resume(1, || Ok(())).unwrap());
    gate.queue().revoke();
    assert_eq!(gate.current_revision(), None);
    assert!(
        !gate
            .try_resume(1, || panic!("stopped observation cannot resume"))
            .unwrap()
    );
    publisher.interface(DeviceInterfaceAction::Arrival, path());
    assert!(matches!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Interface { sequence: 2, .. }
    ));
    assert!(
        !gate
            .try_resume(2, || panic!("a new event cannot revive a stopped monitor"))
            .unwrap()
    );
}

#[test]
fn repeated_stable_resume_preserves_active_revisions_and_explicit_suspension_does_not_revive_them()
{
    let (queue, changes) = EventQueue::exact(2);
    let (stop, _stopped) = mpsc::sync_channel(1);
    let publisher = EventPublisher::new(queue, stop);
    let gate = publisher.exact_gate().unwrap();
    publisher.ready();
    changes.try_recv().unwrap();
    assert!(gate.try_resume(1, || Ok(())).unwrap());
    let active_revision = gate.current_revision().unwrap();
    assert!(
        gate.try_resume(1, || panic!(
            "stable admission must not recommit pool state"
        ))
        .unwrap()
    );
    assert!(gate.is_current(active_revision));
    assert!(gate.suspend_for_reconcile(1));
    assert!(!gate.is_current(active_revision));
    assert!(gate.try_resume(1, || Ok(())).unwrap());
    assert_eq!(gate.current_revision(), Some(active_revision + 1));
    assert!(!gate.is_current(active_revision));
}

#[test]
fn lost_monitor_cannot_resume_after_a_successor_establishes_an_independent_revision() {
    let (old_queue, old_changes) = EventQueue::exact(2);
    let (old_stop, _old_stopped) = mpsc::sync_channel(1);
    let old_publisher = EventPublisher::new(old_queue, old_stop);
    let old_gate = old_publisher.exact_gate().unwrap();
    old_publisher.ready();
    old_changes.try_recv().unwrap();
    assert!(old_gate.try_resume(1, || Ok(())).unwrap());
    let old_revision = old_gate.current_revision().unwrap();
    old_publisher.lost(DeviceChangeLoss::QueueOverflow);
    let (new_queue, new_changes) = EventQueue::exact(2);
    let (new_stop, _new_stopped) = mpsc::sync_channel(1);
    let new_publisher = EventPublisher::new(new_queue, new_stop);
    let new_gate = new_publisher.exact_gate().unwrap();
    new_publisher.ready();
    new_changes.try_recv().unwrap();
    assert!(new_gate.try_resume(1, || Ok(())).unwrap());
    assert!(new_gate.is_current(old_revision));
    assert!(!old_gate.is_current(old_revision));
    assert!(!old_gate.suspend_for_reconcile(1));
    assert!(
        !old_gate
            .try_resume(1, || panic!("lost monitor cannot recommit"))
            .unwrap()
    );
    assert!(new_gate.is_current(old_revision));
}

#[test]
fn admission_revision_exhaustion_ends_observation_and_requests_native_cleanup() {
    let (queue, changes) = EventQueue::exact(2);
    let (stop, stopped) = mpsc::sync_channel(1);
    let publisher = EventPublisher::new(queue, stop);
    let gate = publisher.exact_gate().unwrap();
    publisher.ready();
    changes.try_recv().unwrap();
    // Inject the representable counter boundary rather than requiring an
    // infeasible number of legitimate suspend/resume cycles.
    gate.queue().state().last_revision = u64::MAX;
    assert!(
        gate.try_resume(1, || panic!("exhaustion cannot commit pool state"))
            .unwrap_err()
            .contains("revision exhausted")
    );
    assert_eq!(gate.current_revision(), None);
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Lost {
            after_sequence: 1,
            reason: DeviceChangeLoss::RevisionExhausted,
        }
    );
    assert_eq!(stopped.try_recv(), Ok(()));
    assert!(changes.is_closed());
}

#[test]
fn exact_ready_preserves_registration_callbacks_and_events_during_a_scan() {
    let (events, changes) = EventQueue::exact(8);
    let (retain, retained) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |stopped, events| {
        events.interface(DeviceInterfaceAction::Arrival, path());
        events.ready();
        retain.send(events.clone()).unwrap();
        let _ = stopped.recv();
        Ok(())
    })
    .unwrap();
    let callbacks = retained.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Interface {
            sequence: 1,
            action: DeviceInterfaceAction::Arrival,
            path_utf16: path(),
        }
    );
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Ready { sequence: 2 }
    );

    // The consumer's enumeration is in progress. Both transitions must remain
    // available to replay after its snapshot, even if it contains this path.
    callbacks.interface(DeviceInterfaceAction::Removal, path());
    callbacks.interface(DeviceInterfaceAction::Arrival, path());
    monitor.shutdown().unwrap();

    for (sequence, action) in [
        (3, DeviceInterfaceAction::Removal),
        (4, DeviceInterfaceAction::Arrival),
    ] {
        assert_eq!(
            changes.try_recv().unwrap(),
            ExactDeviceChange::Interface {
                sequence,
                action,
                path_utf16: path(),
            }
        );
    }
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn exact_overflow_keeps_the_prefix_and_terminal_loss_then_unregisters() {
    let (events, changes) = EventQueue::exact(2);
    let released = Arc::new(AtomicBool::new(false));
    let context = released.clone();
    let (retain, retained) = mpsc::channel();
    let (cleaned, cleanup) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |stopped, events| {
        let _context = CallbackContext(context);
        events.ready();
        events.interface(DeviceInterfaceAction::Removal, path());
        events.interface(DeviceInterfaceAction::Arrival, path());
        retain.send(events.clone()).unwrap();
        // Loss itself requests unregistration, without waiting for the owner
        // to consume Lost or close the monitor.
        stopped.recv_timeout(Duration::from_secs(5)).unwrap();
        cleaned.send(()).unwrap();
        Ok(())
    })
    .unwrap();
    let callbacks = retained.recv_timeout(Duration::from_secs(5)).unwrap();
    let automatic_cleanup = cleanup.recv_timeout(Duration::from_secs(5));
    // Join observes the completed cleanup; it cannot restore the lost stream.
    monitor.shutdown().unwrap();

    assert!(
        automatic_cleanup.is_ok(),
        "loss must request native cleanup"
    );
    assert!(released.load(Ordering::Acquire));
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Ready { sequence: 1 }
    );
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Interface {
            sequence: 2,
            action: DeviceInterfaceAction::Removal,
            path_utf16: path(),
        }
    );
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Lost {
            after_sequence: 2,
            reason: DeviceChangeLoss::QueueOverflow,
        }
    );
    callbacks.ready();
    callbacks.interface(DeviceInterfaceAction::Arrival, path());
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn exact_ready_cannot_replace_a_full_registration_event_queue() {
    let (events, changes) = EventQueue::exact(1);
    let monitor = DeviceChangeMonitor::start_worker(events, move |stopped, events| {
        events.interface(DeviceInterfaceAction::Arrival, path());
        wait_for_stop(stopped, &events);
        Ok(())
    })
    .unwrap();
    monitor.shutdown().unwrap();

    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Interface {
            sequence: 1,
            action: DeviceInterfaceAction::Arrival,
            path_utf16: path(),
        }
    );
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Lost {
            after_sequence: 1,
            reason: DeviceChangeLoss::QueueOverflow,
        }
    );
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn exact_cleanup_failure_uses_the_reserved_slot_without_discarding_events() {
    let (events, changes) = EventQueue::exact(2);
    let (retain, retained) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |_, events| {
        events.ready();
        events.interface(DeviceInterfaceAction::Removal, path());
        retain.send(events).unwrap();
        Err("simulated unregister failure; callback context retained".into())
    })
    .unwrap();
    assert_eq!(
        monitor.shutdown().unwrap_err(),
        "simulated unregister failure; callback context retained"
    );
    let callbacks = retained.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Ready { sequence: 1 }
    );
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Interface {
            sequence: 2,
            action: DeviceInterfaceAction::Removal,
            path_utf16: path(),
        }
    );
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Failed("simulated unregister failure; callback context retained".into())
    );
    callbacks.ready();
    changed(&callbacks);
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn exact_loss_and_unregister_failure_are_both_retained() {
    let (events, changes) = EventQueue::exact(1);
    let monitor = DeviceChangeMonitor::start_worker(events, move |_, events| {
        events.ready();
        changed(&events);
        Err("simulated unregister failure after observation loss".into())
    })
    .unwrap();
    assert_eq!(
        monitor.shutdown().unwrap_err(),
        "simulated unregister failure after observation loss"
    );
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Ready { sequence: 1 }
    );
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Lost {
            after_sequence: 1,
            reason: DeviceChangeLoss::QueueOverflow,
        }
    );
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn exact_worker_panic_closes_delivery_even_with_a_retained_callback_context() {
    let (events, changes) = EventQueue::exact(1);
    let (retain, retained) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |_, events| {
        events.ready();
        retain.send(events).unwrap();
        panic!("simulated exact notification worker panic");
    })
    .unwrap();
    assert_eq!(
        monitor.shutdown().unwrap_err(),
        "设备变化监听线程异常结束。"
    );
    let callbacks = retained.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Ready { sequence: 1 }
    );
    callbacks.ready();
    changed(&callbacks);
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn exact_partial_registration_failure_never_publishes_ready() {
    let (events, changes) = EventQueue::exact(1);
    let monitor = DeviceChangeMonitor::start_worker(events, move |_, events| {
        changed(&events);
        Err("simulated second registration failed; first unregistered".into())
    })
    .unwrap();
    assert!(
        monitor
            .shutdown()
            .unwrap_err()
            .contains("registration failed")
    );
    assert!(matches!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Interface { sequence: 1, .. }
    ));
    assert!(matches!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Failed(error) if error.contains("first unregistered")
    ));
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn closing_the_exact_consumer_requests_worker_cleanup() {
    let (events, changes) = EventQueue::exact(1);
    changes.close();
    let released = Arc::new(AtomicBool::new(false));
    let context = released.clone();
    let (cleaned, cleanup) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |stopped, events| {
        let _context = CallbackContext(context);
        wait_for_stop(stopped, &events);
        cleaned.send(()).unwrap();
        Ok(())
    })
    .unwrap();
    let automatic_cleanup = cleanup.recv_timeout(Duration::from_secs(5));
    monitor.shutdown().unwrap();
    assert!(
        automatic_cleanup.is_ok(),
        "closed delivery must stop its worker"
    );
    assert!(released.load(Ordering::Acquire));
    assert!(changes.try_recv().is_err());
}

#[test]
fn exact_sequence_exhaustion_loses_the_stream_without_wrapping() {
    let (queue, changes) = EventQueue::exact(2);
    if let EventQueue::Exact(queue) = &queue {
        // Inject the representable counter boundary without requiring an
        // infeasible number of notifications to exercise checked arithmetic.
        queue.state().last_sequence = u64::MAX - 1;
    }
    let (stop, stopped) = mpsc::sync_channel(1);
    let publisher = EventPublisher::new(queue, stop);
    changed(&publisher);
    publisher.ready();
    assert!(matches!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Interface {
            sequence: u64::MAX,
            ..
        }
    ));
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Lost {
            after_sequence: u64::MAX,
            reason: DeviceChangeLoss::SequenceExhausted,
        }
    );
    assert_eq!(stopped.try_recv(), Ok(()));
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn poisoned_exact_publisher_permanently_loses_observation() {
    let (queue, changes) = EventQueue::exact(1);
    let (stop, stopped) = mpsc::sync_channel(1);
    let publisher = EventPublisher::new(queue, stop);
    let queue = publisher.queue.clone();
    let poisoned = std::thread::spawn(move || {
        if let EventQueue::Exact(queue) = queue.as_ref() {
            let _state = queue.state.lock().unwrap();
            panic!("simulated panic while serializing a notification");
        }
    });
    assert!(poisoned.join().is_err());
    publisher.ready();
    changed(&publisher);
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Lost {
            after_sequence: 0,
            reason: DeviceChangeLoss::PublisherPoisoned,
        }
    );
    assert_eq!(stopped.try_recv(), Ok(()));
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn concurrent_callbacks_have_one_delivery_order_and_contiguous_sequences() {
    let (queue, changes) = EventQueue::exact(17);
    let (stop, _stopped) = mpsc::sync_channel(1);
    let publisher = EventPublisher::new(queue, stop);
    publisher.ready();
    let mut callbacks = Vec::new();
    for action in [
        DeviceInterfaceAction::Removal,
        DeviceInterfaceAction::Arrival,
    ] {
        let publisher = publisher.clone();
        callbacks.push(std::thread::spawn(move || {
            for _ in 0..8 {
                publisher.interface(action, path());
            }
        }));
    }
    for callback in callbacks {
        callback.join().unwrap();
    }
    assert_eq!(
        changes.try_recv().unwrap(),
        ExactDeviceChange::Ready { sequence: 1 }
    );
    let mut removals = 0;
    let mut arrivals = 0;
    for expected in 2..=17 {
        match changes.try_recv().unwrap() {
            ExactDeviceChange::Interface {
                sequence,
                action,
                path_utf16,
            } => {
                assert_eq!(sequence, expected);
                assert_eq!(path_utf16, path());
                match action {
                    DeviceInterfaceAction::Removal => removals += 1,
                    DeviceInterfaceAction::Arrival => arrivals += 1,
                }
            }
            other => panic!("unexpected callback event {other:?}"),
        }
    }
    assert_eq!((removals, arrivals), (8, 8));
    assert!(changes.is_empty());
    assert!(!changes.is_closed());
}

#[cfg(not(windows))]
#[test]
fn exact_observation_explicitly_reports_the_platform_capability_limit() {
    let (monitor, changes) = DeviceChangeMonitor::start_exact().unwrap();
    assert!(matches!(
        changes.recv_blocking().unwrap(),
        ExactDeviceChange::Failed(error) if error.contains("此平台尚未接入设备插拔监听")
    ));
    assert!(monitor.shutdown().is_err());
    assert!(changes.is_closed());
}

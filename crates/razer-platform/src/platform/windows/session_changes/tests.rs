use super::*;
use std::sync::mpsc;
use std::time::Duration;

struct CallbackContext(Arc<AtomicBool>);
impl Drop for CallbackContext {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[test]
fn shutdown_joins_context_release_and_closes_event_delivery() {
    let (events, changes) = async_channel::unbounded();
    let released = Arc::new(AtomicBool::new(false));
    let context = released.clone();
    let owner = Owner::start_worker(events, move |_, events| {
        let _context = CallbackContext(context);
        let _ = events.try_send(SessionChange::Locked);
        Ok(())
    })
    .unwrap();
    owner.shutdown().unwrap();
    assert!(released.load(Ordering::Acquire));
    assert!(matches!(changes.try_recv(), Ok(SessionChange::Locked)));
    assert!(changes.is_closed());
}

#[test]
fn stopping_before_window_publication_keeps_the_startup_gate_closed() {
    let (events, changes) = async_channel::unbounded();
    let (release, may_start) = mpsc::channel();
    let owner = Owner::start_worker(events, move |shared, _| {
        may_start.recv().unwrap();
        assert!(shared.stopped.load(Ordering::Acquire));
        assert_eq!(*shared.hwnd.lock().unwrap(), 0);
        Ok(())
    })
    .unwrap();
    owner.signal_stop().unwrap();
    release.send(()).unwrap();
    owner.shutdown().unwrap();
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn drop_does_not_wait_for_a_worker_holding_the_window_lifecycle_lock() {
    let (events, changes) = async_channel::unbounded();
    let (entered, locked) = mpsc::channel();
    let (release, may_finish) = mpsc::channel();
    let (completed, finished) = mpsc::channel();
    let owner = Owner::start_worker(events, move |shared, _| {
        let owned = shared.hwnd.lock().unwrap();
        entered.send(()).unwrap();
        may_finish.recv().unwrap();
        assert!(shared.stopped.load(Ordering::Acquire));
        drop(owned);
        completed.send(()).unwrap();
        Ok(())
    })
    .unwrap();
    locked.recv_timeout(Duration::from_secs(5)).unwrap();
    let (released, dropped) = mpsc::channel();
    let dropping = std::thread::spawn(move || {
        drop(owner);
        released.send(()).unwrap();
    });
    let result = dropped.recv_timeout(Duration::from_secs(5));
    // Release the simulated native operation even when a blocking Drop regresses.
    release.send(()).unwrap();
    dropping.join().unwrap();
    finished.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        result.is_ok(),
        "Drop must not wait for native window teardown"
    );
    changes.recv_blocking().unwrap_err();
}

#[test]
fn shutdown_preserves_worker_failure_after_joining_it() {
    let (events, changes) = async_channel::unbounded();
    let owner =
        Owner::start_worker(events, |_, _| Err("simulated unregister failure".into())).unwrap();
    assert_eq!(
        owner.shutdown().unwrap_err(),
        "simulated unregister failure"
    );
    assert!(matches!(
        changes.try_recv(),
        Ok(SessionChange::Failed(error)) if error == "simulated unregister failure"
    ));
    assert!(changes.is_closed());
}

#[test]
fn shutdown_reports_a_panicked_worker_and_releases_its_sender() {
    let (events, changes) = async_channel::unbounded();
    let owner =
        Owner::start_worker(events, |_, _| panic!("simulated session worker panic")).unwrap();
    assert_eq!(
        owner.shutdown().unwrap_err(),
        "Session event worker panicked"
    );
    assert!(changes.is_closed());
    assert!(changes.try_recv().is_err());
}

#[test]
fn failed_wakeup_still_joins_after_a_bounded_stop_recheck_and_retains_errors() {
    for cleanup_error in [None, Some("simulated unregister failure")] {
        let (events, changes) = async_channel::unbounded();
        let released = Arc::new(AtomicBool::new(false));
        let context = released.clone();
        let (waiting, wait_started) = mpsc::channel();
        let (elapsed, deadline_passed) = mpsc::channel();
        let owner = Owner::start_worker(events, move |shared, _| {
            let _context = CallbackContext(context);
            let mut waits = 0;
            pump_until_stopped(&shared.stopped, |milliseconds| {
                waiting.send(milliseconds).unwrap();
                waits += 1;
                if waits > 1 {
                    return Err("Stop recheck entered another wait".into());
                }
                deadline_passed.recv().unwrap();
                Ok(MessageCycle::Continue)
            })?;
            cleanup_error.map_or(Ok(()), |error| Err(error.into()))
        })
        .unwrap();
        let wait = wait_started.recv_timeout(Duration::from_secs(5)).unwrap();
        let (failed, wake_attempted) = mpsc::channel();
        let closing = std::thread::spawn(move || {
            owner.shutdown_with(|_| {
                failed.send(()).unwrap();
                Err("simulated PostMessageW failure".into())
            })
        });
        let wake = wake_attempted.recv_timeout(Duration::from_secs(5));
        // Advance the simulated native deadline after the unsuccessful wake.
        // No real window, message API, timer or sleep is needed.
        elapsed.send(()).unwrap();
        let result = closing.join().unwrap();
        assert!(wake.is_ok());
        assert!((1..=100).contains(&wait), "stop recheck must be bounded");
        assert!(
            wait_started.try_recv().is_err(),
            "stop must prevent another wait"
        );
        assert!(released.load(Ordering::Acquire));
        let error = result.unwrap_err();
        assert!(error.contains("simulated PostMessageW failure"));
        if let Some(cleanup) = cleanup_error {
            assert!(error.contains(cleanup));
            assert!(matches!(
                changes.try_recv(),
                Ok(SessionChange::Failed(error)) if error == cleanup
            ));
        }
        assert!(changes.is_closed());
        assert!(changes.try_recv().is_err());
    }
}

#[test]
fn message_wait_failure_returns_to_the_owner_and_quit_does_not_wait_again() {
    let running = AtomicBool::new(false);
    let mut failed_waits = 0;
    assert_eq!(
        pump_until_stopped(&running, |_| {
            failed_waits += 1;
            assert_eq!(failed_waits, 1);
            Err("simulated native wait failure".into())
        })
        .unwrap_err(),
        "simulated native wait failure"
    );
    let mut waits = 0;
    pump_until_stopped(&running, |_| {
        waits += 1;
        assert_eq!(waits, 1);
        Ok(MessageCycle::Quit)
    })
    .unwrap();
    assert_eq!(waits, 1);
}

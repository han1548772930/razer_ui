use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

struct CallbackContext(Arc<AtomicBool>);

impl Drop for CallbackContext {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[test]
fn shutdown_joins_native_cleanup_and_closes_event_delivery() {
    let (events, changes) = async_channel::bounded(1);
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
    let (events, changes) = async_channel::bounded(1);
    let (enter, entered) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |stopped, events| {
        assert!(events.try_send(DeviceChange::Changed).is_ok());
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
    let (events, changes) = async_channel::bounded(1);
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
fn shutdown_preserves_cleanup_failure_and_closes_retained_callback_delivery() {
    let (events, changes) = async_channel::bounded(1);
    let (retain, retained) = mpsc::channel();
    let monitor = DeviceChangeMonitor::start_worker(events, move |_, events| {
        assert!(events.try_send(DeviceChange::Changed).is_ok());
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
    assert!(matches!(
        callback_events.try_send(DeviceChange::Changed),
        Err(async_channel::TrySendError::Closed(_))
    ));
}

#[test]
fn a_panicked_worker_is_joined_and_cannot_keep_its_event_channel_open() {
    let (events, changes) = async_channel::bounded(1);
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
    assert!(matches!(
        callback_events.try_send(DeviceChange::Changed),
        Err(async_channel::TrySendError::Closed(_))
    ));
}

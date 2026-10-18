use super::*;
use crate::mapping::test_transport::{Mock, TIMEOUT};
use razer_ipc::{
    KEY_CAPTURE_LIFECYCLE,
    mapping_control::{Control, Owner},
};
use serde_json::json;

fn receipt() -> serde_json::Value {
    json!({"capturing":true,"mappings_suspended":true,"mode_events_registered":true})
}

fn next_control(owner: &Owner) -> Control {
    let deadline = std::time::Instant::now() + TIMEOUT;
    loop {
        let commands = owner.drain();
        if !commands.is_empty() {
            assert_eq!(commands.len(), 1);
            return commands.into_iter().next().unwrap();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "mapping control timed out"
        );
        thread::yield_now();
    }
}

#[test]
fn capture_waits_for_all_mapping_owners_and_restores_after_transport_release() {
    let registry = Registry::default();
    let mut owner = registry.register(uuid::Uuid::new_v4().to_string()).unwrap();
    let paused = Arc::new(AtomicBool::new(false));
    let pause_observation = paused.clone();
    let dropped = Arc::new(AtomicBool::new(false));
    let drop_observation = dropped.clone();
    let mut event_sent = false;
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::StartShortcutCapture => {
            assert!(pause_observation.load(Ordering::Acquire));
            Ok(receipt())
        }
        ServiceRequest::ShortcutCaptureEvents => {
            let events = if event_sent {
                vec![]
            } else {
                vec![json!({"event":"{\"isEnabled\":true}"})]
            };
            event_sent = true;
            Ok(json!({"events":events}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let mock = mock.on_drop(move || {
        assert!(KEY_CAPTURE_LIFECYCLE.try_lock().is_err());
        drop_observation.store(true, Ordering::Release);
    });
    let session = Session::start_with(|| Ok(mock), registry).unwrap();
    let pause = next_control(&owner);
    assert!(!pause.target().mapping_enabled());
    assert_eq!(pause.target().global_shortcuts_enabled(), None);
    assert!(session.replies.try_recv().is_err());
    paused.store(true, Ordering::Release);
    pause.acknowledge(Ok(()));
    assert!(matches!(
        session.replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready
    ));
    assert!(matches!(
        session.replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Mode(true)
    ));
    session.request_stop();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let restore = next_control(&owner);
    assert!(dropped.load(Ordering::Acquire));
    assert!(restore.target().mapping_enabled());
    assert_eq!(restore.target().global_shortcuts_enabled(), None);
    assert!(!session.is_finished());
    restore.acknowledge(Ok(()));
    session.stop().unwrap();
    owner.finish(Ok(())).unwrap();
    let trace = trace.lock().unwrap();
    assert!(matches!(
        trace.first(),
        Some(ServiceRequest::StartShortcutCapture)
    ));
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn incomplete_start_receipt_never_announces_ready_and_still_shuts_down() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartShortcutCapture => Ok(json!({"capturing":true})),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session = Session::start_with(|| Ok(mock), Registry::default()).unwrap();
    assert!(
        matches!(session.replies.recv_timeout(TIMEOUT).unwrap(), Reply::Failed(error) if error.contains("mappings_suspended"))
    );
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(session.stop().unwrap_err().contains("mappings_suspended"));
    assert!(matches!(
        trace.lock().unwrap().as_slice(),
        [
            ServiceRequest::StartShortcutCapture,
            ServiceRequest::Shutdown
        ]
    ));
}

#[test]
fn malformed_mode_payload_and_rejected_shutdown_are_both_retained() {
    let (mock, _, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartShortcutCapture => Ok(receipt()),
        ServiceRequest::ShortcutCaptureEvents => {
            Ok(json!({"events":[{"event":"{\"isEnabled\":null}"}]}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":false})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session = Session::start_with(|| Ok(mock), Registry::default()).unwrap();
    assert!(matches!(
        session.replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready
    ));
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let error = session.stop().unwrap_err();
    assert!(error.contains("expected a boolean"));
    assert!(error.contains("cleanup was not acknowledged"));
}

#[test]
fn rust_callback_poll_unwind_keeps_shutdown_inside_the_lifecycle_owner() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartShortcutCapture => Ok(receipt()),
        ServiceRequest::ShortcutCaptureEvents => panic!("simulated mode poll panic"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session = Session::start_with(|| Ok(mock), Registry::default()).unwrap();
    assert!(matches!(
        session.replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready
    ));
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(
        session
            .stop()
            .unwrap_err()
            .contains("simulated mode poll panic")
    );
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
}

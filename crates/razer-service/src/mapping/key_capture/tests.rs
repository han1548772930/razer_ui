use super::*;
use crate::mapping::test_transport::{Mock, TIMEOUT};
use serde_json::json;

fn receipt() -> Value {
    json!({"capturing":true,"redirected_devices":0,"devices":[],"unresolved_devices":[]})
}

#[test]
fn capture_receipt_requires_observed_registration_fields() {
    for value in [
        json!({"capturing":true}),
        json!({"capturing":false,"redirected_devices":0,"devices":[],"unresolved_devices":[]}),
        json!({"capturing":true,"redirected_devices":1,"devices":[],"unresolved_devices":[]}),
        json!({"capturing":true,"redirected_devices":0,"devices":[],"unresolved_devices":null}),
    ] {
        assert!(CaptureReady::parse(value).is_err());
    }
    let value = json!({"capturing":true,"redirected_devices":0,"devices":[],"unresolved_devices":[{"path":"unresolved"}]});
    assert_eq!(
        CaptureReady::parse(value).unwrap().unresolved_devices(),
        [json!({"path":"unresolved"})]
    );
}

#[test]
fn global_shortcut_capture_retains_input_and_mode_callbacks_in_the_same_worker() {
    let mut polled = false;
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::StartGlobalShortcutCapture => {
            let mut ready = receipt();
            ready["mode_events_registered"] = true.into();
            Ok(ready)
        }
        ServiceRequest::GlobalShortcutCaptureEvents => {
            if !polled {
                polled = true;
                Ok(json!({"events":[
                    {"kind":"mode","payload":true},
                    {"kind":"input","payload":{"eventType":3,"input":{"type":"key","key":65},"timeTick":2}},
                    {"kind":"mode","payload":false},
                    {"kind":"input","payload":{"eventType":3,"input":{"type":"key","key":66},"timeTick":1}},
                ]}))
            } else {
                Ok(json!({"events":[]}))
            }
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session =
        Session::start_with_registry(|| Ok(mock), Registry::default(), Kind::GlobalShortcut)
            .unwrap();
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready(_)
    ));
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Mode(true)
    ));
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Events(events) if events[0]["input"]["key"] == 65
    ));
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Mode(false)
    ));
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Events(events) if events[0]["input"]["key"] == 66
    ));
    drop(session);
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    assert!(matches!(
        trace.first(),
        Some(ServiceRequest::StartGlobalShortcutCapture)
    ));
    assert!(!trace.iter().any(|request| matches!(
        request,
        ServiceRequest::StartMappingKeyCapture
            | ServiceRequest::StartShortcutCapture
            | ServiceRequest::MappingKeyCaptureEvents
            | ServiceRequest::ShortcutCaptureEvents
    )));
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn global_capture_requires_its_mode_registration_and_preserves_failures() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartGlobalShortcutCapture => Ok(receipt()),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session =
        Session::start_with_registry(|| Ok(mock), Registry::default(), Kind::GlobalShortcut)
            .unwrap();
    assert!(
        matches!(session.events.recv_timeout(TIMEOUT).unwrap(), Reply::Failed(error) if error.contains("mode callback"))
    );
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
}

#[test]
fn malformed_combined_capture_stops_with_its_cleanup() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartGlobalShortcutCapture => {
            let mut ready = receipt();
            ready["mode_events_registered"] = true.into();
            Ok(ready)
        }
        ServiceRequest::GlobalShortcutCaptureEvents => {
            Ok(json!({"events":[{"kind":"mode","payload":"false"}]}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session =
        Session::start_with_registry(|| Ok(mock), Registry::default(), Kind::GlobalShortcut)
            .unwrap();
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready(_)
    ));
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Failed(_)
    ));
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
    drop(session);
}

#[test]
fn focused_capture_forwards_events_and_restores_on_drop() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMappingKeyCapture => Ok(receipt()),
        ServiceRequest::MappingKeyCaptureEvents => {
            Ok(json!({"events":[{"eventType":3,"input":{"key":65}}]}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session = Session::start_with(|| Ok(mock)).unwrap();
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready(_)
    ));
    assert!(
        matches!(session.events.recv_timeout(TIMEOUT).unwrap(), Reply::Events(events) if events == [json!({"eventType":3,"input":{"key":65}})])
    );
    drop(session);
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    assert!(matches!(
        trace.first(),
        Some(ServiceRequest::StartMappingKeyCapture)
    ));
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn startup_failure_still_cleans_partial_capture() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMappingKeyCapture => Ok(json!({"capturing":true})),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session = Session::start_with(|| Ok(mock)).unwrap();
    assert!(
        matches!(session.events.recv_timeout(TIMEOUT).unwrap(), Reply::Failed(error) if error.contains("redirected_devices"))
    );
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        trace.lock().unwrap().as_slice(),
        [
            ServiceRequest::StartMappingKeyCapture,
            ServiceRequest::Shutdown
        ]
    ));
    drop(session);
}

#[test]
fn malformed_events_and_unacknowledged_cleanup_remain_failures() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMappingKeyCapture => Ok(receipt()),
        ServiceRequest::MappingKeyCaptureEvents => Ok(json!({"events":null})),
        ServiceRequest::Shutdown => Ok(json!({"stopped":false})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session = Session::start_with(|| Ok(mock)).unwrap();
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready(_)
    ));
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Failed(_)
    ));
    assert!(
        matches!(session.events.recv_timeout(TIMEOUT).unwrap(), Reply::Failed(error) if error.contains("cleanup was not acknowledged"))
    );
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
    drop(session);
}

#[test]
fn replacement_capture_waits_for_previous_transport_release() {
    let first_dropped = Arc::new(AtomicBool::new(false));
    let drop_observation = first_dropped.clone();
    let (entered, shutting_down) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (mock, _, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::StartMappingKeyCapture => Ok(receipt()),
        ServiceRequest::MappingKeyCaptureEvents => anyhow::bail!("end first capture"),
        ServiceRequest::Shutdown => {
            entered.send(()).unwrap();
            gate.recv_timeout(TIMEOUT).unwrap();
            Ok(json!({"stopped":true}))
        }
        request => panic!("unexpected request: {request:?}"),
    });
    let mock = mock.on_drop(move || {
        assert!(
            LIFECYCLE.try_lock().is_err(),
            "capture lock released before transport"
        );
        drop_observation.store(true, Ordering::Release);
    });
    let first = Session::start_with(|| Ok(mock)).unwrap();
    shutting_down.recv_timeout(TIMEOUT).unwrap();
    assert!(
        LIFECYCLE.try_lock().is_err(),
        "capture lock released during cleanup"
    );
    let (mock, _, second_cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMappingKeyCapture => Ok(receipt()),
        ServiceRequest::MappingKeyCaptureEvents => anyhow::bail!("end second capture"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let second = Session::start_with(move || {
        assert!(
            first_dropped.load(Ordering::Acquire),
            "replacement started before cleanup"
        );
        Ok(mock)
    })
    .unwrap();
    release.send(()).unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        second.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready(_)
    ));
    second_cleanup.recv_timeout(TIMEOUT).unwrap();
    drop(first);
    drop(second);
}

#[test]
fn rust_capture_unwind_still_restores_native_suppression() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMappingKeyCapture => Ok(receipt()),
        ServiceRequest::MappingKeyCaptureEvents => panic!("simulated Rust capture panic"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session = Session::start_with(|| Ok(mock)).unwrap();
    assert!(matches!(
        session.events.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready(_)
    ));
    assert!(
        matches!(session.events.recv_timeout(TIMEOUT).unwrap(), Reply::Failed(error) if error.contains("simulated Rust capture panic"))
    );
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
    drop(session);
}

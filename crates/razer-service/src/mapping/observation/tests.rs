use super::*;
use crate::mapping::test_transport::{Mock, TIMEOUT};
use serde_json::json;

#[test]
fn observation_completion_distinguishes_poll_failure_from_unconfirmed_cleanup() {
    for fail_cleanup in [false, true] {
        let (mock, _, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::MappingObservationStart { observer_guid, .. } => {
                Ok(json!({"observer_guid":observer_guid,"registered":true}))
            }
            ServiceRequest::MappingObservationPoll { .. } => anyhow::bail!("poll failed"),
            ServiceRequest::MappingObservationStop { observer_guid } => {
                Ok(json!({"observer_guid":observer_guid,"registered":fail_cleanup}))
            }
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let session =
            Session::start_with(Target::new(679, "observed-container".into()), || Ok(mock))
                .unwrap();
        cleanup.recv_timeout(TIMEOUT).unwrap();
        let completion = session.stop_with_receipt();
        assert_eq!(completion.operation_error(), Some("poll failed"));
        assert!(completion.result().unwrap_err().contains("poll failed"));
        if fail_cleanup {
            assert!(
                completion
                    .cleanup_error()
                    .unwrap()
                    .contains("cleanup was not acknowledged")
            );
            assert!(completion.cleanup_result().is_err());
        } else {
            assert_eq!(completion.cleanup_error(), None);
            assert_eq!(completion.cleanup_result(), Ok(()));
        }
    }
}

#[test]
fn snaptap_subscription_delivers_typed_events_and_unregisters_before_scope_release() {
    let (entered, waiting) = mpsc::channel();
    let (resume, blocked) = mpsc::channel();
    let mut polled = false;
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingObservationStart {
            target,
            observer_guid,
        } => {
            assert_eq!(target.product_id(), 679);
            assert_eq!(target.container_id(), "observed-container");
            Ok(json!({"observer_guid":observer_guid,"registered":true}))
        }
        ServiceRequest::MappingObservationPoll { observer_guid } if !polled => {
            polled = true;
            Ok(
                json!({"observer_guid":observer_guid,"events":[{"input":{"type":"analogKey","scancode":30,"flag":1,"modifiers":0},"snaptap":true,"timeTick":55}]}),
            )
        }
        ServiceRequest::MappingObservationPoll { observer_guid } => {
            entered.send(()).unwrap();
            blocked.recv_timeout(TIMEOUT).unwrap();
            Ok(json!({"observer_guid":observer_guid,"events":[]}))
        }
        ServiceRequest::MappingObservationStop { observer_guid } => {
            Ok(json!({"observer_guid":observer_guid,"registered":false}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session =
        Session::start_with(Target::new(679, "observed-container".into()), || Ok(mock)).unwrap();
    assert!(matches!(
        session.replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready
    ));
    let Reply::Event(event) = session.replies.recv_timeout(TIMEOUT).unwrap() else {
        panic!("missing observation")
    };
    assert!(event.replayed());
    assert_eq!(event.time_tick(), 55);
    waiting.recv_timeout(TIMEOUT).unwrap();
    session.request_stop();
    resume.send(()).unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    session.stop().unwrap();
    let trace = trace.lock().unwrap();
    assert!(matches!(
        &trace[trace.len() - 2],
        ServiceRequest::MappingObservationStop { .. }
    ));
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn snaptap_observer_rejects_foreign_batch_and_retains_unsubscribe_failure() {
    let (mock, _, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingObservationStart { observer_guid, .. } => {
            Ok(json!({"observer_guid":observer_guid,"registered":true}))
        }
        ServiceRequest::MappingObservationPoll { .. } => {
            Ok(json!({"observer_guid":"other-generation","events":[]}))
        }
        ServiceRequest::MappingObservationStop { .. } => anyhow::bail!("unsubscribe failed"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session =
        Session::start_with(Target::new(679, "observed-container".into()), || Ok(mock)).unwrap();
    assert!(matches!(
        session.replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready
    ));
    let Reply::Failed(error) = session.replies.recv_timeout(TIMEOUT).unwrap() else {
        panic!("failure missing")
    };
    assert!(error.contains("identity changed") && error.contains("unsubscribe failed"));
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(session.stop().unwrap_err(), error);
}

#[test]
fn snaptap_cancel_during_poll_does_not_deliver_late_observations() {
    let (entered, waiting) = mpsc::channel();
    let (resume, blocked) = mpsc::channel();
    let (mock, _, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingObservationStart { observer_guid, .. } => {
            Ok(json!({"observer_guid":observer_guid,"registered":true}))
        }
        ServiceRequest::MappingObservationPoll { observer_guid } => {
            entered.send(()).unwrap();
            blocked.recv_timeout(TIMEOUT).unwrap();
            Ok(
                json!({"observer_guid":observer_guid,"events":[{"input":{"type":"analogKey","scancode":30,"flag":0},"snaptap":false,"timeTick":9}]}),
            )
        }
        ServiceRequest::MappingObservationStop { observer_guid } => {
            Ok(json!({"observer_guid":observer_guid,"registered":false}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let session =
        Session::start_with(Target::new(679, "observed-container".into()), || Ok(mock)).unwrap();
    assert!(matches!(
        session.replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready
    ));
    waiting.recv_timeout(TIMEOUT).unwrap();
    session.request_stop();
    resume.send(()).unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(session.drain().is_empty());
    session.stop().unwrap();
}

#[test]
fn snaptap_failed_start_response_still_unsubscribes_and_releases_scope() {
    for malformed in [true, false] {
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::MappingObservationStart { .. } if malformed => {
                Ok(json!({"registered":true}))
            }
            ServiceRequest::MappingObservationStart { .. } => anyhow::bail!("start response lost"),
            ServiceRequest::MappingObservationStop { .. } => {
                anyhow::bail!("unsubscribe failure retained")
            }
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let session =
            Session::start_with(Target::new(679, "observed-container".into()), || Ok(mock))
                .unwrap();
        let Reply::Failed(error) = session.replies.recv_timeout(TIMEOUT).unwrap() else {
            panic!("failed start reported ready")
        };
        assert!(error.contains(if malformed {
            "observer_guid"
        } else {
            "start response lost"
        }));
        assert!(error.contains("unsubscribe failure retained"));
        cleanup.recv_timeout(TIMEOUT).unwrap();
        assert_eq!(session.stop().unwrap_err(), error);
        let trace = trace.lock().unwrap();
        assert!(matches!(
            trace.as_slice(),
            [
                ServiceRequest::MappingObservationStart { .. },
                ServiceRequest::MappingObservationStop { .. },
                ServiceRequest::Shutdown
            ]
        ));
    }
}

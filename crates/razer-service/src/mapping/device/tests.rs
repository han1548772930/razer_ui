use super::*;
use crate::mapping::test_transport::{Mock, TIMEOUT};
use serde_json::json;

fn ready(replies: &mpsc::Receiver<Reply>) {
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready
    ));
}

fn stopped(replies: &mpsc::Receiver<Reply>) -> (Vec<String>, Result<(), String>) {
    let mut failures = Vec::new();
    loop {
        match replies.recv_timeout(TIMEOUT).unwrap() {
            Reply::Failed(error) => failures.push(error),
            Reply::Stopped(result) => return (failures, result),
            Reply::Published { result, .. } => {
                if let Err(error) = result {
                    failures.push(error);
                }
            }
            Reply::Ready | Reply::Events(_) => {}
        }
    }
}

#[test]
fn registration_requires_matching_owner_and_complete_ready_receipt() {
    for value in [
        json!({"registered":true,"owner_guid":"owner"}),
        json!({"registered":true,"driver_ready":null,"owner_guid":"owner"}),
        json!({"registered":false,"driver_ready":true,"owner_guid":"owner"}),
        json!({"registered":true,"driver_ready":true,"owner_guid":"other"}),
    ] {
        assert!(registration(value, "owner").is_err());
    }
    assert!(
        registration(
            json!({"registered":true,"driver_ready":true,"owner_guid":"owner","extension":1}),
            "owner"
        )
        .is_ok()
    );
}

#[test]
fn publication_and_events_reject_missing_fields_and_stale_owner() {
    for value in [
        json!({"published":true}),
        json!({"published":"true","owner_guid":"owner"}),
        json!({"published":false,"owner_guid":"owner"}),
        json!({"published":true,"owner_guid":"other"}),
    ] {
        assert!(publication(value, "owner").is_err());
    }
    for value in [
        json!({"owner_guid":"owner"}),
        json!({"events":null,"owner_guid":"owner"}),
        json!({"events":[],"owner_guid":"other"}),
    ] {
        assert!(event_batch(value, "owner").is_err());
    }
}

#[test]
fn failed_registration_still_shuts_down_partial_native_ownership() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::MappingDeviceStart { .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":"stale"}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, replies) = Session::start_with(70, "container".into(), || Ok(mock)).unwrap();
    let (failures, result) = stopped(&replies);
    assert_eq!(failures.len(), 1);
    assert!(failures[0].contains("registration receipt"));
    assert!(result.is_ok());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert_eq!(trace.lock().unwrap().len(), 2);
    assert!(matches!(trace.lock().unwrap()[1], ServiceRequest::Shutdown));
    drop(session);
}

#[test]
fn temporary_mapping_is_restored_before_shutdown_and_transport_release() {
    let mut owner = None;
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            owner = Some(owner_guid.clone());
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDevicePublish { owner_guid, .. } => {
            assert_eq!(Some(&owner_guid), owner.as_ref());
            Ok(json!({"published":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDeviceEvents { owner_guid } => {
            assert_eq!(Some(&owner_guid), owner.as_ref());
            Ok(json!({"events":[{"eventName":"native-mapping"}],"owner_guid":owner_guid}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, replies) = Session::start_with(70, "container".into(), || Ok(mock)).unwrap();
    ready(&replies);
    session
        .publish_with_restore(
            17,
            json!({"temporary":true}),
            None,
            Some(json!({"ordinary":true})),
        )
        .unwrap();
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Published {
            revision: 17,
            result: Ok(()),
            ..
        }
    ));
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Events(events) if events == vec![json!({"eventName":"native-mapping"})]
    ));
    drop(session);
    let (failures, result) = stopped(&replies);
    assert!(failures.is_empty());
    assert!(result.is_ok());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    let documents = trace
        .iter()
        .filter_map(|request| match request {
            ServiceRequest::MappingDevicePublish { document, .. } => Some(document.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        documents,
        [json!({"temporary":true}), json!({"ordinary":true})]
    );
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn failed_temporary_publication_preserves_restore_and_cleanup_failure() {
    let mut publish_count = 0;
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDevicePublish { document, .. } => {
            publish_count += 1;
            match publish_count {
                1 => {
                    assert_eq!(document, json!({"temporary":true}));
                    anyhow::bail!("native write partially failed")
                }
                2 => {
                    assert_eq!(document, json!({"ordinary":true}));
                    anyhow::bail!("restore rejected")
                }
                _ => panic!("unexpected publication"),
            }
        }
        ServiceRequest::MappingDeviceEvents { .. } => anyhow::bail!("event connection lost"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, replies) = Session::start_with(70, "container".into(), || Ok(mock)).unwrap();
    ready(&replies);
    session
        .publish_with_restore(
            18,
            json!({"temporary":true}),
            None,
            Some(json!({"ordinary":true})),
        )
        .unwrap();
    let (failures, result) = stopped(&replies);
    assert!(
        failures
            .iter()
            .any(|error| error.contains("native write partially failed"))
    );
    assert!(
        failures
            .iter()
            .any(|error| error.contains("Temporary mapping cleanup: restore rejected"))
    );
    assert!(
        result
            .unwrap_err()
            .contains("Temporary mapping cleanup: restore rejected")
    );
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
    drop(session);
}

#[test]
fn successful_ordinary_publication_replaces_temporary_restore() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDevicePublish { owner_guid, .. } => {
            Ok(json!({"published":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDeviceEvents { owner_guid } => {
            Ok(json!({"events":[],"owner_guid":owner_guid}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, replies) = Session::start_with(70, "container".into(), || Ok(mock)).unwrap();
    ready(&replies);
    session
        .publish_with_restore(1, json!("temporary"), None, Some(json!("original")))
        .unwrap();
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Published { result: Ok(()), .. }
    ));
    session.publish(2, json!("ordinary-edited"), None).unwrap();
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Published {
            revision: 2,
            result: Ok(()),
            ..
        }
    ));
    drop(session);
    assert!(stopped(&replies).1.is_ok());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let documents = trace
        .lock()
        .unwrap()
        .iter()
        .filter_map(|request| match request {
            ServiceRequest::MappingDevicePublish { document, .. } => Some(document.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(documents, [json!("temporary"), json!("ordinary-edited")]);
}

#[test]
fn canceled_start_does_not_register_and_still_releases_transport() {
    let (entered, started) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, replies) = Session::start_with(70, "container".into(), move || {
        entered.send(()).unwrap();
        gate.recv_timeout(TIMEOUT).unwrap();
        Ok(mock)
    })
    .unwrap();
    started.recv_timeout(TIMEOUT).unwrap();
    drop(session);
    release.send(()).unwrap();
    let (failures, result) = stopped(&replies);
    assert_eq!(failures, ["Mapping start canceled"]);
    assert!(result.is_ok());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        trace.lock().unwrap().as_slice(),
        [ServiceRequest::Shutdown]
    ));
}

#[test]
fn queued_publications_keep_latest_revision() {
    let (release, gate) = mpsc::channel();
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDevicePublish { owner_guid, .. } => {
            Ok(json!({"published":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDeviceEvents { owner_guid } => {
            Ok(json!({"events":[],"owner_guid":owner_guid}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, replies) = Session::start_with(70, "container".into(), move || {
        gate.recv_timeout(TIMEOUT).unwrap();
        Ok(mock)
    })
    .unwrap();
    session.publish(1, json!("first"), None).unwrap();
    session.publish(2, json!("second"), None).unwrap();
    release.send(()).unwrap();
    ready(&replies);
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Published {
            revision: 2,
            result: Ok(()),
            ..
        }
    ));
    drop(session);
    assert!(stopped(&replies).1.is_ok());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let documents = trace
        .lock()
        .unwrap()
        .iter()
        .filter_map(|request| match request {
            ServiceRequest::MappingDevicePublish { document, .. } => Some(document.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(documents, [json!("second")]);
}

#[test]
fn graceful_stop_acknowledges_the_latest_document_before_native_removal_and_release() {
    let (start, start_gate) = mpsc::channel();
    let (publishing, publication_entered) = mpsc::channel();
    let (acknowledge, publication_ack) = mpsc::channel();
    let (removing, removal_entered) = mpsc::channel();
    let (finish, shutdown_gate) = mpsc::channel();
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDevicePublish {
            owner_guid,
            document,
        } => {
            assert_eq!(document, json!({"profile":"last-admitted"}));
            publishing.send(()).unwrap();
            publication_ack.recv_timeout(TIMEOUT).unwrap();
            Ok(json!({"published":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingKeyboardGamingIndicator { enabled, .. } => {
            assert!(enabled);
            Ok(json!({"written":true}))
        }
        ServiceRequest::Shutdown => {
            removing.send(()).unwrap();
            shutdown_gate.recv_timeout(TIMEOUT).unwrap();
            Ok(json!({"stopped":true}))
        }
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, replies) = Session::start_with(70, "graceful-stop".into(), move || {
        start_gate.recv_timeout(TIMEOUT).unwrap();
        Ok(mock)
    })
    .unwrap();
    session.publish(1, json!("superseded"), None).unwrap();
    session
        .publish(2, json!({"profile":"last-admitted"}), Some(true))
        .unwrap();
    session.request_stop_after_pending().unwrap();
    session.request_stop_after_pending().unwrap();
    assert!(session.publish(3, json!("late"), None).is_err());
    start.send(()).unwrap();
    ready(&replies);
    publication_entered.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(replies.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(matches!(
        removal_entered.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    acknowledge.send(()).unwrap();
    removal_entered.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        replies.try_recv().unwrap(),
        Reply::Published {
            revision: 2,
            result: Ok(()),
            indicator: Some(Ok(_)),
        }
    ));
    assert!(matches!(replies.try_recv(), Err(mpsc::TryRecvError::Empty)));
    finish.send(()).unwrap();
    assert!(stopped(&replies).1.is_ok());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    assert!(matches!(
        trace.as_slice(),
        [
            ServiceRequest::MappingDeviceStart { .. },
            ServiceRequest::MappingDevicePublish { .. },
            ServiceRequest::MappingKeyboardGamingIndicator { .. },
            ServiceRequest::Shutdown,
        ]
    ));
    drop(session);
}

#[test]
fn dropping_a_draining_owner_cancels_queued_documents_after_the_inflight_callback() {
    let (publishing, publication_entered) = mpsc::channel();
    let (acknowledge, publication_ack) = mpsc::channel();
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDevicePublish {
            owner_guid,
            document,
        } => {
            assert_eq!(document, json!("inflight"));
            publishing.send(()).unwrap();
            publication_ack.recv_timeout(TIMEOUT).unwrap();
            Ok(json!({"published":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, replies) =
        Session::start_with(70, "cancel-draining-owner".into(), || Ok(mock)).unwrap();
    ready(&replies);
    session.publish(1, json!("inflight"), None).unwrap();
    publication_entered.recv_timeout(TIMEOUT).unwrap();
    session.publish(2, json!("queued"), None).unwrap();
    session.request_stop_after_pending().unwrap();
    drop(session);
    acknowledge.send(()).unwrap();
    let (failures, result) = stopped(&replies);
    assert!(failures.is_empty());
    result.unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    assert_eq!(
        trace
            .iter()
            .filter(|request| matches!(request, ServiceRequest::MappingDevicePublish { .. }))
            .count(),
        1
    );
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn missing_shutdown_acknowledgement_is_not_success() {
    let (mock, _, cleanup) = Mock::new(|request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDeviceEvents { owner_guid } => {
            Ok(json!({"events":[],"owner_guid":owner_guid}))
        }
        ServiceRequest::Shutdown => Ok(json!({})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, replies) = Session::start_with(70, "container".into(), || Ok(mock)).unwrap();
    ready(&replies);
    drop(session);
    assert!(stopped(&replies).1.unwrap_err().contains("stopped"));
    cleanup.recv_timeout(TIMEOUT).unwrap();
}

#[test]
fn rust_worker_unwind_restores_temporary_document_and_reports_terminal_result() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDevicePublish {
            owner_guid,
            document,
        } => {
            assert_ne!(document, json!("unexpected"));
            Ok(json!({"published":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDeviceEvents { .. } => panic!("simulated Rust event decode panic"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (release, gate) = mpsc::channel();
    let (session, replies) = Session::start_with(70, "container".into(), move || {
        gate.recv_timeout(TIMEOUT).unwrap();
        Ok(mock)
    })
    .unwrap();
    session
        .publish_with_restore(3, json!("temporary"), None, Some(json!("ordinary")))
        .unwrap();
    release.send(()).unwrap();
    let (failures, result) = stopped(&replies);
    assert!(
        failures
            .iter()
            .any(|error| error.contains("simulated Rust event decode panic"))
    );
    assert!(result.is_ok());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    let documents = trace
        .iter()
        .filter_map(|request| match request {
            ServiceRequest::MappingDevicePublish { document, .. } => Some(document.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(documents, [json!("temporary"), json!("ordinary")]);
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
    drop(session);
}

#[test]
fn rust_restore_unwind_does_not_skip_shutdown() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDevicePublish {
            owner_guid,
            document,
        } => {
            if document == json!("ordinary") {
                panic!("simulated Rust restore panic");
            }
            Ok(json!({"published":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDeviceEvents { .. } => anyhow::bail!("end session"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (release, gate) = mpsc::channel();
    let (session, replies) = Session::start_with(70, "container".into(), move || {
        gate.recv_timeout(TIMEOUT).unwrap();
        Ok(mock)
    })
    .unwrap();
    session
        .publish_with_restore(3, json!("temporary"), None, Some(json!("ordinary")))
        .unwrap();
    release.send(()).unwrap();
    let (failures, result) = stopped(&replies);
    assert!(
        failures
            .iter()
            .any(|error| error.contains("simulated Rust restore panic"))
    );
    assert!(result.unwrap_err().contains("simulated Rust restore panic"));
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
    drop(session);
}

#[test]
fn replacement_of_same_target_waits_until_outgoing_transport_is_released() {
    let target_lock = lifecycle(70, "replacement-target").unwrap();
    let observed_lock = target_lock.clone();
    let first_dropped = Arc::new(AtomicBool::new(false));
    let observed_drop = first_dropped.clone();
    let (entered, shutdown_pending) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (mock, _, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDeviceEvents { .. } => anyhow::bail!("end original session"),
        ServiceRequest::Shutdown => {
            entered.send(()).unwrap();
            gate.recv_timeout(TIMEOUT).unwrap();
            Ok(json!({"stopped":true}))
        }
        request => panic!("unexpected request: {request:?}"),
    });
    let mock = mock.on_drop(move || {
        assert!(
            observed_lock.try_lock().is_err(),
            "target lock released before outgoing transport"
        );
        observed_drop.store(true, Ordering::Release);
    });
    let (first, first_replies) =
        Session::start_with(70, "replacement-target".into(), || Ok(mock)).unwrap();
    shutdown_pending.recv_timeout(TIMEOUT).unwrap();
    assert!(target_lock.try_lock().is_err());
    let (mock, _, second_cleanup) = Mock::new(|request| match request {
        ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
            Ok(json!({"registered":true,"driver_ready":true,"owner_guid":owner_guid}))
        }
        ServiceRequest::MappingDeviceEvents { .. } => anyhow::bail!("end replacement session"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (second, second_replies) =
        Session::start_with(70, "REPLACEMENT-TARGET".into(), move || {
            assert!(
                first_dropped.load(Ordering::Acquire),
                "replacement started before target cleanup"
            );
            Ok(mock)
        })
        .unwrap();
    release.send(()).unwrap();
    assert!(stopped(&first_replies).1.is_ok());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(stopped(&second_replies).1.is_ok());
    second_cleanup.recv_timeout(TIMEOUT).unwrap();
    drop(first);
    drop(second);
}

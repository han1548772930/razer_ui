use super::*;
use crate::mapping::test_transport::{Mock, TIMEOUT};
use serde_json::{Value, json};

fn target() -> NativeTarget {
    NativeTarget::new(196, Uuid::new_v4().to_string()).unwrap()
}
fn common(request: ServiceRequest) -> anyhow::Result<Value> {
    match request {
        ServiceRequest::MappingRotationStart { owner_guid, .. } => Ok(serde_json::to_value(
            NativeRegistration::new(owner_guid, true),
        )?),
        ServiceRequest::MappingRotationStop { owner_guid } => Ok(serde_json::to_value(
            NativeRegistration::new(owner_guid, false),
        )?),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("Unexpected rotation request: {request:?}"),
    }
}
fn progress(owner: String, progress: NativeProgress) -> anyhow::Result<Value> {
    Ok(serde_json::to_value(NativeUpdate::new(owner, progress))?)
}
fn ready(replies: &mpsc::Receiver<Reply>) {
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready
    ));
}
fn completed(
    replies: &mpsc::Receiver<Reply>,
    expected: RequestId,
) -> Result<RotationReply, String> {
    let Reply::Completed { id, result } = replies.recv_timeout(TIMEOUT).unwrap() else {
        panic!("Expected operation completion");
    };
    assert_eq!(id, expected);
    result
}

#[test]
fn explicit_get_and_set_share_registration_without_hidden_readback() {
    let mut operation = None;
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingRotationRequest {
            owner_guid,
            request,
        } => {
            assert!(operation.replace(request).is_none());
            progress(owner_guid, NativeProgress::Pending)
        }
        ServiceRequest::MappingRotationPoll { owner_guid } => {
            let reply = match operation.take().unwrap() {
                RotationRequest::SetAngle(347) => RotationReply::SetComplete,
                RotationRequest::GetAngle => RotationReply::Angle {
                    raw: 271,
                    bytes: 271_i32.to_le_bytes(),
                },
                request => panic!("Unexpected operation: {request:?}"),
            };
            progress(owner_guid, NativeProgress::Completed(reply))
        }
        request => common(request),
    });
    let (session, replies) = Session::start_with(target(), || Ok(mock)).unwrap();
    ready(&replies);
    let set = session.request(RotationRequest::SetAngle(347)).unwrap();
    assert_eq!(
        completed(&replies, set).unwrap(),
        RotationReply::SetComplete
    );
    let get = session.request(RotationRequest::GetAngle).unwrap();
    assert_ne!(get, set);
    assert_eq!(
        completed(&replies, get).unwrap(),
        RotationReply::Angle {
            raw: 271,
            bytes: 271_i32.to_le_bytes()
        }
    );
    session.stop().unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Stopped(Ok(()))
    ));
    let trace = trace.lock().unwrap();
    assert_eq!(trace.len(), 7);
    assert!(matches!(
        &trace[0],
        ServiceRequest::MappingRotationStart { .. }
    ));
    assert!(matches!(
        &trace[1],
        ServiceRequest::MappingRotationRequest {
            request: RotationRequest::SetAngle(347),
            ..
        }
    ));
    assert!(matches!(
        &trace[3],
        ServiceRequest::MappingRotationRequest {
            request: RotationRequest::GetAngle,
            ..
        }
    ));
    assert!(matches!(
        &trace[5],
        ServiceRequest::MappingRotationStop { .. }
    ));
    assert!(matches!(&trace[6], ServiceRequest::Shutdown));
}

#[test]
fn lost_registration_response_still_stops_and_preserves_cleanup_failure() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::MappingRotationStart { .. } => anyhow::bail!("registration response lost"),
        ServiceRequest::MappingRotationStop { .. } => anyhow::bail!("stop failed"),
        ServiceRequest::Shutdown => anyhow::bail!("scope release failed"),
        request => panic!("Unexpected request: {request:?}"),
    });
    let (session, replies) = Session::start_with(target(), || Ok(mock)).unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let error = session.stop().unwrap_err();
    for fragment in [
        "registration response lost",
        "stop failed",
        "scope release failed",
    ] {
        assert!(error.contains(fragment));
    }
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Failed(_)
    ));
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Stopped(Err(_))
    ));
    assert_eq!(trace.lock().unwrap().len(), 3);
}

#[test]
fn cancellation_rejects_late_success_and_waits_for_stop_and_transport_drop() {
    let (submitted, submission) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (stopping, stop_entered) = mpsc::channel();
    let (finish, finishing) = mpsc::channel();
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingRotationRequest { owner_guid, .. } => {
            submitted.send(()).unwrap();
            released.recv_timeout(TIMEOUT).unwrap();
            progress(
                owner_guid,
                NativeProgress::Completed(RotationReply::SetComplete),
            )
        }
        ServiceRequest::MappingRotationStop { owner_guid } => {
            stopping.send(()).unwrap();
            finishing.recv_timeout(TIMEOUT).unwrap();
            common(ServiceRequest::MappingRotationStop { owner_guid })
        }
        request => common(request),
    });
    let (session, replies) = Session::start_with(target(), || Ok(mock)).unwrap();
    ready(&replies);
    let current = session.request(RotationRequest::SetAngle(8)).unwrap();
    submission.recv_timeout(TIMEOUT).unwrap();
    let queued = session.request(RotationRequest::GetAngle).unwrap();
    session.request_stop();
    assert!(session.request(RotationRequest::SetAngle(9)).is_err());
    release.send(()).unwrap();
    assert!(
        completed(&replies, current)
            .unwrap_err()
            .contains("canceled")
    );
    stop_entered.recv_timeout(TIMEOUT).unwrap();
    assert!(replies.try_recv().is_err());
    assert!(cleanup.try_recv().is_err());
    finish.send(()).unwrap();
    session.stop().unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(
        completed(&replies, queued)
            .unwrap_err()
            .contains("before submission")
    );
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Stopped(Ok(()))
    ));
    assert_eq!(
        trace
            .lock()
            .unwrap()
            .iter()
            .filter(|r| matches!(r, ServiceRequest::MappingRotationRequest { .. }))
            .count(),
        1
    );
}

#[test]
fn mismatched_owner_or_reply_fails_without_consuming_as_observed_angle() {
    for mode in 0..3 {
        let (mock, _, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::MappingRotationRequest { owner_guid, .. } => match mode {
                0 => progress(
                    "retired".into(),
                    NativeProgress::Completed(RotationReply::SetComplete),
                ),
                1 => progress(
                    owner_guid,
                    NativeProgress::Completed(RotationReply::SetComplete),
                ),
                _ => progress(
                    owner_guid,
                    NativeProgress::Completed(RotationReply::Angle {
                        raw: 17,
                        bytes: 18_i32.to_le_bytes(),
                    }),
                ),
            },
            request => common(request),
        });
        let (session, replies) = Session::start_with(target(), || Ok(mock)).unwrap();
        ready(&replies);
        let id = session.request(RotationRequest::GetAngle).unwrap();
        let error = completed(&replies, id).unwrap_err();
        assert!(error.contains(if mode == 0 {
            "owner changed"
        } else {
            "does not match"
        }));
        cleanup.recv_timeout(TIMEOUT).unwrap();
        assert!(session.stop().is_err());
    }
}

#[test]
fn cancellation_during_client_creation_skips_registration_and_cleans_scope() {
    let (creating, created) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (mock, trace, cleanup) = Mock::new(common);
    let (session, replies) = Session::start_with(target(), move || {
        creating.send(()).unwrap();
        released.recv_timeout(TIMEOUT).unwrap();
        Ok(mock)
    })
    .unwrap();
    created.recv_timeout(TIMEOUT).unwrap();
    let id = session.request(RotationRequest::GetAngle).unwrap();
    session.request_stop();
    release.send(()).unwrap();
    session.stop().unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(completed(&replies, id).is_err());
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Stopped(Ok(()))
    ));
    assert!(matches!(
        &trace.lock().unwrap()[..],
        [ServiceRequest::Shutdown]
    ));
}

#[test]
fn replacement_waits_for_transport_release_and_uses_distinct_request_identity() {
    let identity = target();
    let (dropping, dropped) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (first_mock, _, first_cleanup) = Mock::new(common);
    let first_mock = first_mock.on_drop(move || {
        dropping.send(()).unwrap();
        released.recv_timeout(TIMEOUT).unwrap();
    });
    let (first, first_replies) = Session::start_with(identity.clone(), || Ok(first_mock)).unwrap();
    ready(&first_replies);
    first.request_stop();
    dropped.recv_timeout(TIMEOUT).unwrap();
    let (created, creation) = mpsc::channel();
    let (second_mock, _, second_cleanup) = Mock::new(common);
    let (second, second_replies) = Session::start_with(identity, move || {
        created.send(()).unwrap();
        Ok(second_mock)
    })
    .unwrap();
    assert_ne!(first.owner, second.owner);
    assert!(creation.try_recv().is_err());
    assert!(second_replies.try_recv().is_err());
    release.send(()).unwrap();
    first.stop().unwrap();
    first_cleanup.recv_timeout(TIMEOUT).unwrap();
    creation.recv_timeout(TIMEOUT).unwrap();
    ready(&second_replies);
    second.stop().unwrap();
    second_cleanup.recv_timeout(TIMEOUT).unwrap();
}

#[test]
fn acknowledged_operation_is_separate_from_later_cleanup_failure() {
    let (mock, _, cleanup) = Mock::new(|request| match request {
        ServiceRequest::MappingRotationRequest { owner_guid, .. } => progress(
            owner_guid,
            NativeProgress::Completed(RotationReply::SetComplete),
        ),
        ServiceRequest::MappingRotationStop { .. } => anyhow::bail!("cleanup failure"),
        request => common(request),
    });
    let (session, replies) = Session::start_with(target(), || Ok(mock)).unwrap();
    ready(&replies);
    let id = session.request(RotationRequest::SetAngle(359)).unwrap();
    assert_eq!(completed(&replies, id).unwrap(), RotationReply::SetComplete);
    assert!(session.stop().unwrap_err().contains("cleanup failure"));
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Failed(_)
    ));
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Stopped(Err(_))
    ));
}

#[test]
fn completed_device_failure_allows_user_retry_on_the_same_registration() {
    let mut failed = false;
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::MappingRotationRequest {
            owner_guid,
            request: RotationRequest::SetAngle(16),
        } => {
            if !failed {
                failed = true;
                progress(
                    owner_guid,
                    NativeProgress::Failed {
                        reason: "device rejected write".into(),
                    },
                )
            } else {
                progress(
                    owner_guid,
                    NativeProgress::Completed(RotationReply::SetComplete),
                )
            }
        }
        request => common(request),
    });
    let (session, replies) = Session::start_with(target(), || Ok(mock)).unwrap();
    ready(&replies);
    let first = session.request(RotationRequest::SetAngle(16)).unwrap();
    assert_eq!(
        completed(&replies, first).unwrap_err(),
        "device rejected write"
    );
    assert!(!session.is_finished());
    let second = session.request(RotationRequest::SetAngle(16)).unwrap();
    assert_eq!(
        completed(&replies, second).unwrap(),
        RotationReply::SetComplete
    );
    session.stop().unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Stopped(Ok(()))
    ));
    let trace = trace.lock().unwrap();
    assert_eq!(
        trace
            .iter()
            .filter(|request| matches!(request, ServiceRequest::MappingRotationStart { .. }))
            .count(),
        1
    );
    assert_eq!(
        trace
            .iter()
            .filter(|request| matches!(request, ServiceRequest::MappingRotationRequest { .. }))
            .count(),
        2
    );
}

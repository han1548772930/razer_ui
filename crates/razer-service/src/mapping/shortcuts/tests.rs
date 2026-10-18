use super::*;
use crate::mapping::test_transport::{Mock, TIMEOUT};
use razer_model::shortcuts::ShortcutOutput;
use serde_json::{Value, json};

static TESTS: Mutex<()> = Mutex::new(());

fn acknowledge(request: ServiceRequest) -> anyhow::Result<Value> {
    Ok(match request {
        ServiceRequest::GlobalShortcutStart { owner_guid } => {
            json!({"owner_guid":owner_guid,"registered":true})
        }
        ServiceRequest::GlobalShortcutPublish {
            owner_guid,
            revision,
            ..
        } => {
            json!({"owner_guid":owner_guid,"revision":revision,"published":true})
        }
        ServiceRequest::GlobalShortcutPoll { owner_guid } => {
            json!({"owner_guid":owner_guid,"events":[]})
        }
        ServiceRequest::GlobalShortcutStop { owner_guid } => {
            json!({"owner_guid":owner_guid,"stopped":true})
        }
        ServiceRequest::MappingExecution {
            owner_guid,
            revision,
            target,
        } => serde_json::to_value(razer_ipc::mapping_control::Receipt::new(
            owner_guid, revision, target,
        ))?,
        ServiceRequest::Shutdown => json!({"stopped":true}),
        request => panic!("unexpected request: {request:?}"),
    })
}

fn ready(replies: &mpsc::Receiver<Reply>) {
    assert!(matches!(
        replies.recv_timeout(TIMEOUT).unwrap(),
        Reply::Ready
    ));
}

fn publication(replies: &mpsc::Receiver<Reply>, expected: u64) -> Result<(), String> {
    match replies.recv_timeout(TIMEOUT).unwrap() {
        Reply::Published { revision, result } => {
            assert_eq!(revision, expected);
            result
        }
        _ => panic!("expected publication receipt"),
    }
}

fn stopped(replies: &mpsc::Receiver<Reply>) -> (Vec<String>, Result<(), String>) {
    let mut failures = Vec::new();
    loop {
        match replies.recv_timeout(TIMEOUT).unwrap() {
            Reply::Failed(error) => failures.push(error),
            Reply::Stopped(result) => return (failures, result),
            Reply::Published { .. } | Reply::Ready | Reply::Unsupported(_) => {
                panic!("unexpected response while stopping")
            }
        }
    }
}

fn cut() -> Shortcut {
    Shortcut::new(
        "local-shortcut".into(),
        "KEY_A".into(),
        vec![],
        false,
        ShortcutOutput::Windows {
            action: "Cut".into(),
        },
    )
}

#[test]
fn retained_compiler_serializes_snapshots_until_stop_shutdown_and_release() {
    let _tests = TESTS.lock().unwrap();
    let released = Arc::new(AtomicBool::new(false));
    let observed = released.clone();
    let (mock, trace, cleanup) = Mock::new(acknowledge);
    let mock = mock.on_drop(move || {
        assert!(
            LIFECYCLE.try_lock().is_err(),
            "lifecycle released before transport"
        );
        observed.store(true, Ordering::Release);
    });
    let (session, replies) = Session::start_with(|| Ok(mock)).unwrap();
    ready(&replies);
    session
        .publish(1, vec![], MacroLibraryFile::default())
        .unwrap();
    session
        .publish(2, vec![cut()], MacroLibraryFile::default())
        .unwrap();
    publication(&replies, 1).unwrap();
    publication(&replies, 2).unwrap();
    assert!(!session.is_finished());
    session.request_stop();
    assert!(
        session
            .publish(3, vec![], MacroLibraryFile::default())
            .is_err()
    );
    let (failures, result) = stopped(&replies);
    assert!(failures.is_empty());
    result.unwrap();
    assert!(session.is_finished());
    assert!(released.load(Ordering::Acquire));
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    let documents: Vec<_> = trace
        .iter()
        .filter_map(|request| match request {
            ServiceRequest::GlobalShortcutPublish {
                owner_guid,
                revision,
                document,
            } => Some((owner_guid, *revision, document)),
            _ => None,
        })
        .collect();
    assert_eq!(documents.len(), 2);
    assert_eq!([documents[0].1, documents[1].1], [1, 2]);
    assert_eq!(documents[0].0, documents[1].0);
    assert_eq!(
        documents[0].2.default_turbos(),
        documents[1].2.default_turbos()
    );
    assert_eq!(documents[0].2.app_engine()["mappings"], json!([]));
    assert!(
        !documents[1].2.app_engine()["mappings"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        matches!(&trace[trace.len() - 2], ServiceRequest::GlobalShortcutStop { owner_guid }
        if owner_guid == documents[0].0)
    );
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn failed_compilation_and_stale_revisions_never_publish_or_replace_context() {
    let _tests = TESTS.lock().unwrap();
    let (mock, trace, cleanup) = Mock::new(acknowledge);
    let (session, replies) = Session::start_with(|| Ok(mock)).unwrap();
    ready(&replies);
    session
        .publish(10, vec![cut()], MacroLibraryFile::default())
        .unwrap();
    publication(&replies, 10).unwrap();
    let mut invalid = cut();
    invalid.id.clear();
    session
        .publish(11, vec![invalid], MacroLibraryFile::default())
        .unwrap();
    assert!(publication(&replies, 11).is_err());
    session
        .publish(11, vec![], MacroLibraryFile::default())
        .unwrap();
    assert!(publication(&replies, 11).unwrap_err().contains("stale"));
    session
        .publish(9, vec![], MacroLibraryFile::default())
        .unwrap();
    assert!(publication(&replies, 9).unwrap_err().contains("stale"));
    session
        .publish(12, vec![], MacroLibraryFile::default())
        .unwrap();
    publication(&replies, 12).unwrap();
    drop(session);
    let (failures, result) = stopped(&replies);
    assert!(failures.is_empty());
    result.unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    let documents: Vec<_> = trace
        .iter()
        .filter_map(|request| match request {
            ServiceRequest::GlobalShortcutPublish {
                revision, document, ..
            } => Some((*revision, document)),
            _ => None,
        })
        .collect();
    assert_eq!(documents.len(), 2);
    assert_eq!([documents[0].0, documents[1].0], [10, 12]);
    assert_eq!(
        documents[0].1.default_turbos(),
        documents[1].1.default_turbos()
    );
}

#[test]
fn partial_or_mismatched_start_still_stops_and_shuts_down_its_owner() {
    let _tests = TESTS.lock().unwrap();
    for reply in [
        json!({}),
        json!({"registered":true,"owner_guid":"stale"}),
        json!({"registered":false,"owner_guid":"stale"}),
    ] {
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::GlobalShortcutStart { .. } => Ok(reply.clone()),
            request => acknowledge(request),
        });
        let (session, replies) = Session::start_with(|| Ok(mock)).unwrap();
        let (failures, result) = stopped(&replies);
        assert_eq!(failures.len(), 1);
        assert!(result.is_err());
        assert!(session.is_finished());
        cleanup.recv_timeout(TIMEOUT).unwrap();
        let trace = trace.lock().unwrap();
        assert_eq!(trace.len(), 3);
        assert!(
            matches!(&trace[1], ServiceRequest::GlobalShortcutStop { owner_guid }
            if matches!(&trace[0], ServiceRequest::GlobalShortcutStart { owner_guid: started } if started == owner_guid))
        );
        assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
    }
}

#[test]
fn native_publication_failures_require_a_later_revision_and_keep_cleanup() {
    let _tests = TESTS.lock().unwrap();
    let mut attempt = 0;
    let (mock, trace, cleanup) = Mock::new(move |request| {
        if let ServiceRequest::GlobalShortcutPublish {
            owner_guid,
            revision,
            ..
        } = &request
        {
            attempt += 1;
            return match attempt {
                1 => anyhow::bail!("dependency storage rejected"),
                2 => Ok(json!({"owner_guid":owner_guid,"revision":revision + 1,"published":true})),
                3 => {
                    Ok(json!({"owner_guid":"previous-owner","revision":revision,"published":true}))
                }
                4 => Ok(json!({"owner_guid":owner_guid,"revision":revision,"published":false})),
                5 => Ok(json!({"owner_guid":owner_guid,"published":true})),
                _ => acknowledge(request),
            };
        }
        acknowledge(request)
    });
    let (session, replies) = Session::start_with(|| Ok(mock)).unwrap();
    ready(&replies);
    for revision in 1..=6 {
        session
            .publish(revision, vec![], MacroLibraryFile::default())
            .unwrap();
        assert_eq!(publication(&replies, revision).is_ok(), revision == 6);
    }
    session
        .publish(6, vec![], MacroLibraryFile::default())
        .unwrap();
    assert!(publication(&replies, 6).unwrap_err().contains("stale"));
    drop(session);
    let (failures, result) = stopped(&replies);
    assert!(failures.is_empty());
    result.unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    assert_eq!(
        trace
            .iter()
            .filter(|request| matches!(request, ServiceRequest::GlobalShortcutPublish { .. }))
            .count(),
        6
    );
    assert!(matches!(
        &trace[trace.len() - 2],
        ServiceRequest::GlobalShortcutStop { .. }
    ));
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn event_batch_preserves_source_strings_and_rejects_another_owner() {
    let _tests = TESTS.lock().unwrap();
    let expected = UnsupportedMapping::new(
        "{\"productId\":70}".into(),
        "[1]".into(),
        "{}".into(),
        u64::MAX,
    );
    let event = expected.clone();
    let mut polled = false;
    let (mock, trace, cleanup) = Mock::new(move |request| {
        if let ServiceRequest::GlobalShortcutPoll { owner_guid } = request {
            if !polled {
                polled = true;
                return Ok(serde_json::to_value(EventBatch::new(
                    owner_guid,
                    vec![event.clone()],
                ))?);
            }
            return Ok(json!({"owner_guid":"previous-owner","events":[]}));
        }
        acknowledge(request)
    });
    let (_session, replies) = Session::start_with(|| Ok(mock)).unwrap();
    ready(&replies);
    assert!(
        matches!(replies.recv_timeout(TIMEOUT).unwrap(), Reply::Unsupported(event) if event == expected)
    );
    let (failures, result) = stopped(&replies);
    assert_eq!(failures.len(), 1);
    assert!(failures[0].contains("owner changed"));
    assert!(result.is_err());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
}

#[test]
fn malformed_events_and_unknown_callback_kind_terminate_with_cleanup() {
    let _tests = TESTS.lock().unwrap();
    for events in [
        json!(null),
        json!([{"event_type":2,"device_info":"{}","input":"{}","output":"{}","time_tick":1}]),
    ] {
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::GlobalShortcutPoll { owner_guid } => {
                Ok(json!({"owner_guid":owner_guid,"events":events}))
            }
            request => acknowledge(request),
        });
        let (_session, replies) = Session::start_with(|| Ok(mock)).unwrap();
        ready(&replies);
        let (failures, result) = stopped(&replies);
        assert_eq!(failures.len(), 1);
        assert!(result.is_err());
        cleanup.recv_timeout(TIMEOUT).unwrap();
        assert!(matches!(
            trace.lock().unwrap().last(),
            Some(ServiceRequest::Shutdown)
        ));
    }
}

#[test]
fn stop_and_independent_shutdown_failures_survive_transport_release() {
    let _tests = TESTS.lock().unwrap();
    for stop_reply in [
        json!({}),
        json!({"owner_guid":"previous-owner","stopped":true}),
        json!({"owner_guid":"previous-owner","stopped":false}),
    ] {
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::GlobalShortcutStop { .. } => Ok(stop_reply.clone()),
            ServiceRequest::Shutdown => anyhow::bail!("engine shutdown rejected"),
            request => acknowledge(request),
        });
        let (session, replies) = Session::start_with(|| Ok(mock)).unwrap();
        ready(&replies);
        session.request_stop();
        let (failures, result) = stopped(&replies);
        assert!(failures.is_empty());
        let error = result.unwrap_err();
        assert!(error.contains("engine shutdown rejected"));
        assert!(error.contains("stop was not confirmed") || error.contains("missing field"));
        cleanup.recv_timeout(TIMEOUT).unwrap();
        assert!(matches!(
            trace.lock().unwrap().last(),
            Some(ServiceRequest::Shutdown)
        ));
    }
}

#[test]
fn cancellation_during_publication_discards_the_next_snapshot_and_finishes_cleanup() {
    let _tests = TESTS.lock().unwrap();
    let (entered, publishing) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (mock, trace, cleanup) = Mock::new(move |request| {
        if matches!(request, ServiceRequest::GlobalShortcutPublish { .. }) {
            entered.send(()).unwrap();
            gate.recv_timeout(TIMEOUT).unwrap();
        }
        acknowledge(request)
    });
    let (session, replies) = Session::start_with(|| Ok(mock)).unwrap();
    ready(&replies);
    session
        .publish(1, vec![], MacroLibraryFile::default())
        .unwrap();
    publishing.recv_timeout(TIMEOUT).unwrap();
    session
        .publish(2, vec![cut()], MacroLibraryFile::default())
        .unwrap();
    session.request_stop();
    assert!(!session.is_finished());
    release.send(()).unwrap();
    publication(&replies, 1).unwrap();
    let (failures, result) = stopped(&replies);
    assert!(failures.is_empty());
    result.unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    assert_eq!(
        trace
            .iter()
            .filter(|request| matches!(request, ServiceRequest::GlobalShortcutPublish { .. }))
            .count(),
        1
    );
    assert!(matches!(
        &trace[trace.len() - 2],
        ServiceRequest::GlobalShortcutStop { .. }
    ));
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn finish_drains_admitted_publications_before_stop_and_rejects_new_snapshots() {
    let _tests = TESTS.lock().unwrap();
    let (entered, publishing) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (mock, trace, cleanup) = Mock::new(move |request| {
        if matches!(
            request,
            ServiceRequest::GlobalShortcutPublish { revision: 1, .. }
        ) {
            entered.send(()).unwrap();
            gate.recv_timeout(TIMEOUT).unwrap();
        }
        acknowledge(request)
    });
    let (session, replies) = Session::start_with(|| Ok(mock)).unwrap();
    ready(&replies);
    session
        .publish(1, vec![], MacroLibraryFile::default())
        .unwrap();
    publishing.recv_timeout(TIMEOUT).unwrap();
    session
        .publish(2, vec![cut()], MacroLibraryFile::default())
        .unwrap();
    session.request_stop_after_pending().unwrap();
    session.request_stop_after_pending().unwrap();
    assert!(
        session
            .publish(3, vec![], MacroLibraryFile::default())
            .is_err()
    );
    assert!(!session.is_finished());
    release.send(()).unwrap();
    publication(&replies, 1).unwrap();
    publication(&replies, 2).unwrap();
    stopped(&replies).1.unwrap();
    assert!(session.is_finished());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    let revisions: Vec<_> = trace
        .iter()
        .filter_map(|request| match request {
            ServiceRequest::GlobalShortcutPublish { revision, .. } => Some(*revision),
            _ => None,
        })
        .collect();
    assert_eq!(revisions, [1, 2]);
    assert!(matches!(
        &trace[trace.len() - 2],
        ServiceRequest::GlobalShortcutStop { .. }
    ));
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn finish_admitted_before_startup_waits_for_predecessor_and_keeps_its_snapshot() {
    let _tests = TESTS.lock().unwrap();
    let (entered, shutting_down) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (mock, _, cleanup) = Mock::new(move |request| {
        if matches!(request, ServiceRequest::Shutdown) {
            entered.send(()).unwrap();
            gate.recv_timeout(TIMEOUT).unwrap();
        }
        acknowledge(request)
    });
    let (first, first_replies) = Session::start_with(|| Ok(mock)).unwrap();
    ready(&first_replies);
    first.request_stop();
    shutting_down.recv_timeout(TIMEOUT).unwrap();
    let (mock, trace, next_cleanup) = Mock::new(acknowledge);
    let (next, replies) = Session::start_with(|| Ok(mock)).unwrap();
    next.publish(7, vec![cut()], MacroLibraryFile::default())
        .unwrap();
    next.request_stop_after_pending().unwrap();
    assert!(trace.lock().unwrap().is_empty());
    release.send(()).unwrap();
    stopped(&first_replies).1.unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    ready(&replies);
    publication(&replies, 7).unwrap();
    stopped(&replies).1.unwrap();
    next_cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(next.is_finished());
    let trace = trace.lock().unwrap();
    assert_eq!(
        trace
            .iter()
            .filter(|request| matches!(
                request,
                ServiceRequest::GlobalShortcutPublish { revision: 7, .. }
            ))
            .count(),
        1
    );
    assert!(matches!(
        &trace[trace.len() - 2],
        ServiceRequest::GlobalShortcutStop { .. }
    ));
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn replacement_waits_for_cleanup_and_a_canceled_waiter_never_creates_transport() {
    let _tests = TESTS.lock().unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let observed = dropped.clone();
    let (entered, shutting_down) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (mock, _, cleanup) = Mock::new(move |request| {
        if matches!(request, ServiceRequest::Shutdown) {
            entered.send(()).unwrap();
            gate.recv_timeout(TIMEOUT).unwrap();
        }
        acknowledge(request)
    });
    let mock = mock.on_drop(move || {
        assert!(LIFECYCLE.try_lock().is_err());
        observed.store(true, Ordering::Release);
    });
    let (first, replies) = Session::start_with(|| Ok(mock)).unwrap();
    ready(&replies);
    first.request_stop();
    shutting_down.recv_timeout(TIMEOUT).unwrap();
    assert!(LIFECYCLE.try_lock().is_err());
    let (canceled, canceled_replies) = Session::start_with(|| -> anyhow::Result<Mock> {
        panic!("canceled queued owner spawned a native process")
    })
    .unwrap();
    canceled.request_stop();
    let (mock, _, second_cleanup) = Mock::new(acknowledge);
    let (second, second_replies) = Session::start_with(move || {
        assert!(
            dropped.load(Ordering::Acquire),
            "replacement started before release"
        );
        Ok(mock)
    })
    .unwrap();
    // Either waiter can obtain the lifecycle next; both must wait for the
    // first transport's native shutdown and release to finish.
    release.send(()).unwrap();
    stopped(&replies).1.unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    ready(&second_replies);
    second.request_stop();
    stopped(&second_replies).1.unwrap();
    second_cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(stopped(&canceled_replies).0.is_empty());
    assert!(first.is_finished() && canceled.is_finished() && second.is_finished());
}

#[test]
fn rust_unwind_keeps_stop_and_shutdown_inside_the_retained_owner() {
    let _tests = TESTS.lock().unwrap();
    let (mock, trace, cleanup) = Mock::new(|request| {
        if matches!(request, ServiceRequest::GlobalShortcutPublish { .. }) {
            panic!("simulated native adapter Rust panic");
        }
        acknowledge(request)
    });
    let (session, replies) = Session::start_with(|| Ok(mock)).unwrap();
    ready(&replies);
    session
        .publish(1, vec![], MacroLibraryFile::default())
        .unwrap();
    let (failures, result) = stopped(&replies);
    assert_eq!(failures.len(), 1);
    assert!(failures[0].contains("simulated native adapter Rust panic"));
    assert!(result.is_err());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    let trace = trace.lock().unwrap();
    assert!(matches!(
        &trace[trace.len() - 2],
        ServiceRequest::GlobalShortcutStop { .. }
    ));
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn owner_registered_after_pause_broadcast_cannot_publish_before_its_initial_ack() {
    use crate::mapping::device;
    let _tests = TESTS.lock().unwrap();
    let registry = Registry::default();
    let (device_pause_entered, device_paused) = mpsc::channel();
    let (continue_device_pause, device_pause) = mpsc::channel();
    let (device_mock, _, device_cleanup) = Mock::new(move |request| {
        Ok(match request {
            ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
                json!({"owner_guid":owner_guid,"registered":true,"driver_ready":true})
            }
            ServiceRequest::MappingDeviceEvents { owner_guid } => {
                json!({"owner_guid":owner_guid,"events":[]})
            }
            ServiceRequest::MappingExecution {
                owner_guid,
                revision,
                target,
            } => {
                if !target.mapping_enabled() {
                    device_pause_entered.send(()).unwrap();
                    device_pause.recv_timeout(TIMEOUT).unwrap();
                }
                serde_json::to_value(razer_ipc::mapping_control::Receipt::new(
                    owner_guid, revision, target,
                ))?
            }
            ServiceRequest::Shutdown => json!({"stopped":true}),
            request => panic!("unexpected request: {request:?}"),
        })
    });
    let (device_session, device_replies) = device::Session::start_with_registry(
        70,
        "pending-scope-startup".into(),
        || Ok(device_mock),
        registry.clone(),
    )
    .unwrap();
    assert!(matches!(
        device_replies.recv_timeout(TIMEOUT).unwrap(),
        device::Reply::Ready
    ));
    let acquire = {
        let registry = registry.clone();
        std::thread::spawn(move || {
            registry.suspend(razer_ipc::mapping_control::Suppression::Mapping)
        })
    };
    device_paused.recv_timeout(TIMEOUT).unwrap();
    let (initial_pause_entered, initial_paused) = mpsc::channel();
    let (continue_initial_pause, initial_pause) = mpsc::channel();
    let (mock, trace, cleanup) = Mock::new(move |request| {
        if matches!(&request,
            ServiceRequest::MappingExecution { target, .. } if !target.mapping_enabled())
        {
            initial_pause_entered.send(()).unwrap();
            initial_pause.recv_timeout(TIMEOUT).unwrap();
        }
        acknowledge(request)
    });
    let (session, replies) = Session::start_with_registry(|| Ok(mock), registry).unwrap();
    session
        .publish(1, vec![], MacroLibraryFile::default())
        .unwrap();
    initial_paused.recv_timeout(TIMEOUT).unwrap();
    continue_device_pause.send(()).unwrap();
    let mut capture = acquire.join().unwrap().unwrap();
    assert!(matches!(replies.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(
        !trace
            .lock()
            .unwrap()
            .iter()
            .any(|request| matches!(request, ServiceRequest::GlobalShortcutPublish { .. }))
    );
    continue_initial_pause.send(()).unwrap();
    ready(&replies);
    publication(&replies, 1).unwrap();
    capture.release().unwrap();
    drop(session);
    stopped(&replies).1.unwrap();
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(trace.lock().unwrap().iter().any(|request| matches!(request,
        ServiceRequest::MappingExecution { target, .. } if target.mapping_enabled())));
    drop(device_session);
    assert!(matches!(
        device_replies.recv_timeout(TIMEOUT).unwrap(),
        device::Reply::Stopped(Ok(()))
    ));
    device_cleanup.recv_timeout(TIMEOUT).unwrap();
}

#[test]
fn rejected_cross_process_suppression_stops_the_actor_and_completes_rollback() {
    let _tests = TESTS.lock().unwrap();
    let registry = Registry::default();
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::MappingExecution { .. } => anyhow::bail!("native mapping pause rejected"),
        request => acknowledge(request),
    });
    let (_session, replies) = Session::start_with_registry(|| Ok(mock), registry.clone()).unwrap();
    ready(&replies);
    let error = registry
        .suspend(razer_ipc::mapping_control::Suppression::Mapping)
        .err()
        .unwrap();
    assert!(error.contains("native mapping pause rejected"));
    let (failures, result) = stopped(&replies);
    assert_eq!(failures.len(), 1);
    assert!(result.is_err());
    cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
}

#[test]
fn recorder_source_started_waits_for_both_mapping_processes_and_restores_them() {
    use crate::mapping::{device, recorder};
    use razer_model::macro_library::{MacroType, recording::Options};
    let _tests = TESTS.lock().unwrap();
    let registry = Registry::default();
    let global_paused = Arc::new(AtomicBool::new(false));
    let device_paused = Arc::new(AtomicBool::new(false));
    let global = global_paused.clone();
    let (global_mock, global_trace, global_cleanup) = Mock::new(move |request| {
        if let ServiceRequest::MappingExecution { target, .. } = &request {
            global.store(!target.mapping_enabled(), Ordering::Release);
        }
        acknowledge(request)
    });
    let (global_session, global_replies) =
        Session::start_with_registry(|| Ok(global_mock), registry.clone()).unwrap();
    ready(&global_replies);
    let device = device_paused.clone();
    let (device_mock, device_trace, device_cleanup) = Mock::new(move |request| {
        Ok(match request {
            ServiceRequest::MappingDeviceStart { owner_guid, .. } => {
                json!({"owner_guid":owner_guid,"registered":true,"driver_ready":true})
            }
            ServiceRequest::MappingDeviceEvents { owner_guid } => {
                json!({"owner_guid":owner_guid,"events":[]})
            }
            ServiceRequest::MappingExecution {
                owner_guid,
                revision,
                target,
            } => {
                device.store(!target.mapping_enabled(), Ordering::Release);
                serde_json::to_value(razer_ipc::mapping_control::Receipt::new(
                    owner_guid, revision, target,
                ))?
            }
            ServiceRequest::Shutdown => json!({"stopped":true}),
            request => panic!("unexpected request: {request:?}"),
        })
    });
    let (device_session, device_replies) = device::Session::start_with_registry(
        70,
        "recording-test-device".into(),
        || Ok(device_mock),
        registry.clone(),
    )
    .unwrap();
    assert!(matches!(
        device_replies.recv_timeout(TIMEOUT).unwrap(),
        device::Reply::Ready
    ));
    let global = global_paused.clone();
    let device = device_paused.clone();
    let mut stopping = false;
    let (record_mock, record_trace, record_cleanup) = Mock::new(move |request| {
        Ok(match request {
            ServiceRequest::StartMacroRecording => {
                assert!(
                    !global.load(Ordering::Acquire) && !device.load(Ordering::Acquire),
                    "source starts recording before the started-event suppression"
                );
                json!({"accepted":true})
            }
            ServiceRequest::MacroRecordingEvents => {
                if stopping {
                    json!({"overflow":false,"events":[{"kind":"stopped","mode":"kSoftware"}]})
                } else {
                    json!({"overflow":false,"events":[{"kind":"started","mode":"kSoftware","received_at_ms":0}]})
                }
            }
            ServiceRequest::SuspendMacroMappings => {
                assert!(
                    global.load(Ordering::Acquire) && device.load(Ordering::Acquire),
                    "recording continued before both real mapping owners paused"
                );
                json!({"suspended":true})
            }
            ServiceRequest::StopMacroRecording => {
                stopping = true;
                assert!(global.load(Ordering::Acquire) && device.load(Ordering::Acquire));
                json!({"accepted":true})
            }
            ServiceRequest::ResumeMacroMappings => json!({"suspended":false,
                "mapping":{"result":true,"reason":""},"global_shortcuts":{"result":true,"reason":""}}),
            ServiceRequest::Shutdown => json!({"stopped":true}),
            request => panic!("unexpected request: {request:?}"),
        })
    });
    let options = Options {
        macro_type: MacroType::Standard,
        phase: None,
        delay_mode: 3,
        fixed: 0.,
        random: [0., 0.],
        next_pair: 1,
        movement_mode: 0,
        shortcut: Vec::new(),
    };
    let (record_session, updates) =
        recorder::spawn_with_registry(options, || Ok(record_mock), registry).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        loop {
            if matches!(
                tokio::time::timeout(TIMEOUT, updates.recv())
                    .await
                    .unwrap()
                    .unwrap(),
                recorder::Update::Recording
            ) {
                break;
            }
        }
    });
    drop(record_session);
    runtime.block_on(async {
        loop {
            if let recorder::Update::Finished(result) =
                tokio::time::timeout(TIMEOUT, updates.recv())
                    .await
                    .unwrap()
                    .unwrap()
            {
                assert!(matches!(result, Ok(None)));
                break;
            }
        }
    });
    assert!(!global_paused.load(Ordering::Acquire) && !device_paused.load(Ordering::Acquire));
    record_cleanup.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        record_trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
    for trace in [global_trace, device_trace] {
        let trace = trace.lock().unwrap();
        let targets: Vec<_> = trace
            .iter()
            .filter_map(|request| match request {
                ServiceRequest::MappingExecution { target, .. } => Some(*target),
                _ => None,
            })
            .collect();
        assert_eq!(targets.len(), 2);
        assert!(!targets[0].mapping_enabled() && targets[0].global_shortcuts_enabled().is_none());
        assert!(
            targets[1].mapping_enabled() && targets[1].global_shortcuts_enabled() == Some(true)
        );
    }
    drop(global_session);
    stopped(&global_replies).1.unwrap();
    global_cleanup.recv_timeout(TIMEOUT).unwrap();
    drop(device_session);
    assert!(matches!(
        device_replies.recv_timeout(TIMEOUT).unwrap(),
        device::Reply::Stopped(Ok(()))
    ));
    device_cleanup.recv_timeout(TIMEOUT).unwrap();
}

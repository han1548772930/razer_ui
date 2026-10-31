use super::*;
use crate::mapping::test_transport::{Mock, TIMEOUT};
use razer_model::macro_library::MacroType;
use serde_json::json;

fn options() -> Options {
    Options {
        macro_type: MacroType::Standard,
        phase: None,
        delay_mode: 3,
        fixed: 0.,
        random: [0., 0.],
        next_pair: 1,
        movement_mode: 0,
        shortcut: Vec::new(),
        finalization_macro_type: MacroType::Standard,
        finalization_delay_mode: 3,
    }
}

fn next(updates: &async_channel::Receiver<Update>) -> Update {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        tokio::time::timeout(TIMEOUT, updates.recv())
            .await
            .unwrap()
            .unwrap()
    })
}

fn finished(updates: &async_channel::Receiver<Update>) -> Result<Option<Vec<ActionItem>>, String> {
    let mut rows = Vec::new();
    loop {
        match next(updates) {
            Update::PostProcessing(request) => request.complete(Ok(options())).unwrap(),
            Update::Commit(request) => request
                .complete(|received| {
                    rows = received;
                    Ok(rows.len())
                })
                .unwrap(),
            Update::Finished(result) => {
                return result.map(|completion| match completion {
                    Completion::Recorded { .. } => Some(rows),
                    Completion::Cancelled | Completion::CancelledAfterCommit { .. } => None,
                });
            }
            _ => {}
        }
    }
}

fn commit(updates: &async_channel::Receiver<Update>, options: Options) -> Vec<ActionItem> {
    loop {
        match next(updates) {
            Update::PostProcessing(request) => request.complete(Ok(options.clone())).unwrap(),
            Update::Commit(request) => {
                let mut rows = Vec::new();
                request
                    .complete(|received| {
                        rows = received;
                        Ok(rows.len())
                    })
                    .unwrap();
                return rows;
            }
            Update::Finished(result) => panic!("recording ended before its commit: {result:?}"),
            _ => {}
        }
    }
}

#[test]
fn dropping_the_page_session_stops_recording_restores_mappings_and_releases_transport() {
    let mut stopped = false;
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents => {
            if stopped {
                Ok(json!({"overflow":false,"events":[{"kind":"stopped","mode":"kSoftware"}]}))
            } else {
                Ok(
                    json!({"overflow":false,"events":[{"kind":"started","mode":"kSoftware","received_at_ms":0}]}),
                )
            }
        }
        ServiceRequest::SuspendMacroMappings => Ok(json!({"suspended":true})),
        ServiceRequest::StopMacroRecording => {
            stopped = true;
            Ok(json!({"accepted":true}))
        }
        ServiceRequest::ResumeMacroMappings => Ok(restored()),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
    while !matches!(next(&updates), Update::Recording) {}
    drop(session);
    assert!(matches!(finished(&updates), Ok(None)));
    cleanup.try_recv().unwrap();
    let trace = trace.lock().unwrap();
    let stop = trace
        .iter()
        .position(|request| matches!(request, ServiceRequest::StopMacroRecording))
        .unwrap();
    let resume = trace
        .iter()
        .position(|request| matches!(request, ServiceRequest::ResumeMacroMappings))
        .unwrap();
    let shutdown = trace
        .iter()
        .position(|request| matches!(request, ServiceRequest::Shutdown))
        .unwrap();
    assert!(stop < resume && resume < shutdown);
}

fn completed_batch() -> Value {
    json!({"overflow":false,"events":[
        {"kind":"started","mode":"kSoftware","received_at_ms":0},
        {"kind":"stopped","mode":"kSoftware"}
    ]})
}

fn restored() -> Value {
    json!({"suspended":false,"mapping":{"result":true,"reason":""},"global_shortcuts":{"result":true,"reason":""}})
}

#[test]
fn shortcut_stop_carries_its_cause_through_restoration_and_postprocessing() {
    let mut started = false;
    let mut stopped = false;
    let (mock, trace, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents if stopped => {
            Ok(json!({"overflow":false,"events":[{"kind":"stopped","mode":"kSoftware"}]}))
        }
        ServiceRequest::MacroRecordingEvents if !started => {
            started = true;
            let mut events = vec![json!({"kind":"started","mode":"kSoftware","received_at_ms":0})];
            for (tick, vk, scan, down) in [
                (1, 65, 30, false),
                (2, 162, 29, false),
                (3, 66, 48, true),
                (4, 66, 48, false),
                (5, 162, 29, true),
                (6, 65, 30, true),
                (7, 65, 30, false),
                (8, 162, 29, false),
            ] {
                events.push(json!({"kind":"item","time_tick":tick,"received_at_ms":tick,"event":[{"action":if down {"keydown"} else {"keyup"},"vkCode":vk,"scanCode":scan,"flags":if down {0} else {128},"time":1000}]}));
            }
            Ok(json!({"overflow":false,"events":events}))
        }
        ServiceRequest::MacroRecordingEvents => Ok(json!({"overflow":false,"events":[]})),
        ServiceRequest::SuspendMacroMappings => Ok(json!({"suspended":true})),
        ServiceRequest::StopMacroRecording => {
            stopped = true;
            Ok(json!({"accepted":true}))
        }
        ServiceRequest::ResumeMacroMappings => Ok(restored()),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let mut settings = options();
    settings.shortcut = vec![
        razer_model::macro_library::recording::ShortcutKey {
            virtual_key: 162,
            key_code: 17,
        },
        razer_model::macro_library::recording::ShortcutKey {
            virtual_key: 65,
            key_code: 65,
        },
    ];
    let (session, updates) = spawn_with(settings, || Ok(mock)).unwrap();
    while !matches!(next(&updates), Update::Recording) {}
    session.send(Command::StopShortcut).unwrap();
    let rows = finished(&updates).unwrap().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| {
        row.keyboard
            .as_ref()
            .is_some_and(|key| key.makecode == Some(66))
    }));
    cleanup.try_recv().unwrap();
    let trace = trace.lock().unwrap();
    let stop = trace
        .iter()
        .position(|request| matches!(request, ServiceRequest::StopMacroRecording))
        .unwrap();
    let restore = trace
        .iter()
        .position(|request| matches!(request, ServiceRequest::ResumeMacroMappings))
        .unwrap();
    assert!(stop < restore);
    assert!(matches!(trace.last(), Some(ServiceRequest::Shutdown)));
}

#[test]
fn completion_is_sent_after_native_shutdown_and_transport_release() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents => Ok(completed_batch()),
        ServiceRequest::ResumeMacroMappings => Ok(restored()),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
    assert!(matches!(finished(&updates), Ok(Some(rows)) if rows.is_empty()));
    cleanup
        .try_recv()
        .expect("Finished preceded transport release");
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
    drop(session);
}

#[test]
fn cancellation_during_shutdown_is_reported_after_the_commit_acknowledgement() {
    let (entered, wait_entered) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let (mock, _, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents => Ok(completed_key_batch()),
        ServiceRequest::ResumeMacroMappings => Ok(restored()),
        ServiceRequest::Shutdown => {
            entered.send(()).unwrap();
            wait_release.recv().unwrap();
            Ok(json!({"stopped":true}))
        }
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
    assert_eq!(commit(&updates, options()).len(), 2);
    wait_entered.recv_timeout(TIMEOUT).unwrap();
    session.send(Command::Cancel).unwrap();
    release.send(()).unwrap();
    loop {
        if let Update::Finished(result) = next(&updates) {
            assert_eq!(result, Ok(Completion::CancelledAfterCommit { count: 2 }));
            break;
        }
    }
    cleanup.try_recv().unwrap();
}

#[test]
fn unwinding_recording_still_shuts_down_and_keeps_cleanup_failure() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMacroRecording => panic!("recording transport panic"),
        ServiceRequest::Shutdown => anyhow::bail!("cleanup rejection"),
        request => panic!("unexpected request: {request:?}"),
    });
    let (_session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
    let error = finished(&updates).unwrap_err();
    assert!(error.contains("recording transport panic"));
    assert!(error.contains("cleanup rejection"));
    cleanup.try_recv().unwrap();
    assert_eq!(trace.lock().unwrap().len(), 2);
}

#[test]
fn refused_or_malformed_start_is_not_an_accepted_recording() {
    for receipt in [
        json!({"accepted":false}),
        json!({}),
        json!({"accepted":"true"}),
    ] {
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::StartMacroRecording => Ok(receipt.clone()),
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let (_session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
        assert!(finished(&updates).is_err());
        cleanup.try_recv().unwrap();
        assert_eq!(trace.lock().unwrap().len(), 2);
    }
}

#[test]
fn refused_or_malformed_suspension_cleans_up_without_announcing_recording() {
    for receipt in [
        json!({"suspended":false}),
        json!({}),
        json!({"suspended":"true"}),
    ] {
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
            ServiceRequest::MacroRecordingEvents => Ok(json!({"overflow":false,"events":[
                {"kind":"started","mode":"kSoftware","received_at_ms":0}
            ]})),
            ServiceRequest::SuspendMacroMappings => Ok(receipt.clone()),
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let (_session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
        loop {
            match next(&updates) {
                Update::Recording => panic!("recording announced before suppression confirmation"),
                Update::Finished(result) => {
                    assert!(result.is_err());
                    break;
                }
                _ => {}
            }
        }
        cleanup.try_recv().unwrap();
        assert!(matches!(
            trace.lock().unwrap().last(),
            Some(ServiceRequest::Shutdown)
        ));
    }
}

#[test]
fn malformed_restoration_reports_failure_after_commit_and_still_shuts_down() {
    for receipt in [
        json!({"global_shortcuts":{"result":true}}),
        json!({"suspended":false,"mapping":{"result":true,"reason":""},"global_shortcuts":{"result":"true","reason":""}}),
    ] {
        let (mock, trace, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
            ServiceRequest::MacroRecordingEvents => Ok(completed_batch()),
            ServiceRequest::ResumeMacroMappings => Ok(receipt.clone()),
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let (_session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
        assert!(commit(&updates, options()).is_empty());
        assert!(finished(&updates).unwrap_err().contains("恢复响应格式错误"));
        cleanup.try_recv().unwrap();
        assert!(matches!(
            trace.lock().unwrap().last(),
            Some(ServiceRequest::Shutdown)
        ));
    }
}

#[test]
fn source_false_restoration_is_data_and_does_not_announce_shortcut_success() {
    let (mock, _, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents => Ok(completed_batch()),
        ServiceRequest::ResumeMacroMappings => Ok(json!({
            "suspended":false,
            "mapping":{"result":false,"reason":"no temporary mapping owned"},
            "global_shortcuts":{"result":false,"reason":"source rejection"}
        })),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (_session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
    loop {
        match next(&updates) {
            Update::PostProcessing(request) => request.complete(Ok(options())).unwrap(),
            Update::Commit(request) => request.complete(|rows| Ok(rows.len())).unwrap(),
            Update::GlobalShortcutsRestored => panic!("source false announced as success"),
            Update::Finished(result) => {
                assert_eq!(result, Ok(Completion::Recorded { count: 0 }));
                break;
            }
            _ => {}
        }
    }
    cleanup.try_recv().unwrap();
}

#[test]
fn overflow_cannot_commit_partial_recording_and_shutdown_is_retained() {
    let (mock, trace, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents => Ok(json!({"overflow":true,"events":[]})),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (_session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
    assert!(finished(&updates).unwrap_err().contains("溢出"));
    cleanup.try_recv().unwrap();
    assert!(matches!(
        trace.lock().unwrap().last(),
        Some(ServiceRequest::Shutdown)
    ));
}

#[test]
fn successor_waits_until_the_previous_transport_has_been_released() {
    let (entered, wait_entered) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let (first, _, first_cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMacroRecording => anyhow::bail!("first failed"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let first = first.on_drop(move || {
        entered.send(()).unwrap();
        wait_release.recv().unwrap();
    });
    let (_first, first_updates) = spawn_with(options(), || Ok(first)).unwrap();
    wait_entered.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        LIFECYCLE.try_lock(),
        Err(std::sync::TryLockError::WouldBlock)
    ));
    let (opened, wait_opened) = mpsc::channel();
    let (second, _, second_cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMacroRecording => anyhow::bail!("second failed"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (_second, second_updates) = spawn_with(options(), move || {
        opened.send(()).unwrap();
        Ok(second)
    })
    .unwrap();
    assert!(wait_opened.try_recv().is_err());
    release.send(()).unwrap();
    assert!(
        finished(&first_updates)
            .unwrap_err()
            .contains("first failed")
    );
    first_cleanup.try_recv().unwrap();
    wait_opened.recv_timeout(TIMEOUT).unwrap();
    assert!(
        finished(&second_updates)
            .unwrap_err()
            .contains("second failed")
    );
    second_cleanup.try_recv().unwrap();
}

#[test]
fn focused_capture_waits_for_recording_restoration_and_transport_release() {
    assert!(std::ptr::eq(&LIFECYCLE, &razer_ipc::KEY_CAPTURE_LIFECYCLE));
    let released = Arc::new(AtomicBool::new(false));
    let observed_release = released.clone();
    let (entered, wait_entered) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let (recorder, _, recorder_cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents => Ok(completed_batch()),
        ServiceRequest::ResumeMacroMappings => Ok(restored()),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let recorder = recorder.on_drop(move || {
        entered.send(()).unwrap();
        wait_release.recv_timeout(TIMEOUT).unwrap();
        released.store(true, Ordering::Release);
    });
    let (_recording, updates) = spawn_with(options(), || Ok(recorder)).unwrap();
    assert!(commit(&updates, options()).is_empty());
    wait_entered.recv_timeout(TIMEOUT).unwrap();
    assert!(matches!(
        razer_ipc::KEY_CAPTURE_LIFECYCLE.try_lock(),
        Err(std::sync::TryLockError::WouldBlock)
    ));

    let (capture, _, capture_cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMappingKeyCapture => Ok(
            json!({"capturing":true,"redirected_devices":0,"devices":[],"unresolved_devices":[]}),
        ),
        ServiceRequest::MappingKeyCaptureEvents => anyhow::bail!("capture finished"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let capture = crate::mapping::key_capture::Session::start_with(move || {
        anyhow::ensure!(
            observed_release.load(Ordering::Acquire),
            "focused capture started while recorder could still restore mappings"
        );
        Ok(capture)
    })
    .unwrap();
    release.send(()).unwrap();
    assert!(finished(&updates).is_ok());
    recorder_cleanup.recv_timeout(TIMEOUT).unwrap();
    capture_cleanup.recv_timeout(TIMEOUT).unwrap();
    let replies = capture.drain();
    assert!(matches!(
        replies.first(),
        Some(crate::mapping::key_capture::Reply::Ready(_))
    ));
    assert!(matches!(
        replies.last(),
        Some(crate::mapping::key_capture::Reply::Failed(error)) if error.contains("capture finished")
    ));
}

fn completed_key_batch() -> Value {
    json!({"overflow":false,"events":[
        {"kind":"started","mode":"kSoftware","received_at_ms":0},
        {"kind":"item","time_tick":1,"received_at_ms":10,"event":[{"action":"keydown","vkCode":65,"scanCode":30,"flags":0,"time":1000}]},
        {"kind":"item","time_tick":2,"received_at_ms":20,"event":[{"action":"keyup","vkCode":65,"scanCode":30,"flags":128,"time":1000}]},
        {"kind":"stopped","mode":"kSoftware"}
    ]})
}

#[test]
fn queued_preview_options_are_applied_before_the_first_callback() {
    let (release, wait_release) = mpsc::channel();
    let (mock, _, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents => Ok(json!({"overflow":false,"events":[
            {"kind":"started","mode":"kSoftware","received_at_ms":0},
            {"kind":"item","time_tick":1,"received_at_ms":10,"event":[
                {"action":"keyup","vkCode":231,"scanCode":0,"flags":128,"time":1000}
            ]},
            {"kind":"stopped","mode":"kSoftware"}
        ]})),
        ServiceRequest::ResumeMacroMappings => Ok(restored()),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, updates) = spawn_with(options(), move || {
        wait_release.recv_timeout(TIMEOUT).unwrap();
        Ok(mock)
    })
    .unwrap();
    let mut current = options();
    current.macro_type = MacroType::Sequence;
    current.finalization_macro_type = MacroType::Sequence;
    session
        .send(Command::PreviewOptions(current.clone()))
        .unwrap();
    release.send(()).unwrap();
    let mut committed = false;
    loop {
        match next(&updates) {
            Update::PreviewFailed(error) => panic!("preview used obsolete options: {error}"),
            Update::PostProcessing(request) => request.complete(Ok(current.clone())).unwrap(),
            Update::Commit(request) => request
                .complete(|rows| {
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0].keyboard.as_ref().unwrap().makecode, Some(231));
                    committed = true;
                    Ok(rows.len())
                })
                .unwrap(),
            Update::Finished(result) => {
                assert_eq!(result, Ok(Completion::Recorded { count: 1 }));
                assert!(committed);
                cleanup.try_recv().unwrap();
                break;
            }
            _ => {}
        }
    }
}

#[test]
fn current_postprocessing_options_and_commit_acknowledgement_precede_restore() {
    for kind in [MacroType::Standard, MacroType::Sequence, MacroType::Phased] {
        let (mock, trace, cleanup) = Mock::new(|request| match request {
            ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
            ServiceRequest::MacroRecordingEvents => Ok(completed_key_batch()),
            ServiceRequest::ResumeMacroMappings => Ok(restored()),
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let (_session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
        let mut received = None;
        loop {
            match next(&updates) {
                Update::PostProcessing(request) => {
                    assert!(
                        !trace
                            .lock()
                            .unwrap()
                            .iter()
                            .any(|r| matches!(r, ServiceRequest::ResumeMacroMappings))
                    );
                    let mut current = options();
                    current.macro_type = kind;
                    current.finalization_macro_type = kind;
                    current.phase = (kind == MacroType::Phased).then_some(2);
                    current.next_pair = 99;
                    request.complete(Ok(current)).unwrap();
                }
                Update::Commit(request) => {
                    assert!(
                        !trace
                            .lock()
                            .unwrap()
                            .iter()
                            .any(|r| matches!(r, ServiceRequest::ResumeMacroMappings))
                    );
                    request
                        .complete(|rows| {
                            assert_eq!(rows.len(), if kind == MacroType::Sequence { 1 } else { 2 });
                            assert!(
                                rows.iter()
                                    .all(|row| row.keyboard.as_ref().unwrap().pair_id == Some(99))
                            );
                            if kind == MacroType::Phased {
                                assert!(rows.iter().all(|row| row.phase == Some(2)));
                            }
                            let count = rows.len();
                            received = Some(rows);
                            Ok(count)
                        })
                        .unwrap();
                }
                Update::Finished(result) => {
                    assert_eq!(
                        result,
                        Ok(Completion::Recorded {
                            count: received.unwrap().len()
                        })
                    );
                    cleanup.try_recv().unwrap();
                    assert!(matches!(
                        trace.lock().unwrap().last(),
                        Some(ServiceRequest::Shutdown)
                    ));
                    break;
                }
                _ => {}
            }
        }
    }
}

#[test]
fn rejecting_or_dropping_either_page_consumer_restores_and_releases_the_owner() {
    for (during_commit, dropped) in [(false, false), (false, true), (true, false), (true, true)] {
        let (mock, trace, cleanup) = Mock::new(|request| match request {
            ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
            ServiceRequest::MacroRecordingEvents => Ok(completed_key_batch()),
            ServiceRequest::ResumeMacroMappings => Ok(restored()),
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let (_session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
        loop {
            match next(&updates) {
                Update::PostProcessing(request) if during_commit => {
                    request.complete(Ok(options())).unwrap()
                }
                Update::PostProcessing(request) => {
                    if dropped {
                        drop(request);
                    } else {
                        request
                            .complete(Err("current macro was removed".into()))
                            .unwrap();
                    }
                }
                Update::Commit(request) => {
                    assert!(during_commit);
                    if dropped {
                        drop(request);
                    } else {
                        request
                            .complete(|_| Err("current macro was removed".into()))
                            .unwrap();
                    }
                }
                Update::Finished(result) => {
                    let error = result.unwrap_err();
                    assert!(error.contains(if dropped {
                        "未完成"
                    } else {
                        "current macro was removed"
                    }));
                    cleanup.try_recv().unwrap();
                    assert!(
                        trace
                            .lock()
                            .unwrap()
                            .iter()
                            .any(|r| matches!(r, ServiceRequest::ResumeMacroMappings))
                    );
                    break;
                }
                _ => {}
            }
        }
    }
}

#[test]
fn cancel_while_a_page_reply_is_pending_never_commits_and_releases_the_session() {
    for during_commit in [false, true] {
        let (mock, trace, cleanup) = Mock::new(|request| match request {
            ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
            ServiceRequest::MacroRecordingEvents => Ok(completed_key_batch()),
            ServiceRequest::ResumeMacroMappings => Ok(restored()),
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let (session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
        let mut pending_options = None;
        let mut pending_commit = None;
        loop {
            match next(&updates) {
                Update::PostProcessing(request) if during_commit => {
                    request.complete(Ok(options())).unwrap()
                }
                Update::PostProcessing(request) => {
                    pending_options = Some(request);
                    session.send(Command::Cancel).unwrap();
                }
                Update::Commit(request) => {
                    pending_commit = Some(request);
                    session.send(Command::Cancel).unwrap();
                }
                Update::Finished(result) => {
                    assert_eq!(result, Ok(Completion::Cancelled));
                    cleanup.try_recv().unwrap();
                    assert!(matches!(
                        trace.lock().unwrap().last(),
                        Some(ServiceRequest::Shutdown)
                    ));
                    drop((pending_options, pending_commit));
                    break;
                }
                _ => {}
            }
        }
    }
}

#[test]
fn postprocessing_and_restoration_failures_are_both_retained() {
    let (mock, _, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents => Ok(completed_batch()),
        ServiceRequest::ResumeMacroMappings => anyhow::bail!("restore transport failure"),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let (_session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
    loop {
        match next(&updates) {
            Update::PostProcessing(request) => request
                .complete(Err("invalid current recording options".into()))
                .unwrap(),
            Update::Finished(result) => {
                let error = result.unwrap_err();
                assert!(error.contains("invalid current recording options"));
                assert!(error.contains("restore transport failure"));
                cleanup.try_recv().unwrap();
                break;
            }
            Update::Commit(_) => panic!("failed postprocessing requested a commit"),
            _ => {}
        }
    }
}

fn observed_geometry(tag: u8) -> razer_platform::macro_geometry::Snapshot {
    razer_platform::macro_geometry::Snapshot {
        monitors: json!([{"isPrimaryMonitor":true,"MonitorRect":{"left":0,"top":0,"right":1920,"bottom":1080},"dpiScaleX":1,"dpiScaleY":1,"tag":tag}]),
        screen: json!({"MonitorRect":{"left":0,"top":0,"width":1920,"height":1080}}),
    }
}

#[test]
fn monitor_query_failures_remain_visible_and_only_block_a_consumer_that_requires_geometry() {
    for movement in [0, 1] {
        let (mock, trace, cleanup) = Mock::new(|request| match request {
            ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
            ServiceRequest::MacroRecordingEvents => Ok(completed_key_batch()),
            ServiceRequest::ResumeMacroMappings => Ok(restored()),
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed_calls = calls.clone();
        let (_session, updates) = spawn_with_geometry(
            options(),
            || Ok(mock),
            Registry::default(),
            move || {
                let call = observed_calls.fetch_add(1, Ordering::AcqRel);
                Err(if call == 0 {
                    "before query rejected".into()
                } else {
                    "after query rejected".into()
                })
            },
        )
        .unwrap();
        let mut failures = Vec::new();
        let mut committed = false;
        loop {
            match next(&updates) {
                Update::ObservationFailed(error) => failures.push(error),
                Update::PostProcessing(request) => {
                    let mut current = options();
                    current.movement_mode = movement;
                    request.complete(Ok(current)).unwrap();
                }
                Update::Commit(request) => {
                    assert_eq!(movement, 0);
                    request
                        .complete(|rows| {
                            committed = true;
                            Ok(rows.len())
                        })
                        .unwrap();
                }
                Update::Finished(result) => {
                    assert_eq!(calls.load(Ordering::Acquire), 2);
                    assert_eq!(failures.len(), 2);
                    if movement == 0 {
                        assert!(committed);
                        assert_eq!(result, Ok(Completion::Recorded { count: 2 }));
                    } else {
                        assert!(!committed);
                        let error = result.unwrap_err();
                        assert!(error.contains("before query rejected"));
                        assert!(error.contains("after query rejected"));
                    }
                    assert!(
                        trace
                            .lock()
                            .unwrap()
                            .iter()
                            .any(|request| matches!(request, ServiceRequest::ResumeMacroMappings))
                    );
                    cleanup.try_recv().unwrap();
                    break;
                }
                _ => {}
            }
        }
    }
}

#[test]
fn movement_enabled_by_the_current_consumer_uses_both_original_geometry_observations() {
    let (mock, _, cleanup) = Mock::new(|request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents => Ok(json!({"overflow":false,"events":[
            {"kind":"started","mode":"kSoftware","received_at_ms":0},
            {"kind":"item","time_tick":1,"received_at_ms":10,"event":[{"action":"mousemove","x":20,"y":30,"time":1000}]},
            {"kind":"stopped","mode":"kSoftware"}
        ]})),
        ServiceRequest::ResumeMacroMappings => Ok(restored()),
        ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
        request => panic!("unexpected request: {request:?}"),
    });
    let mut call = 0;
    let (_session, updates) = spawn_with_geometry(
        options(),
        || Ok(mock),
        Registry::default(),
        move || {
            call += 1;
            Ok(observed_geometry(call))
        },
    )
    .unwrap();
    let mut current = options();
    current.movement_mode = 3;
    let rows = commit(&updates, current);
    assert_eq!(rows.len(), 1);
    let movement = rows[0].mouse_movement.as_ref().unwrap();
    let geometry = movement.geometry.as_ref().unwrap();
    assert_eq!(movement.mode, 3);
    assert_eq!(geometry.monitors_before, observed_geometry(1).monitors);
    assert_eq!(geometry.monitors_after, observed_geometry(2).monitors);
    assert!(finished(&updates).is_ok());
    cleanup.try_recv().unwrap();
}

#[test]
fn cancelled_consumers_may_drop_their_reply_without_becoming_a_commit_failure() {
    for during_commit in [false, true] {
        let (mock, _, cleanup) = Mock::new(|request| match request {
            ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
            ServiceRequest::MacroRecordingEvents => Ok(completed_key_batch()),
            ServiceRequest::ResumeMacroMappings => Ok(restored()),
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let (session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
        loop {
            match next(&updates) {
                Update::PostProcessing(request) if during_commit => {
                    request.complete(Ok(options())).unwrap()
                }
                Update::PostProcessing(request) => {
                    session.send(Command::Cancel).unwrap();
                    drop(request);
                }
                Update::Commit(request) => {
                    session.send(Command::Cancel).unwrap();
                    drop(request);
                }
                Update::Finished(result) => {
                    assert_eq!(result, Ok(Completion::Cancelled));
                    cleanup.try_recv().unwrap();
                    break;
                }
                _ => {}
            }
        }
    }
}

#[test]
fn malformed_callback_envelopes_never_reach_recording_or_postprocessing() {
    for batch in [
        json!({"events":[]}),
        json!({"overflow":"false","events":[]}),
        json!({"overflow":false,"events":[{"kind":"started","mode":"unknown"}]}),
        json!({"overflow":false,"events":[{"kind":"stopped"}]}),
        json!({"overflow":false,"events":[{"kind":"error"}]}),
        json!({"overflow":false,"events":[{"kind":"future"}]}),
    ] {
        let (mock, _, cleanup) = Mock::new(move |request| match request {
            ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
            ServiceRequest::MacroRecordingEvents => Ok(batch.clone()),
            ServiceRequest::Shutdown => Ok(json!({"stopped":true})),
            request => panic!("unexpected request: {request:?}"),
        });
        let (_session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
        assert!(matches!(next(&updates), Update::Finished(Err(_))));
        cleanup.try_recv().unwrap();
    }
}

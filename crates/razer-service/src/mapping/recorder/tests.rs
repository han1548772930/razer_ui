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
    loop {
        match next(updates) {
            Update::Finished(result) => return result,
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
fn cancellation_during_shutdown_cannot_commit_recorded_rows() {
    let (entered, wait_entered) = mpsc::channel();
    let (release, wait_release) = mpsc::channel();
    let (mock, _, cleanup) = Mock::new(move |request| match request {
        ServiceRequest::StartMacroRecording => Ok(json!({"accepted":true})),
        ServiceRequest::MacroRecordingEvents => Ok(completed_batch()),
        ServiceRequest::ResumeMacroMappings => Ok(restored()),
        ServiceRequest::Shutdown => {
            entered.send(()).unwrap();
            wait_release.recv().unwrap();
            Ok(json!({"stopped":true}))
        }
        request => panic!("unexpected request: {request:?}"),
    });
    let (session, updates) = spawn_with(options(), || Ok(mock)).unwrap();
    wait_entered.recv_timeout(TIMEOUT).unwrap();
    session.send(Command::Cancel).unwrap();
    release.send(()).unwrap();
    assert!(matches!(finished(&updates), Ok(None)));
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
fn malformed_restoration_cannot_commit_recorded_rows_and_still_shuts_down() {
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
            Update::GlobalShortcutsRestored => panic!("source false announced as success"),
            Update::Finished(result) => {
                assert!(matches!(result, Ok(Some(rows)) if rows.is_empty()));
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

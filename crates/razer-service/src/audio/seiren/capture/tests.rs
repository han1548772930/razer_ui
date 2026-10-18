use super::*;
use std::sync::atomic::Ordering;

#[test]
fn dispatch_unwind_does_not_skip_stop_or_shutdown() {
    let owner = owner();
    let (updates, _) = mpsc::channel();
    let mut calls = Vec::new();
    let error = run_with_dispatch(
        &owner,
        &updates,
        |request| match request {
            ServiceRequest::SeirenAudioCaptureStart { .. } => {
                calls.push("start");
                panic!("simulated IPC dispatch unwind");
            }
            ServiceRequest::SeirenAudioCaptureStop { .. } => {
                calls.push("stop");
                Ok(reply(&owner, true))
            }
            ServiceRequest::Shutdown => {
                calls.push("shutdown");
                Ok(serde_json::json!({"stopped":true}))
            }
            _ => panic!("unwound initialization must not poll"),
        },
        || {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("simulated IPC dispatch unwind"));
    assert_eq!(calls, ["start", "stop", "shutdown"]);
}

fn owner() -> Request {
    Request::new(
        23,
        1422,
        Default::default(),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap()
}
fn reply(owner: &Request, terminal: bool) -> serde_json::Value {
    serde_json::to_value(CapturePoll {
        session_id: owner.owner.session_id,
        product_id: owner.owner.product_id,
        started: !terminal,
        terminal,
        canceled: terminal,
        error: None,
        frame: None,
    })
    .unwrap()
}

#[::core::prelude::v1::test]
fn invalid_start_reply_still_stops_the_original_session_and_shuts_down() {
    let owner = owner();
    let (updates, _) = mpsc::channel();
    let mut calls = Vec::new();
    let result = run_with_dispatch(
        &owner,
        &updates,
        |request| match request {
            ServiceRequest::SeirenAudioCaptureStart {
                session_id,
                product_id,
                options: _,
            } => {
                assert_eq!((session_id, product_id), (23, 1422));
                calls.push("start");
                Ok(serde_json::json!({"session_id":999,"product_id":1422}))
            }
            ServiceRequest::SeirenAudioCaptureStop { session_id } => {
                assert_eq!(session_id, 23);
                calls.push("stop");
                Ok(reply(&owner, true))
            }
            ServiceRequest::Shutdown => {
                calls.push("shutdown");
                Ok(serde_json::json!({"stopped":true}))
            }
            _ => panic!("invalid initialization must not poll"),
        },
        || panic!("invalid initialization must not wait"),
    );
    assert!(result.is_err());
    assert_eq!(calls, ["start", "stop", "shutdown"]);
}

#[::core::prelude::v1::test]
fn cancel_during_initialization_retains_cleanup_errors_and_shutdown() {
    let owner = owner();
    let (updates, _) = mpsc::channel();
    let mut calls = Vec::new();
    let result = run_with_dispatch(
        &owner,
        &updates,
        |request| match request {
            ServiceRequest::SeirenAudioCaptureStart { .. } => {
                calls.push("start");
                owner.owner.canceled.store(true, Ordering::Release);
                Ok(reply(&owner, false))
            }
            ServiceRequest::SeirenAudioCaptureStop { .. } => {
                calls.push("stop");
                let mut response = reply(&owner, true);
                response["error"] = serde_json::json!("COM release failed");
                Ok(response)
            }
            ServiceRequest::Shutdown => {
                calls.push("shutdown");
                Ok(serde_json::json!({"stopped":true}))
            }
            _ => panic!("canceled initialization must not poll"),
        },
        || panic!("canceled initialization must not wait"),
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("COM release failed")
    );
    assert_eq!(calls, ["start", "stop", "shutdown"]);
}

#[::core::prelude::v1::test]
fn terminal_poll_already_acknowledges_worker_cleanup() {
    let owner = owner();
    let (updates, _) = mpsc::channel();
    let mut calls = Vec::new();
    let result = run_with_dispatch(
        &owner,
        &updates,
        |request| match request {
            ServiceRequest::SeirenAudioCaptureStart { .. } => {
                calls.push("start");
                Ok(reply(&owner, false))
            }
            ServiceRequest::SeirenAudioCapturePoll { .. } => {
                calls.push("poll");
                let mut response = reply(&owner, true);
                response["error"] = serde_json::json!("Endpoint invalidated");
                Ok(response)
            }
            ServiceRequest::Shutdown => {
                calls.push("shutdown");
                Ok(serde_json::json!({"stopped":true}))
            }
            _ => panic!("terminal Poll has released its controller owner"),
        },
        || {},
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Endpoint invalidated")
    );
    assert_eq!(calls, ["start", "poll", "shutdown"]);
}

#[::core::prelude::v1::test]
fn foreign_scope_or_samples_before_ready_are_rejected() {
    let owner = owner();
    let mut foreign = reply(&owner, false);
    foreign["session_id"] = serde_json::json!(24);
    assert!(decode(foreign, &owner).is_err());
    let invalid = CapturePoll {
        session_id: 23,
        product_id: 1422,
        started: false,
        terminal: false,
        canceled: false,
        error: None,
        frame: Some(CaptureFrame {
            output_sample_rate: Some(48_000),
            capture_sample_rate: 96_000,
            mono_pcm: vec![0.1],
            microphone_levels: None,
            headphone_levels: None,
            endpoint_id: "mock-endpoint".into(),
            label: "Seiren simulated endpoint".into(),
            default_fallback: false,
            diagnostics: Vec::new(),
        }),
    };
    assert!(decode(serde_json::to_value(invalid).unwrap(), &owner).is_err());
}

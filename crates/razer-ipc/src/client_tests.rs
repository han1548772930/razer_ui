//! In-memory worker transport only. No child, DLL, device or OS service starts.
use super::*;
use std::io::Cursor;

fn reply(id: u64, data: Option<Value>, error: Option<&str>, fatal: bool) -> Vec<u8> {
    let response = ResponseEnvelope {
        id,
        data,
        error: error.map(str::to_owned),
        fatal,
    };
    let mut frame = FRAME_PREFIX.as_bytes().to_vec();
    frame.extend(serde_json::to_vec(&response).unwrap());
    frame.push(b'\n');
    frame
}

fn simulated_client(bytes: Vec<u8>) -> (ServiceClient, Receiver<Vec<u8>>) {
    let (sender, responses) = mpsc::channel();
    let diagnostics = worker_diagnostics::WorkerDiagnostics::default();
    // A small reader forces frames across fill_buf boundaries, just like a
    // pipe with partial writes, while remaining entirely deterministic.
    let _ = forward_responses(
        &mut BufReader::with_capacity(7, Cursor::new(bytes)),
        sender,
        &diagnostics,
    );
    let (requests, pending) = mpsc::channel();
    (
        ServiceClient {
            child: None,
            #[cfg(windows)]
            job: None,
            requests: Some(requests),
            responses,
            input_worker: None,
            output_worker: None,
            diagnostic_worker: None,
            diagnostics,
            timeout: Duration::ZERO,
            next_id: 1,
            stopped: false,
            shutdown_confirmation: None,
            philips_measurement_started: false,
            request_guard: None,
        },
        pending,
    )
}

fn assert_closed(client: &mut ServiceClient, pending: &Receiver<Vec<u8>>) {
    assert!(client.is_stopped());
    assert!(client.requests.is_none());
    assert!(client.child.is_none());
    #[cfg(windows)]
    assert!(client.job.is_none());
    // The initial request was sent, but a stopped connection cannot enqueue
    // another write or consume a later response from the same worker.
    assert!(pending.try_recv().is_ok());
    assert!(matches!(
        pending.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    let error = client
        .request(ServiceRequest::KeyboardLayoutRead)
        .unwrap_err();
    assert!(error.to_string().contains("已停止"));
}

#[test]
fn corrupt_prefixed_response_closes_client_before_later_success() {
    let mut bytes = format!("{FRAME_PREFIX}not-json\n").into_bytes();
    bytes.extend(reply(
        1,
        Some(serde_json::json!({"layout": 1})),
        None,
        false,
    ));
    let (mut client, pending) = simulated_client(bytes);
    let error = client
        .request(ServiceRequest::KeyboardLayoutRead)
        .unwrap_err();
    assert!(format!("{error:#}").contains("响应格式无效"));
    assert_closed(&mut client, &pending);
    assert!(matches!(
        client.responses.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}

#[test]
fn partial_response_preserves_framing_error_and_closes_client() {
    let (mut client, pending) = simulated_client(format!("{FRAME_PREFIX}{{\"id\":1").into_bytes());
    let error = client
        .request(ServiceRequest::KeyboardLayoutRead)
        .unwrap_err();
    assert!(format!("{error:#}").contains("不完整的设备服务数据帧"));
    assert_closed(&mut client, &pending);
}

#[test]
fn oversized_response_preserves_frame_limit_failure() {
    let (mut client, pending) = simulated_client(vec![b'x'; MAX_FRAME_BYTES + 1]);
    let error = client
        .request(ServiceRequest::KeyboardLayoutRead)
        .unwrap_err();
    assert!(format!("{error:#}").contains("设备服务数据帧过大"));
    assert_closed(&mut client, &pending);
}

#[test]
fn legacy_vendor_log_does_not_hide_valid_fragmented_response() {
    let mut bytes = vec![0xff, 0x81, b'\r', b'\n'];
    bytes.extend(reply(
        1,
        Some(serde_json::json!({"layout": 17})),
        None,
        false,
    ));
    let (mut client, _pending) = simulated_client(bytes);
    assert_eq!(
        client.request(ServiceRequest::KeyboardLayoutRead).unwrap(),
        serde_json::json!({"layout": 17})
    );
    assert!(!client.is_stopped());
    let error = client
        .request(ServiceRequest::KeyboardLayoutRead)
        .unwrap_err();
    assert!(error.to_string().contains("输出已关闭"));
    assert!(client.is_stopped());
}

#[test]
fn source_rejection_keeps_connection_available_for_next_request() {
    let mut bytes = reply(1, None, Some("simulated native rejection"), false);
    bytes.extend(reply(
        2,
        Some(serde_json::json!({"layout": 23})),
        None,
        false,
    ));
    let (mut client, _pending) = simulated_client(bytes);
    let error = client
        .request(ServiceRequest::KeyboardLayoutRead)
        .unwrap_err();
    assert_eq!(error.to_string(), "simulated native rejection");
    assert!(!client.is_stopped());
    assert_eq!(
        client.request(ServiceRequest::KeyboardLayoutRead).unwrap(),
        serde_json::json!({"layout": 23})
    );
}

#[test]
fn contradictory_or_missing_response_outcomes_close_the_connection() {
    for (data, error) in [
        (Some(serde_json::json!({"layout": 23})), Some("failed")),
        (None, None),
    ] {
        let (mut client, pending) = simulated_client(reply(1, data, error, false));
        let error = client
            .request(ServiceRequest::KeyboardLayoutRead)
            .unwrap_err();
        assert!(format!("{error:#}").contains("响应状态无效"));
        assert_closed(&mut client, &pending);
    }
}

#[test]
fn fatal_response_can_preserve_a_final_source_observation() {
    let value = serde_json::json!({"samples": [], "diagnostics": ["owner retired"]});
    let (mut client, pending) = simulated_client(reply(1, Some(value.clone()), None, true));
    assert_eq!(
        client.request(ServiceRequest::KeyboardLayoutRead).unwrap(),
        value
    );
    assert_closed(&mut client, &pending);
}

#[test]
fn worker_failure_includes_stderr_written_during_owned_cleanup_and_stdout_diagnostics() {
    let (mut client, pending) = simulated_client(b"source read failed\n".to_vec());
    let (release, wait) = mpsc::channel();
    client.input_worker = Some(thread::spawn(move || {
        release.send(()).unwrap();
        Ok(())
    }));
    let diagnostics = client.diagnostics.clone();
    client.diagnostic_worker = Some(thread::spawn(move || {
        wait.recv().unwrap();
        diagnostics.capture_stderr(&b"restore failed during shutdown"[..])
    }));
    let error = client
        .request(ServiceRequest::KeyboardLayoutRead)
        .unwrap_err();
    let text = format!("{error:#}");
    assert!(text.contains("输出已关闭"));
    assert!(text.contains("stderr:\nrestore failed during shutdown"));
    assert!(text.contains("stdout:\nsource read failed"));
    assert!(client.diagnostic_worker.is_none());
    assert_closed(&mut client, &pending);
}

#[test]
fn diagnostic_worker_failure_does_not_skip_other_pipe_cleanup() {
    let (mut client, _) = simulated_client(Vec::new());
    client.input_worker = Some(thread::spawn(|| anyhow::bail!("input failed")));
    client.output_worker = Some(thread::spawn(|| anyhow::bail!("output failed")));
    client.diagnostic_worker = Some(thread::spawn(|| anyhow::bail!("diagnostic failed")));
    let error = client.stop().unwrap_err();
    let text = format!("{error:#}");
    for expected in ["input failed", "output failed", "diagnostic failed"] {
        assert!(text.contains(expected));
    }
    assert!(client.input_worker.is_none());
    assert!(client.output_worker.is_none());
    assert!(client.diagnostic_worker.is_none());
}

#[test]
fn stale_sequence_closes_connection_without_reusing_later_response() {
    let mut bytes = reply(27, Some(serde_json::json!({"layout": 27})), None, false);
    bytes.extend(reply(
        1,
        Some(serde_json::json!({"layout": 1})),
        None,
        false,
    ));
    let (mut client, pending) = simulated_client(bytes);
    let error = client
        .request(ServiceRequest::KeyboardLayoutRead)
        .unwrap_err();
    assert!(error.to_string().contains("序号不匹配"));
    assert_closed(&mut client, &pending);
}

#[test]
fn fatal_response_and_shutdown_release_the_request_channel() {
    for (request, bytes, expected_error) in [
        (
            ServiceRequest::KeyboardLayoutRead,
            reply(1, None, Some("simulated fatal"), true),
            Some("simulated fatal"),
        ),
        (
            ServiceRequest::Shutdown,
            reply(1, Some(serde_json::json!({"stopped": true})), None, false),
            None,
        ),
    ] {
        let (mut client, pending) = simulated_client(bytes);
        let result = client.request(request);
        match expected_error {
            Some(error) => assert_eq!(result.unwrap_err().to_string(), error),
            None => assert_eq!(result.unwrap(), serde_json::json!({"stopped": true})),
        }
        assert_closed(&mut client, &pending);
    }
}

#[test]
fn receiver_source_factory_is_optional_on_the_existing_wire_contract() {
    let old = serde_json::json!({"operation":"receiver_wireless_status","path":"observed-path","device_container_id":"observed-owner"});
    let request: ServiceRequest = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), old);
    let source = ServiceRequest::ReceiverWirelessStatus {
        path: "observed-path".into(),
        device_container_id: "observed-owner".into(),
        source_product_id: Some(756),
    };
    assert_eq!(
        serde_json::to_value(source).unwrap()["source_product_id"],
        756
    );
    let old = serde_json::json!({"operation":"receiver_pairing_start","operation_id":"operation","path":"observed-path","device_container_id":"observed-owner","category":"MOUSE","action":{"action":"scan"}});
    let request: ServiceRequest = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), old);
}

#[test]
fn cancelled_discovery_guard_does_not_submit_a_request() {
    let (mut client, pending) = simulated_client(Vec::new());
    client.set_request_guard(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        false,
    )));
    let error = client.request(ServiceRequest::HidNodes).unwrap_err();
    assert!(format!("{error:#}").contains("取消"));
    assert!(client.is_stopped());
    assert!(matches!(
        pending.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}

#[test]
fn receiver_cancellation_flag_uses_the_same_admission_and_cleanup_owner() {
    let (mut client, pending) = simulated_client(Vec::new());
    client.set_cancellation_guard(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        true,
    )));
    let error = client
        .request(ServiceRequest::ReceiverBrightnessRead {
            path: "observed-receiver".into(),
            device_container_id: "observed-container".into(),
        })
        .unwrap_err();
    assert!(format!("{error:#}").contains("取消"));
    assert!(client.is_stopped());
    assert!(matches!(
        pending.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
}

#[test]
fn receiver_lighting_attribute_receipt_wire_requires_the_exact_worker_identity() {
    let expected = b"{\"id\":1,\"request\":{\"operation\":\"receiver_lighting_chroma_attribute_result\",\"operation_id\":\"dock-owner\",\"worker_id\":\"00000000-0000-0000-0000-000000000002\",\"request_id\":1,\"receipt\":{\"kind\":\"canceled\"}}}\n";
    let (mut client, pending) = simulated_client(reply(
        1,
        Some(serde_json::json!({"consumed":true})),
        None,
        false,
    ));
    client
        .request(ServiceRequest::ReceiverLightingChromaAttributeResult {
            operation_id: "dock-owner".into(),
            worker_id: uuid::Uuid::from_u128(2),
            request_id: 1,
            receipt: razer_device::receiver_lighting::ChromaAttributeReceipt::Canceled,
        })
        .unwrap();
    assert_eq!(pending.try_recv().unwrap(), expected);
    let missing_worker = br#"{"operation":"receiver_lighting_chroma_attribute_result","operation_id":"dock-owner","request_id":1,"receipt":{"kind":"canceled"}}"#;
    assert!(serde_json::from_slice::<ServiceRequest>(missing_worker).is_err());
}

#[test]
fn receiver_cancellation_interrupts_inflight_request_and_rejects_its_late_receipt() {
    let (mut client, pending) = simulated_client(Vec::new());
    let (response_sender, responses) = mpsc::channel();
    client.responses = responses;
    let canceled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    client.set_cancellation_guard(canceled.clone());
    let worker = std::thread::spawn(move || {
        let frame = pending.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(
            String::from_utf8(frame)
                .unwrap()
                .contains("receiver_brightness_write")
        );
        canceled.store(true, std::sync::atomic::Ordering::Release);
        drop(response_sender);
        assert!(matches!(
            pending.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    });
    let result = client.request(ServiceRequest::ReceiverBrightnessWrite {
        path: "observed-receiver".into(),
        device_container_id: "observed-container".into(),
        percent: 100,
    });
    assert!(result.is_err());
    assert!(client.is_stopped());
    assert!(client.request(ServiceRequest::HidNodes).is_err());
    worker.join().unwrap();

    let (mut client, pending) = simulated_client(reply(
        1,
        Some(serde_json::json!({"source_completed":true})),
        None,
        false,
    ));
    let canceled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    client.set_cancellation_guard(canceled.clone());
    let response = client.responses.recv().unwrap().unwrap();
    canceled.store(true, std::sync::atomic::Ordering::Release);
    let error = client.complete_response(1, false, response).unwrap_err();
    assert!(format!("{error:#}").contains("旧响应未提交"));
    // This stage exercises receipt delivery directly, so it has no outbound
    // request. Cancellation must close the queue without introducing one.
    assert!(client.is_stopped());
    assert!(client.requests.is_none());
    assert!(matches!(
        pending.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    assert!(client.request(ServiceRequest::HidNodes).is_err());
}

#[test]
fn discovery_guard_interrupts_inflight_ipc_and_prevents_later_queries() {
    let (mut client, pending) = simulated_client(Vec::new());
    let (response_sender, responses) = mpsc::channel();
    client.responses = responses;
    client.timeout = Duration::from_secs(5);
    let guard = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    client.set_request_guard(guard.clone());
    let worker = std::thread::spawn(move || {
        let request = pending.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(String::from_utf8(request).unwrap().contains("hid_nodes"));
        guard.store(false, std::sync::atomic::Ordering::Release);
        assert!(matches!(
            pending.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
        drop(response_sender);
    });
    let error = client.request(ServiceRequest::HidNodes).unwrap_err();
    assert!(format!("{error:#}").contains("取消"));
    assert!(client.is_stopped());
    assert!(client.request(ServiceRequest::KeyboardLayoutRead).is_err());
    worker.join().unwrap();
}

#[test]
fn cancellation_after_receipt_does_not_publish_the_old_response() {
    let (mut client, pending) = simulated_client(reply(
        1,
        Some(serde_json::json!({"layout": 17})),
        None,
        false,
    ));
    let guard = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    client.set_request_guard(guard.clone());
    // Model recv_timeout winning the race with cancellation deterministically:
    // the frame was received, but its scope expires before result delivery.
    let response = client.responses.recv().unwrap().unwrap();
    guard.store(false, std::sync::atomic::Ordering::Release);
    let error = client.complete_response(1, false, response).unwrap_err();
    assert!(format!("{error:#}").contains("旧响应未提交"));
    assert!(client.is_stopped());
    assert!(matches!(
        pending.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    assert!(client.request(ServiceRequest::KeyboardLayoutRead).is_err());
}

#[test]
fn shutdown_after_scope_cancellation_preserves_service_result_and_closes() {
    for (data, error) in [
        (Some(serde_json::json!({"stopped": true})), None),
        (None, Some("simulated cleanup failure")),
    ] {
        let (mut client, pending) = simulated_client(reply(1, data.clone(), error, false));
        client.set_request_guard(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
            false,
        )));
        let result = client.request(ServiceRequest::Shutdown);
        match error {
            Some(error) => assert_eq!(result.unwrap_err().to_string(), error),
            None => assert_eq!(result.unwrap(), data.unwrap()),
        }
        assert_closed(&mut client, &pending);
    }
}

#[test]
fn shutdown_rejects_missing_negative_and_wrongly_typed_cleanup_receipts() {
    for data in [
        serde_json::json!({"stopped": false}),
        serde_json::json!({"stopped": "true"}),
        serde_json::json!({"other": true}),
    ] {
        let (mut client, pending) = simulated_client(reply(1, Some(data), None, false));
        assert!(client.request(ServiceRequest::Shutdown).is_err());
        assert!(client.shutdown_confirmation().is_none());
        client.finish_termination().unwrap();
        assert!(client.shutdown_confirmation().is_none());
        assert_closed(&mut client, &pending);
    }
}

#[test]
fn shutdown_preserves_cleanup_extension_fields() {
    let receipt = serde_json::json!({
        "stopped": true,
        "mouse_tracking_cleanup": {"acknowledged": false, "diagnostic": "preserved"}
    });
    let (mut client, pending) = simulated_client(reply(1, Some(receipt.clone()), None, false));
    assert_eq!(client.request(ServiceRequest::Shutdown).unwrap(), receipt);
    assert_eq!(client.shutdown_confirmation().unwrap().request_id(), 1);
    assert_closed(&mut client, &pending);
}

#[test]
fn shutdown_joins_both_pipe_workers_and_reports_all_cleanup_failures() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (mut client, pending) = simulated_client(reply(
        1,
        Some(serde_json::json!({"stopped": true})),
        None,
        false,
    ));
    let input_finished = Arc::new(AtomicBool::new(false));
    let input_result = input_finished.clone();
    client.input_worker = Some(thread::spawn(move || {
        input_result.store(true, Ordering::Release);
        anyhow::bail!("simulated input flush failure")
    }));
    let output_finished = Arc::new(AtomicBool::new(false));
    let output_result = output_finished.clone();
    client.output_worker = Some(thread::spawn(move || {
        output_result.store(true, Ordering::Release);
        anyhow::bail!("simulated output read failure")
    }));
    let error = client.request(ServiceRequest::Shutdown).unwrap_err();
    let text = format!("{error:#}");
    assert!(text.contains("simulated input flush failure"));
    assert!(text.contains("simulated output read failure"));
    let confirmation = client.shutdown_confirmation().unwrap();
    assert_eq!(confirmation.request_id(), 1);
    assert!(input_finished.load(Ordering::Acquire));
    assert!(output_finished.load(Ordering::Acquire));
    assert!(client.input_worker.is_none());
    assert!(client.output_worker.is_none());
    client.finish_termination().unwrap();
    assert_eq!(client.shutdown_confirmation(), Some(confirmation));
    assert_closed(&mut client, &pending);
}

#[test]
fn service_rejection_and_wrong_reply_cannot_become_shutdown_confirmation_after_process_release() {
    for bytes in [
        reply(1, None, Some("native owner restoration failed"), false),
        reply(2, Some(serde_json::json!({"stopped":true})), None, false),
        reply(
            1,
            Some(serde_json::json!({"stopped":true})),
            Some("contradictory"),
            false,
        ),
        reply(1, None, None, false),
    ] {
        let (mut client, pending) = simulated_client(bytes);
        assert!(client.shutdown_confirmation().is_none());
        assert!(client.request(ServiceRequest::Shutdown).is_err());
        assert!(client.shutdown_confirmation().is_none());
        client.finish_termination().unwrap();
        assert!(client.is_stopped());
        assert!(client.shutdown_confirmation().is_none());
        assert_closed(&mut client, &pending);
    }
}

#[test]
fn pipe_panic_does_not_skip_the_other_owned_join() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (mut client, _) = simulated_client(Vec::new());
    client.input_worker = Some(thread::spawn(|| panic!("simulated pipe panic")));
    let output_finished = Arc::new(AtomicBool::new(false));
    let output_result = output_finished.clone();
    client.output_worker = Some(thread::spawn(move || {
        output_result.store(true, Ordering::Release);
        Ok(())
    }));
    assert!(client.stop().unwrap_err().to_string().contains("输入线程"));
    assert!(output_finished.load(Ordering::Acquire));
    assert!(client.input_worker.is_none());
    assert!(client.output_worker.is_none());
}

#[test]
fn drop_reaps_pipe_workers_after_request_admission_has_already_stopped() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (mut client, pending) = simulated_client(Vec::new());
    client.stopped = true;
    let input_closed = Arc::new(AtomicBool::new(false));
    let input_result = input_closed.clone();
    client.input_worker = Some(thread::spawn(move || {
        assert!(matches!(pending.recv(), Err(mpsc::RecvError)));
        input_result.store(true, Ordering::Release);
        Ok(())
    }));
    let output_closed = Arc::new(AtomicBool::new(false));
    let output_result = output_closed.clone();
    client.output_worker = Some(thread::spawn(move || {
        output_result.store(true, Ordering::Release);
        Ok(())
    }));
    drop(client);
    assert!(input_closed.load(Ordering::Acquire));
    assert!(output_closed.load(Ordering::Acquire));
}

#[test]
fn input_worker_preserves_frame_order_and_flush_errors() {
    struct FlushFailure;
    impl Write for FlushFailure {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("simulated flush failure"))
        }
    }
    let (requests, pending) = mpsc::channel();
    requests.send(b"first\n".to_vec()).unwrap();
    requests.send(b"second\n".to_vec()).unwrap();
    drop(requests);
    let mut written = Vec::new();
    forward_requests(&mut written, pending).unwrap();
    assert_eq!(written, b"first\nsecond\n");

    let (requests, pending) = mpsc::channel();
    requests.send(b"request\n".to_vec()).unwrap();
    drop(requests);
    let error = forward_requests(FlushFailure, pending).unwrap_err();
    assert!(format!("{error:#}").contains("simulated flush failure"));
}

#[test]
fn finish_termination_retries_stopped_cleanup_without_a_native_shutdown_receipt() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (mut client, pending) = simulated_client(Vec::new());
    client.stopped = true;
    let input_finished = Arc::new(AtomicBool::new(false));
    let input_result = input_finished.clone();
    client.input_worker = Some(thread::spawn(move || {
        assert!(matches!(pending.recv(), Err(mpsc::RecvError)));
        input_result.store(true, Ordering::Release);
        anyhow::bail!("simulated termination input failure")
    }));
    let output_finished = Arc::new(AtomicBool::new(false));
    let output_result = output_finished.clone();
    client.output_worker = Some(thread::spawn(move || {
        output_result.store(true, Ordering::Release);
        anyhow::bail!("simulated termination output failure")
    }));

    let error = client.finish_termination().unwrap_err();
    let text = format!("{error:#}");
    assert!(text.contains("simulated termination input failure"));
    assert!(text.contains("simulated termination output failure"));
    assert!(input_finished.load(Ordering::Acquire));
    assert!(output_finished.load(Ordering::Acquire));
    assert!(client.requests.is_none());
    assert!(client.input_worker.is_none());
    assert!(client.output_worker.is_none());
    assert!(client.is_stopped());
    assert!(client.shutdown_confirmation().is_none());

    client.finish_termination().unwrap();
    client.finish_termination().unwrap();
    assert!(client.request(ServiceRequest::Shutdown).is_err());
    assert!(client.shutdown_confirmation().is_none());
}

#[test]
fn output_worker_distinguishes_clean_eof_from_corrupt_frames() {
    let (send, receive) = mpsc::channel();
    forward_responses(&mut Cursor::new(Vec::new()), send, &Default::default()).unwrap();
    let Err(error) = receive.recv().unwrap() else {
        panic!("clean EOF must still wake a pending request")
    };
    assert!(error.to_string().contains("输出已关闭"));

    let (send, receive) = mpsc::channel();
    let error = forward_responses(
        &mut Cursor::new(format!("{FRAME_PREFIX}not-json\n")),
        send,
        &Default::default(),
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("响应格式无效"));
    assert!(receive.recv().unwrap().is_err());
}

struct SimulatedChild {
    events: Vec<&'static str>,
    observations: std::collections::VecDeque<std::io::Result<bool>>,
    killed: std::io::Result<()>,
    waited: std::io::Result<()>,
}

impl ChildProcess for SimulatedChild {
    fn exited(&mut self) -> std::io::Result<bool> {
        self.events.push("observe");
        self.observations.pop_front().unwrap_or(Ok(false))
    }
    fn terminate(&mut self) -> std::io::Result<()> {
        self.events.push("terminate");
        std::mem::replace(&mut self.killed, Ok(()))
    }
    fn wait_for_exit(&mut self) -> std::io::Result<()> {
        self.events.push("wait");
        std::mem::replace(&mut self.waited, Ok(()))
    }
}

#[test]
fn child_exit_is_reaped_before_cleanup_returns() {
    for observed in [false, true] {
        let mut child = SimulatedChild {
            events: Vec::new(),
            observations: [Ok(observed)].into(),
            killed: Ok(()),
            waited: Ok(()),
        };
        terminate_child(&mut child).unwrap();
        assert_eq!(
            child.events,
            if observed {
                vec!["observe"]
            } else {
                vec!["observe", "terminate", "wait"]
            }
        );
    }
}

#[test]
fn failed_kill_requires_an_independent_reaped_exit_observation() {
    let mut child = SimulatedChild {
        events: Vec::new(),
        observations: [Ok(false), Ok(true)].into(),
        killed: Err(std::io::Error::other("simulated exit race")),
        waited: Ok(()),
    };
    terminate_child(&mut child).unwrap();
    assert_eq!(child.events, ["observe", "terminate", "observe"]);
}

#[test]
fn failed_kill_preserves_all_status_errors_without_waiting_on_a_live_child() {
    let mut child = SimulatedChild {
        events: Vec::new(),
        observations: [
            Err(std::io::Error::other("simulated status failure")),
            Err(std::io::Error::other("simulated second status failure")),
        ]
        .into(),
        killed: Err(std::io::Error::other("simulated kill failure")),
        waited: Err(std::io::Error::other("simulated wait failure")),
    };
    let error = terminate_child(&mut child).unwrap_err();
    let text = format!("{error:#}");
    assert!(text.contains("simulated status failure"));
    assert!(text.contains("simulated kill failure"));
    assert!(text.contains("simulated second status failure"));
    assert_eq!(child.events, ["observe", "terminate", "observe"]);
}

#[test]
fn unresolved_child_stays_owned_until_a_later_cleanup_establishes_exit() {
    let mut child = Some(SimulatedChild {
        events: Vec::new(),
        observations: [Ok(false)].into(),
        killed: Ok(()),
        waited: Err(std::io::Error::other("simulated wait failure")),
    });
    let error = reap_child(&mut child).unwrap_err();
    assert!(format!("{error:#}").contains("simulated wait failure"));
    assert!(child.is_some());
    assert_eq!(
        child.as_ref().unwrap().events,
        ["observe", "terminate", "wait"]
    );
    reap_child(&mut child).unwrap();
    assert!(child.is_none());
}

#[test]
fn child_kill_acknowledgement_does_not_replace_wait_completion() {
    struct WaitingChild {
        entered: Sender<()>,
        release: Receiver<()>,
        finished: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }
    impl ChildProcess for WaitingChild {
        fn exited(&mut self) -> std::io::Result<bool> {
            Ok(false)
        }
        fn terminate(&mut self) -> std::io::Result<()> {
            Ok(())
        }
        fn wait_for_exit(&mut self) -> std::io::Result<()> {
            self.entered.send(()).unwrap();
            self.release.recv().unwrap();
            self.finished
                .store(true, std::sync::atomic::Ordering::Release);
            Ok(())
        }
    }
    let (entered, waiting) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let child_finished = finished.clone();
    let owner = thread::spawn(move || {
        terminate_child(&mut WaitingChild {
            entered,
            release: released,
            finished: child_finished.clone(),
        })
        .unwrap();
        assert!(child_finished.load(std::sync::atomic::Ordering::Acquire));
    });
    waiting.recv().unwrap();
    assert!(!finished.load(std::sync::atomic::Ordering::Acquire));
    release.send(()).unwrap();
    owner.join().unwrap();
    assert!(finished.load(std::sync::atomic::Ordering::Acquire));
}

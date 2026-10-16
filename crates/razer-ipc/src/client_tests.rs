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
    // A small reader forces frames across fill_buf boundaries, just like a
    // pipe with partial writes, while remaining entirely deterministic.
    forward_responses(&mut BufReader::with_capacity(7, Cursor::new(bytes)), sender);
    let (requests, pending) = mpsc::channel();
    (
        ServiceClient {
            child: None,
            #[cfg(windows)]
            job: None,
            requests: Some(requests),
            responses,
            timeout: Duration::ZERO,
            next_id: 1,
            stopped: false,
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

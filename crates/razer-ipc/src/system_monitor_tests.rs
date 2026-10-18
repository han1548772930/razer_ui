//! Fixed wire contracts without spawning a worker or invoking a native library.
use super::*;
use serde_json::json;

#[test]
fn system_monitor_start_preserves_session_and_physical_container_identity() {
    let expected = br#"{"operation":"system_monitor_start","owner_guid":"session","container_id":"physical-device","product_id":3893}"#;
    let request = ServiceRequest::SystemMonitorStart {
        owner_guid: "session".into(),
        container_id: "physical-device".into(),
        product_id: 3893,
    };
    assert_eq!(serde_json::to_vec(&request).unwrap(), expected);
    let decoded: ServiceRequest = serde_json::from_slice(expected).unwrap();
    assert!(
        matches!(decoded, ServiceRequest::SystemMonitorStart { owner_guid, container_id, product_id:3893 } if owner_guid == "session" && container_id == "physical-device")
    );
    for frame in [
        json!({"operation":"system_monitor_start","owner_guid":"session","product_id":3893}),
        json!({"operation":"system_monitor_start","container_id":"physical-device","product_id":3893}),
    ] {
        assert!(serde_json::from_value::<ServiceRequest>(frame).is_err());
    }
}

#[test]
fn system_monitor_poll_and_stop_require_their_owner() {
    for (request, expected) in [
        (
            ServiceRequest::SystemMonitorPoll {
                owner_guid: "session".into(),
            },
            json!({"operation":"system_monitor_poll","owner_guid":"session"}),
        ),
        (
            ServiceRequest::SystemMonitorStop {
                owner_guid: "session".into(),
            },
            json!({"operation":"system_monitor_stop","owner_guid":"session"}),
        ),
    ] {
        assert_eq!(serde_json::to_value(request).unwrap(), expected);
        assert!(serde_json::from_value::<ServiceRequest>(expected).is_ok());
    }
    assert!(
        serde_json::from_value::<ServiceRequest>(json!({"operation":"system_monitor_stop"}))
            .is_err()
    );
}

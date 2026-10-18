use super::*;
use crate::ServiceRequest;
use serde_json::json;

#[test]
fn mouse_input_monitoring_wire_preserves_owner_and_requires_identity() {
    let request = ServiceRequest::MouseInputMonitoringStart {
        owner_guid: "session-1".into(),
    };
    let expected = br#"{"operation":"mouse_input_monitoring_start","owner_guid":"session-1"}"#;
    assert_eq!(serde_json::to_vec(&request).unwrap(), expected);
    assert!(matches!(
        serde_json::from_slice::<ServiceRequest>(expected).unwrap(),
        ServiceRequest::MouseInputMonitoringStart { owner_guid } if owner_guid == "session-1"
    ));
    for operation in [
        "mouse_input_monitoring_start",
        "mouse_input_monitoring_poll",
        "mouse_input_monitoring_stop",
    ] {
        assert!(serde_json::from_value::<ServiceRequest>(json!({"operation":operation})).is_err());
    }
}

#[test]
fn mouse_input_monitoring_native_batch_keeps_nested_source_json() {
    let rows = r#"[{"timeTick":"88","data":"{\"type\":\"mouseMove\",\"x\":17,\"y\":-2}"}]"#;
    let batch = InputEvent::from_native(
        r#"{"containerId":"physical","productId":191,"vendorId":5426,"usbInstanceId":"usb-1"}"#,
        7,
        rows.into(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&batch).unwrap(),
        json!({"containerId":"physical","productId":191,"vendorId":5426,
            "usbInstanceId":"usb-1","inputMonitoringEventType":7,"jsonEvent":rows})
    );
    let movement =
        razer_device::dpi_matcher::InputBatch::from_callback(&serde_json::to_value(batch).unwrap())
            .unwrap();
    assert_eq!(movement.movement[0].x, 17.);
    assert_eq!(movement.movement[0].time_tick, 88.);
}

#[test]
fn mouse_input_monitoring_rejects_missing_identity_oversize_and_stale_receipts() {
    for identity in [
        r#"{"containerId":"x","productId":190}"#,
        r#"{"containerId":"","productId":190,"vendorId":5426}"#,
        r#"{"containerId":"x","productId":"190","vendorId":5426}"#,
    ] {
        assert!(InputEvent::from_native(identity, 1, "[]".into()).is_err());
    }
    assert!(
        InputEvent::from_native(
            r#"{"containerId":"x","productId":190,"vendorId":5426}"#,
            1,
            " ".repeat(MAX_CALLBACK_BYTES)
        )
        .is_err()
    );
    assert!(
        Receipt::new("old".into(), true)
            .validate("new", true)
            .is_err()
    );
    assert!(
        Receipt::new("same".into(), true)
            .validate("same", false)
            .is_err()
    );
    assert!(
        Poll::new("old".into(), vec![], None)
            .into_events("new")
            .is_err()
    );
    assert!(
        Poll::new("same".into(), vec![], Some("overflow".into()))
            .into_events("same")
            .is_err()
    );
}

#[test]
fn mouse_input_monitoring_poll_revalidates_deserialized_events_and_batch_bounds() {
    let valid = json!({"containerId":"physical","productId":190,"vendorId":5426,
        "usbInstanceId":null,"inputMonitoringEventType":1,"jsonEvent":"[]"});
    for invalid in [
        {
            let mut event = valid.clone();
            event["containerId"] = json!("");
            event
        },
        {
            let mut event = valid.clone();
            event["vendorId"] = json!(0);
            event
        },
        {
            let mut event = valid.clone();
            event["jsonEvent"] = json!(" ".repeat(MAX_CALLBACK_BYTES));
            event
        },
    ] {
        let poll: Poll =
            serde_json::from_value(json!({"owner_guid":"same","events":[invalid]})).unwrap();
        assert!(poll.into_events("same").is_err());
    }
    let event: InputEvent = serde_json::from_value(valid).unwrap();
    assert!(
        Poll::new("same".into(), vec![event.clone(); 129], None)
            .into_events("same")
            .is_err()
    );
    assert_eq!(
        Poll::new("same".into(), vec![event; 128], None)
            .into_events("same")
            .unwrap()
            .len(),
        128
    );
}

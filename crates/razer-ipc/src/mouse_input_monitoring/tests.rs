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
        StartAcknowledgement::started("old".into())
            .validate("new")
            .is_err()
    );
    assert_eq!(
        StartAcknowledgement::rejected("same".into(), "already started")
            .validate("same")
            .unwrap(),
        StartStatus::AlreadyStarted
    );
    assert!(
        StartAcknowledgement::rejected("same".into(), "")
            .validate("same")
            .is_err()
    );
    assert!(
        StartAcknowledgement::started("same".into())
            .validate("same")
            .is_ok()
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
    let large = InputEvent::from_native(
        r#"{"containerId":"physical","productId":190,"vendorId":5426}"#,
        1,
        format!("[{}]", " ".repeat(900_000)),
    )
    .unwrap();
    assert!(
        Poll::new("same".into(), vec![large; 4], None)
            .into_events("same")
            .is_err()
    );
}

fn final_callback(tick: u64) -> serde_json::Value {
    json!({
        "containerId":"physical", "productId":226, "vendorId":5426,
        "usbInstanceId":"usb-1", "inputMonitoringEventType":1,
        "jsonEvent":format!(r#"[{{"timeTick":"{tick}","data":"{{\"type\":\"mouseMove\",\"x\":3,\"y\":4}}"}}]"#)
    })
}

fn started_session() -> Session {
    let mut session = Session::new("current".into()).unwrap();
    session
        .start(|_| Ok(json!({"owner_guid":"current","result":true,"reason":""})))
        .unwrap();
    session
}

#[test]
fn mouse_input_monitoring_negative_start_ack_is_visible_and_keeps_stop_obligation() {
    let mut session = Session::new("current".into()).unwrap();
    let error = session
        .start(|_| {
            Ok(json!({
                "owner_guid":"current",
                "result":false,
                "reason":"already started"
            }))
        })
        .unwrap_err();
    assert!(error.to_string().contains("already started"));
    assert!(session.possibly_started());
    assert!(
        session
            .repeat_start(|_| panic!("A rejected first START cannot admit repeat requests"))
            .is_err()
    );
    session
        .stop(|_| {
            Ok(json!({
                "owner_guid":"current",
                "monitoring":false,
                "events":[],
                "more":false
            }))
        })
        .unwrap();
    assert!(!session.possibly_started());
}

#[test]
fn mouse_input_monitoring_repeated_start_crosses_ipc_and_preserves_negative_ack() {
    let mut session = started_session();
    let acknowledgement = session
        .repeat_start(|request| {
            assert_eq!(
                serde_json::to_vec(&request)?,
                br#"{"operation":"mouse_input_monitoring_start","owner_guid":"current"}"#
            );
            Ok(json!({"owner_guid":"current","result":false,"reason":"already started"}))
        })
        .unwrap();
    assert!(!acknowledgement.result());
    assert_eq!(acknowledgement.reason(), "already started");
    assert!(session.possibly_started());
    assert!(
        session
            .start(|_| panic!("Repeat does not clear the capture owner"))
            .is_err()
    );
    assert_eq!(
        session
            .poll(|_| Ok(json!({"owner_guid":"current","events":[final_callback(88)]})))
            .unwrap()
            .len(),
        1
    );
    session
        .stop(|_| Ok(json!({"owner_guid":"current","monitoring":false,"events":[],"more":false})))
        .unwrap();
    assert!(
        session
            .repeat_start(|_| panic!("Stopped capture cannot repeat START"))
            .is_err()
    );
}

#[test]
fn mouse_input_monitoring_repeat_rejects_stale_malformed_and_transport_failures() {
    let mut session = started_session();
    for response in [
        json!({"owner_guid":"old","result":false,"reason":"already started"}),
        json!({"owner_guid":"current","result":false}),
        json!({"owner_guid":"current","result":false,"reason":""}),
        json!({"owner_guid":"current","result":true,"reason":"already started"}),
        json!({"owner_guid":"current","result":false,"reason":"invalid thread"}),
    ] {
        assert!(session.repeat_start(|_| Ok(response.clone())).is_err());
        assert!(session.possibly_started());
    }
    assert!(
        session
            .repeat_start(|_| anyhow::bail!("lost repeat reply"))
            .is_err()
    );
    assert!(session.possibly_started());
    assert!(session.stop(|_| anyhow::bail!("lost STOP reply")).is_err());
    assert!(
        session
            .repeat_start(|_| panic!("STOP has already begun"))
            .is_err()
    );
    assert!(
        session
            .poll(|_| panic!("A draining capture cannot poll"))
            .is_err()
    );
    session
        .stop(|_| Ok(json!({"owner_guid":"current","monitoring":false,"events":[],"more":false})))
        .unwrap();
    session
        .start(|_| Ok(json!({"owner_guid":"current","result":true,"reason":""})))
        .unwrap();
}

#[test]
fn mouse_input_monitoring_lost_initial_start_ack_cannot_admit_repeat() {
    let mut session = Session::new("current".into()).unwrap();
    assert!(
        session
            .start(|_| anyhow::bail!("lost START reply"))
            .is_err()
    );
    assert!(
        session
            .repeat_start(|_| panic!("Partial START must first finish STOP"))
            .is_err()
    );
    assert!(session.possibly_started());
}

#[test]
fn mouse_input_monitoring_stop_wire_carries_final_callbacks_and_required_continuation() {
    let event: InputEvent = serde_json::from_value(final_callback(88)).unwrap();
    let encoded =
        serde_json::to_value(StopReceipt::new("current".into(), vec![event], true, None)).unwrap();
    assert_eq!(
        encoded,
        json!({"owner_guid":"current","monitoring":false,
            "events":[final_callback(88)],"more":true,"failure":null})
    );
    for incomplete in [
        json!({"owner_guid":"current","monitoring":false}),
        json!({"owner_guid":"current","monitoring":false,"events":[]}),
        json!({"owner_guid":"current","monitoring":false,"more":false}),
    ] {
        assert!(serde_json::from_value::<StopReceipt>(incomplete).is_err());
    }
}

#[test]
fn mouse_input_monitoring_stop_delivers_all_pages_in_order_before_releasing_its_owner() {
    let mut session = started_session();
    let mut pages = std::collections::VecDeque::from([
        json!({"owner_guid":"current","monitoring":false,
            "events":[final_callback(88), final_callback(89)],"more":true}),
        json!({"owner_guid":"current","monitoring":false,
            "events":[final_callback(90)],"more":false}),
    ]);
    let mut callbacks = Vec::new();
    session
        .stop_with(
            |request| {
                assert!(matches!(request,
                    ServiceRequest::MouseInputMonitoringStop { owner_guid } if owner_guid == "current"));
                Ok(pages.pop_front().unwrap())
            },
            |events| {
                callbacks.extend(events);
                Ok(())
            },
        )
        .unwrap();
    assert!(pages.is_empty());
    assert!(!session.possibly_started());
    assert_eq!(
        serde_json::to_value(callbacks).unwrap(),
        json!([final_callback(88), final_callback(89), final_callback(90)])
    );
    session
        .stop(|_| panic!("a fully drained owner must not send another STOP"))
        .unwrap();
}

#[test]
fn mouse_input_monitoring_stop_rejects_wrong_acknowledgements_and_empty_continuations() {
    for invalid in [
        json!({"owner_guid":"old","monitoring":false,"events":[],"more":false}),
        json!({"owner_guid":"current","monitoring":true,"events":[],"more":false}),
        json!({"owner_guid":"current","monitoring":false,"events":[],"more":true}),
        json!({"owner_guid":"current","monitoring":false,"events":[]}),
    ] {
        let mut session = started_session();
        assert!(
            session
                .stop_with(|_| Ok(invalid.clone()), |_| panic!("unaccepted callback"))
                .is_err()
        );
        assert!(session.possibly_started());
        session
            .stop(|_| {
                Ok(json!({"owner_guid":"current","monitoring":false,"events":[],"more":false}))
            })
            .unwrap();
        assert!(!session.possibly_started());
    }
}

#[test]
fn mouse_input_monitoring_stop_drains_after_observer_failure_and_preserves_transport_failure() {
    for lose_last_receipt in [false, true] {
        let mut session = started_session();
        let mut pages = 0;
        let mut observations = 0;
        let error = session
            .stop_with(
                |_| {
                    pages += 1;
                    if pages == 3 && lose_last_receipt {
                        anyhow::bail!("lost final STOP receipt");
                    }
                    Ok(json!({"owner_guid":"current","monitoring":false,
                        "events":[final_callback(pages)],"more":pages < 3}))
                },
                |_| {
                    observations += 1;
                    anyhow::bail!("observer rejected the callback");
                },
            )
            .unwrap_err();
        assert_eq!(pages, 3);
        assert_eq!(observations, 1);
        assert!(error.to_string().contains("observer rejected the callback"));
        assert_eq!(session.possibly_started(), lose_last_receipt);
        assert_eq!(
            error.to_string().contains("lost final STOP receipt"),
            lose_last_receipt
        );
    }
}

#[test]
fn mouse_input_monitoring_stop_revalidates_final_events_without_abandoning_cleanup() {
    let valid = final_callback(88);
    for invalid_events in [
        {
            let mut event = valid.clone();
            event["vendorId"] = json!(0);
            json!([event])
        },
        json!(vec![valid.clone(); 129]),
        {
            let mut event = valid.clone();
            event["jsonEvent"] = json!(" ".repeat(MAX_CALLBACK_BYTES));
            json!([event])
        },
    ] {
        let mut session = started_session();
        let mut requests = 0;
        assert!(
            session
                .stop_with(
                    |_| {
                        requests += 1;
                        Ok(if requests == 1 {
                            json!({"owner_guid":"current","monitoring":false,
                                "events":invalid_events,"more":true})
                        } else {
                            json!({"owner_guid":"current","monitoring":false,
                                "events":[valid],"more":false})
                        })
                    },
                    |_| panic!("a failed stream cannot publish later pages as complete"),
                )
                .is_err()
        );
        assert_eq!(requests, 2);
        assert!(!session.possibly_started());
    }
}

#[test]
fn mouse_input_monitoring_stop_keeps_a_final_capture_failure_distinct_from_native_stop() {
    let mut session = started_session();
    let mut requests = 0;
    let failure = session
        .stop_with(
            |_| {
                requests += 1;
                Ok(json!({"owner_guid":"current","monitoring":false,
                    "events":[final_callback(requests)],"more":requests == 1,
                    "failure":"final device identity failed"}))
            },
            |_| panic!("the callback failure must stay visible"),
        )
        .unwrap_err();
    assert_eq!(failure.to_string(), "final device identity failed");
    assert_eq!(requests, 2);
    assert!(!session.possibly_started());
}

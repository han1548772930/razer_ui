use super::*;
fn identity() -> identity::Identity {
    identity::Identity {
        container_id: "{observed-container}".into(),
        vendor_id: 5426,
        product_id: 70,
        usb_instance_id: Some("USB\\actual".into()),
    }
}
fn sample(path: &str, tick: u64, x: i32, y: i32, flags: u16) -> Sample {
    Sample {
        path: path.into(),
        x,
        y,
        button_flags: flags,
        button_data: 0,
        modifiers: 0x200,
        time_tick: tick,
    }
}
fn rows(event: &InputEvent) -> Vec<Value> {
    serde_json::from_str(event.json_event()).unwrap()
}
#[test]
fn source_flush_is_strictly_after_100000_microseconds_and_button_flushes_movement_first() {
    let mut state = State::default();
    state
        .sample(sample("HID/a", 1000, 3, 4, 0), |_| Ok(identity()))
        .unwrap();
    state
        .sample(sample("HID/a", 101000, 5, 6, 0), |_| {
            panic!("identity must be retained")
        })
        .unwrap();
    assert!(state.pending.is_empty());
    state
        .sample(sample("HID/a", 101001, -1, -2, 0), |_| unreachable!())
        .unwrap();
    let event = state.pending.pop_front().unwrap();
    let values = rows(&event);
    assert_eq!(values.len(), 3);
    assert_eq!(values[0]["timeTick"], "1000");
    assert_eq!(
        serde_json::from_str::<Value>(values[2]["data"].as_str().unwrap()).unwrap(),
        json!({"type":"mouseMove","x":-1,"y":-2})
    );
    state
        .sample(sample("HID/a", 101002, 7, 8, 1), |_| unreachable!())
        .unwrap();
    let values = rows(&state.pending.pop_front().unwrap());
    assert_eq!(values.len(), 2);
    assert_eq!(
        serde_json::from_str::<Value>(values[0]["data"].as_str().unwrap()).unwrap()["type"],
        "mouseMove"
    );
    assert_eq!(
        serde_json::from_str::<Value>(values[1]["data"].as_str().unwrap()).unwrap(),
        json!({"type":"mouse","id":1,"data":0,"extraInformation":0,"modifiers":512})
    );
}
#[test]
fn device_removal_flushes_only_that_identity_and_reconnect_observes_a_new_identity() {
    let mut state = State::default();
    state
        .sample(sample("HID/a", 1, 1, 0, 0), |_| Ok(identity()))
        .unwrap();
    state
        .sample(sample("HID/b", 2, 2, 0, 0), |_| {
            let mut id = identity();
            id.product_id = 71;
            Ok(id)
        })
        .unwrap();
    state.removed("hid/A").unwrap();
    assert_eq!(state.pending.len(), 1);
    assert_eq!(state.pending[0].product_id(), 70);
    assert!(state.devices.contains_key("hid/b"));
    state
        .sample(sample("HID/a", 3, 3, 0, 1), |_| {
            let mut id = identity();
            id.product_id = 72;
            Ok(id)
        })
        .unwrap();
    assert_eq!(state.pending[1].product_id(), 72);
}
#[test]
fn failed_identity_does_not_create_an_empty_or_default_device_batch() {
    let mut state = State::default();
    assert!(
        state
            .sample(sample("HID/a", 1, 2, 3, 0), |_| anyhow::bail!(
                "identity changed"
            ))
            .is_err()
    );
    assert!(state.devices.is_empty());
    assert!(state.pending.is_empty());
}

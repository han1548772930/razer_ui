use super::*;
fn request(command: &'static str, payload: Value) -> MouseDynamicRequested {
    MouseDynamicRequested {
        request_id: 1,
        scope: MouseDynamicScope {
            owner: EntityId::from(1),
            profile_epoch: 2,
            connection_epoch: 3,
        },
        product_id: 190,
        command,
        payload,
        timer_tick: Some(42),
        guard: Arc::new(AtomicBool::new(true)),
        edit_revision: 1,
    }
}
#[test]
fn mouse_dynamic_source_request_requires_typed_complete_custom_float32_payload() {
    let points = vec![1.25f32; 250];
    let mut request = request(
        "ON_SET_DYNAMIC_SENSITIVITY",
        json!({"dynamicSensitivity":{
        "state":1,"mode":3,"customSensorAccelerations":points,"templateId":3,"points":[]}}),
    );
    let MouseDynamicAction::Write(setting) = request.action().unwrap() else {
        panic!("actual source setter required")
    };
    assert!(setting.enabled);
    assert_eq!(
        setting.mode,
        razer_device::mouse_sensor::AccelerationMode::Custom
    );
    assert_eq!(setting.custom_sensor_accelerations.len(), 250);
    request.payload["dynamicSensitivity"]["customSensorAccelerations"] = json!([1.]);
    assert!(request.action().is_err());
}
#[test]
fn mouse_dynamic_source_request_rejects_missing_unknown_or_wrongly_typed_fields() {
    for value in [
        json!({"state":1,"mode":2}),
        json!({"state":2,"mode":2,"customSensorAccelerations":[]}),
        json!({"state":1,"mode":7,"customSensorAccelerations":[]}),
        json!({"state":"1","mode":2,"customSensorAccelerations":[]}),
    ] {
        assert!(
            request(
                "ON_SET_DYNAMIC_SENSITIVITY",
                json!({"dynamicSensitivity":value})
            )
            .action()
            .is_err()
        );
    }
}
#[test]
fn mouse_dynamic_monitor_request_has_independent_owner_guard_from_device_mutations() {
    let device = request(
        "ON_SET_DYNAMIC_SENSITIVITY",
        json!({"dynamicSensitivity":{"state":0,"mode":0,"customSensorAccelerations":[]}}),
    );
    let monitor = request("DYNAMIC_SENSITIVITY_UI_COMMAND", json!({"type":"START"}));
    assert!(matches!(
        monitor.action(),
        Ok(MouseDynamicAction::Monitor(true))
    ));
    monitor.guard().store(false, Ordering::Release);
    assert!(!monitor.guard().load(Ordering::Acquire));
    assert!(device.guard().load(Ordering::Acquire));
    assert_eq!(device.timer_tick(), Some(42));
    assert!(
        request("DYNAMIC_SENSITIVITY_UI_COMMAND", json!({"type":"UNKNOWN"}))
            .action()
            .is_err()
    );
}

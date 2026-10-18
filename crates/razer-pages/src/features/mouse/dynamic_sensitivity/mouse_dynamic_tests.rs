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
#[::core::prelude::v1::test]
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
#[::core::prelude::v1::test]
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
#[::core::prelude::v1::test]
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

fn begin(requests: &mut Requests, command: &'static str) -> MouseDynamicRequested {
    let mut request = request(command, json!({}));
    request.request_id = requests.begin(command);
    request.edit_revision = requests.edit_revision;
    request
}

#[::core::prelude::v1::test]
fn mouse_dynamic_monitor_retirement_preserves_writes_across_repeated_mounts() {
    let mut requests = Requests::default();
    let write = begin(&mut requests, "ON_SET_DYNAMIC_SENSITIVITY");
    for _ in 0..64 {
        requests.retire_monitors();
        let start = begin(&mut requests, "DYNAMIC_SENSITIVITY_UI_COMMAND");
        assert!(requests.complete(&start, MouseDynamicCompletion::Monitoring(true)));
        assert_eq!(requests.pending.len(), 2);
        start.guard.store(false, Ordering::Release);
        requests.retire_monitors();
        assert_eq!(requests.pending.len(), 1);
        assert!(!requests.complete(&start, MouseDynamicCompletion::Monitoring(false)));
        assert!(requests.current(&write));
        let stop = begin(&mut requests, "DYNAMIC_SENSITIVITY_UI_COMMAND");
        assert!(requests.complete(&stop, MouseDynamicCompletion::Monitoring(false)));
        assert_eq!(requests.pending.len(), 1);
    }
    assert!(requests.complete(&write, MouseDynamicCompletion::Cancelled));
    assert!(requests.pending.is_empty());
}

#[::core::prelude::v1::test]
fn mouse_dynamic_old_failure_retires_request_without_overwriting_new_edit_diagnostics() {
    let mut requests = Requests::default();
    let old = begin(&mut requests, "ON_SET_DYNAMIC_SENSITIVITY");
    let current = begin(&mut requests, "ON_SET_DYNAMIC_SENSITIVITY");
    assert!(requests.complete(
        &current,
        MouseDynamicCompletion::Failed("current write failure".into())
    ));
    assert!(requests.complete(
        &old,
        MouseDynamicCompletion::Failed("old write failure".into())
    ));
    assert_eq!(
        requests.last_failure.as_deref(),
        Some("current write failure")
    );
    assert!(requests.pending.is_empty());
}

#[::core::prelude::v1::test]
fn mouse_dynamic_old_read_failure_does_not_clear_new_custom_curve_loading() {
    let mut requests = Requests::default();
    let old_read = begin(&mut requests, "READ_DYNAMIC_SENSITIVITY");
    let write = begin(&mut requests, "ON_SET_DYNAMIC_SENSITIVITY");
    requests.loading = true;
    assert!(requests.complete(
        &old_read,
        MouseDynamicCompletion::Failed("old diagnostic read failure".into())
    ));
    assert!(requests.loading);
    assert!(requests.last_failure.is_none());
    assert!(requests.complete(
        &write,
        MouseDynamicCompletion::Unsupported("current route failure".into())
    ));
    assert!(!requests.loading);
    assert_eq!(
        requests.last_failure.as_deref(),
        Some("current route failure")
    );
}

#[::core::prelude::v1::test]
fn mouse_dynamic_completions_require_exact_pending_command_and_live_request_owner() {
    let mut requests = Requests::default();
    let write = begin(&mut requests, "ON_SET_DYNAMIC_SENSITIVITY");
    let mut wrong = write.clone();
    wrong.command = "READ_DYNAMIC_SENSITIVITY";
    assert!(!requests.complete(
        &wrong,
        MouseDynamicCompletion::Failed("wrong command".into())
    ));
    assert!(requests.current(&write));
    write.guard.store(false, Ordering::Release);
    assert!(!requests.complete(
        &write,
        MouseDynamicCompletion::Failed("canceled owner".into())
    ));
    assert!(requests.last_failure.is_none());
}

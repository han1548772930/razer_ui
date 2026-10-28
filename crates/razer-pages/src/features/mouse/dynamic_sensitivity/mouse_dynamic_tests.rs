use super::requests::{Lifecycle, Requests};
use super::*;

#[::core::prelude::v1::test]
fn mouse_dynamic_monitor_navigation_is_product_specific() {
    for product_id in [190, 192, 196, 222, 229, 239] {
        assert!(monitor_should_run(
            product_id,
            true,
            "ADVANCED",
            true,
            Some(true),
            true,
        ));
        assert!(!monitor_should_run(
            product_id,
            true,
            "TAB_PERFORMANCE",
            true,
            Some(true),
            true,
        ));
    }
    assert!(monitor_should_run(
        226,
        true,
        "TAB_PERFORMANCE",
        true,
        Some(true),
        true,
    ));
    assert!(monitor_should_run(226, true, "ADVANCED", true, None, true,));
    assert!(!monitor_should_run(
        226,
        true,
        "TAB_PERFORMANCE",
        true,
        Some(false),
        true,
    ));
    assert!(!monitor_should_run(
        226,
        false,
        "TAB_PERFORMANCE",
        true,
        Some(true),
        true,
    ));
    assert!(!monitor_should_run(
        226,
        true,
        "HELP",
        true,
        Some(true),
        true,
    ));
}

#[::core::prelude::v1::test]
fn mouse_dynamic_monitor_command_does_not_restart_an_existing_226_monitor() {
    assert_eq!(monitor_command(true, true), None);
    assert_eq!(monitor_command(false, true), Some(false));
    assert_eq!(monitor_command(true, false), Some(true));
}

#[::core::prelude::v1::test]
fn mouse_dynamic_speed_boundary_retains_infinity_for_the_source_chart_clipping() {
    let grid = Grid {
        gap_x: 15.,
        gap_y: 0.25,
        start_y: 1.,
    };
    let value = observed_mouse_speed(f64::INFINITY).unwrap();
    assert!(value.is_infinite());
    let projected = grid.pixel(CurvePoint { x: value, y: 0. })[0];
    assert_eq!(projected.min(1080.), 1080.);
    for value in [0., 5., 50000.] {
        assert_eq!(observed_mouse_speed(value), Some(value));
    }
    for value in [f64::NAN, f64::NEG_INFINITY, -1.] {
        assert!(observed_mouse_speed(value).is_none());
    }
}

fn lifecycle_request(lifecycle: &mut Lifecycle, command: &'static str) -> MouseDynamicRequested {
    let mut request = request(command, json!({}));
    request.scope = lifecycle.scope();
    request.request_id = lifecycle.requests.begin(command);
    request.edit_revision = lifecycle.requests.edit_revision;
    request.guard = if command == "DYNAMIC_SENSITIVITY_UI_COMMAND" {
        lifecycle.monitor_guard.clone()
    } else {
        lifecycle.guard.clone()
    };
    request
}

#[::core::prelude::v1::test]
fn mouse_dynamic_speed_stop_and_profile_changes_keep_the_source_expiry() {
    for product_id in [190, 192, 196, 222, 226, 229, 239] {
        let mut lifecycle = Lifecycle::new(EntityId::from(product_id));
        lifecycle.monitor_guard = Arc::new(AtomicBool::new(true));
        let connection = lifecycle.scope().connection();
        let expiry = lifecycle.observe_speed(connection, 27.).unwrap();
        let stop = lifecycle_request(&mut lifecycle, "DYNAMIC_SENSITIVITY_UI_COMMAND");
        assert!(
            lifecycle
                .complete(&stop, MouseDynamicCompletion::Monitoring(false))
                .is_some()
        );
        assert_eq!(lifecycle.speed(), 27.);
        lifecycle.restore_profile();
        assert_eq!(lifecycle.speed(), 27.);
        assert!(lifecycle.expire_speed(expiry));
        assert_eq!(lifecycle.speed(), 0.);
    }
}

#[::core::prelude::v1::test]
fn mouse_dynamic_profile_change_preserves_capture_but_invalidates_device_requests() {
    let mut lifecycle = Lifecycle::new(EntityId::from(1));
    lifecycle.monitor_guard = Arc::new(AtomicBool::new(true));
    lifecycle.monitor_requested = true;
    let monitoring = lifecycle_request(&mut lifecycle, "DYNAMIC_SENSITIVITY_UI_COMMAND");
    let reading = lifecycle_request(&mut lifecycle, "READ_DYNAMIC_SENSITIVITY");
    let connection = lifecycle.scope().connection();
    let expiry = lifecycle.observe_speed(connection, 31.).unwrap();
    lifecycle.restore_profile();
    assert!(lifecycle.monitor_requested);
    assert!(monitoring.guard().load(Ordering::Acquire));
    assert!(lifecycle.request_is_current(&monitoring));
    assert!(!lifecycle.request_is_current(&reading));
    assert!(!reading.guard().load(Ordering::Acquire));
    assert!(
        lifecycle
            .complete(
                &reading,
                MouseDynamicCompletion::Device(reading_report(0, 0))
            )
            .is_none()
    );
    assert!(
        lifecycle
            .complete(&monitoring, MouseDynamicCompletion::Monitoring(true))
            .is_some()
    );
    assert_eq!(lifecycle.speed(), 31.);
    assert!(lifecycle.expire_speed(expiry));
    assert_eq!(lifecycle.speed(), 0.);
    assert_eq!(lifecycle.requests.enabled(), None);
}

#[::core::prelude::v1::test]
fn mouse_dynamic_speed_rejects_other_owners_replaced_connections_and_old_expiry() {
    let mut lifecycle = Lifecycle::new(EntityId::from(1));
    let old = lifecycle.scope().connection();
    let old_expiry = lifecycle.observe_speed(old, 41.).unwrap();
    let replacement_expiry = lifecycle.observe_speed(old, 43.).unwrap();
    assert!(!lifecycle.expire_speed(old_expiry));
    assert_eq!(lifecycle.speed(), 43.);
    let foreign = Lifecycle::new(EntityId::from(2)).scope().connection();
    assert!(lifecycle.observe_speed(foreign, 99.).is_none());
    assert_eq!(lifecycle.speed(), 43.);
    lifecycle.invalidate_connection();
    assert_eq!(lifecycle.speed(), 0.);
    assert!(lifecycle.observe_speed(old, 99.).is_none());
    let current = lifecycle.scope().connection();
    let expiry = lifecycle.observe_speed(current, 47.).unwrap();
    assert!(!lifecycle.expire_speed(replacement_expiry));
    assert_eq!(lifecycle.speed(), 47.);
    assert!(lifecycle.expire_speed(expiry));
    assert_eq!(lifecycle.speed(), 0.);
}

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
fn mouse_dynamic_observer_attach_completion_retires_only_the_attach_request() {
    let mut requests = Requests::default();
    let attach = begin(&mut requests, "DYNAMIC_SENSITIVITY_UI_COMMAND");
    assert!(requests.complete(&attach, MouseDynamicCompletion::ObserverAttached));
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

fn reading_report(state: u8, mode: u8) -> Report {
    serde_json::from_value(json!({
        "requested":null, "writes":null, "write_failure":null, "read_failure":null,
        "reading":{"raw_state":state, "raw_mode":mode,"custom_sensor_accelerations":null}
    }))
    .unwrap()
}

#[::core::prelude::v1::test]
fn mouse_dynamic_device_read_populates_control_values_without_local_profile_fallback() {
    let mut requests = Requests::default();
    assert_eq!((requests.enabled(), requests.mode()), (None, None));
    let read = begin(&mut requests, "READ_DYNAMIC_SENSITIVITY");
    assert!(matches!(requests.observation, Observation::Loading));
    assert!(requests.complete(&read, MouseDynamicCompletion::Device(reading_report(1, 2))));
    assert_eq!((requests.enabled(), requests.mode()), (Some(true), Some(2)));
    let read = begin(&mut requests, "READ_DYNAMIC_SENSITIVITY");
    assert!(requests.complete(
        &read,
        MouseDynamicCompletion::Failed("actual GET failed".into())
    ));
    assert_eq!((requests.enabled(), requests.mode()), (None, None));
    assert!(
        matches!(&requests.observation, Observation::Failed(error) if error == "actual GET failed")
    );
}

#[::core::prelude::v1::test]
fn mouse_dynamic_unknown_and_reconnect_do_not_invent_disabled_or_classic_state() {
    let mut requests = Requests::default();
    let read = begin(&mut requests, "READ_DYNAMIC_SENSITIVITY");
    requests.complete(&read, MouseDynamicCompletion::Device(reading_report(7, 99)));
    assert_eq!((requests.enabled(), requests.mode()), (None, None));
    assert_eq!(
        requests.last_failure.as_deref(),
        Some("Unknown dynamic sensitivity values: state 7, mode 99")
    );
    assert_eq!(
        (
            requests.reading().unwrap().raw_state,
            requests.reading().unwrap().raw_mode
        ),
        (7, 99)
    );
    requests.reset_observation(false);
    assert!(matches!(requests.observation, Observation::Disconnected));
    assert!(!requests.complete(&read, MouseDynamicCompletion::Device(reading_report(1, 2))));
    requests.reset_observation(true);
    assert_eq!((requests.enabled(), requests.mode()), (None, None));
    let reconnected = begin(&mut requests, "READ_DYNAMIC_SENSITIVITY");
    requests.complete(
        &reconnected,
        MouseDynamicCompletion::Device(reading_report(0, 1)),
    );
    assert_eq!(
        (requests.enabled(), requests.mode()),
        (Some(false), Some(1))
    );
}

#[::core::prelude::v1::test]
fn mouse_dynamic_readback_preserves_explicit_edit_until_real_device_confirmation() {
    let mut requests = Requests::default();
    let initial = begin(&mut requests, "READ_DYNAMIC_SENSITIVITY");
    requests.complete(
        &initial,
        MouseDynamicCompletion::Device(reading_report(1, 0)),
    );
    requests.edit = Some(
        Setting::new(
            true,
            razer_device::mouse_sensor::AccelerationMode::Jump,
            vec![],
        )
        .unwrap(),
    );
    let write = begin(&mut requests, "ON_SET_DYNAMIC_SENSITIVITY");
    requests.complete(
        &write,
        MouseDynamicCompletion::Failed("write acknowledgement lost".into()),
    );
    let read = begin(&mut requests, "READ_DYNAMIC_SENSITIVITY");
    requests.complete(&read, MouseDynamicCompletion::Device(reading_report(1, 1)));
    assert_eq!(requests.reading().unwrap().raw_mode, 1);
    assert_eq!(requests.mode(), Some(2));
    assert!(requests.edit.is_some());
    assert!(requests.last_failure.is_some());
    let read = begin(&mut requests, "READ_DYNAMIC_SENSITIVITY");
    requests.complete(&read, MouseDynamicCompletion::Device(reading_report(1, 2)));
    assert!(requests.edit.is_none());
    assert_eq!(requests.mode(), Some(2));
    assert_eq!(
        requests.last_failure.as_deref(),
        Some("write acknowledgement lost")
    );
}

#[::core::prelude::v1::test]
fn mouse_dynamic_read_before_an_edit_cannot_replace_its_confirmation() {
    let mut requests = Requests::default();
    let stale = begin(&mut requests, "READ_DYNAMIC_SENSITIVITY");
    let write = begin(&mut requests, "ON_SET_DYNAMIC_SENSITIVITY");
    requests.complete(&write, MouseDynamicCompletion::Cancelled);
    let fresh = begin(&mut requests, "READ_DYNAMIC_SENSITIVITY");
    requests.complete(&fresh, MouseDynamicCompletion::Device(reading_report(1, 2)));
    requests.complete(&stale, MouseDynamicCompletion::Device(reading_report(0, 0)));
    assert_eq!((requests.enabled(), requests.mode()), (Some(true), Some(2)));
}

use super::*;
use razer_device::mouse_current_dpi::DpiPair;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, atomic::AtomicBool},
};

#[derive(Default)]
struct Behavior {
    identity: bool,
    fail_execute: bool,
    panic_execute: bool,
    fail_shutdown: bool,
    fail_termination: bool,
    cancel_on_shutdown: bool,
    retain_partial_ble: bool,
}

struct Scripted {
    calls: Rc<RefCell<Vec<&'static str>>>,
    behavior: Behavior,
    valid: Arc<AtomicBool>,
}

fn report() -> DpiReport {
    serde_json::from_value(serde_json::json!({
        "requested":null,"stages":null,"active":{"x":2000,"y":2000},
        "acknowledgements":[],"verified":true
    }))
    .unwrap()
}

impl Transport for Scripted {
    fn execute(&mut self) -> anyhow::Result<DpiReport> {
        self.calls.borrow_mut().push("execute");
        assert!(!self.behavior.panic_execute, "scripted DPI failure");
        ensure!(!self.behavior.fail_execute, "device read failed");
        if self.behavior.identity {
            Ok(serde_json::from_value(serde_json::json!({
                "requested":{"initialize":true,"stages":null,"active":null,"profile_guid":null},
                "stages":null,"active":null,"serial_number":"actual-mouse-serial","acknowledgements":[],"verified":true
            }))?)
        } else {
            Ok(report())
        }
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("shutdown");
        if self.behavior.cancel_on_shutdown {
            self.valid.store(false, Ordering::Release);
        }
        ensure!(!self.behavior.fail_shutdown, "shutdown failed");
        Ok(())
    }
    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("terminate");
        ensure!(!self.behavior.fail_termination, "join failed");
        Ok(())
    }
    fn outcome(&self) -> Option<Outcome> {
        self.behavior.retain_partial_ble.then(|| {
            let mut outcome = Outcome::from_report(report());
            outcome.error = Some("BLE request canceled after a real response".into());
            outcome
        })
    }
}

impl Drop for Scripted {
    fn drop(&mut self) {
        self.calls.borrow_mut().push("drop");
    }
}

fn run(behavior: Behavior) -> (Completion, Vec<&'static str>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let valid = Arc::new(AtomicBool::new(true));
    let outcome = run_with(&valid, || {
        Ok(Scripted {
            calls: calls.clone(),
            behavior,
            valid: valid.clone(),
        })
    });
    let log = calls.borrow().clone();
    (outcome, log)
}

#[test]
fn actual_readback_is_delivered_only_after_shutdown_join_and_destruction() {
    let (completion, calls) = run(Behavior::default());
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert_eq!(
        completion.report().unwrap().active,
        Some(DpiPair { x: 2000, y: 2000 })
    );
    assert!(completion.cleanup_error().is_none());
}
#[test]
fn observed_identity_selects_local_profile_only_after_transport_cleanup() {
    let valid = AtomicBool::new(true);
    let (mut completion, calls) = run(Behavior {
        identity: true,
        ..Default::default()
    });
    observe_local_profile(
        &mut completion,
        &valid,
        true,
        "receiver-usb-serial",
        |serial| {
            assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
            assert_eq!(serial, "actual-mouse-serial");
            Ok(None)
        },
    );
    assert!(matches!(completion.local_profile, Some(Ok(None))));
    assert!(completion.report().unwrap().active.is_none());
}
#[test]
fn failed_identity_and_canceled_profile_read_never_use_a_previous_serial() {
    let valid = AtomicBool::new(true);
    let (mut failed, _) = run(Behavior {
        fail_execute: true,
        ..Default::default()
    });
    observe_local_profile(&mut failed, &valid, true, "old-peer", |_| {
        panic!("failed identity must not select storage")
    });
    assert!(matches!(failed.local_profile, Some(Err(_))));
    let (mut completed, _) = run(Behavior {
        identity: true,
        ..Default::default()
    });
    observe_local_profile(&mut completed, &valid, true, "old-peer", |_| {
        valid.store(false, Ordering::Release);
        Ok(None)
    });
    assert!(matches!(completed.local_profile, Some(Err(_))));
}

#[test]
fn a_failed_read_still_releases_the_service_in_full_order() {
    let (completion, calls) = run(Behavior {
        fail_execute: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(
        completion
            .result
            .as_ref()
            .unwrap_err()
            .contains("device read failed")
    );
    assert!(completion.report().is_none());
}

#[test]
fn a_panicked_read_cannot_skip_cleanup_or_publish_success() {
    let (completion, calls) = run(Behavior {
        panic_execute: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.result.is_err());
}

#[test]
fn shutdown_and_join_failures_remain_distinct_from_the_valid_observation() {
    let (completion, calls) = run(Behavior {
        fail_shutdown: true,
        fail_termination: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.report().is_some());
    let cleanup = completion.cleanup_error().unwrap();
    assert!(cleanup.contains("shutdown failed"));
    assert!(cleanup.contains("join failed"));
    assert!(completion.result().is_err());
    assert_eq!(
        completion.actual_report().unwrap().active,
        Some(DpiPair { x: 2000, y: 2000 })
    );
}

#[test]
fn canceled_ble_transport_keeps_its_validated_partial_report_without_success() {
    let (completion, calls) = run(Behavior {
        fail_execute: true,
        retain_partial_ble: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.result().is_err());
    assert!(completion.cleanup_error().is_none());
    assert_eq!(
        completion.actual_report().unwrap().active,
        Some(DpiPair { x: 2000, y: 2000 })
    );
}

#[test]
fn failure_of_both_device_read_and_cleanup_preserves_all_errors() {
    let (completion, _) = run(Behavior {
        fail_execute: true,
        fail_shutdown: true,
        fail_termination: true,
        ..Default::default()
    });
    assert!(
        completion
            .result
            .unwrap_err()
            .contains("device read failed")
    );
    assert!(
        completion
            .shutdown_error
            .unwrap()
            .contains("shutdown failed")
    );
    assert!(
        completion
            .termination_error
            .unwrap()
            .contains("join failed")
    );
}

#[test]
fn canceled_scope_never_starts_native_io() {
    let completion = run_with::<Scripted>(&AtomicBool::new(false), || {
        panic!("factory called after cancel")
    });
    assert!(completion.result.is_err());
}

#[test]
fn cancellation_during_cleanup_rejects_the_late_observation() {
    let (completion, calls) = run(Behavior {
        cancel_on_shutdown: true,
        ..Default::default()
    });
    assert_eq!(calls, ["execute", "shutdown", "terminate", "drop"]);
    assert!(completion.report().is_none());
    assert!(completion.result.unwrap_err().contains("取消"));
}

#[test]
fn service_creation_failure_is_not_a_device_read_or_cleanup_success() {
    let completion = run_with::<Scripted>(&AtomicBool::new(true), || anyhow::bail!("spawn failed"));
    assert!(completion.result.unwrap_err().contains("spawn failed"));
    assert!(completion.shutdown_error.is_none());
    assert!(completion.termination_error.is_none());
}

fn local_device() -> razer_model::model::Device {
    use razer_model::model::Profile;
    let mut device = razer_model::product_preview::registered_preview(226).unwrap();
    device.serial_number = "actual-serial".into();
    device.active_profile = "local-id".into();
    device.source_device_settings = Some(serde_json::json!({
        "deviceMetadatas":{"actual-serial":{"activeProfileGuid":"actual-local-guid"}}
    }));
    device.profiles.push(Profile {
        id: "local-id".into(),
        guid: "actual-local-guid".into(),
        name: "Local".into(),
        source_settings: Some(serde_json::json!({
            "guid":"actual-local-guid","name":"Local","obmSlotId":[2],
            "dpiStages":{"enable":true,"active":1,"stages":[{"x":1600,"y":1700}]}
        })),
        settings: None,
        dpi_stages: None,
    });
    device
}

#[test]
fn ble_initialization_uses_observed_serial_and_actual_ordered_source_profiles() {
    let device = local_device();
    let request = local_profiles_for_serial(&device, "actual-serial")
        .unwrap()
        .unwrap();
    assert_eq!(request.owner, LOCAL_GUEST_OBM_OWNER);
    assert_eq!(request.active_profile_guid, "actual-local-guid");
    assert_eq!(request.profiles[0].obm_slot_ids, [2]);
    assert_eq!(
        request.profiles[0]
            .dpi_stages
            .as_ref()
            .unwrap()
            .active_stage(),
        1
    );
    assert!(
        local_profiles_for_serial(&device, "old-serial")
            .unwrap()
            .is_none()
    );
    let preview = razer_model::product_preview::registered_preview(226).unwrap();
    assert!(
        local_profiles_for_serial(&preview, "actual-serial")
            .unwrap()
            .is_none()
    );
}

#[test]
fn ble_missing_metadata_and_slot_associations_never_synthesize_dpi_defaults() {
    let mut device = local_device();
    device.source_device_settings = Some(serde_json::json!({}));
    assert!(
        local_profiles_for_serial(&device, "actual-serial")
            .unwrap()
            .is_none()
    );
    device.source_device_settings = local_device().source_device_settings;
    device.profiles[0]
        .source_settings
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("obmSlotId");
    device.profiles[0]
        .source_settings
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("dpiStages");
    let request = local_profiles_for_serial(&device, "actual-serial")
        .unwrap()
        .unwrap();
    assert!(request.profiles[0].obm_slot_ids.is_empty());
    assert!(request.profiles[0].dpi_stages.is_none());
    for (field, value) in [
        ("obmSlotId", serde_json::json!(null)),
        ("obmSlotId", serde_json::json!([0])),
        ("dpiStages", serde_json::json!(null)),
        ("guid", serde_json::json!("another-guid")),
    ] {
        let mut invalid = device.clone();
        invalid.profiles[0].source_settings.as_mut().unwrap()[field] = value;
        assert!(
            local_profiles_for_serial(&invalid, "actual-serial").is_err(),
            "{field}"
        );
    }
    device.source_device_settings = Some(serde_json::json!({"deviceMetadatas":null}));
    assert!(local_profiles_for_serial(&device, "actual-serial").is_err());
}

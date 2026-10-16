//! Typed wire round trips only; no worker process or OS endpoint starts here.
use super::*;
use razer_device::thx_submission::{
    ThxOperationStatus, ThxSubmissionPlan, ThxSubmissionProgress, ThxSubmissionScope,
};
fn scope() -> ThxSubmissionScope {
    ThxSubmissionScope {
        product_id: 1398,
        container_id: "mock-container".into(),
        serial_number: "mock".into(),
        profile_id: "local".into(),
        generation: 9,
    }
}
#[test]
fn thx_scope_and_setter_round_trip_preserve_timer_profile_and_local_presets() {
    let request = RequestEnvelope {
        id: 42,
        request: ServiceRequest::ThxStart {
            operation_id: "op".into(),
            scope: scope(),
            plan: ThxSubmissionPlan::SetSpatial {
                enabled: false,
                presets: vec![serde_json::json!({"index":0,"currentActive":true})],
                timer_tick: 123,
            },
        },
    };
    let restored: RequestEnvelope =
        serde_json::from_slice(&serde_json::to_vec(&request).unwrap()).unwrap();
    let ServiceRequest::ThxStart {
        scope: received,
        plan,
        ..
    } = restored.request
    else {
        panic!()
    };
    assert_eq!(received, scope());
    assert_eq!(
        plan,
        ThxSubmissionPlan::SetSpatial {
            enabled: false,
            presets: vec![serde_json::json!({"index":0,"currentActive":true})],
            timer_tick: 123
        }
    );
}
#[test]
fn thx_cancel_poll_and_partial_failure_round_trip_without_success() {
    for request in [
        ServiceRequest::ThxCancel {
            operation_id: "op".into(),
        },
        ServiceRequest::ThxPoll {
            operation_id: "op".into(),
        },
    ] {
        let bytes = serde_json::to_vec(&request).unwrap();
        let parsed: ServiceRequest = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(serde_json::to_vec(&parsed).unwrap(), bytes);
    }
    let status = ThxOperationStatus::Finished(ThxSubmissionProgress {
        scope: scope(),
        plan: ThxSubmissionPlan::ReadSpatial,
        render_setup: None,
        render_shutdown: None,
        roller_mapping: None,
        initial_observation: None,
        outcome: None,
        firmware: vec![],
        error: Some("unavailable".into()),
        cleanup_error: Some("cleanup failed".into()),
    });
    assert_eq!(
        serde_json::from_value::<ThxOperationStatus>(serde_json::to_value(&status).unwrap())
            .unwrap(),
        status
    );
}
#[test]
fn thx_wire_rejects_untyped_export_and_non_boolean_spatial_request() {
    let invalid = serde_json::json!({"operation":"thx_start","operation_id":"op","scope":scope(),"plan":{"CallExport":{"name":"SetProperty","args":[]}}});
    assert!(serde_json::from_value::<ServiceRequest>(invalid).is_err());
    let invalid = serde_json::json!({"operation":"thx_start","operation_id":"op","scope":scope(),"plan":{"SetSpatial":{"enabled":1,"presets":[],"timer_tick":0}}});
    assert!(serde_json::from_value::<ServiceRequest>(invalid).is_err());
}

#[test]
fn thx_lifecycle_trigger_round_trip_cannot_be_confused_with_a_read() {
    use razer_device::thx_v4::ThxRenderSetupTrigger;
    for trigger in [
        ThxRenderSetupTrigger::InitialDeviceSetup,
        ThxRenderSetupTrigger::DefaultPlaybackDeviceChanged,
        ThxRenderSetupTrigger::PostRestoreDeviceData,
    ] {
        let request = ServiceRequest::ThxStart {
            operation_id: "render-setup".into(),
            scope: scope(),
            plan: ThxSubmissionPlan::SetupRender { trigger },
        };
        let restored: ServiceRequest =
            serde_json::from_slice(&serde_json::to_vec(&request).unwrap()).unwrap();
        let ServiceRequest::ThxStart {
            plan,
            scope: restored_scope,
            ..
        } = restored
        else {
            panic!()
        };
        assert_eq!(restored_scope, scope());
        assert_eq!(plan, ThxSubmissionPlan::SetupRender { trigger });
        assert_ne!(plan, ThxSubmissionPlan::ReadSpatial);
        assert_eq!(
            trigger.force_rebind(),
            trigger == ThxRenderSetupTrigger::DefaultPlaybackDeviceChanged
        );
    }
}

#[test]
fn thx_retained_owner_start_and_submit_keep_separate_owner_and_operation_ids() {
    use razer_device::thx_v4::ThxRenderSetupTrigger;
    let start = ServiceRequest::ThxOwnerStart {
        owner_id: "physical-owner".into(),
        operation_id: "setup-job".into(),
        scope: scope(),
        plan: ThxSubmissionPlan::SetupRender {
            trigger: ThxRenderSetupTrigger::InitialDeviceSetup,
        },
    };
    let bytes = serde_json::to_vec(&start).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["operation"],
        "thx_owner_start"
    );
    let ServiceRequest::ThxOwnerStart {
        owner_id,
        operation_id,
        scope: received_scope,
        plan,
    } = serde_json::from_slice(&bytes).unwrap()
    else {
        panic!("Owner start became a one-off job")
    };
    assert_eq!(owner_id, "physical-owner");
    assert_eq!(operation_id, "setup-job");
    assert_eq!(received_scope, scope());
    assert_eq!(
        plan,
        ThxSubmissionPlan::SetupRender {
            trigger: ThxRenderSetupTrigger::InitialDeviceSetup
        }
    );

    let mut changed = scope();
    changed.profile_id = "next-local-profile".into();
    changed.generation += 1;
    for plan in [
        ThxSubmissionPlan::ReadSpatial,
        ThxSubmissionPlan::ReadRollerMapping,
        ThxSubmissionPlan::SetRollerMappings {
            mappings: vec![serde_json::json!({"outputType":"disableGroup"})],
        },
        ThxSubmissionPlan::ShutdownRender,
        ThxSubmissionPlan::SetupRender {
            trigger: ThxRenderSetupTrigger::DefaultPlaybackDeviceChanged,
        },
    ] {
        let submit = ServiceRequest::ThxOwnerSubmit {
            owner_id: owner_id.clone(),
            operation_id: "next-job".into(),
            scope: changed.clone(),
            plan: plan.clone(),
        };
        let bytes = serde_json::to_vec(&submit).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["operation"],
            "thx_owner_submit"
        );
        let ServiceRequest::ThxOwnerSubmit {
            owner_id: received_owner,
            operation_id,
            scope: received_scope,
            plan: received_plan,
        } = serde_json::from_slice(&bytes).unwrap()
        else {
            panic!("Owner submit lost its lifetime")
        };
        assert_eq!(received_owner, owner_id);
        assert_eq!(operation_id, "next-job");
        assert_eq!(received_scope, changed);
        assert_eq!(received_plan, plan);
    }
}

#[test]
fn thx_owner_release_is_distinct_from_cancel_and_render_shutdown() {
    let release = ServiceRequest::ThxOwnerRelease {
        owner_id: "physical-owner".into(),
    };
    let encoded = serde_json::to_value(&release).unwrap();
    assert_eq!(
        encoded,
        serde_json::json!({"operation":"thx_owner_release","owner_id":"physical-owner"})
    );
    let ServiceRequest::ThxOwnerRelease { owner_id } = serde_json::from_value(encoded).unwrap()
    else {
        panic!("Release became a submitted shutdown or per-job cancellation")
    };
    assert_eq!(owner_id, "physical-owner");
}

#[test]
fn thx_shutdown_receipt_keeps_ack_and_cancellation_without_claiming_completion() {
    use razer_device::thx_v4::{
        ThxLifecycleReceipt, ThxOperation, ThxPropertyRequest, ThxRenderShutdownOutcome,
        ThxRenderShutdownStatus, ThxReply, ThxTransportKind,
    };
    let progress = ThxSubmissionProgress {
        scope: scope(),
        plan: ThxSubmissionPlan::ShutdownRender,
        render_setup: None,
        roller_mapping: None,
        render_shutdown: Some(ThxRenderShutdownOutcome {
            status: ThxRenderShutdownStatus::Canceled,
            transport_kind: ThxTransportKind::Simulated,
            shutdown_completed: false,
            receipts: vec![ThxLifecycleReceipt {
                operation: ThxOperation::SetProperty(
                    ThxPropertyRequest::new("eq_enabled", serde_json::json!(false), None).unwrap(),
                ),
                reply: Some(ThxReply::RpcAcknowledged),
                error: Some("job canceled after response".into()),
            }],
            warnings: vec![],
            error: Some("job canceled".into()),
        }),
        initial_observation: None,
        outcome: None,
        firmware: vec![],
        error: Some("job canceled".into()),
        cleanup_error: Some("resource release failed".into()),
    };
    let status = ThxOperationStatus::Finished(progress);
    assert_eq!(
        serde_json::from_slice::<ThxOperationStatus>(&serde_json::to_vec(&status).unwrap())
            .unwrap(),
        status
    );
}

#[test]
fn thx_older_progress_without_shutdown_field_does_not_invent_a_shutdown() {
    let mut encoded = serde_json::to_value(ThxSubmissionProgress {
        scope: scope(),
        plan: ThxSubmissionPlan::ReadSpatial,
        render_setup: None,
        render_shutdown: None,
        roller_mapping: None,
        initial_observation: None,
        outcome: None,
        firmware: vec![],
        error: Some("unavailable".into()),
        cleanup_error: None,
    })
    .unwrap();
    encoded.as_object_mut().unwrap().remove("render_shutdown");
    let restored: ThxSubmissionProgress = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored.plan, ThxSubmissionPlan::ReadSpatial);
    assert!(restored.render_shutdown.is_none());
    assert_eq!(restored.error.as_deref(), Some("unavailable"));
}

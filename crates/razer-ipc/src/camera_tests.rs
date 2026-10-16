//! Wire-schema checks only: no ServiceClient, worker process, DLL or device.
use super::*;
use razer_device::{
    camera::{CameraControlProperty, CameraSetting, StandardCameraProperty, StandardCameraSetting},
    camera_submission::{
        CameraCommand, CameraOperationStatus, CameraScope, CameraSubmissionPlan,
        CameraSubmissionProgress,
    },
};

fn scope() -> CameraScope {
    CameraScope {
        product_id: 3596,
        container_id: "{01234567-89ab-cdef-0123-456789abcdef}".into(),
    }
}
#[test]
fn camera_task_start_retains_container_actual_source_name_and_compound_plan() {
    let plan = CameraSubmissionPlan {
        commands: vec![CameraCommand::StandardRead(
            StandardCameraProperty::CameraControl(CameraControlProperty::Focus),
        )],
    };
    let envelope = RequestEnvelope {
        id: 301,
        request: ServiceRequest::CameraTaskStart {
            operation_id: "camera named task".into(),
            scope: scope(),
            task_name: "CAMERA_AUTOFRAMING".into(),
            plan: plan.clone(),
        },
    };
    let restored: RequestEnvelope =
        serde_json::from_slice(&serde_json::to_vec(&envelope).unwrap()).unwrap();
    let ServiceRequest::CameraTaskStart {
        scope: target,
        task_name,
        plan: actual,
        ..
    } = restored.request
    else {
        panic!("wrong named camera request")
    };
    assert_eq!(target, scope());
    assert_eq!(task_name, "CAMERA_AUTOFRAMING");
    assert_eq!(actual, plan);
}

#[test]
fn camera_family_exposure_ipc_preserves_raw_iso_signed_requests_and_partial_receipts() {
    use razer_device::{
        camera_exposure_family::{
            CameraFamilyExposureRequest as E, CameraFamilyExposureTransport,
            CameraFamilyExposureValue as V, execute_camera_family_exposure,
            validate_camera_family_exposure_progress,
        },
        camera_extensions::CameraKsCallOutcome,
        camera_submission::CameraCommandOutcome,
    };
    struct MemoryKs;
    impl CameraFamilyExposureTransport for MemoryKs {
        fn begin_topology(&mut self) -> anyhow::Result<u32> {
            Ok(1)
        }
        fn begin_node(&mut self, _: u32) -> anyhow::Result<()> {
            Ok(())
        }
        fn ks_property(
            &mut self,
            h: &[u8; 32],
            data: &mut [u8; 8],
        ) -> anyhow::Result<CameraKsCallOutcome> {
            if h[20] == 1 {
                *data = [0, 0, 9, 0xfe, 0xdc, 0xba, 0x98, 0];
            }
            Ok(CameraKsCallOutcome {
                hresult: 0,
                bytes_returned: if h[20] == 1 { 8 } else { 0 },
            })
        }
        fn end_node(&mut self) {}
        fn end_topology(&mut self) {}
        fn wait_ms(&mut self, _: u32) -> anyhow::Result<()> {
            Ok(())
        }
    }
    for (pid, request, expected) in [
        (
            3595,
            E::GetIsoSensitivity,
            Some(V::IsoSensitivity { iso: 0xfedcba98 }),
        ),
        (
            3596,
            E::SetShutterSpeed { value: -258 },
            Some(V::Acknowledged { native_value: -258 }),
        ),
        (3596, E::GetIsoSensitivity, None),
    ] {
        let command = CameraCommand::FamilyExposure(request);
        let plan = CameraSubmissionPlan {
            commands: vec![command.clone()],
        };
        let expected_scope = CameraScope {
            product_id: pid,
            ..scope()
        };
        let envelope = RequestEnvelope {
            id: 92,
            request: ServiceRequest::CameraStart {
                operation_id: "family exposure \"request\"\n92".into(),
                scope: expected_scope.clone(),
                plan: plan.clone(),
            },
        };
        let restored: RequestEnvelope =
            serde_json::from_slice(&serde_json::to_vec(&envelope).unwrap()).unwrap();
        let ServiceRequest::CameraStart {
            scope,
            plan: restored_plan,
            ..
        } = restored.request
        else {
            panic!("missing camera request")
        };
        assert_eq!(scope, expected_scope);
        assert_eq!(restored_plan, plan);
        let p = execute_camera_family_exposure(&mut MemoryKs, pid, request, &|| Ok(()));
        validate_camera_family_exposure_progress(pid, request, &p).unwrap();
        assert_eq!(p.value, expected);
        let status = CameraOperationStatus::Finished(CameraSubmissionProgress {
            completed: p.completed,
            outcomes: vec![CameraCommandOutcome::FamilyExposure(p.clone())],
            error: p.failure.as_ref().map(|f| f.message.clone()),
            ..Default::default()
        });
        let restored: CameraOperationStatus =
            serde_json::from_slice(&serde_json::to_vec(&status).unwrap()).unwrap();
        assert_eq!(restored, status);
    }
}
#[test]
fn camera_ipc_envelope_preserves_exact_scope_and_signed_manual_exposure() {
    let plan = CameraSubmissionPlan {
        commands: vec![CameraCommand::StandardWrite(StandardCameraSetting {
            property: StandardCameraProperty::CameraControl(CameraControlProperty::Exposure),
            setting: CameraSetting {
                value: -7,
                is_auto: false,
            },
        })],
    };
    let envelope = RequestEnvelope {
        id: 17,
        request: ServiceRequest::CameraStart {
            operation_id: "camera \"intent\"\n17".into(),
            scope: scope(),
            plan: plan.clone(),
        },
    };
    let raw = serde_json::to_vec(&envelope).unwrap();
    let restored: RequestEnvelope = serde_json::from_slice(&raw).unwrap();
    assert_eq!(restored.id, 17);
    let ServiceRequest::CameraStart {
        operation_id,
        scope: target,
        plan: commands,
    } = restored.request
    else {
        panic!("wrong operation")
    };
    assert_eq!(operation_id, "camera \"intent\"\n17");
    assert_eq!(target, scope());
    assert_eq!(commands, plan);
    let value: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(value["request"]["operation"], "camera_start");
}
#[test]
fn camera_ipc_cancel_and_poll_share_the_retained_operation_identifier() {
    for (operation, request) in [
        (
            "camera_cancel",
            ServiceRequest::CameraCancel {
                operation_id: "op-1".into(),
            },
        ),
        (
            "camera_poll",
            ServiceRequest::CameraPoll {
                operation_id: "op-1".into(),
            },
        ),
    ] {
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["operation"], operation);
        assert_eq!(value["operation_id"], "op-1");
        assert!(serde_json::from_value::<ServiceRequest>(value).is_ok());
    }
    assert!(
        serde_json::from_value::<ServiceRequest>(serde_json::json!({"operation":"camera_cancel"}))
            .is_err()
    );
}
#[test]
fn camera_ipc_rejects_untyped_native_commands_before_dispatch() {
    let value = serde_json::json!({"operation":"camera_start","operation_id":"op","scope":scope(),
        "plan":{"commands":[{"CallExport":{"name":"setCameraExposure","args":[-7,false]}}]}});
    assert!(serde_json::from_value::<ServiceRequest>(value).is_err());
    let value = serde_json::json!({"operation":"camera_start","operation_id":"op","scope":scope(),
        "plan":{"commands":[{"StandardWrite":{"property":{"CameraControl":"Exposure"},"setting":{"value":2147483648_i64,"is_auto":false}}}]}});
    assert!(serde_json::from_value::<ServiceRequest>(value).is_err());
}

#[test]
fn camera_ipc_preserves_supplemental_native_exports_and_pro_model_scope() {
    use razer_device::camera_extensions::CameraExtensionRequest;
    for request in [
        CameraExtensionRequest::SetHdrMode { mode: -1 },
        CameraExtensionRequest::RebootDevice,
        CameraExtensionRequest::RestoreDefaultAndReboot,
    ] {
        let plan = CameraSubmissionPlan {
            commands: vec![CameraCommand::Extension(request)],
        };
        plan.validate(3594).unwrap();
        assert!(plan.validate(3595).is_err());
        assert!(plan.validate(3596).is_err());
        let envelope = RequestEnvelope {
            id: 61,
            request: ServiceRequest::CameraStart {
                operation_id: request.action().into(),
                scope: CameraScope {
                    product_id: 3594,
                    ..scope()
                },
                plan: plan.clone(),
            },
        };
        let restored: RequestEnvelope =
            serde_json::from_slice(&serde_json::to_vec(&envelope).unwrap()).unwrap();
        let ServiceRequest::CameraStart {
            operation_id,
            scope,
            plan: restored_plan,
        } = restored.request
        else {
            panic!("missing typed camera submission")
        };
        assert_eq!(operation_id, request.action());
        assert_eq!(scope.product_id, 3594);
        assert_eq!(restored_plan, plan);
    }
}
#[test]
fn camera_ipc_partial_failure_is_finished_without_success_or_persistence() {
    let command = CameraCommand::StandardRead(StandardCameraProperty::CameraControl(
        CameraControlProperty::Focus,
    ));
    let status = CameraOperationStatus::Finished(CameraSubmissionProgress {
        unconfirmed: Some(command.clone()),
        error: Some("simulated reply loss".into()),
        ..Default::default()
    });
    let envelope = ResponseEnvelope {
        id: 23,
        data: Some(serde_json::to_value(&status).unwrap()),
        error: None,
        fatal: false,
    };
    let restored: ResponseEnvelope =
        serde_json::from_slice(&serde_json::to_vec(&envelope).unwrap()).unwrap();
    let status: CameraOperationStatus = serde_json::from_value(restored.data.unwrap()).unwrap();
    let CameraOperationStatus::Finished(progress) = status else {
        panic!("missing completion")
    };
    assert!(!progress.completed);
    assert!(progress.outcomes.is_empty());
    assert_eq!(progress.unconfirmed, Some(command));
    assert_eq!(progress.error.as_deref(), Some("simulated reply loss"));
    assert!(progress.rejected_outcome.is_none());
}

#[test]
fn camera_ipc_family_focus_preserves_model_packet_and_all_unconfirmed_node_receipts() {
    use razer_device::camera_extensions::CameraKsCallOutcome;
    use razer_device::camera_focus_family::{
        CameraFamilyFocusRequest, CameraFamilyFocusTransport, execute_camera_family_focus,
    };
    struct MemoryKs {
        calls: usize,
    }
    impl CameraFamilyFocusTransport for MemoryKs {
        fn begin_topology(&mut self) -> anyhow::Result<u32> {
            Ok(2)
        }
        fn begin_node(&mut self, _: u32) -> anyhow::Result<()> {
            Ok(())
        }
        fn ks_property(
            &mut self,
            header: &[u8; 32],
            data: &mut [u8; 8],
        ) -> anyhow::Result<CameraKsCallOutcome> {
            self.calls += 1;
            if header[20] == 2 {
                Ok(CameraKsCallOutcome {
                    hresult: 0,
                    bytes_returned: 0,
                })
            } else {
                *data = [0; 8];
                Ok(CameraKsCallOutcome {
                    hresult: if self.calls == 2 { 1 } else { 0 },
                    bytes_returned: 8,
                })
            }
        }
        fn end_node(&mut self) {}
        fn end_topology(&mut self) {}
        fn wait_ms(&mut self, _: u32) -> anyhow::Result<()> {
            Ok(())
        }
    }
    let request = CameraFamilyFocusRequest::SetFdafMode {
        mode: 257,
        light_condition: -1,
    };
    let command = CameraCommand::FamilyFocus(request);
    let envelope = RequestEnvelope {
        id: 41,
        request: ServiceRequest::CameraStart {
            operation_id: "family-focus".into(),
            scope: scope(),
            plan: CameraSubmissionPlan {
                commands: vec![command.clone()],
            },
        },
    };
    let restored: RequestEnvelope =
        serde_json::from_value(serde_json::to_value(envelope).unwrap()).unwrap();
    let ServiceRequest::CameraStart {
        plan,
        scope: restored_scope,
        ..
    } = restored.request
    else {
        panic!("missing family request")
    };
    assert_eq!(plan.commands, [command]);
    assert_eq!(restored_scope, scope());
    let progress =
        execute_camera_family_focus(&mut MemoryKs { calls: 0 }, 3596, request, &|| Ok(()));
    assert!(
        progress.completed
            && progress.attempts[0].set_attempted
            && !progress.attempts[0].acknowledged
    );
    let status = CameraOperationStatus::Finished(CameraSubmissionProgress {
        completed: true,
        outcomes: vec![
            razer_device::camera_submission::CameraCommandOutcome::FamilyFocus(progress.clone()),
        ],
        ..Default::default()
    });
    let restored: CameraOperationStatus =
        serde_json::from_slice(&serde_json::to_vec(&status).unwrap()).unwrap();
    assert_eq!(restored, status);
    assert_eq!(progress.command.unwrap().bytes, [46, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(progress.attempts.len(), 2);
    assert!(progress.attempts[1].acknowledged);
}

#[test]
fn camera_family_processing_ipc_preserves_queried_companion_and_partial_write_receipts() {
    use razer_device::{
        camera_extensions::CameraKsCallOutcome,
        camera_focus_family::CameraFamilyFocusTransport,
        camera_processing_family::{
            CameraFamilyProcessingRequest as P, CameraFamilyProcessingValue as V,
            execute_camera_family_processing, validate_camera_family_processing_progress,
        },
        camera_submission::CameraCommandOutcome,
    };
    struct MemoryKs {
        scans: usize,
        active: bool,
        reject_write: bool,
    }
    impl CameraFamilyFocusTransport for MemoryKs {
        fn begin_topology(&mut self) -> anyhow::Result<u32> {
            assert!(!self.active);
            self.active = true;
            self.scans += 1;
            Ok(1)
        }
        fn begin_node(&mut self, _: u32) -> anyhow::Result<()> {
            assert!(self.active);
            Ok(())
        }
        fn ks_property(
            &mut self,
            header: &[u8; 32],
            data: &mut [u8; 8],
        ) -> anyhow::Result<CameraKsCallOutcome> {
            assert!(self.active);
            if header[20] == 2 {
                assert_eq!(
                    *data,
                    if self.scans == 1 {
                        [8, 0, 0, 0, 0, 0, 0, 0]
                    } else {
                        [0x28, 0, 0x91, 1, 0, 0, 0, 0]
                    }
                );
                return Ok(CameraKsCallOutcome {
                    hresult: if self.reject_write && self.scans == 2 {
                        -2147467259
                    } else {
                        0
                    },
                    bytes_returned: 0,
                });
            }
            *data = [0, 0, 0x91, 0x72, 0, 0, 0, 0];
            Ok(CameraKsCallOutcome {
                hresult: 0,
                bytes_returned: 8,
            })
        }
        fn end_node(&mut self) {}
        fn end_topology(&mut self) {
            assert!(self.active);
            self.active = false;
        }
        fn wait_ms(&mut self, _: u32) -> anyhow::Result<()> {
            Ok(())
        }
    }
    let request = P::SetDynamicNoiseReduction {
        channel: 1,
        enabled: true,
    };
    let plan = CameraSubmissionPlan {
        commands: vec![CameraCommand::FamilyProcessing(request)],
    };
    let envelope = RequestEnvelope {
        id: 93,
        request: ServiceRequest::CameraTaskStart {
            operation_id: "family processing \"request\"\n93".into(),
            scope: scope(),
            task_name: "dynamicNoiseReduction".into(),
            plan: plan.clone(),
        },
    };
    let restored: RequestEnvelope =
        serde_json::from_slice(&serde_json::to_vec(&envelope).unwrap()).unwrap();
    let ServiceRequest::CameraTaskStart {
        plan: actual,
        scope: target,
        task_name,
        ..
    } = restored.request
    else {
        panic!("missing typed family processing request")
    };
    assert_eq!(actual, plan);
    assert_eq!(target, scope());
    assert_eq!(task_name, "dynamicNoiseReduction");
    for reject_write in [false, true] {
        let mut memory = MemoryKs {
            scans: 0,
            active: false,
            reject_write,
        };
        let progress = execute_camera_family_processing(&mut memory, 3596, request, &|| Ok(()));
        validate_camera_family_processing_progress(3596, request, &progress).unwrap();
        assert!(!memory.active);
        assert_eq!(memory.scans, 2);
        assert_eq!(progress.completed, !reject_write);
        assert_eq!(progress.exchanges.len(), 2);
        assert!(progress.exchanges[0].completed);
        assert_eq!(
            progress.value,
            (!reject_write).then_some(V::NativeValue { value: 1 })
        );
        let status = CameraOperationStatus::Finished(CameraSubmissionProgress {
            completed: progress.completed,
            error: progress.failure.as_ref().map(|f| f.message.clone()),
            outcomes: vec![CameraCommandOutcome::FamilyProcessing(progress.clone())],
            ..Default::default()
        });
        let envelope = ResponseEnvelope {
            id: 93,
            data: Some(serde_json::to_value(&status).unwrap()),
            error: None,
            fatal: false,
        };
        let restored: ResponseEnvelope =
            serde_json::from_slice(&serde_json::to_vec(&envelope).unwrap()).unwrap();
        let restored: CameraOperationStatus =
            serde_json::from_value(restored.data.unwrap()).unwrap();
        assert_eq!(restored, status);
        let CameraOperationStatus::Finished(restored) = restored else {
            panic!("missing family processing completion")
        };
        let CameraCommandOutcome::FamilyProcessing(restored) = &restored.outcomes[0] else {
            panic!("lost multi-exchange processing receipt")
        };
        validate_camera_family_processing_progress(3596, request, restored).unwrap();
    }
}

use super::*;

fn initialization() -> Vec<(ThxOperation, ThxReply)> {
    vec![
        (ThxOperation::AddDevice, ThxReply::NativeBool(true)),
        (ThxOperation::CheckStatus, ThxReply::NativeBool(true)),
    ]
}
fn set(id: &str, enabled: bool) -> ThxOperation {
    ThxOperation::SetProperty(ThxPropertyRequest::boolean(id, enabled))
}
fn presets() -> Vec<Value> {
    vec![
        json!({"index":0,"currentActive":false,"bandValues":[0,1]}),
        json!({"index":7,"currentActive":true,"bandValues":[2,3]}),
    ]
}
fn enable_sequence() -> Vec<(ThxOperation, ThxReply)> {
    vec![
        (
            ThxOperation::SetFirmwarePresetEqEnabled(false),
            ThxReply::FirmwareStatus("SUCCESS".into()),
        ),
        (set("eq_enabled", true), ThxReply::NativeBool(true)),
        (
            ThxOperation::RestoreCloudEqStereo {
                presets: presets(),
                selected_index: Some(json!(7)),
            },
            ThxReply::CloudEqBool(true),
        ),
        (set("spatial_enabled", true), ThxReply::NativeBool(true)),
        (
            ThxOperation::GetProperty("spatial_enabled".into()),
            ThxReply::Property(Some(json!(true))),
        ),
    ]
}
fn controller(script: Vec<(ThxOperation, ThxReply)>) -> ThxV4Controller<SimulatedThxV4Transport> {
    let mut full = initialization();
    full.extend(script);
    let mut controller = ThxV4Controller::new(1398, SimulatedThxV4Transport::new(full)).unwrap();
    controller
        .bind(|_| unreachable!("ready on first check"))
        .unwrap();
    controller
}

#[test]
fn current_source_owner_bindings_are_independently_gated() {
    let document: Value = serde_json::from_str(include_str!(
        "../../testdata/re/evidence/thx-source-current.json"
    ))
    .unwrap();
    for cap in CAPABILITIES {
        let owner = document["products"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["product_id"] == cap.product_id)
            .unwrap();
        let binding = owner["render_bindings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["feature_key"] == "thxv4_render")
            .unwrap();
        assert_eq!(binding["config"]["thxProductId"], cap.thx_product_id);
        assert_eq!(binding["config"]["endpointId"], cap.endpoint_id);
        assert_eq!(binding["config"]["features"]["ON_SET_SPATIAL_ENABLE"], true);
        assert_eq!(cap.direct_transport_available, cfg!(windows));
    }
    // V3, Carol, Blade and Barracuda do not inherit this Valkyrie chain merely
    // because their DLL exports or source property names overlap.
    for unsupported in [1306, 1392, 1442, 3942, 709, 1427, 0, u32::MAX] {
        assert!(capability(unsupported).is_none());
        assert!(ThxV4Controller::new(unsupported, UnavailableThxV4Transport).is_err());
    }
}

#[test]
fn ida_type_inference_preserves_unsigned_and_first_array_element_branches() {
    assert_eq!(infer_property_type(&json!(true)), ThxPropertyType::Bool);
    assert_eq!(infer_property_type(&json!(-3)), ThxPropertyType::Int);
    assert_eq!(infer_property_type(&json!(3)), ThxPropertyType::None);
    assert_eq!(infer_property_type(&json!(0)), ThxPropertyType::None);
    assert_eq!(infer_property_type(&json!(u64::MAX)), ThxPropertyType::None);
    assert_eq!(infer_property_type(&json!(3.5)), ThxPropertyType::Float);
    assert_eq!(infer_property_type(&json!("x")), ThxPropertyType::String);
    assert_eq!(
        infer_property_type(&json!(["x", true])),
        ThxPropertyType::StringArray
    );
    assert_eq!(
        infer_property_type(&json!([1.5, "x"])),
        ThxPropertyType::FloatArray
    );
    assert_eq!(
        infer_property_type(&json!([-1, true])),
        ThxPropertyType::Int
    );
    assert_eq!(
        infer_property_type(&json!([true, "x"])),
        ThxPropertyType::Bool
    );
    for value in [
        json!([1, 1.5]),
        json!([]),
        json!([[], "x"]),
        json!(null),
        json!({"value":true}),
    ] {
        assert_eq!(infer_property_type(&value), ThxPropertyType::None);
    }
}

#[test]
fn parsed_values_are_embedded_as_json_with_exact_type_override() {
    let request = ThxPropertyRequest::parse("level", "1", Some("Float")).unwrap();
    assert_eq!(
        request.wire_json(),
        json!({"id":"level","value":1,"type":"Float"})
    );
    assert_eq!(
        ThxPropertyRequest::parse("spatial_enabled", "false", None)
            .unwrap()
            .wire_json(),
        json!({"id":"spatial_enabled","value":false,"type":"Bool"})
    );
    assert!(ThxPropertyRequest::parse("level", "{bad json}", None).is_err());
    assert_eq!(
        ThxPropertyRequest::parse("level", "-2", Some(""))
            .unwrap()
            .property_type,
        ThxPropertyType::Int
    );
    assert!(ThxPropertyRequest::new("level", json!(1), Some(&"x".repeat(100))).is_err());
    assert!(ThxPropertyRequest::new("level", json!(1), Some(&"界".repeat(34))).is_err());
    assert!(ThxPropertyRequest::new("level", json!(1), Some(&"界".repeat(33))).is_ok());
    assert!(ThxPropertyRequest::new("level", json!(1), Some("Float\0Int")).is_err());
}

#[test]
fn identity_and_getter_null_semantics_do_not_invent_defaults() {
    let identity = ThxDeviceIdentity::new("razer_blacksharkv3pro_pc", "headphones_game").unwrap();
    assert_eq!(
        identity.wire_json(),
        json!({"productId":"razer_blacksharkv3pro_pc","endpointId":"headphones_game"})
    );
    assert_eq!(
        identity.cache_key(),
        "razer_blacksharkv3pro_pc_headphones_game"
    );
    assert_eq!(
        ThxDeviceIdentity::new("x", "").unwrap().wire_json(),
        json!({"productId":"x","endpointId":""})
    );
    assert_eq!(ThxDeviceIdentity::new("x", "").unwrap().cache_key(), "x");
    assert!(ThxDeviceIdentity::new("", "").is_err());
    assert!(ThxDeviceIdentity::new("x\0y", "").is_err());
    assert_eq!(decode_native_property_reply(None).unwrap(), None);
    assert_eq!(
        decode_native_property_reply(Some("false")).unwrap(),
        Some(json!(false))
    );
    assert_eq!(
        decode_native_property_reply(Some("null")).unwrap(),
        Some(Value::Null)
    );
    assert!(decode_native_property_reply(Some("not-json")).is_err());
}

#[test]
fn enable_runs_source_before_hook_then_native_spatial_then_added_getter() {
    let mut controller = controller(enable_sequence());
    let task = controller.prepare_spatial(true, presets(), 200);
    assert_eq!(
        controller.state().local_draft.as_ref().unwrap().is_enabled,
        true
    );
    assert_eq!(controller.state().acknowledged_spatial_enabled, None);
    assert_eq!(controller.transport().exchanges.len(), 2);
    let outcome = controller.execute_spatial(&task, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::Verified);
    assert_eq!(outcome.transport_kind, ThxTransportKind::Simulated);
    assert!(outcome.native_acknowledged);
    assert_eq!(outcome.readback, Some(true));
    assert_eq!(
        outcome.operations,
        enable_sequence()
            .into_iter()
            .map(|(op, _)| op)
            .collect::<Vec<_>>()
    );
    assert_eq!(controller.state().acknowledged_spatial_enabled, Some(true));
    assert!(controller.state().valkyrie_spatial_enabled);
    assert_eq!(controller.transport().remaining(), 0);
}

#[test]
fn disabling_restores_all_presets_in_source_order() {
    let disable = vec![
        (set("eq_enabled", false), ThxReply::NativeBool(true)),
        (
            ThxOperation::SetFirmwarePresetEqEnabled(true),
            ThxReply::FirmwareStatus("SUCCESS".into()),
        ),
        (
            ThxOperation::RestoreCloudEqStereo {
                presets: presets(),
                selected_index: None,
            },
            ThxReply::CloudEqBool(true),
        ),
        (set("spatial_enabled", false), ThxReply::NativeBool(true)),
        (
            ThxOperation::GetProperty("spatial_enabled".into()),
            ThxReply::Property(Some(json!(false))),
        ),
    ];
    let mut script = enable_sequence();
    script.extend(disable.clone());
    let mut controller = controller(script);
    let enable = controller.prepare_spatial(true, presets(), 1);
    assert_eq!(
        controller.execute_spatial(&enable, || Ok(())).status,
        SpatialStatus::Verified
    );
    let task = controller.prepare_spatial(false, presets(), 2);
    let outcome = controller.execute_spatial(&task, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::Verified);
    assert_eq!(
        outcome.operations,
        disable.into_iter().map(|(op, _)| op).collect::<Vec<_>>()
    );
    assert_eq!(controller.state().acknowledged_spatial_enabled, Some(false));
}

#[test]
fn same_state_still_runs_eq_and_firmware_but_skips_cloud_restore() {
    let repeated = vec![
        (
            ThxOperation::SetFirmwarePresetEqEnabled(false),
            ThxReply::FirmwareStatus("SUCCESS".into()),
        ),
        (set("eq_enabled", true), ThxReply::NativeBool(true)),
        (set("spatial_enabled", true), ThxReply::NativeBool(true)),
        (
            ThxOperation::GetProperty("spatial_enabled".into()),
            ThxReply::Property(Some(json!(true))),
        ),
    ];
    let mut script = enable_sequence();
    script.extend(repeated.clone());
    let mut controller = controller(script);
    let task = controller.prepare_spatial(true, presets(), 1);
    controller.execute_spatial(&task, || Ok(()));
    let task = controller.prepare_spatial(true, presets(), 2);
    let outcome = controller.execute_spatial(&task, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::Verified);
    assert_eq!(
        outcome.operations,
        repeated.into_iter().map(|(op, _)| op).collect::<Vec<_>>()
    );
}

#[test]
fn firmware_fail_and_cloud_false_are_source_continue_branches() {
    let mut script = enable_sequence();
    script[0].1 = ThxReply::FirmwareStatus("FAIL".into());
    script[2].1 = ThxReply::CloudEqBool(false);
    let mut controller = controller(script);
    let task = controller.prepare_spatial(true, presets(), 1);
    let outcome = controller.execute_spatial(&task, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::Verified);
    assert_eq!(outcome.warnings.len(), 2);
    assert_eq!(outcome.operations.len(), 5);
    assert!(outcome.native_acknowledged);
}

#[test]
fn missing_active_cloud_preset_logs_and_continues_like_source() {
    let script = vec![
        (
            ThxOperation::SetFirmwarePresetEqEnabled(false),
            ThxReply::FirmwareStatus("SUCCESS".into()),
        ),
        (set("eq_enabled", true), ThxReply::NativeBool(true)),
        (set("spatial_enabled", true), ThxReply::NativeBool(true)),
        (
            ThxOperation::GetProperty("spatial_enabled".into()),
            ThxReply::Property(Some(json!(true))),
        ),
    ];
    let mut controller = controller(script);
    let task = controller.prepare_spatial(true, vec![], 1);
    let outcome = controller.execute_spatial(&task, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::Verified);
    assert_eq!(outcome.warnings.len(), 1);
    assert_eq!(outcome.operations.len(), 4);
}

#[test]
fn native_eq_rejection_stops_later_operations_and_preserves_local_draft() {
    let mut script = enable_sequence();
    script[1].1 = ThxReply::NativeBool(false);
    let mut controller = controller(script);
    let task = controller.prepare_spatial(true, presets(), 1);
    let outcome = controller.execute_spatial(&task, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::NativeRejected);
    assert_eq!(outcome.operations.len(), 2);
    assert_eq!(controller.transport().remaining(), 3);
    assert_eq!(
        controller.state().local_draft.as_ref().unwrap().is_enabled,
        true
    );
    assert!(!controller.state().valkyrie_spatial_enabled);
    assert_eq!(controller.state().runtime_spatial_enabled, None);
    assert_eq!(controller.state().acknowledged_spatial_enabled, None);
}

#[test]
fn spatial_rejection_leaves_before_hook_cache_but_no_spatial_ack() {
    let mut script = enable_sequence();
    script[3].1 = ThxReply::NativeBool(false);
    let mut controller = controller(script);
    let task = controller.prepare_spatial(true, presets(), 1);
    let outcome = controller.execute_spatial(&task, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::NativeRejected);
    assert_eq!(outcome.operations.len(), 4);
    assert!(controller.state().valkyrie_spatial_enabled);
    assert_eq!(controller.state().runtime_spatial_enabled, None);
    assert!(!outcome.native_acknowledged);
    assert_eq!(controller.state().acknowledged_spatial_enabled, None);
}

#[test]
fn readback_mismatch_does_not_commit_requested_value_as_acknowledged() {
    let mut script = enable_sequence();
    script[4].1 = ThxReply::Property(Some(json!(false)));
    let mut controller = controller(script);
    let task = controller.prepare_spatial(true, presets(), 1);
    let outcome = controller.execute_spatial(&task, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::ReadbackMismatch);
    assert!(outcome.native_acknowledged);
    assert_eq!(outcome.readback, Some(false));
    assert_eq!(controller.state().runtime_spatial_enabled, Some(true));
    assert_eq!(controller.state().observed_spatial_enabled, Some(false));
    assert_eq!(controller.state().acknowledged_spatial_enabled, None);
    assert_eq!(
        controller.state().local_draft.as_ref().unwrap().is_enabled,
        true
    );
}

#[test]
fn missing_null_and_malformed_getter_do_not_fake_acknowledgement() {
    for reply in [
        ThxReply::Property(None),
        ThxReply::Property(Some(Value::Null)),
        ThxReply::Property(Some(json!({"value":true}))),
        ThxReply::NativeBool(true),
    ] {
        let mut script = enable_sequence();
        script[4].1 = reply;
        let mut controller = controller(script);
        let task = controller.prepare_spatial(true, presets(), 1);
        let outcome = controller.execute_spatial(&task, || Ok(()));
        assert_eq!(outcome.status, SpatialStatus::ReadbackFailed);
        assert!(outcome.native_acknowledged);
        assert_eq!(controller.state().acknowledged_spatial_enabled, None);
    }
}

#[test]
fn transport_exception_stops_before_native_eq_without_rollback_claim() {
    let mut controller = controller(vec![]);
    controller.transport_mut().failure(
        ThxOperation::SetFirmwarePresetEqEnabled(false),
        "driver unavailable",
    );
    let task = controller.prepare_spatial(true, presets(), 1);
    let outcome = controller.execute_spatial(&task, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::TransportFailed);
    assert_eq!(
        outcome.operations,
        vec![ThxOperation::SetFirmwarePresetEqEnabled(false)]
    );
    assert!(!outcome.native_acknowledged);
    assert!(outcome.error.unwrap().contains("driver unavailable"));
}

#[test]
fn cancel_after_first_exchange_stops_additional_writes() {
    let mut controller = controller(enable_sequence());
    let task = controller.prepare_spatial(true, presets(), 1);
    let mut validations = 0;
    let outcome = controller.execute_spatial(&task, || {
        validations += 1;
        if validations == 2 {
            bail!("task canceled")
        } else {
            Ok(())
        }
    });
    assert_eq!(outcome.status, SpatialStatus::Canceled);
    assert_eq!(outcome.operations.len(), 1);
    assert_eq!(controller.transport().remaining(), 4);
    assert_eq!(controller.state().acknowledged_spatial_enabled, None);
    assert_eq!(
        controller.state().local_draft.as_ref().unwrap().is_enabled,
        true
    );
}

#[test]
fn stale_task_and_foreign_identity_do_not_write_or_overwrite_newer_state() {
    let mut controller = controller(enable_sequence());
    let first = controller.prepare_spatial(true, presets(), 1);
    let second = controller.prepare_spatial(false, presets(), 2);
    let before = controller.transport().exchanges.len();
    let outcome = controller.execute_spatial(&first, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::Stale);
    assert_eq!(controller.transport().exchanges.len(), before);
    assert_eq!(
        controller.state().local_draft.as_ref().unwrap().is_enabled,
        false
    );
    assert_eq!(controller.state().last_error, None);
    let mut foreign = controller.observation_scope();
    foreign.device.endpoint_id = "microphone".into();
    assert_eq!(
        controller
            .refresh_spatial(&foreign, second.timer_tick, || Ok(()))
            .status,
        SpatialStatus::Stale
    );
    assert_eq!(controller.transport().exchanges.len(), before);
}

#[test]
fn getter_refresh_is_observation_without_native_write_or_storage_ack() {
    let mut controller = controller(vec![(
        ThxOperation::GetProperty("spatial_enabled".into()),
        ThxReply::Property(Some(json!(true))),
    )]);
    let scope = controller.observation_scope();
    let outcome = controller.refresh_spatial(&scope, 1, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::Observed);
    assert_eq!(outcome.readback, Some(true));
    assert!(!outcome.native_acknowledged);
    assert_eq!(controller.state().runtime_spatial_enabled, None);
    assert_eq!(controller.state().observed_spatial_enabled, Some(true));
    assert_eq!(controller.state().acknowledged_spatial_enabled, None);
    assert_eq!(controller.state().local_draft, None);
}

#[test]
fn canceled_getter_and_old_refresh_cannot_update_observation_or_new_draft() {
    let mut controller = controller(vec![(
        ThxOperation::GetProperty("spatial_enabled".into()),
        ThxReply::Property(Some(json!(true))),
    )]);
    let scope = controller.observation_scope();
    let mut validations = 0;
    let result = controller.refresh_spatial(&scope, 1, || {
        validations += 1;
        if validations == 2 {
            bail!("canceled after getter reply")
        } else {
            Ok(())
        }
    });
    assert_eq!(result.status, SpatialStatus::Canceled);
    assert_eq!(controller.state().runtime_spatial_enabled, None);
    assert_eq!(controller.state().local_draft, None);
    controller.prepare_spatial(false, presets(), 2);
    assert_eq!(
        controller.refresh_spatial(&scope, 1, || Ok(())).status,
        SpatialStatus::Stale
    );
    assert_eq!(
        controller.state().local_draft.as_ref().unwrap().timer_tick,
        2
    );
}

#[test]
fn canceled_final_getter_does_not_commit_setter_ack_as_readback() {
    let mut controller = controller(enable_sequence());
    let task = controller.prepare_spatial(true, presets(), 1);
    let mut validations = 0;
    let outcome = controller.execute_spatial(&task, || {
        validations += 1;
        if validations == 10 {
            bail!("canceled before getter acceptance")
        } else {
            Ok(())
        }
    });
    assert_eq!(outcome.status, SpatialStatus::Canceled);
    assert!(outcome.native_acknowledged);
    assert_eq!(outcome.readback, None);
    assert_eq!(controller.state().acknowledged_spatial_enabled, None);
    assert_eq!(controller.transport().remaining(), 0);
}

#[test]
fn source_binding_has_three_status_attempts_with_virtual_500ms_retries() {
    let script = vec![
        (ThxOperation::AddDevice, ThxReply::NativeBool(true)),
        (ThxOperation::CheckStatus, ThxReply::NativeBool(false)),
        (ThxOperation::CheckStatus, ThxReply::NativeBool(false)),
        (ThxOperation::CheckStatus, ThxReply::NativeBool(true)),
    ];
    let mut controller = ThxV4Controller::new(1398, SimulatedThxV4Transport::new(script)).unwrap();
    let mut delays = vec![];
    controller
        .bind(|delay| {
            delays.push(delay);
            Ok(())
        })
        .unwrap();
    assert_eq!(delays, vec![500, 500]);
    assert!(controller.state().ready);
}

#[test]
fn status_exhaustion_and_add_false_fail_without_ready_binding() {
    let script = vec![
        (ThxOperation::AddDevice, ThxReply::NativeBool(true)),
        (ThxOperation::CheckStatus, ThxReply::NativeBool(false)),
        (ThxOperation::CheckStatus, ThxReply::NativeBool(false)),
        (ThxOperation::CheckStatus, ThxReply::NativeBool(false)),
    ];
    let mut controller = ThxV4Controller::new(1398, SimulatedThxV4Transport::new(script)).unwrap();
    let mut retries = 0;
    assert!(
        controller
            .bind(|delay| {
                assert_eq!(delay, 500);
                retries += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(retries, 3);
    assert!(!controller.state().ready);
    let mut controller = ThxV4Controller::new(
        1398,
        SimulatedThxV4Transport::new([(ThxOperation::AddDevice, ThxReply::NativeBool(false))]),
    )
    .unwrap();
    assert!(controller.bind(|_| unreachable!()).is_err());
    assert!(!controller.state().ready);
}

#[test]
fn unavailable_transport_keeps_explicit_gap_and_never_fakes_success() {
    let mut controller = ThxV4Controller::new(1398, UnavailableThxV4Transport).unwrap();
    assert!(controller.bind(|_| Ok(())).is_err());
    let task = controller.prepare_spatial(true, presets(), 1);
    let outcome = controller.execute_spatial(&task, || Ok(()));
    assert_eq!(outcome.status, SpatialStatus::Unavailable);
    assert_eq!(outcome.transport_kind, ThxTransportKind::Unavailable);
    assert!(outcome.operations.is_empty());
    assert!(!outcome.native_acknowledged);
    assert_eq!(controller.state().acknowledged_spatial_enabled, None);
}

#[test]
fn unconfigured_simulator_errors_and_remove_invalidates_pending_generation() {
    let mut controller = controller(vec![(
        ThxOperation::RemoveDevice,
        ThxReply::NativeBool(true),
    )]);
    let task = controller.prepare_spatial(true, presets(), 1);
    controller.remove().unwrap();
    assert!(!controller.state().ready);
    assert_eq!(
        controller.execute_spatial(&task, || Ok(())).status,
        SpatialStatus::Stale
    );
    let mut simulator = SimulatedThxV4Transport::new([]);
    assert!(
        simulator
            .exchange(
                &ThxDeviceIdentity::new("test", "").unwrap(),
                &ThxOperation::AddDevice
            )
            .is_err()
    );
}

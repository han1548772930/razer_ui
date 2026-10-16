//! Scripted lifecycle exchanges and virtual waits; no target or OS execution.
use super::*;
use std::cell::Cell;

fn setting(id: &str) -> ThxOperation {
    ThxOperation::SetProperty(ThxPropertyRequest::boolean(id, true))
}
fn successful() -> Vec<(ThxOperation, ThxReply)> {
    vec![
        (ThxOperation::AddDevice, ThxReply::LocalBinding(true)),
        (ThxOperation::CheckStatus, ThxReply::LocalReady(true)),
        (setting("upmix_enabled"), ThxReply::RpcAcknowledged),
        (setting("eq_enabled"), ThxReply::RpcAcknowledged),
        (
            setting("spatial_advanced_early_reflections_enabled"),
            ThxReply::RpcAcknowledged,
        ),
        (
            setting("spatial_advanced_late_reverb_enabled"),
            ThxReply::RpcAcknowledged,
        ),
    ]
}
fn controller(script: Vec<(ThxOperation, ThxReply)>) -> ThxV4Controller<SimulatedThxV4Transport> {
    ThxV4Controller::new(1398, SimulatedThxV4Transport::new(script)).unwrap()
}
fn setup(c: &mut ThxV4Controller<SimulatedThxV4Transport>) -> ThxRenderSetupOutcome {
    c.setup_render(
        ThxRenderSetupTrigger::InitialDeviceSetup,
        || Ok(()),
        |_| panic!("Already ready; a setter must not be retried"),
    )
}

#[test]
fn render_setup_all_six_owners_keep_source_settings_separate_from_spatial_state() {
    for pid in [1398, 1401, 2638, 2641, 4124, 4126] {
        let mut c = ThxV4Controller::new(pid, SimulatedThxV4Transport::new(successful())).unwrap();
        let result = setup(&mut c);
        assert_eq!(result.status, ThxRenderSetupStatus::Completed);
        assert!(result.setup_succeeded && result.binding_ready);
        assert!(result.error.is_none() && result.warnings.is_empty());
        assert_eq!(result.receipts.len(), 6);
        assert!(
            result
                .receipts
                .iter()
                .all(|receipt| receipt.error.is_none())
        );
        assert_eq!(c.transport().remaining(), 0);
        assert!(c.state().ready);
        assert_eq!(c.state().runtime_spatial_enabled, None);
        assert_eq!(c.state().observed_spatial_enabled, None);
        assert_eq!(c.state().acknowledged_spatial_enabled, None);
        assert!(c.state().local_draft.is_none());
        assert!(!c.state().valkyrie_spatial_enabled);
    }
}

#[test]
fn render_setup_wire_ids_and_order_match_independently_parsed_current_source() {
    let evidence: Value = serde_json::from_str(include_str!(
        "../../testdata/re/evidence/thx-lifecycle-source-current.json"
    ))
    .unwrap();
    for product in evidence["products"].as_array().unwrap() {
        let pid = product["product_id"].as_u64().unwrap() as u32;
        let mut c = ThxV4Controller::new(pid, SimulatedThxV4Transport::new(successful())).unwrap();
        let result = setup(&mut c);
        let writes = result.receipts[2..]
            .iter()
            .map(|receipt| {
                let ThxOperation::SetProperty(request) = &receipt.operation else {
                    panic!()
                };
                (
                    request.id.clone(),
                    request.value.clone(),
                    request.property_type.clone(),
                )
            })
            .collect::<Vec<_>>();
        let original = product["initialization_calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|call| {
                (
                    call["property_id"].as_str().unwrap().to_owned(),
                    call["requested_boolean"].clone(),
                    ThxPropertyType::Bool,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(writes, original);
        assert_eq!(
            product["actual_native_property_ids"]["setStereoUpmix"],
            "upmix_enabled"
        );
    }
}

#[test]
fn render_setup_false_at_any_setting_logs_and_continues_without_success_ack() {
    for index in 2..6 {
        let mut script = successful();
        script[index].1 = ThxReply::NativeBool(false);
        let mut c = controller(script);
        let result = setup(&mut c);
        assert!(result.setup_succeeded);
        assert_eq!(result.status, ThxRenderSetupStatus::Completed);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(
            result.receipts[index].reply,
            Some(ThxReply::NativeBool(false))
        );
        assert_eq!(result.receipts.len(), 6);
        assert_eq!(c.transport().remaining(), 0);
    }
}

#[test]
fn render_setup_completed_rpc_rejection_preserves_code_and_continues() {
    let mut script = successful();
    script[2].1 = ThxReply::RpcRejected {
        code: 14,
        message: Some("mock unavailable".into()),
    };
    let mut c = controller(script);
    let result = setup(&mut c);
    assert!(result.setup_succeeded);
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].contains("14"));
    assert!(matches!(
        result.receipts[2].reply,
        Some(ThxReply::RpcRejected { code: 14, .. })
    ));
    assert_eq!(result.receipts.len(), 6);
}

#[test]
fn render_setup_exception_stops_remaining_settings_and_keeps_partial_ack() {
    let mut transport = SimulatedThxV4Transport::new(successful()[..3].iter().cloned());
    transport.failure(
        setting("eq_enabled"),
        "mock incomplete RPC with unknown outcome",
    );
    let mut c = ThxV4Controller::new(1398, transport).unwrap();
    let result = setup(&mut c);
    assert!(!result.setup_succeeded);
    assert!(result.binding_ready);
    assert_eq!(result.status, ThxRenderSetupStatus::TransportFailed);
    assert_eq!(result.receipts.len(), 4);
    assert_eq!(result.receipts[2].reply, Some(ThxReply::RpcAcknowledged));
    assert!(result.receipts[3].reply.is_none());
    assert!(
        result.receipts[3]
            .error
            .as_ref()
            .unwrap()
            .contains("unknown outcome")
    );
    assert_eq!(c.transport().exchanges.len(), 4);
}

#[test]
fn render_setup_add_false_never_checks_or_applies_settings() {
    let mut c = controller(vec![(ThxOperation::AddDevice, ThxReply::NativeBool(false))]);
    let result = setup(&mut c);
    assert_eq!(result.status, ThxRenderSetupStatus::SetupRejected);
    assert!(!result.binding_ready && !result.setup_succeeded);
    assert_eq!(result.receipts.len(), 1);
}

#[test]
fn render_setup_three_false_checks_wait_including_final_500ms() {
    let mut script = vec![(ThxOperation::AddDevice, ThxReply::LocalBinding(true))];
    script.extend((0..3).map(|_| (ThxOperation::CheckStatus, ThxReply::LocalReady(false))));
    let mut c = controller(script);
    let mut waits = vec![];
    let result = c.setup_render(
        ThxRenderSetupTrigger::InitialDeviceSetup,
        || Ok(()),
        |ms| {
            waits.push(ms);
            Ok(())
        },
    );
    assert_eq!(result.status, ThxRenderSetupStatus::SetupRejected);
    assert_eq!(waits, vec![500, 500, 500]);
    assert_eq!(result.retry_waits_ms, waits);
    assert_eq!(result.receipts.len(), 4);
    assert!(!c.state().ready);
}

#[test]
fn render_setup_ready_on_third_check_applies_settings_once() {
    let mut script = successful();
    script.splice(
        1..1,
        (0..2).map(|_| (ThxOperation::CheckStatus, ThxReply::NativeBool(false))),
    );
    let mut c = controller(script);
    let mut waits = vec![];
    let result = c.setup_render(
        ThxRenderSetupTrigger::PostRestoreDeviceData,
        || Ok(()),
        |ms| {
            waits.push(ms);
            Ok(())
        },
    );
    assert!(result.setup_succeeded);
    assert_eq!(waits, vec![500, 500]);
    assert_eq!(result.receipts.len(), 8);
    assert_eq!(c.transport().remaining(), 0);
}

#[test]
fn render_setup_force_remove_false_continues_with_distinct_local_binding_receipt() {
    let mut script = vec![(ThxOperation::RemoveDevice, ThxReply::LocalBinding(false))];
    script.extend(successful());
    let mut c = controller(script);
    let result = c.setup_render(
        ThxRenderSetupTrigger::DefaultPlaybackDeviceChanged,
        || Ok(()),
        |_| unreachable!(),
    );
    assert!(result.setup_succeeded);
    assert_eq!(result.receipts.len(), 7);
    assert_eq!(
        result.receipts[0].reply,
        Some(ThxReply::LocalBinding(false))
    );
    assert_eq!(result.warnings.len(), 1);
}

#[test]
fn render_setup_force_remove_exception_prevents_add() {
    let mut transport = SimulatedThxV4Transport::new([]);
    transport.failure(ThxOperation::RemoveDevice, "mock remove exception");
    let mut c = ThxV4Controller::new(1398, transport).unwrap();
    let result = c.setup_render(
        ThxRenderSetupTrigger::DefaultPlaybackDeviceChanged,
        || Ok(()),
        |_| unreachable!(),
    );
    assert_eq!(result.status, ThxRenderSetupStatus::TransportFailed);
    assert_eq!(result.receipts.len(), 1);
    assert!(!result.setup_succeeded);
}

#[test]
fn render_setup_cancel_after_ack_retains_receipt_and_stops_later_setters() {
    let mut c = controller(successful());
    let checks = Cell::new(0);
    let result = c.setup_render(
        ThxRenderSetupTrigger::InitialDeviceSetup,
        || {
            checks.set(checks.get() + 1);
            ensure!(
                checks.get() < 6,
                "mock identity expired after stereo upmix ACK"
            );
            Ok(())
        },
        |_| unreachable!(),
    );
    assert_eq!(result.status, ThxRenderSetupStatus::Canceled);
    assert!(!result.setup_succeeded);
    assert_eq!(result.receipts.len(), 3);
    assert_eq!(result.receipts[2].reply, Some(ThxReply::RpcAcknowledged));
    assert_eq!(c.transport().remaining(), 3);
}

#[test]
fn render_setup_canceled_virtual_wait_does_not_retry_or_write() {
    let mut c = controller(vec![
        (ThxOperation::AddDevice, ThxReply::LocalBinding(true)),
        (ThxOperation::CheckStatus, ThxReply::LocalReady(false)),
    ]);
    let canceled = Cell::new(false);
    let result = c.setup_render(
        ThxRenderSetupTrigger::InitialDeviceSetup,
        || {
            ensure!(!canceled.get(), "mock canceled");
            Ok(())
        },
        |ms| {
            assert_eq!(ms, 500);
            canceled.set(true);
            bail!("mock wait canceled")
        },
    );
    assert_eq!(result.status, ThxRenderSetupStatus::Canceled);
    assert_eq!(result.retry_waits_ms, vec![500]);
    assert_eq!(result.receipts.len(), 2);
}

#[test]
fn render_setup_source_has_no_once_guard_for_successive_restore_callbacks() {
    let mut script = successful();
    script.extend(successful());
    let mut c = controller(script);
    for _ in 0..2 {
        let result = c.setup_render(
            ThxRenderSetupTrigger::PostRestoreDeviceData,
            || Ok(()),
            |_| unreachable!(),
        );
        assert!(result.setup_succeeded);
        assert_eq!(result.receipts.len(), 6);
    }
    assert_eq!(c.transport().exchanges.len(), 12);
    assert_eq!(c.transport().remaining(), 0);
}

#[test]
fn render_setup_unavailable_transport_does_not_invent_initialized_state() {
    let mut c = ThxV4Controller::new(1398, UnavailableThxV4Transport).unwrap();
    let result = c.setup_render(
        ThxRenderSetupTrigger::InitialDeviceSetup,
        || Ok(()),
        |_| unreachable!(),
    );
    assert_eq!(result.status, ThxRenderSetupStatus::Unavailable);
    assert!(!result.setup_succeeded);
    assert!(result.receipts.is_empty());
}

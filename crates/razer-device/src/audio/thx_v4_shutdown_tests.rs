//! Pure protocol/controller checks with scripted transport results only.
use super::*;
use std::cell::Cell;

fn operations() -> Vec<ThxOperation> {
    vec![
        ThxOperation::SetProperty(ThxPropertyRequest::boolean("eq_enabled", false)),
        ThxOperation::SetProperty(ThxPropertyRequest::boolean("spatial_enabled", false)),
        ThxOperation::RemoveDevice,
    ]
}

fn successful() -> Vec<(ThxOperation, ThxReply)> {
    operations()
        .into_iter()
        .zip([
            ThxReply::RpcAcknowledged,
            ThxReply::RpcAcknowledged,
            ThxReply::LocalBinding(true),
        ])
        .collect()
}

fn seed_state<T: ThxV4Transport>(controller: &mut ThxV4Controller<T>) {
    controller.state.ready = true;
    controller.state.local_draft = Some(ThxLocalDraft {
        is_enabled: false,
        timer_tick: 81,
    });
    controller.state.valkyrie_spatial_enabled = true;
    controller.state.runtime_spatial_enabled = Some(true);
    controller.state.observed_spatial_enabled = Some(false);
    controller.state.acknowledged_spatial_enabled = Some(true);
    controller.state.last_error = Some("previous spatial diagnostic".into());
    controller.state.generation = 17;
}

fn controller(script: Vec<(ThxOperation, ThxReply)>) -> ThxV4Controller<SimulatedThxV4Transport> {
    let mut controller = ThxV4Controller::new(1398, SimulatedThxV4Transport::new(script)).unwrap();
    seed_state(&mut controller);
    controller
}

fn assert_preserved<T: ThxV4Transport>(
    controller: &ThxV4Controller<T>,
    before: &ThxV4State,
    ready: bool,
) {
    let mut expected = before.clone();
    expected.ready = ready;
    assert_eq!(controller.state(), &expected);
}

#[test]
fn render_shutdown_sequence_matches_independent_current_source_for_all_six_owners() {
    let evidence: Value = serde_json::from_str(include_str!(
        "../../testdata/re/evidence/thx-shutdown-source-current.json"
    ))
    .unwrap();
    let mut owners = vec![];
    for product in evidence["products"].as_array().unwrap() {
        let product_id = product["product_id"].as_u64().unwrap() as u32;
        owners.push(product_id);
        let source_calls = product["render_shutdown_calls"].as_array().unwrap();
        let original = source_calls
            .iter()
            .map(|call| match call["method"].as_str().unwrap() {
                "removeDevice" => ThxOperation::RemoveDevice,
                _ => ThxOperation::SetProperty(ThxPropertyRequest::boolean(
                    call["property_id"].as_str().unwrap(),
                    call["requested_boolean"].as_bool().unwrap(),
                )),
            })
            .collect::<Vec<_>>();
        let mut c =
            ThxV4Controller::new(product_id, SimulatedThxV4Transport::new(successful())).unwrap();
        let result = c.shutdown_render(|| Ok(()));
        assert!(result.shutdown_completed);
        assert_eq!(
            result
                .receipts
                .iter()
                .map(|receipt| receipt.operation.clone())
                .collect::<Vec<_>>(),
            original
        );
        assert_eq!(c.transport().remaining(), 0);
    }
    assert_eq!(owners, [1398, 1401, 2638, 2641, 4124, 4126]);
}

#[test]
fn render_shutdown_completed_preserves_source_caches_draft_observations_and_diagnostics() {
    let mut c = controller(successful());
    let before = c.state().clone();
    let result = c.shutdown_render(|| Ok(()));
    assert_eq!(result.status, ThxRenderShutdownStatus::Completed);
    assert_eq!(result.transport_kind, ThxTransportKind::Simulated);
    assert!(result.shutdown_completed);
    assert!(result.error.is_none() && result.warnings.is_empty());
    assert!(
        result
            .receipts
            .iter()
            .all(|receipt| receipt.error.is_none())
    );
    assert_eq!(result.receipts.len(), 3);
    assert_eq!(c.transport().remaining(), 0);
    assert_preserved(&c, &before, false);
}

#[test]
fn render_shutdown_native_false_at_each_stage_continues_through_removal() {
    for index in 0..3 {
        let mut script = successful();
        script[index].1 = ThxReply::NativeBool(false);
        let mut c = controller(script);
        let before = c.state().clone();
        let result = c.shutdown_render(|| Ok(()));
        assert_eq!(result.status, ThxRenderShutdownStatus::Completed);
        assert!(result.shutdown_completed);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(
            result.receipts[index].reply,
            Some(ThxReply::NativeBool(false))
        );
        assert_eq!(result.receipts.len(), 3);
        assert_eq!(c.transport().remaining(), 0);
        assert_preserved(&c, &before, false);
    }
}

#[test]
fn render_shutdown_all_false_completes_without_implying_settings_accepted() {
    let mut script = successful();
    script[0].1 = ThxReply::NativeBool(false);
    script[1].1 = ThxReply::NativeBool(false);
    script[2].1 = ThxReply::LocalBinding(false);
    let mut c = controller(script);
    let result = c.shutdown_render(|| Ok(()));
    assert!(result.shutdown_completed);
    assert_eq!(result.warnings.len(), 3);
    assert!(result.receipts.iter().all(|receipt| matches!(
        receipt.reply,
        Some(ThxReply::NativeBool(false) | ThxReply::LocalBinding(false))
    )));
    assert!(!c.state().ready);
}

#[test]
fn render_shutdown_completed_rpc_rejections_remain_distinct_and_continue() {
    let mut script = successful();
    for (index, code) in [7, 14].into_iter().enumerate() {
        script[index].1 = ThxReply::RpcRejected {
            code,
            message: Some(format!("mock status {code}")),
        };
    }
    let mut c = controller(script);
    let before = c.state().clone();
    let result = c.shutdown_render(|| Ok(()));
    assert!(result.shutdown_completed);
    assert_eq!(result.warnings.len(), 2);
    assert!(result.error.is_none());
    assert!(matches!(
        result.receipts[0].reply,
        Some(ThxReply::RpcRejected { code: 7, .. })
    ));
    assert!(matches!(
        result.receipts[1].reply,
        Some(ThxReply::RpcRejected { code: 14, .. })
    ));
    assert_eq!(result.receipts.len(), 3);
    assert_preserved(&c, &before, false);
}

#[test]
fn render_shutdown_transport_failure_at_each_stage_stops_remaining_calls() {
    for index in 0..3 {
        let full = successful();
        let mut transport = SimulatedThxV4Transport::new(full[..index].iter().cloned());
        transport.failure(
            full[index].0.clone(),
            "mock incomplete exchange, effects unknown",
        );
        transport.script.extend(
            full[index + 1..]
                .iter()
                .cloned()
                .map(|(operation, reply)| (operation, Ok(reply))),
        );
        let mut c = ThxV4Controller::new(1398, transport).unwrap();
        seed_state(&mut c);
        let before = c.state().clone();
        let result = c.shutdown_render(|| Ok(()));
        assert_eq!(result.status, ThxRenderShutdownStatus::TransportFailed);
        assert!(!result.shutdown_completed);
        assert_eq!(result.receipts.len(), index + 1);
        assert_eq!(c.transport().remaining(), 2 - index);
        assert!(result.receipts[index].reply.is_none());
        assert!(
            result.receipts[index]
                .error
                .as_ref()
                .unwrap()
                .contains("effects unknown")
        );
        assert!(
            result.receipts[..index]
                .iter()
                .all(|receipt| receipt.reply.is_some())
        );
        assert_preserved(&c, &before, true);
    }
}

#[test]
fn render_shutdown_wrong_setter_response_type_stops_before_spatial_and_remove() {
    let mut script = successful();
    script[0].1 = ThxReply::LocalReady(true);
    let mut c = controller(script);
    let result = c.shutdown_render(|| Ok(()));
    assert_eq!(result.status, ThxRenderShutdownStatus::TransportFailed);
    assert!(!result.shutdown_completed);
    assert_eq!(result.receipts[0].reply, Some(ThxReply::LocalReady(true)));
    assert!(result.error.unwrap().contains("wrong response type"));
    assert_eq!(c.transport().remaining(), 2);
    assert!(c.state().ready);
}

#[test]
fn render_shutdown_removal_rpc_rejection_is_contract_error_not_local_release() {
    let mut script = successful();
    script[2].1 = ThxReply::RpcRejected {
        code: 13,
        message: None,
    };
    let mut c = controller(script);
    let before = c.state().clone();
    let result = c.shutdown_render(|| Ok(()));
    assert_eq!(result.status, ThxRenderShutdownStatus::TransportFailed);
    assert!(!result.shutdown_completed);
    assert_eq!(result.receipts.len(), 3);
    assert!(result.error.unwrap().contains("wrong response type"));
    assert_preserved(&c, &before, true);
}

#[test]
fn render_shutdown_guard_cancellation_before_first_exchange_sends_nothing() {
    let mut c = controller(successful());
    let before = c.state().clone();
    let result = c.shutdown_render(|| bail!("mock close canceled before exchange"));
    assert_eq!(result.status, ThxRenderShutdownStatus::Canceled);
    assert!(!result.shutdown_completed);
    assert!(result.receipts.is_empty());
    assert!(c.transport().exchanges.is_empty());
    assert_preserved(&c, &before, true);
}

#[test]
fn render_shutdown_guard_after_each_reply_keeps_receipt_and_stops_next_exchange() {
    for index in 0..3 {
        let mut c = controller(successful());
        let before = c.state().clone();
        let checks = Cell::new(0);
        let result = c.shutdown_render(|| {
            checks.set(checks.get() + 1);
            ensure!(
                checks.get() != (index + 1) * 2,
                "mock identity expired after reply"
            );
            Ok(())
        });
        assert_eq!(result.status, ThxRenderShutdownStatus::Canceled);
        assert!(!result.shutdown_completed);
        assert_eq!(result.receipts.len(), index + 1);
        assert_eq!(
            result.receipts[index].reply,
            Some(successful()[index].1.clone())
        );
        assert!(
            result.receipts[index]
                .error
                .as_ref()
                .unwrap()
                .contains("identity expired")
        );
        assert_eq!(c.transport().remaining(), 2 - index);
        assert_preserved(&c, &before, index != 2);
    }
}

#[test]
fn render_shutdown_guard_before_second_exchange_does_not_invent_attempted_receipt() {
    let mut c = controller(successful());
    let checks = Cell::new(0);
    let result = c.shutdown_render(|| {
        checks.set(checks.get() + 1);
        ensure!(checks.get() != 3, "mock canceled before spatial exchange");
        Ok(())
    });
    assert_eq!(result.status, ThxRenderShutdownStatus::Canceled);
    assert_eq!(result.receipts.len(), 1);
    assert_eq!(result.receipts[0].reply, Some(ThxReply::RpcAcknowledged));
    assert!(result.receipts[0].error.is_none());
    assert_eq!(c.transport().remaining(), 2);
}

struct InterruptedTransport {
    prefix: SimulatedThxV4Transport,
    interrupted_operation: ThxOperation,
    received_reply: ThxReply,
    exchanges: Vec<ThxOperation>,
}
impl ThxV4Transport for InterruptedTransport {
    fn kind(&self) -> ThxTransportKind {
        ThxTransportKind::Simulated
    }
    fn exchange(
        &mut self,
        identity: &ThxDeviceIdentity,
        operation: &ThxOperation,
    ) -> Result<ThxReply> {
        self.exchanges.push(operation.clone());
        if operation == &self.interrupted_operation {
            Err(anyhow::Error::new(ThxExchangeInterrupted {
                reply: self.received_reply.clone(),
                error: "mock scope expired inside transport after reply".into(),
            })
            .context("mock direct guarded exchange"))
        } else {
            self.prefix.exchange(identity, operation)
        }
    }
}
fn interrupted_at(index: usize, reply: ThxReply) -> ThxV4Controller<InterruptedTransport> {
    let mut c = ThxV4Controller::new(
        1398,
        InterruptedTransport {
            prefix: SimulatedThxV4Transport::new(successful()[..index].iter().cloned()),
            interrupted_operation: operations()[index].clone(),
            received_reply: reply,
            exchanges: vec![],
        },
    )
    .unwrap();
    seed_state(&mut c);
    c
}

#[test]
fn render_shutdown_interrupted_exchange_preserves_received_ack_and_transport_error() {
    let mut c = interrupted_at(0, ThxReply::RpcAcknowledged);
    let before = c.state().clone();
    let result = c.shutdown_render(|| Ok(()));
    assert_eq!(result.status, ThxRenderShutdownStatus::TransportFailed);
    assert!(!result.shutdown_completed);
    assert_eq!(result.receipts.len(), 1);
    assert_eq!(result.receipts[0].reply, Some(ThxReply::RpcAcknowledged));
    assert!(
        result.receipts[0]
            .error
            .as_ref()
            .unwrap()
            .contains("inside transport")
    );
    assert_eq!(c.transport().exchanges.len(), 1);
    assert_preserved(&c, &before, true);
}

#[test]
fn render_shutdown_interrupted_rpc_rejection_does_not_resume_false_continuation() {
    let reply = ThxReply::RpcRejected {
        code: 14,
        message: Some("mock rejected".into()),
    };
    let mut c = interrupted_at(1, reply.clone());
    let result = c.shutdown_render(|| Ok(()));
    assert_eq!(result.status, ThxRenderShutdownStatus::TransportFailed);
    assert!(!result.shutdown_completed);
    assert_eq!(result.receipts.len(), 2);
    assert_eq!(result.receipts[1].reply, Some(reply));
    assert!(result.receipts[1].error.is_some());
    assert!(result.warnings.is_empty());
    assert_eq!(c.transport().exchanges.len(), 2);
    assert!(c.state().ready);
}

#[test]
fn render_shutdown_received_removal_before_transport_guard_error_invalidates_ownership_only() {
    let mut c = interrupted_at(2, ThxReply::LocalBinding(false));
    let before = c.state().clone();
    let result = c.shutdown_render(|| Ok(()));
    assert_eq!(result.status, ThxRenderShutdownStatus::TransportFailed);
    assert!(!result.shutdown_completed);
    assert_eq!(result.receipts.len(), 3);
    assert_eq!(
        result.receipts[2].reply,
        Some(ThxReply::LocalBinding(false))
    );
    assert!(result.receipts[2].error.is_some());
    assert_preserved(&c, &before, false);
}

#[test]
fn render_shutdown_unavailable_transport_never_reports_completed_or_changes_state() {
    let mut c = ThxV4Controller::new(1398, UnavailableThxV4Transport).unwrap();
    seed_state(&mut c);
    let before = c.state().clone();
    let result = c.shutdown_render(|| panic!("Unavailable transport must send nothing"));
    assert_eq!(result.status, ThxRenderShutdownStatus::Unavailable);
    assert!(!result.shutdown_completed);
    assert!(result.receipts.is_empty());
    assert!(result.error.is_some());
    assert_preserved(&c, &before, true);
}

#[test]
fn render_shutdown_repeated_explicit_cleanup_does_not_rebind_or_skip_source_calls() {
    let mut script = successful();
    let mut repeated = successful();
    repeated[2].1 = ThxReply::LocalBinding(false);
    script.extend(repeated);
    let mut c = controller(script);
    assert!(c.shutdown_render(|| Ok(())).shutdown_completed);
    let second = c.shutdown_render(|| Ok(()));
    assert!(second.shutdown_completed);
    assert_eq!(second.warnings.len(), 1);
    assert_eq!(second.receipts.len(), 3);
    assert_eq!(c.transport().exchanges.len(), 6);
    assert_eq!(c.transport().remaining(), 0);
}

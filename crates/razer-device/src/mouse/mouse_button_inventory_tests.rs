use super::*;
use crate::feature_exchange::tests::ExchangeProbe;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

#[derive(Debug)]
struct ReadOwnerRetired;

impl std::fmt::Display for ReadOwnerRetired {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("inventory owner retired after native GET")
    }
}

impl std::error::Error for ReadOwnerRetired {}

enum Step {
    Send(Vec<u8>),
    Get(Vec<u8>),
    SendError(Vec<u8>),
}
struct Transport {
    steps: RefCell<VecDeque<Step>>,
}
impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
        }
    }
    fn finish(&self) {
        assert!(self.steps.borrow().is_empty(), "unconsumed simulated I/O");
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        match self.steps.borrow_mut().pop_front().expect("unexpected OUT") {
            Step::Send(expected) => {
                assert_eq!(report, expected);
                Ok(())
            }
            Step::SendError(expected) => {
                assert_eq!(report, expected);
                Err(crate::backend::native_io_failure(
                    crate::backend::NativeIoOutcome::Rejected,
                    anyhow::anyhow!("simulated button failure"),
                ))
            }
            _ => panic!("OUT where IN was expected"),
        }
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        let Step::Get(expected) = self.steps.borrow_mut().pop_front().expect("unexpected IN")
        else {
            panic!("IN where OUT was expected")
        };
        report.copy_from_slice(&expected);
        Ok(91)
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
fn cap(product: u32) -> DeviceReadCapability {
    let mut value = crate::device_reads::capability(product).unwrap().clone();
    value.sleep_between_out_ms = 0;
    value.sleep_between_out_in_ms = 0;
    value.sleep_between_in_ms = 0;
    value.max_retry_out = 1;
    value.max_retry_in = 1;
    value
}
fn txs() -> impl FnMut() -> Result<u8> {
    let mut next = 0;
    move || {
        next = next % 30 + 1;
        Ok(next)
    }
}
fn wire(target: ButtonTarget, transaction: u8, response: bool) -> Vec<u8> {
    let mut packet = vec![0; 91];
    packet[1] = if response { 2 } else { 0 };
    packet[2] = transaction;
    packet[6..9].copy_from_slice(&[10, 2, 140]);
    let header = [target.profile_id, target.button_id, target.mode.raw()];
    packet[9..12].copy_from_slice(&header);
    if response {
        packet[12..19].copy_from_slice(&[0xee, 3, 1, 2, 3, 0xa5, 0x5a]);
    }
    packet[89] = packet[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    packet
}
fn fixture(product: u32, profile: u8, fail: Option<usize>) -> Vec<Step> {
    profile_targets(product, profile)
        .unwrap()
        .into_iter()
        .enumerate()
        .flat_map(|(i, target)| {
            let transaction = (i % 30 + 1) as u8;
            if fail == Some(i) {
                vec![Step::SendError(wire(target, transaction, false))]
            } else {
                vec![
                    Step::Send(wire(target, transaction, false)),
                    Step::Get(wire(target, transaction, true)),
                ]
            }
        })
        .collect()
}

#[test]
fn current_inventory_equals_independently_audited_final_product_defaults() {
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../testdata/re/mouse-button-inventory-current-source.json"
    ))
    .unwrap();
    for product in source["products"].as_array().unwrap() {
        let id = product["product_id"].as_u64().unwrap() as u32;
        let cap = capability(id).unwrap();
        let expected: Vec<_> = product["firmware_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        assert_eq!(cap.firmware_ids, expected);
        assert_eq!(
            cap.inputs.len(),
            product["inputs"].as_array().unwrap().len()
        );
        for (actual, input) in cap.inputs.iter().zip(product["inputs"].as_array().unwrap()) {
            assert_eq!(actual.input_id, input["input_id"].as_str().unwrap());
            assert_eq!(
                actual.firmware_id,
                input["firmware_id"].as_u64().map(|v| v as u8)
            );
            assert_eq!(
                actual.mode,
                if input["mode"] == "NormalMode" {
                    AssignmentMode::Normal
                } else {
                    AssignmentMode::Hypershift
                }
            );
        }
    }
}

#[test]
fn product_overrides_resolve_actual_hypershift_and_ai_firmware_targets() {
    assert_eq!(
        target(226, 3, "DKM_HYPERSHIFT", AssignmentMode::Normal)
            .unwrap()
            .button_id,
        57
    );
    assert_eq!(
        target(226, 3, "DKM_AI_PROMPT", AssignmentMode::Hypershift)
            .unwrap()
            .button_id,
        128
    );
    for product in [170, 204, 45066] {
        assert!(target(product, 3, "DKM_HYPERSHIFT", AssignmentMode::Normal).is_err());
        assert_eq!(
            target(product, 3, "DKM_SB_02", AssignmentMode::Normal)
                .unwrap()
                .button_id,
            15
        );
    }
    assert!(target(226, 3, "DKM_SB_02", AssignmentMode::Normal).is_err());
}

#[test]
fn generic_only_inputs_are_rejected_but_actual_hypershift_only_inputs_resolve() {
    assert!(target(170, 2, "DKM_M_01", AssignmentMode::Normal).is_err());
    assert!(target(170, 2, "RepeatScrollLeft", AssignmentMode::Normal).is_err());
    assert!(target(170, 2, "DKM_M_03", AssignmentMode::Normal).is_err());
    assert_eq!(
        target(170, 2, "DKM_M_03", AssignmentMode::Hypershift)
            .unwrap()
            .button_id,
        6
    );
    // Hypershift-only rows do not add IDs to the source scan inventory.
    assert!(!capability(170).unwrap().firmware_ids.contains(&6));
    assert!(!capability(170).unwrap().firmware_ids.contains(&104));
}

#[test]
fn profile_targets_use_unique_sorted_ids_for_each_complete_mode_pass() {
    for product in [170, 204, 226, 45066] {
        let targets = profile_targets(product, 7).unwrap();
        let n = capability(product).unwrap().firmware_ids.len();
        assert_eq!(targets.len(), n * 2);
        assert!(targets.iter().all(|t| t.profile_id == 7));
        assert!(
            targets[..n]
                .iter()
                .all(|t| t.mode == AssignmentMode::Normal)
        );
        assert!(
            targets[n..]
                .iter()
                .all(|t| t.mode == AssignmentMode::Hypershift)
        );
        assert!(
            targets[..n]
                .windows(2)
                .all(|pair| pair[0].button_id < pair[1].button_id)
        );
        assert_eq!(
            targets[..n].iter().map(|t| t.button_id).collect::<Vec<_>>(),
            targets[n..].iter().map(|t| t.button_id).collect::<Vec<_>>()
        );
    }
}

#[test]
fn full_profile_reads_exact_source_order_for_all_four_products() {
    for product in [170, 204, 226, 45066] {
        let transport = Transport::new(fixture(product, 3, None));
        let cap = cap(product);
        let report = MouseButtonInventory::new(&transport, &cap)
            .unwrap()
            .read_profile(3, txs(), || Ok(()))
            .unwrap();
        assert!(report.complete());
        assert!(report.source_scan_completed);
        for entry in &report.entries {
            assert!(entry.attempted);
            assert!(entry.error.is_none());
            let observed = entry.observed.as_ref().unwrap();
            assert_eq!(observed.assignment.target, entry.target);
            assert_eq!(observed.assignment.function_id, 0xee);
            assert_eq!(observed.assignment.data_size, 3);
            assert_eq!(observed.raw_data, [1, 2, 3, 0xa5, 0x5a]);
            assert!(!entry.input_ids.is_empty());
        }
        transport.finish();
    }
}

#[test]
fn source_per_button_failure_continues_all_other_reads_and_returns_aggregate_progress() {
    let transport = Transport::new(fixture(170, 2, Some(2)));
    let cap = cap(170);
    let failure = MouseButtonInventory::new(&transport, &cap)
        .unwrap()
        .read_profile(2, txs(), || Ok(()))
        .expect_err("partial read must not be accepted");
    assert_eq!(failure.phase, ButtonInventoryFailurePhase::Read);
    assert!(failure.progress.source_scan_completed);
    assert!(!failure.progress.complete());
    assert_eq!(
        failure
            .progress
            .entries
            .iter()
            .filter(|entry| entry.observed.is_some())
            .count(),
        25
    );
    let failed = &failure.progress.entries[2];
    assert!(failed.attempted);
    assert!(failed.observed.is_none());
    assert!(
        failed
            .error
            .as_ref()
            .unwrap()
            .contains("simulated button failure")
    );
    assert_eq!(failure.failed_target, Some(failed.target));
    transport.finish();
}

#[test]
fn invalid_transactions_are_unattempted_and_never_replace_device_observations() {
    let transport = Transport::new([]);
    let cap = cap(170);
    let failure = MouseButtonInventory::new(&transport, &cap)
        .unwrap()
        .read_profile(2, || Ok(31), || Ok(()))
        .expect_err("invalid transactions must fail");
    assert_eq!(failure.phase, ButtonInventoryFailurePhase::Read);
    assert!(failure.progress.source_scan_completed);
    assert!(
        failure
            .progress
            .entries
            .iter()
            .all(|entry| !entry.attempted && entry.observed.is_none() && entry.error.is_some())
    );
    transport.finish();
}

#[test]
fn cancellation_retains_completed_read_and_stops_remaining_modes() {
    let transport = Transport::new(fixture(170, 2, None).into_iter().take(2));
    let canceled = Cell::new(false);
    let after_read = || canceled.set(true);
    let device = ExchangeProbe::with_on_finish(&transport, &after_read);
    let cap = cap(170);
    let failure = MouseButtonInventory::new(&device, &cap)
        .unwrap()
        .read_profile(2, txs(), || {
            anyhow::ensure!(!canceled.get(), "cancel before next button");
            Ok(())
        })
        .expect_err("cancel must stop later I/O");
    assert_eq!(failure.phase, ButtonInventoryFailurePhase::Cancelled);
    assert!(crate::backend::exchange_release_completed(&failure.error));
    assert!(!failure.progress.source_scan_completed);
    assert!(!failure.progress.complete());
    assert!(failure.progress.entries[0].attempted);
    assert!(failure.progress.entries[0].observed.is_some());
    assert!(failure.progress.entries[0].error.is_some());
    assert_eq!(
        failure.failed_target,
        Some(failure.progress.entries[0].target)
    );
    assert!(!failure.progress.entries[1].attempted);
    assert!(failure.progress.entries[1].error.is_none());
    assert!(
        failure.progress.entries[2..]
            .iter()
            .all(|entry| !entry.attempted && entry.observed.is_none() && entry.error.is_none())
    );
    transport.finish();
}

#[test]
fn release_failure_stops_at_the_current_target_without_losing_its_assignment() {
    let transport = Transport::new(fixture(170, 2, None).into_iter().take(2));
    let device = ExchangeProbe::failing_finish(&transport);
    let cap = cap(170);
    let failure = MouseButtonInventory::new(&device, &cap)
        .unwrap()
        .read_profile(2, txs(), || Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, ButtonInventoryFailurePhase::Read);
    assert!(crate::backend::exchange_finalization_failed(&failure.error));
    assert!(!crate::backend::exchange_release_completed(&failure.error));
    assert!(!failure.progress.source_scan_completed);
    assert!(!failure.progress.complete());
    let failed = &failure.progress.entries[0];
    assert_eq!(failure.failed_target, Some(failed.target));
    assert!(failed.attempted);
    assert!(failed.error.is_some());
    let observed = failed.observed.as_ref().unwrap();
    assert_eq!(observed.assignment.target, failed.target);
    assert_eq!(observed.assignment.function_id, 0xee);
    assert_eq!(observed.raw_data, [1, 2, 3, 0xa5, 0x5a]);
    assert!(
        failure.progress.entries[1..]
            .iter()
            .all(|entry| { !entry.attempted && entry.observed.is_none() && entry.error.is_none() })
    );
    transport.finish();
}

#[test]
fn cancellation_during_get_keeps_the_complete_assignment_on_the_failed_target() {
    let transport = Transport::new(fixture(170, 2, None).into_iter().take(2));
    let cap = cap(170);
    let failure = MouseButtonInventory::new(&transport, &cap)
        .unwrap()
        .read_profile(2, txs(), || {
            if transport.steps.borrow().is_empty() {
                return Err(ReadOwnerRetired.into());
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(failure.phase, ButtonInventoryFailurePhase::Cancelled);
    assert!(failure.error.is::<ReadOwnerRetired>());
    assert!(!failure.progress.source_scan_completed);
    assert!(!failure.progress.complete());
    let failed = &failure.progress.entries[0];
    assert_eq!(failure.failed_target, Some(failed.target));
    assert!(failed.attempted);
    assert!(failed.error.is_some());
    let observed = failed.observed.as_ref().unwrap();
    assert_eq!(observed.assignment.target, failed.target);
    assert_eq!(observed.assignment.function_id, 0xee);
    assert_eq!(observed.assignment.data_size, 3);
    assert_eq!(observed.raw_data, [1, 2, 3, 0xa5, 0x5a]);
    assert!(
        failure.progress.entries[1..]
            .iter()
            .all(|entry| { !entry.attempted && entry.observed.is_none() && entry.error.is_none() })
    );
    transport.finish();
}

#[test]
fn canceled_get_does_not_promote_invalid_assignment_payloads_into_inventory_observations() {
    let first = profile_targets(170, 2).unwrap()[0];
    for invalid in 0..3 {
        let mut incoming = wire(first, 1, true);
        match invalid {
            0 => incoming[9] = 3,
            1 => incoming[11] = 2,
            2 => incoming[6] = 9,
            _ => unreachable!(),
        }
        let transport = Transport::new([Step::Send(wire(first, 1, false)), Step::Get(incoming)]);
        let cap = cap(170);
        let failure = MouseButtonInventory::new(&transport, &cap)
            .unwrap()
            .read_profile(2, txs(), || {
                if transport.steps.borrow().is_empty() {
                    return Err(ReadOwnerRetired.into());
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(failure.phase, ButtonInventoryFailurePhase::Cancelled);
        assert!(failure.error.is::<ReadOwnerRetired>());
        assert!(crate::CompletedFeatureExchange::from_error(&failure.error).is_some());
        assert!(CompletedButtonRead::from_error(&failure.error).is_none());
        assert_eq!(failure.failed_target, Some(first));
        assert!(!failure.progress.complete());
        assert!(!failure.progress.source_scan_completed);
        assert!(failure.progress.entries[0].attempted);
        assert!(failure.progress.entries[0].observed.is_none());
        assert!(failure.progress.entries[0].error.is_some());
        assert!(failure.progress.entries[1..].iter().all(|entry| {
            !entry.attempted && entry.observed.is_none() && entry.error.is_none()
        }));
        transport.finish();
    }
}

#[test]
fn invalid_profile_and_transport_are_rejected_before_any_io() {
    let transport = Transport::new([]);
    assert!(profile_targets(170, 0).is_err());
    assert!(profile_targets(999, 1).is_err());
    let mut cap = cap(170);
    let failure = MouseButtonInventory::new(&transport, &cap)
        .unwrap()
        .read_profile(0, txs(), || Ok(()))
        .expect_err("zero profile must fail");
    assert_eq!(failure.phase, ButtonInventoryFailurePhase::Prepare);
    assert!(failure.progress.entries.is_empty());
    cap.claim_interface = 3;
    assert!(MouseButtonInventory::new(&transport, &cap).is_err());
    transport.finish();
}

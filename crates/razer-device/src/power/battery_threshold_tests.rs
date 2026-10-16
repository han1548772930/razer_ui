//! Strict synthetic exchanges drive production codecs; no native I/O is opened.
use super::*;
use std::{cell::RefCell, collections::VecDeque};

enum Step {
    Send(Vec<u8>, bool),
    Get(Vec<u8>, usize),
    ReadError,
}

struct Transport {
    steps: RefCell<VecDeque<Step>>,
    sent: RefCell<Vec<Vec<u8>>>,
    reads: RefCell<usize>,
}

impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
            sent: RefCell::new(Vec::new()),
            reads: RefCell::new(0),
        }
    }
    fn finish(&self) {
        assert!(
            self.steps.borrow().is_empty(),
            "Required exchanges were not consumed"
        );
    }
}

impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        let Some(Step::Send(expected, success)) = self.steps.borrow_mut().pop_front() else {
            anyhow::bail!("Unexpected Feature OUT");
        };
        assert_eq!(
            report, expected,
            "Every request byte must match the source fixture"
        );
        self.sent.borrow_mut().push(report.to_vec());
        ensure!(success, "Synthetic send failure");
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        *self.reads.borrow_mut() += 1;
        match self.steps.borrow_mut().pop_front() {
            Some(Step::Get(reply, count)) => {
                assert_eq!(report.len(), 91);
                // Interface selection is checked across products in device_query_tests.
                assert_eq!(&report[1..], [0; 90]);
                report[..reply.len()].copy_from_slice(&reply);
                Ok(count)
            }
            Some(Step::ReadError) => anyhow::bail!("Synthetic read failure"),
            _ => anyhow::bail!("Unexpected Feature IN"),
        }
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn packet(header: [u8; 3], payload: &[u8], txn: u8) -> Vec<u8> {
    // Independent source packet fixture: ID/status/transaction/remaining/reserved,
    // data size/class/command, 80 payload bytes, XOR checksum and reserved tail.
    let mut bytes = vec![0u8; 91];
    bytes[2] = txn;
    bytes[6..9].copy_from_slice(&header);
    bytes[9..9 + payload.len()].copy_from_slice(payload);
    bytes[89] = bytes[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    bytes
}

fn reply(header: [u8; 3], payload: &[u8], txn: u8, status: u8) -> Vec<u8> {
    let mut bytes = packet(header, payload, txn);
    bytes[1] = status;
    bytes[6] = payload.len() as u8;
    bytes
}

fn send(header: [u8; 3], payload: &[u8], txn: u8) -> Step {
    Step::Send(packet(header, payload, txn), true)
}
fn receive(header: [u8; 3], payload: &[u8], txn: u8, status: u8) -> Step {
    Step::Get(reply(header, payload, txn, status), 91)
}

fn cap(pid: u32) -> DeviceReadCapability {
    let mut cap = crate::device_reads::capability(pid).unwrap().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_out = 1;
    cap.max_retry_in = 1;
    cap
}

fn transactions() -> impl FnMut() -> anyhow::Result<u8> {
    let mut next = 0;
    move || {
        next += 1;
        Ok(next)
    }
}

#[test]
fn percentages_use_source_ceiling_then_floor_and_preserve_raw_observations() {
    assert_eq!(encode_percent(1).unwrap(), 3);
    assert_eq!(encode_percent(5).unwrap(), 13);
    assert_eq!(encode_percent(10).unwrap(), 26);
    assert_eq!(encode_percent(50).unwrap(), 128);
    assert_eq!(encode_percent(100).unwrap(), 255);
    for percent in 0..=100 {
        assert_eq!(decode(encode_percent(percent).unwrap()).percent, percent);
    }
    for raw in 0..=255 {
        assert_eq!(decode(raw).raw_value, raw);
    }
    assert_eq!(decode(127).percent, 49);
    assert_eq!(decode(128).percent, 50);
    assert!(encode_percent(101).is_err());
    assert!(encode_percent(255).is_err());
}

#[test]
fn every_audited_product_runs_setter_ack_then_independent_readback() {
    for &pid in AUDITED_PRODUCTS {
        let device = Transport::new([
            send(SET, &[13], 1),
            receive(SET, &[13], 1, 2),
            send(GET, &[], 2),
            receive(GET, &[13], 2, 2),
        ]);
        let result = apply(&device, &cap(pid), 5, transactions(), || Ok(())).unwrap();
        assert!(result.verified);
        assert_eq!(result.acknowledged, decode(13));
        assert_eq!(result.observed, decode(13));
        assert_eq!(result.requested_percent, 5);
        device.finish();
    }
}

#[test]
fn acknowledgement_data_does_not_replace_the_source_getter_confirmation() {
    let device = Transport::new([
        send(SET, &[26], 1),
        receive(SET, &[0], 1, 2),
        send(GET, &[], 2),
        receive(GET, &[26], 2, 2),
    ]);
    let result = apply(&device, &cap(182), 10, transactions(), || Ok(())).unwrap();
    assert_eq!(result.acknowledged.percent, 0);
    assert_eq!(result.observed.percent, 10);
    assert!(result.verified);
    device.finish();
}

#[test]
fn stale_readback_is_unconfirmed_and_does_not_trigger_an_extra_write() {
    let device = Transport::new([
        send(SET, &[26], 1),
        receive(SET, &[26], 1, 2),
        send(GET, &[], 2),
        receive(GET, &[13], 2, 2),
    ]);
    let error = apply(&device, &cap(182), 10, transactions(), || Ok(())).unwrap_err();
    assert!(error.to_string().contains("readback differs"));
    assert_eq!(device.sent.borrow().len(), 2);
    device.finish();
}

#[test]
fn failed_readback_keeps_prior_acknowledgement_separate_from_verified_success() {
    let device = Transport::new([
        send(SET, &[26], 1),
        receive(SET, &[26], 1, 2),
        send(GET, &[], 2),
        Step::ReadError,
    ]);
    let error = apply(&device, &cap(182), 10, transactions(), || Ok(())).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("acknowledged, but readback is unconfirmed")
    );
    device.finish();
}

#[test]
fn canceled_after_ack_stops_before_allocating_or_sending_a_getter() {
    let device = Transport::new([send(SET, &[26], 1), receive(SET, &[26], 1, 2)]);
    let error = apply(&device, &cap(182), 10, transactions(), || {
        ensure!(
            *device.reads.borrow() == 0,
            "Synthetic session canceled after ACK"
        );
        Ok(())
    })
    .unwrap_err();
    assert!(format!("{error:#}").contains("canceled"));
    assert_eq!(device.sent.borrow().len(), 1);
    device.finish();
}

#[test]
fn invalid_settings_product_classes_and_transactions_never_send() {
    let device = Transport::new([]);
    assert!(apply(&device, &cap(182), 101, transactions(), || Ok(())).is_err());
    let mut unsupported = cap(182);
    unsupported.product_id = 190;
    assert!(apply(&device, &unsupported, 10, transactions(), || Ok(())).is_err());
    let mut wrong = cap(182);
    wrong.source_class = "rzDevice30".into();
    assert!(read(&device, &wrong, 1, || Ok(())).is_err());
    for transaction in [31, 128, 255] {
        assert!(read(&device, &cap(182), transaction, || Ok(())).is_err());
    }
    assert!(
        read(&device, &cap(182), 1, || anyhow::bail!(
            "Canceled before submit"
        ))
        .is_err()
    );
    assert!(device.sent.borrow().is_empty());
    device.finish();
}

#[test]
fn busy_status_retries_input_without_resending_a_mutation() {
    let mut cap = cap(182);
    cap.max_retry_in = 2;
    let device = Transport::new([
        send(SET, &[26], 1),
        receive(SET, &[], 1, 1),
        receive(SET, &[26], 1, 2),
        send(GET, &[], 2),
        receive(GET, &[26], 2, 2),
    ]);
    assert!(
        apply(&device, &cap, 10, transactions(), || Ok(()))
            .unwrap()
            .verified
    );
    assert_eq!(device.sent.borrow().len(), 2);
    device.finish();
}

#[test]
fn wrong_transaction_or_command_retries_the_same_query_with_a_finite_budget() {
    let mut cap = cap(182);
    cap.max_retry_out = 2;
    for first in [receive(GET, &[26], 9, 2), receive(SET, &[26], 1, 2)] {
        let device = Transport::new([
            send(GET, &[], 1),
            first,
            send(GET, &[], 1),
            receive(GET, &[26], 1, 2),
        ]);
        assert_eq!(read(&device, &cap, 1, || Ok(())).unwrap().percent, 10);
        assert_eq!(device.sent.borrow().len(), 2);
        device.finish();
    }
}

#[test]
fn partial_and_zero_payload_responses_fail_without_padding() {
    for incoming in [
        Step::Get(reply(GET, &[26], 1, 2)[..90].to_vec(), 90),
        receive(GET, &[], 1, 2),
    ] {
        let device = Transport::new([send(GET, &[], 1), incoming]);
        assert!(read(&device, &cap(182), 1, || Ok(())).is_err());
        device.finish();
    }
}

#[test]
fn host_strips_returned_prefix_before_decoding_threshold() {
    let mut bytes = reply(GET, &[26], 1, 2);
    bytes[0] = 1;
    let device = Transport::new([send(GET, &[], 1), Step::Get(bytes, 91)]);
    assert_eq!(read(&device, &cap(182), 1, || Ok(())).unwrap().percent, 10);
    device.finish();
}

#[test]
fn unsupported_status_is_terminal_and_source_send_failure_retries_are_bounded() {
    let mut cap = cap(182);
    cap.max_retry_out = 2;
    let device = Transport::new([send(GET, &[], 1), receive(GET, &[], 1, 5)]);
    assert!(
        read(&device, &cap, 1, || Ok(()))
            .unwrap_err()
            .to_string()
            .contains("unsupported")
    );
    device.finish();
    let device = Transport::new([
        Step::Send(packet(GET, &[], 1), false),
        Step::Send(packet(GET, &[], 1), false),
    ]);
    assert!(
        read(&device, &cap, 1, || Ok(()))
            .unwrap_err()
            .to_string()
            .contains("retries exhausted")
    );
    device.finish();
}

#[test]
fn capabilities_match_reverified_current_source_including_changed_bundle() {
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../testdata/re/battery-threshold-current-source.json"
    ))
    .unwrap();
    let products: Vec<u32> = source["products"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["product_id"].as_u64().unwrap() as u32)
        .collect();
    assert_eq!(products, AUDITED_PRODUCTS);
    for p in source["products"].as_array().unwrap() {
        assert_eq!(p["get_command"], serde_json::json!(GET));
        assert_eq!(p["set_command"], serde_json::json!(SET));
    }
    assert!(source["gaps"].as_array().unwrap().is_empty());
    let current = &source["products"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["product_id"] == 222)
        .unwrap();
    assert_eq!(
        current["independent_current_source_audit"],
        "docs/re/mouse-222-device-current-source.json"
    );
}

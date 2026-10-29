use super::*;
use crate::feature_exchange::tests::ExchangeProbe;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};
enum Step {
    Send(Vec<u8>),
    Get(Vec<u8>),
    SendError(Vec<u8>),
    GetError,
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
        assert!(self.steps.borrow().is_empty());
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        match self.steps.borrow_mut().pop_front().expect("unexpected OUT") {
            Step::Send(expected) => assert_eq!(report, expected),
            Step::SendError(expected) => {
                assert_eq!(report, expected);
                anyhow::bail!("simulated OUT failure")
            }
            _ => panic!("OUT where IN expected"),
        };
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        match self.steps.borrow_mut().pop_front().expect("unexpected IN") {
            Step::Get(expected) => report.copy_from_slice(&expected),
            Step::GetError => anyhow::bail!("simulated IN failure"),
            _ => panic!("IN where OUT expected"),
        };
        Ok(91)
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
fn cap(product_id: u32) -> DeviceReadCapability {
    let mut c = crate::device_reads::capability(product_id).unwrap().clone();
    c.max_retry_out = 1;
    c.max_retry_in = 1;
    c.sleep_between_out_ms = 0;
    c.sleep_between_out_in_ms = 0;
    c.sleep_between_in_ms = 0;
    c
}
fn packet(command: [u8; 3], data: &[u8], transaction: u8, status: u8, out: bool) -> Vec<u8> {
    let mut p = vec![0; 91];
    p[1] = status;
    p[2] = transaction;
    p[6..9].copy_from_slice(&command);
    if !out {
        p[6] = data.len() as u8;
    }
    p[9..9 + data.len()].copy_from_slice(data);
    p[89] = p[3..89].iter().fold(0, |x, v| x ^ v);
    p
}
fn send(command: [u8; 3], data: &[u8], tx: u8) -> Step {
    Step::Send(packet(command, data, tx, 0, true))
}
fn get(command: [u8; 3], data: &[u8], tx: u8) -> Step {
    Step::Get(packet(command, data, tx, 2, false))
}
fn txs() -> impl FnMut() -> Result<u8> {
    let mut next = 0;
    move || {
        next = next % 30 + 1;
        Ok(next)
    }
}

#[test]
fn gates_and_rate_codes_equal_source_verified_static_tables() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../testdata/re/mouse-profile-polling-current-source.json"
    ))
    .unwrap();
    for product in evidence["products"].as_array().unwrap() {
        assert!(capability(product["product_id"].as_u64().unwrap() as u32));
        assert_eq!(product["active_obm_options"]["highSpeedPollingRate"], false);
        assert_eq!(product["active_obm_options"]["debounceTime"], false);
        let rates: Vec<_> = product["rates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["hz"].as_u64().unwrap() as u16,
                    r["code"].as_u64().unwrap() as u8,
                )
            })
            .collect();
        assert_eq!(PROFILE_POLLING_RATES, rates);
    }
    assert!(!capability(999));
}
#[test]
fn each_known_rate_reads_raw_profile_and_source_hz_on_all_four_products() {
    for product in PROFILE_POLLING_PRODUCT_IDS {
        for &(hz, code) in PROFILE_POLLING_RATES {
            let transport = Transport::new([send(GET, &[7], 1), get(GET, &[7, code], 1)]);
            let cap = cap(*product);
            let value = MouseProfilePolling::new(&transport, &cap)
                .unwrap()
                .read(7, txs(), || Ok(()))
                .unwrap();
            assert_eq!(value.known_hz, Some(hz));
            assert_eq!(value.source_hz, hz);
            assert_eq!(value.raw_code, code);
            transport.finish();
        }
    }
}
#[test]
fn unknown_code_preserves_raw_tail_separately_from_source_default() {
    let transport = Transport::new([send(GET, &[3], 1), get(GET, &[3, 0xf0, 0xa5], 1)]);
    let cap = cap(170);
    let value = MouseProfilePolling::new(&transport, &cap)
        .unwrap()
        .read(3, txs(), || Ok(()))
        .unwrap();
    assert_eq!(value.known_hz, None);
    assert_eq!(value.source_hz, 100);
    assert_eq!(value.raw_code, 0xf0);
    assert_eq!(value.raw_payload, [3, 0xf0, 0xa5]);
    transport.finish();
}
#[test]
fn writes_supported_source_rates_and_confirms_independent_raw_readback() {
    for &(hz, code) in PROFILE_POLLING_RATES {
        let transport = Transport::new([
            send(SET, &[4, code], 1),
            get(SET, &[], 1),
            send(GET, &[4], 2),
            get(GET, &[4, code], 2),
        ]);
        let cap = cap(226);
        let report = MouseProfilePolling::new(&transport, &cap)
            .unwrap()
            .apply(4, hz, true, txs(), || Ok(()))
            .unwrap();
        assert!(report.verified());
        assert!(report.source_completed);
        assert!(report.attempted && report.acknowledgment_received);
        transport.finish();
    }
}
#[test]
fn ack_and_stale_setter_echo_do_not_substitute_for_a_readback() {
    let transport = Transport::new([send(SET, &[2, 2], 1), get(SET, &[2, 1], 1)]);
    let cap = cap(170);
    let report = MouseProfilePolling::new(&transport, &cap)
        .unwrap()
        .apply(2, 500, false, txs(), || Ok(()))
        .unwrap();
    assert!(!report.verified());
    assert_eq!(report.acknowledgment_data, [2, 1]);
    assert!(report.source_completed);
    transport.finish();
}
#[test]
fn raw_restore_does_not_reencode_unknown_code_as_one_hundred_hz() {
    let old = parse(3, &[3, 0xf0, 0x99]).unwrap();
    let transport = Transport::new([
        send(SET, &[3, 0xf0], 1),
        get(SET, &[], 1),
        send(GET, &[3], 2),
        get(GET, &[3, 0xf0], 2),
    ]);
    let cap = cap(170);
    let report = MouseProfilePolling::new(&transport, &cap)
        .unwrap()
        .restore(&old, true, txs(), || Ok(()))
        .unwrap();
    assert!(report.verified());
    assert_eq!(report.requested_code, 0xf0);
    transport.finish();
}
#[test]
fn readback_with_same_source_default_but_different_raw_code_fails() {
    let old = parse(3, &[3, 0xf0]).unwrap();
    let transport = Transport::new([
        send(SET, &[3, 0xf0], 1),
        get(SET, &[], 1),
        send(GET, &[3], 2),
        get(GET, &[3, 10], 2),
    ]);
    let cap = cap(170);
    let failure = MouseProfilePolling::new(&transport, &cap)
        .unwrap()
        .restore(&old, true, txs(), || Ok(()))
        .expect_err("same source100 default cannot verify rawcode change");
    assert_eq!(failure.phase, ProfilePollingFailurePhase::Readback);
    assert!(failure.device_state_may_have_changed());
    assert_eq!(failure.progress.observed.unwrap().raw_code, 10);
    transport.finish();
}
#[test]
fn rejects_unsupported_rate_and_zero_profile_without_io() {
    let transport = Transport::new([]);
    let cap = cap(170);
    let api = MouseProfilePolling::new(&transport, &cap).unwrap();
    for (profile, hz) in [(0, 1000), (1, 8000), (1, 999)] {
        let failure = api
            .apply(profile, hz, true, txs(), || Ok(()))
            .expect_err("invalid preparation");
        assert_eq!(failure.phase, ProfilePollingFailurePhase::Prepare);
        assert!(!failure.device_state_may_have_changed());
    }
    assert!(api.read(0, txs(), || Ok(())).is_err());
    transport.finish();
}
#[test]
fn malformed_and_wrong_profile_replies_fail_without_synthetic_values() {
    for reply in [vec![3], vec![4, 1]] {
        let transport = Transport::new([send(GET, &[3], 1), get(GET, &reply, 1)]);
        let cap = cap(170);
        assert!(
            MouseProfilePolling::new(&transport, &cap)
                .unwrap()
                .read(3, txs(), || Ok(()))
                .is_err()
        );
        transport.finish();
    }
}
#[test]
fn raw_restore_requires_consistent_actual_observation_fields() {
    let old = parse(3, &[3, 1]).unwrap();
    let mut forged = old.clone();
    forged.raw_code = 2;
    let transport = Transport::new([]);
    let cap = cap(170);
    let failure = MouseProfilePolling::new(&transport, &cap)
        .unwrap()
        .restore(&forged, true, txs(), || Ok(()))
        .expect_err("contradictory raw observation");
    assert_eq!(failure.phase, ProfilePollingFailurePhase::Prepare);
    assert!(!failure.device_state_may_have_changed());
    transport.finish();
}
#[test]
fn invalid_transaction_and_presend_cancel_are_unattempted() {
    let transport = Transport::new([]);
    let cap = cap(170);
    let api = MouseProfilePolling::new(&transport, &cap).unwrap();
    let failure = api
        .apply(3, 1000, true, || Ok(31), || Ok(()))
        .expect_err("invalid transaction");
    assert!(!failure.device_state_may_have_changed());
    let canceled = Cell::new(false);
    let failure = api
        .apply(
            3,
            1000,
            true,
            || {
                canceled.set(true);
                Ok(1)
            },
            || {
                anyhow::ensure!(!canceled.get(), "cancel before OUT");
                Ok(())
            },
        )
        .expect_err("cancel before send");
    assert_eq!(failure.phase, ProfilePollingFailurePhase::Cancelled);
    assert!(!failure.device_state_may_have_changed());
    transport.finish();
}
#[test]
fn finish_failure_retains_the_actual_ack_without_source_completion_or_readback() {
    let transport = Transport::new([send(SET, &[3, 1], 1), get(SET, &[9], 1)]);
    let cap = cap(170);
    let device = ExchangeProbe::failing_finish(&transport);
    let failure = MouseProfilePolling::new(&device, &cap)
        .unwrap()
        .apply(3, 1000, true, txs(), || Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, ProfilePollingFailurePhase::Write);
    assert!(failure.progress.acknowledgment_received);
    assert_eq!(failure.progress.acknowledgment_data, [9]);
    assert!(!failure.progress.source_completed);
    assert!(failure.progress.observed.is_none());
    assert!(!failure.progress.verified());
    transport.finish();
}

#[test]
fn cancellation_after_ack_preserves_ack_and_stops_get() {
    let transport = Transport::new([send(SET, &[3, 1], 1), get(SET, &[], 1)]);
    let cap = cap(170);
    let canceled = Cell::new(false);
    let after_ack = || canceled.set(true);
    let device = ExchangeProbe::with_on_finish(&transport, &after_ack);
    let failure = MouseProfilePolling::new(&device, &cap)
        .unwrap()
        .apply(3, 1000, true, txs(), || {
            anyhow::ensure!(!canceled.get(), "cancel after ACK");
            Ok(())
        })
        .expect_err("cancel after ACK");
    assert_eq!(failure.phase, ProfilePollingFailurePhase::Cancelled);
    assert!(failure.progress.acknowledgment_received);
    assert!(failure.device_state_may_have_changed());
    assert!(!failure.progress.verified());
    transport.finish();
}
#[test]
fn transport_failures_preserve_attempt_without_claiming_ack() {
    let Step::Send(expected) = send(SET, &[3, 1], 1) else {
        unreachable!()
    };
    for steps in [
        vec![Step::SendError(expected)],
        vec![send(SET, &[3, 1], 1), Step::GetError],
    ] {
        let transport = Transport::new(steps);
        let cap = cap(170);
        let failure = MouseProfilePolling::new(&transport, &cap)
            .unwrap()
            .apply(3, 1000, true, txs(), || Ok(()))
            .expect_err("I/O failure");
        assert_eq!(failure.phase, ProfilePollingFailurePhase::Write);
        assert!(failure.device_state_may_have_changed());
        assert!(!failure.progress.acknowledgment_received);
        transport.finish();
    }
}
#[test]
fn busy_polling_waits_for_ack_without_a_duplicate_write() {
    let transport = Transport::new([
        send(SET, &[3, 1], 1),
        Step::Get(packet(SET, &[], 1, 1, false)),
        get(SET, &[], 1),
    ]);
    let mut cap = cap(170);
    cap.max_retry_in = 2;
    let report = MouseProfilePolling::new(&transport, &cap)
        .unwrap()
        .apply(3, 1000, false, txs(), || Ok(()))
        .unwrap();
    assert!(report.source_completed);
    assert!(!report.verified());
    transport.finish();
}
#[test]
fn mismatched_current_source_transport_is_rejected() {
    let transport = Transport::new([]);
    let mut cap = cap(226);
    cap.claim_interface = 0;
    assert!(MouseProfilePolling::new(&transport, &cap).is_err());
    cap.claim_interface = 3;
    cap.transaction_prefix = 0x20;
    assert!(MouseProfilePolling::new(&transport, &cap).is_err());
    transport.finish();
}

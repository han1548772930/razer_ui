use super::*;
use crate::device_reads::DeviceReadValue;
use std::{cell::RefCell, collections::VecDeque};

struct Step { command: [u8; 3], payload: Vec<u8>, reply: Option<Vec<u8>> }
struct Transport {
    steps: RefCell<VecDeque<Step>>,
    current: RefCell<Option<(Vec<u8>, Option<Vec<u8>>)>>,
    sent: RefCell<Vec<Vec<u8>>>,
}
impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self { Self { steps: RefCell::new(steps.into_iter().collect()), current: RefCell::new(None), sent: RefCell::new(Vec::new()) } }
    fn finished(&self) { assert!(self.steps.borrow().is_empty()); assert!(self.current.borrow().is_none()); }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        let step = self.steps.borrow_mut().pop_front().expect("unexpected Feature command");
        assert_eq!(report.len(), 91);
        assert_eq!(report[0], 0);
        assert_eq!(&report[6..9], &step.command);
        assert_eq!(&report[9..9 + step.payload.len()], &step.payload);
        assert_eq!(report[89], report[3..89].iter().fold(0, |a, b| a ^ b));
        self.sent.borrow_mut().push(report.to_vec());
        *self.current.borrow_mut() = Some((report.to_vec(), step.reply));
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let (request, reply) = self.current.borrow_mut().take().expect("GET follows OUT");
        let data = reply.context("simulated GET failure")?;
        report.copy_from_slice(&request);
        report[1] = 2;
        report[6] = data.len() as u8;
        report[9..89].fill(0);
        report[9..9 + data.len()].copy_from_slice(&data);
        Ok(report.len())
    }
    fn metadata(&self) -> serde_json::Value { serde_json::Value::Null }
}
fn step(command: [u8; 3], payload: &[u8], reply: Option<&[u8]>) -> Step {
    Step { command, payload: payload.to_vec(), reply: reply.map(<[u8]>::to_vec) }
}
fn cap(physical: u32) -> DeviceReadCapability {
    let mut cap = transport(physical).unwrap();
    cap.sleep_between_in_ms = 0;
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap
}
fn transactions() -> impl FnMut() -> anyhow::Result<u8> { let mut n = 0; move || { let value = n; n = (n + 1) % 31; Ok(value) } }

#[test]
fn wired_and_ordinary_receiver_share_high_speed_profile_one() {
    for physical in [190, 191] {
        let cap = cap(physical);
        assert_eq!(cap.product_id, 190);
        assert_eq!(cap.source_class, "rzDevice25");
        assert_eq!(cap.queries[0].command, [2, 0, 192]);
        assert_eq!(cap.queries[0].payload, [1, 0]);
        assert_eq!(cap.polling_value_offset, Some(1));
        assert_eq!(cap.polling_physical_product_id, Some(physical));
    }
    for physical in [182, 183, 179, 164] { assert!(transport(physical).is_err()); }
    assert!(!UI_RATES.contains(&250));
    assert_eq!(rate_code(250).unwrap(), 32);
}

#[test]
fn equal_runtime_get_skips_setter_and_temporary_task_never_reads_obm() {
    let device = Transport::new([step(GET, &[1, 0], Some(&[1, 8]))]);
    let report = request(&device, &cap(190), &Change::new(1000), transactions(), || Ok(())).unwrap();
    assert!(!report.runtime().changed && report.runtime().verified);
    assert!(report.profile_slots().is_empty() && report.slot_writes().is_empty());
    device.finished();
}

#[test]
fn ordinary_sender_uses_source_profile_and_accepts_non_echo_ack_before_independent_readback() {
    let device = Transport::new([
        step(GET, &[1, 0], Some(&[1, 8])),
        step(SET, &[1, 1], Some(&[0x99])),
        step(GET, &[1, 0], Some(&[1, 1])),
    ]);
    let result = apply(&device, &cap(191), 8000, transactions(), || Ok(())).unwrap();
    assert!(result.changed && result.verified);
    assert_eq!(result.previous, Some(DeviceReadValue::Polling { hz: 1000 }));
    assert_eq!(result.observed, Some(DeviceReadValue::Polling { hz: 8000 }));
    assert_eq!(result.acknowledged.unwrap().data, [0x99]);
    device.finished();
}

#[test]
fn failed_pre_get_remains_unobserved_and_does_not_prevent_source_set_task() {
    let device = Transport::new([
        step(GET, &[1, 0], None), step(SET, &[1, 4], Some(&[])), step(GET, &[1, 0], Some(&[1, 4])),
    ]);
    let result = apply(&device, &cap(190), 2000, transactions(), || Ok(())).unwrap();
    assert!(result.previous.is_none() && result.verified);
    device.finished();
}

#[test]
fn setter_ack_is_not_a_successful_runtime_readback() {
    let device = Transport::new([
        step(GET, &[1, 0], Some(&[1, 8])), step(SET, &[1, 1], Some(&[])), step(GET, &[1, 0], Some(&[1, 16])),
    ]);
    assert!(apply(&device, &cap(190), 8000, transactions(), || Ok(())).is_err());
    device.finished();
}

#[test]
fn cancellation_after_pre_get_stops_the_setter() {
    let device = Transport::new([step(GET, &[1, 0], Some(&[1, 8]))]);
    assert!(apply(&device, &cap(190), 8000, transactions(), || {
        ensure!(device.sent.borrow().is_empty(), "connection generation changed"); Ok(())
    }).is_err());
    assert_eq!(device.sent.borrow().len(), 1);
    // The cancelled exchange may not consume its pending IN response.
    assert!(device.steps.borrow().is_empty());
}

#[test]
fn obm_task_uses_only_matching_observed_slots_and_retains_ack_without_claiming_readback() {
    let device = Transport::new([
        step(GET, &[4, 0], Some(&[4, 8])), step(SET, &[4, 2], Some(&[0x77])),
        step(GET, &[7, 0], Some(&[7, 2])),
    ]);
    let slots = [(2, "other"), (4, "active"), (7, "active")].map(|(profile_id, guid)| crate::mouse_onboard::OnboardProfileAssociation { profile_id, guid: guid.into() });
    let writes = synchronize_slots(&device, &cap(190), "active", &slots, 4000, &mut transactions(), &|| Ok(())).unwrap();
    assert_eq!(writes.iter().map(SlotWrite::profile_id).collect::<Vec<_>>(), [4, 7]);
    assert_eq!(writes[0].acknowledgment().unwrap().data, [0x77]);
    assert!(writes[1].acknowledgment().is_none());
    device.finished();
}

#[test]
fn missing_guid_match_and_invalid_request_never_mutate_other_profiles() {
    let device = Transport::new([]);
    let slots = [crate::mouse_onboard::OnboardProfileAssociation { profile_id: 4, guid: "other".into() }];
    assert!(synchronize_slots(&device, &cap(190), "absent", &slots, 1000, &mut transactions(), &|| Ok(())).unwrap().is_empty());
    assert!(apply(&device, &cap(190), 16000, transactions(), || Ok(())).is_err());
    assert!(request(&device, &cap(190), &Change::new(1000).with_profile_guid(String::new()), transactions(), || Ok(())).is_err());
    assert!(device.sent.borrow().is_empty());
}

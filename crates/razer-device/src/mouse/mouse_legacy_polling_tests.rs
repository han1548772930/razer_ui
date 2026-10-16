use super::*;
use crate::device_reads::DeviceReadValue;
use std::cell::RefCell;
use std::collections::VecDeque;

struct Step {
    command: [u8; 3],
    payload: Vec<u8>,
    reply: Option<Vec<u8>>,
}
struct Transport {
    steps: RefCell<VecDeque<Step>>,
    current: RefCell<Option<(Vec<u8>, Option<Vec<u8>>)>>,
    sent: RefCell<Vec<Vec<u8>>>,
}
impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
            current: RefCell::new(None),
            sent: RefCell::new(vec![]),
        }
    }
    fn finished(&self) {
        assert!(self.steps.borrow().is_empty());
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        let step = self
            .steps
            .borrow_mut()
            .pop_front()
            .expect("unexpected command");
        assert_eq!(report.len(), 91);
        assert_eq!(report[0], 0);
        assert_eq!(&report[6..9], &step.command);
        assert_eq!(&report[9..9 + step.payload.len()], &step.payload);
        assert_eq!(report[89], report[3..89].iter().fold(0, |a, b| a ^ b));
        self.sent.borrow_mut().push(report.to_vec());
        *self.current.borrow_mut() = Some((report.to_vec(), step.reply));
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        let (request, reply) = self.current.borrow_mut().take().expect("GET follows OUT");
        let data = reply.context("simulated failed read")?;
        report.copy_from_slice(&request);
        report[1] = 2;
        report[6] = data.len() as u8;
        report[9..89].fill(0);
        report[9..9 + data.len()].copy_from_slice(&data);
        Ok(report.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
fn step(command: [u8; 3], payload: &[u8], reply: Option<&[u8]>) -> Step {
    Step {
        command,
        payload: payload.to_vec(),
        reply: reply.map(<[u8]>::to_vec),
    }
}
fn cap() -> DeviceReadCapability {
    let mut cap = read_capability().clone();
    cap.sleep_between_in_ms = 0;
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap
}
fn transactions() -> impl FnMut() -> Result<u8> {
    let mut value = 0;
    move || {
        let current = value;
        value = (value + 1) % 31;
        Ok(current)
    }
}
#[test]
fn unchanged_rate_does_not_send_setter_or_restore_device_mode() {
    let transport = Transport::new([
        step(GET_MODE, &[], Some(&[3, 0])),
        step(GET_POLLING, &[], Some(&[8])),
    ]);
    let result = apply_polling(&transport, &cap(), 125, transactions(), || Ok(())).unwrap();
    assert!(!result.changed);
    assert!(result.verified);
    transport.finished();
}
#[test]
fn writes_after_mode_switch_and_accepts_non_echo_set_ack_before_readback() {
    let transport = Transport::new([
        step(GET_MODE, &[], Some(&[0, 0])),
        step(SET_MODE, &[3, 0], Some(&[])),
        step(GET_POLLING, &[], Some(&[8])),
        step(SET_POLLING, &[1], Some(&[0])),
        step(GET_POLLING, &[], Some(&[1])),
    ]);
    let result = apply_polling(&transport, &cap(), 1000, transactions(), || Ok(())).unwrap();
    assert!(result.changed && result.verified);
    assert!(matches!(
        result.observed,
        Some(DeviceReadValue::Polling { hz: 1000 })
    ));
    assert!(matches!(
        result.previous,
        Some(DeviceReadValue::Polling { hz: 125 })
    ));
    transport.finished();
}
#[test]
fn initial_read_failure_remains_absent_and_still_writes_like_source_task() {
    let transport = Transport::new([
        step(GET_MODE, &[], Some(&[3, 0])),
        step(GET_POLLING, &[], None),
        step(SET_POLLING, &[2], Some(&[])),
        step(GET_POLLING, &[], Some(&[2])),
    ]);
    let result = apply_polling(&transport, &cap(), 500, transactions(), || Ok(())).unwrap();
    assert!(result.previous.is_none());
    assert!(result.verified);
    transport.finished();
}
#[test]
fn mismatched_readback_never_becomes_success() {
    let transport = Transport::new([
        step(GET_MODE, &[], Some(&[3, 0])),
        step(GET_POLLING, &[], Some(&[8])),
        step(SET_POLLING, &[1], Some(&[])),
        step(GET_POLLING, &[], Some(&[2])),
    ]);
    assert!(apply_polling(&transport, &cap(), 1000, transactions(), || Ok(())).is_err());
    transport.finished();
}
#[test]
fn invalid_rate_and_cancelled_request_send_nothing() {
    let transport = Transport::new([]);
    assert!(apply_polling(&transport, &cap(), 8000, transactions(), || Ok(())).is_err());
    assert!(
        apply_polling(&transport, &cap(), 1000, transactions(), || anyhow::bail!(
            "cancelled"
        ))
        .is_err()
    );
    assert!(transport.sent.borrow().is_empty());
}
#[test]
fn unknown_source_code_is_display_fallback_not_an_enabled_ui_choice() {
    assert!(!UI_POLLING_RATES.contains(&100));
    let transport = Transport::new([
        step(GET_MODE, &[], Some(&[3, 0])),
        step(GET_POLLING, &[], Some(&[77])),
        step(SET_POLLING, &[8], Some(&[])),
        step(GET_POLLING, &[], Some(&[8])),
    ]);
    let result = apply_polling(&transport, &cap(), 125, transactions(), || Ok(())).unwrap();
    assert!(matches!(
        result.previous,
        Some(DeviceReadValue::Polling { hz: 100 })
    ));
    transport.finished();
}

#[test]
fn dpi_sends_profile_one_and_accepts_ack_without_fabricating_readback() {
    let transport = Transport::new([
        step(GET_MODE, &[], Some(&[3, 0])),
        step(SET_DPI, &[1, 0x06, 0x40, 0x0c, 0x80, 0, 0], Some(&[])),
    ]);
    let result = crate::device_writes::apply(
        &transport,
        &cap(),
        &DeviceWriteSetting::Dpi { x: 1600, y: 3200 },
        transactions(),
        || Ok(()),
    )
    .unwrap();
    assert!(result.changed && !result.verified);
    assert!(result.previous.is_none() && result.observed.is_none());
    let ack = result.acknowledged.unwrap();
    assert_eq!(ack.command, SET_DPI);
    assert!(ack.data.is_empty());
    transport.finished();
}

#[test]
fn dpi_non_echo_payload_is_retained_and_not_treated_as_observed_dpi() {
    let transport = Transport::new([
        step(GET_MODE, &[], Some(&[3, 0])),
        step(SET_DPI, &[1, 0, 100, 0x3e, 0x80, 0, 0], Some(&[9, 8, 7])),
    ]);
    let result = apply_dpi(&transport, &cap(), 100, 16000, transactions(), || Ok(())).unwrap();
    assert_eq!(result.acknowledged.unwrap().data, [9, 8, 7]);
    assert!(result.observed.is_none());
    transport.finished();
}

#[test]
fn dpi_ack_failure_and_cancellation_never_report_completion() {
    let transport = Transport::new([
        step(GET_MODE, &[], Some(&[3, 0])),
        step(SET_DPI, &[1, 3, 32, 3, 32, 0, 0], None),
    ]);
    assert!(apply_dpi(&transport, &cap(), 800, 800, transactions(), || Ok(())).is_err());
    transport.finished();
    let transport = Transport::new([]);
    assert!(apply_dpi(&transport, &cap(), 99, 800, transactions(), || Ok(())).is_err());
    assert!(apply_dpi(&transport, &cap(), 800, 16001, transactions(), || Ok(())).is_err());
    assert!(
        apply_dpi(
            &transport,
            &cap(),
            800,
            800,
            transactions(),
            || anyhow::bail!("cancelled")
        )
        .is_err()
    );
    assert!(transport.sent.borrow().is_empty());
}

#[test]
fn dpi_cancelled_after_mode_query_never_sends_setter() {
    let transport = Transport::new([step(GET_MODE, &[], Some(&[3, 0]))]);
    let valid = || {
        anyhow::ensure!(transport.sent.borrow().is_empty(), "profile changed");
        Ok(())
    };
    assert!(apply_dpi(&transport, &cap(), 800, 800, transactions(), valid).is_err());
    transport.finished();
    assert_eq!(transport.sent.borrow().len(), 1);
}

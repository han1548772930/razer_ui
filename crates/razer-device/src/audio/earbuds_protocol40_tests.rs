//! Expected request bytes are written out from the receipt's source
//! `_createDataSend` layout and command constants, not produced by the encoder.
use super::*;
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

struct NoSleep;
impl Sleeper for NoSleep {
    fn sleep(&self, _: Duration) {}
}

enum In {
    Body(Vec<u8>),
    Empty,
    Error,
}

#[derive(Default)]
struct Mock {
    sends: RefCell<Vec<Vec<u8>>>,
    send_counts: RefCell<VecDeque<usize>>,
    replies: RefCell<VecDeque<In>>,
    gets: Cell<usize>,
}

impl Mock {
    fn with(replies: Vec<In>) -> Self {
        Self {
            replies: RefCell::new(replies.into()),
            ..Self::default()
        }
    }
}

impl FeatureTransport for Mock {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        self.send_feature_counted(report).map(|_| ())
    }
    fn send_feature_counted(&self, report: &[u8]) -> Result<usize> {
        self.sends.borrow_mut().push(report.to_vec());
        Ok(self
            .send_counts
            .borrow_mut()
            .pop_front()
            .unwrap_or(report.len()))
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        self.gets.set(self.gets.get() + 1);
        match self.replies.borrow_mut().pop_front() {
            Some(In::Body(body)) => {
                report[0] = REPORT_ID;
                report[1..1 + body.len()].copy_from_slice(&body);
                Ok(REPORT_LENGTH)
            }
            Some(In::Empty) | None => Ok(0),
            Some(In::Error) => bail!("device removed"),
        }
    }
    fn metadata(&self) -> Value {
        Value::Null
    }
}

/// Reply body: status, transaction, 0,0,0, size, class, id, data...
fn reply(status: u8, tx: u8, class: u8, id: u8, data: &[u8]) -> In {
    let mut body = vec![0u8; 63];
    body[0] = status;
    body[1] = tx;
    body[5] = data.len() as u8;
    body[6] = class;
    body[7] = id;
    body[8..8 + data.len()].copy_from_slice(data);
    In::Body(body)
}

fn expected(prefix: &[u8], checksum: u8) -> Vec<u8> {
    let mut report = vec![0u8; 64];
    report[0] = 2;
    report[1..1 + prefix.len()].copy_from_slice(prefix);
    report[62] = checksum;
    report
}

fn ok() -> Result<()> {
    Ok(())
}

#[test]
fn auto_pause_get_bytes_and_parse() {
    let mock = Mock::with(vec![reply(2, 0x61, 24, 158, &[1, 1])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let got = session.get_auto_pause(&ok).unwrap();
    assert_eq!(
        got,
        AutoPauseReply {
            profile_id: 1,
            enabled: true
        }
    );
    // [1,24,158] param [1]; tx = AudioHeadsetSpeakerMic(3)<<5 | 1.
    assert_eq!(
        mock.sends.borrow()[0],
        expected(&[0, 0x61, 0, 0, 0, 1, 24, 158, 1], 0x86)
    );
}

#[test]
fn auto_pause_set_bytes() {
    let mock = Mock::with(vec![reply(2, 0x61, 24, 30, &[1, 1])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    session.set_auto_pause(true, &ok).unwrap();
    assert_eq!(
        mock.sends.borrow()[0],
        expected(&[0, 0x61, 0, 0, 0, 2, 24, 30, 1, 1], 0x04)
    );
}

#[test]
fn anc_set_bytes_ambient_reduced() {
    let mock = Mock::with(vec![reply(2, 0x61, 24, 6, &[1, 80, 1])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let mut runtime = AncRuntimeState::default();
    let request = AncSetting {
        enabled: true,
        mode: AncMode::Ambient,
        level: AncLevel::Reduced,
    };
    assert_eq!(
        set_anc(&mut session, &mut runtime, request, &ok).unwrap(),
        AncWrite::Acknowledged
    );
    assert_eq!(
        mock.sends.borrow()[0],
        expected(&[0, 0x61, 0, 0, 0, 3, 24, 6, 1, 80, 1], 0x4D)
    );
    assert_eq!(runtime.0, request);
}

#[test]
fn transaction_wraps_after_31() {
    let mut counter = TransactionCounter::default();
    let ids: Vec<u8> = (0..32)
        .map(|_| counter.next(TransactionDeviceType::AudioHeadsetSpeakerMic))
        .collect();
    assert_eq!(ids[0], 0x61);
    assert_eq!(ids[30], 0x7F);
    assert_eq!(ids[31], 0x61);
}

#[test]
fn busy_then_success_reads_again_without_resend() {
    let mock = Mock::with(vec![
        reply(1, 0x61, 24, 158, &[]),
        reply(2, 0x61, 24, 158, &[1, 0]),
    ]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    assert!(!session.get_auto_pause(&ok).unwrap().enabled);
    assert_eq!(mock.sends.borrow().len(), 1);
    assert_eq!(mock.gets.get(), 2);
}

#[test]
fn failed_status_resends_and_not_supported_stops() {
    let mock = Mock::with(vec![
        reply(3, 0x61, 24, 158, &[]),
        reply(2, 0x61, 24, 158, &[1, 1]),
    ]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    assert!(session.get_auto_pause(&ok).unwrap().enabled);
    assert_eq!(mock.sends.borrow().len(), 2);

    let mock = Mock::with(vec![reply(5, 0x61, 24, 158, &[])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    assert!(session.get_auto_pause(&ok).is_err());
    assert_eq!(mock.sends.borrow().len(), 1);

    let mock = Mock::with(vec![reply(9, 0x61, 24, 158, &[])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    assert!(session.get_auto_pause(&ok).is_err());
    assert_eq!(mock.sends.borrow().len(), 1);
}

#[test]
fn empty_reads_are_not_observations() {
    let mock = Mock::with(vec![]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    assert!(session.get_auto_pause(&ok).is_err());
    assert_eq!(mock.sends.borrow().len(), MAX_RETRY_OUT);
    assert_eq!(mock.gets.get(), MAX_RETRY_OUT * MAX_RETRY_IN);
}

#[test]
fn short_reply_payload_is_an_error() {
    let mock = Mock::with(vec![reply(2, 0x61, 24, 158, &[1])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    assert!(session.get_auto_pause(&ok).is_err());
}

#[test]
fn short_send_count_is_not_an_ack() {
    let mock = Mock::with(vec![reply(2, 0x61, 24, 158, &[1, 1])]);
    mock.send_counts.borrow_mut().push_back(10);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    assert!(session.get_auto_pause(&ok).unwrap().enabled);
    assert_eq!(mock.sends.borrow().len(), 2);
    assert_eq!(mock.gets.get(), 1);
}

#[test]
fn restore_skips_write_when_device_matches_and_publishes_local() {
    let mock = Mock::with(vec![reply(2, 0x61, 24, 158, &[1, 1])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let outcome = restore_auto_pause(
        &mut session,
        AutoPauseProfileField { is_enabled: None },
        &ok,
    )
    .unwrap();
    assert_eq!(outcome.observation, AutoPauseObservation::Observed(true));
    assert_eq!(outcome.write, WriteOutcome::Skipped);
    assert!(outcome.published_local);
    assert_eq!(mock.sends.borrow().len(), 1);
}

#[test]
fn restore_writes_local_when_device_differs() {
    // Device on, local profile off: device value differs from local default.
    let mock = Mock::with(vec![
        reply(2, 0x61, 24, 158, &[1, 1]),
        reply(2, 0x62, 24, 30, &[1, 0]),
    ]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let field = AutoPauseProfileField {
        is_enabled: Some(false),
    };
    let outcome = restore_auto_pause(&mut session, field, &ok).unwrap();
    assert_eq!(outcome.write, WriteOutcome::Confirmed);
    assert_eq!(
        mock.sends.borrow()[1],
        expected(&[0, 0x62, 0, 0, 0, 2, 24, 30, 1, 0], 0x05)
    );
}

#[test]
fn restore_read_failure_retains_local_intent_without_submitting_a_set() {
    let mock = Mock::with(vec![reply(5, 0x61, 24, 158, &[])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let outcome = restore_auto_pause(
        &mut session,
        AutoPauseProfileField {
            is_enabled: Some(true),
        },
        &ok,
    )
    .unwrap();
    assert!(matches!(
        outcome.observation,
        AutoPauseObservation::ReadFailed(_)
    ));
    assert!(matches!(outcome.write, WriteOutcome::Failed(_)));
    assert!(outcome.published_local);
    assert_eq!(mock.sends.borrow().len(), 1);
}

#[test]
fn disconnect_during_read_is_reported_not_defaulted() {
    let mock = Mock::with(vec![In::Error, In::Error]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let outcome = restore_auto_pause(
        &mut session,
        AutoPauseProfileField {
            is_enabled: Some(false),
        },
        &ok,
    )
    .unwrap();
    assert!(matches!(
        outcome.observation,
        AutoPauseObservation::ReadFailed(_)
    ));
    assert!(matches!(outcome.write, WriteOutcome::Failed(_)));
    assert_eq!(mock.sends.borrow().len(), 1);
    let _ = In::Empty;
}

#[test]
fn stale_generation_rejects_before_io() {
    let mock = Mock::with(vec![reply(2, 0x61, 24, 158, &[1, 1])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let stale = || -> Result<()> { bail!("device generation changed") };
    assert!(
        restore_auto_pause(
            &mut session,
            AutoPauseProfileField { is_enabled: None },
            &stale
        )
        .is_err()
    );
    assert!(mock.sends.borrow().is_empty());
}

#[test]
fn stale_generation_after_reply_rejects_result() {
    let mock = Mock::with(vec![reply(2, 0x61, 24, 158, &[1, 1])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let calls = Cell::new(0);
    let validate = || -> Result<()> {
        calls.set(calls.get() + 1);
        if calls.get() > 1 && !mock.sends.borrow().is_empty() {
            bail!("canceled")
        }
        Ok(())
    };
    assert!(session.get_auto_pause(&validate).is_err());
}

#[test]
fn anc_equal_runtime_skips_io_and_failure_keeps_runtime() {
    let mock = Mock::with(vec![reply(5, 0x61, 24, 6, &[])]);
    let mut session = Protocol40Session::with_sleeper(&mock, NoSleep);
    let mut runtime = AncRuntimeState::default();
    assert_eq!(
        set_anc(&mut session, &mut runtime, ANC_PROFILE_DEFAULT, &ok).unwrap(),
        AncWrite::Skipped
    );
    assert!(mock.sends.borrow().is_empty());
    let off = AncSetting {
        enabled: false,
        ..ANC_PROFILE_DEFAULT
    };
    assert!(matches!(
        set_anc(&mut session, &mut runtime, off, &ok).unwrap(),
        AncWrite::Failed(_)
    ));
    assert_eq!(runtime.0, ANC_PROFILE_DEFAULT);
    assert_eq!(off.set_params(), [1, 0, 10, 0, 0, 0]);
}

#[test]
fn anc_notification_keeps_level() {
    let profile = AncSetting {
        enabled: false,
        mode: AncMode::Anc,
        level: AncLevel::Reduced,
    };
    let ambient = apply_anc_notification(profile, &[5, 33, 80]).unwrap();
    assert_eq!(
        ambient,
        AncSetting {
            enabled: true,
            mode: AncMode::Ambient,
            level: AncLevel::Reduced
        }
    );
    assert!(apply_anc_notification(profile, &[5, 33]).is_none());
    assert!(apply_anc_notification(profile, &[5, 33, 2]).is_none());
    assert!(apply_anc_notification(profile, &[5, 49, 1]).is_none());
}

#[test]
fn reduce_firmware_gate_matches_source_identity_compare() {
    assert!(!reduce_blocked(Some("1.4.6.0")));
    assert!(reduce_blocked(Some("1.4.6.1")));
    assert!(reduce_blocked(Some("1.4.5.9")));
    assert!(!reduce_blocked(Some("1.4.7.0")));
    assert!(!reduce_blocked(None));
    assert!(!reduce_blocked(Some("unknown")));
}

#[test]
fn per_pid_registration() {
    for pid in [2657, 2660, 2664, 2668, 2689] {
        assert!(supports_auto_pause(pid));
    }
    assert!(supports_anc(2657));
    for pid in [2660, 2664, 2668, 2689, 1427] {
        assert!(!supports_anc(pid));
    }
    assert!(!supports_auto_pause(1427));
}

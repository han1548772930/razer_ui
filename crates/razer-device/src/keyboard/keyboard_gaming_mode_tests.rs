use super::*;
use serde_json::json;
use std::{cell::Cell, collections::VecDeque, sync::Mutex};

#[test]
fn release_failure_retains_indicator_ack_without_readback_or_verification() {
    for &product in AUDITED_INDICATOR_PRODUCTS {
        let device = Mock::new([reply(&[0xd3])]);
        let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&device);
        let failure = apply_with_cap(&probe, &cap(product), true, true, &mut sequence(), &|| {
            Ok(())
        })
        .unwrap_err();
        assert_eq!(failure.phase, IndicatorFailurePhase::Write);
        assert!(failure.report.attempted);
        assert_eq!(failure.report.acknowledgment_data, Some(vec![0xd3]));
        assert!(failure.report.observed.is_none());
        assert!(!failure.report.verified());
        assert!(format!("{:#}", failure.error).contains("simulated explicit release failed"));
        assert_eq!(device.sent().len(), 1);
    }
}

struct Reply {
    status: u8,
    data: Vec<u8>,
    count: usize,
    report_id: u8,
    declared_length: Option<u8>,
    wrong_transaction: bool,
    wrong_command: bool,
}

fn reply(data: &[u8]) -> Reply {
    Reply {
        status: 2,
        data: data.to_vec(),
        count: 91,
        report_id: 0,
        declared_length: None,
        wrong_transaction: false,
        wrong_command: false,
    }
}

struct Mock {
    sent: Mutex<Vec<Vec<u8>>>,
    replies: Mutex<VecDeque<Reply>>,
}

impl Mock {
    fn new(replies: impl IntoIterator<Item = Reply>) -> Self {
        Self {
            sent: Mutex::new(Vec::new()),
            replies: Mutex::new(replies.into_iter().collect()),
        }
    }
    fn sent(&self) -> Vec<Vec<u8>> {
        self.sent.lock().unwrap().clone()
    }
}

impl FeatureTransport for Mock {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(report.to_vec());
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .context("mock reply exhausted")?;
        let sent = self.sent.lock().unwrap().last().unwrap().clone();
        report.fill(0);
        report[0] = reply.report_id;
        report[1] = reply.status;
        report[2] = sent[2] ^ u8::from(reply.wrong_transaction);
        report[6] = reply.declared_length.unwrap_or(reply.data.len() as u8);
        report[7..9].copy_from_slice(&sent[7..9]);
        report[8] ^= u8::from(reply.wrong_command);
        report[9..9 + reply.data.len()].copy_from_slice(&reply.data);
        Ok(reply.count)
    }
    fn metadata(&self) -> serde_json::Value {
        json!({"simulated_gaming_indicator": true})
    }
}

fn cap(product_id: u32) -> KeyboardSettingsCapability {
    let mut cap = capability(product_id).unwrap().clone();
    cap.transport.max_retry_in = 1;
    cap.transport.max_retry_out = 1;
    cap.transport.sleep_between_out_ms = 0;
    cap.transport.sleep_between_out_in_ms = 0;
    cap.transport.sleep_between_in_ms = 0;
    cap
}

fn sequence() -> impl FnMut() -> anyhow::Result<u8> {
    let mut sequence = 0;
    move || {
        let current = sequence;
        sequence = (sequence + 1) % 31;
        Ok(current)
    }
}

#[test]
fn source_mode_policy_uses_actual_game_observation_and_missing_means_off() {
    for observed in [None, Some(false), Some(true)] {
        assert!(!effective_indicator(GameModePolicy::Disabled, observed));
        assert!(effective_indicator(GameModePolicy::Enabled, observed));
    }
    assert!(!effective_indicator(GameModePolicy::DuringGame, None));
    assert!(!effective_indicator(
        GameModePolicy::DuringGame,
        Some(false)
    ));
    assert!(effective_indicator(GameModePolicy::DuringGame, Some(true)));
}

#[test]
fn every_registered_indicator_uses_led_class_zero_id_eight_and_wired_commands() {
    assert_eq!(
        AUDITED_INDICATOR_PRODUCTS,
        [600, 602, 664, 678, 679, 688, 697, 716, 717, 724, 727]
    );
    for &pid in AUDITED_INDICATOR_PRODUCTS {
        for enabled in [false, true] {
            let cap = cap(pid);
            let device = Mock::new([reply(&[]), reply(&[0, 8, u8::from(enabled)])]);
            let result =
                apply_with_cap(&device, &cap, enabled, true, &mut sequence(), &|| Ok(())).unwrap();
            assert!(result.verified());
            assert_eq!(result.observed.as_ref().unwrap().enabled(), Some(enabled));
            let sent = device.sent();
            assert_eq!(sent.len(), 2);
            assert_eq!(&sent[0][6..12], &[3, 3, 0, 0, 8, u8::from(enabled)]);
            assert_eq!(&sent[1][6..12], &[3, 3, 128, 0, 8, 0]);
            assert_eq!((sent[0][2], sent[1][2]), (0, 1));
            for report in sent {
                assert_eq!(report.len(), 91);
                assert_eq!(report[0], 0);
                assert_eq!(report[89], report[3..89].iter().fold(0, |crc, b| crc ^ b));
                assert_eq!(report[90], 0);
            }
        }
    }
}

#[test]
fn unregistered_pro_type_receiver_ble_and_unknown_products_fail_before_io() {
    let device = Mock::new([]);
    for pid in [
        708, 0, 691, 756, 603, 604, 625, 626, 662, 663, 698, 699, 706, 707, 713, 714, 718, 722,
        723, 725, 726, 730,
    ] {
        assert!(read(&device, pid, 0, || Ok(())).is_err());
        let error = apply(&device, pid, true, true, sequence(), || Ok(())).unwrap_err();
        assert_eq!(error.phase, IndicatorFailurePhase::Prepare);
        assert!(!error.report.attempted);
        assert!(!error.report.verified());
    }
    assert!(device.sent().is_empty());
}

#[test]
fn original_acknowledgement_mode_keeps_empty_or_arbitrary_data_without_verification() {
    let cap = cap(600);
    for data in [vec![], vec![0, 8, 0], vec![9, 7, 200]] {
        let device = Mock::new([reply(&data)]);
        let result =
            apply_with_cap(&device, &cap, true, false, &mut sequence(), &|| Ok(())).unwrap();
        assert_eq!(result.acknowledgment_data, Some(data));
        assert!(result.observed.is_none());
        assert!(!result.verified());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn setter_echo_does_not_substitute_for_an_independent_getter() {
    let cap = cap(600);
    let device = Mock::new([reply(&[0, 8, 0]), reply(&[0, 8, 1])]);
    let result = apply_with_cap(&device, &cap, true, true, &mut sequence(), &|| Ok(())).unwrap();
    assert_eq!(result.acknowledgment_data, Some(vec![0, 8, 0]));
    assert!(result.verified());
    assert_eq!(device.sent().len(), 2);
}

#[test]
fn getter_preserves_unknown_actual_state_and_does_not_guess_enabled() {
    let cap = cap(600);
    let device = Mock::new([reply(&[0, 8, 255])]);
    let result = read_with_cap(&device, &cap, 30, &|| Ok(())).unwrap();
    assert_eq!(result.raw_state, 255);
    assert_eq!(result.enabled(), None);
    assert_eq!(device.sent()[0][2], 30);
}

#[test]
fn getter_rejects_truncation_wrong_led_class_and_wrong_led_id() {
    let cap = cap(600);
    for data in [vec![], vec![0, 8], vec![1, 8, 1], vec![0, 5, 1]] {
        let device = Mock::new([reply(&data)]);
        assert!(read_with_cap(&device, &cap, 0, &|| Ok(())).is_err());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn rejected_setter_never_starts_readback_and_retains_attempt_without_ack() {
    let cap = cap(600);
    for status in [0, 3, 4, 5, 255] {
        let device = Mock::new([Reply {
            status,
            ..reply(&[])
        }]);
        let error =
            apply_with_cap(&device, &cap, true, true, &mut sequence(), &|| Ok(())).unwrap_err();
        assert_eq!(error.phase, IndicatorFailurePhase::Write);
        assert!(error.report.attempted);
        assert!(error.report.acknowledgment_data.is_none());
        assert!(!error.report.verified());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn failed_partial_different_or_unknown_readback_retains_ack_and_never_verifies() {
    let cap = cap(600);
    for getter in [
        Reply {
            status: 5,
            ..reply(&[])
        },
        reply(&[0, 8]),
        reply(&[0, 8, 0]),
        reply(&[0, 8, 2]),
    ] {
        let device = Mock::new([reply(&[]), getter]);
        let error =
            apply_with_cap(&device, &cap, true, true, &mut sequence(), &|| Ok(())).unwrap_err();
        assert_eq!(error.phase, IndicatorFailurePhase::Readback);
        assert_eq!(error.report.acknowledgment_data, Some(Vec::new()));
        assert!(!error.report.verified());
        assert_eq!(device.sent().len(), 2);
    }
}

#[test]
fn invalid_transaction_fails_before_a_write_attempt_and_wire_wrap_stays_zero_to_thirty() {
    let cap = cap(600);
    let device = Mock::new([]);
    let error = apply_with_cap(&device, &cap, true, true, &mut || Ok(31), &|| Ok(())).unwrap_err();
    assert_eq!(error.phase, IndicatorFailurePhase::Prepare);
    assert!(!error.report.attempted);
    assert!(read_with_cap(&device, &cap, 128, &|| Ok(())).is_err());
    assert!(device.sent().is_empty());
    let device = Mock::new((0..63).map(|_| reply(&[0, 8, 1])));
    let mut next = sequence();
    for _ in 0..63 {
        read_with_cap(&device, &cap, next().unwrap(), &|| Ok(())).unwrap();
    }
    assert_eq!(
        device.sent().iter().map(|r| r[2]).collect::<Vec<_>>(),
        (0..63).map(|i| (i % 31) as u8).collect::<Vec<_>>()
    );
}

#[test]
fn cancel_before_mutation_or_after_ack_stops_later_io_and_never_reports_verified() {
    let cap = cap(600);
    let device = Mock::new([]);
    let error = apply_with_cap(&device, &cap, true, true, &mut sequence(), &|| {
        anyhow::bail!("cancelled")
    })
    .unwrap_err();
    assert_eq!(error.phase, IndicatorFailurePhase::Prepare);
    assert!(!error.report.attempted);
    assert!(device.sent().is_empty());
    let device = Mock::new([reply(&[])]);
    let completed = Cell::new(false);
    let on_finish = || completed.set(true);
    let probe = crate::feature_exchange::tests::ExchangeProbe::with_on_finish(&device, &on_finish);
    let error = apply_with_cap(&probe, &cap, true, true, &mut sequence(), &|| {
        ensure!(!completed.get(), "cancel after ACK");
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error.phase, IndicatorFailurePhase::Write);
    assert_eq!(error.report.acknowledgment_data, Some(Vec::new()));
    assert!(!error.report.verified());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn malformed_report_and_identity_correlation_are_rejected() {
    let cap = cap(600);
    for getter in [
        Reply {
            count: 90,
            ..reply(&[0, 8, 1])
        },
        Reply {
            declared_length: Some(81),
            ..reply(&[0, 8, 1])
        },
        Reply {
            wrong_transaction: true,
            ..reply(&[0, 8, 1])
        },
        Reply {
            wrong_command: true,
            ..reply(&[0, 8, 1])
        },
    ] {
        let device = Mock::new([getter]);
        assert!(read_with_cap(&device, &cap, 0, &|| Ok(())).is_err());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn returned_hid_prefix_is_discarded_by_the_source_host_before_gaming_mode_decode() {
    let device = Mock::new([Reply {
        report_id: 9,
        ..reply(&[0, 8, 1])
    }]);
    assert!(read_with_cap(&device, &cap(600), 0, &|| Ok(())).is_ok());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn busy_response_reuses_request_transaction_without_resending() {
    let mut cap = cap(600);
    cap.transport.max_retry_in = 2;
    let device = Mock::new([
        Reply {
            status: 1,
            ..reply(&[])
        },
        reply(&[0, 8, 1]),
    ]);
    assert_eq!(
        read_with_cap(&device, &cap, 0, &|| Ok(()))
            .unwrap()
            .enabled(),
        Some(true)
    );
    assert_eq!(device.sent().len(), 1);
}

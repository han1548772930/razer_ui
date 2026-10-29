use super::*;
use std::{cell::Cell, collections::VecDeque, sync::Mutex};

#[test]
fn release_failure_retains_analog_ack_without_readback_or_later_chunks() {
    let device = Mock::new([reply(&[0xd3])]);
    let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&device);
    let failure = write(
        &probe,
        &cap_for(678),
        KeyboardAnalogChange::Actuation {
            profile_id: 1,
            points: (1..=16).map(point).collect(),
        },
        true,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(failure.phase, KeyboardAnalogFailurePhase::Write);
    assert_eq!(failure.report.prepared_commands, 2);
    assert_eq!(failure.report.attempted_writes, 1);
    assert_eq!(failure.report.transport_send_attempts, 1);
    assert_eq!(failure.report.acknowledgements.len(), 1);
    assert_eq!(failure.report.acknowledgements[0].data, [0xd3]);
    assert!(failure.report.confirmations.is_empty());
    assert!(!failure.report.verified());
    assert!(format!("{:#}", failure.error).contains("simulated explicit release failed"));
    assert_eq!(device.sent().len(), 1);
}

struct Reply {
    status: u8,
    data: Vec<u8>,
    count: usize,
    report_id: u8,
    wrong_transaction: bool,
    wrong_command: bool,
    declared_size: Option<u8>,
}
fn reply(data: &[u8]) -> Reply {
    Reply {
        status: 2,
        data: data.to_vec(),
        count: 91,
        report_id: 0,
        wrong_transaction: false,
        wrong_command: false,
        declared_size: None,
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
        let r = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .context("Mock reply exhausted")?;
        let sent = self.sent.lock().unwrap().last().unwrap().clone();
        report.fill(0);
        report[0] = r.report_id;
        report[1] = r.status;
        report[2] = sent[2] ^ u8::from(r.wrong_transaction);
        report[6] = r.declared_size.unwrap_or(r.data.len() as u8);
        report[7..9].copy_from_slice(&sent[7..9]);
        report[8] ^= u8::from(r.wrong_command);
        report[9..9 + r.data.len()].copy_from_slice(&r.data);
        Ok(r.count)
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated_keyboard_analog":true})
    }
}
fn cap_for(pid: u32) -> KeyboardSettingsCapability {
    let mut cap = keyboard_settings::capability(pid).unwrap().clone();
    cap.transport.max_retry_in = 1;
    cap.transport.max_retry_out = 1;
    cap.transport.sleep_between_out_ms = 0;
    cap.transport.sleep_between_out_in_ms = 0;
    cap.transport.sleep_between_in_ms = 0;
    cap
}
fn sequence_from(start: u8) -> impl FnMut() -> anyhow::Result<u8> {
    let mut n = start;
    move || {
        let value = n;
        n = (n + 1) % 31;
        Ok(value)
    }
}
fn point(key_id: u8) -> ActuationPoint {
    ActuationPoint {
        key_id,
        threshold_high: 0x4CC8,
        threshold_low: 0x0666,
    }
}
fn rapid(key_id: u8) -> RapidTriggerKey {
    RapidTriggerKey {
        key_id,
        enabled: 1,
        up_sensitivity: 0x1234,
        down_sensitivity: 0x5678,
    }
}
fn actuation_data(profile: u8, points: &[ActuationPoint]) -> Vec<u8> {
    let mut data = vec![0, profile];
    for p in points {
        data.push(p.key_id);
        data.extend_from_slice(&p.threshold_high.to_be_bytes());
        data.extend_from_slice(&p.threshold_low.to_be_bytes());
    }
    data
}
fn rapid_data(profile: u8, keys: &[RapidTriggerKey]) -> Vec<u8> {
    let mut data = vec![0, profile];
    for p in keys {
        data.extend_from_slice(&[p.key_id, p.enabled]);
        data.extend_from_slice(&p.up_sensitivity.to_be_bytes());
        data.extend_from_slice(&p.down_sensitivity.to_be_bytes());
    }
    data
}
fn pair(mode: SnapTapMode) -> SnapTapPair {
    SnapTapPair {
        key_id1: 32,
        key_id2: 34,
        mode,
    }
}
fn continuous() -> KeyboardAnalogChange {
    KeyboardAnalogChange::ContinuousRapidTrigger {
        profile_id: 1,
        enabled: true,
    }
}

#[test]
fn descriptors_follow_actual_analog_and_snap_tap_product_gates() {
    assert_eq!(
        AUDITED_KEYBOARDS
            .iter()
            .map(|c| c.product_id)
            .collect::<Vec<_>>(),
        [678, 679, 688, 716, 724, 727]
    );
    for pid in [678, 679, 688] {
        let c = capability(pid).unwrap();
        assert!(c.analog_v3 && c.kill_switch);
        assert_eq!(c.snap_tap, pid == 688);
    }
    for pid in [716, 724, 727] {
        let c = capability(pid).unwrap();
        assert!(!c.analog_v3 && !c.kill_switch && c.snap_tap);
    }
    for pid in [
        0, 600, 602, 664, 697, 708, 717, 713, 714, 722, 723, 725, 726, 756,
    ] {
        assert!(capability(pid).is_none());
    }
}

#[test]
fn actuation_packets_use_data_type_zero_profile_second_big_endian_and_actual_size() {
    for pid in [678, 679, 688] {
        let points = vec![point(2)];
        let data = actuation_data(5, &points);
        let device = Mock::new([reply(&[]), reply(&data)]);
        let result = write(
            &device,
            &cap_for(pid),
            KeyboardAnalogChange::Actuation {
                profile_id: 5,
                points,
            },
            true,
            sequence_from(0),
            || Ok(()),
        )
        .unwrap();
        assert!(result.verified());
        let sent = device.sent();
        assert_eq!(sent.len(), 2);
        assert_eq!(
            &sent[0][6..16],
            &[7, 2, 25, 0, 5, 2, 0x4C, 0xC8, 0x06, 0x66]
        );
        assert_eq!(&sent[1][6..16], &[7, 2, 153, 0, 5, 2, 0, 0, 0, 0]);
        assert_eq!((sent[0][2], sent[1][2]), (0, 1));
        for r in sent {
            assert_eq!(r.len(), 91);
            assert_eq!(r[0], 0);
            assert_eq!(r[89], r[3..89].iter().fold(0, |a, b| a ^ b));
            assert_eq!(r[90], 0);
        }
    }
}

#[test]
fn actuation_chunks_at_fifteen_and_allocates_zero_after_thirty() {
    let points: Vec<_> = (1..=16).map(point).collect();
    let device = Mock::new([
        reply(&[]),
        reply(&actuation_data(2, &points[..15])),
        reply(&[]),
        reply(&actuation_data(2, &points[15..])),
    ]);
    let r = write(
        &device,
        &cap_for(678),
        KeyboardAnalogChange::Actuation {
            profile_id: 2,
            points,
        },
        true,
        sequence_from(28),
        || Ok(()),
    )
    .unwrap();
    assert!(r.verified());
    assert_eq!(
        (
            r.prepared_commands,
            r.attempted_writes,
            r.transport_send_attempts
        ),
        (2, 2, 2)
    );
    assert_eq!(
        device.sent().iter().map(|r| r[2]).collect::<Vec<_>>(),
        [28, 29, 30, 0]
    );
    assert_eq!(
        device.sent().iter().map(|r| r[6]).collect::<Vec<_>>(),
        [77, 77, 7, 7]
    );
    assert_eq!(
        r.confirmations
            .iter()
            .map(|r| r.acknowledgement_index)
            .collect::<Vec<_>>(),
        [0, 1]
    );
}

#[test]
fn rapid_trigger_chunks_at_thirteen_with_eighty_byte_full_batch() {
    let keys: Vec<_> = (1..=27).map(rapid).collect();
    let device = Mock::new([reply(&[]), reply(&[]), reply(&[])]);
    let r = write(
        &device,
        &cap_for(679),
        KeyboardAnalogChange::RapidTrigger {
            profile_id: 3,
            keys,
        },
        false,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(r.prepared_commands, 3);
    assert!(!r.verified());
    let sent = device.sent();
    assert_eq!(sent.iter().map(|r| r[6]).collect::<Vec<_>>(), [80, 80, 8]);
    assert_eq!(
        &sent[0][7..17],
        &[2, 26, 0, 3, 1, 1, 0x12, 0x34, 0x56, 0x78]
    );
    assert_eq!(&sent[1][9..13], &[0, 3, 14, 1]);
    assert_eq!(&sent[2][9..13], &[0, 3, 27, 1]);
}

#[test]
fn rapid_trigger_independent_readback_matches_all_raw_fields() {
    let keys = vec![rapid(8)];
    let device = Mock::new([reply(&[]), reply(&rapid_data(1, &keys))]);
    let r = write(
        &device,
        &cap_for(688),
        KeyboardAnalogChange::RapidTrigger {
            profile_id: 1,
            keys,
        },
        true,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap();
    assert!(r.verified());
    assert_eq!(
        &device.sent()[1][6..17],
        &[8, 2, 154, 0, 1, 8, 0, 0, 0, 0, 0]
    );
}

#[test]
fn continuous_rt_and_kill_switch_use_their_separate_fixed_commands() {
    for pid in [678, 679, 688] {
        for enabled in [false, true] {
            let device = Mock::new([reply(&[]), reply(&[4, u8::from(enabled)])]);
            let r = write(
                &device,
                &cap_for(pid),
                KeyboardAnalogChange::ContinuousRapidTrigger {
                    profile_id: 4,
                    enabled,
                },
                true,
                sequence_from(0),
                || Ok(()),
            )
            .unwrap();
            assert!(r.verified());
            assert_eq!(&device.sent()[0][6..11], &[2, 2, 32, 4, u8::from(enabled)]);
            assert_eq!(&device.sent()[1][6..11], &[2, 2, 160, 4, 0]);
            let device = Mock::new([reply(&[]), reply(&[3, u8::from(enabled), 32, 34])]);
            let r = write(
                &device,
                &cap_for(pid),
                KeyboardAnalogChange::KillSwitch {
                    profile_id: 3,
                    enabled,
                    key_id1: 32,
                    key_id2: 34,
                },
                true,
                sequence_from(0),
                || Ok(()),
            )
            .unwrap();
            assert!(r.verified());
            assert_eq!(
                &device.sent()[0][6..13],
                &[4, 2, 33, 3, u8::from(enabled), 32, 34]
            );
            assert_eq!(&device.sent()[1][6..13], &[4, 2, 161, 3, 0, 0, 0]);
        }
    }
}

#[test]
fn snap_tap_all_registered_products_encode_modes_and_keep_fixed_fifteen_bytes() {
    for pid in [688, 716, 724, 727] {
        for mode in [
            SnapTapMode::LastInput,
            SnapTapMode::PriorityLeft,
            SnapTapMode::PriorityRight,
            SnapTapMode::Neutral,
            SnapTapMode::CompareLevel,
        ] {
            let mut data = vec![2, 1, 1, 32, 34, mode as u8];
            data.resize(15, 0);
            let device = Mock::new([reply(&[]), reply(&data)]);
            let r = write(
                &device,
                &cap_for(pid),
                KeyboardAnalogChange::SnapTap {
                    profile_id: 2,
                    enabled: true,
                    pairs: vec![pair(mode)],
                },
                true,
                sequence_from(0),
                || Ok(()),
            )
            .unwrap();
            assert!(r.verified());
            assert_eq!(
                &device.sent()[0][6..15],
                &[15, 2, 39, 2, 1, 1, 32, 34, mode as u8]
            );
            assert_eq!(&device.sent()[1][6..12], &[15, 2, 167, 2, 0, 0]);
            assert!(
                matches!(&r.confirmations[0].observation,KeyboardAnalogObservation::SnapTap { declared_pairs:1,pairs,.. } if pairs.len()==4)
            );
        }
    }
}

#[test]
fn snap_tap_supports_four_pairs_and_direct_empty_pair_clear() {
    let pairs = vec![pair(SnapTapMode::LastInput); 4];
    let device = Mock::new([reply(&[])]);
    let r = write(
        &device,
        &cap_for(727),
        KeyboardAnalogChange::SnapTap {
            profile_id: 1,
            enabled: false,
            pairs,
        },
        false,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(r.acknowledgements[0].payload.len(), 15);
    assert_eq!(&device.sent()[0][9..12], &[1, 4, 0]);
    let device = Mock::new([reply(&[]), reply(&[1, 0, 0])]);
    let r = write(
        &device,
        &cap_for(716),
        KeyboardAnalogChange::SnapTap {
            profile_id: 1,
            enabled: false,
            pairs: vec![],
        },
        true,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap();
    assert!(r.verified());
}

#[test]
fn unsupported_product_or_feature_fails_before_device_io() {
    let device = Mock::new([]);
    for pid in [600, 602, 664, 697, 708, 717] {
        let e = write(
            &device,
            &cap_for(pid),
            continuous(),
            true,
            sequence_from(0),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(e.phase, KeyboardAnalogFailurePhase::Prepare);
        assert!(!e.device_state_may_have_changed());
    }
    for pid in [716, 724, 727] {
        for query in [
            KeyboardAnalogQuery::ContinuousRapidTrigger { profile_id: 1 },
            KeyboardAnalogQuery::KillSwitch { profile_id: 1 },
            KeyboardAnalogQuery::Actuation {
                profile_id: 1,
                key_ids: vec![1],
            },
        ] {
            assert!(read(&device, &cap_for(pid), query, 0, || Ok(())).is_err());
        }
    }
    for pid in [678, 679] {
        assert!(
            read(
                &device,
                &cap_for(pid),
                KeyboardAnalogQuery::SnapTap { profile_id: 1 },
                0,
                || Ok(())
            )
            .is_err()
        );
    }
    assert!(device.sent().is_empty());
}

#[test]
fn every_write_is_prepared_before_any_chunk_touches_transport() {
    let device = Mock::new([]);
    let mut keys: Vec<_> = (1..=14).map(rapid).collect();
    keys[13].enabled = 2;
    let e = write(
        &device,
        &cap_for(678),
        KeyboardAnalogChange::RapidTrigger {
            profile_id: 1,
            keys,
        },
        false,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(e.phase, KeyboardAnalogFailurePhase::Prepare);
    assert_eq!(e.report.prepared_commands, 0);
    assert!(!e.device_state_may_have_changed());
    assert!(device.sent().is_empty());
}

#[test]
fn invalid_empty_duplicate_zero_key_or_zero_profile_rejects_before_io() {
    let device = Mock::new([]);
    for (profile_id, points) in [
        (1, vec![]),
        (1, vec![point(1), point(1)]),
        (1, vec![point(0)]),
        (0, vec![point(1)]),
    ] {
        let e = write(
            &device,
            &cap_for(678),
            KeyboardAnalogChange::Actuation { profile_id, points },
            false,
            sequence_from(0),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(e.phase, KeyboardAnalogFailurePhase::Prepare);
    }
    let e = write(
        &device,
        &cap_for(688),
        KeyboardAnalogChange::SnapTap {
            profile_id: 1,
            enabled: true,
            pairs: vec![pair(SnapTapMode::LastInput); 5],
        },
        false,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(e.phase, KeyboardAnalogFailurePhase::Prepare);
    assert!(device.sent().is_empty());
}

#[test]
fn wildcard_all_key_write_has_ack_but_requires_concrete_keys_for_verification() {
    let device = Mock::new([reply(&[])]);
    let change = KeyboardAnalogChange::Actuation {
        profile_id: 1,
        points: vec![point(ALL_KEYS)],
    };
    let r = write(
        &device,
        &cap_for(678),
        change.clone(),
        false,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(device.sent()[0][11], 255);
    assert!(!r.verified());
    let e = write(
        &device,
        &cap_for(678),
        change,
        true,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(e.phase, KeyboardAnalogFailurePhase::Prepare);
    assert_eq!(e.report.attempted_writes, 0);
    assert_eq!(device.sent().len(), 1);
    assert!(
        read(
            &device,
            &cap_for(678),
            KeyboardAnalogQuery::Actuation {
                profile_id: 1,
                key_ids: vec![255]
            },
            0,
            || Ok(())
        )
        .is_err()
    );
}

#[test]
fn source_acknowledgement_keeps_empty_arbitrary_and_echo_data_without_fake_read() {
    for data in [vec![], vec![1, 1], vec![17, 99, 255]] {
        let device = Mock::new([reply(&data)]);
        let r = write(
            &device,
            &cap_for(678),
            continuous(),
            false,
            sequence_from(0),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(r.acknowledgements[0].data, data);
        assert!(r.confirmations.is_empty());
        assert!(!r.verified());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn setter_echo_cannot_confirm_different_independent_observation() {
    let device = Mock::new([reply(&[1, 1]), reply(&[1, 0])]);
    let e = write(
        &device,
        &cap_for(678),
        continuous(),
        true,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(e.phase, KeyboardAnalogFailurePhase::Readback);
    assert_eq!(e.report.acknowledgements[0].data, [1, 1]);
    assert_eq!(e.report.confirmations.len(), 1);
    assert!(!e.report.confirmations[0].matches_request);
    assert!(!e.report.verified());
}

#[test]
fn getters_reject_truncated_wrong_profile_data_type_or_unrequested_duplicate_keys() {
    for data in [
        vec![],
        vec![0, 1, 1],
        vec![1, 1, 1, 0, 1, 0, 2],
        vec![0, 2, 1, 0, 1, 0, 2],
        vec![0, 1, 2, 0, 1, 0, 2],
        vec![0, 1, 1, 0, 1, 0, 2, 1, 0, 1, 0, 2],
    ] {
        let device = Mock::new([reply(&data)]);
        assert!(
            read(
                &device,
                &cap_for(678),
                KeyboardAnalogQuery::Actuation {
                    profile_id: 1,
                    key_ids: vec![1]
                },
                0,
                || Ok(())
            )
            .is_err()
        );
    }
    for data in [vec![], vec![0, 1, 1, 1, 0], vec![0, 2, 1, 1, 0, 1, 0, 2]] {
        let device = Mock::new([reply(&data)]);
        assert!(
            read(
                &device,
                &cap_for(678),
                KeyboardAnalogQuery::RapidTrigger {
                    profile_id: 1,
                    key_ids: vec![1]
                },
                0,
                || Ok(())
            )
            .is_err()
        );
    }
    for data in [vec![], vec![1], vec![2, 1], vec![1, 1, 32]] {
        let device = Mock::new([reply(&data)]);
        assert!(
            read(
                &device,
                &cap_for(678),
                KeyboardAnalogQuery::KillSwitch { profile_id: 1 },
                0,
                || Ok(())
            )
            .is_err()
        );
    }
}

#[test]
fn snap_tap_getter_validates_record_length_count_and_zero_padding_on_confirmation() {
    for data in [
        vec![],
        vec![1, 0],
        vec![2, 0, 1],
        vec![1, 1, 1],
        vec![1, 5, 1],
        vec![1, 0, 1, 9],
    ] {
        let device = Mock::new([reply(&data)]);
        assert!(
            read(
                &device,
                &cap_for(688),
                KeyboardAnalogQuery::SnapTap { profile_id: 1 },
                0,
                || Ok(())
            )
            .is_err()
        );
    }
    let device = Mock::new([reply(&[]), reply(&[1, 1, 1, 32, 34, 0, 0, 0, 7])]);
    let e = write(
        &device,
        &cap_for(688),
        KeyboardAnalogChange::SnapTap {
            profile_id: 1,
            enabled: true,
            pairs: vec![pair(SnapTapMode::LastInput)],
        },
        true,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(e.report.confirmations.len(), 1);
    assert!(!e.report.confirmations[0].matches_request);
}

#[test]
fn getters_preserve_unknown_raw_enabled_and_snap_tap_mode_codes() {
    let device = Mock::new([reply(&[1, 7])]);
    let r = read(
        &device,
        &cap_for(678),
        KeyboardAnalogQuery::ContinuousRapidTrigger { profile_id: 1 },
        30,
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        r,
        KeyboardAnalogObservation::ContinuousRapidTrigger {
            profile_id: 1,
            raw_enabled: 7
        }
    );
    let mut keys = vec![rapid(3)];
    keys[0].enabled = 9;
    let device = Mock::new([reply(&rapid_data(1, &keys))]);
    assert_eq!(
        read(
            &device,
            &cap_for(679),
            KeyboardAnalogQuery::RapidTrigger {
                profile_id: 1,
                key_ids: vec![3]
            },
            0,
            || Ok(())
        )
        .unwrap(),
        KeyboardAnalogObservation::RapidTrigger {
            profile_id: 1,
            keys
        }
    );
    let device = Mock::new([reply(&[1, 1, 3, 32, 34, 240])]);
    assert!(
        matches!(read(&device,&cap_for(727),KeyboardAnalogQuery::SnapTap { profile_id:1 },0,||Ok(())).unwrap(),KeyboardAnalogObservation::SnapTap { raw_enabled:3,pairs,.. } if pairs[0].raw_mode==240)
    );
}

#[test]
fn later_chunk_failure_retains_ack_and_attempts_without_fabricated_rollback() {
    let points: Vec<_> = (1..=16).map(point).collect();
    let device = Mock::new([
        reply(&[]),
        Reply {
            status: 5,
            ..reply(&[])
        },
    ]);
    let e = write(
        &device,
        &cap_for(678),
        KeyboardAnalogChange::Actuation {
            profile_id: 1,
            points,
        },
        false,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(e.phase, KeyboardAnalogFailurePhase::Write);
    assert_eq!(
        (
            e.report.prepared_commands,
            e.report.attempted_writes,
            e.report.acknowledgements.len()
        ),
        (2, 2, 1)
    );
    assert!(e.device_state_may_have_changed());
    assert!(!e.report.verified());
}

#[test]
fn readback_failure_retains_actual_ack_and_previous_chunk_confirmation() {
    let points: Vec<_> = (1..=16).map(point).collect();
    let device = Mock::new([
        reply(&[]),
        reply(&actuation_data(1, &points[..15])),
        reply(&[]),
        reply(&[0, 1]),
    ]);
    let e = write(
        &device,
        &cap_for(678),
        KeyboardAnalogChange::Actuation {
            profile_id: 1,
            points,
        },
        true,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(e.phase, KeyboardAnalogFailurePhase::Readback);
    assert_eq!(e.report.acknowledgements.len(), 2);
    assert_eq!(e.report.confirmations.len(), 1);
    assert!(e.report.confirmations[0].matches_request);
    assert!(!e.report.verified());
}

#[test]
fn cancelled_before_send_is_zero_attempt_while_cancelled_after_send_is_possible_change() {
    let device = Mock::new([]);
    let e = write(
        &device,
        &cap_for(678),
        continuous(),
        false,
        sequence_from(0),
        || anyhow::bail!("cancelled"),
    )
    .unwrap_err();
    assert!(!e.device_state_may_have_changed());
    assert!(device.sent().is_empty());
    let e = write(
        &device,
        &cap_for(678),
        continuous(),
        false,
        sequence_from(0),
        || {
            ensure!(device.sent().is_empty(), "cancelled after native send");
            Ok(())
        },
    )
    .unwrap_err();
    assert!(e.device_state_may_have_changed());
    assert_eq!(e.report.attempted_writes, 1);
    assert_eq!(e.report.transport_send_attempts, 1);
    assert!(e.report.acknowledgements.is_empty());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn retransmission_counts_each_transport_send_but_verifies_one_logical_write() {
    let mut cap = cap_for(678);
    cap.transport.max_retry_out = 2;
    let device = Mock::new([
        Reply {
            wrong_transaction: true,
            ..reply(&[])
        },
        reply(&[]),
        reply(&[1, 1]),
    ]);
    let r = write(&device, &cap, continuous(), true, sequence_from(0), || {
        Ok(())
    })
    .unwrap();
    assert!(r.verified());
    assert_eq!(
        (
            r.attempted_writes,
            r.transport_send_attempts,
            r.acknowledgements.len()
        ),
        (1, 2, 1)
    );
    assert_eq!(
        device.sent().iter().map(|r| r[2]).collect::<Vec<_>>(),
        [0, 0, 1]
    );
}

#[test]
fn invalid_transaction_allocator_and_forged_capability_fail_before_actual_write() {
    let device = Mock::new([]);
    let e = write(
        &device,
        &cap_for(678),
        continuous(),
        true,
        || Ok(31),
        || Ok(()),
    )
    .unwrap_err();
    assert!(!e.device_state_may_have_changed());
    assert!(
        read(
            &device,
            &cap_for(678),
            KeyboardAnalogQuery::ContinuousRapidTrigger { profile_id: 1 },
            31,
            || Ok(())
        )
        .is_err()
    );
    for variant in 0..7 {
        let mut cap = cap_for(678);
        match variant {
            0 => cap.transport.claim_interface = 2,
            1 => cap.transport.source_class = "rzDevice25DualLinkKeyboard".into(),
            2 => cap.transport.direct_pids = vec![725],
            3 => cap.transport.transaction_prefix = 0x80,
            4 => cap.transport.report_bytes = 90,
            5 => cap.transport.vendor_id = 0,
            6 => cap.source_class = "forged".into(),
            _ => unreachable!(),
        }
        let e = write(&device, &cap, continuous(), true, sequence_from(0), || {
            Ok(())
        })
        .unwrap_err();
        assert_eq!(e.phase, KeyboardAnalogFailurePhase::Prepare);
        assert!(!e.device_state_may_have_changed());
    }
    assert!(device.sent().is_empty());
}

#[test]
fn rejected_status_wrong_command_short_report_or_oversize_reply_preserves_attempt_without_ack() {
    let mut cases = vec![
        Reply {
            wrong_command: true,
            ..reply(&[])
        },
        Reply {
            count: 90,
            ..reply(&[])
        },
        Reply {
            declared_size: Some(81),
            ..reply(&[])
        },
    ];
    for status in [0, 3, 4, 5, 255] {
        cases.push(Reply {
            status,
            ..reply(&[])
        });
    }
    for r in cases {
        let device = Mock::new([r]);
        let e = write(
            &device,
            &cap_for(678),
            continuous(),
            true,
            sequence_from(0),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(e.phase, KeyboardAnalogFailurePhase::Write);
        assert_eq!(e.report.attempted_writes, 1);
        assert!(e.report.acknowledgements.is_empty());
        assert!(!e.report.verified());
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn returned_prefix_does_not_reject_an_analog_acknowledgment() {
    let device = Mock::new([Reply {
        report_id: 1,
        ..reply(&[])
    }]);
    let result = write(
        &device,
        &cap_for(678),
        continuous(),
        false,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(result.attempted_writes, 1);
    assert_eq!(result.acknowledgements.len(), 1);
    assert!(!result.verified());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn readback_accepts_reordered_records_by_observed_key_identity() {
    let points = vec![point(2), point(3)];
    let reversed = vec![point(3), point(2)];
    let device = Mock::new([reply(&[]), reply(&actuation_data(1, &reversed))]);
    assert!(
        write(
            &device,
            &cap_for(678),
            KeyboardAnalogChange::Actuation {
                profile_id: 1,
                points
            },
            true,
            sequence_from(0),
            || Ok(())
        )
        .unwrap()
        .verified()
    );
}

#[path = "keyboard_analog_expanded_tests.rs"]
mod expanded;

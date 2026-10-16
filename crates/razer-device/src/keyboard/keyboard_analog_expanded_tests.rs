use super::*;

const EXPANDED_IDS: [u32; 10] = [719, 720, 721, 728, 740, 741, 742, 746, 747, 756];

// Only the test build can implement the sealed trait. Start with a validated
// real descriptor, then remove waiting/retry overhead for a simulated device.
struct SimulatedCapability(AnalogCapabilityView);
impl sealed::Sealed for SimulatedCapability {}
impl KeyboardAnalogCapabilitySource for SimulatedCapability {
    fn analog_capability_view(&self) -> anyhow::Result<AnalogCapabilityView> {
        Ok(self.0.clone())
    }
}

fn independent_cap_for(pid: u32) -> SimulatedCapability {
    let descriptor = transport_capability(pid).unwrap();
    let descriptor = if pid == 747 {
        descriptor.with_target_keymap(2)
    } else {
        descriptor
    };
    let mut view = descriptor.analog_capability_view().unwrap();
    view.transport.max_retry_in = 1;
    view.transport.max_retry_out = 1;
    view.transport.sleep_between_out_ms = 0;
    view.transport.sleep_between_out_in_ms = 0;
    view.transport.sleep_between_in_ms = 0;
    SimulatedCapability(view)
}

fn actuation_query() -> KeyboardAnalogQuery {
    KeyboardAnalogQuery::Actuation {
        profile_id: 1,
        key_ids: vec![2],
    }
}

fn actuation_change() -> KeyboardAnalogChange {
    KeyboardAnalogChange::Actuation {
        profile_id: 1,
        points: vec![point(2)],
    }
}

#[test]
fn independent_descriptors_pin_each_product_gate_and_transport_without_brightness() {
    assert_eq!(
        AUDITED_ANALOG_TRANSPORTS
            .iter()
            .map(|c| c.product_id)
            .collect::<Vec<_>>(),
        [
            678, 679, 688, 716, 719, 720, 721, 724, 727, 728, 740, 741, 742, 746, 747, 756
        ]
    );
    for pid in EXPANDED_IDS {
        let cap = transport_capability(pid).unwrap();
        assert!(cap.analog_v3 && cap.snap_tap);
        assert!(!cap.kill_switch);
        assert_eq!(cap.category, if pid == 747 { "KEYPAD" } else { "KEYBOARD" });
        assert_eq!(cap.hardware_profile_slots, if pid == 747 { 3 } else { 5 });
        assert_eq!(cap.target_keymap_for_per_key, pid == 747);
        assert!(cap.target_keymap_id.is_none());
        assert_eq!(cap.transport.claim_interface, 3);
        let view = cap.analog_capability_view().unwrap();
        let t = view.transport;
        assert_eq!((t.vendor_id, t.report_id, t.report_bytes), (5426, 0, 91));
        assert_eq!(t.direct_pids, [pid]);
        assert_eq!((t.transaction_prefix, t.transaction_modulus), (0, 31));
        assert_eq!((t.max_retry_in, t.max_retry_out), (10, 20));
        assert_eq!(
            (
                t.sleep_between_out_ms,
                t.sleep_between_out_in_ms,
                t.sleep_between_in_ms
            ),
            (5, 5, 5)
        );
        // The original settings-based path remains its original six-product
        // audit. The independent analog descriptor carries no brightness flag.
        assert!(capability(pid).is_none());
    }
    for pid in [678, 679, 688, 716, 724, 727] {
        let independent = transport_capability(pid).unwrap();
        let original = capability(pid).unwrap();
        assert_eq!(
            (
                independent.analog_v3,
                independent.kill_switch,
                independent.snap_tap
            ),
            (original.analog_v3, original.kill_switch, original.snap_tap)
        );
    }
}

#[test]
fn independent_capability_rejects_forged_flags_category_slots_or_wire_facts_before_io() {
    let device = Mock::new([]);
    for variant in 0..11 {
        let mut cap = transport_capability(747).unwrap().with_target_keymap(2);
        match variant {
            0 => cap.product_id = 999,
            1 => cap.analog_v3 = false,
            2 => cap.kill_switch = true,
            3 => cap.snap_tap = false,
            4 => cap.category = "KEYBOARD",
            5 => cap.hardware_profile_slots = 5,
            6 => cap.target_keymap_for_per_key = false,
            7 => cap.transport.claim_interface = 2,
            8 => cap.transport.claim_interfaces_by_pid = &[(747, 2)],
            9 => cap.transport.hardware_profile_slots = 5,
            10 => cap.transport.product_id = 756,
            _ => unreachable!(),
        }
        let error = write(
            &device,
            &cap,
            actuation_change(),
            false,
            sequence_from(0),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, KeyboardAnalogFailurePhase::Prepare);
        assert!(!error.device_state_may_have_changed());
    }
    let keyboard_with_keymap = transport_capability(756).unwrap().with_target_keymap(2);
    assert!(keyboard_with_keymap.analog_capability_view().is_err());
    assert!(device.sent().is_empty());
}

#[test]
fn every_expanded_product_executes_four_set_get_chains_with_independent_confirmation() {
    for pid in EXPANDED_IDS {
        for family in 0..4 {
            let (change, data, commands, needs_keymap) = match family {
                0 => (
                    actuation_change(),
                    actuation_data(1, &[point(2)]),
                    [SET_ACTUATION, GET_ACTUATION],
                    pid == 747,
                ),
                1 => (
                    KeyboardAnalogChange::RapidTrigger {
                        profile_id: 1,
                        keys: vec![rapid(2)],
                    },
                    rapid_data(1, &[rapid(2)]),
                    [SET_RAPID_TRIGGER, GET_RAPID_TRIGGER],
                    pid == 747,
                ),
                2 => (
                    continuous(),
                    vec![1, 1],
                    [SET_CONTINUOUS, GET_CONTINUOUS],
                    false,
                ),
                3 => (
                    KeyboardAnalogChange::SnapTap {
                        profile_id: 1,
                        enabled: true,
                        pairs: vec![pair(SnapTapMode::CompareLevel)],
                    },
                    vec![1, 1, 1, 32, 34, 4],
                    [SET_SNAP_TAP, GET_SNAP_TAP],
                    false,
                ),
                _ => unreachable!(),
            };
            let mut replies = Vec::new();
            if needs_keymap {
                replies.push(reply(&[0xAA]));
            }
            replies.extend([reply(&[]), reply(&data)]);
            let device = Mock::new(replies);
            let report = write(
                &device,
                &independent_cap_for(pid),
                change,
                true,
                sequence_from(29),
                || Ok(()),
            )
            .unwrap();
            assert!(report.verified(), "PID {pid} family {family}");
            assert_eq!(report.prepared_commands, 1);
            assert_eq!(report.attempted_writes, 1);
            assert_eq!(report.transport_send_attempts, 1);
            assert_eq!(report.acknowledgements.len(), 1);
            assert_eq!(report.confirmations.len(), 1);
            assert_eq!(report.target_keymap.attempted, needs_keymap);
            let sent = device.sent();
            assert_eq!(sent.len(), if needs_keymap { 3 } else { 2 });
            let offset = usize::from(needs_keymap);
            for (packet, command) in sent[offset..].iter().zip(commands) {
                assert_eq!(&packet[7..9], &command[1..]);
                assert_eq!(packet[89], packet[3..89].iter().fold(0, |a, b| a ^ b));
            }
            if needs_keymap {
                assert_eq!(&sent[0][6..10], &[1, 2, 55, 2]);
                assert_eq!(sent.iter().map(|p| p[2]).collect::<Vec<_>>(), [29, 30, 0]);
                assert_eq!(report.target_keymap.acknowledgement.unwrap().data, [0xAA]);
            }
        }
    }
}

#[test]
fn every_expanded_product_executes_standalone_gets_and_preserves_observed_raw_values() {
    for pid in EXPANDED_IDS {
        for (query, data, expected) in [
            (
                actuation_query(),
                actuation_data(1, &[point(2)]),
                KeyboardAnalogObservation::Actuation {
                    profile_id: 1,
                    points: vec![point(2)],
                },
            ),
            (
                KeyboardAnalogQuery::RapidTrigger {
                    profile_id: 1,
                    key_ids: vec![2],
                },
                rapid_data(1, &[rapid(2)]),
                KeyboardAnalogObservation::RapidTrigger {
                    profile_id: 1,
                    keys: vec![rapid(2)],
                },
            ),
            (
                KeyboardAnalogQuery::ContinuousRapidTrigger { profile_id: 1 },
                vec![1, 7],
                KeyboardAnalogObservation::ContinuousRapidTrigger {
                    profile_id: 1,
                    raw_enabled: 7,
                },
            ),
            (
                KeyboardAnalogQuery::SnapTap { profile_id: 1 },
                vec![1, 1, 3, 32, 34, 240],
                KeyboardAnalogObservation::SnapTap {
                    profile_id: 1,
                    raw_enabled: 3,
                    declared_pairs: 1,
                    pairs: vec![ObservedSnapTapPair {
                        key_id1: 32,
                        key_id2: 34,
                        raw_mode: 240,
                    }],
                },
            ),
        ] {
            let target = pid == 747 && requires_target_keymap(&query);
            let mut replies = Vec::new();
            if target {
                replies.push(reply(&[]));
            }
            replies.push(reply(&data));
            let device = Mock::new(replies);
            let report = read_scoped(
                &device,
                &independent_cap_for(pid),
                query,
                sequence_from(30),
                || Ok(()),
            )
            .unwrap();
            assert_eq!(report.observation, expected);
            assert_eq!(report.target_keymap.attempted, target);
            assert_eq!(device.sent().len(), if target { 2 } else { 1 });
        }
    }
}

#[test]
fn wired_capability_never_grants_receiver_ble_or_shared_class_only_products() {
    for pid in [0, 713, 714, 722, 723, 725, 726, 757, 758, 999, 600, 708] {
        assert!(transport_capability(pid).is_none(), "PID {pid}");
    }
    assert!(transport_capability(756).is_some());
}

#[test]
fn expanded_kill_switch_is_rejected_despite_shared_wire_methods() {
    let device = Mock::new([]);
    for pid in EXPANDED_IDS {
        let cap = transport_capability(pid).unwrap();
        assert!(
            read(
                &device,
                &cap,
                KeyboardAnalogQuery::KillSwitch { profile_id: 1 },
                0,
                || Ok(())
            )
            .is_err()
        );
        let error = write(
            &device,
            &cap,
            KeyboardAnalogChange::KillSwitch {
                profile_id: 1,
                enabled: true,
                key_id1: 32,
                key_id2: 34,
            },
            false,
            sequence_from(0),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, KeyboardAnalogFailurePhase::Prepare);
        assert!(!error.device_state_may_have_changed());
    }
    assert!(device.sent().is_empty());
}

#[test]
fn keypad_requires_positive_observed_keymap_before_any_per_key_operation() {
    let device = Mock::new([]);
    for cap in [
        transport_capability(747).unwrap(),
        transport_capability(747).unwrap().with_target_keymap(0),
    ] {
        let error = read_scoped(
            &device,
            &cap,
            actuation_query(),
            sequence_from(0),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, KeyboardAnalogReadFailurePhase::Prepare);
        assert!(!error.device_state_may_have_changed());
        let error = write(
            &device,
            &cap,
            actuation_change(),
            false,
            sequence_from(0),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, KeyboardAnalogFailurePhase::Prepare);
        assert!(!error.device_state_may_have_changed());
    }
    assert!(device.sent().is_empty());
}

#[test]
fn single_transaction_read_rejects_keypad_queries_even_with_valid_target() {
    let device = Mock::new([]);
    assert!(
        read(
            &device,
            &independent_cap_for(747),
            actuation_query(),
            0,
            || Ok(())
        )
        .is_err()
    );
    assert!(device.sent().is_empty());
}

#[test]
fn scoped_read_preallocates_and_validates_both_transactions_before_target_selection() {
    for transactions in [[31, 0], [0, 31], [0, 0], [0x80, 1]] {
        let device = Mock::new([]);
        let mut values = transactions.into_iter();
        let error = read_scoped(
            &device,
            &independent_cap_for(747),
            actuation_query(),
            || values.next().context("No simulated transaction"),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, KeyboardAnalogReadFailurePhase::Prepare);
        assert!(!error.device_state_may_have_changed());
        assert!(device.sent().is_empty());
    }
}

#[test]
fn scoped_read_uses_exact_caller_allocations_and_keeps_target_ack_separate() {
    let device = Mock::new([reply(&[2]), reply(&actuation_data(1, &[point(2)]))]);
    let calls = Cell::new(0);
    let report = read_scoped(
        &device,
        &independent_cap_for(747),
        actuation_query(),
        || {
            let index = calls.get();
            calls.set(index + 1);
            Ok([30, 0][index])
        },
        || Ok(()),
    )
    .unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(
        device.sent().iter().map(|p| p[2]).collect::<Vec<_>>(),
        [30, 0]
    );
    assert_eq!(&device.sent()[0][6..10], &[1, 2, 55, 2]);
    assert_eq!(&device.sent()[1][6..12], &[7, 2, 153, 0, 1, 2]);
    assert_eq!(report.target_keymap.transport_send_attempts, 1);
    assert_eq!(report.target_keymap.acknowledgement.unwrap().data, [2]);
    assert!(matches!(
        report.observation,
        KeyboardAnalogObservation::Actuation { .. }
    ));
}

#[test]
fn failed_target_selection_never_sends_analog_get_or_set() {
    for target_reply in [
        Reply {
            status: 5,
            ..reply(&[])
        },
        Reply {
            wrong_transaction: true,
            ..reply(&[])
        },
        Reply {
            wrong_command: true,
            ..reply(&[])
        },
        Reply {
            count: 90,
            ..reply(&[])
        },
    ] {
        let device = Mock::new([target_reply]);
        let error = read_scoped(
            &device,
            &independent_cap_for(747),
            actuation_query(),
            sequence_from(0),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, KeyboardAnalogReadFailurePhase::TargetKeymap);
        assert!(error.device_state_may_have_changed());
        assert!(error.target_keymap.acknowledgement.is_none());
        assert_eq!(device.sent().len(), 1);
    }
    let device = Mock::new([Reply {
        status: 5,
        ..reply(&[])
    }]);
    let error = write(
        &device,
        &independent_cap_for(747),
        actuation_change(),
        true,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KeyboardAnalogFailurePhase::TargetKeymap);
    assert!(error.device_state_may_have_changed());
    assert_eq!(error.report.target_keymap.transport_send_attempts, 1);
    assert_eq!(error.report.attempted_writes, 0);
    assert!(error.report.acknowledgements.is_empty());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn getter_failure_retains_successful_target_mutation_and_raw_ack() {
    let device = Mock::new([reply(&[0xEF]), reply(&[])]);
    let error = read_scoped(
        &device,
        &independent_cap_for(747),
        actuation_query(),
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KeyboardAnalogReadFailurePhase::Read);
    assert!(error.device_state_may_have_changed());
    assert_eq!(error.target_keymap.acknowledgement.unwrap().data, [0xEF]);
    assert_eq!(device.sent().len(), 2);
}

#[test]
fn analog_write_failure_retains_target_ack_without_counting_it_as_value_write() {
    let device = Mock::new([
        reply(&[2]),
        Reply {
            status: 5,
            ..reply(&[])
        },
    ]);
    let error = write(
        &device,
        &independent_cap_for(747),
        actuation_change(),
        true,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KeyboardAnalogFailurePhase::Write);
    assert!(error.device_state_may_have_changed());
    assert_eq!(
        error.report.target_keymap.acknowledgement.unwrap().data,
        [2]
    );
    assert_eq!(
        (
            error.report.attempted_writes,
            error.report.transport_send_attempts
        ),
        (1, 1)
    );
    assert!(error.report.acknowledgements.is_empty());
    assert!(error.report.confirmations.is_empty());
}

#[test]
fn keypad_actuation_and_rapid_chunks_select_once_then_write_and_read_each_chunk() {
    for rapid_mode in [false, true] {
        let (change, parts) = if rapid_mode {
            let keys: Vec<_> = (1..=14).map(rapid).collect();
            let parts = [rapid_data(1, &keys[..13]), rapid_data(1, &keys[13..])];
            (
                KeyboardAnalogChange::RapidTrigger {
                    profile_id: 1,
                    keys,
                },
                parts,
            )
        } else {
            let points: Vec<_> = (1..=16).map(point).collect();
            let parts = [
                actuation_data(1, &points[..15]),
                actuation_data(1, &points[15..]),
            ];
            (
                KeyboardAnalogChange::Actuation {
                    profile_id: 1,
                    points,
                },
                parts,
            )
        };
        let device = Mock::new([
            reply(&[]),
            reply(&[]),
            reply(&parts[0]),
            reply(&[]),
            reply(&parts[1]),
        ]);
        let report = write(
            &device,
            &independent_cap_for(747),
            change,
            true,
            sequence_from(29),
            || Ok(()),
        )
        .unwrap();
        assert!(report.verified());
        assert_eq!(
            (
                report.prepared_commands,
                report.attempted_writes,
                report.transport_send_attempts
            ),
            (2, 2, 2)
        );
        assert_eq!(report.target_keymap.transport_send_attempts, 1);
        assert_eq!(report.acknowledgements.len(), 2);
        assert_eq!(report.confirmations.len(), 2);
        let sent = device.sent();
        assert_eq!(
            sent.iter().map(|p| p[2]).collect::<Vec<_>>(),
            [29, 30, 0, 1, 2]
        );
        assert_eq!(
            sent.iter().map(|p| p[8]).collect::<Vec<_>>(),
            if rapid_mode {
                vec![55, 26, 154, 26, 154]
            } else {
                vec![55, 25, 153, 25, 153]
            }
        );
        assert_eq!(
            sent.iter().map(|p| p[6]).collect::<Vec<_>>(),
            if rapid_mode {
                vec![1, 80, 80, 8, 8]
            } else {
                vec![1, 77, 77, 7, 7]
            }
        );
    }
}

#[test]
fn invalid_later_key_is_prepared_before_selecting_keymap() {
    let mut keys: Vec<_> = (1..=14).map(rapid).collect();
    keys[13].enabled = 2;
    let device = Mock::new([]);
    let error = write(
        &device,
        &independent_cap_for(747),
        KeyboardAnalogChange::RapidTrigger {
            profile_id: 1,
            keys,
        },
        false,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KeyboardAnalogFailurePhase::Prepare);
    assert!(!error.device_state_may_have_changed());
    assert!(device.sent().is_empty());
}

#[test]
fn keypad_continuous_and_snap_do_not_need_or_set_target_keymap() {
    let cap = transport_capability(747).unwrap();
    for (query, data) in [
        (
            KeyboardAnalogQuery::ContinuousRapidTrigger { profile_id: 1 },
            vec![1, 1],
        ),
        (
            KeyboardAnalogQuery::SnapTap { profile_id: 1 },
            vec![1, 0, 0],
        ),
    ] {
        let device = Mock::new([reply(&data)]);
        read(&device, &cap, query, 0, || Ok(())).unwrap();
        assert_eq!(device.sent().len(), 1);
        assert_ne!(device.sent()[0][8], SET_TARGET_OBM_KEYMAP[2]);
    }
}

#[test]
fn cancellation_before_target_send_reports_no_device_mutation() {
    let device = Mock::new([]);
    let error = read_scoped(
        &device,
        &independent_cap_for(747),
        actuation_query(),
        sequence_from(0),
        || anyhow::bail!("Simulated cancellation"),
    )
    .unwrap_err();
    assert_eq!(error.phase, KeyboardAnalogReadFailurePhase::TargetKeymap);
    assert!(!error.device_state_may_have_changed());
    let error = write(
        &device,
        &independent_cap_for(747),
        actuation_change(),
        true,
        sequence_from(0),
        || anyhow::bail!("Simulated cancellation"),
    )
    .unwrap_err();
    assert_eq!(error.phase, KeyboardAnalogFailurePhase::TargetKeymap);
    assert!(!error.device_state_may_have_changed());
    assert!(device.sent().is_empty());
}

#[test]
fn cancellation_after_target_ack_preserves_selection_without_analog_write() {
    for read_mode in [false, true] {
        let device = Mock::new([reply(&[2])]);
        let calls = Cell::new(0);
        let validate = || {
            calls.set(calls.get() + 1);
            ensure!(calls.get() < 5, "Simulated cancellation after target ACK");
            Ok(())
        };
        if read_mode {
            let error = read_scoped(
                &device,
                &independent_cap_for(747),
                actuation_query(),
                sequence_from(0),
                validate,
            )
            .unwrap_err();
            assert_eq!(error.phase, KeyboardAnalogReadFailurePhase::Read);
            assert!(error.device_state_may_have_changed());
            assert_eq!(error.target_keymap.acknowledgement.unwrap().data, [2]);
        } else {
            let error = write(
                &device,
                &independent_cap_for(747),
                actuation_change(),
                true,
                sequence_from(0),
                validate,
            )
            .unwrap_err();
            assert_eq!(error.phase, KeyboardAnalogFailurePhase::Write);
            assert!(error.device_state_may_have_changed());
            assert_eq!(error.report.attempted_writes, 0);
            assert_eq!(
                error.report.target_keymap.acknowledgement.unwrap().data,
                [2]
            );
        }
        assert_eq!(device.sent().len(), 1);
    }
}

#[test]
fn allocator_failure_after_target_ack_retains_the_real_mutation() {
    let device = Mock::new([reply(&[2])]);
    let mut transactions = [Ok(0), Err(anyhow::anyhow!("Simulated allocator failure"))].into_iter();
    let error = write(
        &device,
        &independent_cap_for(747),
        actuation_change(),
        false,
        || transactions.next().unwrap(),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KeyboardAnalogFailurePhase::Write);
    assert!(error.device_state_may_have_changed());
    assert_eq!(error.report.attempted_writes, 0);
    assert_eq!(
        error.report.target_keymap.acknowledgement.unwrap().data,
        [2]
    );
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn repeated_target_transaction_never_sends_the_analog_write() {
    let device = Mock::new([reply(&[])]);
    let error = write(
        &device,
        &independent_cap_for(747),
        actuation_change(),
        false,
        || Ok(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KeyboardAnalogFailurePhase::Write);
    assert!(error.device_state_may_have_changed());
    assert_eq!(error.report.attempted_writes, 0);
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn target_selection_retries_remain_distinct_from_analog_value_send_counts() {
    let mut cap = independent_cap_for(747);
    cap.0.transport.max_retry_out = 2;
    let device = Mock::new([
        Reply {
            wrong_transaction: true,
            ..reply(&[])
        },
        reply(&[]),
        reply(&[]),
    ]);
    let report = write(
        &device,
        &cap,
        actuation_change(),
        false,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(report.target_keymap.transport_send_attempts, 2);
    assert_eq!(report.attempted_writes, 1);
    assert_eq!(report.transport_send_attempts, 1);
    assert_eq!(report.acknowledgements.len(), 1);
    assert!(!report.verified());
    assert_eq!(
        device.sent().iter().map(|p| p[2]).collect::<Vec<_>>(),
        [0, 0, 1]
    );
}

#[test]
fn invalid_target_write_transaction_is_rejected_before_any_mutation() {
    let device = Mock::new([]);
    for transaction in [31, 0x80, 255] {
        let error = write(
            &device,
            &independent_cap_for(747),
            actuation_change(),
            false,
            || Ok(transaction),
            || Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.phase, KeyboardAnalogFailurePhase::TargetKeymap);
        assert!(!error.device_state_may_have_changed());
        assert_eq!(error.report.target_keymap.transport_send_attempts, 0);
        assert_eq!(error.report.attempted_writes, 0);
    }
    assert!(device.sent().is_empty());
}

#[test]
fn cancellation_after_target_send_preserves_attempt_without_inventing_ack() {
    let device = Mock::new([]);
    let calls = Cell::new(0);
    let error = write(
        &device,
        &independent_cap_for(747),
        actuation_change(),
        false,
        sequence_from(0),
        || {
            calls.set(calls.get() + 1);
            ensure!(calls.get() < 2, "Simulated cancellation after target send");
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, KeyboardAnalogFailurePhase::TargetKeymap);
    assert!(error.device_state_may_have_changed());
    assert_eq!(error.report.target_keymap.transport_send_attempts, 1);
    assert!(error.report.target_keymap.acknowledgement.is_none());
    assert_eq!(error.report.attempted_writes, 0);
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn later_keypad_chunk_failure_retains_target_and_first_value_ack() {
    let device = Mock::new([
        reply(&[2]),
        reply(&[]),
        Reply {
            status: 5,
            ..reply(&[])
        },
    ]);
    let error = write(
        &device,
        &independent_cap_for(747),
        KeyboardAnalogChange::Actuation {
            profile_id: 1,
            points: (1..=16).map(point).collect(),
        },
        false,
        sequence_from(0),
        || Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.phase, KeyboardAnalogFailurePhase::Write);
    assert!(error.device_state_may_have_changed());
    assert_eq!(
        error
            .report
            .target_keymap
            .acknowledgement
            .as_ref()
            .unwrap()
            .data,
        [2]
    );
    assert_eq!(error.report.prepared_commands, 2);
    assert_eq!(error.report.attempted_writes, 2);
    assert_eq!(error.report.acknowledgements.len(), 1);
    assert!(!error.report.verified());
    assert_eq!(device.sent().len(), 3);
}

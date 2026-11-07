use super::*;
use crate::backend::FeatureTransport;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    time::Duration,
};
struct NoSleep;
impl Sleeper for NoSleep {
    fn sleep(&self, _: Duration) {}
}
#[derive(Default)]
struct Io {
    sent: RefCell<Vec<Vec<u8>>>,
    replies: RefCell<VecDeque<Vec<u8>>>,
    canceled_after_read: Cell<bool>,
    canceled: Cell<bool>,
}
impl Io {
    fn reply(&self, status: u8, transaction: u8, class: u8, command: u8, data: &[u8]) {
        let mut reply = vec![0; 64];
        reply[0] = 2;
        reply[1] = status;
        reply[2] = transaction;
        reply[6] = data.len() as u8;
        reply[7] = class;
        reply[8] = command;
        reply[9..9 + data.len()].copy_from_slice(data);
        self.replies.borrow_mut().push_back(reply);
    }
}
impl FeatureTransport for Io {
    fn send_feature(&self, bytes: &[u8]) -> Result<()> {
        self.sent.borrow_mut().push(bytes.to_vec());
        Ok(())
    }
    fn get_feature(&self, out: &mut [u8]) -> Result<usize> {
        let data = self
            .replies
            .borrow_mut()
            .pop_front()
            .context("scripted read failed")?;
        out[..data.len()].copy_from_slice(&data);
        if self.canceled_after_read.get() {
            self.canceled.set(true);
        }
        Ok(data.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
fn execute<S: Sleeper>(
    session: &mut Protocol40Session<'_, S>,
    pid: u32,
    operation: Operation,
    validate: &dyn Fn() -> Result<()>,
) -> Result<Report> {
    super::execute(
        session,
        pid,
        operation,
        &crate::earbuds_anc::Cache::default(),
        validate,
        validate,
    )
}

fn valid() -> Result<()> {
    Ok(())
}

#[test]
fn input_source_switch_has_no_payload_restore_or_synthetic_observation() {
    for pid in AUTO_PAUSE_PRODUCTS {
        let io = Io::default();
        io.reply(2, 0x61, 24, 29, &[]);
        let report = execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            pid,
            Operation::SwitchInputSource,
            &valid,
        )
        .unwrap();
        assert_eq!(report.write, WriteOutcome::Confirmed);
        assert_eq!(report.observation, AutoPauseObservation::Unobserved);
        assert_eq!(report.connected, None);
        assert_eq!(report.left_battery, None);
        let sent = io.sent.borrow();
        assert_eq!(sent.len(), 1);
        assert_eq!(&sent[0][6..9], &[0, 24, 29]);
        assert!(sent[0][9..62].iter().all(|byte| *byte == 0));
        assert_eq!(sent[0][62], 24 ^ 29);
        assert_eq!(sent[0][63], 0);
    }
}

#[test]
fn input_source_failure_and_cancel_do_not_confirm_switching() {
    let io = Io::default();
    assert!(
        execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2657,
            Operation::SwitchInputSource,
            &|| anyhow::bail!("canceled")
        )
        .is_err()
    );
    assert!(io.sent.borrow().is_empty());
    let io = Io::default();
    assert!(
        execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2657,
            Operation::SwitchInputSource,
            &valid
        )
        .is_err()
    );
}

#[test]
fn firmware_job_keeps_failures_separate_from_other_device_observations() {
    for pid in AUTO_PAUSE_PRODUCTS {
        let io = Io::default();
        let command = if pid == 2689 { 135 } else { 147 };
        io.reply(2, 0xe1, 0, command, &[1, 4, 6, 7]);
        let report = execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            pid,
            Operation::Firmware,
            &valid,
        )
        .unwrap();
        assert!(
            matches!(report.firmware, FirmwareObservation::Observed(ref reading)
            if reading.version().to_string() == "1.4.6.7")
        );
        assert_eq!(report.observation, AutoPauseObservation::Unobserved);
        assert_eq!(report.connected, None);
        assert_eq!(report.write, WriteOutcome::Skipped);
        assert_eq!(io.sent.borrow().len(), 1);
    }
    let io = Io::default();
    io.reply(2, 0xe1, 0, 147, &[1, 4]);
    let report = execute(
        &mut Protocol40Session::with_sleeper(&io, NoSleep),
        2657,
        Operation::Firmware,
        &valid,
    )
    .unwrap();
    assert!(matches!(
        report.firmware,
        FirmwareObservation::ReadFailed(_)
    ));
    assert_eq!(io.sent.borrow().len(), 1);
}

#[test]
fn all_five_connection_battery_gates_do_not_read_feature_settings() {
    for pid in AUTO_PAUSE_PRODUCTS {
        let io = Io::default();
        let high = if pid == 2689 { 0x60 } else { 0 };
        io.reply(2, high | 1, 24, 154, &[1, 3, 0]);
        io.reply(2, high | 2, 7, 128, &[88, 71]);
        let report = execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            pid,
            Operation::Read,
            &valid,
        )
        .unwrap();
        assert_eq!(report.connected, Some(true));
        assert_eq!(report.observation, AutoPauseObservation::Unobserved);
        assert!(
            report.do_not_disturb.is_none()
                && report.gesture.is_none()
                && report.tap_speed.is_none()
                && report.voice_prompt.is_none()
        );
        assert!(!report.has_disconnected_earbud());
        assert_eq!(
            io.sent
                .borrow()
                .iter()
                .map(|bytes| (bytes[2], bytes[6..10].to_vec()))
                .collect::<Vec<_>>(),
            [
                (high | 1, vec![1, 24, 154, 1]),
                (high | 2, vec![0, 7, 128, 0])
            ]
        );
    }
}

#[test]
fn dnd_all_products_read_before_set_and_require_the_set_business_status() {
    for pid in AUTO_PAUSE_PRODUCTS {
        let io = Io::default();
        io.reply(2, 0x61, 24, 133, &[1, 0]);
        io.reply(2, 0x62, 24, 5, &[]);
        let report = execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            pid,
            Operation::DoNotDisturb { enabled: true },
            &valid,
        )
        .unwrap();
        assert_eq!(
            report.do_not_disturb,
            Some(crate::earbuds_do_not_disturb::Observation::Observed(true))
        );
        assert_eq!(report.write, WriteOutcome::Confirmed);
        let sent = io.sent.borrow();
        assert_eq!(sent.len(), 2);
        assert_eq!(&sent[0][6..10], &[1, 24, 133, 1]);
        assert_eq!(&sent[1][6..11], &[2, 24, 5, 1, 1]);
    }
    let io = Io::default();
    io.reply(2, 0x61, 24, 133, &[1, 0]);
    io.reply(5, 0x62, 24, 5, &[]);
    let failed = execute(
        &mut Protocol40Session::with_sleeper(&io, NoSleep),
        2657,
        Operation::DoNotDisturb { enabled: true },
        &valid,
    )
    .unwrap();
    assert!(matches!(failed.write, WriteOutcome::Failed(_)));
    assert_eq!(
        failed.do_not_disturb,
        Some(crate::earbuds_do_not_disturb::Observation::Observed(false))
    );
}

#[test]
fn dnd_equal_state_skips_set_while_failed_or_unknown_get_never_claims_off() {
    let io = Io::default();
    io.reply(2, 0x61, 24, 133, &[1, 0]);
    let report = execute(
        &mut Protocol40Session::with_sleeper(&io, NoSleep),
        2660,
        Operation::DoNotDisturb { enabled: false },
        &valid,
    )
    .unwrap();
    assert_eq!(report.write, WriteOutcome::Skipped);
    assert_eq!(io.sent.borrow().len(), 1);
    for (status, state) in [(5, 0), (2, 2)] {
        let io = Io::default();
        io.reply(status, 0x61, 24, 133, &[1, state]);
        let result = execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2664,
            Operation::DoNotDisturb { enabled: false },
            &valid,
        );
        if status == 2 {
            let report = result.unwrap();
            assert_eq!(
                report.do_not_disturb,
                Some(ToggleObservation::Unknown(state))
            );
            assert!(matches!(report.write, WriteOutcome::Failed(_)));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(io.sent.borrow().len(), 1);
    }
}

#[test]
fn all_boolean_restores_preserve_unknown_bytes_without_sending_a_default() {
    for product in AUTO_PAUSE_PRODUCTS {
        for raw in [2, 255] {
            for (operation, command, data) in [
                (
                    Operation::Restore {
                        enabled: Some(false),
                    },
                    158,
                    vec![1, raw],
                ),
                (
                    Operation::DoNotDisturb { enabled: false },
                    133,
                    vec![1, raw],
                ),
                (
                    Operation::RestoreVoicePrompt {
                        enabled: Some(false),
                    },
                    144,
                    vec![1, 255, raw],
                ),
                (
                    Operation::RestoreGesture {
                        enabled: Some(false),
                    },
                    162,
                    vec![1, raw],
                ),
            ] {
                let io = Io::default();
                io.reply(2, 0x61, 24, command, &data);
                let report = execute(
                    &mut Protocol40Session::with_sleeper(&io, NoSleep),
                    product,
                    operation,
                    &valid,
                )
                .unwrap();
                let observed = match operation {
                    Operation::Restore { .. } => report.observation,
                    Operation::DoNotDisturb { .. } => report.do_not_disturb.unwrap(),
                    Operation::RestoreVoicePrompt { .. } => report.voice_prompt.unwrap(),
                    Operation::RestoreGesture { .. } => report.gesture.unwrap(),
                    _ => unreachable!(),
                };
                assert_eq!(observed, ToggleObservation::Unknown(raw));
                assert!(matches!(report.write, WriteOutcome::Failed(_)));
                assert_eq!(io.sent.borrow().len(), 1, "{product}/{command}/{raw}");
                assert_eq!(io.sent.borrow()[0][8], command);
            }
        }
    }
}

#[test]
fn dnd_cancellation_after_get_does_not_send_the_requested_set() {
    let io = Io::default();
    io.reply(2, 0x61, 24, 133, &[1, 0]);
    io.canceled_after_read.set(true);
    let validate = || {
        ensure!(!io.canceled.get(), "canceled");
        Ok(())
    };
    assert!(
        execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2689,
            Operation::DoNotDisturb { enabled: true },
            &validate
        )
        .is_err()
    );
    assert_eq!(io.sent.borrow().len(), 1);
}

#[test]
fn source_firmware_precedes_registration_order_restores_and_each_feature_get_runs_once() {
    for pid in AUTO_PAUSE_PRODUCTS {
        let io = Io::default();
        let high = if pid == 2689 { 0x60 } else { 0 };
        io.reply(2, high | 1, 24, 154, &[1, 3, 0]);
        io.reply(2, high | 2, 7, 128, &[80, 90]);
        io.reply(
            2,
            0xe3,
            0,
            if pid == 2689 { 135 } else { 147 },
            &[1, 5, 0, 0],
        );
        for (transaction, command, data) in [
            (0x64, 133, &[1, 1][..]),
            (0x65, 5, &[][..]),
            (0x66, 162, &[1, 0][..]),
            (0x67, 34, &[][..]),
            (0x68, 167, &[1, 0, 0, 120][..]),
            (0x69, 39, &[][..]),
            (0x6a, 144, &[1, 255, 0][..]),
            (0x6b, 16, &[][..]),
            (0x6c, 158, &[1, 0][..]),
            (0x6d, 30, &[][..]),
        ] {
            io.reply(2, transaction, 24, command, data);
        }
        let mut session = Protocol40Session::with_sleeper(&io, NoSleep);
        let gate = execute(&mut session, pid, Operation::Read, &valid).unwrap();
        assert_eq!(gate.observation, AutoPauseObservation::Unobserved);
        execute(&mut session, pid, Operation::Firmware, &valid).unwrap();
        if pid == 2657 {
            execute(
                &mut session,
                pid,
                Operation::Anc {
                    setting: ANC_PROFILE_DEFAULT,
                },
                &valid,
            )
            .unwrap();
        }
        for operation in [
            Operation::DoNotDisturb { enabled: false },
            Operation::RestoreGesture { enabled: None },
            Operation::RestoreTapSpeed { speed: None },
            Operation::RestoreVoicePrompt { enabled: None },
            Operation::Restore { enabled: None },
        ] {
            assert_eq!(
                execute(&mut session, pid, operation, &valid).unwrap().write,
                WriteOutcome::Confirmed
            );
        }
        let expected = [
            154,
            128,
            if pid == 2689 { 135 } else { 147 },
            133,
            5,
            162,
            34,
            167,
            39,
            144,
            16,
            158,
            30,
        ];
        assert_eq!(
            io.sent
                .borrow()
                .iter()
                .map(|packet| packet[8])
                .collect::<Vec<_>>(),
            expected
        );
        assert!(io.replies.borrow().is_empty());
    }
}

#[test]
fn disconnected_gate_sends_no_battery_auto_pause_or_restore_default() {
    let io = Io::default();
    io.reply(2, 1, 24, 154, &[1, 0, 0]);
    let report = execute(
        &mut Protocol40Session::with_sleeper(&io, NoSleep),
        2657,
        Operation::Read,
        &valid,
    )
    .unwrap();
    assert_eq!(report.connected, Some(false));
    assert_eq!(report.observation, AutoPauseObservation::Unobserved);
    assert_eq!(io.sent.borrow().len(), 1);
}

#[test]
fn failed_or_short_connection_never_starts_initialization() {
    for data in [&[][..], &[1][..], &[2, 1][..], &[1, 4][..]] {
        let io = Io::default();
        io.reply(2, 1, 24, 154, data);
        assert!(
            execute(
                &mut Protocol40Session::with_sleeper(&io, NoSleep),
                2660,
                Operation::Read,
                &valid
            )
            .is_err()
        );
        assert_eq!(io.sent.borrow().len(), 1);
    }
}

#[test]
fn user_write_sends_only_source_set_and_requires_business_ack() {
    let io = Io::default();
    io.reply(2, 0x61, 24, 30, &[]);
    let report = execute(
        &mut Protocol40Session::with_sleeper(&io, NoSleep),
        2664,
        Operation::Write { enabled: true },
        &valid,
    )
    .unwrap();
    assert_eq!(report.write, WriteOutcome::Confirmed);
    assert_eq!(report.connected, None);
    assert_eq!(report.observation, AutoPauseObservation::Observed(true));
    assert_eq!(io.sent.borrow()[0][6..11], [2, 24, 30, 1, 1]);
    assert_eq!(io.sent.borrow().len(), 1);
    let io = Io::default();
    io.reply(5, 0x61, 24, 30, &[]);
    assert!(
        execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2664,
            Operation::Write { enabled: true },
            &valid
        )
        .is_err()
    );
}

#[test]
fn canceled_connection_read_prevents_every_following_packet() {
    let io = Io::default();
    io.reply(2, 1, 24, 154, &[1, 3, 0]);
    io.canceled_after_read.set(true);
    let validate = || {
        ensure!(!io.canceled.get(), "scope canceled");
        Ok(())
    };
    assert!(
        execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2668,
            Operation::Read,
            &validate
        )
        .is_err()
    );
    assert_eq!(io.sent.borrow().len(), 1);
}

#[test]
fn battery_notification_uses_event_levels_and_only_rechecks_connection() {
    let io = Io::default();
    io.reply(2, 1, 24, 154, &[1, 3, 0]);
    let report = execute(
        &mut Protocol40Session::with_sleeper(&io, NoSleep),
        2657,
        Operation::Battery {
            levels: Some([255, 88]),
        },
        &valid,
    )
    .unwrap();
    assert_eq!(io.sent.borrow().len(), 1);
    assert!(report.has_disconnected_earbud());
    assert_eq!(report.observation, AutoPauseObservation::Unobserved);
    assert_eq!(report.right_battery, Some(88));
}

#[test]
fn source_transaction_owner_survives_separate_tasks_and_preserves_type_bits() {
    let io = Io::default();
    io.reply(2, 0x61, 24, 30, &[]);
    io.reply(2, 0x62, 24, 30, &[]);
    let mut counter = TransactionCounter::default();
    let mut next = |kind| Ok(counter.next(kind));
    for enabled in [true, false] {
        let mut session = Protocol40Session::with_transaction_source(&io, NoSleep, &mut next);
        execute(&mut session, 2657, Operation::Write { enabled }, &valid).unwrap();
    }
    assert_eq!(
        io.sent
            .borrow()
            .iter()
            .map(|bytes| bytes[2])
            .collect::<Vec<_>>(),
        [0x61, 0x62]
    );
}

#[test]
fn short_native_count_cannot_zero_fill_missing_auto_pause_state_into_false() {
    let io = Io::default();
    io.reply(2, 0x61, 24, 158, &[1, 1]);
    io.replies.borrow_mut().back_mut().unwrap().truncate(10);
    let mut session = Protocol40Session::with_sleeper(&io, NoSleep);
    assert!(session.get_auto_pause(&valid).is_err());
    assert_eq!(io.sent.borrow().len(), 1);
}

#[test]
fn voice_user_write_is_direct_and_targets_all_prompts_on_every_product() {
    for pid in AUTO_PAUSE_PRODUCTS {
        let io = Io::default();
        io.reply(2, 0x61, 24, 16, &[]);
        let report = execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            pid,
            Operation::VoicePrompt { enabled: false },
            &valid,
        )
        .unwrap();
        assert_eq!(
            report.voice_prompt,
            Some(ToggleObservation::Observed(false))
        );
        assert_eq!(report.write, WriteOutcome::Confirmed);
        assert_eq!(io.sent.borrow().len(), 1);
        assert_eq!(&io.sent.borrow()[0][6..12], &[3, 24, 16, 1, 255, 0]);
    }
}

#[test]
fn voice_restore_reads_before_a_conditional_set_and_preserves_failed_set_reading() {
    let io = Io::default();
    io.reply(2, 0x61, 24, 144, &[1, 255, 1]);
    let same = execute(
        &mut Protocol40Session::with_sleeper(&io, NoSleep),
        2657,
        Operation::RestoreVoicePrompt { enabled: None },
        &valid,
    )
    .unwrap();
    assert_eq!(same.write, WriteOutcome::Skipped);
    assert_eq!(io.sent.borrow().len(), 1);
    for status in [2, 5] {
        let io = Io::default();
        io.reply(2, 0x61, 24, 144, &[1, 255, 0]);
        io.reply(status, 0x62, 24, 16, &[]);
        let result = execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2664,
            Operation::RestoreVoicePrompt { enabled: None },
            &valid,
        )
        .unwrap();
        assert_eq!(&io.sent.borrow()[0][6..11], &[2, 24, 144, 1, 255]);
        assert_eq!(&io.sent.borrow()[1][6..12], &[3, 24, 16, 1, 255, 1]);
        assert_eq!(
            result.voice_prompt,
            Some(ToggleObservation::Observed(status == 2))
        );
        if status == 2 {
            assert_eq!(result.write, WriteOutcome::Confirmed);
        } else {
            assert!(matches!(result.write, WriteOutcome::Failed(_)));
        }
    }
}

#[test]
fn voice_restore_never_uses_a_failed_read_as_the_missing_field_default() {
    for (status, data) in [(5, vec![]), (2, vec![1, 255]), (2, vec![1, 255, 2])] {
        let io = Io::default();
        io.reply(status, 0x61, 24, 144, &data);
        let result = execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2689,
            Operation::RestoreVoicePrompt { enabled: None },
            &valid,
        )
        .unwrap();
        if data == [1, 255, 2] {
            assert_eq!(result.voice_prompt, Some(ToggleObservation::Unknown(2)));
        } else {
            assert!(matches!(
                result.voice_prompt,
                Some(ToggleObservation::ReadFailed(_))
            ));
        }
        assert!(matches!(result.write, WriteOutcome::Failed(_)));
        assert_eq!(io.sent.borrow().len(), 1);
    }
}

#[test]
fn voice_restore_cancellation_after_get_prevents_set() {
    let io = Io::default();
    io.reply(2, 0x61, 24, 144, &[1, 255, 0]);
    io.canceled_after_read.set(true);
    let check = || {
        ensure!(!io.canceled.get(), "canceled");
        Ok(())
    };
    assert!(
        execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2668,
            Operation::RestoreVoicePrompt { enabled: None },
            &check
        )
        .is_err()
    );
    assert_eq!(io.sent.borrow().len(), 1);
}

#[test]
fn gesture_and_each_tap_speed_use_direct_user_packets_on_all_products() {
    use crate::earbuds_gesture::{TapObservation, TapSpeed};
    for pid in AUTO_PAUSE_PRODUCTS {
        for enabled in [false, true] {
            let io = Io::default();
            io.reply(2, 0x61, 24, 34, &[]);
            let report = execute(
                &mut Protocol40Session::with_sleeper(&io, NoSleep),
                pid,
                Operation::Gesture { enabled },
                &valid,
            )
            .unwrap();
            assert_eq!(report.gesture, Some(ToggleObservation::Observed(enabled)));
            let sent = io.sent.borrow();
            assert_eq!(sent.len(), 1);
            assert_eq!(&sent[0][6..11], &[2, 24, 34, 1, u8::from(enabled)]);
        }
        for (speed, ticks) in [
            (TapSpeed::Fast, 30),
            (TapSpeed::Normal, 60),
            (TapSpeed::Slow, 90),
            (TapSpeed::Slowest, 120),
        ] {
            let io = Io::default();
            io.reply(2, 0x61, 24, 39, &[]);
            let report = execute(
                &mut Protocol40Session::with_sleeper(&io, NoSleep),
                pid,
                Operation::TapSpeed { speed },
                &valid,
            )
            .unwrap();
            assert_eq!(report.tap_speed, Some(TapObservation::Observed(ticks)));
            let sent = io.sent.borrow();
            assert_eq!(sent.len(), 1);
            assert_eq!(&sent[0][6..13], &[4, 24, 39, 1, 1, 0, ticks as u8]);
        }
    }
}
#[test]
fn gesture_and_tap_restore_failures_do_not_write_default_or_discard_fresh_read() {
    use crate::earbuds_gesture::{self as gesture, TapObservation};
    for truncated in [false, true] {
        let io = Io::default();
        io.reply(if truncated { 2 } else { 5 }, 0x61, 24, 162, &[1]);
        let report = gesture::restore(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            None,
            &valid,
        )
        .unwrap();
        assert!(matches!(
            report.observation,
            ToggleObservation::ReadFailed(_)
        ));
        assert_eq!(io.sent.borrow().len(), 1);
        let io = Io::default();
        io.reply(if truncated { 2 } else { 5 }, 0x61, 24, 167, &[1, 0, 0]);
        let report = gesture::restore_tap(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            None,
            &valid,
        )
        .unwrap();
        assert!(matches!(report.observation, TapObservation::ReadFailed(_)));
        assert_eq!(io.sent.borrow().len(), 1);
    }
    let io = Io::default();
    io.reply(2, 0x61, 24, 162, &[1, 0]);
    io.reply(5, 0x62, 24, 34, &[]);
    let report = gesture::restore(
        &mut Protocol40Session::with_sleeper(&io, NoSleep),
        None,
        &valid,
    )
    .unwrap();
    assert_eq!(report.observation, ToggleObservation::Observed(false));
    assert!(matches!(report.write, WriteOutcome::Failed(_)));
    let io = Io::default();
    io.reply(2, 0x61, 24, 167, &[1, 0, 1, 44]);
    io.reply(5, 0x62, 24, 39, &[]);
    let report = gesture::restore_tap(
        &mut Protocol40Session::with_sleeper(&io, NoSleep),
        None,
        &valid,
    )
    .unwrap();
    assert_eq!(report.observation, TapObservation::Observed(300));
    assert!(matches!(report.write, WriteOutcome::Failed(_)));
}
#[test]
fn gesture_restore_cancellation_and_source_firmware_gates_are_preserved() {
    use crate::earbuds_gesture as gesture;
    let io = Io::default();
    io.canceled_after_read.set(true);
    io.reply(2, 0x61, 24, 162, &[1, 0]);
    let validate = || {
        ensure!(!io.canceled.get(), "canceled");
        Ok(())
    };
    assert!(
        gesture::restore(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            None,
            &validate
        )
        .is_err()
    );
    assert_eq!(io.sent.borrow().len(), 1);
    for (pid, min, lower, next) in [
        (2657, "1.3.6.0", "1.3.5.0", "1.3.7.0"),
        (2660, "1.3.1.0", "1.3.0.0", "1.3.2.0"),
        (2664, "1.3.1.0", "1.3.0.0", "1.3.2.0"),
        (2689, "1.0.2.0", "1.0.1.0", "1.0.3.0"),
    ] {
        assert!(gesture::tap_supported(pid, Some(min)));
        assert!(!gesture::tap_supported(pid, Some(lower)));
        assert!(gesture::tap_supported(pid, Some(next)));
        assert!(gesture::tap_supported(pid, None));
    }
    assert!(!gesture::tap_supported(2657, Some("1.3.6.1")));
    assert!(!gesture::tap_supported(2668, Some("9.9.9.9")));
}

#[test]
fn anc_jobs_keep_runtime_skips_and_failed_writes_distinct_from_confirmation() {
    let io = Io::default();
    let skipped = execute(
        &mut Protocol40Session::with_sleeper(&io, NoSleep),
        2657,
        Operation::Anc {
            setting: ANC_PROFILE_DEFAULT,
        },
        &valid,
    )
    .unwrap()
    .anc
    .unwrap();
    assert_eq!(skipped.write, AncWrite::Skipped);
    assert_eq!(skipped.confirmation, None);
    assert!(io.sent.borrow().is_empty());
    let requested = AncSetting {
        enabled: false,
        ..ANC_PROFILE_DEFAULT
    };
    for status in [2, 5] {
        let io = Io::default();
        io.reply(status, 0x61, 24, 6, &[]);
        let report = execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2657,
            Operation::Anc { setting: requested },
            &valid,
        )
        .unwrap()
        .anc
        .unwrap();
        if status == 2 {
            assert_eq!(report.write, AncWrite::Acknowledged);
            assert_eq!(report.confirmation, Some(requested));
            assert_eq!(report.runtime, AncRuntimeState(requested));
        } else {
            assert!(matches!(report.write, AncWrite::Failed(_)));
            assert_eq!(report.confirmation, None);
            assert_eq!(report.runtime, AncRuntimeState::default());
        }
        assert_eq!(&io.sent.borrow()[0][6..12], &[3, 24, 6, 1, 0, 10]);
    }
    assert!(
        execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2664,
            Operation::Anc { setting: requested },
            &valid
        )
        .is_err()
    );
}

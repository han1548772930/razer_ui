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
fn valid() -> Result<()> {
    Ok(())
}

#[test]
fn all_five_read_connection_and_battery_before_auto_pause_with_actual_transaction_branch() {
    for pid in AUTO_PAUSE_PRODUCTS {
        let io = Io::default();
        let high = if pid == 2689 { 0x60 } else { 0 };
        io.reply(2, high | 1, 24, 154, &[1, 3, 0]);
        io.reply(2, high | 2, 7, 128, &[88, 71]);
        io.reply(2, 0x63, 24, 158, &[1, 0]);
        let report = execute(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            pid,
            Operation::Read,
            &valid,
        )
        .unwrap();
        assert_eq!(report.connected, Some(true));
        assert_eq!(report.observation, AutoPauseObservation::Observed(false));
        assert!(!report.has_disconnected_earbud());
        let sent = io.sent.borrow();
        assert_eq!(
            sent.iter()
                .map(|bytes| (bytes[2], bytes[6..10].to_vec()))
                .collect::<Vec<_>>(),
            [
                (high | 1, vec![1, 24, 154, 1]),
                (high | 2, vec![0, 7, 128, 0]),
                (0x63, vec![1, 24, 158, 1])
            ]
        );
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
    for data in [&[][..], &[1][..], &[2, 1][..]] {
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

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
    cancel_after_read: Cell<bool>,
    canceled: Cell<bool>,
}
impl Io {
    fn reply(&self, status: u8, transaction: u8, command: u8, data: &[u8]) {
        let mut bytes = vec![0; 64];
        bytes[0] = 2;
        bytes[1] = status;
        bytes[2] = transaction;
        bytes[6] = data.len() as u8;
        bytes[8] = command;
        bytes[9..9 + data.len()].copy_from_slice(data);
        self.replies.borrow_mut().push_back(bytes);
    }
}
impl FeatureTransport for Io {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        self.sent.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        let bytes = self
            .replies
            .borrow_mut()
            .pop_front()
            .context("No scripted firmware response")?;
        report[..bytes.len()].copy_from_slice(&bytes);
        if self.cancel_after_read.get() {
            self.canceled.set(true);
        }
        Ok(bytes.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

#[test]
fn firmware_uses_each_current_product_selector_and_dock_transaction_bits() {
    for product in protocol::AUTO_PAUSE_PRODUCTS {
        let io = Io::default();
        let (size, command, source) = if product == 2689 {
            (0, 135, Source::Extended)
        } else {
            (4, 147, Source::DockExtended)
        };
        io.reply(2, 0xe1, command, &[1, 3, 6, 0]);
        let result = read(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            product,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(result.version().components(), [1, 3, 6, 0]);
        assert_eq!(result.version().to_string(), "1.3.6.0");
        assert_eq!(result.source(), source);
        assert_eq!(result.dock_failure(), None);
        // Full request expectation is independent of encode_request.
        let mut expected = vec![0; 64];
        expected[0] = 2;
        expected[2] = 0xe1;
        expected[6] = size;
        expected[8] = command;
        expected[62] = size ^ command;
        assert_eq!(io.sent.borrow().as_slice(), &[expected]);
    }
}

#[test]
fn failed_or_empty_dock_reply_uses_only_the_source_extended_fallback() {
    for status in [2, 5] {
        let io = Io::default();
        io.reply(status, 0xe1, 147, &[]);
        io.reply(2, 0xe2, 135, &[1, 0, 2, 9]);
        let result = read(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2657,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(result.version().components(), [1, 0, 2, 9]);
        assert_eq!(result.source(), Source::Extended);
        assert!(result.dock_failure().is_some());
        assert_eq!(
            io.sent
                .borrow()
                .iter()
                .map(|r| (r[2], r[6], r[8]))
                .collect::<Vec<_>>(),
            [(0xe1, 4, 147), (0xe2, 0, 135)]
        );
    }
}

#[test]
fn incomplete_version_and_failed_queries_do_not_publish_defaults() {
    let io = Io::default();
    io.reply(2, 0xe1, 147, &[1, 3]);
    assert!(
        read(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2657,
            &|| Ok(())
        )
        .is_err()
    );
    assert_eq!(io.sent.borrow().len(), 1);
    let io = Io::default();
    io.reply(5, 0xe1, 147, &[]);
    io.reply(5, 0xe2, 135, &[]);
    assert!(
        read(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2664,
            &|| Ok(())
        )
        .is_err()
    );
}

#[test]
fn canceled_dock_query_never_issues_a_fallback() {
    let io = Io::default();
    io.cancel_after_read.set(true);
    io.reply(5, 0xe1, 147, &[]);
    let valid = || {
        ensure!(!io.canceled.get(), "firmware read canceled");
        Ok(())
    };
    assert!(
        read(
            &mut Protocol40Session::with_sleeper(&io, NoSleep),
            2660,
            &valid
        )
        .is_err()
    );
    assert_eq!(io.sent.borrow().len(), 1);
}

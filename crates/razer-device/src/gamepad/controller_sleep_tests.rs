use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

struct Transport {
    sent: RefCell<Vec<Vec<u8>>>,
    replies: RefCell<VecDeque<Vec<u8>>>,
}
impl Transport {
    fn new(replies: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            sent: RefCell::new(Vec::new()),
            replies: RefCell::new(replies.into_iter().collect()),
        }
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("Expected collection Feature SET")
    }
    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("Expected collection Feature GET")
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let reply = self
            .replies
            .borrow_mut()
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("No simulated reply"))?;
        report[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}
// Expected reports below spell the source bytes independently of the encoder.
fn reply(report_id: u8, transaction: u8, command: u8, status: u8, payload: &[u8]) -> Vec<u8> {
    let mut reply = vec![0; 91];
    reply[0] = report_id;
    reply[1] = status;
    reply[2] = transaction;
    reply[6] = payload.len() as u8;
    reply[7] = 7;
    reply[8] = command;
    reply[9..9 + payload.len()].copy_from_slice(payload);
    reply
}
fn set(minutes: u16, enabled: bool) -> SleepOperation {
    SleepOperation::Set {
        power_saving: PowerSaving::new(minutes, enabled).unwrap(),
    }
}

#[test]
fn both_current_products_set_minutes_as_seconds_then_read_independently() {
    for (pid, selector, payload, delay) in
        [(2636, 4, [0x2c, 0x01], 10), (2676, 10, [0x01, 0x2c], 30)]
    {
        let io = Transport::new([
            reply(selector, 0, 3, 2, &[]),
            reply(selector, 1, 0x83, 2, &payload),
        ]);
        let mut delays = vec![];
        let probe = crate::feature_exchange::tests::ExchangeProbe::new(&io);
        let result = SleepProtocol::new(&probe, capability(pid).unwrap(), |wait| {
            delays.push(wait);
            Ok(())
        })
        .execute(&set(5, true))
        .unwrap();
        assert_eq!(result.completion, SleepCompletion::Confirmed);
        assert_eq!(
            result.observation(),
            Some(SleepObservation { seconds: 300 })
        );
        let sent = io.sent.borrow();
        assert_eq!(sent.len(), 2, "No setter pre-read or extra enable command");
        assert_eq!(&sent[0][6..11], &[2, 7, 3, payload[0], payload[1]]);
        assert_eq!(&sent[1][6..11], &[2, 7, 0x83, 0, 0]);
        assert_eq!(sent[0][2], 0);
        assert_eq!(sent[1][2], 1);
        assert_eq!(
            probe.timings(),
            vec![
                crate::backend::ExchangeTiming::MutexOut {
                    interval: Duration::from_millis(delay)
                };
                2
            ]
        );
        for bytes in sent.iter() {
            assert_eq!(bytes[89], bytes[3..89].iter().fold(0, |a, b| a ^ b));
        }
        assert_eq!(
            delays,
            [
                Duration::ZERO,
                Duration::from_millis(delay),
                Duration::ZERO,
                Duration::from_millis(delay)
            ]
        );
    }
}

#[test]
fn setter_exception_still_gets_and_confirms_raw_seconds() {
    let io = Transport::new([reply(4, 0, 3, 5, &[]), reply(4, 1, 0x83, 2, &[0x2c, 0x01])]);
    let result = SleepProtocol::new(&io, capability(2636).unwrap(), |_| Ok(()))
        .execute(&set(5, true))
        .unwrap();
    assert!(matches!(result.write, SleepWriteStatus::Failed { .. }));
    assert_eq!(result.completion, SleepCompletion::Confirmed);
    assert_eq!(io.sent.borrow().len(), 2);
}

#[test]
fn different_raw_seconds_fail_even_when_rounded_ui_minutes_match() {
    let io = Transport::new([
        reply(10, 0, 3, 2, &[]),
        reply(10, 1, 0x83, 2, &[0x01, 0x2d]),
    ]);
    let result = SleepProtocol::new(&io, capability(2676).unwrap(), |_| Ok(()))
        .execute(&set(5, false))
        .unwrap();
    assert_eq!(result.completion, SleepCompletion::Mismatch);
    assert_eq!(result.observation().unwrap().power_saving().value, 5);
    assert_eq!(result.observation().unwrap().seconds, 301);
    assert_eq!(
        io.sent.borrow().len(),
        2,
        "isEnabled does not send an enable command"
    );
}

#[test]
fn failed_initial_get_does_not_write_defaults_or_invent_an_observation() {
    let io = Transport::new([reply(4, 0, 0x83, 5, &[])]);
    let result = SleepProtocol::new(&io, capability(2636).unwrap(), |_| Ok(()))
        .execute(&SleepOperation::Read)
        .unwrap();
    assert_eq!(result.completion, SleepCompletion::Failed);
    assert_eq!(result.write, SleepWriteStatus::NotRequested);
    assert!(result.observation().is_none());
    assert_eq!(io.sent.borrow()[0][8], 0x83);
    assert_eq!(io.sent.borrow().len(), 1);
}

#[test]
fn exact_two_byte_get_and_js_rounding_preserve_real_zero_and_unknown_choices() {
    for (seconds, minutes, enabled) in [
        (0, 0, false),
        (29, 0, false),
        (30, 1, true),
        (89, 1, true),
        (90, 2, true),
        (65535, 1092, true),
    ] {
        let observed = SleepObservation { seconds };
        assert_eq!(observed.power_saving().value, minutes);
        assert_eq!(observed.power_saving().is_enabled, enabled);
    }
    for bytes in [vec![], vec![1], vec![1, 2, 3]] {
        let io = Transport::new([reply(4, 0, 0x83, 2, &bytes)]);
        let result = SleepProtocol::new(&io, capability(2636).unwrap(), |_| Ok(()))
            .execute(&SleepOperation::Read)
            .unwrap();
        assert_eq!(result.completion, SleepCompletion::Failed);
        assert!(result.observation().is_none());
    }
}

#[test]
fn cancel_before_set_and_after_set_preserves_transfer_facts_without_rollback() {
    let io = Transport::new([]);
    let result = SleepProtocol::new(&io, capability(2636).unwrap(), |_| Ok(()))
        .execute_with_cancel(&set(15, true), || true)
        .unwrap();
    assert_eq!(result.completion, SleepCompletion::CanceledBeforeOperation);
    assert_eq!(result.write, SleepWriteStatus::NotAttempted);
    assert!(io.sent.borrow().is_empty());
    let io = Transport::new([reply(4, 0, 3, 2, &[])]);
    let checks = Cell::new(0);
    let result = SleepProtocol::new(&io, capability(2636).unwrap(), |_| Ok(()))
        .execute_with_cancel(&set(15, true), || {
            let count = checks.get();
            checks.set(count + 1);
            count == 1
        })
        .unwrap();
    assert_eq!(result.completion, SleepCompletion::CanceledAfterSet);
    assert_eq!(result.write, SleepWriteStatus::Acknowledged);
    assert_eq!(result.read, SleepReadStatus::Unobserved);
    assert_eq!(io.sent.borrow().len(), 1);
}

#[test]
fn cancellation_during_get_has_no_extra_post_get_abort_checkpoint() {
    let io = Transport::new([reply(4, 0, 3, 2, &[]), reply(4, 1, 0x83, 2, &[0x2c, 0x01])]);
    let abort = Cell::new(false);
    let waits = Cell::new(0);
    let result = SleepProtocol::new(&io, capability(2636).unwrap(), |_| {
        waits.set(waits.get() + 1);
        if waits.get() == 4 {
            abort.set(true)
        }
        Ok(())
    })
    .execute_with_cancel(&set(5, true), || abort.get())
    .unwrap();
    assert!(abort.get());
    assert_eq!(result.completion, SleepCompletion::Confirmed);
}

#[test]
fn malformed_input_and_misrepresented_readbacks_are_rejected() {
    for value in [
        serde_json::json!({"value":5}),
        serde_json::json!({"value":-1,"isEnabled":true}),
        serde_json::json!({"value":1093,"isEnabled":true}),
        serde_json::json!({"value":5,"isEnabled":true,"extra":0}),
    ] {
        assert!(serde_json::from_value::<PowerSaving>(value).is_err());
    }
    assert!(PowerSaving::new(1093, true).is_err());
    assert!(capability(2629).is_none());
    assert!(capability(2636).unwrap().accepts(2636));
    assert!(!capability(2636).unwrap().accepts(2676));
    assert!(capability(2676).unwrap().accepts(2681));
    let result = SleepResult {
        write: SleepWriteStatus::Acknowledged,
        read: SleepReadStatus::Observed {
            observation: SleepObservation { seconds: 301 },
        },
        completion: SleepCompletion::Confirmed,
    };
    assert!(result.validate_for(&set(5, true)).is_err());
}

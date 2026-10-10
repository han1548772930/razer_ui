//! Simulated transport regressions for partial writes; no OS or device I/O.
use super::*;
use std::{cell::RefCell, collections::VecDeque};

struct Transport {
    responses: RefCell<VecDeque<anyhow::Result<Vec<u8>>>>,
    sent: RefCell<Vec<Vec<u8>>>,
    send_error: bool,
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.borrow_mut().push(report.to_vec());
        if self.send_error {
            anyhow::bail!("uncertain send failure");
        }
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let reply = self
            .responses
            .borrow_mut()
            .pop_front()
            .expect("unexpected extra read")?;
        report.copy_from_slice(&reply);
        Ok(reply.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn transport(replies: Vec<anyhow::Result<Vec<u8>>>) -> Transport {
    Transport {
        responses: RefCell::new(replies.into()),
        sent: RefCell::new(Vec::new()),
        send_error: false,
    }
}
fn capability() -> ReceiverCapability {
    let mut cap = super::super::receiver_capabilities::capability(164)
        .unwrap()
        .clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap
}
fn transactions() -> impl FnMut() -> anyhow::Result<u8> {
    let mut counter = 0;
    move || {
        let transaction = 224 | counter;
        counter += 1;
        Ok(transaction)
    }
}
// Independent current-source fixtures: command/profile/region in source bytes.
fn reply(transaction: u8, command_id: u8, raw: u8) -> anyhow::Result<Vec<u8>> {
    let mut packet = vec![0; 91];
    packet[1] = 2;
    packet[2] = transaction;
    packet[6..12].copy_from_slice(&[3, 15, command_id, 1, 15, raw]);
    Ok(packet)
}

#[test]
fn unchanged_real_percentage_does_not_send_a_setter() {
    let device = transport(vec![reply(224, 132, 255)]);
    let result = apply(&device, &capability(), 100, transactions(), || Ok(())).unwrap();
    assert!(result.source_completed);
    assert!(!result.write_sent && !result.write_attempted);
    assert!(result.acknowledged.is_none());
    assert_eq!(result.observed.unwrap().raw_value, 255);
    let reports = device.sent.borrow();
    assert_eq!(reports.len(), 1);
    assert_eq!(&reports[0][6..12], &[3, 15, 132, 1, 15, 0]);
}

#[test]
fn original_write_stops_after_setter_response_without_an_extra_getter() {
    let device = transport(vec![reply(224, 132, 0), reply(225, 4, 127)]);
    let result = apply(&device, &capability(), 50, transactions(), || Ok(())).unwrap();
    assert!(result.write_sent && result.write_attempted);
    assert!(result.source_completed);
    assert_eq!(result.previous.percent, 0);
    assert_eq!(result.acknowledged.unwrap().raw_value, 127);
    assert!(result.observed.is_none());
    assert!(result.error.is_none());
    assert_eq!(
        device.sent.borrow().len(),
        2,
        "original TaskRunner has no post-setter getter"
    );
}

#[test]
fn source_task_does_not_add_a_requested_value_comparison_to_setter_response() {
    let device = transport(vec![reply(224, 132, 0), reply(225, 4, 127)]);
    let result = apply(&device, &capability(), 100, transactions(), || Ok(())).unwrap();
    assert!(result.write_sent);
    assert!(result.source_completed);
    assert_eq!(result.acknowledged.unwrap().percent, 50);
    assert!(result.observed.is_none());
    assert!(result.error.is_none());
    assert_eq!(device.sent.borrow().len(), 2);
}

#[test]
fn source_unsupported_status_does_not_retry_the_mutation() {
    let mut unsupported = reply(225, 4, 255).unwrap();
    unsupported[1] = 5;
    let device = transport(vec![reply(224, 132, 0), Ok(unsupported)]);
    let result = apply(&device, &capability(), 100, transactions(), || Ok(())).unwrap();
    assert!(result.write_sent && result.write_attempted);
    assert!(!result.source_completed);
    assert!(result.acknowledged.is_none() && result.observed.is_none());
    assert_eq!(device.sent.borrow().len(), 2);
}

#[test]
fn an_uncertain_send_is_reported_without_resending_or_fabricating_an_ack() {
    let device = transport(vec![reply(224, 132, 0)]);
    let mut sends = 0;
    struct FailingSetter<'a> {
        base: &'a Transport,
        calls: RefCell<&'a mut usize>,
    }
    impl FeatureTransport for FailingSetter<'_> {
        fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
            **self.calls.borrow_mut() += 1;
            if report[8] == 4 {
                self.base.sent.borrow_mut().push(report.to_vec());
                anyhow::bail!("uncertain setter failure");
            }
            self.base.send_feature(report)
        }
        fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
            self.base.get_feature(report)
        }
        fn metadata(&self) -> serde_json::Value {
            serde_json::Value::Null
        }
    }
    let failed = FailingSetter {
        base: &device,
        calls: RefCell::new(&mut sends),
    };
    let result = apply(&failed, &capability(), 100, transactions(), || Ok(())).unwrap();
    assert!(result.write_attempted);
    assert!(!result.write_sent && !result.source_completed);
    assert!(result.acknowledged.is_none());
    assert!(result.error.unwrap().contains("uncertain setter failure"));
    drop(failed);
    assert_eq!(sends, 2);
}

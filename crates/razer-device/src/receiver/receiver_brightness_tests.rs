//! Simulated transport regressions for partial writes; no OS or device I/O.
use super::*;
use std::{cell::RefCell, collections::VecDeque};

struct Transport {
    responses: RefCell<VecDeque<anyhow::Result<Vec<u8>>>>,
    sent: RefCell<Vec<Vec<u8>>>,
    send_error: bool,
    collection_sends: std::cell::Cell<usize>,
    get_selectors: RefCell<Vec<u8>>,
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.borrow_mut().push(report.to_vec());
        if self.send_error {
            return Err(crate::backend::native_io_failure(
                crate::backend::NativeIoOutcome::Rejected,
                anyhow::anyhow!("uncertain send failure"),
            ));
        }
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let reply = self
            .responses
            .borrow_mut()
            .pop_front()
            .expect("unexpected extra read")?;
        report[..reply.len()].copy_from_slice(&reply);
        Ok(reply.len())
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.collection_sends.set(self.collection_sends.get() + 1);
        self.send_feature(report)
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.get_selectors.borrow_mut().push(report[0]);
        self.get_feature(report)
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
        collection_sends: std::cell::Cell::new(0),
        get_selectors: RefCell::new(Vec::new()),
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
    assert_eq!(reports[0][0], 0);
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
fn setter_release_failure_retains_its_ack_without_completing_the_source_task() {
    let device = transport(vec![reply(224, 132, 0), reply(225, 4, 127)]);
    let finish_check = || {
        ensure!(
            device.sent.borrow().last().unwrap()[8] != 4,
            "simulated setter release failure"
        );
        Ok(())
    };
    let probe =
        crate::feature_exchange::tests::ExchangeProbe::with_finish_check(&device, &finish_check);
    let result = apply(&probe, &capability(), 50, transactions(), || Ok(())).unwrap();
    assert!(!result.source_completed);
    assert!(result.write_attempted && result.write_sent);
    assert_eq!(result.acknowledged.as_ref().unwrap().raw_value, 127);
    assert!(result.observed.is_none());
    assert!(
        result
            .error
            .as_deref()
            .unwrap()
            .contains("setter release failure")
    );
    assert_eq!(device.sent.borrow().len(), 2);
    assert_eq!(device.get_selectors.borrow().len(), 2);
    assert!(device.responses.borrow().is_empty());
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
                return Err(crate::backend::native_io_failure(
                    crate::backend::NativeIoOutcome::Rejected,
                    anyhow::anyhow!("uncertain setter failure"),
                ));
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

#[test]
fn receiver_write_progress_uses_terminal_native_facts_when_the_owner_is_canceled() {
    use crate::backend::{NativeIoOutcome, native_io_failure, native_io_outcome};

    struct SendBoundary {
        outcome: Option<NativeIoOutcome>,
        calls: std::cell::Cell<usize>,
    }
    impl FeatureTransport for SendBoundary {
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            panic!("receiver must retain the counted native boundary")
        }
        fn send_feature_at_collection_max_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
            assert_eq!(report.len(), 91);
            assert_eq!(&report[6..12], &[3, 15, 4, 1, 15, 127]);
            self.calls.set(self.calls.get() + 1);
            let error = anyhow::anyhow!("receiver adapter stopped");
            Err(match self.outcome {
                Some(outcome) => native_io_failure(outcome, error),
                None => error,
            })
        }
        fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
            panic!("failed or stale sends must not read a response")
        }
        fn metadata(&self) -> serde_json::Value {
            serde_json::Value::Null
        }
    }

    for (outcome, attempted, sent) in [
        (None, false, false),
        (Some(NativeIoOutcome::NotCalled), false, false),
        (Some(NativeIoOutcome::Rejected), true, false),
        (
            Some(NativeIoOutcome::Returned { byte_count: 0 }),
            true,
            false,
        ),
        (
            Some(NativeIoOutcome::Returned { byte_count: 91 }),
            true,
            true,
        ),
    ] {
        let io = SendBoundary {
            outcome,
            calls: std::cell::Cell::new(0),
        };
        let mut progress = crate::receiver_feature_channel::WriteProgress::default();
        let error = crate::receiver_feature_channel::exchange(
            &io,
            &capability(),
            [3, 15, 4],
            &[1, 15, 127],
            224,
            &|| {
                ensure!(io.calls.get() == 0, "receiver owner canceled during send");
                Ok(())
            },
            &mut progress,
        )
        .unwrap_err();
        assert_eq!((progress.attempted, progress.sent), (attempted, sent));
        assert!(!progress.acknowledged);
        assert!(progress.acknowledged_reply.is_none());
        let retained = error
            .downcast_ref::<crate::ReceiverExchangeFailure>()
            .unwrap();
        assert_eq!(
            (retained.progress.attempted, retained.progress.sent),
            (attempted, sent)
        );
        assert_eq!(native_io_outcome(&error), outcome);
        assert_eq!(io.calls.get(), 1);
        assert!(format!("{error:#}").contains("receiver owner canceled during send"));
    }
}

#[test]
fn busy_then_an_in_exception_retains_original_out_retry_state() {
    let mut busy = reply(224, 132, 0).unwrap();
    busy[1] = 1;
    let device = transport(vec![
        Ok(busy),
        Err(anyhow::anyhow!("IN interrupted after busy")),
        reply(224, 132, 255),
    ]);
    let value = read(&device, &capability(), 224, || Ok(())).unwrap();
    assert_eq!(value.percent, 100);
    assert_eq!(device.sent.borrow().len(), 2);
    assert_eq!(device.sent.borrow()[0], device.sent.borrow()[1]);
}

#[test]
fn busy_then_terminal_owner_failure_preserves_the_error_without_retrying_out() {
    let mut busy = reply(224, 132, 0).unwrap();
    busy[1] = 1;
    let device = transport(vec![
        Ok(busy),
        Err(crate::backend::finalization_failure(anyhow::anyhow!(
            "receiver native owner retired"
        ))),
        reply(224, 132, 255),
    ]);
    let error = read(&device, &capability(), 224, || Ok(())).unwrap_err();
    assert!(crate::backend::exchange_finalization_failed(&error));
    assert_eq!(
        error.root_cause().to_string(),
        "receiver native owner retired"
    );
    assert_eq!(device.sent.borrow().len(), 1);
    assert_eq!(device.get_selectors.borrow().len(), 2);
    assert_eq!(device.responses.borrow().len(), 1);
}

#[test]
fn an_in_exception_without_busy_does_not_resend_out() {
    let device = transport(vec![Err(anyhow::anyhow!("first IN interrupted"))]);
    assert!(read(&device, &capability(), 224, || Ok(())).is_err());
    assert_eq!(device.sent.borrow().len(), 1);
}

#[test]
fn a_new_out_resets_the_previous_busy_retry_flag() {
    let mut busy = reply(224, 132, 0).unwrap();
    busy[1] = 1;
    let mut cap = capability();
    cap.max_retry_in = 1;
    let device = transport(vec![Ok(busy), Err(anyhow::anyhow!("next OUT first IN"))]);
    assert!(read(&device, &cap, 224, || Ok(())).is_err());
    assert_eq!(device.sent.borrow().len(), 2);
}

#[test]
fn response_profile_and_region_are_observations_not_extra_conditions() {
    let mut response = reply(224, 132, 127).unwrap();
    response[9] = 7;
    response[10] = 4;
    let device = transport(vec![Ok(response)]);
    let value = read(&device, &capability(), 224, || Ok(())).unwrap();
    assert_eq!(
        (value.profile_id, value.region_id, value.percent),
        (7, 4, 50)
    );
}

#[test]
fn response_slice_uses_the_actual_transport_length() {
    let mut response = reply(224, 132, 255).unwrap();
    response[6] = 80;
    response.truncate(12);
    let device = transport(vec![Ok(response)]);
    assert_eq!(
        read(&device, &capability(), 224, || Ok(()))
            .unwrap()
            .percent,
        100
    );
}

#[test]
fn serial_query_uses_the_same_source_exception_retry_state() {
    use super::super::receiver_identity::read_serial;
    let mut response = reply(224, 130, 0).unwrap();
    response[7] = 0;
    response[6] = 6;
    response[9..15].copy_from_slice(b"SERIAL");
    let mut busy = response.clone();
    busy[1] = 1;
    let device = transport(vec![
        Ok(busy),
        Err(anyhow::anyhow!("IN failed")),
        Ok(response),
    ]);
    assert_eq!(
        read_serial(&device, &capability(), 224, || Ok(())).unwrap(),
        "SERIAL"
    );
    assert_eq!(device.sent.borrow().len(), 2);

    let first_in_error = transport(vec![Err(anyhow::anyhow!("first IN failed"))]);
    assert!(read_serial(&first_in_error, &capability(), 224, || Ok(())).is_err());
    assert_eq!(first_in_error.sent.borrow().len(), 1);

    let mut send_error = transport(vec![]);
    send_error.send_error = true;
    assert!(read_serial(&send_error, &capability(), 224, || Ok(())).is_err());
    assert_eq!(send_error.sent.borrow().len(), 1);
}

#[test]
fn serial_parser_preserves_an_empty_source_ascii_result() {
    use super::super::receiver_identity::read_serial;
    let mut response = reply(224, 130, 0).unwrap();
    response[7] = 0;
    let device = transport(vec![Ok(response)]);
    assert_eq!(
        read_serial(&device, &capability(), 224, || Ok(())).unwrap(),
        ""
    );
}

#[test]
fn linker_brightness_and_serial_use_host_get_selector_without_matching_returned_prefix() {
    use super::super::receiver_identity::read_serial;
    for (pid, selector) in [(164, 0), (241, 3)] {
        let mut cap = super::super::receiver_capabilities::capability(pid)
            .unwrap()
            .clone();
        cap.sleep_between_out_ms = 0;
        cap.sleep_between_out_in_ms = 0;
        cap.sleep_between_in_ms = 0;
        let mut before = reply(224, 132, 0).unwrap();
        let mut ack = reply(225, 4, 127).unwrap();
        let mut serial = reply(226, 130, 0).unwrap();
        before[0] = 9;
        ack[0] = 9;
        serial[0] = 9;
        serial[7] = 0;
        serial[6] = 6;
        serial[9..15].copy_from_slice(b"SERIAL");
        let device = transport(vec![Ok(before), Ok(ack), Ok(serial)]);
        let result = apply(&device, &cap, 50, transactions(), || Ok(())).unwrap();
        assert!(result.source_completed && result.write_sent);
        assert!(result.observed.is_none());
        assert_eq!(
            read_serial(&device, &cap, 226, || Ok(())).unwrap(),
            "SERIAL"
        );
        assert_eq!(device.collection_sends.get(), 3);
        assert_eq!(*device.get_selectors.borrow(), [selector; 3]);
        assert!(
            device
                .sent
                .borrow()
                .iter()
                .all(|packet| packet[0] == selector)
        );
    }
}

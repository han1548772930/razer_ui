//! Current sendCommand/_getUSBTransferInResult retry paths, exercised without HID.
use super::*;
use crate::device_reads::{DeviceReadKind, DeviceReadValue};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

enum Input {
    Status(u8),
    Stale,
    Failed,
    Short,
}

struct Transport {
    inputs: RefCell<VecDeque<Input>>,
    outputs: RefCell<Vec<Vec<u8>>>,
}

impl Transport {
    fn new(inputs: impl IntoIterator<Item = Input>) -> Self {
        Self {
            inputs: RefCell::new(inputs.into_iter().collect()),
            outputs: RefCell::new(Vec::new()),
        }
    }
}

impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.outputs.borrow_mut().push(report.to_vec());
        Ok(())
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let input = self
            .inputs
            .borrow_mut()
            .pop_front()
            .expect("unexpected IN operation");
        if matches!(input, Input::Failed) {
            anyhow::bail!("simulated transport IN failure");
        }
        let outputs = self.outputs.borrow();
        report.copy_from_slice(outputs.last().expect("IN must follow OUT"));
        report[1] = match input {
            Input::Status(status) => status,
            _ => 2,
        };
        report[6] = 2;
        report[9..11].copy_from_slice(&[0, 255]);
        if matches!(input, Input::Stale) {
            report[2] ^= 1;
        }
        Ok(if matches!(input, Input::Short) {
            report.len() - 1
        } else {
            report.len()
        })
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn battery_cap() -> (DeviceReadCapability, ReadCommand) {
    let mut cap = device_reads::capability(182).unwrap().clone();
    // Keep the original retry count; suppress wall-clock delays for simulation.
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    let command = cap
        .queries
        .iter()
        .find(|q| q.name == DeviceReadKind::Battery)
        .unwrap()
        .clone();
    (cap, command)
}

#[test]
fn busy_polls_input_without_resending_and_success_decodes_battery() {
    let (cap, command) = battery_cap();
    let transport = Transport::new([Input::Status(1), Input::Status(2)]);
    let value = read_device(&transport, &cap, &command, 7, || Ok(())).unwrap();
    assert!(matches!(value, DeviceReadValue::Battery { percent: 100 }));
    assert_eq!(transport.outputs.borrow().len(), 1);
    assert!(transport.inputs.borrow().is_empty());
}

#[test]
fn stale_or_failed_input_resends_same_transaction_before_accepting_value() {
    for first in [Input::Stale, Input::Failed, Input::Status(3)] {
        let (cap, command) = battery_cap();
        let transport = Transport::new([first, Input::Status(2)]);
        assert!(matches!(
            read_device(&transport, &cap, &command, 13, || Ok(())).unwrap(),
            DeviceReadValue::Battery { percent: 100 }
        ));
        let outputs = transport.outputs.borrow();
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0], outputs[1]);
        assert_eq!(outputs[0][2], 13);
    }
}

#[test]
fn truncated_report_cannot_be_promoted_to_success_or_retried_as_complete() {
    let (cap, command) = battery_cap();
    let transport = Transport::new([Input::Short]);
    assert!(read_device(&transport, &cap, &command, 7, || Ok(())).is_err());
    assert_eq!(transport.outputs.borrow().len(), 1);
}

#[test]
fn validation_failure_after_input_retains_value_only_as_failure_diagnostics() {
    let (cap, command) = battery_cap();
    let transport = Transport::new([Input::Status(2)]);
    let result = read_device(&transport, &cap, &command, 7, || {
        anyhow::ensure!(
            !transport.inputs.borrow().is_empty(),
            "route disappeared after IN"
        );
        Ok(())
    });
    assert!(result.is_err());
    assert!(matches!(
        completed_reading(&result),
        Some(DeviceReadValue::Battery { percent: 100 })
    ));
    assert_eq!(
        crate::backend::native_io_outcome(result.as_ref().unwrap_err()),
        Some(crate::backend::NativeIoOutcome::Returned { byte_count: 91 })
    );
    assert_eq!(
        crate::backend::native_read_bytes(result.as_ref().unwrap_err())
            .unwrap()
            .len(),
        91
    );
    assert_eq!(
        result.unwrap_err().root_cause().to_string(),
        "route disappeared after IN"
    );
    assert_eq!(transport.outputs.borrow().len(), 1);
    assert!(transport.inputs.borrow().is_empty());
}

#[test]
fn a_release_error_retains_a_decoded_reply_only_as_failure_diagnostics() {
    let (cap, command) = battery_cap();
    let transport = Transport::new([Input::Status(2)]);
    let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&transport);
    let result = read_device(&probe, &cap, &command, 7, || Ok(()));
    assert!(result.is_err());
    assert!(matches!(
        completed_reading(&result),
        Some(DeviceReadValue::Battery { percent: 100 })
    ));
    assert!(format!("{:#}", result.unwrap_err()).contains("simulated explicit release failed"));
    assert_eq!(transport.outputs.borrow().len(), 1);
    assert!(transport.inputs.borrow().is_empty());
}

#[test]
fn a_release_error_cannot_turn_a_truncated_reply_into_decoded_evidence() {
    let (cap, command) = battery_cap();
    let transport = Transport::new([Input::Short]);
    let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&transport);
    let result = read_device(&probe, &cap, &command, 7, || Ok(()));
    assert!(result.is_err());
    assert!(completed_reading(&result).is_none());
    assert_eq!(transport.outputs.borrow().len(), 1);
    assert!(transport.inputs.borrow().is_empty());
}

#[test]
fn exhausted_source_retry_budget_is_an_error_not_an_empty_battery() {
    let (cap, command) = battery_cap();
    let transport = Transport::new((0..cap.max_retry_out).map(|_| Input::Status(3)));
    assert!(read_device(&transport, &cap, &command, 7, || Ok(())).is_err());
    assert_eq!(
        transport.outputs.borrow().len(),
        usize::from(cap.max_retry_out)
    );
    assert!(transport.inputs.borrow().is_empty());
}

#[test]
fn cancellation_before_out_does_not_submit_a_report() {
    let (cap, command) = battery_cap();
    let transport = Transport::new([]);
    assert!(read_device(&transport, &cap, &command, 7, || anyhow::bail!("cancelled")).is_err());
    assert!(transport.outputs.borrow().is_empty());
}

#[derive(Debug)]
struct NativeQueryFailure;

impl std::fmt::Display for NativeQueryFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("native query result rejected before publication")
    }
}

impl std::error::Error for NativeQueryFailure {}

#[derive(Debug)]
struct QueryOwnerExpired;

impl std::fmt::Display for QueryOwnerExpired {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("query owner expired after native return")
    }
}

impl std::error::Error for QueryOwnerExpired {}

#[derive(Clone, Copy)]
enum TerminalRead {
    Returned(usize),
    RetainedError(usize),
    UnprovenError(Option<crate::backend::NativeIoOutcome>),
}

struct TerminalTransport {
    response: Vec<u8>,
    result: TerminalRead,
    current: Cell<bool>,
    retire_after_send: bool,
    retire_after_get: bool,
    send_error: Option<Option<crate::backend::NativeIoOutcome>>,
    outputs: Cell<usize>,
    inputs: Cell<usize>,
}

impl TerminalTransport {
    fn new(response: Vec<u8>, result: TerminalRead, retire_after_get: bool) -> Self {
        Self {
            response,
            result,
            current: Cell::new(true),
            retire_after_send: false,
            retire_after_get,
            send_error: None,
            outputs: Cell::new(0),
            inputs: Cell::new(0),
        }
    }

    fn deadline(&self) -> anyhow::Result<()> {
        if self.current.get() {
            Ok(())
        } else {
            Err(QueryOwnerExpired.into())
        }
    }
}

impl FeatureTransport for TerminalTransport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        crate::backend::require_complete_send(self.send_feature_counted(report)?, report.len())
    }

    fn send_feature_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        assert_eq!(
            self.outputs.get(),
            0,
            "terminal query must never repeat OUT"
        );
        self.outputs.set(1);
        if self.retire_after_send {
            self.current.set(false);
        }
        match self.send_error {
            None => Ok(report.len()),
            Some(outcome) => {
                let error = NativeQueryFailure.into();
                Err(match outcome {
                    Some(outcome) => crate::backend::native_io_failure(outcome, error),
                    None => error,
                })
            }
        }
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(self.inputs.get(), 0, "terminal query must never repeat IN");
        self.inputs.set(1);
        report.copy_from_slice(&self.response);
        if self.retire_after_get {
            self.current.set(false);
        }
        match self.result {
            TerminalRead::Returned(count) => Ok(count),
            TerminalRead::RetainedError(count) => Err(crate::backend::complete_native_read(
                Ok(count),
                report,
                NativeQueryFailure.into(),
            )),
            TerminalRead::UnprovenError(outcome) => {
                let error = NativeQueryFailure.into();
                Err(match outcome {
                    Some(outcome) => crate::backend::native_io_failure(outcome, error),
                    None => error,
                })
            }
        }
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn query_reply(command: [u8; 3], transaction: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 91];
    bytes[1] = 2;
    bytes[2] = transaction;
    bytes[6..9].copy_from_slice(&command);
    bytes[6] = payload.len() as u8;
    bytes[9..9 + payload.len()].copy_from_slice(payload);
    bytes
}

fn receiver_cap() -> crate::receiver_capabilities::ReceiverCapability {
    let mut cap = crate::receiver_capabilities::capability(241)
        .unwrap()
        .clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap
}

#[test]
fn terminal_device_read_receipts_share_the_normal_decoder_and_stop_retries() {
    let (cap, command) = battery_cap();
    for (case, count, accepted) in [
        (0, 91, true),
        (1, 91, false),
        (2, 91, false),
        (3, 91, false),
        (4, 91, false),
        (5, 91, false),
        (6, 91, false),
        (7, 91, false),
        (0, 0, false),
        (0, 90, false),
    ] {
        let mut bytes = query_reply(command.command, 7, &[0, 255]);
        match case {
            1 => bytes[1] = 1,
            2 => bytes[2] ^= 1,
            3 => bytes[7] ^= 1,
            4 => bytes[8] ^= 1,
            5 => bytes[6] = 1,
            6 => bytes[9] = 1,
            7 => bytes[1] = 5,
            _ => {}
        }
        for retire_after_get in [false, true] {
            let transport = TerminalTransport::new(
                bytes.clone(),
                TerminalRead::RetainedError(count),
                retire_after_get,
            );
            let result = read_device(&transport, &cap, &command, 7, || transport.deadline());
            let error = result.as_ref().unwrap_err();
            assert!(error.is::<NativeQueryFailure>());
            assert_eq!(completed_reading(&result).is_some(), accepted);
            if accepted {
                assert!(matches!(
                    completed_reading(&result),
                    Some(DeviceReadValue::Battery { percent: 100 })
                ));
            }
            assert_eq!(
                crate::backend::native_io_outcome(error),
                Some(crate::backend::NativeIoOutcome::Returned { byte_count: count })
            );
            assert_eq!(crate::backend::native_read_bytes(error), bytes.get(..count));
            assert_eq!(transport.outputs.get(), 1);
            assert_eq!(transport.inputs.get(), 1);
        }
    }
}

#[test]
fn query_failures_with_counts_but_without_owned_input_bytes_never_create_readings() {
    use crate::backend::NativeIoOutcome;
    let (device_cap, command) = battery_cap();
    let receiver_cap = receiver_cap();
    for receiver in [false, true] {
        let bytes = if receiver {
            query_reply(receiver_cap.command, 7, &[2, 1, 0, 182, 0xfe, 0xab, 0xcd])
        } else {
            query_reply(command.command, 7, &[0, 255])
        };
        for outcome in [
            None,
            Some(NativeIoOutcome::NotCalled),
            Some(NativeIoOutcome::Rejected),
            Some(NativeIoOutcome::Returned { byte_count: 91 }),
        ] {
            let transport =
                TerminalTransport::new(bytes.clone(), TerminalRead::UnprovenError(outcome), true);
            let error = if receiver {
                read_receiver(&transport, &receiver_cap, 7, || transport.deadline()).unwrap_err()
            } else {
                read_device(&transport, &device_cap, &command, 7, || {
                    transport.deadline()
                })
                .unwrap_err()
            };
            assert!(error.is::<NativeQueryFailure>());
            assert_eq!(crate::backend::native_io_outcome(&error), outcome);
            assert!(crate::backend::native_read_bytes(&error).is_none());
            assert!(CompletedDeviceRead::from_error(&error).is_none());
            assert!(CompletedReceiverRead::from_error(&error).is_none());
            assert_eq!(transport.outputs.get(), 1);
            assert_eq!(transport.inputs.get(), 1);
        }
    }
}

#[test]
fn canceled_queries_preserve_exact_native_counts_without_promoting_short_input() {
    let (device_cap, command) = battery_cap();
    let receiver_cap = receiver_cap();
    for receiver in [false, true] {
        let bytes = if receiver {
            query_reply(receiver_cap.command, 7, &[2, 1, 0, 182, 0xfe, 0xab, 0xcd])
        } else {
            query_reply(command.command, 7, &[0, 255])
        };
        for byte_count in [0, 90, 91, 92] {
            let transport =
                TerminalTransport::new(bytes.clone(), TerminalRead::Returned(byte_count), true);
            let error = if receiver {
                read_receiver(&transport, &receiver_cap, 7, || transport.deadline()).unwrap_err()
            } else {
                read_device(&transport, &device_cap, &command, 7, || {
                    transport.deadline()
                })
                .unwrap_err()
            };
            assert!(error.is::<QueryOwnerExpired>());
            assert_eq!(
                crate::backend::native_io_outcome(&error),
                Some(crate::backend::NativeIoOutcome::Returned { byte_count })
            );
            assert_eq!(
                crate::backend::native_read_bytes(&error),
                bytes.get(..byte_count)
            );
            if receiver {
                assert_eq!(
                    CompletedReceiverRead::from_error(&error).is_some(),
                    byte_count == 91
                );
                assert!(CompletedDeviceRead::from_error(&error).is_none());
            } else {
                assert_eq!(
                    CompletedDeviceRead::from_error(&error).is_some(),
                    byte_count == 91
                );
                assert!(CompletedReceiverRead::from_error(&error).is_none());
            }
            assert_eq!(transport.outputs.get(), 1);
            assert_eq!(transport.inputs.get(), 1);
        }
    }
}

#[test]
fn query_deadlines_keep_actual_send_counts_and_original_native_failure_causes() {
    use crate::backend::NativeIoOutcome;
    let (device_cap, command) = battery_cap();
    let receiver_cap = receiver_cap();
    for receiver in [false, true] {
        for send_error in [
            None,
            Some(None),
            Some(Some(NativeIoOutcome::NotCalled)),
            Some(Some(NativeIoOutcome::Rejected)),
            Some(Some(NativeIoOutcome::Returned { byte_count: 37 })),
        ] {
            let mut transport =
                TerminalTransport::new(vec![0; 91], TerminalRead::Returned(91), false);
            transport.retire_after_send = true;
            transport.send_error = send_error;
            let error = if receiver {
                read_receiver(&transport, &receiver_cap, 7, || transport.deadline()).unwrap_err()
            } else {
                read_device(&transport, &device_cap, &command, 7, || {
                    transport.deadline()
                })
                .unwrap_err()
            };
            assert_eq!(
                crate::backend::native_io_outcome(&error),
                send_error.unwrap_or(Some(NativeIoOutcome::Returned { byte_count: 91 }))
            );
            assert_eq!(error.is::<NativeQueryFailure>(), send_error.is_some());
            if send_error.is_none() {
                assert!(error.is::<QueryOwnerExpired>());
            }
            assert!(CompletedDeviceRead::from_error(&error).is_none());
            assert!(CompletedReceiverRead::from_error(&error).is_none());
            assert_eq!(transport.outputs.get(), 1);
            assert_eq!(transport.inputs.get(), 0);
        }
    }
}

#[test]
fn receiver_read_terminal_input_and_release_failures_keep_only_valid_device_lists() {
    let cap = receiver_cap();
    for payload in [vec![0], vec![2, 1, 0, 182, 0xfe, 0xab, 0xcd]] {
        let expected = if payload[0] == 0 {
            Vec::new()
        } else {
            vec![(182, 1), (0xabcd, 0xfe)]
        };
        for retire_after_get in [false, true] {
            let bytes = query_reply(cap.command, 7, &payload);
            let transport =
                TerminalTransport::new(bytes, TerminalRead::RetainedError(91), retire_after_get);
            let error = read_receiver(&transport, &cap, 7, || transport.deadline()).unwrap_err();
            assert!(error.is::<NativeQueryFailure>());
            assert_eq!(
                CompletedReceiverRead::from_error(&error).unwrap().devices(),
                expected
            );
            assert_eq!(transport.outputs.get(), 1);
            assert_eq!(transport.inputs.get(), 1);
        }
        let bytes = query_reply(cap.command, 7, &payload);
        let transport = TerminalTransport::new(bytes, TerminalRead::Returned(91), false);
        let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&transport);
        let error = read_receiver(&probe, &cap, 7, || Ok(())).unwrap_err();
        assert!(crate::backend::exchange_finalization_failed(&error));
        assert_eq!(
            CompletedReceiverRead::from_error(&error).unwrap().devices(),
            expected
        );
        assert_eq!(transport.outputs.get(), 1);
        assert_eq!(transport.inputs.get(), 1);
    }
}

#[test]
fn rejected_receiver_inputs_never_become_an_empty_or_current_device_list() {
    let cap = receiver_cap();
    for (case, count) in [
        (0, 0),
        (0, 90),
        (1, 91),
        (2, 91),
        (3, 91),
        (4, 91),
        (5, 91),
        (6, 91),
    ] {
        let mut bytes = query_reply(cap.command, 7, &[1, 1, 0, 182]);
        match case {
            1 => bytes[1] = 1,
            2 => bytes[2] ^= 1,
            3 => bytes[7] ^= 1,
            4 => bytes[8] ^= 1,
            5 => bytes[6] = 3,
            6 => bytes[1] = 5,
            _ => {}
        }
        let transport = TerminalTransport::new(bytes, TerminalRead::RetainedError(count), false);
        let error = read_receiver(&transport, &cap, 7, || transport.deadline()).unwrap_err();
        assert!(error.is::<NativeQueryFailure>());
        assert!(CompletedReceiverRead::from_error(&error).is_none());
        assert_eq!(transport.outputs.get(), 1);
        assert_eq!(transport.inputs.get(), 1);
    }
}

#[test]
fn receiver_owner_loss_after_full_input_or_matched_release_never_returns_success() {
    let cap = receiver_cap();
    let expected = [(182, 1), (0xabcd, 0xfe)];
    for during_get in [false, true] {
        let bytes = query_reply(cap.command, 7, &[2, 1, 0, 182, 0xfe, 0xab, 0xcd]);
        let transport = TerminalTransport::new(bytes, TerminalRead::Returned(91), during_get);
        let retire = || transport.current.set(false);
        let probe =
            crate::feature_exchange::tests::ExchangeProbe::with_on_finish(&transport, &retire);
        let error = read_receiver(&probe, &cap, 7, || transport.deadline()).unwrap_err();
        assert!(error.is::<QueryOwnerExpired>());
        assert_eq!(
            crate::backend::exchange_release_completed(&error),
            !during_get
        );
        assert_eq!(
            CompletedReceiverRead::from_error(&error).unwrap().devices(),
            expected
        );
        assert_eq!(transport.outputs.get(), 1);
        assert_eq!(transport.inputs.get(), 1);
    }
}

#[derive(Default)]
struct FramingTransport {
    sent: RefCell<Vec<u8>>,
    get_ids: RefCell<Vec<u8>>,
    node: Option<crate::backend::HidNode>,
}

impl FeatureTransport for FramingTransport {
    fn hid_node(&self) -> Option<&crate::backend::HidNode> {
        self.node.as_ref()
    }

    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("source host SET must opt into the observed collection maximum")
    }

    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.replace(report.to_vec());
        Ok(())
    }

    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        anyhow::bail!("source host GET must opt into the observed collection maximum")
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.get_ids.borrow_mut().push(report[0]);
        report.copy_from_slice(&self.sent.borrow());
        // Source host slice(1) does not require the returned prefix to equal
        // either the GET selector or the distinct SET Report ID.
        report[0] = 0x7f;
        report[1] = 2;
        report[6] = 4;
        report[9..13].copy_from_slice(&[1, 2, 3, 4]);
        Ok(report.len())
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

#[test]
fn physical_interface_get_selector_is_independent_of_set_and_returned_prefix() {
    // Current wired PID602 uses interface 2; wired PID664 uses 3. Binding must
    // not pick PID662's first map row for PID664's direct transport.
    for (product, interface) in [
        (124, 0),
        (143, 0),
        (148, 0),
        (156, 0),
        (165, 0),
        (167, 0),
        (170, 0),
        (175, 0),
        (180, 0),
        (182, 0),
        (184, 0),
        (185, 0),
        (192, 0),
        (196, 0),
        (204, 0),
        (211, 0),
        (218, 0),
        (222, 0),
        (226, 3),
        (231, 3),
        (235, 3),
        (600, 3),
        (602, 2),
        (664, 3),
        (697, 3),
        (708, 3),
        (716, 3),
        (717, 3),
        (724, 3),
        (727, 3),
        (45066, 0),
    ] {
        let mut cap = device_reads::capability(product)
            .unwrap()
            .for_physical_product(product)
            .unwrap();
        cap.sleep_between_out_ms = 0;
        cap.sleep_between_out_in_ms = 0;
        cap.sleep_between_in_ms = 0;
        let command = cap
            .queries
            .iter()
            .find(|q| q.name == DeviceReadKind::Firmware)
            .unwrap();
        let transport = FramingTransport::default();
        let value = read_device(
            &transport,
            &cap,
            command,
            cap.transaction_prefix | 3,
            || Ok(()),
        )
        .unwrap();
        assert!(matches!(value, DeviceReadValue::Firmware { version } if version == "1.2.3.4"));
        assert_eq!(transport.sent.borrow()[0], 0);
        assert_eq!(*transport.get_ids.borrow(), [interface]);
        let reply = crate::feature_exchange::exchange(
            &transport,
            &cap,
            command.command,
            &[],
            cap.transaction_prefix | 4,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(reply, [1, 2, 3, 4]);
        assert_eq!(transport.sent.borrow()[0], 0);
        assert_eq!(*transport.get_ids.borrow(), [interface, interface]);
    }
}

#[test]
fn explicit_report_id_overrides_get_interface_without_changing_send() {
    let mut cap = device_reads::capability(602)
        .unwrap()
        .for_physical_product(602)
        .unwrap();
    cap.report_id = 5;
    assert_eq!(cap.get_report_id(), 5);
    let command = cap.queries.first().unwrap();
    assert_eq!(device_reads::query_report(&cap, command, 0).unwrap()[0], 5);
    assert!(
        device_reads::capability(664)
            .unwrap()
            .for_physical_product(9999)
            .is_err()
    );
    assert_eq!(
        device_reads::capability(664)
            .unwrap()
            .for_physical_product(662)
            .unwrap()
            .get_report_id(),
        2
    );
}

fn simulated_node(product_id: u16, interface_number: i32) -> crate::backend::HidNode {
    crate::backend::HidNode {
        path: b"explicit-simulated-collection".to_vec(),
        vendor_id: 0x1532,
        product_id,
        usage_page: 0xff00,
        usage: 1,
        interface_number,
        release_number: 0,
        serial_number: None,
        manufacturer: None,
        product: None,
    }
}

#[test]
fn source_pid_map_uses_the_opened_physical_product_for_reads_and_setter_acks() {
    for (product, physical, interface) in [
        (664, 662, 2),
        (664, 664, 3),
        (708, 706, 2),
        (708, 708, 3),
        (716, 713, 2),
        (716, 716, 3),
        (717, 717, 3),
        (717, 730, 2),
        (724, 722, 2),
        (724, 724, 3),
        (727, 725, 2),
        (727, 727, 3),
    ] {
        let mut cap = device_reads::capability(product).unwrap().clone();
        cap.sleep_between_out_ms = 0;
        cap.sleep_between_out_in_ms = 0;
        cap.sleep_between_in_ms = 0;
        for branch in cap.physical_transports_by_pid.values_mut() {
            branch.sleep_between_out_ms = 0;
            branch.sleep_between_out_in_ms = 0;
            branch.sleep_between_in_ms = 0;
        }
        let command = cap
            .queries
            .iter()
            .find(|q| q.name == DeviceReadKind::Firmware)
            .unwrap();
        // Windows can expose -1 after the service separately verified the
        // interface with SetupAPI. The PID map remains the source of framing.
        let transport = FramingTransport {
            node: Some(simulated_node(physical, -1)),
            ..Default::default()
        };
        let value = read_device(
            &transport,
            &cap,
            command,
            cap.transaction_prefix | 3,
            || Ok(()),
        )
        .unwrap();
        assert!(matches!(value, DeviceReadValue::Firmware { version } if version == "1.2.3.4"));
        let reply = crate::feature_exchange::exchange(
            &transport,
            &cap,
            command.command,
            &[],
            cap.transaction_prefix | 4,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(reply, [1, 2, 3, 4]);
        assert_eq!(*transport.get_ids.borrow(), [interface, interface]);
        assert_eq!(
            transport.sent.borrow()[0],
            if u32::from(physical) == product {
                0
            } else {
                interface
            }
        );
        assert_eq!(
            transport.sent.borrow()[2],
            if u32::from(physical) == product { 4 } else { 128 | 4 }
        );
    }
}

#[test]
fn unresolved_pid_maps_fail_before_any_io_and_bound_masters_keep_source_interface() {
    let cap = device_reads::capability(664).unwrap();
    let command = cap.queries.first().unwrap();
    for node in [
        None,
        Some(simulated_node(9999, 3)),
        Some(simulated_node(663, 3)),
    ] {
        let transport = FramingTransport {
            node,
            ..Default::default()
        };
        assert!(
            read_device(&transport, cap, command, cap.transaction_prefix | 3, || Ok(
                ()
            ))
            .is_err()
        );
        assert!(
            crate::feature_exchange::exchange(
                &transport,
                cap,
                command.command,
                &[],
                cap.transaction_prefix | 4,
                &|| Ok(()),
            )
            .is_err()
        );
        assert!(transport.sent.borrow().is_empty());
        assert!(transport.get_ids.borrow().is_empty());
    }
    let physical = FramingTransport {
        node: Some(simulated_node(664, 3)),
        ..Default::default()
    };
    let bound = cap.for_physical_product(662).unwrap();
    assert!(bound.claim_interfaces_by_pid.is_empty());
    assert_eq!(bound.get_report_id_for_transport(&physical).unwrap(), 2);
    let mut explicit = cap.clone();
    explicit.report_id = 7;
    assert_eq!(
        explicit
            .get_report_id_for_transport(&FramingTransport::default())
            .unwrap(),
        7
    );
}

#[test]
fn every_current_factory_selects_physical_report_ids_and_counter_namespace() {
    for (product, dongle, interface, radio_interface, prefix) in [
        (124, 125, 0, 0, 0),
        (143, 144, 0, 0, 0),
        (148, 148, 0, 0, 0),
        (156, 156, 0, 0, 0),
        (165, 166, 0, 0, 0),
        (167, 168, 0, 0, 0),
        (170, 171, 0, 0, 0),
        (175, 176, 0, 0, 0),
        (180, 180, 0, 0, 0),
        (182, 183, 0, 0, 0),
        (184, 184, 0, 0, 0),
        (185, 185, 0, 0, 0),
        (192, 193, 0, 0, 0),
        (196, 197, 0, 0, 0),
        (204, 205, 0, 0, 0),
        (211, 212, 0, 0, 0),
        (218, 219, 0, 0, 0),
        (222, 223, 0, 0, 0),
        (226, 227, 3, 3, 0),
        (231, 232, 3, 3, 0),
        (235, 234, 3, 3, 0),
        (600, 625, 3, 3, 128),
        (602, 604, 2, 2, 128),
        (664, 662, 3, 2, 128),
        (697, 698, 3, 3, 128),
        (708, 706, 3, 2, 128),
        (716, 713, 3, 2, 128),
        (717, 730, 3, 2, 128),
        (724, 722, 3, 2, 128),
        (727, 725, 3, 2, 128),
        (45066, 45067, 0, 0, 0),
    ] {
        let cap = device_reads::capability(product).unwrap();
        assert!(
            !cap.physical_transports_by_pid.is_empty(),
            "unresolved PID {product}"
        );
        for (physical, claim) in [(product, interface), (dongle, radio_interface)] {
            let bound = cap.for_physical_product(physical).unwrap();
            let dual = physical == dongle;
            assert_eq!(bound.report_id, if dual { claim } else { 0 });
            assert_eq!(bound.get_report_id(), claim);
            assert_eq!(bound.transaction_prefix, if dual { prefix } else { 0 });
            assert_eq!(bound.sleep_between_out_ms, if dual { 25 } else { 5 });
            assert!(bound.physical_transports_by_pid.is_empty());
            let transport = FramingTransport {
                node: Some(simulated_node(physical as u16, i32::from(claim))),
                ..Default::default()
            };
            for counter in [0, 1, 30] {
                let (selected, actual) = cap
                    .for_exchange(&transport, cap.transaction_prefix | counter)
                    .unwrap();
                assert_eq!(actual, (if dual { prefix } else { 0 }) | counter);
                assert_eq!(selected.report_id, bound.report_id);
            }
        }
        // A cached master replaces the transport identity/interface while the
        // selected logical radio class still determines the counter prefix.
        let master = cap.for_duallink_master(241, 3).unwrap();
        assert_eq!(master.direct_pids, [241]);
        assert_eq!(master.report_id, 3);
        assert_eq!(master.get_report_id(), 3);
        assert_eq!(master.transaction_prefix, prefix);
    }
}

#[test]
fn all_nonzero_receiver_interfaces_use_their_selected_sender_prefix() {
    for (pid, interface) in [
        (241, 3),
        (227, 3),
        (232, 3),
        (234, 3),
        (625, 3),
        (604, 2),
        (635, 2),
        (656, 2),
        (662, 2),
        (698, 3),
        (706, 2),
        (713, 2),
        (730, 2),
        (722, 2),
        (725, 2),
        (757, 3),
    ] {
        let cap = crate::receiver_capabilities::capability(pid).unwrap();
        assert_eq!(cap.report_id, interface, "physical receiver {pid}");
        assert_eq!(cap.get_report_id(), interface);
        let report =
            crate::receiver_protocol::query_report(cap, cap.transaction_prefix | 3).unwrap();
        assert_eq!(report[0], interface);
    }
    let cap = crate::receiver_capabilities::capability(756).unwrap();
    assert_eq!(cap.report_id, 0, "source ordinary keyboard sender");
    assert_eq!(cap.get_report_id(), 3);
    let cap = crate::receiver_capabilities::capability(164).unwrap();
    assert_eq!(cap.report_id, 0, "source Linker interface zero");
    assert_eq!(cap.get_report_id(), 0);
}

#[test]
fn linker_query_firmware_and_serial_share_the_selected_primary_report_id() {
    for (product, report_id) in [(164, 0), (241, 3)] {
        let transport = FramingTransport::default();
        let cap = crate::receiver_capabilities::capability(product).unwrap();
        let mut cap = cap.clone();
        cap.sleep_between_out_ms = 0;
        cap.sleep_between_out_in_ms = 0;
        cap.sleep_between_in_ms = 0;
        // The source serial reader accepts printable bytes within its returned
        // length; this fixture returns a complete four-byte firmware payload.
        let serial = crate::receiver_identity::read_serial(&transport, &cap, 224, || Ok(()));
        assert_eq!(serial.unwrap(), "");
        assert_eq!(transport.sent.borrow()[0], report_id);
        assert_eq!(transport.get_ids.borrow().as_slice(), [report_id]);
        transport.get_ids.borrow_mut().clear();
        let mut firmware = device_reads::capability(u32::from(product))
            .unwrap()
            .clone();
        firmware.sleep_between_out_ms = 0;
        firmware.sleep_between_out_in_ms = 0;
        firmware.sleep_between_in_ms = 0;
        let value = read_device(&transport, &firmware, &firmware.queries[0], 225, || Ok(()));
        assert!(matches!(value.unwrap(), DeviceReadValue::Firmware { .. }));
        assert_eq!(transport.sent.borrow()[0], report_id);
        assert_eq!(transport.get_ids.borrow().as_slice(), [report_id]);
    }
}

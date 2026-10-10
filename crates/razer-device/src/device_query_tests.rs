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
fn validation_failure_after_input_discards_value() {
    let (cap, command) = battery_cap();
    let transport = Transport::new([Input::Status(2)]);
    let validations = Cell::new(0);
    let result = read_device(&transport, &cap, &command, 7, || {
        validations.set(validations.get() + 1);
        anyhow::ensure!(validations.get() < 3, "route disappeared after IN");
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(validations.get(), 3);
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

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
    for pid in [241, 756] {
        let cap = crate::receiver_capabilities::capability(pid).unwrap();
        assert_eq!(cap.report_id, 0, "source ordinary/linker sender {pid}");
        assert_eq!(cap.get_report_id(), 3);
    }
}

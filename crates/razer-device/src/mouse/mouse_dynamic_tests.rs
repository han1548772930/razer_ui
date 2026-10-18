use super::*;
use std::{cell::RefCell, collections::VecDeque};

enum Step {
    Out(Vec<u8>),
    In(Vec<u8>),
    Error,
}
struct Device {
    steps: RefCell<VecDeque<Step>>,
    node: crate::backend::HidNode,
}
impl Device {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
            node: crate::backend::HidNode {
                path: b"simulated-dynamic-mouse190".to_vec(),
                vendor_id: 5426,
                product_id: 190,
                usage_page: 1,
                usage: 2,
                interface_number: 0,
                release_number: 0,
                serial_number: None,
                manufacturer: None,
                product: None,
            },
        }
    }
}
impl FeatureTransport for Device {
    fn hid_node(&self) -> Option<&crate::backend::HidNode> {
        Some(&self.node)
    }
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        match self
            .steps
            .borrow_mut()
            .pop_front()
            .expect("unexpected request")
        {
            Step::Out(expected) => {
                assert_eq!(report, expected);
                Ok(())
            }
            _ => panic!("outgoing source command differs"),
        }
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        match self
            .steps
            .borrow_mut()
            .pop_front()
            .expect("unexpected response")
        {
            Step::In(expected) => {
                report.copy_from_slice(&expected);
                Ok(expected.len())
            }
            Step::Error => anyhow::bail!("lost response"),
            _ => panic!("incoming source response differs"),
        }
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
fn packet(command: [u8; 3], payload: &[u8], tx: u8, incoming: bool) -> Vec<u8> {
    let mut bytes = vec![0; 91];
    bytes[2] = tx % 31;
    bytes[6..9].copy_from_slice(&command);
    bytes[9..9 + payload.len()].copy_from_slice(payload);
    bytes[89] = bytes[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    if incoming {
        bytes[1] = 2;
        bytes[6] = payload.len() as u8;
    }
    bytes
}
fn exchange(steps: &mut Vec<Step>, command: [u8; 3], payload: &[u8], reply: &[u8], tx: u8) {
    steps.push(Step::Out(packet(command, payload, tx, false)));
    steps.push(Step::In(packet(command, reply, tx, true)));
}
fn cap() -> DeviceReadCapability {
    let mut cap = capability(190)
        .unwrap()
        .transport()
        .for_physical_product(190)
        .unwrap();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap
}
fn run(
    device: &Device,
    setting: Option<&Setting>,
    wait: &mut dyn FnMut(Duration) -> anyhow::Result<()>,
) -> Report {
    let mut tx = 0;
    let result = diagnose(
        device,
        &cap(),
        setting,
        &mut || {
            tx += 1;
            Ok(tx % 31)
        },
        &|| Ok(()),
        wait,
    )
    .unwrap();
    assert!(
        device.steps.borrow().is_empty(),
        "not all independent source requests were sent: {result:?}"
    );
    result
}
#[test]
fn mouse_dynamic_getters_preserve_unknown_values_and_do_not_read_local_defaults() {
    let mut steps = vec![];
    exchange(&mut steps, [2, 11, 144], &[1], &[1, 7], 1);
    exchange(&mut steps, [2, 11, 145], &[1], &[1, 99], 2);
    let report = run(&Device::new(steps), None, &mut |_| {
        panic!("read must not delay upload")
    });
    let reading = report.reading.unwrap();
    assert_eq!((reading.raw_state, reading.raw_mode), (7, 99));
    assert_eq!(reading.enabled(), None);
    assert_eq!(reading.mode(), None);
    assert!(reading.custom_sensor_accelerations.is_none());
}
#[test]
fn mouse_dynamic_setter_ack_does_not_replace_independent_readback() {
    let setting = Setting::new(true, AccelerationMode::Jump, vec![]).unwrap();
    let mut steps = vec![];
    exchange(&mut steps, [2, 11, 16], &[1, 1], &[], 1);
    exchange(&mut steps, [2, 11, 17], &[1, 2], &[], 2);
    exchange(&mut steps, [2, 11, 144], &[1], &[1, 1], 3);
    exchange(&mut steps, [2, 11, 145], &[1], &[1, 1], 4);
    let mut report = run(&Device::new(steps), Some(&setting), &mut |_| Ok(()));
    assert!(!report.confirmed());
    assert_eq!(report.reading.as_ref().unwrap().raw_mode, 1);
    report.writes.as_mut().unwrap().acknowledgements[1].payload[1] = 0;
    assert!(report.validate_reply(Some(&setting)).is_err());
}
#[test]
fn mouse_dynamic_partial_setter_failure_keeps_real_delivery_and_observed_state() {
    let setting = Setting::new(true, AccelerationMode::Natural, vec![]).unwrap();
    let mut steps = vec![];
    exchange(&mut steps, [2, 11, 16], &[1, 1], &[], 1);
    steps.push(Step::Out(packet([2, 11, 17], &[1, 1], 2, false)));
    steps.push(Step::Error);
    exchange(&mut steps, [2, 11, 144], &[1], &[1, 1], 3);
    exchange(&mut steps, [2, 11, 145], &[1], &[1, 0], 4);
    let report = run(&Device::new(steps), Some(&setting), &mut |_| Ok(()));
    assert!(!report.confirmed());
    assert!(report.write_failure.is_some());
    assert_eq!(report.writes.as_ref().unwrap().attempted_writes, 2);
    assert_eq!(report.writes.as_ref().unwrap().acknowledgements.len(), 1);
    assert_eq!(report.reading.unwrap().raw_mode, 0);
}
#[test]
fn mouse_dynamic_custom_upload_and_get_all_250_little_endian_float32_points() {
    let points = (0..250)
        .map(|index| 1. + index as f32 / 256.)
        .collect::<Vec<_>>();
    let setting = Setting::new(true, AccelerationMode::Custom, points.clone()).unwrap();
    let mut steps = vec![];
    exchange(&mut steps, [2, 11, 16], &[1, 1], &[], 1);
    exchange(&mut steps, [2, 11, 17], &[1, 3], &[], 2);
    let mut tx = 2;
    for (chunk, values) in points.chunks(8).enumerate() {
        tx += 1;
        let mut payload = vec![1, 3, 250, (chunk * 8) as u8, values.len() as u8];
        for value in values {
            payload.extend(value.to_le_bytes());
        }
        exchange(&mut steps, [37, 11, 19], &payload, &[], tx);
    }
    tx += 1;
    exchange(&mut steps, [2, 11, 144], &[1], &[1, 1], tx);
    tx += 1;
    exchange(&mut steps, [2, 11, 145], &[1], &[1, 3], tx);
    for (chunk, values) in points.chunks(8).enumerate() {
        tx += 1;
        let payload = [1, 3, 250, (chunk * 8) as u8, values.len() as u8];
        let mut reply = payload.to_vec();
        for value in values {
            reply.extend(value.to_le_bytes());
        }
        exchange(&mut steps, [37, 11, 147], &payload, &reply, tx);
    }
    let mut waits = 0;
    let report = run(&Device::new(steps), Some(&setting), &mut |delay| {
        assert_eq!(delay, Duration::from_millis(15));
        waits += 1;
        Ok(())
    });
    assert_eq!(waits, 31);
    assert!(report.confirmed());
    assert_eq!(report.writes.unwrap().acknowledgements.len(), 34);
}
#[test]
fn mouse_dynamic_cancellation_prevents_any_transport_or_upload() {
    let device = Device::new([]);
    let mut tx = || panic!("canceled owner must not reserve a transaction");
    assert!(
        execute(
            &device,
            &cap(),
            None,
            &mut tx,
            &|| anyhow::bail!("canceled"),
            &mut |_| Ok(())
        )
        .is_err()
    );
}

#[test]
fn mouse_dynamic_source_setter_finishes_on_ack_without_inserting_getters_or_fabricating_observation()
 {
    let setting = Setting::new(true, AccelerationMode::Jump, vec![]).unwrap();
    let mut steps = vec![];
    exchange(&mut steps, [2, 11, 16], &[1, 1], &[], 1);
    exchange(&mut steps, [2, 11, 17], &[1, 2], &[], 2);
    let device = Device::new(steps);
    let mut tx = 0;
    let report = execute(
        &device,
        &cap(),
        Some(&setting),
        &mut || {
            tx += 1;
            Ok(tx)
        },
        &|| Ok(()),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert!(device.steps.borrow().is_empty());
    assert!(report.source_acknowledged());
    assert!(!report.confirmed());
    assert!(report.reading.is_none());
    assert!(report.read_failure.is_none());
}

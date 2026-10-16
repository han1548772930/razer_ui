//! Current 182 setter/getter source bytes; simulated retained Feature transport.
use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

struct Step {
    header: [u8; 3],
    payload: Vec<u8>,
    reply: Option<Vec<u8>>,
}

struct Transport {
    steps: RefCell<VecDeque<Step>>,
    current: RefCell<Option<(Vec<u8>, Option<Vec<u8>>)>>,
    sent: RefCell<Vec<Vec<u8>>>,
    reads: Cell<usize>,
}

impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
            current: RefCell::new(None),
            sent: RefCell::new(Vec::new()),
            reads: Cell::new(0),
        }
    }
}

impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        let step = self
            .steps
            .borrow_mut()
            .pop_front()
            .expect("unexpected report after terminal state");
        assert_eq!(report.len(), 91);
        assert_eq!(&report[6..9], &step.header);
        assert_eq!(&report[9..9 + step.payload.len()], &step.payload);
        assert_eq!(
            report[89],
            report[3..89].iter().fold(0, |sum, byte| sum ^ byte)
        );
        self.sent.borrow_mut().push(report.to_vec());
        *self.current.borrow_mut() = Some((report.to_vec(), step.reply));
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        let current = self.current.borrow();
        let (sent, reply) = current.as_ref().unwrap();
        let data = reply
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("simulated readback failure"))?;
        report.copy_from_slice(sent);
        report[1] = 2;
        report[6] = data.len() as u8;
        report[9..9 + data.len()].copy_from_slice(data);
        Ok(report.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn cap() -> DeviceReadCapability {
    let mut cap = crate::device_reads::capability(182).unwrap().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap
}

fn step(header: [u8; 3], payload: &[u8], reply: Option<&[u8]>) -> Step {
    Step {
        header,
        payload: payload.to_vec(),
        reply: reply.map(Vec::from),
    }
}

#[test]
fn source_dpi_write_requires_getter_setter_ack_and_separate_matching_readback() {
    let device = Transport::new([
        step([7, 4, 133], &[0], Some(&[0, 3, 32, 3, 32, 0, 0])),
        step(
            [7, 4, 5],
            &[0, 6, 64, 12, 128, 0, 0],
            Some(&[0, 6, 64, 12, 128, 0, 0]),
        ),
        step([7, 4, 133], &[0], Some(&[0, 6, 64, 12, 128, 0, 0])),
    ]);
    let mut txn = 0;
    let result = apply(
        &device,
        &cap(),
        &DeviceWriteSetting::Dpi { x: 1600, y: 3200 },
        || {
            txn += 1;
            Ok(txn)
        },
        || Ok(()),
    )
    .unwrap();
    assert!(result.changed && result.verified);
    assert!(matches!(
        result.previous,
        Some(DeviceReadValue::Dpi { x: 800, y: 800 })
    ));
    assert!(matches!(
        result.observed,
        Some(DeviceReadValue::Dpi { x: 1600, y: 3200 })
    ));
    assert_eq!(
        device
            .sent
            .borrow()
            .iter()
            .map(|r| r[2])
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(device.steps.borrow().is_empty());
}

#[test]
fn matching_previous_value_does_not_send_a_setter() {
    let device = Transport::new([step([2, 0, 142], &[1], Some(&[1, 1]))]);
    let result = apply(
        &device,
        &cap(),
        &DeviceWriteSetting::Polling { hz: 1000 },
        || Ok(1),
        || Ok(()),
    )
    .unwrap();
    assert!(!result.changed);
    assert!(result.verified);
    assert_eq!(device.sent.borrow().len(), 1);
}

#[test]
fn setter_ack_is_never_enough_when_readback_fails_or_differs() {
    for reply in [None, Some(&[1, 2][..])] {
        let device = Transport::new([
            step([2, 0, 142], &[1], Some(&[1, 8])),
            step([2, 0, 14], &[1, 1], Some(&[1, 1])),
            step([2, 0, 142], &[1], reply),
        ]);
        assert!(
            apply(
                &device,
                &cap(),
                &DeviceWriteSetting::Polling { hz: 1000 },
                || Ok(3),
                || Ok(())
            )
            .is_err()
        );
        assert_eq!(device.sent.borrow().len(), 3);
        assert!(device.steps.borrow().is_empty());
    }
}

#[test]
fn idle_has_big_endian_units_and_no_profile_selector() {
    let device = Transport::new([
        step([2, 7, 3], &[0x12, 0x34], Some(&[0x12, 0x34])),
        step([2, 7, 131], &[], Some(&[0x12, 0x34])),
    ]);
    assert!(
        apply(
            &device,
            &cap(),
            &DeviceWriteSetting::Idle { raw_time: 0x1234 },
            || Ok(1),
            || Ok(())
        )
        .unwrap()
        .verified
    );
    assert_eq!(device.sent.borrow().len(), 2);
    assert!(device.steps.borrow().is_empty());
}

#[test]
fn current182_idle_sends_before_reading_and_only_compares_the_getter() {
    // 85191.Rt retains setter jsonData as fromDevice without comparing it.
    // A successful ACK can contain a different/empty payload; the subsequent
    // getter remains the sole timeToSleep comparison and success authority.
    for ack in [&[][..], &[0, 0][..], &[0xab, 0xcd][..]] {
        let device = Transport::new([
            step([2, 7, 3], &[0x01, 0x2c], Some(ack)),
            step([2, 7, 131], &[], Some(&[0x01, 0x2c])),
        ]);
        let mut transaction = 0;
        let result = apply(
            &device,
            &cap(),
            &DeviceWriteSetting::Idle { raw_time: 300 },
            || {
                transaction += 1;
                Ok(transaction)
            },
            || Ok(()),
        )
        .unwrap();
        assert!(result.previous.is_none());
        assert!(result.changed && result.verified);
        assert!(matches!(
            result.observed,
            Some(DeviceReadValue::Idle { raw_time: 300 })
        ));
        assert_eq!(result.acknowledged.unwrap().data, ack);
        assert_eq!(
            device
                .sent
                .borrow()
                .iter()
                .map(|report| report[2])
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert!(device.steps.borrow().is_empty());
    }
}

#[test]
fn current182_idle_never_reports_success_after_failed_ack_or_readback() {
    let device = Transport::new([step([2, 7, 3], &[0x01, 0x2c], None)]);
    assert!(
        apply(
            &device,
            &cap(),
            &DeviceWriteSetting::Idle { raw_time: 300 },
            || Ok(1),
            || Ok(())
        )
        .is_err()
    );
    assert_eq!(device.sent.borrow().len(), 1);
    for observed in [None, Some(&[0x01, 0x2d][..])] {
        let device = Transport::new([
            step([2, 7, 3], &[0x01, 0x2c], Some(&[])),
            step([2, 7, 131], &[], observed),
        ]);
        assert!(
            apply(
                &device,
                &cap(),
                &DeviceWriteSetting::Idle { raw_time: 300 },
                || Ok(1),
                || Ok(())
            )
            .is_err()
        );
        assert_eq!(device.sent.borrow().len(), 2);
        assert!(device.steps.borrow().is_empty());
    }
}

#[test]
fn current182_idle_cancellation_before_or_after_setter_stops_later_io() {
    let device = Transport::new([]);
    assert!(
        apply(
            &device,
            &cap(),
            &DeviceWriteSetting::Idle { raw_time: 300 },
            || Ok(1),
            || anyhow::bail!("canceled before submission")
        )
        .is_err()
    );
    assert!(device.sent.borrow().is_empty());

    let device = Transport::new([step([2, 7, 3], &[0x01, 0x2c], Some(&[]))]);
    assert!(
        apply(
            &device,
            &cap(),
            &DeviceWriteSetting::Idle { raw_time: 300 },
            || Ok(1),
            || {
                anyhow::ensure!(device.reads.get() == 0, "canceled after setter ACK");
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(device.sent.borrow().len(), 1);
    assert_eq!(device.reads.get(), 1);
    assert!(device.steps.borrow().is_empty());
}

#[test]
fn unsupported_dpi_polling_and_cancelled_routes_never_send_a_setter() {
    for setting in [
        DeviceWriteSetting::Dpi { x: 99, y: 800 },
        DeviceWriteSetting::Polling { hz: 999 },
    ] {
        let device = Transport::new([]);
        assert!(apply(&device, &cap(), &setting, || Ok(1), || Ok(())).is_err());
        assert!(device.sent.borrow().is_empty());
    }
    let device = Transport::new([step([2, 0, 142], &[1], Some(&[1, 8]))]);
    let validation = Cell::new(0);
    let result = apply(
        &device,
        &cap(),
        &DeviceWriteSetting::Polling { hz: 1000 },
        || Ok(1),
        || {
            validation.set(validation.get() + 1);
            anyhow::ensure!(
                validation.get() < 5,
                "scope cancelled after previous observation"
            );
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(device.sent.borrow().len(), 1);
}

#[test]
fn typed_catalog_preserves_ordinary_profile_and_high_speed_rate_bytes() {
    // Current source selects different codes for the same 1000 Hz setting:
    // ordinary/profile => 1, USB high speed => 8. Numeric Hz must not erase it.
    for (product, hz, header, payload, method) in [
        (148, 333, [1, 0, 5], vec![3], "setPollingRate"),
        (124, 1000, [2, 0, 14], vec![1, 1], "setProfilePollingRate"),
        (
            226,
            1000,
            [2, 0, 64],
            vec![1, 8],
            "setUSBHighSpeedPollingRate",
        ),
        (
            235,
            8000,
            [2, 0, 64],
            vec![1, 1],
            "setUSBHighSpeedPollingRate",
        ),
    ] {
        let source = capability(product).unwrap();
        let command = prepare(source, &DeviceWriteSetting::Polling { hz }).unwrap();
        assert_eq!(command.command, header);
        assert_eq!(command.payload, payload);
        assert_eq!(command.method, method);
    }
    // High-speed enums have no ordinary 333 Hz code; ordinary sources have no 8 kHz.
    for (product, hz) in [(226, 333), (124, 8000)] {
        assert!(
            prepare(
                capability(product).unwrap(),
                &DeviceWriteSetting::Polling { hz }
            )
            .is_err()
        );
    }
}

#[test]
fn typed_catalog_keeps_source_dpi_limits_and_legacy_selector_separate() {
    let high_speed = capability(226).unwrap();
    let command = prepare(high_speed, &DeviceWriteSetting::Dpi { x: 50000, y: 100 }).unwrap();
    assert_eq!(command.payload, [0, 0xc3, 0x50, 0, 100, 0, 0]);
    assert!(prepare(high_speed, &DeviceWriteSetting::Dpi { x: 50001, y: 100 }).is_err());

    let legacy = capability(70).unwrap();
    assert_eq!(legacy.source_class, WriteSourceClass::Legacy);
    let command = prepare(legacy, &DeviceWriteSetting::Dpi { x: 16000, y: 16000 }).unwrap();
    assert_eq!(command.payload, [1, 0x3e, 0x80, 0x3e, 0x80, 0, 0]);
    assert!(prepare(legacy, &DeviceWriteSetting::Idle { raw_time: 1 }).is_err());
    assert!(capability(0).is_none());
}

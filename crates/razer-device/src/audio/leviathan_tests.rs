use super::*;
use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

struct Simulated {
    sent: Mutex<Vec<Vec<u8>>>,
    replies: Mutex<VecDeque<(u8, Vec<u8>)>>,
    canceled: AtomicBool,
    cancel_after_get: bool,
}
impl Simulated {
    fn new(replies: impl IntoIterator<Item = (u8, Vec<u8>)>) -> Self {
        Self {
            sent: Mutex::new(vec![]),
            replies: Mutex::new(replies.into_iter().collect()),
            canceled: AtomicBool::new(false),
            cancel_after_get: false,
        }
    }
    fn execute(&self, op: Operation) -> anyhow::Result<Report> {
        let mut cap = transport().clone();
        cap.sleep_between_out_in_ms = 0;
        cap.sleep_between_in_ms = 0;
        cap.max_retry_in = 3;
        cap.max_retry_out = 2;
        let mut tx = 0;
        execute(
            self,
            &cap,
            op,
            || {
                let value = tx;
                tx += 1;
                Ok(value)
            },
            || {
                ensure!(!self.canceled.load(Ordering::Acquire), "canceled");
                Ok(())
            },
        )
    }
}
impl FeatureTransport for Simulated {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("source uses collection-sized transfer")
    }
    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("source uses collection-sized transfer")
    }
    fn send_feature_at_collection_max(&self, bytes: &[u8]) -> anyhow::Result<()> {
        assert_eq!(bytes.len(), 65);
        assert_eq!(bytes[0], 8);
        assert_eq!(bytes[63], bytes[3..63].iter().fold(0, |sum, b| sum ^ b));
        self.sent.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }
    fn get_feature_at_collection_max(&self, bytes: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(bytes.len(), 65);
        assert_eq!(bytes[0], 8);
        let (status, data) = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .context("missing simulated response")?;
        let sent = self.sent.lock().unwrap();
        let sent = sent.last().unwrap();
        bytes.fill(0);
        bytes[1] = status;
        bytes[2] = sent[2];
        bytes[6] = data.len() as u8;
        bytes[7..9].copy_from_slice(&sent[7..9]);
        bytes[9..9 + data.len()].copy_from_slice(&data);
        if self.cancel_after_get {
            self.canceled.store(true, Ordering::Release);
        }
        Ok(65)
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}
#[test]
fn interrupted_noa_transfers_do_not_retry_and_keep_the_short_frame_ack() {
    use crate::backend::{NativeIoOutcome, native_io_outcome};
    use crate::keyboard_control_test_transport::{Mock, reply};
    let mut cap = transport().clone();
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    for outcome in [
        NativeIoOutcome::NotCalled,
        NativeIoOutcome::Returned { byte_count: 64 },
        NativeIoOutcome::Returned { byte_count: 65 },
    ] {
        let device = Mock::new([]).fail_next_send(outcome);
        let error = execute(&device, &cap, Operation::Bass(7), || Ok(0), || Ok(())).unwrap_err();
        assert_eq!(native_io_outcome(&error), Some(outcome));
        assert_eq!(device.sent().len(), 1);
        let progress = &error.downcast_ref::<ExecutionFailure>().unwrap().progress;
        assert_eq!(
            progress.setter_sent,
            outcome == NativeIoOutcome::Returned { byte_count: 65 }
        );
        assert!(progress.acknowledgements.is_empty());
    }
    let mut response = reply(&[7, 8]);
    response.count = 65;
    let device = Mock::new([response]).interrupt_next_read();
    let error = execute(&device, &cap, Operation::Bass(7), || Ok(0), || Ok(())).unwrap_err();
    let progress = &error.downcast_ref::<ExecutionFailure>().unwrap().progress;
    assert!(progress.setter_sent);
    assert_eq!(progress.acknowledgements[0].data, [7, 8]);
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn leviathan_source_setters_use_distinct_profiles_without_invented_getters() {
    for (operation, bytes) in [
        (Operation::Bass(7), [2, 8, 7, 1, 7]),
        (Operation::Output { headset: true }, [2, 8, 17, 1, 1]),
        (Operation::Mode(3), [2, 8, 16, 0, 3]),
    ] {
        // Setter ACK data is not an invented echo/readback requirement.
        let device = Simulated::new([(2, vec![])]);
        let report = device.execute(operation).unwrap();
        assert!(report.setter_sent);
        assert_eq!(report.acknowledgements.len(), 1);
        assert_eq!(&device.sent.lock().unwrap()[0][6..11], &bytes);
    }
}
#[test]
fn leviathan_timeout_and_busy_continue_get_before_resending() {
    let device = Simulated::new([(4, vec![]), (1, vec![]), (2, vec![])]);
    device.execute(Operation::Bass(4)).unwrap();
    assert_eq!(device.sent.lock().unwrap().len(), 1);
}
#[test]
fn leviathan_failed_status_resends_same_transaction_but_unsupported_stops() {
    let device = Simulated::new([(3, vec![]), (2, vec![])]);
    device.execute(Operation::Mode(1)).unwrap();
    let sent = device.sent.lock().unwrap();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0], sent[1]);
    let device = Simulated::new([(5, vec![])]);
    assert!(device.execute(Operation::Mode(1)).is_err());
    assert_eq!(device.sent.lock().unwrap().len(), 1);
}
#[test]
fn leviathan_bluetooth_reads_source_and_only_writes_on_change() {
    let device = Simulated::new([(2, vec![0, 3])]);
    let report = device.execute(Operation::Bluetooth).unwrap();
    assert!(!report.setter_sent);
    assert_eq!(report.previous_input, Some(3));
    let device = Simulated::new([(2, vec![0, 4]), (2, vec![])]);
    let report = device.execute(Operation::Bluetooth).unwrap();
    assert!(report.setter_sent);
    let sent = device.sent.lock().unwrap();
    assert_eq!(&sent[0][6..11], &[2, 8, 138, 0, 0]);
    assert_eq!(&sent[1][6..11], &[2, 8, 10, 0, 3]);
    assert_eq!(sent[1][2], 1);
}
#[test]
fn leviathan_cancellation_after_input_read_prevents_mutation() {
    use crate::backend::{
        ExchangeInterruption, NativeIoOutcome, exchange_interrupted, native_io_outcome,
        native_read_bytes,
    };

    let mut device = Simulated::new([(2, vec![0, 4]), (2, vec![])]);
    device.cancel_after_get = true;
    let error = device.execute(Operation::Bluetooth).unwrap_err();
    assert!(exchange_interrupted(&error));
    assert_eq!(
        error.downcast_ref::<ExchangeInterruption>(),
        Some(&ExchangeInterruption::Validation)
    );
    assert_eq!(error.root_cause().to_string(), "canceled");
    assert_eq!(
        native_io_outcome(&error),
        Some(NativeIoOutcome::Returned { byte_count: 65 })
    );
    let mut received = [0; 65];
    received[1] = 2;
    received[6..11].copy_from_slice(&[2, 8, 138, 0, 4]);
    assert_eq!(native_read_bytes(&error), Some(received.as_slice()));

    let progress = &error.downcast_ref::<ExecutionFailure>().unwrap().progress;
    assert_eq!(
        progress.acknowledgements,
        [Acknowledgement {
            command: [2, 8, 138],
            transaction: 0,
            data: vec![0, 4],
        }]
    );
    assert_eq!(progress.requested, Operation::Bluetooth);
    assert!(!progress.setter_sent);
    assert!(progress.previous_input.is_none());
    assert!(progress.headphone_connected.is_none());
    let sent = device.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0][2], 0);
    assert_eq!(&sent[0][6..11], &[2, 8, 138, 0, 0]);
    assert_eq!(device.replies.lock().unwrap().len(), 1);
}

#[test]
fn leviathan_interrupted_invalid_input_replies_never_become_acknowledgements() {
    use crate::backend::{
        NativeIoOutcome, exchange_interrupted, native_io_outcome, native_read_bytes,
    };
    use crate::keyboard_control_test_transport::{Mock, Reply, reply};

    let mut cap = transport().clone();
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    let valid = Reply {
        count: 65,
        ..reply(&[0, 4])
    };
    for response in [
        Reply {
            count: 64,
            ..valid.clone()
        },
        Reply {
            count: 66,
            ..valid.clone()
        },
        Reply {
            status: 3,
            ..valid.clone()
        },
        Reply {
            wrong_transaction: true,
            ..valid.clone()
        },
        Reply {
            wrong_command: true,
            ..valid.clone()
        },
        Reply {
            declared_length: Some(55),
            ..valid.clone()
        },
    ] {
        let count = response.count;
        let device = Mock::new([response, valid.clone()]).interrupt_next_read();
        let error = execute(&device, &cap, Operation::Bluetooth, || Ok(0), || Ok(())).unwrap_err();
        assert!(exchange_interrupted(&error));
        assert_eq!(
            native_io_outcome(&error),
            Some(NativeIoOutcome::Returned { byte_count: count })
        );
        assert_eq!(
            native_read_bytes(&error).map(<[u8]>::len),
            (count <= 65).then_some(count)
        );
        let progress = &error.downcast_ref::<ExecutionFailure>().unwrap().progress;
        assert!(progress.acknowledgements.is_empty());
        assert!(!progress.setter_sent);
        assert!(progress.previous_input.is_none());
        assert!(progress.headphone_connected.is_none());
        let sent = device.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(&sent[0][6..11], &[2, 8, 138, 0, 0]);
    }
}

#[test]
fn leviathan_valid_ack_survives_finish_failure_and_stops_bluetooth_followup() {
    for operation in [Operation::Bass(4), Operation::Bluetooth] {
        let acknowledged = vec![0x51, 4];
        let device = Simulated::new([(2, acknowledged.clone()), (2, vec![])]);
        let probe = crate::feature_exchange::tests::ExchangeProbe::failing_finish(&device);
        let mut cap = transport().clone();
        cap.sleep_between_out_in_ms = 0;
        cap.sleep_between_in_ms = 0;
        let error = execute(&probe, &cap, operation, || Ok(0), || Ok(())).unwrap_err();
        assert_eq!(
            error.root_cause().to_string(),
            "simulated explicit release failed"
        );
        let progress = &error.downcast_ref::<ExecutionFailure>().unwrap().progress;
        assert_eq!(progress.acknowledgements.len(), 1);
        assert_eq!(progress.acknowledgements[0].data, acknowledged);
        assert_eq!(
            progress.setter_sent,
            matches!(operation, Operation::Bass(_))
        );
        assert!(progress.previous_input.is_none());
        assert_eq!(device.sent.lock().unwrap().len(), 1);
        assert_eq!(device.replies.lock().unwrap().len(), 1);
    }
}
#[test]
fn leviathan_bad_ranges_and_truncated_reads_never_fabricate_observations() {
    for operation in [Operation::Bass(0), Operation::Bass(8), Operation::Mode(4)] {
        let device = Simulated::new([]);
        assert!(device.execute(operation).is_err());
        assert!(device.sent.lock().unwrap().is_empty());
    }
    let device = Simulated::new([(2, vec![])]);
    assert!(device.execute(Operation::ReadIo).is_err());
    let device = Simulated::new([(2, vec![1, 1, 0, 0, 1])]);
    assert_eq!(
        device
            .execute(Operation::ReadIo)
            .unwrap()
            .headphone_connected,
        Some(true)
    );
}

#[test]
fn leviathan_interrupt_parser_preserves_event_fields_and_little_endian_camera_values() {
    use super::{Notification, notification};
    assert_eq!(
        notification(&[5, 19, 1, 2, 3, 4, 5]).unwrap(),
        Some(Notification::Io {
            headphone: 1,
            usb: 2,
            aux: 3,
            optical: 4,
            bluetooth: 5
        })
    );
    assert_eq!(
        notification(&[5, 28, 1]).unwrap(),
        Some(Notification::Output { state: 1 })
    );
    assert_eq!(
        notification(&[5, 26, 3]).unwrap(),
        Some(Notification::Mode { mode: 3 })
    );
    assert_eq!(
        notification(&[5, 18, 3]).unwrap(),
        Some(Notification::Input { source: 3 })
    );
    let mut camera = vec![5, 29, 1];
    camera.extend_from_slice(&(-12.25f32).to_le_bytes());
    camera.extend_from_slice(&1.75f32.to_le_bytes());
    assert_eq!(
        notification(&camera).unwrap(),
        Some(Notification::Camera {
            locked: 1,
            direction: -12.25,
            distance: 1.75
        })
    );
    for bytes in [&[5][..], &[5, 19, 1, 2, 3, 4], &camera[..10]] {
        assert!(notification(bytes).is_err());
    }
    assert_eq!(notification(&[8, 19]).unwrap(), None);
    assert_eq!(notification(&[5, 255]).unwrap(), None);
}

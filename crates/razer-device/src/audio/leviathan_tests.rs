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
    let mut device = Simulated::new([(2, vec![0, 4])]);
    device.cancel_after_get = true;
    assert!(device.execute(Operation::Bluetooth).is_err());
    assert_eq!(device.sent.lock().unwrap().len(), 1);
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

use super::*;
use anyhow::ensure;
use razer_device::{
    backend::FeatureTransport, device_reads::DeviceReadCapability,
    keyboard_onboard_profiles::ReadOptions,
};
use std::cell::{Cell, RefCell};

struct Device {
    sent: RefCell<Vec<Vec<u8>>>,
    peer_reads: Cell<usize>,
    before: u8,
    after: u8,
    read_fails: bool,
    cancel_on_last_read: bool,
    canceled: Cell<bool>,
}

impl Device {
    fn new(before: u8, after: u8) -> Self {
        Self {
            sent: RefCell::new(Vec::new()),
            peer_reads: Cell::new(0),
            before,
            after,
            read_fails: false,
            cancel_on_last_read: false,
            canceled: Cell::new(false),
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(!self.canceled.get(), "connection replaced");
        Ok(())
    }

    fn commands(&self) -> Vec<[u8; 2]> {
        self.sent
            .borrow()
            .iter()
            .map(|packet| [packet[7], packet[8]])
            .collect()
    }
}

impl FeatureTransport for Device {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        self.sent.borrow_mut().push(report.to_vec());
        Ok(())
    }

    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        let sent = self.sent.borrow();
        let last = sent.last().unwrap();
        report.copy_from_slice(last);
        report[1] = 2;
        let data: &[u8] = match last[7..9] {
            [0, 191] => {
                let count = self.peer_reads.get();
                self.peer_reads.set(count + 1);
                let status = if count == 0 { self.before } else { self.after };
                report[6] = 4;
                report[9..13].copy_from_slice(&[1, status, 2, 88]);
                return Ok(report.len());
            }
            [6, 142] => {
                if self.read_fails {
                    report[1] = 5;
                }
                &[0x7f, 0xff, 0, 1, 0, 0, 0, 0, 0xf0, 0, 0, 0, 0, 7]
            }
            [6, 128] => &[0, 0],
            [5, 138] => &[6],
            [5, 128] => &[0],
            [5, 129] => {
                if self.cancel_on_last_read {
                    self.canceled.set(true);
                }
                &[0]
            }
            _ => panic!("Unexpected OBM request: {:?}", &last[6..9]),
        };
        report[6] = data.len() as u8;
        report[9..9 + data.len()].copy_from_slice(data);
        Ok(report.len())
    }

    fn send_feature_at_collection_max(&self, report: &[u8]) -> Result<()> {
        self.send_feature(report)
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> Result<usize> {
        self.get_feature(report)
    }

    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"transport":"scripted"})
    }
}

struct TestSession<'a> {
    device: &'a Device,
    cap: DeviceReadCapability,
    wireless: bool,
    transaction: u8,
}

impl Session for TestSession<'_> {
    fn confirm_peer(&mut self) -> Result<()> {
        if self.wireless {
            let mut receiver = razer_device::receiver_capabilities::capability(625)
                .unwrap()
                .clone();
            receiver.sleep_between_out_ms = 0;
            receiver.sleep_between_in_ms = 0;
            receiver.sleep_between_out_in_ms = 0;
            receiver.max_retry_in = 1;
            receiver.max_retry_out = 1;
            let peers = razer_device::device_query::read_receiver(
                self.device,
                &receiver,
                receiver.transaction_prefix | self.transaction,
                || self.device.validate(),
            )?;
            self.transaction = (self.transaction + 1) % 31;
            razer_device::keyboard_metadata::validate_peer(600, 600, &peers)?;
        }
        Ok(())
    }

    fn read_profiles(&mut self) -> Result<CompleteReading> {
        razer_device::keyboard_onboard_profiles::read_complete(
            self.device,
            &self.cap,
            &ReadOptions {
                layout_id: 1,
                locally_known_macro_guids: Default::default(),
            },
            || {
                let next = self.transaction;
                self.transaction = (next + 1) % 31;
                Ok(self.cap.transaction_prefix | next)
            },
            &|| self.device.validate(),
        )
    }
}

fn execute(device: &Device, wireless: bool) -> Result<CompleteReading> {
    let mut cap =
        razer_device::keyboard_metadata::hid_capability(600, if wireless { 625 } else { 600 })
            .unwrap()
            .clone();
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap.sleep_between_in_ms = 0;
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    read(&mut TestSession {
        device,
        cap,
        wireless,
        transaction: 0,
    })
}

#[test]
fn keyboard_onboard_session_rechecks_wireless_peer_before_publishing_complete_snapshot() {
    let direct = Device::new(0, 0);
    assert_eq!(
        execute(&direct, false)
            .unwrap()
            .snapshot
            .inventory
            .max_profiles_supported,
        6
    );
    let expected = [[6, 142], [6, 128], [5, 138], [5, 128], [5, 129]];
    assert_eq!(direct.commands(), expected);
    assert_eq!(direct.peer_reads.get(), 0);

    for status in [0, 1, 255] {
        let device = Device::new(1, status);
        let result = execute(&device, true);
        assert_eq!(result.is_ok(), status == 1);
        let mut commands = vec![[0, 191]];
        commands.extend(expected);
        commands.push([0, 191]);
        assert_eq!(device.commands(), commands);
        assert_eq!(device.peer_reads.get(), 2);
    }
}

#[test]
fn keyboard_onboard_session_offline_failure_and_replaced_connection_never_publish() {
    let offline = Device::new(0, 1);
    assert!(execute(&offline, true).is_err());
    assert_eq!(offline.commands(), [[0, 191]]);

    let mut failed = Device::new(1, 1);
    failed.read_fails = true;
    assert!(execute(&failed, true).is_err());
    assert_eq!(failed.commands(), [[0, 191], [6, 142]]);

    let mut canceled = Device::new(1, 1);
    canceled.cancel_on_last_read = true;
    let error = execute(&canceled, true).unwrap_err();
    assert!(format!("{error:#}").contains("connection replaced"));
    assert_eq!(canceled.peer_reads.get(), 1);
    assert_eq!(canceled.commands().last(), Some(&[5, 129]));
}

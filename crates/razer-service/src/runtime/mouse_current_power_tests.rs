use crate::runtime::receiver_settings::Setting;
use anyhow::ensure;
use razer_device::{backend::FeatureTransport, mouse_current_power::Change};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};

struct Simulated {
    before: Vec<(u16, u8)>,
    after: Vec<(u16, u8)>,
    peer_reads: Cell<usize>,
    sent: RefCell<Vec<Vec<u8>>>,
    gets: RefCell<Vec<u8>>,
    stale_getter: Cell<bool>,
    cancel_after_setter: bool,
    canceled: Cell<bool>,
}

impl Simulated {
    fn new(before: Vec<(u16, u8)>, after: Vec<(u16, u8)>) -> Self {
        Self {
            before,
            after,
            peer_reads: Cell::new(0),
            sent: RefCell::new(vec![]),
            gets: RefCell::new(vec![]),
            stale_getter: Cell::new(false),
            cancel_after_setter: false,
            canceled: Cell::new(false),
        }
    }
}

impl FeatureTransport for Simulated {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("Current source uses a collection-sized Feature SET");
    }
    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("Current source uses a collection-sized Feature GET");
    }
    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        assert_eq!(report.len(), 91);
        self.sent.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.gets.borrow_mut().push(report[0]);
        let sent = self.sent.borrow();
        let sent = sent.last().unwrap();
        report.copy_from_slice(sent);
        report[0] = 93;
        report[1] = 2;
        let data = match sent[7..9] {
            [0, 191] => {
                let peers = if self.peer_reads.get() == 0 {
                    &self.before
                } else {
                    &self.after
                };
                self.peer_reads.set(self.peer_reads.get() + 1);
                let mut data = vec![peers.len() as u8];
                for (peer, status) in peers {
                    let [hi, lo] = peer.to_be_bytes();
                    data.extend([*status, hi, lo]);
                }
                data
            }
            [7, 3] => {
                if self.cancel_after_setter {
                    self.canceled.set(true);
                }
                vec![]
            }
            [7, 131] => {
                if self.stale_getter.replace(false) {
                    report[2] = sent[2].wrapping_sub(1);
                    vec![3, 231]
                } else {
                    vec![0, 120]
                }
            }
            _ => panic!("Unexpected current power command"),
        };
        report[6] = data.len() as u8;
        report[9..9 + data.len()].copy_from_slice(&data);
        Ok(report.len())
    }
    fn metadata(&self) -> Value {
        json!({"simulated":true})
    }
}

fn execute(
    product: u32,
    master: u32,
    peer: u32,
    change: &Change,
    device: &Simulated,
) -> anyhow::Result<Value> {
    let setting = Setting::CurrentPower(change);
    let session = setting.session(product, master, peer)?;
    let prefix = session.transport().transaction_prefix;
    let mut sequence = 0;
    setting.execute(
        &session,
        device,
        &mut || {
            let id = prefix | sequence;
            sequence = (sequence + 1) % 31;
            Ok(id)
        },
        &|| {
            ensure!(!device.canceled.get(), "Power fixture canceled");
            Ok(())
        },
    )
}

#[test]
fn receiver_power_keeps_both_peer_queries_setter_and_readback_in_one_source_session() {
    for (product, master, peer, interface) in [
        (124, 125, 125, 0),
        (226, 227, 227, 3),
        // Current PID241 DeviceInfo.claimInterface=3 also owns a borrowed
        // mouse session; its mouse transaction namespace remains zero.
        (226, 241, 227, 3),
        (235, 662, 234, 2),
    ] {
        let device = Simulated::new(vec![(peer as u16, 1)], vec![(peer as u16, 1)]);
        let response = execute(product, master, peer, &Change::PowerSaving(2), &device).unwrap();
        assert_eq!(response["report"]["time_to_sleep"], 120);
        assert_eq!(response["report"]["setter_sent"], true);
        assert_eq!(response["receiver_before"], json!([[peer, 1]]));
        assert_eq!(response["receiver_after"], json!([[peer, 1]]));
        assert_eq!(response["setting_scope"], "basic_device");
        assert_eq!(
            response["transport_source_class"],
            "rzDevice25DualLinkMouse"
        );
        let sent = device.sent.borrow();
        assert_eq!(sent.len(), 4);
        assert_eq!(
            sent.iter().map(|packet| packet[2]).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        assert_eq!(
            sent.iter()
                .map(|packet| packet[7..9].to_vec())
                .collect::<Vec<_>>(),
            [[0, 191], [7, 3], [7, 131], [0, 191]]
        );
        assert_eq!(&sent[1][6..11], &[2, 7, 3, 0, 120]);
        assert_eq!(&sent[2][6..11], &[2, 7, 131, 0, 0]);
        for packet in sent.iter() {
            assert_eq!(packet[0], interface);
            assert_eq!(
                packet[89],
                packet[3..89].iter().fold(0, |sum, byte| sum ^ byte)
            );
        }
        assert_eq!(*device.gets.borrow(), [interface; 4]);
    }
}

#[test]
fn receiver_power_rejects_wrong_source_peer_before_any_transport_work() {
    for (product, master, peer) in [
        (226, 241, 234),
        (226, 241, 226),
        (226, 9999, 227),
        (117, 2308, 117),
        (190, 191, 190),
    ] {
        let device = Simulated::new(vec![(peer as u16, 1)], vec![(peer as u16, 1)]);
        assert!(execute(product, master, peer, &Change::ReadPowerSaving, &device).is_err());
        assert!(device.sent.borrow().is_empty());
    }
}

#[test]
fn receiver_power_never_confirms_offline_absent_or_duplicate_peers_before_or_after_tasks() {
    for peers in [
        vec![(227, 0)],
        vec![(234, 1)],
        vec![(227, 1), (227, 1)],
        vec![],
    ] {
        for before in [true, false] {
            let device = if before {
                Simulated::new(peers.clone(), vec![(227, 1)])
            } else {
                Simulated::new(vec![(227, 1)], peers.clone())
            };
            assert!(execute(226, 241, 227, &Change::PowerSaving(2), &device).is_err());
            assert_eq!(device.sent.borrow().len(), if before { 1 } else { 4 });
        }
    }
}

#[test]
fn receiver_power_retries_stale_getter_without_accepting_its_value_or_advancing_transaction() {
    let device = Simulated::new(vec![(227, 1)], vec![(227, 1)]);
    device.stale_getter.set(true);
    let response = execute(226, 241, 227, &Change::PowerSaving(2), &device).unwrap();
    assert_eq!(response["report"]["time_to_sleep"], 120);
    let sent = device.sent.borrow();
    assert_eq!(
        sent.iter().map(|packet| packet[2]).collect::<Vec<_>>(),
        [0, 1, 2, 2, 3]
    );
    assert_eq!(sent[2], sent[3]);
    assert_eq!(device.peer_reads.get(), 2);
}

#[test]
fn canceled_receiver_power_stops_after_ack_without_claiming_confirmation() {
    let mut device = Simulated::new(vec![(227, 1)], vec![(227, 1)]);
    device.cancel_after_setter = true;
    assert!(execute(226, 241, 227, &Change::PowerSaving(2), &device).is_err());
    assert_eq!(device.sent.borrow().len(), 2);
    assert_eq!(device.peer_reads.get(), 1);
}

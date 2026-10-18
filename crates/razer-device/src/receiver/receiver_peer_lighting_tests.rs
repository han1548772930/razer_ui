use super::*;
use std::collections::VecDeque;
use std::sync::Mutex;
struct Fake {
    sent: Mutex<Vec<Vec<u8>>>,
    payload: Vec<u8>,
    statuses: Mutex<VecDeque<u8>>,
}
impl FeatureTransport for Fake {
    fn send_feature(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }
    fn get_feature(&self, bytes: &mut [u8]) -> anyhow::Result<usize> {
        let sent = self.sent.lock().unwrap().last().unwrap().clone();
        bytes.fill(0);
        bytes[1] = self.statuses.lock().unwrap().pop_front().unwrap_or(2);
        bytes[2] = sent[2];
        bytes[6] = self.payload.len() as u8;
        bytes[7..9].copy_from_slice(&sent[7..9]);
        bytes[9..9 + self.payload.len()].copy_from_slice(&self.payload);
        Ok(91)
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated":true})
    }
}
fn fake(payload: &[u8]) -> Fake {
    Fake {
        sent: Mutex::new(Vec::new()),
        payload: payload.to_vec(),
        statuses: Mutex::new(VecDeque::new()),
    }
}

fn immediate_support(pid: u16, edition: u8) -> PeerSupport {
    let mut support = super::super::peer_support(pid, edition).unwrap();
    support.max_retry_in = 1;
    support.max_retry_out = 1;
    support.sleep_time_between_in_ms = 0;
    support.sleep_time_between_out_ms = 0;
    support.sleep_time_between_out_in_ms = 0;
    support
}

#[test]
fn failed_custom_selection_retains_partial_mode_restoration_until_real_acknowledgements() {
    let mut peer = PeerHid::new(immediate_support(214, 0)).unwrap();
    let io = fake(&[]);
    io.statuses.lock().unwrap().extend([2, 3, 3]);
    assert!(!peer.restoration_required());
    assert!(peer.select_custom(&io, &|| Ok(())).is_err());
    assert!(peer.restoration_required());
    assert!(!peer.custom_selected());
    assert!(peer.restore(&io, &|| Ok(())).is_err());
    assert!(peer.restoration_required());
    assert_eq!(peer.restore(&io, &|| Ok(())).unwrap(), 2);
    assert!(!peer.restoration_required());
    assert!(!peer.custom_selected());
    let sent = io.sent.lock().unwrap();
    assert_eq!(&sent[0][6..11], &[2, 0, 4, 3, 0]);
    assert_eq!(&sent[1][6..15], &[6, 15, 2, 0, 0, 8, 0, 0, 0]);
    assert_eq!(&sent[3][6..11], &[2, 0, 4, 0, 0]);
    assert_eq!(&sent[4][6..15], &[6, 15, 2, 1, 0, 3, 0, 0, 0]);
}

#[test]
fn source_mode_timeout_exception_completes_without_fabricating_feature_acknowledgements() {
    let support = immediate_support(227, 0);
    assert!(support.ignore_set_device_mode_timeout_error);
    let mut peer = PeerHid::new(support).unwrap();
    let io = fake(&[]);
    io.statuses.lock().unwrap().extend([4, 2]);
    assert_eq!(peer.select_custom(&io, &|| Ok(())).unwrap(), 1);
    assert!(peer.restoration_required());
    assert!(peer.custom_selected());
    assert_eq!(io.sent.lock().unwrap().len(), 2);
    let mut strict = PeerHid::new(immediate_support(214, 0)).unwrap();
    let io = fake(&[]);
    io.statuses.lock().unwrap().push_back(4);
    assert!(strict.select_custom(&io, &|| Ok(())).is_err());
    assert_eq!(strict.acknowledged, 0);
    assert!(strict.restoration_required());
}

#[test]
fn krakoff_mode_and_effect_use_both_namespaces_but_frame_only_dock() {
    let mut peer = PeerHid::new(super::super::peer_support(221, 0).unwrap()).unwrap();
    let io = fake(&[]);
    assert_eq!(peer.select_custom(&io, &|| Ok(())).unwrap(), 4);
    let rows = io.sent.lock().unwrap().clone();
    assert_eq!(rows.iter().map(|r| r[0]).collect::<Vec<_>>(), [3, 3, 3, 3]);
    assert_eq!(
        rows.iter().map(|r| r[2]).collect::<Vec<_>>(),
        [224, 0, 225, 1]
    );
    assert_eq!(&rows[2][9..15], &[0, 0, 8, 0, 0, 0]);
    let frame = EngineFrame {
        device_handle: 7,
        region_id: 0,
        num_row: 1,
        num_col: 17,
        frame: vec![1; 51],
    };
    assert_eq!(
        peer.send_frame(&io, &frame, 7, 1, 17, &|| Ok(())).unwrap(),
        1
    );
    let sent = io.sent.lock().unwrap();
    let last = sent.last().unwrap();
    assert_eq!(last[2], 226);
    assert_eq!(&last[6..9], &[56, 15, 3]);
    assert_eq!(&last[9..14], &[0, 0, 0, 0, 16]);
}
#[test]
fn edition_battery_and_dock_getters_require_real_payloads_and_source_prefixes() {
    let support = super::super::peer_support(227, 0).unwrap();
    let mut peer = PeerHid::new(support).unwrap();
    let edition = fake(&[0, 128, 1]);
    assert_eq!(
        peer.edition(&edition, &|| Ok(())).unwrap(),
        Edition {
            layout: 0,
            edition: 128,
            firmware: 1
        }
    );
    assert_eq!(edition.sent.lock().unwrap()[0][0], 3);
    let battery = fake(&[0, 128]);
    assert_eq!(peer.battery(&battery, &|| Ok(())).unwrap(), 50);
    assert_eq!(&battery.sent.lock().unwrap()[0][6..10], &[2, 7, 128, 0]);
    assert!(peer.battery(&fake(&[1, 255]), &|| Ok(())).is_err());
    assert!(peer.on_dock(&fake(&[1]), &|| Ok(())).unwrap());
    assert!(!peer.on_dock(&fake(&[2]), &|| Ok(())).unwrap());
}
#[test]
fn white_manifest_uses_ordinary_sender_and_cancel_rejects_before_device_write() {
    let mut peer = PeerHid::new(super::super::peer_support(227, 128).unwrap()).unwrap();
    let io = fake(&[0, 255]);
    assert_eq!(peer.battery(&io, &|| Ok(())).unwrap(), 100);
    assert_eq!(io.sent.lock().unwrap()[0][0], 0);
    let io = fake(&[]);
    assert!(
        peer.select_custom(&io, &|| anyhow::bail!("superseded"))
            .is_err()
    );
    assert!(io.sent.lock().unwrap().is_empty());
}

#[test]
fn static_exit_uses_the_hardware_effect_namespace_for_both_physical_routes() {
    // DeviceManifest_45066_0 / 45067_0 select static #a3e01f. The current
    // 1306 constant is Static=1; 6411.AL writes that ID without translation.
    for product in [45066, 45067] {
        let mut peer = PeerHid::new(immediate_support(product, 0)).unwrap();
        let io = fake(&[]);
        peer.select_custom(&io, &|| Ok(())).unwrap();
        assert_eq!(peer.restore(&io, &|| Ok(())).unwrap(), 2);
        let sent = io.sent.lock().unwrap();
        assert_eq!(sent.len(), 4);
        assert_eq!(&sent[2][6..11], &[2, 0, 4, 0, 0]);
        assert_eq!(&sent[3][6..18], &[9, 15, 2, 1, 0, 1, 0, 0, 1, 163, 224, 31]);
        assert_eq!(sent[3][2], 3);
        assert_eq!(
            sent[3][89],
            sent[3][3..89].iter().fold(0, |sum, byte| sum ^ byte)
        );
        assert!(!peer.restoration_required());
    }
}

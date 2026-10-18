//! In-memory firmware simulation only. No DLL, vendor script or device execution.
use super::*;
use crate::{
    mouse_button_inventory, mouse_onboard::OnboardIdentityRequest, onboard_macro::MacroWriteRequest,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

#[derive(Clone)]
struct Macro {
    metadata: Vec<u8>,
    data: Vec<u8>,
}
#[derive(Clone)]
struct Profile {
    metadata: Vec<u8>,
    dpi: Vec<u8>,
    stages: Vec<u8>,
    brightness: BTreeMap<u8, u8>,
    scrolling: BTreeMap<(u8, u8), u8>,
    buttons: BTreeMap<(u8, u8), (u8, u8, [u8; 5])>,
    polling: u8,
}
#[derive(Clone)]
struct Firmware {
    profiles: BTreeMap<u8, Profile>,
    macros: BTreeMap<u16, Macro>,
    polls: usize,
}
struct Device {
    firmware: RefCell<Firmware>,
    pending: RefCell<Option<Vec<u8>>>,
    reports: RefCell<Vec<Vec<u8>>>,
    reads: Cell<usize>,
    fail_send: Cell<Option<(u8, u8)>>,
    fail_get: Cell<Option<(u8, u8)>>,
    mismatch: Cell<Option<(u8, u8)>>,
    format_busy: Cell<usize>,
    format_error_reads: Cell<usize>,
    format_clears: Cell<bool>,
    after_format: Cell<bool>,
    faults_after_format: Cell<bool>,
    faults_after_restore: Cell<bool>,
    fragmented_available: Cell<Option<u32>>,
    recyclable_memory: Cell<u32>,
    product: u32,
}

fn profile(product: u32, id: u8) -> Profile {
    let request = OnboardIdentityRequest {
        guid: "12345678-1234-5678-9012-123456789abc".into(),
        name: format!("Profile {id}"),
        owner: "Owner\u{100}Keep".into(),
        mapping_hash: matches!(product, 170 | 204).then_some([1, 2, 3, 4, 5, 6]),
    };
    let mut metadata = crate::mouse_onboard::encode_identity(product, &request).unwrap();
    metadata[210] = id + 10;
    let mut buttons = BTreeMap::new();
    for mode in [0, 1] {
        for &button in mouse_button_inventory::capability(product)
            .unwrap()
            .firmware_ids
        {
            buttons.insert((button, mode), (0xee, 5, [button, mode, id, 0x81, 0x23]));
        }
    }
    let mut scrolling = BTreeMap::from([
        ((2, 20), 1),
        ((2, 22), 1),
        ((2, 23), 0),
        ((2, 34), 3),
        ((2, 35), 4),
    ]);
    if product == 226 {
        scrolling.insert((21, 11), 7);
        scrolling.insert((2, 48), 2);
    }
    Profile {
        metadata,
        dpi: vec![id, 3, 32, 6, 64, 0, 0],
        stages: vec![id, 2, 2, 1, 3, 32, 6, 64, 0, 0, 2, 6, 64, 12, 128, 0, 0],
        brightness: BTreeMap::from([(1, 129), (4, 201)]),
        scrolling,
        buttons,
        polling: 1,
    }
}

impl Device {
    fn new(product: u32) -> Self {
        let mut macros = BTreeMap::new();
        for (id, len) in [(0, 73), (17, 145)] {
            let request = MacroWriteRequest {
                guid: "12345678-1234-5678-9012-123456789abc".into(),
                num_items: 2,
                name: format!("Macro {id}"),
                timestamp: "20261012".into(),
                hash: "0123456789abcdef".into(),
                data: (0..len).map(|n| n as u8).collect(),
            };
            let mut metadata = crate::onboard_macro::encode_metadata(&request).unwrap();
            metadata[120..136].fill(0xb5);
            metadata[230] = 0xed;
            macros.insert(
                id,
                Macro {
                    metadata,
                    data: request.data,
                },
            );
        }
        Self {
            firmware: RefCell::new(Firmware {
                profiles: BTreeMap::from([(1, profile(product, 1)), (2, profile(product, 2))]),
                macros,
                polls: 0,
            }),
            pending: RefCell::new(None),
            reports: RefCell::new(Vec::new()),
            reads: Cell::new(0),
            fail_send: Cell::new(None),
            fail_get: Cell::new(None),
            mismatch: Cell::new(None),
            format_busy: Cell::new(0),
            format_error_reads: Cell::new(0),
            format_clears: Cell::new(true),
            after_format: Cell::new(false),
            faults_after_format: Cell::new(false),
            faults_after_restore: Cell::new(false),
            fragmented_available: Cell::new(None),
            recyclable_memory: Cell::new(0),
            product,
        }
    }
    fn sent(&self, class: u8, id: u8) -> usize {
        self.reports
            .borrow()
            .iter()
            .filter(|r| r[7] == class && r[8] == id)
            .count()
    }
    fn dispatch(&self, report: &[u8]) -> Result<Vec<u8>> {
        let class = report[7];
        let id = report[8];
        let p = &report[9..9 + usize::from(report[6])];
        let mut fw = self.firmware.borrow_mut();
        Ok(match (class, id) {
            (6, 142) => {
                let mut bytes = vec![0x7f, 0xff];
                bytes.extend_from_slice(&8192u32.to_be_bytes());
                let used: usize = fw
                    .macros
                    .values()
                    .map(|m| (m.data.len() + 256).div_ceil(256) * 256)
                    .sum();
                bytes.extend_from_slice(
                    &self
                        .fragmented_available
                        .get()
                        .unwrap_or(8192u32 - used as u32)
                        .to_be_bytes(),
                );
                bytes.extend_from_slice(&self.recyclable_memory.get().to_be_bytes());
                bytes
            }
            (6, 128) => (fw.macros.len() as u16).to_be_bytes().to_vec(),
            (6, 139) => {
                let offset = u16::from_be_bytes([p[0], p[1]]) as usize;
                let count = (p.len() - 4) / 2;
                let ids: Vec<_> = fw.macros.keys().copied().skip(offset).take(count).collect();
                let mut bytes = (offset as u16).to_be_bytes().to_vec();
                bytes.extend_from_slice(&(ids.len() as u16).to_be_bytes());
                for id in ids {
                    bytes.extend_from_slice(&id.to_be_bytes());
                }
                bytes
            }
            (6, 140) => {
                let id = u16::from_be_bytes([p[0], p[1]]);
                let offset = u16::from_be_bytes([p[2], p[3]]) as usize;
                let len = p.len() - 6;
                let mut bytes = p[..4].to_vec();
                bytes.extend_from_slice(&(len as u16).to_be_bytes());
                bytes.extend_from_slice(&fw.macros[&id].metadata[offset..offset + len]);
                bytes
            }
            (6, 136) => {
                let id = u16::from_be_bytes([p[0], p[1]]);
                let mut bytes = p[..2].to_vec();
                bytes.extend_from_slice(&(fw.macros[&id].data.len() as u32).to_be_bytes());
                bytes
            }
            (6, 137) => {
                let id = u16::from_be_bytes([p[0], p[1]]);
                let offset = u32::from_be_bytes(p[2..6].try_into().unwrap()) as usize;
                let len = p[6] as usize;
                let mut bytes = p[..7].to_vec();
                bytes.extend_from_slice(&fw.macros[&id].data[offset..offset + len]);
                bytes
            }
            (6, 8) => {
                let id = u16::from_be_bytes([p[0], p[1]]);
                let len = u32::from_be_bytes(p[2..6].try_into().unwrap()) as usize;
                fw.macros.insert(
                    id,
                    Macro {
                        metadata: vec![0; 250],
                        data: vec![0; len],
                    },
                );
                Vec::new()
            }
            (6, 12) => {
                let id = u16::from_be_bytes([p[0], p[1]]);
                let offset = u16::from_be_bytes([p[2], p[3]]) as usize;
                assert_eq!(&p[4..6], &250u16.to_be_bytes());
                fw.macros.get_mut(&id).unwrap().metadata[offset..offset + p.len() - 6]
                    .copy_from_slice(&p[6..]);
                Vec::new()
            }
            (6, 9) => {
                let id = u16::from_be_bytes([p[0], p[1]]);
                let offset = u32::from_be_bytes(p[2..6].try_into().unwrap()) as usize;
                let len = p[6] as usize;
                assert_eq!(p.len(), len + 7);
                fw.macros.get_mut(&id).unwrap().data[offset..offset + len].copy_from_slice(&p[7..]);
                Vec::new()
            }
            (5, 138) => vec![4],
            (5, 128) => vec![fw.profiles.len() as u8],
            (5, 129) => {
                let mut data = vec![fw.profiles.len() as u8];
                data.extend(fw.profiles.keys().copied());
                data
            }
            (5, 136) => {
                let profile = p[0];
                let offset = u16::from_be_bytes([p[1], p[2]]) as usize;
                let len = report[6] as usize - 5;
                let mut bytes = p[..3].to_vec();
                bytes.extend_from_slice(&(len as u16).to_be_bytes());
                bytes.extend_from_slice(&fw.profiles[&profile].metadata[offset..offset + len]);
                bytes
            }
            (5, 2) => {
                fw.profiles
                    .entry(p[0])
                    .or_insert_with(|| profile(self.product, p[0]));
                Vec::new()
            }
            (5, 8) => {
                let profile = p[0];
                let offset = u16::from_be_bytes([p[1], p[2]]) as usize;
                fw.profiles.get_mut(&profile).unwrap().metadata[offset..offset + p.len() - 5]
                    .copy_from_slice(&p[5..]);
                Vec::new()
            }
            (5, 3) => {
                fw.profiles.remove(&p[0]);
                Vec::new()
            }
            (6, 10) => {
                assert_eq!(p, &[0, 0, 0, 2, 0, 0]);
                fw.polls = 0;
                Vec::new()
            }
            (6, 138) => {
                assert_eq!(p, &[0, 0, 0, 2, 0, 0]);
                fw.polls += 1;
                if fw.polls <= self.format_busy.get() {
                    vec![0, 0, 1, 2]
                } else {
                    if self.format_clears.get() {
                        fw.macros.clear();
                    }
                    self.after_format.set(true);
                    self.fragmented_available.set(None);
                    self.recyclable_memory.set(0);
                    vec![0, 0, 2, 2]
                }
            }
            (15, 128) => vec![1, 0, 1, 1, 1, 4, 0, 3, 1, 1],
            (15, 132) => vec![p[0], p[1], fw.profiles[&p[0]].brightness[&p[1]]],
            (15, 4) => {
                fw.profiles
                    .get_mut(&p[0])
                    .unwrap()
                    .brightness
                    .insert(p[1], p[2]);
                Vec::new()
            }
            (4, 133) => fw.profiles[&p[0]].dpi.clone(),
            (4, 5) => {
                fw.profiles.get_mut(&p[0]).unwrap().dpi = p.to_vec();
                Vec::new()
            }
            (4, 134) => fw.profiles[&p[0]].stages.clone(),
            (4, 6) => {
                let mut data = p.to_vec();
                for i in 0..p[2] as usize {
                    data[3 + 7 * i] += 1;
                }
                fw.profiles.get_mut(&p[0]).unwrap().stages = data;
                Vec::new()
            }
            (2, 140) => {
                let assignment = fw.profiles[&p[0]].buttons[&(p[1], p[2])];
                let mut data = p[..3].to_vec();
                data.extend_from_slice(&[assignment.0, assignment.1]);
                data.extend_from_slice(&assignment.2);
                data
            }
            (2, 12) => {
                fw.profiles
                    .get_mut(&p[0])
                    .unwrap()
                    .buttons
                    .insert((p[1], p[2]), (p[3], p[4], p[5..10].try_into().unwrap()));
                Vec::new()
            }
            (2, _) | (21, _) if id & 0x80 != 0 => {
                vec![p[0], fw.profiles[&p[0]].scrolling[&(class, id & 0x7f)]]
            }
            (2, _) | (21, _) => {
                fw.profiles
                    .get_mut(&p[0])
                    .unwrap()
                    .scrolling
                    .insert((class, id), p[1]);
                Vec::new()
            }
            (0, _) => {
                if id & 0x80 != 0 {
                    vec![p[0], fw.profiles[&p[0]].polling]
                } else {
                    fw.profiles.get_mut(&p[0]).unwrap().polling = p[1];
                    Vec::new()
                }
            }
            other => panic!("Unexpected storage simulation command {other:?}"),
        })
    }
    fn faults_active(&self) -> bool {
        (!self.faults_after_format.get() || self.after_format.get())
            && (!self.faults_after_restore.get()
                || (self.after_format.get() && self.firmware.borrow().macros.len() == 2))
    }
}

impl FeatureTransport for Device {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        assert_eq!(report.len(), 91);
        assert!(report[6] <= 80);
        assert_eq!(report[89], report[3..89].iter().fold(0, |a, b| a ^ b));
        self.reports.borrow_mut().push(report.to_vec());
        if self.faults_active() && self.fail_send.get() == Some((report[7], report[8])) {
            anyhow::bail!("synthetic OUT lost");
        }
        *self.pending.borrow_mut() = Some(report.to_vec());
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        self.reads.set(self.reads.get() + 1);
        let request = self
            .pending
            .borrow_mut()
            .take()
            .expect("No simulated request for IN");
        if self.faults_active() && self.fail_get.get() == Some((request[7], request[8])) {
            anyhow::bail!("synthetic IN lost");
        }
        if (request[7], request[8]) == (6, 138) && self.format_error_reads.get() > 0 {
            self.format_error_reads
                .set(self.format_error_reads.get() - 1);
            anyhow::bail!("synthetic format status read failed");
        }
        let mut data = self.dispatch(&request)?;
        if self.faults_active()
            && self.mismatch.get() == Some((request[7], request[8]))
            && !data.is_empty()
        {
            let last = data.len() - 1;
            data[last] ^= 1;
        }
        report.fill(0);
        report[1] = 2;
        report[2] = request[2];
        report[6] = data.len() as u8;
        report[7] = request[7];
        report[8] = request[8];
        report[9..9 + data.len()].copy_from_slice(&data);
        report[89] = report[3..89].iter().fold(0, |a, b| a ^ b);
        Ok(91)
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn cap(product: u32) -> DeviceReadCapability {
    let mut cap = crate::device_reads::capability(product).unwrap().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_out = 1;
    cap.max_retry_in = 1;
    cap
}
fn txs() -> impl FnMut() -> Result<u8> {
    let mut tx = 0;
    move || {
        tx = (tx + 1) % 31;
        Ok(tx)
    }
}
fn bounds() -> StorageReadBounds {
    StorageReadBounds {
        max_macro_bytes: 512,
        max_total_macro_bytes: 1024,
    }
}

#[test]
fn all_four_products_compact_and_independently_restore_exact_observed_fields() {
    for product in [170, 204, 226, 45066] {
        let device = Device::new(product);
        let cap = cap(product);
        let mut waits = Vec::new();
        let report = MouseStorage::new(&device, &cap)
            .unwrap()
            .compact(bounds(), &mut txs(), &|| Ok(()), &mut |duration| {
                waits.push(duration);
                Ok(())
            })
            .unwrap();
        assert!(report.verified());
        assert!(report.backup.complete());
        assert!(report.after.as_ref().unwrap().complete());
        assert_eq!(waits, [Duration::from_millis(100)]);
        assert_eq!(report.format_statuses, [2]);
        assert_eq!(device.sent(5, 3), 1);
        assert_eq!(device.sent(6, 10), 1);
        assert!(
            report
                .io
                .iter()
                .filter(|step| step.mutation)
                .all(|step| step.attempted && step.reply_received)
        );
        assert_eq!(report.backup.macros[0].metadata.macro_id, 0);
        assert_eq!(
            report.after.as_ref().unwrap().macros[0].metadata.extra_info,
            [0xb5; 16]
        );
        let firmware = device.firmware.borrow();
        assert_eq!(firmware.macros[&0].metadata[230], 0xed);
        assert_eq!(firmware.profiles[&2].metadata[210], 12);
        assert_eq!(firmware.profiles[&2].brightness[&1], 129);
        let stage_write = device
            .reports
            .borrow()
            .iter()
            .find(|r| r[7] == 4 && r[8] == 6)
            .unwrap()
            .clone();
        assert_eq!(stage_write[6], 17);
        assert_eq!(stage_write[12], 0);
        assert_eq!(stage_write[19], 1);
    }
}

#[test]
fn backup_is_read_only_and_retains_firmware_bytes() {
    let device = Device::new(170);
    let cap = cap(170);
    let backup = MouseStorage::new(&device, &cap)
        .unwrap()
        .backup(bounds(), &mut txs(), &|| Ok(()))
        .unwrap();
    assert!(backup.complete());
    assert!(backup.io.iter().all(|step| !step.mutation));
    assert_eq!(backup.macros.len(), 2);
    assert_eq!(backup.profiles.len(), 2);
    assert!(backup.profiles[0].identity.is_none());
    assert!(
        backup
            .profiles
            .iter()
            .all(|profile| profile.buttons.as_ref().unwrap().complete())
    );
    assert_eq!(device.sent(5, 3), 0);
    assert_eq!(device.sent(6, 10), 0);
}

#[test]
fn per_macro_and_total_backup_bounds_abort_before_format() {
    for bounds in [
        StorageReadBounds {
            max_macro_bytes: 72,
            max_total_macro_bytes: 1024,
        },
        StorageReadBounds {
            max_macro_bytes: 512,
            max_total_macro_bytes: 200,
        },
    ] {
        let device = Device::new(170);
        let cap = cap(170);
        let failure = MouseStorage::new(&device, &cap)
            .unwrap()
            .compact(bounds, &mut txs(), &|| Ok(()), &mut |_| {
                panic!("Bounded failure must not wait")
            })
            .unwrap_err();
        assert_eq!(failure.phase, StorageFailurePhase::Backup);
        assert!(!failure.progress.backup.complete());
        assert!(!failure.device_state_may_have_changed());
        assert_eq!(device.sent(5, 3), 0);
        assert_eq!(device.sent(6, 10), 0);
    }
}

#[test]
fn a_missing_button_observation_prevents_every_destructive_exchange() {
    let device = Device::new(170);
    device.fail_get.set(Some((2, 140)));
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::Backup);
    assert!(!failure.device_state_may_have_changed());
    let buttons = failure.progress.backup.profiles[0]
        .buttons
        .as_ref()
        .unwrap();
    assert!(buttons.source_scan_completed);
    assert!(!buttons.complete());
    assert!(buttons.entries.iter().all(|entry| entry.error.is_some()));
    assert_eq!(device.sent(5, 3), 0);
}

#[test]
fn first_observed_profile_must_be_the_reserved_runtime_slot() {
    let device = Device::new(170);
    device.firmware.borrow_mut().profiles.remove(&1);
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::Backup);
    assert!(!failure.device_state_may_have_changed());
    assert_eq!(device.sent(5, 3), 0);
    assert_eq!(device.sent(6, 10), 0);
}

#[test]
fn source_format_busy_and_transient_errors_have_a_three_poll_bound() {
    for (busy, errors) in [(2, 0), (0, 2)] {
        let device = Device::new(170);
        device.format_busy.set(busy);
        device.format_error_reads.set(errors);
        let cap = cap(170);
        let report = MouseStorage::new(&device, &cap)
            .unwrap()
            .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
            .unwrap();
        assert!(report.verified());
        assert_eq!(device.sent(6, 138), 3);
        assert_eq!(
            report
                .io
                .iter()
                .filter(|step| step.command == GET_FORMAT && step.error.is_some())
                .count(),
            errors
        );
    }
}

#[test]
fn incomplete_format_does_not_restore_or_claim_completion() {
    let device = Device::new(170);
    device.format_busy.set(3);
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::FormatPoll);
    assert_eq!(failure.progress.format_statuses, [1, 1, 1]);
    assert!(failure.device_state_may_have_changed());
    assert!(!failure.progress.verified());
    assert!(failure.progress.restoration.is_empty());
    assert_eq!(device.sent(6, 138), 3);
}

#[test]
fn status_completion_without_empty_inventory_stops_before_restoration() {
    let device = Device::new(170);
    device.format_clears.set(false);
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::VerifyEmpty);
    assert_eq!(failure.progress.format_statuses, [2]);
    assert!(!failure.progress.verified());
    assert!(failure.progress.restoration.is_empty());
    assert_eq!(device.sent(6, 8), 0);
}

#[test]
fn lost_delete_ack_and_lost_format_ack_are_retained_as_possible_changes() {
    for (command, phase) in [
        ((5, 3), StorageFailurePhase::DeleteProfiles),
        ((6, 10), StorageFailurePhase::FormatStart),
    ] {
        let device = Device::new(170);
        device.fail_get.set(Some(command));
        let cap = cap(170);
        let failure = MouseStorage::new(&device, &cap)
            .unwrap()
            .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
            .unwrap_err();
        assert_eq!(failure.phase, phase);
        assert!(failure.device_state_may_have_changed());
        assert!(!failure.progress.verified());
        let step = failure.progress.io.last().unwrap();
        assert!(step.attempted);
        assert!(!step.reply_received);
        assert!(step.error.is_some());
        assert!(failure.progress.backup.complete());
        assert!(failure.progress.restoration.is_empty());
    }
}

#[test]
fn cancellation_during_format_wait_keeps_the_complete_backup_and_stops_polls() {
    let device = Device::new(170);
    let cap = cap(170);
    let cancelled = Cell::new(false);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(
            bounds(),
            &mut txs(),
            &|| {
                if cancelled.get() {
                    anyhow::bail!("Cancelled");
                }
                Ok(())
            },
            &mut |duration| {
                assert_eq!(duration, Duration::from_millis(100));
                cancelled.set(true);
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::Cancelled);
    assert!(failure.device_state_may_have_changed());
    assert!(failure.progress.backup.complete());
    assert_eq!(device.sent(6, 138), 0);
}

#[test]
fn constructor_requires_the_audited_collection_and_feature_route() {
    let device = Device::new(170);
    for variant in 0..4 {
        let mut cap = cap(170);
        match variant {
            0 => cap.vendor_id = 1,
            1 => cap.claim_interface = 3,
            2 => cap.report_bytes = 90,
            _ => cap.product_id = 222,
        }
        assert!(MouseStorage::new(&device, &cap).is_err());
    }
}

#[test]
fn backup_cancellation_is_read_only_and_retains_completed_macros() {
    let device = Device::new(170);
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(
            bounds(),
            &mut txs(),
            &|| {
                if device.reads.get() > 15 {
                    anyhow::bail!("Cancelled");
                }
                Ok(())
            },
            &mut |_| Ok(()),
        )
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::Cancelled);
    assert!(!failure.device_state_may_have_changed());
    assert!(!failure.progress.backup.macros.is_empty());
    assert_eq!(device.sent(5, 3), 0);
}

#[test]
fn each_restore_stage_stops_and_retains_the_complete_backup_on_failure() {
    for (command, phase) in [
        ((5, 2), StorageFailurePhase::RestoreIdentity),
        ((4, 5), StorageFailurePhase::RestoreSettings),
        ((0, 14), StorageFailurePhase::RestorePolling),
        ((2, 12), StorageFailurePhase::RestoreButtons),
        ((6, 8), StorageFailurePhase::RestoreMacros),
    ] {
        let device = Device::new(170);
        device.fail_get.set(Some(command));
        device.faults_after_format.set(true);
        let cap = cap(170);
        let failure = MouseStorage::new(&device, &cap)
            .unwrap()
            .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
            .unwrap_err();
        assert_eq!(failure.phase, phase);
        assert!(failure.device_state_may_have_changed());
        assert!(failure.progress.backup.complete());
        assert!(!failure.progress.verified());
        assert!(failure.progress.after.is_none());
        if phase == StorageFailurePhase::RestoreSettings {
            assert!(failure.progress.io.last().unwrap().attempted);
            assert!(!failure.progress.io.last().unwrap().reply_received);
        } else if phase == StorageFailurePhase::RestoreMacros {
            let StorageRestoreEntry::Macro { report, .. } =
                failure.progress.restoration.last().unwrap()
            else {
                panic!("Missing failed macro report")
            };
            assert!(report.steps[0].attempted);
            assert!(!report.steps[0].acknowledgment_received);
        }
    }
}

#[test]
fn restore_setting_acknowledgment_is_not_independent_readback() {
    let device = Device::new(170);
    device.mismatch.set(Some((4, 133)));
    device.faults_after_format.set(true);
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::RestoreSettings);
    assert!(!failure.progress.verified());
    let io = &failure.progress.io;
    let setter = &io[io.len() - 2];
    let getter = io.last().unwrap();
    assert!(setter.mutation && setter.attempted && setter.reply_received);
    assert!(setter.response.is_empty());
    assert!(!getter.mutation && getter.reply_received);
    assert_ne!(
        getter.response,
        failure.progress.backup.profiles[0].settings[0].value
    );
}

#[test]
fn final_full_readback_detects_changes_after_successful_individual_restores() {
    let device = Device::new(170);
    device.mismatch.set(Some((4, 133)));
    device.faults_after_restore.set(true);
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::Readback);
    assert!(!failure.progress.verified());
    assert!(failure.progress.after.as_ref().unwrap().complete());
    assert_eq!(
        failure
            .progress
            .restoration
            .iter()
            .filter(|entry| matches!(entry, StorageRestoreEntry::Macro { .. }))
            .count(),
        2
    );
    assert!(failure.device_state_may_have_changed());
}

#[test]
fn malformed_format_selector_stops_with_the_complete_backup() {
    let device = Device::new(170);
    device.mismatch.set(Some((6, 138)));
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::FormatPoll);
    assert!(failure.progress.restoration.is_empty());
    assert_eq!(device.sent(6, 138), 1);
    assert!(failure.progress.backup.complete());
}

#[test]
fn unsupported_observed_button_size_is_rejected_before_delete() {
    let device = Device::new(170);
    device
        .firmware
        .borrow_mut()
        .profiles
        .get_mut(&1)
        .unwrap()
        .buttons
        .get_mut(&(1, 0))
        .unwrap()
        .1 = 6;
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::Backup);
    assert!(!failure.device_state_may_have_changed());
    assert!(
        failure.progress.backup.profiles[0]
            .buttons
            .as_ref()
            .unwrap()
            .complete()
    );
    assert_eq!(device.sent(5, 3), 0);
}

#[test]
fn unsupported_observed_dpi_indices_prevent_formatting() {
    let device = Device::new(170);
    device
        .firmware
        .borrow_mut()
        .profiles
        .get_mut(&1)
        .unwrap()
        .stages[3] = 3;
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::Backup);
    assert!(!failure.device_state_may_have_changed());
    assert_eq!(device.sent(6, 10), 0);
}

#[test]
fn observed_high_macro_id_is_not_formatted_into_an_unrestorable_state() {
    let device = Device::new(170);
    let saved = device.firmware.borrow_mut().macros.remove(&0).unwrap();
    device.firmware.borrow_mut().macros.insert(0x8000, saved);
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, StorageFailurePhase::Backup);
    assert!(!failure.device_state_may_have_changed());
    assert_eq!(device.sent(6, 10), 0);
}

#[test]
fn unknown_observed_polling_code_is_restored_without_default_conversion() {
    let device = Device::new(170);
    device
        .firmware
        .borrow_mut()
        .profiles
        .get_mut(&2)
        .unwrap()
        .polling = 77;
    let cap = cap(170);
    let report = MouseStorage::new(&device, &cap)
        .unwrap()
        .compact(bounds(), &mut txs(), &|| Ok(()), &mut |_| Ok(()))
        .unwrap();
    assert!(report.verified());
    let polling = report.backup.profiles[1].polling.as_ref().unwrap();
    assert_eq!(polling.raw_code, 77);
    assert_eq!(polling.known_hz, None);
    assert_eq!(polling.source_hz, 100);
    assert_eq!(device.firmware.borrow().profiles[&2].polling, 77);
}

fn new_macro() -> MacroWriteRequest {
    MacroWriteRequest {
        guid: "12345678-1234-5678-9012-123456789abc".into(),
        num_items: 2,
        name: "New".into(),
        timestamp: "20261012".into(),
        hash: "NewHash".into(),
        data: vec![1, 4, 2, 4],
    }
}

#[test]
fn actual_macro_add_compacts_fragmented_storage_then_uploads_and_reads_back() {
    let device = Device::new(170);
    device.fragmented_available.set(Some(256));
    device.recyclable_memory.set(4096);
    let cap = cap(170);
    let report = MouseStorage::new(&device, &cap)
        .unwrap()
        .write_macro_with_compaction(
            18,
            &new_macro(),
            bounds(),
            &mut txs(),
            &|| Ok(()),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(report.verified());
    assert!(report.initial.as_ref().unwrap().steps.is_empty());
    assert!(report.compaction.as_ref().unwrap().verified());
    assert!(report.upload.as_ref().unwrap().verified());
    assert!(report.device_state_may_have_changed());
    assert_eq!(device.sent(6, 10), 1);
    assert_eq!(
        device
            .firmware
            .borrow()
            .macros
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [0, 17, 18]
    );
}

#[test]
fn ordinary_macro_upload_has_no_format_or_profile_mutation() {
    let device = Device::new(170);
    let cap = cap(170);
    let report = MouseStorage::new(&device, &cap)
        .unwrap()
        .write_macro_with_compaction(
            18,
            &new_macro(),
            bounds(),
            &mut txs(),
            &|| Ok(()),
            &mut |_| panic!("Upload with enough space does not wait"),
        )
        .unwrap();
    assert!(report.verified());
    assert!(report.compaction.is_none());
    assert!(report.initial.is_none());
    assert_eq!(device.sent(6, 10), 0);
    assert_eq!(device.sent(5, 3), 0);
}

#[test]
fn duplicate_ids_and_insufficient_total_space_never_trigger_compaction() {
    for (id, available, recyclable) in [(0, 256, 4096), (18, 256, 0)] {
        let device = Device::new(170);
        device.fragmented_available.set(Some(available));
        device.recyclable_memory.set(recyclable);
        let cap = cap(170);
        let failure = MouseStorage::new(&device, &cap)
            .unwrap()
            .write_macro_with_compaction(
                id,
                &new_macro(),
                bounds(),
                &mut txs(),
                &|| Ok(()),
                &mut |_| panic!("Prepare failure does not wait"),
            )
            .unwrap_err();
        assert_eq!(failure.phase, MacroStorageFailurePhase::Upload);
        assert_eq!(failure.macro_phase, Some(MacroFailurePhase::Prepare));
        assert!(!failure.progress.device_state_may_have_changed());
        assert!(failure.progress.compaction.is_none());
        assert_eq!(device.sent(6, 10), 0);
    }
}

#[test]
fn macro_add_stops_on_failed_compaction_without_allocating_the_requested_id() {
    let device = Device::new(170);
    device.fragmented_available.set(Some(256));
    device.recyclable_memory.set(4096);
    device.format_busy.set(3);
    let cap = cap(170);
    let failure = MouseStorage::new(&device, &cap)
        .unwrap()
        .write_macro_with_compaction(
            18,
            &new_macro(),
            bounds(),
            &mut txs(),
            &|| Ok(()),
            &mut |_| Ok(()),
        )
        .unwrap_err();
    assert_eq!(failure.phase, MacroStorageFailurePhase::Compaction);
    assert_eq!(failure.storage_phase, Some(StorageFailurePhase::FormatPoll));
    assert!(
        failure
            .progress
            .compaction
            .as_ref()
            .unwrap()
            .backup
            .complete()
    );
    assert!(failure.progress.upload.is_none());
    assert_eq!(device.sent(6, 8), 0);
    assert!(!failure.progress.verified());
}

#[test]
fn receipt_pins_commands_completion_bounds_and_lossless_restore_policy() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../testdata/re/mouse-compaction-current-source.json"
    ))
    .unwrap();
    assert_eq!(
        receipt["product_ids"],
        serde_json::json!([170, 204, 226, 45066])
    );
    assert_eq!(
        receipt["format_sequence"]["set_command"],
        serde_json::json!(SET_FORMAT)
    );
    assert_eq!(
        receipt["format_sequence"]["get_command"],
        serde_json::json!(GET_FORMAT)
    );
    assert_eq!(receipt["format_sequence"]["wait_ms"], 100);
    assert_eq!(receipt["format_sequence"]["status_poll_bound"], 3);
    assert_eq!(receipt["format_sequence"]["completed_status"], 2);
    for product in receipt["products"].as_array().unwrap() {
        assert!(
            product["options_receipt"]["source"]
                .as_str()
                .unwrap()
                .contains("resetProfile:!1")
        );
        for helper in product["formatHelpers"].as_array().unwrap() {
            assert_eq!(helper["receipt"]["sha256"].as_str().unwrap().len(), 64);
            assert!(
                helper["receipt"]["path"]
                    .as_str()
                    .unwrap()
                    .starts_with(".work/device-functions-current/")
            );
        }
    }
}

use super::*;
use crate::{
    feature_exchange::tests::{ExchangeProbe, LeaseEvent},
    keyboard_submission::{Intent, submit},
    onboard_macro::MacroWriteRequest,
};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

const ACTIVE: &str = "11223344-5566-7788-99aa-bbccddeeff12";
const NEW_MACRO: &str = "22334455-6677-8899-aabb-ccddeeff1122";
const STANDBY: &str = "33445566-7788-99aa-bbcc-ddeeff112233";
const STANDBY_PROFILE: &str = "66778899-aabb-4ccd-8eef-112233445566";
const LOCKED: &str = "44556677-8899-aabb-ccdd-eeff11223344";
const ORPHAN: &str = "55667788-99aa-bbcc-ddee-ff1122334455";

#[test]
fn locked_assignment_release_failure_retains_raw_response_and_blocks_macro_mutations() {
    let mut firmware = Firmware::new(4096);
    firmware.foreign_profile = true;
    firmware.saved(7, LOCKED);
    firmware.saved(8, ORPHAN);
    let finish_check = || {
        ensure!(
            firmware.sent.borrow().last().unwrap()[7..9] != [2, 158],
            "simulated locked assignment release failed"
        );
        Ok(())
    };
    let probe =
        crate::feature_exchange::tests::ExchangeProbe::with_finish_check(&firmware, &finish_check);
    let failure = submit(
        &probe,
        740,
        ACTIVE,
        intent(context(false)),
        transactions(),
        || Ok(()),
    )
    .unwrap_err();
    assert!(!failure.receipt.completed);
    assert_eq!(failure.receipt.submissions.len(), 1);
    let step = &failure.receipt.submissions[0];
    assert_eq!(step.command, [80, 2, 158]);
    assert!(step.attempted);
    assert_eq!(
        step.acknowledgement,
        Some(vec![4, 31, 0, 3, 2, 0, 7, 0, 0, 0])
    );
    let progress = failure.receipt.macro_synchronization.as_ref().unwrap();
    assert!(progress.locked_ids.is_empty());
    assert!(progress.mutations.is_empty());
    assert!(firmware.mutations().is_empty());
    assert_eq!(
        firmware.macros.borrow().keys().copied().collect::<Vec<_>>(),
        [7, 8]
    );
    assert!(format!("{:#}", failure.error).contains("simulated locked assignment release failed"));
    assert_eq!(firmware.sent.borrow().last().unwrap()[7..9], [2, 158]);
}

#[derive(Clone)]
struct SavedMacro {
    metadata: [u8; 250],
    data: Vec<u8>,
}

struct Firmware {
    sent: RefCell<Vec<Vec<u8>>>,
    received: Cell<usize>,
    reject_once: RefCell<Option<[u8; 2]>>,
    macros: RefCell<BTreeMap<u16, SavedMacro>>,
    available: u32,
    foreign_profile: bool,
    corrupt_readback: bool,
}

fn guid_bytes(guid: &str) -> [u8; 16] {
    let text = guid.replace('-', "");
    let mut bytes = [0; 16];
    for (index, value) in bytes.iter_mut().enumerate() {
        *value = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap();
    }
    bytes[..4].reverse();
    bytes[4..6].reverse();
    bytes[6..8].reverse();
    bytes
}

impl Firmware {
    fn new(available: u32) -> Self {
        Self {
            sent: RefCell::new(Vec::new()),
            received: Cell::new(0),
            reject_once: RefCell::new(None),
            macros: RefCell::new(BTreeMap::new()),
            available,
            foreign_profile: false,
            corrupt_readback: false,
        }
    }
    fn saved(&self, id: u16, guid: &str) {
        let mut metadata = [0; 250];
        metadata[..16].copy_from_slice(&guid_bytes(guid));
        metadata[16..20].copy_from_slice(&1u32.to_le_bytes());
        metadata[20..25].copy_from_slice(b"saved");
        self.macros.borrow_mut().insert(
            id,
            SavedMacro {
                metadata,
                data: vec![1, 4],
            },
        );
    }
    fn mutations(&self) -> Vec<(u8, u8, Vec<u8>)> {
        self.sent
            .borrow()
            .iter()
            .filter(|packet| packet[7] == 6 && packet[8] < 128 || packet[7..9] == [2, 13])
            .map(|packet| {
                (
                    packet[7],
                    packet[8],
                    packet[9..9 + usize::from(packet[6])].to_vec(),
                )
            })
            .collect()
    }
    fn data(&self, packet: &[u8]) -> Vec<u8> {
        let payload = &packet[9..9 + usize::from(packet[6])];
        match (packet[7], packet[8]) {
            (5, 129) => {
                if self.foreign_profile {
                    vec![3, 1, 3, 4]
                } else {
                    vec![2, 1, 3]
                }
            }
            (5, 136) => {
                let slot = payload[0];
                let offset = usize::from(u16::from_be_bytes([payload[1], payload[2]]));
                let count = (250 - offset).min(64);
                let mut raw = [0; 250];
                raw[..16].copy_from_slice(&guid_bytes(if slot == 3 {
                    ACTIVE
                } else if slot == 4 {
                    LOCKED
                } else {
                    ORPHAN
                }));
                raw[16..21].copy_from_slice(b"local");
                let owner = if slot == 4 {
                    b"foreign-owner".as_slice()
                } else {
                    crate::keyboard_magnetic_mapping::guest_profile_owner().as_bytes()
                };
                raw[116..116 + owner.len()].copy_from_slice(owner);
                let mut data = vec![slot, payload[1], payload[2], 0, count as u8];
                data.extend_from_slice(&raw[offset..offset + count]);
                data
            }
            (2, 132) => vec![1, 31],
            (2, 158) => {
                let mut data = payload.to_vec();
                for row in data[1..].chunks_exact_mut(9) {
                    if row[1] == 0 {
                        row[2] = 3;
                        row[3] = 2;
                        row[4] = 0;
                        row[5] = 7;
                    } else {
                        row[2] = 2;
                        row[3] = 2;
                        row[4] = 0;
                        row[5] = 4;
                    }
                }
                data
            }
            (6, 142) => {
                let mut data = vec![0x7f, 0xff];
                data.extend_from_slice(&4096u32.to_be_bytes());
                data.extend_from_slice(&self.available.to_be_bytes());
                data.extend_from_slice(&0u32.to_be_bytes());
                data
            }
            (6, 128) => (self.macros.borrow().len() as u16).to_be_bytes().to_vec(),
            (6, 139) => {
                let offset = usize::from(u16::from_be_bytes([payload[0], payload[1]]));
                let entries = self
                    .macros
                    .borrow()
                    .keys()
                    .copied()
                    .skip(offset)
                    .take(32)
                    .collect::<Vec<_>>();
                let mut data = payload[..2].to_vec();
                data.extend_from_slice(&(entries.len() as u16).to_be_bytes());
                for id in entries {
                    data.extend_from_slice(&id.to_be_bytes());
                }
                data
            }
            (6, 140) => {
                let id = u16::from_be_bytes([payload[0], payload[1]]);
                let offset = usize::from(u16::from_be_bytes([payload[2], payload[3]]));
                let count = (250 - offset).min(64);
                let saved = self.macros.borrow();
                let mut data = payload[..4].to_vec();
                data.extend_from_slice(&250u16.to_be_bytes());
                data.extend_from_slice(&saved[&id].metadata[offset..offset + count]);
                if self.corrupt_readback && id == 9 && offset == 192 {
                    *data.last_mut().unwrap() ^= 1;
                }
                data
            }
            (6, 136) => {
                let id = u16::from_be_bytes([payload[0], payload[1]]);
                let mut data = payload[..2].to_vec();
                data.extend_from_slice(
                    &(self.macros.borrow()[&id].data.len() as u32).to_be_bytes(),
                );
                data
            }
            (6, 137) => {
                let id = u16::from_be_bytes([payload[0], payload[1]]);
                let offset = u32::from_be_bytes(payload[2..6].try_into().unwrap()) as usize;
                let count = usize::from(payload[6]);
                let mut data = payload[..7].to_vec();
                data.extend_from_slice(&self.macros.borrow()[&id].data[offset..offset + count]);
                data
            }
            (6, 8) => {
                let id = u16::from_be_bytes([payload[0], payload[1]]);
                let count = u32::from_be_bytes(payload[2..6].try_into().unwrap()) as usize;
                assert!(
                    self.macros
                        .borrow_mut()
                        .insert(
                            id,
                            SavedMacro {
                                metadata: [0; 250],
                                data: vec![0; count]
                            }
                        )
                        .is_none()
                );
                vec![]
            }
            (6, 12) => {
                let id = u16::from_be_bytes([payload[0], payload[1]]);
                let offset = usize::from(u16::from_be_bytes([payload[2], payload[3]]));
                self.macros.borrow_mut().get_mut(&id).unwrap().metadata
                    [offset..offset + payload.len() - 6]
                    .copy_from_slice(&payload[6..]);
                vec![]
            }
            (6, 9) => {
                let id = u16::from_be_bytes([payload[0], payload[1]]);
                let offset = u32::from_be_bytes(payload[2..6].try_into().unwrap()) as usize;
                let count = usize::from(payload[6]);
                self.macros.borrow_mut().get_mut(&id).unwrap().data[offset..offset + count]
                    .copy_from_slice(&payload[7..7 + count]);
                vec![]
            }
            (6, 3) => {
                let id = u16::from_be_bytes([payload[0], payload[1]]);
                assert!(self.macros.borrow_mut().remove(&id).is_some());
                vec![]
            }
            (2, 13) => vec![],
            command => panic!("unexpected simulated command {command:?}"),
        }
    }
}
impl FeatureTransport for Firmware {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        assert_eq!(report.len(), 91);
        assert_eq!(
            report[89],
            report[3..89].iter().fold(0, |sum, byte| sum ^ byte)
        );
        self.sent.borrow_mut().push(report.to_vec());
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        let packet = self.sent.borrow().last().unwrap().clone();
        self.received.set(self.received.get() + 1);
        if self
            .reject_once
            .borrow()
            .is_some_and(|command| packet[7..9] == command)
        {
            self.reject_once.borrow_mut().take();
            report.fill(0);
            report[1] = 5;
            report[2] = packet[2];
            report[7..9].copy_from_slice(&packet[7..9]);
            return Ok(91);
        }
        let data = self.data(&packet);
        report.fill(0);
        report[1] = 2;
        report[2] = packet[2];
        report[6] = data.len() as u8;
        report[7..9].copy_from_slice(&packet[7..9]);
        report[9..9 + data.len()].copy_from_slice(&data);
        Ok(91)
    }
    fn metadata(&self) -> Value {
        json!({"simulated":true})
    }
}

fn context(with_standby: bool) -> MacroSynchronization {
    let mut profiles = vec![crate::onboard_macro_codec::ProfileMacroReferences {
        guid: ACTIVE.into(),
        macro_guids: vec![NEW_MACRO.into()],
    }];
    if with_standby {
        profiles.push(crate::onboard_macro_codec::ProfileMacroReferences {
            guid: STANDBY_PROFILE.into(),
            macro_guids: vec![STANDBY.into()],
        });
    }
    MacroSynchronization {
        profiles,
        macros: vec![MacroWriteRequest {
            guid: NEW_MACRO.into(),
            name: "new".into(),
            num_items: 2,
            timestamp: String::new(),
            hash: "0123456789abcdef0123456789abcdef".into(),
            data: vec![1, 4, 2, 4],
        }],
        random_candidates: vec![9],
    }
}
fn intent(context: MacroSynchronization) -> Intent {
    Intent::MappingChanges {
        assignments: vec![
            crate::keyboard_magnetic_mapping::PreparedAssignment::Macro {
                key_id: 31,
                mode: 0,
                guid: NEW_MACRO.into(),
                playback: crate::onboard_assignment::MacroPlayback::Once,
            },
        ],
        points: Vec::new(),
        keys: Vec::new(),
        macro_sync: Some(context),
        single_key_snap_tap: None,
    }
}
fn transactions() -> impl FnMut() -> anyhow::Result<u8> {
    let mut tx = 0;
    move || {
        let result = tx;
        tx = (tx + 1) % 31;
        Ok(result)
    }
}

// The current 740/746 Ec/Lt and OBM engine chains observe both profiles, load
// the catalog, upload and independently read back the macro, then reload the
// catalog before mapping either slot. These expected commands do not use the
// Rust command builders or the simulated device's output.
const UPLOAD_COMMANDS: [[u8; 2]; 38] = [
    [5, 129],
    [5, 136],
    [5, 136],
    [5, 136],
    [5, 136],
    [5, 136],
    [5, 136],
    [5, 136],
    [5, 136],
    [6, 142],
    [6, 128],
    [6, 142],
    [6, 128],
    [6, 8],
    [6, 12],
    [6, 12],
    [6, 12],
    [6, 12],
    [6, 9],
    [6, 142],
    [6, 140],
    [6, 140],
    [6, 140],
    [6, 140],
    [6, 136],
    [6, 137],
    [6, 128],
    [6, 139],
    [6, 142],
    [6, 128],
    [6, 139],
    [6, 140],
    [6, 140],
    [6, 140],
    [6, 140],
    [6, 136],
    [2, 13],
    [2, 13],
];

fn standby_delete_commands() -> Vec<[u8; 2]> {
    let mut commands = UPLOAD_COMMANDS[..9].to_vec();
    commands.extend([[6, 142], [6, 128], [6, 139]]);
    for _ in [0, 5] {
        commands.extend([[6, 140]; 4]);
        commands.push([6, 136]);
    }
    // The first add fails its calculated capacity check after inventory. The
    // delete observes its own inventory before its ACK, refresh and readback.
    commands.extend([
        [6, 142],
        [6, 128],
        [6, 139],
        [6, 142],
        [6, 128],
        [6, 139],
        [6, 3],
        [6, 142],
        [6, 128],
        [6, 139],
    ]);
    commands
}

#[derive(Debug)]
struct ReleaseFault {
    exchange: usize,
}

impl std::fmt::Display for ReleaseFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "release rejected at exchange {}", self.exchange)
    }
}

impl std::error::Error for ReleaseFault {}

fn assert_release_fault(error: &anyhow::Error, exchange: usize) {
    let error = error
        .downcast_ref::<MacroWriteFailure>()
        .map(MacroWriteFailure::error)
        .unwrap_or(error);
    assert!(crate::backend::exchange_finalization_failed(error));
    assert!(!crate::backend::exchange_release_completed(error));
    assert_eq!(
        error.downcast_ref::<ReleaseFault>().unwrap().exchange,
        exchange
    );
}

fn assert_exchange_trace(
    firmware: &Firmware,
    probe: &ExchangeProbe<'_>,
    commands: &[[u8; 2]],
    cancelled: bool,
) {
    let sent = firmware.sent.borrow();
    assert_eq!(
        sent.iter()
            .map(|packet| [packet[7], packet[8]])
            .collect::<Vec<_>>(),
        commands
    );
    assert_eq!(firmware.received.get(), commands.len());
    let events = probe.events();
    assert_eq!(events.len(), commands.len() * 4);
    for (index, events) in events.chunks_exact(4).enumerate() {
        assert!(matches!(events[0], LeaseEvent::Begin(_)));
        assert_eq!(events[1], LeaseEvent::Send);
        assert_eq!(events[2], LeaseEvent::Get);
        assert_eq!(
            events[3],
            if cancelled && index + 1 == commands.len() {
                LeaseEvent::ForcedDrop
            } else {
                LeaseEvent::Finish
            }
        );
    }
}

#[test]
fn magnetic_profile_inventory_release_failure_never_retries_or_starts_macro_io() {
    for product in [740, 746] {
        for stop in 1..=9 {
            let firmware = Firmware::new(4096);
            let finish_check = || {
                if firmware.received.get() == stop {
                    return Err(ReleaseFault { exchange: stop }.into());
                }
                Ok(())
            };
            let probe = ExchangeProbe::with_finish_check(&firmware, &finish_check);
            let failure = submit(
                &probe,
                product,
                ACTIVE,
                intent(context(false)),
                transactions(),
                || Ok(()),
            )
            .unwrap_err();
            assert_release_fault(&failure.error, stop);
            assert!(!failure.receipt.completed);
            assert!(failure.receipt.inventory.is_empty());
            assert!(failure.receipt.submissions.is_empty());
            assert!(failure.receipt.macro_synchronization.is_none());
            assert!(firmware.macros.borrow().is_empty());
            assert_exchange_trace(&firmware, &probe, &UPLOAD_COMMANDS[..stop], false);
        }
    }
}

#[test]
fn magnetic_profile_inventory_retries_an_ordinary_source_error_and_uses_readback() {
    for product in [740, 746] {
        let firmware = Firmware::new(4096);
        *firmware.reject_once.borrow_mut() = Some([5, 136]);
        let probe = ExchangeProbe::new(&firmware);
        let receipt = submit(
            &probe,
            product,
            ACTIVE,
            intent(context(false)),
            transactions(),
            || Ok(()),
        )
        .unwrap();
        assert!(receipt.completed);
        assert_eq!(receipt.inventory.len(), 2);
        assert_eq!(receipt.inventory[0].metadata_attempts, 2);
        assert_eq!(receipt.inventory[1].metadata_attempts, 1);
        assert_eq!(receipt.matched_slots, [3]);
        assert!(
            receipt.macro_synchronization.unwrap().mutations[0]
                .progress
                .verified()
        );
        let mut commands = UPLOAD_COMMANDS.to_vec();
        commands.insert(1, [5, 136]);
        assert_exchange_trace(&firmware, &probe, &commands, false);
        let sent = firmware.sent.borrow();
        assert_eq!(&sent[1][9..14], &[1, 0, 0, 0, 0]);
        assert_eq!(&sent[2][9..14], &[1, 0, 0, 0, 0]);
    }
}

#[test]
fn magnetic_macro_release_failure_preserves_progress_and_stops_the_complete_submission_chain() {
    for product in [740, 746] {
        for stop in 10..=38 {
            let firmware = Firmware::new(4096);
            let finish_check = || {
                if firmware.received.get() == stop {
                    return Err(ReleaseFault { exchange: stop }.into());
                }
                Ok(())
            };
            let probe = ExchangeProbe::with_finish_check(&firmware, &finish_check);
            let failure = submit(
                &probe,
                product,
                ACTIVE,
                intent(context(false)),
                transactions(),
                || Ok(()),
            )
            .unwrap_err();
            assert_release_fault(&failure.error, stop);
            assert!(!failure.receipt.completed);
            assert_eq!(failure.receipt.submissions.len(), stop.saturating_sub(36));
            for step in &failure.receipt.submissions {
                assert!(step.attempted);
                assert_eq!(step.acknowledgement, Some(Vec::new()));
            }
            let progress = failure.receipt.macro_synchronization.unwrap();
            assert_eq!(progress.after.is_some(), stop >= 37);
            assert_eq!(progress.before.is_some(), stop >= 12);
            assert_eq!(progress.mutations.len(), usize::from(stop >= 12));
            if let Some(mutation) = progress.mutations.first() {
                let expected_phase = match stop {
                    12..=13 => Some(MacroFailurePhase::Observe),
                    14 => Some(MacroFailurePhase::Allocate),
                    15..=18 => Some(MacroFailurePhase::Metadata),
                    19 => Some(MacroFailurePhase::Data),
                    20 => Some(MacroFailurePhase::StorageRefresh),
                    21..=28 => Some(MacroFailurePhase::Readback),
                    29..=38 => None,
                    _ => unreachable!(),
                };
                assert_eq!(mutation.failure_phase, expected_phase, "{product}/{stop}");
                let report = &mutation.progress;
                assert_eq!(report.verified(), stop >= 29);
                assert_eq!(report.storage_before.is_some(), stop >= 14);
                assert_eq!(report.storage_after.is_some(), stop >= 21);
                assert_eq!(report.metadata_readback.is_some(), stop >= 25);
                assert_eq!(report.data_size_readback.is_some(), stop >= 26);
                assert_eq!(report.data_readback.is_some(), stop >= 27);
                assert_eq!(report.inventory_after.is_some(), stop >= 29);
                assert_eq!(report.steps.len(), stop.saturating_sub(13).min(6));
                for (index, step) in report.steps.iter().enumerate() {
                    assert!(step.attempted);
                    assert!(step.acknowledgment_received);
                    assert!(step.acknowledgment_data.is_empty());
                    assert_eq!(step.error.is_some(), index + 14 == stop);
                }
                if stop <= 28 {
                    assert_eq!(
                        progress.calculated_memory,
                        Some(
                            KeyboardMacroMemory::observe(progress.before.as_ref().unwrap(), 2)
                                .unwrap()
                        )
                    );
                }
            }
            assert_eq!(firmware.macros.borrow().contains_key(&9), stop >= 14);
            assert_exchange_trace(&firmware, &probe, &UPLOAD_COMMANDS[..stop], false);
        }
    }
}

#[test]
fn magnetic_macro_standby_release_failure_never_retries_the_add() {
    let commands = standby_delete_commands();
    for product in [740, 746] {
        for (stop, phase) in [
            (26, MacroFailurePhase::Observe),
            (27, MacroFailurePhase::Observe),
            (28, MacroFailurePhase::Observe),
            (29, MacroFailurePhase::Delete),
            (30, MacroFailurePhase::StorageRefresh),
            (31, MacroFailurePhase::Readback),
            (32, MacroFailurePhase::Readback),
        ] {
            let firmware = Firmware::new(1536);
            firmware.saved(0, STANDBY);
            firmware.saved(5, STANDBY);
            let finish_check = || {
                if firmware.received.get() == stop {
                    return Err(ReleaseFault { exchange: stop }.into());
                }
                Ok(())
            };
            let probe = ExchangeProbe::with_finish_check(&firmware, &finish_check);
            let failure = submit(
                &probe,
                product,
                ACTIVE,
                intent(context(true)),
                transactions(),
                || Ok(()),
            )
            .unwrap_err();
            assert_release_fault(&failure.error, stop);
            assert!(!failure.receipt.completed);
            assert!(failure.receipt.submissions.is_empty());
            let progress = failure.receipt.macro_synchronization.unwrap();
            assert!(progress.after.is_none());
            assert_eq!(progress.mutations.len(), 2);
            assert_eq!(
                progress.mutations[0].failure_phase,
                Some(MacroFailurePhase::InsufficientStorage)
            );
            let deletion = &progress.mutations[1];
            assert!(deletion.removed);
            assert_eq!(deletion.failure_phase, Some(phase));
            assert!(!deletion.progress.verified());
            assert_eq!(deletion.progress.steps.len(), usize::from(stop >= 29));
            if let Some(step) = deletion.progress.steps.first() {
                assert_eq!(step.command, [2, 6, 3]);
                assert_eq!(step.payload, [0, 5]);
                assert!(step.attempted);
                assert!(step.acknowledgment_received);
                assert!(step.acknowledgment_data.is_empty());
                assert_eq!(step.error.is_some(), stop == 29);
            }
            assert_eq!(
                progress.calculated_memory,
                Some(KeyboardMacroMemory::observe(progress.before.as_ref().unwrap(), 2).unwrap())
            );
            assert!(firmware.macros.borrow().contains_key(&0));
            assert_eq!(firmware.macros.borrow().contains_key(&5), stop < 29);
            assert!(!firmware.macros.borrow().contains_key(&9));
            assert_exchange_trace(&firmware, &probe, &commands[..stop], false);
        }
    }
}

#[test]
fn magnetic_macro_standby_ordinary_delete_failure_still_retries_the_add() {
    for product in [740, 746] {
        let firmware = Firmware::new(1536);
        firmware.saved(0, STANDBY);
        firmware.saved(5, STANDBY);
        *firmware.reject_once.borrow_mut() = Some([6, 3]);
        let probe = ExchangeProbe::new(&firmware);
        let failure = submit(
            &probe,
            product,
            ACTIVE,
            intent(context(true)),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
        assert!(!failure.receipt.completed);
        assert!(failure.receipt.submissions.is_empty());
        let progress = failure.receipt.macro_synchronization.unwrap();
        assert!(progress.after.is_none());
        assert_eq!(progress.mutations.len(), 3);
        assert_eq!(
            progress.mutations[0].failure_phase,
            Some(MacroFailurePhase::InsufficientStorage)
        );
        assert_eq!(
            progress.mutations[1].failure_phase,
            Some(MacroFailurePhase::Delete)
        );
        assert_eq!(
            progress.mutations[2].failure_phase,
            Some(MacroFailurePhase::InsufficientStorage)
        );
        let step = &progress.mutations[1].progress.steps[0];
        assert!(step.attempted);
        assert!(!step.acknowledgment_received);
        assert!(step.error.as_ref().unwrap().contains("status 5"));
        assert_eq!(
            firmware.macros.borrow().keys().copied().collect::<Vec<_>>(),
            [0, 5]
        );
        let mut commands = standby_delete_commands()[..29].to_vec();
        commands.extend([[6, 142], [6, 128], [6, 139]]);
        assert_exchange_trace(&firmware, &probe, &commands, false);
    }
}

#[test]
fn magnetic_macro_release_failure_keeps_its_cause_when_the_owner_is_invalidated() {
    for product in [740, 746] {
        for stop in [2, 26, 29, 30, 31] {
            let firmware = Firmware::new(1536);
            firmware.saved(0, STANDBY);
            firmware.saved(5, STANDBY);
            let invalidated = Cell::new(false);
            let finish_check = || {
                if firmware.received.get() == stop {
                    invalidated.set(true);
                    return Err(ReleaseFault { exchange: stop }.into());
                }
                Ok(())
            };
            let probe = ExchangeProbe::with_finish_check(&firmware, &finish_check);
            let failure = submit(
                &probe,
                product,
                ACTIVE,
                intent(context(true)),
                transactions(),
                || {
                    ensure!(!invalidated.get(), "owner invalidated after failed release");
                    Ok(())
                },
            )
            .unwrap_err();
            assert_release_fault(&failure.error, stop);
            assert!(!failure.receipt.completed);
            assert!(failure.receipt.submissions.is_empty());
            assert_exchange_trace(&firmware, &probe, &standby_delete_commands()[..stop], false);
        }
    }
}

#[test]
fn magnetic_macro_cancellation_after_mutation_ack_preserves_response_and_gives_up_the_lease() {
    for product in [740, 746] {
        for stop in 14..=19 {
            let firmware = Firmware::new(4096);
            let probe = ExchangeProbe::new(&firmware);
            let failure = submit(
                &probe,
                product,
                ACTIVE,
                intent(context(false)),
                transactions(),
                || {
                    ensure!(
                        firmware.received.get() < stop,
                        "cancelled after acknowledged mutation"
                    );
                    Ok(())
                },
            )
            .unwrap_err();
            assert!(!failure.receipt.completed);
            assert!(failure.receipt.submissions.is_empty());
            let progress = failure.receipt.macro_synchronization.unwrap();
            assert!(progress.after.is_none());
            assert_eq!(progress.mutations.len(), 1);
            let mutation = &progress.mutations[0];
            assert_eq!(mutation.failure_phase, Some(MacroFailurePhase::Cancelled));
            assert!(!mutation.progress.verified());
            assert_eq!(mutation.progress.steps.len(), stop - 13);
            let step = mutation.progress.steps.last().unwrap();
            assert!(step.attempted);
            assert!(step.acknowledgment_received);
            assert!(step.acknowledgment_data.is_empty());
            assert!(
                step.error
                    .as_ref()
                    .unwrap()
                    .contains("cancelled after acknowledged mutation")
            );
            assert_exchange_trace(&firmware, &probe, &UPLOAD_COMMANDS[..stop], true);
        }
    }
}

#[test]
fn magnetic_macro_standby_cancellation_keeps_the_delete_ack_and_never_retries_the_add() {
    for product in [740, 746] {
        let firmware = Firmware::new(1536);
        firmware.saved(0, STANDBY);
        firmware.saved(5, STANDBY);
        let probe = ExchangeProbe::new(&firmware);
        let failure = submit(
            &probe,
            product,
            ACTIVE,
            intent(context(true)),
            transactions(),
            || {
                ensure!(
                    firmware.received.get() < 29,
                    "cancelled after delete acknowledgement"
                );
                Ok(())
            },
        )
        .unwrap_err();
        assert!(!failure.receipt.completed);
        assert!(failure.receipt.submissions.is_empty());
        let progress = failure.receipt.macro_synchronization.unwrap();
        assert!(progress.after.is_none());
        assert_eq!(progress.mutations.len(), 2);
        let deletion = &progress.mutations[1];
        assert_eq!(deletion.failure_phase, Some(MacroFailurePhase::Cancelled));
        assert!(!deletion.progress.verified());
        assert_eq!(deletion.progress.steps.len(), 1);
        let step = &deletion.progress.steps[0];
        assert_eq!(step.command, [2, 6, 3]);
        assert!(step.attempted);
        assert!(step.acknowledgment_received);
        assert!(step.acknowledgment_data.is_empty());
        assert_eq!(
            firmware.macros.borrow().keys().copied().collect::<Vec<_>>(),
            [0]
        );
        assert_exchange_trace(&firmware, &probe, &standby_delete_commands()[..29], true);
    }
}

#[test]
fn magnetic_macro_upload_is_read_back_before_real_id_assignments() {
    let firmware = Firmware::new(4096);
    let receipt = submit(
        &firmware,
        740,
        ACTIVE,
        intent(context(false)),
        transactions(),
        || Ok(()),
    )
    .unwrap();
    assert!(receipt.completed);
    let progress = receipt.macro_synchronization.unwrap();
    assert_eq!(progress.mutations.len(), 1);
    assert!(progress.mutations[0].progress.verified());
    assert_eq!(progress.after.unwrap().inventory.ids, [9]);
    let writes = firmware.mutations();
    assert_eq!(writes[0], (6, 8, vec![0, 9, 0, 0, 0, 4]));
    assert_eq!(
        writes[writes.len() - 2..],
        [
            (2, 13, vec![3, 31, 0, 3, 3, 0, 9, 1, 0, 0]),
            (2, 13, vec![1, 31, 0, 3, 3, 0, 9, 1, 0, 0])
        ]
    );
    assert!(
        writes
            .iter()
            .all(|(class, id, _)| !(*class == 6 && *id == 10))
    );
    let packets = firmware.sent.borrow();
    let first_assignment = packets.iter().position(|p| p[7..9] == [2, 13]).unwrap();
    assert!(
        packets[..first_assignment]
            .iter()
            .any(|p| p[7..9] == [6, 137])
    );
}

#[test]
fn magnetic_macro_capacity_cleanup_evicts_standby_in_source_order_and_preserves_zero() {
    let firmware = Firmware::new(1536);
    firmware.saved(0, STANDBY);
    firmware.saved(5, STANDBY);
    let receipt = submit(
        &firmware,
        746,
        ACTIVE,
        intent(context(true)),
        transactions(),
        || Ok(()),
    )
    .unwrap();
    // Current Ec marks macros as standby only for local profiles absent from
    // the device inventory; Lt then excludes firmware ID zero from eviction.
    assert!(
        receipt
            .inventory
            .iter()
            .all(|profile| profile.guid != STANDBY_PROFILE)
    );
    let progress = receipt.macro_synchronization.unwrap();
    assert_eq!(
        progress.mutations[0].failure_phase,
        Some(MacroFailurePhase::InsufficientStorage)
    );
    assert_eq!(progress.mutations[1].progress.macro_id, 5);
    assert!(progress.mutations[1].removed);
    assert!(progress.mutations[2].progress.verified());
    assert_eq!(
        firmware.macros.borrow().keys().copied().collect::<Vec<_>>(),
        [0, 9]
    );
    let writes = firmware.mutations();
    assert_eq!(writes[0], (6, 3, vec![0, 5]));
    assert_eq!(writes[1], (6, 8, vec![0, 9, 0, 0, 0, 4]));
}

#[test]
fn magnetic_macro_locked_owner_assignments_protect_hardware_ids_while_orphans_are_removed() {
    let mut firmware = Firmware::new(4096);
    firmware.foreign_profile = true;
    firmware.saved(7, LOCKED);
    firmware.saved(8, ORPHAN);
    let receipt = submit(
        &firmware,
        740,
        ACTIVE,
        intent(context(false)),
        transactions(),
        || Ok(()),
    )
    .unwrap();
    let progress = receipt.macro_synchronization.unwrap();
    assert_eq!(progress.locked_ids, BTreeSet::from([7]));
    assert_eq!(
        firmware.macros.borrow().keys().copied().collect::<Vec<_>>(),
        [7, 9]
    );
    assert_eq!(firmware.mutations()[0], (6, 3, vec![0, 8]));
}

#[test]
fn magnetic_macro_cancellation_preserves_attempt_and_never_publishes_assignment() {
    let firmware = Firmware::new(4096);
    let failure = submit(
        &firmware,
        740,
        ACTIVE,
        intent(context(false)),
        transactions(),
        || {
            ensure!(
                !firmware.sent.borrow().iter().any(|p| p[7..9] == [6, 8]),
                "canceled after allocation send"
            );
            Ok(())
        },
    )
    .unwrap_err();
    assert!(!failure.receipt.completed);
    let progress = failure.receipt.macro_synchronization.unwrap();
    let mutation = &progress.mutations[0];
    assert_eq!(mutation.failure_phase, Some(MacroFailurePhase::Cancelled));
    assert!(mutation.progress.steps[0].attempted);
    assert!(!mutation.progress.steps[0].acknowledgment_received);
    assert!(
        firmware
            .mutations()
            .iter()
            .all(|(class, id, _)| !(*class == 2 && *id == 13))
    );
}

#[test]
fn magnetic_macro_failed_independent_readback_stops_all_key_assignments() {
    let mut firmware = Firmware::new(4096);
    firmware.corrupt_readback = true;
    let failure = submit(
        &firmware,
        746,
        ACTIVE,
        intent(context(false)),
        transactions(),
        || Ok(()),
    )
    .unwrap_err();
    assert!(!failure.receipt.completed);
    let progress = failure.receipt.macro_synchronization.unwrap();
    assert_eq!(
        progress.mutations[0].failure_phase,
        Some(MacroFailurePhase::Readback)
    );
    assert!(!progress.mutations[0].progress.verified());
    assert!(progress.mutations[0].progress.metadata_readback.is_some());
    assert!(
        firmware
            .mutations()
            .iter()
            .all(|(class, id, _)| !(*class == 2 && *id == 13))
    );
}

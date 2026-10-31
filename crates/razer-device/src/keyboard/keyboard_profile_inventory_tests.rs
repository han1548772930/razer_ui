use super::*;
use crate::keyboard_control_test_transport::{
    Mock, Reply, assert_report_integrity, assert_report_integrity_with_id, reply, sequence,
};

fn capability(product_id: u32) -> DeviceReadCapability {
    let mut cap = keyboard_metadata::capability(product_id).unwrap().clone();
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap.sleep_between_in_ms = 0;
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap
}

fn metadata(name: &str) -> [u8; 250] {
    let mut bytes = [0; 250];
    bytes[..16].copy_from_slice(&[
        0x71, 0xd3, 6, 0xaf, 0x1f, 0x86, 0x98, 0x4b, 0x8d, 0x78, 0xbf, 0xae, 0xf5, 0xcf, 0xde, 0xbf,
    ]);
    bytes[16..16 + name.len()].copy_from_slice(name.as_bytes());
    bytes[116..121].copy_from_slice(b"owner");
    bytes[244..250].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
    bytes
}

fn metadata_replies(slot: u8, bytes: &[u8; 250]) -> Vec<Reply> {
    (0u16..250)
        .step_by(64)
        .map(|offset| {
            let length = usize::from((250 - offset).min(64));
            let [high, low] = offset.to_be_bytes();
            let mut data = vec![slot, high, low, 0, 0];
            data.extend_from_slice(&bytes[usize::from(offset)..usize::from(offset) + length]);
            reply(&data)
        })
        .collect()
}

fn inventory_replies(maximum: u8, ids: &[u8]) -> Vec<Reply> {
    let mut list = vec![ids.len() as u8];
    list.extend_from_slice(ids);
    vec![reply(&[maximum]), reply(&[ids.len() as u8]), reply(&list)]
}

#[test]
fn one_shot_admission_failure_stops_metadata_restarts_for_every_obm_product() {
    use crate::feature_exchange::tests::ExchangeProbe;
    use std::cell::Cell;
    for product_id in OBM_PRODUCTS {
        let cap = capability(*product_id);
        let device = Mock::new([]);
        let admissions = Cell::new(0);
        let admit = || {
            let previous = admissions.replace(admissions.get() + 1);
            ensure!(previous != 0, "one-shot profile metadata admission failure");
            Ok(())
        };
        let probe = ExchangeProbe::with_admission_check(&device, &admit);
        let error =
            read_raw_metadata_hid(&probe, &cap, 7, &mut sequence(), &|| Ok(())).unwrap_err();
        assert!(crate::backend::exchange_interrupted(&error));
        assert_eq!(admissions.get(), 1, "PID{product_id}");
        assert!(device.sent().is_empty());
    }
}

#[test]
fn keyboard_profile_inventory_limits_reads_to_actual_hid_obm_mounts() {
    assert_eq!(OBM_PRODUCTS.len(), 42);
    for product in OBM_PRODUCTS {
        assert!(supports(*product));
        validate_capability(&capability(*product)).unwrap();
    }
    for product in [
        515, 542, 555, 574, 580, 592, 654, 659, 673, 7390, 757, 758, 2595,
    ] {
        assert!(!supports(product));
    }
    let device = Mock::new([]);
    assert!(read_hid(&device, &capability(659), sequence(), &|| Ok(())).is_err());
    let mut ble = capability(756);
    ble.source_class = "rzDevice30".into();
    assert!(read_hid(&device, &ble, sequence(), &|| Ok(())).is_err());
    assert!(device.sent().is_empty());
}

#[test]
fn keyboard_profile_inventory_756_partial_slots_decode_actual_bytes_without_capacity_inference() {
    use crate::keyboard_latest_restore::Slot;
    let first = metadata("Actual 756 first slot");
    let mut unknown = [0xff; 250];
    unknown[16..116].fill(0);
    unknown[16..23].copy_from_slice(b"unknown");
    let slots = [
        Slot {
            slot_id: 7,
            guid: "af06d371-861f-4b98-8d78-bfaef5cfdebf".into(),
        },
        Slot {
            slot_id: 2,
            guid: "ffffffff-ffff-ffff-ffff-ffffffffffff".into(),
        },
    ];
    let raw = [first.to_vec(), unknown.to_vec()];
    let observed = decode_756_slots(&slots, &raw).unwrap();
    assert_eq!(
        observed.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(),
        [7, 2]
    );
    let ProfileMetadataObservation::Observed {
        identity,
        mapping_hash,
    } = &observed[0].metadata
    else {
        panic!("first slot was discarded")
    };
    assert_eq!(identity.name, "Actual 756 first slot");
    assert_eq!(
        mapping_hash,
        &MappingHashObservation::Observed([1, 2, 3, 4, 5, 6])
    );
    let ProfileMetadataObservation::Observed { identity, .. } = &observed[1].metadata else {
        panic!("unknown GUID was discarded")
    };
    assert_eq!(identity.guid_status, GuidStatus::Unrecognized);
    assert!(decode_756_slots(&slots, &raw[..1]).is_err());
    let mut malformed = raw.clone();
    malformed[0].truncate(249);
    assert!(decode_756_slots(&slots, &malformed).is_err());
    let mut foreign = slots.clone();
    foreign[0].guid = foreign[1].guid.clone();
    assert!(decode_756_slots(&foreign, &raw).is_err());
    let mut repeated = slots.clone();
    repeated[1].slot_id = repeated[0].slot_id;
    assert!(decode_756_slots(&repeated, &raw).is_err());
}

#[test]
fn keyboard_profile_inventory_reads_actual_679_first_slot_in_source_order() {
    let mut replies = inventory_replies(6, &[1]);
    replies.extend(metadata_replies(1, &metadata("设备 当前值")));
    let device = Mock::new(replies);
    let cap = capability(679);
    let observed = read_hid(&device, &cap, sequence(), &|| Ok(())).unwrap();
    assert_eq!(observed.max_profiles_supported, 6);
    assert_eq!(observed.declared_profile_count, 1);
    assert_eq!(observed.slots[0].slot_id, 1);
    let ProfileMetadataObservation::Observed {
        identity,
        mapping_hash,
    } = &observed.slots[0].metadata
    else {
        panic!("Analog V3 first slot was not observed")
    };
    assert_eq!(identity.guid, "af06d371-861f-4b98-8d78-bfaef5cfdebf");
    assert_eq!(identity.guid_status, GuidStatus::Recognized);
    assert_eq!(identity.name, "设备 当前值");
    assert_eq!(identity.owner, "owner");
    assert_eq!(mapping_hash, &MappingHashObservation::Unobserved);
    let reports = device.sent();
    assert_eq!(reports.len(), 7);
    assert_eq!(&reports[0][6..9], &[1, 5, 138]);
    assert_eq!(&reports[1][6..9], &[1, 5, 128]);
    assert_eq!(&reports[2][6..9], &[80, 5, 129]);
    for (report, offset) in reports[3..].iter().zip([0, 64, 128, 192]) {
        assert_eq!(
            &report[6..9],
            &[if offset == 192 { 63 } else { 69 }, 5, 136]
        );
        assert_eq!(&report[9..14], &[1, 0, offset, 0, 0]);
    }
    assert_eq!(cap.sleep_between_out_in_ms, 0);
    assert_report_integrity(&reports);
}

#[test]
fn keyboard_profile_inventory_skips_first_list_entry_without_inventing_its_identity() {
    let mut replies = inventory_replies(5, &[4, 1]);
    replies.extend(metadata_replies(1, &metadata("实际槽位一")));
    let device = Mock::new(replies);
    let observed = read_hid(&device, &capability(550), sequence(), &|| Ok(())).unwrap();
    assert_eq!(observed.slots[0].slot_id, 4);
    assert_eq!(
        observed.slots[0].metadata,
        ProfileMetadataObservation::SourceSkippedFirstSlot
    );
    assert_eq!(observed.slots[1].slot_id, 1);
    let ProfileMetadataObservation::Observed { identity, .. } = &observed.slots[1].metadata else {
        panic!("Second returned slot was not read")
    };
    assert_eq!(identity.name, "实际槽位一");
    assert!(device.sent()[3..].iter().all(|report| report[9] == 1));
}

#[test]
fn keyboard_profile_inventory_reads_hash_only_for_the_current_metadata_schema() {
    for product in [747, 756] {
        let mut replies = inventory_replies(6, &[2]);
        replies.extend(metadata_replies(2, &metadata("设备命名")));
        let device = Mock::new(replies);
        let observed = read_hid(&device, &capability(product), sequence(), &|| Ok(())).unwrap();
        assert!(matches!(
            &observed.slots[0].metadata,
            ProfileMetadataObservation::Observed {
                mapping_hash: MappingHashObservation::Observed([1, 2, 3, 4, 5, 6]),
                ..
            }
        ));
    }
}

#[test]
fn keyboard_profile_inventory_radio_preserves_the_physical_factory_report_and_transaction() {
    let mut cap = keyboard_metadata::hid_capability(756, 757).unwrap().clone();
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap.sleep_between_in_ms = 0;
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    let mut replies = inventory_replies(6, &[2]);
    replies.extend(metadata_replies(2, &metadata("无线设备值")));
    let device = Mock::new(replies);
    let mut counter = sequence();
    let observed = read_hid(
        &device,
        &cap,
        || counter().map(|id| cap.transaction_prefix | id),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(observed.slots[0].slot_id, 2);
    let reports = device.sent();
    assert_eq!(cap.source_class, "rzDevice25DualLinkKeyboard");
    assert_eq!(cap.report_id, 3);
    assert_eq!(cap.transaction_prefix, 0x80);
    for (index, report) in reports.iter().enumerate() {
        assert_eq!(report[2], 0x80 | index as u8);
    }
    assert_report_integrity_with_id(&reports, 3);
}

#[test]
fn keyboard_profile_inventory_latest_restore_uses_the_same_actual_metadata_reader() {
    use crate::keyboard_latest_protocol::{Operation, Reply as LatestReply, execute_hid};
    let cap = capability(756);
    let mut replies = vec![reply(&[1, 2])];
    replies.extend(metadata_replies(2, &metadata("设备恢复清单")));
    let device = Mock::new(replies);
    let observed =
        execute_hid(&device, &cap, &Operation::Inventory, sequence(), &|| Ok(())).unwrap();
    let LatestReply::Inventory { slots, metadata } = observed else {
        panic!("Latest restore did not retain actual inventory")
    };
    assert_eq!(slots[0].slot_id, 2);
    assert_eq!(slots[0].guid, "af06d371-861f-4b98-8d78-bfaef5cfdebf");
    assert_eq!(decode_identity(&metadata[0]).unwrap().name, "设备恢复清单");
    assert_eq!(device.sent().len(), 5);
}

#[test]
fn keyboard_profile_inventory_preserves_unknown_final_guid_after_ten_source_attempts() {
    let mut unknown = metadata("未知设备标识");
    unknown[..16].fill(0xff);
    let mut replies = inventory_replies(6, &[3]);
    for _ in 0..10 {
        replies.extend(metadata_replies(3, &unknown));
    }
    let device = Mock::new(replies);
    let observed = read_hid(&device, &capability(679), sequence(), &|| Ok(())).unwrap();
    let ProfileMetadataObservation::Observed { identity, .. } = &observed.slots[0].metadata else {
        panic!("Actual unknown metadata disappeared")
    };
    assert_eq!(identity.guid, "ffffffff-ffff-ffff-ffff-ffffffffffff");
    assert_eq!(identity.guid_status, GuidStatus::Unrecognized);
    assert_eq!(identity.name, "未知设备标识");
    assert_eq!(device.sent().len(), 43);
}

#[test]
fn keyboard_profile_inventory_accepts_only_a_real_empty_inventory() {
    let device = Mock::new(inventory_replies(0, &[]));
    let observed = read_hid(&device, &capability(679), sequence(), &|| Ok(())).unwrap();
    assert_eq!(observed.max_profiles_supported, 0);
    assert_eq!(observed.declared_profile_count, 0);
    assert!(observed.slots.is_empty());
    assert_eq!(device.sent().len(), 3);
    let device = Mock::new([reply(&[])]);
    assert!(read_hid(&device, &capability(679), sequence(), &|| Ok(())).is_err());
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn keyboard_profile_inventory_rejects_truncated_conflicting_and_repeated_slot_lists() {
    for (count, list) in [
        (2, vec![2, 1]),
        (1, vec![2, 1, 2]),
        (2, vec![2, 1, 1]),
        (1, vec![1, 0]),
    ] {
        let device = Mock::new([reply(&[6]), reply(&[count]), reply(&list)]);
        assert!(read_hid(&device, &capability(679), sequence(), &|| Ok(())).is_err());
        assert_eq!(device.sent().len(), 3);
    }
}

#[test]
fn keyboard_profile_inventory_rejects_foreign_or_incomplete_metadata_without_defaulting() {
    for bad in [reply(&[2, 0, 0, 0, 0]), reply(&[1, 0, 0, 0, 0])] {
        let mut replies = inventory_replies(6, &[1]);
        replies.push(bad);
        let device = Mock::new(replies);
        assert!(read_hid(&device, &capability(679), sequence(), &|| Ok(())).is_err());
        assert_eq!(device.sent().len(), 4);
    }
}

#[test]
fn keyboard_profile_inventory_transport_failure_keeps_read_failed_after_bounded_retry() {
    let mut replies = inventory_replies(6, &[1]);
    for _ in 0..10 {
        let mut failed = reply(&[]);
        failed.status = 5;
        replies.push(failed);
    }
    let device = Mock::new(replies);
    assert!(read_hid(&device, &capability(679), sequence(), &|| Ok(())).is_err());
    assert_eq!(device.sent().len(), 13);
}

#[test]
fn keyboard_profile_inventory_cancellation_stops_before_another_chunk_or_retry() {
    let mut replies = inventory_replies(6, &[1]);
    replies.extend(metadata_replies(1, &metadata("真实设备")));
    let device = Mock::new(replies);
    let observed = read_hid(&device, &capability(679), sequence(), &|| {
        ensure!(device.sent().len() < 4, "simulated connection replaced");
        Ok(())
    });
    assert!(observed.is_err());
    assert_eq!(device.sent().len(), 4);
}

#[test]
fn keyboard_profile_inventory_active_read_preserves_raw_id_without_selecting_a_profile() {
    for slot_id in [0, 7, 255] {
        let device = Mock::new([reply(&[slot_id])]);
        assert_eq!(
            read_active_hid(&device, &capability(679), sequence(), &|| Ok(())).unwrap(),
            ActiveProfileReading { slot_id }
        );
        assert_eq!(&device.sent()[0][6..9], &[1, 5, 132]);
    }
    let device = Mock::new([reply(&[])]);
    assert!(read_active_hid(&device, &capability(679), sequence(), &|| Ok(())).is_err());
}

#[test]
fn keyboard_profile_inventory_shared_decoder_keeps_source_utf8_and_owner_boundaries() {
    let mut bytes = metadata("设备名称");
    bytes[16..116].fill(b'X');
    bytes[115] = b'Y';
    bytes[116..180].fill(0);
    bytes[117] = 0xe9;
    let identity = decode_identity(&bytes).unwrap();
    assert_eq!(identity.name, "X".repeat(99));
    assert_eq!(identity.owner, "é");
    let latest = crate::keyboard_latest_protocol::profile_metadata(2, &bytes).unwrap();
    assert_eq!(latest.guid, identity.guid);
    assert_eq!(latest.name, identity.name);
    assert_eq!(latest.owner, identity.owner);
    assert!(decode_identity(&bytes[..249]).is_err());
}

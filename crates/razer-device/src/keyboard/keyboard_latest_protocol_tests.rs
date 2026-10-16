use super::*;
use crate::keyboard_control_test_transport::{Mock, assert_report_integrity_with_id, reply};
// Current rzDevice25DualLinkKeyboard._getTransactionId returns 128 | counter,
// with counter cycling through 0..30 (756 MW, adapter PID757).
fn sequence() -> impl FnMut() -> anyhow::Result<u8> {
    let mut sequence = crate::keyboard_control_test_transport::sequence();
    move || sequence().map(|counter| 0x80 | counter)
}
fn cap() -> DeviceReadCapability {
    let mut cap = capability(757).unwrap().for_physical_product(757).unwrap();
    cap.max_retry_out = 1;
    cap.max_retry_in = 1;
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap
}
#[test]
fn ordinary_v3_assignment_keeps_source_packet_size_override() {
    let device = Mock::new([reply(&[])]);
    let receipt = execute_hid(
        &device,
        &cap(),
        &Operation::Setting(Setting::ButtonAssignment {
            slot: 4,
            key_id: 31,
            mode: 1,
            fn_id: 2,
            data_size: 2,
            data: [8, 4, 0, 0, 0],
        }),
        sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(receipt.acknowledged());
    let sent = device.sent();
    assert_eq!(sent[0][2], 0x80);
    assert_eq!(&sent[0][6..9], &[10, 2, 13]);
    assert_eq!(&sent[0][9..19], &[4, 31, 1, 2, 2, 8, 4, 0, 0, 0]);
    assert!(sent[0][19..89].iter().all(|byte| *byte == 0));
    assert_report_integrity_with_id(&sent, 3);
}
#[test]
fn multi_actuation_continues_after_a_chunk_error_without_claiming_full_success() {
    let mut rejected = reply(&[]);
    rejected.status = 5;
    let device = Mock::new([rejected, reply(&[])]);
    let points = (1..=16)
        .map(|key_id| ActuationPoint {
            key_id,
            threshold_high: 256,
            threshold_low: 19656,
        })
        .collect();
    let report = execute_hid(
        &device,
        &cap(),
        &Operation::Actuation { slot: 6, points },
        sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(!report.acknowledged());
    let Reply::Write { chunks } = report else { panic!() };
    assert_eq!(chunks.len(), 2);
    assert!(chunks[0].error.is_some());
    assert!(chunks[1].acknowledgement.is_some());
    let sent = device.sent();
    assert_eq!(&sent[0][6..11], &[77, 2, 25, 0, 6]);
    assert_eq!(&sent[1][6..11], &[7, 2, 25, 0, 6]);
    assert_report_integrity_with_id(&sent, 3);
}
#[test]
fn getter_uses_fixed_key_records_and_rejects_duplicate_observations() {
    let mut data = vec![0, 1];
    data.extend_from_slice(&[31, 1, 0, 0x4c, 0xc8, 32, 1, 0, 0x4c, 0xc8]);
    let device = Mock::new([reply(&data)]);
    let report = execute_hid(
        &device,
        &cap(),
        &Operation::ReadActuation {
            slot: 1,
            keys: vec![31, 32],
        },
        sequence(),
        &|| Ok(()),
    )
    .unwrap();
    let Reply::Actuation { points } = report else {
        panic!()
    };
    assert_eq!(points.len(), 2);
    assert_eq!(
        &device.sent()[0][9..21],
        &[0, 1, 31, 0, 0, 0, 0, 32, 0, 0, 0, 0]
    );
    data[7] = 31;
    let device = Mock::new([reply(&data)]);
    assert!(
        execute_hid(
            &device,
            &cap(),
            &Operation::ReadActuation {
                slot: 1,
                keys: vec![31, 32]
            },
            sequence(),
            &|| Ok(())
        )
        .is_err()
    );
}
#[test]
fn an_empty_real_inventory_does_not_fabricate_presets_or_slots() {
    let device = Mock::new([reply(&[0])]);
    let report = execute_hid(&device, &cap(), &Operation::Inventory, sequence(), &|| {
        Ok(())
    })
    .unwrap();
    assert!(
        matches!(report,Reply::Inventory{slots,metadata} if slots.is_empty()&&metadata.is_empty())
    );
    assert!(capability(758).is_err());
    assert!(capability(692).is_err());
}

#[test]
fn profile_metadata_uses_utf8_name_owner_and_full_hash_offsets() {
    let mut bytes = [0u8; 250];
    bytes[..16].copy_from_slice(&[
        0x71, 0xd3, 6, 0xaf, 0x1f, 0x86, 0x98, 0x4b, 0x8d, 0x78, 0xbf, 0xae, 0xf5, 0xcf, 0xde, 0xbf,
    ]);
    let name = "本地 Profile".as_bytes();
    bytes[16..16 + name.len()].copy_from_slice(name);
    bytes[116..121].copy_from_slice(b"owner");
    bytes[244..250].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
    let parsed = profile_metadata(6, &bytes).unwrap();
    assert_eq!(parsed.guid, "af06d371-861f-4b98-8d78-bfaef5cfdebf");
    assert_eq!(parsed.name, "本地 Profile");
    assert_eq!(parsed.owner, "owner");
    assert_eq!(parsed.key_mapping_hash, [1, 2, 3, 4, 5, 6]);
    bytes[16..116].fill(b'X');
    assert_eq!(profile_metadata(6, &bytes).unwrap().name.len(), 99);
    assert!(profile_metadata(6, &bytes[..249]).is_err());
}

#[test]
fn navigation_metadata_preserves_mapping_hash_and_stops_after_failed_chunk() {
    let metadata = ProfileMetadata {
        slot_id: 1,
        guid: "af06d371-861f-4b98-8d78-bfaef5cfdebf".into(),
        name: String::new(),
        owner: String::new(),
        key_mapping_hash: [1, 2, 3, 4, 5, 6],
    };
    let bytes = profile_metadata_bytes(&metadata).unwrap();
    assert_eq!(profile_metadata(1, &bytes).unwrap(), metadata);
    let device = Mock::new((0..5).map(|_| reply(&[])));
    let receipt = execute_hid(
        &device,
        &cap(),
        &Operation::WriteProfileMetadata(metadata.clone()),
        sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(receipt.acknowledged());
    let sent = device.sent();
    assert_eq!(&sent[0][6..10], &[1, 5, 2, 1]);
    assert_eq!(sent.len(), 5);
    assert_eq!(
        sent.iter().map(|packet| packet[2]).collect::<Vec<_>>(),
        [128, 129, 130, 131, 132]
    );
    let mut written = Vec::new();
    for (index, packet) in sent[1..].iter().enumerate() {
        let offset = (index * 64) as u16;
        let count = (250 - usize::from(offset)).min(64);
        assert_eq!(&packet[6..9], &[(count + 5) as u8, 5, 8]);
        assert_eq!(
            &packet[9..14],
            &[1, offset.to_be_bytes()[0], offset.to_be_bytes()[1], 0, 250]
        );
        written.extend_from_slice(&packet[14..14 + count]);
    }
    assert_eq!(written, bytes);
    assert_report_integrity_with_id(&sent, 3);
    let mut failed = reply(&[]);
    failed.status = 5;
    let device = Mock::new([reply(&[]), reply(&[]), failed, reply(&[])]);
    let receipt = execute_hid(
        &device,
        &cap(),
        &Operation::WriteProfileMetadata(metadata),
        sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(!receipt.acknowledged());
    assert_eq!(device.sent().len(), 3);
    let Reply::Write { chunks } = receipt else { panic!() };
    assert_eq!(chunks.last().unwrap().offset, 64);
    assert!(chunks.last().unwrap().error.is_some());
}

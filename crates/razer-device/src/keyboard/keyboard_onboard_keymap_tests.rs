use super::*;
use crate::keyboard_control_test_transport::{Mock, assert_report_integrity, reply, sequence};
use std::cell::Cell;

fn capability() -> DeviceReadCapability {
    let mut cap = crate::keyboard_metadata::capability(747).unwrap().clone();
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap
}

fn address() -> Address {
    Address::new(2, 3).unwrap()
}

#[test]
fn keymap_reads_preserve_short_inventory_and_raw_active_target_values() {
    let device = Mock::new([reply(&[2, 3, 1, 4]), reply(&[2, 0]), reply(&[7])]);
    let inventory = read_inventory(&device, &capability(), 2, 0, &|| Ok(())).unwrap();
    assert_eq!(inventory.profile_id(), 2);
    assert_eq!(inventory.total_enabled(), 3);
    assert_eq!(inventory.keymap_ids(), [1, 4]);
    assert_eq!(
        read_active(&device, &capability(), 2, 1, &|| Ok(())).unwrap(),
        0
    );
    assert_eq!(
        read_target(&device, &capability(), 2, &|| Ok(())).unwrap(),
        7
    );
    let sent = device.sent();
    assert_eq!(&sent[0][6..10], &[1, 2, 178, 2]);
    assert_eq!(&sent[1][6..11], &[2, 2, 182, 2, 0]);
    assert_eq!(&sent[2][6..9], &[1, 2, 183]);
    assert_report_integrity(&sent);
}

#[test]
fn keymap_updates_keep_requested_ids_separate_from_acknowledgement_data() {
    let device = Mock::new([reply(&[99]), reply(&[]), reply(&[44])]);
    for (tx, operation, command, expected) in [
        (0, Update::Add, 51, vec![99]),
        (1, Update::Remove, 52, vec![]),
        (2, Update::Activate, 54, vec![44]),
    ] {
        assert_eq!(
            update(&device, &capability(), address(), operation, tx, &|| Ok(())).unwrap(),
            expected
        );
        assert_eq!(
            &device.sent().last().unwrap()[6..11],
            &[2, 2, command, 2, 3]
        );
    }
    assert_report_integrity(&device.sent());
}

#[test]
fn keymap_name_reads_real_offsets_and_retries_from_zero_after_failure() {
    let mut bytes = [0; 120];
    bytes[..16].copy_from_slice(&[
        0x44, 0x33, 0x22, 0x11, 0x66, 0x55, 0x88, 0x77, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0,
    ]);
    bytes[16..24].copy_from_slice(&[0xef, 0xbb, 0xbf, b'B', b'o', b'a', b'r', b'd']);
    let mut first = vec![2, 3, 0, 120];
    first.extend_from_slice(&bytes[..64]);
    let mut last = vec![2, 3, 64, 120];
    last.extend_from_slice(&bytes[64..]);
    let mut failed = reply(&[]);
    failed.status = 5;
    let device = Mock::new([failed, reply(&first), reply(&last)]);
    let name = read_name(&device, &capability(), address(), sequence(), &|| Ok(())).unwrap();
    assert_eq!(name.guid(), "11223344-5566-7788-99aa-bbccddeeff00");
    assert_eq!(name.name(), "Board");
    assert_eq!(name.raw(), bytes);
    let sent = device.sent();
    assert_eq!(sent.len(), 3);
    assert_eq!(&sent[0][6..13], &[68, 2, 181, 2, 3, 0, 120]);
    assert_eq!(&sent[1][6..13], &[68, 2, 181, 2, 3, 0, 120]);
    assert_eq!(&sent[2][6..13], &[60, 2, 181, 2, 3, 64, 120]);
}

#[test]
fn keymap_name_encoding_keeps_wire_guid_and_the_source_utf8_byte_limit() {
    let bytes = name_bytes("11223344-5566-7788-99aa-bbccddeeff00", "Board").unwrap();
    assert_eq!(
        &bytes[..22],
        &[
            0x44, 0x33, 0x22, 0x11, 0x66, 0x55, 0x88, 0x77, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
            0xff, 0, b'B', b'o', b'a', b'r', b'd', 0
        ]
    );
    let name = format!("{}é", "a".repeat(98));
    let bytes = name_bytes("11223344-5566-7788-99aa-bbccddeeff00", &name).unwrap();
    assert_eq!(bytes[114], 0xc3);
    assert_eq!(&bytes[115..], &[0; 5]);
    let preset = name_bytes("013dfcf3-ca58-4516-9300-4709300", "Keymap 1").unwrap();
    assert_eq!(
        &preset[..16],
        &[
            0xf3, 0xfc, 0x3d, 0x01, 0x58, 0xca, 0x16, 0x45, 0x93, 0x00, 0x47, 0x09, 0x30, 0, 0, 0
        ]
    );
    let preset = name_bytes("fee0d4f3-2130-445f-0000-00074", "Keymap 3").unwrap();
    assert_eq!(
        &preset[..16],
        &[
            0xf3, 0xd4, 0xe0, 0xfe, 0x30, 0x21, 0x5f, 0x44, 0, 0, 0, 0x07, 0x40, 0, 0, 0
        ]
    );
    for invalid in ["", "not-a-guid", "112233445566778899aabbccddeeff001"] {
        assert!(name_bytes(invalid, "Keymap").is_err());
    }
}

#[test]
fn empty_keymap_name_observation_survives_later_failed_source_attempts() {
    let bytes = name_bytes("11223344-5566-7788-99aa-bbccddeeff00", "").unwrap();
    let mut first = vec![2, 3, 0, 120];
    first.extend_from_slice(&bytes[..64]);
    let mut last = vec![2, 3, 64, 120];
    last.extend_from_slice(&bytes[64..]);
    let mut responses = vec![reply(&first), reply(&last)];
    for _ in 1..10 {
        let mut failure = reply(&[]);
        failure.status = 5;
        responses.push(failure);
    }
    let device = Mock::new(responses);
    let name = read_name(&device, &capability(), address(), sequence(), &|| Ok(())).unwrap();
    assert_eq!(name.guid(), "11223344-5566-7788-99aa-bbccddeeff00");
    assert_eq!(name.name(), "");
    assert_eq!(name.raw(), bytes);
    assert_eq!(device.sent().len(), 11);
}

#[test]
fn keymap_name_write_uses_two_exact_chunks_and_preserves_partial_cancel_ack() {
    let bytes = std::array::from_fn(|ix| ix as u8);
    let device = Mock::new([reply(&[1]), reply(&[2])]);
    let mut acknowledgements = Vec::new();
    write_name(
        &device,
        &capability(),
        address(),
        &bytes,
        sequence(),
        &|| Ok(()),
        &mut |step| acknowledgements.push(step),
    )
    .unwrap();
    assert_eq!(
        acknowledgements,
        [
            NameWriteProgress::Attempted {
                offset: 0,
                length: 64
            },
            NameWriteProgress::Acknowledged {
                offset: 0,
                data: vec![1]
            },
            NameWriteProgress::Attempted {
                offset: 64,
                length: 56
            },
            NameWriteProgress::Acknowledged {
                offset: 64,
                data: vec![2]
            },
        ]
    );
    let sent = device.sent();
    assert_eq!(&sent[0][6..13], &[68, 2, 53, 2, 3, 0, 120]);
    assert_eq!(&sent[0][13..77], &(0..64).collect::<Vec<_>>());
    assert_eq!(&sent[1][6..13], &[60, 2, 53, 2, 3, 64, 120]);
    assert_eq!(&sent[1][13..69], &(64..120).collect::<Vec<_>>());
    assert_report_integrity(&sent);

    let device = Mock::new([reply(&[9])]).interrupt_next_read();
    let mut acknowledged = Vec::new();
    let error = write_name(
        &device,
        &capability(),
        address(),
        &bytes,
        sequence(),
        &|| Ok(()),
        &mut |step| acknowledged.push(step),
    )
    .unwrap_err();
    assert!(crate::backend::exchange_interrupted(&error));
    assert_eq!(
        acknowledged,
        [
            NameWriteProgress::Attempted {
                offset: 0,
                length: 64
            },
            NameWriteProgress::Acknowledged {
                offset: 0,
                data: vec![9]
            },
        ]
    );
    assert_eq!(device.sent().len(), 1);
}

#[test]
fn keymap_address_and_one_shot_admission_fail_before_any_transfer() {
    assert!(serde_json::from_str::<Address>(r#"{"profile_id":0,"keymap_id":1}"#).is_err());
    let checks = Cell::new(0);
    let guard = || {
        let count = checks.replace(checks.get() + 1);
        ensure!(count != 0, "one-shot connection change");
        Ok(())
    };
    let device = Mock::new([]);
    let error = read_name(&device, &capability(), address(), sequence(), &guard).unwrap_err();
    assert!(crate::backend::exchange_interrupted(&error));
    assert!(device.sent().is_empty());
    assert_eq!(checks.get(), 1);
}

#[test]
fn keymap_name_failed_sends_keep_attempts_without_fabricating_acknowledgements() {
    let device = Mock::new([]).fail_sends(10);
    let mut progress = Vec::new();
    assert!(
        write_name(
            &device,
            &capability(),
            address(),
            &[0; 120],
            sequence(),
            &|| Ok(()),
            &mut |step| progress.push(step),
        )
        .is_err()
    );
    assert_eq!(device.sent().len(), 10);
    assert_eq!(
        progress,
        vec![
            NameWriteProgress::Attempted {
                offset: 0,
                length: 64
            };
            10
        ]
    );
}

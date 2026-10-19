use super::*;
use crate::keyboard_control_test_transport::{
    Mock, Reply, assert_report_integrity, reply, sequence,
};
use crate::onboard_macro::{MacroMetadata, MacroSummary};

fn capability() -> DeviceReadCapability {
    let mut cap = crate::keyboard_metadata::capability(PRODUCT_ID)
        .unwrap()
        .clone();
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap.sleep_between_in_ms = 0;
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap
}

fn keys(ids: &[u8]) -> Vec<KeyIdentity> {
    ids.iter()
        .map(|id| KeyIdentity {
            input_id: format!("source-key-{id}"),
            firmware_key_id: *id,
            ignore_normal: false,
            ignore_hypershift: false,
        })
        .collect()
}

fn initial_replies(ids: &[u8]) -> Vec<Reply> {
    let mut list = vec![ids.len() as u8];
    list.extend_from_slice(ids);
    let mut replies = vec![
        reply(&[0x7f, 0xff, 0, 1, 0, 0, 0, 0, 0xf0, 0, 0, 0, 0, 7]),
        reply(&[0, 0]),
        reply(&[6]),
        reply(&[ids.len() as u8]),
        reply(&list),
    ];
    for slot in ids {
        let mut bytes = [0; 250];
        bytes[..16].copy_from_slice(&[
            0x71, 0xd3, 6, 0xaf, 0x1f, 0x86, 0x98, 0x4b, 0x8d, 0x78, 0xbf, 0xae, 0xf5, 0xcf, 0xde,
            0xbf,
        ]);
        bytes[16..21].copy_from_slice(b"owned");
        bytes[116..121].copy_from_slice(b"owner");
        for offset in (0usize..250).step_by(64) {
            let size = (250 - offset).min(64);
            let mut data = vec![*slot, 0, offset as u8, 0, 0];
            data.extend_from_slice(&bytes[offset..offset + size]);
            replies.push(reply(&data));
        }
    }
    replies
}

fn assignments(slot: u8, mode: Mode, ids: &[u8], function: u8, first_data: u8) -> Reply {
    let mut bytes = vec![slot];
    for id in ids {
        bytes.extend_from_slice(&[*id, mode.wire(), function, 2, first_data, *id, 0, 0, 0]);
    }
    reply(&bytes)
}

fn rapid(slot: u8, ids: &[u8], enabled: u8, sensitivity: u16) -> Reply {
    let mut bytes = vec![0, slot];
    for id in ids {
        bytes.extend_from_slice(&[*id, enabled]);
        bytes.extend_from_slice(&sensitivity.to_be_bytes());
        bytes.extend_from_slice(&(sensitivity + 1).to_be_bytes());
    }
    reply(&bytes)
}

fn actuation(slot: u8, ids: &[u8], high: u16, low: u16) -> Reply {
    let mut bytes = vec![0, slot];
    for id in ids {
        bytes.push(*id);
        bytes.extend_from_slice(&high.to_be_bytes());
        bytes.extend_from_slice(&low.to_be_bytes());
    }
    reply(&bytes)
}

fn mode_replies(slot: u8, mode: Mode, ids: &[u8], threshold: u16) -> Vec<Reply> {
    let mut replies = Vec::new();
    for batch in ids.chunks(8) {
        replies.push(assignments(slot, mode, batch, 2, 0x11));
    }
    for batch in ids.chunks(13) {
        replies.push(rapid(slot, batch, 9, threshold));
    }
    for batch in ids.chunks(15) {
        replies.push(actuation(slot, batch, threshold + 1, threshold + 2));
    }
    replies
}

fn brightness(slot: u8, raw: u8) -> [Reply; 2] {
    [reply(&[7, 1, 2, 3, 4]), reply(&[slot, 7, raw])]
}

fn read_small(device: &Mock, ids: &[u8]) -> Result<Snapshot> {
    let cap = capability();
    let settings = settings_capability(&cap)?;
    read_selected(
        device,
        &cap,
        &settings,
        1,
        keys(ids),
        &mut sequence(),
        &|| Ok(()),
    )
}

#[test]
fn keyboard_onboard_profiles_reads_source_order_and_actual_unknown_flags() {
    let mut replies = initial_replies(&[2, 5]);
    replies.extend(brightness(2, 51));
    replies.extend(brightness(5, 153));
    for slot in [2, 5] {
        replies.extend(mode_replies(slot, Mode::Normal, &[4, 8], 1000));
        replies.extend(mode_replies(slot, Mode::Hypershift, &[4, 8], 2000));
    }
    replies.extend([reply(&[2, 7, 4, 8]), reply(&[5, 8, 8, 4])]);
    let device = Mock::new(replies);
    let observed = read_small(&device, &[4, 8]).unwrap();
    assert_eq!(observed.macros.inventory.storage.available_memory, 61440);
    assert_eq!(
        observed
            .inventory
            .slots
            .iter()
            .map(|slot| slot.slot_id)
            .collect::<Vec<_>>(),
        [2, 5]
    );
    assert_eq!(
        observed.profiles[0]
            .brightness
            .observed()
            .unwrap()
            .raw_value,
        51
    );
    assert_eq!(
        observed.profiles[1].brightness.observed().unwrap().percent,
        60
    );
    assert_eq!(
        observed.profiles[0].actuation(4).unwrap().threshold_low,
        2002
    );
    assert_eq!(observed.profiles[0].rapid_trigger(8).unwrap().enabled, 9);
    assert_eq!(
        observed.profiles[0]
            .kill_switch
            .observed()
            .unwrap()
            .raw_enabled,
        7
    );
    let reports = device.sent();
    let commands = reports
        .iter()
        .map(|report| (report[7], report[8]))
        .collect::<Vec<_>>();
    let mut expected = vec![(6, 142), (6, 128), (5, 138), (5, 128), (5, 129)];
    expected.extend([(5, 136); 8]);
    expected.extend([(15, 128), (15, 132), (15, 128), (15, 132)]);
    expected.extend([(2, 158), (2, 154), (2, 153)].repeat(4));
    expected.extend([(2, 161), (2, 161)]);
    assert_eq!(commands, expected);
    assert_eq!(&reports[14][9..11], &[2, 7]);
    assert_eq!(&reports[16][9..11], &[5, 7]);
    assert_report_integrity(&reports);
}

#[test]
fn keyboard_onboard_profiles_keeps_source_batch_sizes_and_modes() {
    let ids: Vec<_> = (1..=34).collect();
    let device = Mock::new(mode_replies(3, Mode::Hypershift, &ids, 900));
    let cap = capability();
    let settings = settings_capability(&cap).unwrap();
    let reading = read_mode(
        &device,
        &settings,
        3,
        Mode::Hypershift,
        &ids,
        &mut sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(reading.assignments.len(), 5);
    assert_eq!(reading.rapid_trigger.len(), 3);
    assert_eq!(reading.actuation.len(), 3);
    let reports = device.sent();
    assert_eq!(
        reports.iter().map(|report| report[6]).collect::<Vec<_>>(),
        [73, 73, 73, 73, 19, 80, 80, 50, 77, 77, 22]
    );
    assert_eq!(&reports[0][6..14], &[73, 2, 158, 3, 1, 1, 0, 0]);
    assert_eq!(&reports[5][6..12], &[80, 2, 154, 0, 3, 1]);
    assert_eq!(&reports[8][6..12], &[77, 2, 153, 0, 3, 1]);
    assert_report_integrity(&reports);
}

#[test]
fn keyboard_onboard_profiles_assignment_retries_zero_twice_and_keeps_unknown_functions() {
    let zero = reply(&[3, 4, 0, 255, 0, 0, 0, 0, 0, 0]);
    let mut bad = zero.clone();
    bad.status = 3;
    for second in [zero.clone(), bad] {
        let device = Mock::new([
            zero.clone(),
            second,
            rapid(3, &[4], 0, 420),
            actuation(3, &[4], 123, 456),
        ]);
        let settings = settings_capability(&capability()).unwrap();
        let pass = read_mode(
            &device,
            &settings,
            3,
            Mode::Normal,
            &[4],
            &mut sequence(),
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(pass.assignments[0].attempts.len(), 2);
        assert_eq!(pass.assignments[0].observed().unwrap()[0].function_id, 255);
        assert_eq!(pass.assignments[0].observed().unwrap()[0].data, [0; 5]);
        assert_eq!(device.sent().len(), 4);
        assert!(matches!(
            pass.rapid_trigger[0].reading,
            Reading::Observed(_)
        ));
    }
}

#[test]
fn keyboard_onboard_profiles_rejects_malformed_records_without_synthetic_success() {
    let cap = capability();
    let invalid = [
        vec![5, 4, 0, 2, 1, 9, 0, 0, 0, 0],
        vec![3, 4, 1, 2, 1, 9, 0, 0, 0, 0],
        vec![3, 5, 0, 2, 1, 9, 0, 0, 0, 0],
        vec![3, 4, 0, 2, 6, 9, 0, 0, 0, 0],
        vec![3, 4, 0, 2, 1, 9, 0, 0, 0],
    ];
    for bytes in invalid {
        let device = Mock::new([reply(&bytes)]);
        assert!(
            read_assignments(
                &device,
                &cap,
                3,
                Mode::Normal,
                &[4],
                &mut sequence(),
                &|| Ok(())
            )
            .is_err()
        );
        assert_eq!(device.sent().len(), 1);
    }
    let device = Mock::new([assignments(3, Mode::Normal, &[4, 4], 2, 1)]);
    assert!(
        read_assignments(
            &device,
            &cap,
            3,
            Mode::Normal,
            &[4, 8],
            &mut sequence(),
            &|| Ok(())
        )
        .is_err()
    );
}

#[test]
fn keyboard_onboard_profiles_retains_actual_normal_values_when_later_pass_fails() {
    let mut replies = initial_replies(&[3]);
    replies.extend(brightness(3, 63));
    replies.extend(mode_replies(3, Mode::Normal, &[4], 900));
    let mut failure = reply(&[]);
    failure.status = 3;
    replies.extend([failure.clone(), failure.clone(), failure]);
    replies.push(reply(&[3, 0, 4, 8]));
    let device = Mock::new(replies);
    let observed = read_small(&device, &[4]).unwrap();
    let profile = &observed.profiles[0];
    assert_eq!(profile.actuation(4).unwrap().threshold_low, 902);
    assert_eq!(profile.rapid_trigger(4).unwrap().up_sensitivity, 900);
    assert!(matches!(
        profile.passes[1].actuation[0].reading,
        Reading::Failed { .. }
    ));
    assert!(profile.assignment(Mode::Hypershift, 4).is_none());
}

#[test]
fn keyboard_onboard_profiles_reports_failed_brightness_and_continues_real_reads() {
    let mut replies = initial_replies(&[3]);
    replies.push(reply(&[7, 1, 2, 3, 4]));
    let mut failure = reply(&[]);
    failure.status = 3;
    replies.push(failure);
    replies.extend(mode_replies(3, Mode::Normal, &[4], 123));
    replies.extend(mode_replies(3, Mode::Hypershift, &[4], 456));
    replies.push(reply(&[3, 1, 4, 8]));
    let device = Mock::new(replies);
    let observed = read_small(&device, &[4]).unwrap();
    assert!(matches!(
        observed.profiles[0].brightness,
        Reading::Failed { .. }
    ));
    assert_eq!(
        observed.profiles[0].actuation(4).unwrap().threshold_low,
        458
    );
}

#[test]
fn keyboard_onboard_profiles_cancellation_stops_before_other_commands() {
    for limit in [1, 12] {
        let mut replies = initial_replies(&[3]);
        replies.extend(brightness(3, 20));
        replies.extend(mode_replies(3, Mode::Normal, &[4], 1000));
        let device = Mock::new(replies);
        let cap = capability();
        let settings = settings_capability(&cap).unwrap();
        let result = read_selected(
            &device,
            &cap,
            &settings,
            1,
            keys(&[4]),
            &mut sequence(),
            &|| {
                ensure!(device.sent().len() < limit, "caller cancelled");
                Ok(())
            },
        );
        assert!(format!("{:#}", result.unwrap_err()).contains("caller cancelled"));
        assert_eq!(device.sent().len(), limit);
    }
}

#[test]
fn keyboard_onboard_profiles_uses_generated_physical_inventory_and_rejects_wrong_factory() {
    let keys = selected_keys(1).unwrap();
    assert_eq!(keys.len(), 82);
    assert!(
        keys.windows(2)
            .all(|pair| pair[0].firmware_key_id < pair[1].firmware_key_id)
    );
    assert_eq!(keys.iter().filter(|key| key.ignore_hypershift).count(), 6);
    assert!(keys.iter().all(|key| !key.ignore_normal));
    let device = Mock::new([]);
    for alter in [0, 1, 2] {
        let mut cap = capability();
        match alter {
            0 => cap.product_id = 740,
            1 => cap.claim_interface = 2,
            _ => cap.source_class = "rzDevice30".into(),
        }
        assert!(read_hid(&device, &cap, 1, sequence(), &|| Ok(())).is_err());
    }
    assert!(device.sent().is_empty());
}

fn summary(id: u16, guid: &str) -> MacroSummary {
    MacroSummary {
        metadata: MacroMetadata {
            macro_id: id,
            guid: guid.into(),
            num_items: 1,
            name: "observed macro".into(),
            extra_info: [0; 16],
            timestamp: "device time".into(),
            hash: "device hash".into(),
            raw: vec![9; 250],
            blocks: Vec::new(),
        },
        data_size: 1,
        allocated_size_with_name: 512,
    }
}

#[test]
fn keyboard_onboard_profiles_detail_consumer_skips_known_guids_and_reads_size_content_then_continuous()
 {
    let initial = Mock::new(initial_replies(&[]));
    let mut snapshot = read_small(&initial, &[4]).unwrap();
    snapshot.macros.macros = vec![summary(7, "already-local"), summary(8, "from-device")];
    snapshot.inventory = ProfileInventory {
        max_profiles_supported: 6,
        declared_profile_count: 1,
        slots: vec![crate::keyboard_profile_inventory::ProfileSlot {
            slot_id: 3,
            metadata: crate::keyboard_profile_inventory::ProfileMetadataObservation::Observed {
                identity: crate::keyboard_profile_inventory::ProfileIdentity {
                    guid: "af06d371-861f-4b98-8d78-bfaef5cfdebf".into(),
                    guid_status: crate::keyboard_profile_inventory::GuidStatus::Recognized,
                    name: "device profile".into(),
                    owner: "owner".into(),
                },
                mapping_hash: crate::keyboard_profile_inventory::MappingHashObservation::Unobserved,
            },
        }],
    };
    let device = Mock::new([
        reply(&[0, 8, 0, 0, 0, 3]),
        reply(&[0, 8, 0, 0, 0, 0, 3, 0x10, 0x20, 0x30]),
        reply(&[3, 9]),
    ]);
    let details = read_import_details(
        &device,
        &capability(),
        &snapshot,
        &BTreeSet::from(["already-local".to_owned()]),
        sequence(),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(details.macros[0].content, Reading::Unobserved);
    let content = details.macros[1].content.observed().unwrap();
    assert_eq!(content.data, [0x10, 0x20, 0x30]);
    assert_eq!(content.metadata.guid, "from-device");
    assert_eq!(content.metadata.raw, [9; 250]);
    assert_eq!(
        details.continuous_rapid_trigger[0].raw_enabled,
        Reading::Observed(9)
    );
    let reports = device.sent();
    assert_eq!(
        reports
            .iter()
            .map(|report| (report[6], report[7], report[8]))
            .collect::<Vec<_>>(),
        [(6, 6, 136), (10, 6, 137), (2, 2, 160)]
    );
    assert_eq!(&reports[1][9..16], &[0, 8, 0, 0, 0, 0, 3]);
    assert_report_integrity(&reports);
}

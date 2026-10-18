//! Independent byte fixtures and mandatory synthetic Feature exchanges.
use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

enum Step {
    Send(Vec<u8>, bool),
    Get(Vec<u8>, usize),
    ReadError,
}
struct Transport {
    steps: RefCell<VecDeque<Step>>,
    sends: Cell<usize>,
    reads: Cell<usize>,
}
impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
            sends: Cell::new(0),
            reads: Cell::new(0),
        }
    }
    fn finish(&self) {
        assert!(
            self.steps.borrow().is_empty(),
            "Fixture exchanges not consumed"
        );
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        let Step::Send(expected, success) =
            self.steps.borrow_mut().pop_front().expect("Unexpected OUT")
        else {
            panic!("OUT where IN was required")
        };
        assert_eq!(
            report, expected,
            "Nominal command, packet size, metadata, padding or CRC differs"
        );
        self.sends.set(self.sends.get() + 1);
        ensure!(success, "Synthetic OUT failure");
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        // Physical GET selectors are verified by the shared query tests.
        assert_eq!(report.len(), 91);
        assert_eq!(&report[1..], [0; 90]);
        self.reads.set(self.reads.get() + 1);
        match self.steps.borrow_mut().pop_front().expect("Unexpected IN") {
            Step::Get(reply, count) => {
                report.copy_from_slice(&reply);
                Ok(count)
            }
            Step::ReadError => anyhow::bail!("Synthetic IN error"),
            Step::Send(..) => panic!("IN where OUT was required"),
        }
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn packet(command: [u8; 3], data_size: u8, payload: &[u8], transaction: u8) -> Vec<u8> {
    let mut report = vec![0; 91];
    report[2] = transaction;
    report[6] = data_size;
    report[7] = command[1];
    report[8] = command[2];
    report[9..9 + payload.len()].copy_from_slice(payload);
    report[89] = report[3..89].iter().fold(0, |a, b| a ^ b);
    report
}
fn response(command: [u8; 3], data: &[u8], transaction: u8, status: u8) -> Vec<u8> {
    let mut report = packet(command, data.len() as u8, data, transaction);
    report[1] = status;
    report
}
fn exchange_steps(command: [u8; 3], size: u8, payload: &[u8], data: &[u8], tx: u8) -> [Step; 2] {
    [
        Step::Send(packet(command, size, payload, tx), true),
        Step::Get(response(command, data, tx, 2), 91),
    ]
}
fn cap(product_id: u32) -> DeviceReadCapability {
    let mut cap = crate::device_reads::capability(product_id).unwrap().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_out = 1;
    cap.max_retry_in = 1;
    cap
}
fn current_scroll_cap(product_id: u32) -> DeviceReadCapability {
    let mut capability = crate::mouse_current_dpi::capability(product_id)
        .unwrap()
        .transport()
        .for_physical_product(product_id)
        .unwrap();
    capability.sleep_between_out_ms = 0;
    capability.sleep_between_out_in_ms = 0;
    capability.sleep_between_in_ms = 0;
    capability.max_retry_out = 1;
    capability.max_retry_in = 1;
    capability
}
fn transactions() -> impl FnMut() -> Result<u8> {
    let mut current = 0;
    move || {
        current = (current + 1) % 31;
        Ok(current)
    }
}
fn next(tx: &mut u8) -> u8 {
    *tx = (*tx + 1) % 31;
    *tx
}
fn inventory(steps: &mut Vec<Step>, maximum: u8, count: u8, ids: &[u8], tx: &mut u8) {
    steps.extend(exchange_steps([1, 5, 138], 1, &[], &[maximum], next(tx)));
    steps.extend(exchange_steps([1, 5, 128], 1, &[], &[count], next(tx)));
    let mut list = vec![ids.len() as u8];
    list.extend_from_slice(ids);
    steps.extend(exchange_steps([80, 5, 129], 80, &[], &list, next(tx)));
}
fn identity(hash: [u8; 6]) -> Vec<u8> {
    // Windows GUID order for 12345678-9abc-def0-1234-56789abcdef0.
    let mut result = vec![0; 250];
    result[..16].copy_from_slice(&[
        0x78, 0x56, 0x34, 0x12, 0xbc, 0x9a, 0xf0, 0xde, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde,
        0xf0,
    ]);
    result[16..28].copy_from_slice(b"Test Profile");
    result[116..121].copy_from_slice(b"owner");
    result[244..250].copy_from_slice(&hash);
    result
}
fn request(hash: Option<[u8; 6]>) -> OnboardIdentityRequest {
    OnboardIdentityRequest {
        guid: "12345678-9abc-def0-1234-56789abcdef0".into(),
        name: "Test Profile".into(),
        owner: "owner".into(),
        mapping_hash: hash,
    }
}
fn read_identity(steps: &mut Vec<Step>, profile: u8, raw: &[u8], tx: &mut u8) {
    for (offset, count) in [(0usize, 64usize), (64, 64), (128, 64), (192, 58)] {
        let offset_bytes = (offset as u16).to_be_bytes();
        let mut result = vec![profile, offset_bytes[0], offset_bytes[1], 0, 250];
        result.extend_from_slice(&raw[offset..offset + count]);
        steps.extend(exchange_steps(
            [80, 5, 136],
            (count + 5) as u8,
            &[profile, offset_bytes[0], offset_bytes[1], 0, 0],
            &result,
            next(tx),
        ));
    }
}
fn write_identity(steps: &mut Vec<Step>, profile: u8, raw: &[u8], ack: &[u8], tx: &mut u8) {
    steps.extend(exchange_steps([1, 5, 2], 1, &[profile], ack, next(tx)));
    for (offset, count) in [(0usize, 64usize), (64, 64), (128, 64), (192, 58)] {
        let offset_bytes = (offset as u16).to_be_bytes();
        let mut payload = vec![profile, offset_bytes[0], offset_bytes[1], 0, 250];
        payload.extend_from_slice(&raw[offset..offset + count]);
        steps.extend(exchange_steps(
            [80, 5, 8],
            (count + 5) as u8,
            &payload,
            ack,
            next(tx),
        ));
    }
}

#[test]
fn scroll_profile_reader_keeps_hardware_inventory_order_and_does_not_expand_writers() {
    for product_id in [153, 170, 203, 204, 214, 226, 45066] {
        let mut steps = Vec::new();
        let mut tx = 0;
        inventory(&mut steps, 8, 3, &[1, 7, 3], &mut tx);
        read_identity(&mut steps, 7, &identity([0; 6]), &mut tx);
        let mut other = identity([0; 6]);
        other[0] = 0x79;
        read_identity(&mut steps, 3, &other, &mut tx);
        let transport = Transport::new(steps);
        let capability = current_scroll_cap(product_id);
        let observed =
            read_profile_associations(&transport, &capability, &mut transactions(), &|| Ok(()))
                .unwrap();
        assert_eq!(
            observed,
            vec![
                OnboardProfileAssociation {
                    profile_id: 7,
                    guid: "12345678-9abc-def0-1234-56789abcdef0".into(),
                },
                OnboardProfileAssociation {
                    profile_id: 3,
                    guid: "12345679-9abc-def0-1234-56789abcdef0".into(),
                },
            ]
        );
        assert_eq!(transport.sends.get(), 11);
        assert_eq!(
            MouseOnboard::new(&transport, &capability).is_ok(),
            AUDITED_PRODUCTS.contains(&product_id)
                && capability.source_class == "rzDevice25DualLinkMouse"
        );
        if !AUDITED_PRODUCTS.contains(&product_id) {
            assert!(encode_identity(product_id, &request(None)).is_err());
        }
        transport.finish();
    }
}

#[test]
fn scroll_profile_inventory_read_cancellation_stops_before_metadata_or_mutations() {
    let mut steps = Vec::new();
    inventory(&mut steps, 8, 2, &[1, 7], &mut 0);
    let transport = Transport::new(steps);
    let error = read_profile_associations(
        &transport,
        &current_scroll_cap(153),
        &mut transactions(),
        &|| {
            ensure!(transport.reads.get() < 3, "Owner canceled after inventory");
            Ok(())
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("canceled"));
    assert_eq!(transport.sends.get(), 3);
    transport.finish();
}

#[test]
fn audited_source_commands_and_product_branches_match_receipts() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../testdata/re/mouse-onboard-current-source.json"
    ))
    .unwrap();
    assert_eq!(evidence["product_ids"], serde_json::json!(AUDITED_PRODUCTS));
    let expected = [
        ("getMaxProfilesSupported", [1, 5, 138]),
        ("getNumberOfProfiles", [1, 5, 128]),
        ("getProfileIDList", [80, 5, 129]),
        ("getProfileName", [80, 5, 136]),
        ("setProfileName", [80, 5, 8]),
        ("createNewProfile", [1, 5, 2]),
        ("deleteProfile", [1, 5, 3]),
        ("getMacroStorageInfo", [14, 6, 142]),
    ];
    for product in evidence["products"].as_array().unwrap() {
        for (name, command) in expected {
            let method = product["methods"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["name"] == name)
                .unwrap();
            assert_eq!(method["command"], serde_json::json!(command));
            assert_eq!(method["receipt"]["sha256"].as_str().unwrap().len(), 64);
        }
        assert_eq!(
            product["semantics"]["chunk_offsets"],
            serde_json::json!([0, 64, 128, 192])
        );
        assert_eq!(
            product["semantics"]["dynamic_payload_sizes"],
            serde_json::json!([69, 69, 69, 63])
        );
        let setter = product["genericMethods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["name"] == "setProfileNameStructure")
            .unwrap();
        let source = setter["receipt"]["source"].as_str().unwrap();
        let product_id = product["product_id"].as_u64().unwrap();
        assert_eq!(
            source.contains("Array.isArray("),
            matches!(product_id, 170 | 204)
        );
    }
}

#[test]
fn gate_rejects_unaudited_product_and_different_startup_class() {
    let transport = Transport::new([]);
    let mut wrong = cap(170);
    wrong.product_id = 167;
    assert!(MouseOnboard::new(&transport, &wrong).is_err());
    let mut wrong = cap(170);
    wrong.source_class = "rzDevice30".into();
    assert!(MouseOnboard::new(&transport, &wrong).is_err());
    transport.finish();
}

#[test]
fn guid_and_metadata_match_independent_wire_bytes() {
    assert_eq!(
        encode_identity(170, &request(Some([1, 2, 3, 4, 5, 6]))).unwrap(),
        identity([1, 2, 3, 4, 5, 6])
    );
    let upper = OnboardIdentityRequest {
        guid: "12345678-9ABC-DEF0-1234-56789ABCDEF0".into(),
        ..request(None)
    };
    assert_eq!(encode_identity(226, &upper).unwrap(), identity([0; 6]));
}

#[test]
fn invalid_guid_or_owner_is_rejected_before_hardware() {
    for guid in [
        "",
        "123456789abcdef0123456789abcdef0",
        "12345678-9abc-def0-1234-56789abcdeg0",
        "12345678x9abc-def0-1234-56789abcdef0",
        "🙂345678-9abc-def0-1234-56789abcdef0",
    ] {
        let bad = OnboardIdentityRequest {
            guid: guid.into(),
            ..request(None)
        };
        assert!(encode_identity(170, &bad).is_err());
    }
    let long = OnboardIdentityRequest {
        owner: "😀".repeat(65),
        ..request(None)
    };
    let t = Transport::new([]);
    let c = cap(170);
    let mouse = MouseOnboard::new(&t, &c).unwrap();
    let error = mouse
        .write_identity(3, &long, &mut transactions(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Prepare);
    assert!(!error.device_state_may_have_changed());
    t.finish();
}

#[test]
fn newer_hash_is_preserved_and_older_source_requires_zero() {
    for pid in [170, 204] {
        assert_eq!(
            &encode_identity(pid, &request(Some([1, 2, 3, 4, 5, 6]))).unwrap()[244..],
            &[1, 2, 3, 4, 5, 6]
        );
    }
    for pid in [226, 45066] {
        assert!(encode_identity(pid, &request(Some([1, 2, 3, 4, 5, 6]))).is_err());
        assert_eq!(
            &encode_identity(pid, &request(None)).unwrap()[244..],
            &[0; 6]
        );
    }
}

#[test]
fn unicode_name_utf8_and_owner_utf16_low_bytes_reproduce_source() {
    let req = OnboardIdentityRequest {
        name: "硬件".into(),
        owner: "中😀".into(),
        ..request(None)
    };
    let raw = encode_identity(170, &req).unwrap();
    assert_eq!(&raw[16..22], &[0xe7, 0xa1, 0xac, 0xe4, 0xbb, 0xb6]);
    assert_eq!(&raw[116..119], &[0x2d, 0x3d, 0]);
    let decoded = decode_identity(170, 3, raw, 1);
    assert_eq!(decoded.name, "硬件");
    assert_eq!(decoded.owner, "-=");
}

#[test]
fn source_full_name_drops_last_byte_and_preserves_raw_truncation() {
    let req = OnboardIdentityRequest {
        name: "X".repeat(110),
        ..request(None)
    };
    let raw = encode_identity(170, &req).unwrap();
    assert_eq!(&raw[16..116], &[b'X'; 100]);
    let decoded = decode_identity(170, 3, raw.clone(), 1);
    assert_eq!(decoded.name, "X".repeat(99));
    assert_eq!(decoded.raw_metadata, raw);
}

#[test]
fn invalid_truncated_utf8_and_leading_bom_match_text_decoder_semantics() {
    let req = OnboardIdentityRequest {
        name: format!("{}中", "X".repeat(98)),
        ..request(None)
    };
    let decoded = decode_identity(170, 3, encode_identity(170, &req).unwrap(), 1);
    assert_eq!(decoded.name, format!("{}�", "X".repeat(98)));
    let req = OnboardIdentityRequest {
        name: "\u{feff}hello".into(),
        ..request(None)
    };
    assert_eq!(
        decode_identity(170, 3, encode_identity(170, &req).unwrap(), 1).name,
        "hello"
    );
}

#[test]
fn owner_getter_uses_only_first_64_bytes_and_trims_edge_nuls_only() {
    let req = OnboardIdentityRequest {
        owner: format!("\0A\0B{}tail", "C".repeat(60)),
        ..request(None)
    };
    let decoded = decode_identity(170, 3, encode_identity(170, &req).unwrap(), 1);
    assert_eq!(decoded.owner, format!("A\0B{}", "C".repeat(60)));
    assert_eq!(&decoded.raw_metadata[180..184], b"tail");
}

#[test]
fn identity_getter_transfers_actual_offsets_sizes_and_parses_real_bytes() {
    for pid in AUDITED_PRODUCTS {
        let raw = identity([1, 2, 3, 4, 5, 6]);
        let mut steps = Vec::new();
        let mut tx = 0;
        read_identity(&mut steps, 3, &raw, &mut tx);
        let t = Transport::new(steps);
        let c = cap(*pid);
        let m = MouseOnboard::new(&t, &c).unwrap();
        let obs = m.read_identity(3, &mut transactions(), &|| Ok(())).unwrap();
        assert_eq!(obs.guid, "12345678-9abc-def0-1234-56789abcdef0");
        assert_eq!(obs.name, "Test Profile");
        assert_eq!(obs.owner, "owner");
        assert_eq!(obs.raw_metadata, raw);
        assert_eq!(obs.attempts, 1);
        assert!(obs.source_guid_accepted);
        assert_eq!(
            obs.mapping_hash,
            matches!(pid, 170 | 204).then_some([1, 2, 3, 4, 5, 6])
        );
        t.finish();
    }
}

#[test]
fn source_guid_filter_retries_ten_complete_reads_and_returns_observed_final_bytes() {
    let raw = vec![0; 250];
    let mut steps = Vec::new();
    let mut tx = 0;
    for _ in 0..10 {
        read_identity(&mut steps, 3, &raw, &mut tx);
    }
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let obs = m.read_identity(3, &mut transactions(), &|| Ok(())).unwrap();
    assert_eq!(obs.attempts, 10);
    assert!(!obs.source_guid_accepted);
    assert_eq!(obs.raw_metadata, raw);
    assert_eq!(t.sends.get(), 40);
    t.finish();
}

#[test]
fn guid_retry_stops_when_later_actual_bytes_pass_the_source_filter() {
    let mut steps = Vec::new();
    let mut tx = 0;
    read_identity(&mut steps, 3, &vec![0; 250], &mut tx);
    read_identity(&mut steps, 3, &identity([0; 6]), &mut tx);
    let t = Transport::new(steps);
    let c = cap(204);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let obs = m.read_identity(3, &mut transactions(), &|| Ok(())).unwrap();
    assert_eq!(obs.attempts, 2);
    assert!(obs.source_guid_accepted);
    t.finish();
}

#[test]
fn inventory_getter_checks_hardware_capacity_count_ids_without_local_defaults() {
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 5, 3, &[1, 5, 3], &mut tx);
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    assert_eq!(
        m.read_inventory(&mut transactions(), &|| Ok(())).unwrap(),
        OnboardInventory {
            max_profiles: 5,
            profile_count: 3,
            profile_ids: vec![1, 5, 3]
        }
    );
    t.finish();
}

#[test]
fn duplicate_zero_ids_and_count_capacity_disagreement_are_errors() {
    for (maximum, count, ids) in [
        (5, 2, vec![1, 1]),
        (5, 2, vec![1, 0]),
        (5, 3, vec![1, 3]),
        (1, 2, vec![1, 3]),
    ] {
        let mut steps = Vec::new();
        let mut tx = 0;
        inventory(&mut steps, maximum, count, &ids, &mut tx);
        let t = Transport::new(steps);
        let c = cap(170);
        let m = MouseOnboard::new(&t, &c).unwrap();
        assert!(m.read_inventory(&mut transactions(), &|| Ok(())).is_err());
        t.finish();
    }
}

#[test]
fn missing_capacity_and_truncated_id_list_do_not_become_empty_inventory() {
    let t = Transport::new(exchange_steps([1, 5, 138], 1, &[], &[], 1));
    let c = cap(170);
    assert!(
        MouseOnboard::new(&t, &c)
            .unwrap()
            .read_inventory(&mut transactions(), &|| Ok(()))
            .is_err()
    );
    t.finish();
    let mut steps = Vec::new();
    steps.extend(exchange_steps([1, 5, 138], 1, &[], &[5], 1));
    steps.extend(exchange_steps([1, 5, 128], 1, &[], &[2], 2));
    steps.extend(exchange_steps([80, 5, 129], 80, &[], &[2, 1], 3));
    let t = Transport::new(steps);
    assert!(
        MouseOnboard::new(&t, &c)
            .unwrap()
            .read_inventory(&mut transactions(), &|| Ok(()))
            .is_err()
    );
    t.finish();
}

#[test]
fn identity_write_create_chunks_and_independent_readback_are_source_ordered() {
    for pid in AUDITED_PRODUCTS {
        let hash = if matches!(pid, 170 | 204) {
            [1, 2, 3, 4, 5, 6]
        } else {
            [0; 6]
        };
        let raw = identity(hash);
        let mut steps = Vec::new();
        let mut tx = 0;
        inventory(&mut steps, 5, 1, &[1], &mut tx);
        write_identity(&mut steps, 3, &raw, &[], &mut tx);
        read_identity(&mut steps, 3, &raw, &mut tx);
        inventory(&mut steps, 5, 2, &[1, 3], &mut tx);
        let t = Transport::new(steps);
        let c = cap(*pid);
        let m = MouseOnboard::new(&t, &c).unwrap();
        let req = request(matches!(pid, 170 | 204).then_some(hash));
        let report = m
            .write_identity(3, &req, &mut transactions(), &|| Ok(()))
            .unwrap();
        assert!(report.verified());
        assert_eq!(report.steps.len(), 5);
        assert!(
            report.steps.iter().all(|s| s.attempted
                && s.acknowledgment_received
                && s.acknowledgment_data.is_empty())
        );
        assert_eq!(
            report
                .steps
                .iter()
                .map(|s| s.report_data_size)
                .collect::<Vec<_>>(),
            vec![1, 69, 69, 69, 63]
        );
        assert_eq!(report.identity_readback_blocks.len(), 4);
        assert_eq!(report.identity_readback.unwrap().raw_metadata, raw);
        assert_eq!(report.inventory_after.unwrap().profile_count, 2);
        t.finish();
    }
}

#[test]
fn existing_identity_rewrite_still_creates_and_never_invents_delete_or_full_content_save() {
    let raw = identity([4; 6]);
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 2, 2, &[1, 3], &mut tx);
    write_identity(&mut steps, 3, &raw, &[9, 8], &mut tx);
    read_identity(&mut steps, 3, &raw, &mut tx);
    inventory(&mut steps, 2, 2, &[3, 1], &mut tx);
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let report = m
        .write_identity(3, &request(Some([4; 6])), &mut transactions(), &|| Ok(()))
        .unwrap();
    assert!(report.verified());
    assert!(report.steps.iter().all(|s| s.acknowledgment_data == [9, 8]));
    t.finish();
}

#[test]
fn full_capacity_prevents_new_slot_before_any_mutation() {
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 2, 2, &[1, 3], &mut tx);
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let error = m
        .write_identity(4, &request(None), &mut transactions(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Observe);
    assert!(!error.device_state_may_have_changed());
    assert!(error.progress.steps.is_empty());
    assert!(error.progress.inventory_before.is_some());
    t.finish();
}

#[test]
fn reserved_profile_one_and_zero_are_rejected_without_io() {
    for profile in [0, 1] {
        let t = Transport::new([]);
        let c = cap(170);
        let m = MouseOnboard::new(&t, &c).unwrap();
        assert!(
            m.read_identity(profile, &mut transactions(), &|| Ok(()))
                .is_err()
        );
        assert_eq!(
            m.write_identity(profile, &request(None), &mut transactions(), &|| Ok(()))
                .unwrap_err()
                .phase,
            OnboardFailurePhase::Prepare
        );
        assert_eq!(
            m.delete_profile(profile, &mut transactions(), &|| Ok(()))
                .unwrap_err()
                .phase,
            OnboardFailurePhase::Prepare
        );
        t.finish();
    }
}

#[test]
fn failed_chunk_retains_create_ack_and_stops_without_later_chunks() {
    let raw = identity([0; 6]);
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 5, 1, &[1], &mut tx);
    steps.extend(exchange_steps([1, 5, 2], 1, &[3], &[], next(&mut tx)));
    let mut payload = vec![3, 0, 0, 0, 250];
    payload.extend_from_slice(&raw[..64]);
    let current = next(&mut tx);
    steps.push(Step::Send(packet([80, 5, 8], 69, &payload, current), true));
    steps.push(Step::Get(response([80, 5, 8], &[], current, 3), 91));
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let error = m
        .write_identity(3, &request(None), &mut transactions(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Write);
    assert_eq!(error.failed_step, Some(1));
    assert!(error.device_state_may_have_changed());
    assert_eq!(error.progress.steps.len(), 2);
    assert!(error.progress.steps[0].acknowledgment_received);
    assert!(!error.progress.steps[1].acknowledgment_received);
    assert!(!error.progress.verified());
    t.finish();
}

#[test]
fn all_setter_acks_do_not_confirm_different_hardware_metadata() {
    let raw = identity([0; 6]);
    let mut changed = raw.clone();
    changed[244] = 9;
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 5, 1, &[1], &mut tx);
    write_identity(&mut steps, 3, &raw, &raw[..5], &mut tx);
    read_identity(&mut steps, 3, &changed, &mut tx);
    let t = Transport::new(steps);
    let c = cap(226);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let error = m
        .write_identity(3, &request(None), &mut transactions(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Readback);
    assert_eq!(error.progress.steps.len(), 5);
    assert!(
        error
            .progress
            .steps
            .iter()
            .all(|s| s.acknowledgment_received)
    );
    assert_eq!(
        error.progress.identity_readback.unwrap().raw_metadata,
        changed
    );
    assert!(error.progress.inventory_after.is_none());
    t.finish();
}

#[test]
fn matching_metadata_without_the_slot_in_inventory_is_not_verified() {
    let raw = identity([0; 6]);
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 5, 1, &[1], &mut tx);
    write_identity(&mut steps, 3, &raw, &[], &mut tx);
    read_identity(&mut steps, 3, &raw, &mut tx);
    inventory(&mut steps, 5, 1, &[1], &mut tx);
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let error = m
        .write_identity(3, &request(None), &mut transactions(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Readback);
    assert!(error.progress.identity_readback.is_some());
    assert_eq!(error.progress.inventory_after.unwrap().profile_ids, vec![1]);
    t.finish();
}

#[test]
fn wrong_block_offset_retains_completed_readback_blocks_and_fails() {
    let raw = identity([0; 6]);
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 5, 1, &[1], &mut tx);
    write_identity(&mut steps, 3, &raw, &[], &mut tx);
    let mut reads = Vec::new();
    read_identity(&mut reads, 3, &raw, &mut tx);
    if let Step::Get(reply, _) = &mut reads[3] {
        reply[11] = 65;
    }
    steps.extend(reads.into_iter().take(4));
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let error = m
        .write_identity(3, &request(None), &mut transactions(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Readback);
    assert_eq!(error.progress.identity_readback_blocks.len(), 2);
    assert!(error.progress.identity_readback.is_none());
    t.finish();
}

#[test]
fn truncated_identity_payload_and_wrong_profile_fail_without_zero_filling() {
    for data in [vec![3, 0, 0, 0, 250], vec![4; 69]] {
        let t = Transport::new(exchange_steps([80, 5, 136], 69, &[3, 0, 0, 0, 0], &data, 1));
        let c = cap(170);
        assert!(
            MouseOnboard::new(&t, &c)
                .unwrap()
                .read_identity(3, &mut transactions(), &|| Ok(()))
                .is_err()
        );
        t.finish();
    }
}

#[test]
fn storage_refresh_keeps_big_endian_raw_fields() {
    let raw = [
        0x12, 0x34, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x76, 0x54, 0x32, 0x10,
    ];
    let t = Transport::new(exchange_steps([14, 6, 142], 14, &[], &raw, 1));
    let c = cap(170);
    assert_eq!(
        MouseOnboard::new(&t, &c)
            .unwrap()
            .read_storage_info(&mut transactions(), &|| Ok(()))
            .unwrap(),
        OnboardStorageInfo {
            max_macro_id: 0x1234,
            max_memory: 0x01234567,
            available_memory: 0x89abcdef,
            recyclable_memory: 0x76543210
        }
    );
    t.finish();
}

#[test]
fn delete_ack_refreshes_storage_then_checks_actual_remaining_slots() {
    for pid in AUDITED_PRODUCTS {
        let mut steps = Vec::new();
        let mut tx = 0;
        inventory(&mut steps, 5, 3, &[1, 5, 3], &mut tx);
        steps.extend(exchange_steps([1, 5, 3], 1, &[3], &[], next(&mut tx)));
        steps.extend(exchange_steps(
            [14, 6, 142],
            14,
            &[],
            &[0, 9, 0, 0, 16, 0, 0, 0, 12, 0, 0, 0, 4, 0],
            next(&mut tx),
        ));
        inventory(&mut steps, 5, 2, &[5, 1], &mut tx);
        let t = Transport::new(steps);
        let c = cap(*pid);
        let m = MouseOnboard::new(&t, &c).unwrap();
        let report = m
            .delete_profile(3, &mut transactions(), &|| Ok(()))
            .unwrap();
        assert!(report.verified());
        assert_eq!(report.steps.len(), 1);
        assert_eq!(report.storage_refresh.unwrap().recyclable_memory, 1024);
        assert_eq!(report.inventory_after.unwrap().profile_ids, vec![5, 1]);
        t.finish();
    }
}

#[test]
fn absent_profile_cannot_be_deleted_from_a_local_default() {
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 5, 1, &[1], &mut tx);
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let error = m
        .delete_profile(3, &mut transactions(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Observe);
    assert!(!error.device_state_may_have_changed());
    t.finish();
}

#[test]
fn deletion_storage_failure_retains_ack_without_claiming_verified() {
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 5, 2, &[1, 3], &mut tx);
    steps.extend(exchange_steps([1, 5, 3], 1, &[3], &[], next(&mut tx)));
    steps.extend(exchange_steps([14, 6, 142], 14, &[], &[], next(&mut tx)));
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let error = m
        .delete_profile(3, &mut transactions(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::StorageRefresh);
    assert!(error.progress.steps[0].acknowledgment_received);
    assert!(error.device_state_may_have_changed());
    assert!(!error.progress.verified());
    t.finish();
}

#[test]
fn successful_delete_ack_with_unchanged_slots_is_an_explicit_readback_failure() {
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 5, 2, &[1, 3], &mut tx);
    steps.extend(exchange_steps([1, 5, 3], 1, &[3], &[], next(&mut tx)));
    steps.extend(exchange_steps(
        [14, 6, 142],
        14,
        &[],
        &[0; 14],
        next(&mut tx),
    ));
    inventory(&mut steps, 5, 2, &[1, 3], &mut tx);
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let error = m
        .delete_profile(3, &mut transactions(), &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Readback);
    assert!(error.progress.storage_refresh.is_some());
    assert!(!error.progress.verified());
    t.finish();
}

#[test]
fn cancellation_before_first_io_has_no_mutation_or_transport_effects() {
    let t = Transport::new([]);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let error = m
        .write_identity(3, &request(None), &mut transactions(), &|| {
            anyhow::bail!("Canceled")
        })
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Observe);
    assert!(!error.device_state_may_have_changed());
    t.finish();
}

#[test]
fn cancellation_after_create_ack_stops_before_first_block() {
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 5, 1, &[1], &mut tx);
    steps.extend(exchange_steps([1, 5, 2], 1, &[3], &[], next(&mut tx)));
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    // Each operation has one caller validation plus four I/O validations.
    let checks = Cell::new(0);
    let validate = || {
        checks.set(checks.get() + 1);
        ensure!(checks.get() <= 20, "Canceled");
        Ok(())
    };
    let error = m
        .write_identity(3, &request(None), &mut transactions(), &validate)
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Write);
    assert_eq!(error.progress.steps.len(), 1);
    assert_eq!(error.failed_step, None);
    assert!(error.progress.steps[0].acknowledgment_received);
    assert!(error.device_state_may_have_changed());
    t.finish();
}

#[test]
fn canceled_reply_does_not_turn_into_a_setter_ack() {
    let mut steps = Vec::new();
    let mut tx = 0;
    inventory(&mut steps, 5, 1, &[1], &mut tx);
    steps.extend(exchange_steps([1, 5, 2], 1, &[3], &[], next(&mut tx)));
    let t = Transport::new(steps);
    let c = cap(170);
    let m = MouseOnboard::new(&t, &c).unwrap();
    let checks = Cell::new(0);
    let validate = || {
        checks.set(checks.get() + 1);
        ensure!(checks.get() < 20, "Canceled after reply");
        Ok(())
    };
    let error = m
        .write_identity(3, &request(None), &mut transactions(), &validate)
        .unwrap_err();
    assert_eq!(error.phase, OnboardFailurePhase::Write);
    assert_eq!(error.progress.steps.len(), 1);
    assert!(!error.progress.steps[0].acknowledgment_received);
    assert!(error.device_state_may_have_changed());
    t.finish();
}

#[test]
fn dynamic_exchange_changes_only_packet_size_and_keeps_nominal_command() {
    let payload = [3, 0, 192, 0, 250];
    let t = Transport::new(exchange_steps([80, 5, 136], 63, &payload, &[7, 8], 1));
    let c = cap(170);
    assert_eq!(
        exchange_with_report_data_size(&t, &c, [80, 5, 136], &payload, 1, 63, &|| Ok(())).unwrap(),
        vec![7, 8]
    );
    t.finish();
    let t = Transport::new(exchange_steps([80, 5, 136], 80, &payload, &[7, 8], 1));
    assert_eq!(
        crate::feature_exchange::exchange(&t, &c, [80, 5, 136], &payload, 1, &|| Ok(())).unwrap(),
        vec![7, 8]
    );
    t.finish();
}

#[test]
fn invalid_dynamic_size_or_transaction_is_rejected_without_io() {
    let t = Transport::new([]);
    let c = cap(170);
    for size in [0, 4, 81] {
        assert!(
            exchange_with_report_data_size(
                &t,
                &c,
                [80, 5, 136],
                &[3, 0, 0, 0, 0],
                1,
                size,
                &|| Ok(())
            )
            .is_err()
        );
    }
    assert!(
        exchange_with_report_data_size(&t, &c, [80, 5, 136], &[3], 31, 69, &|| Ok(())).is_err()
    );
    t.finish();
}

#[test]
fn busy_and_transport_retry_preserve_dynamic_size_and_transaction() {
    let mut c = cap(170);
    c.max_retry_out = 2;
    c.max_retry_in = 2;
    let p = [3, 0, 0, 0, 0];
    let out = packet([80, 5, 136], 69, &p, 1);
    let t = Transport::new([
        Step::Send(out.clone(), false),
        Step::Send(out, true),
        Step::Get(response([80, 5, 136], &[], 1, 1), 91),
        Step::Get(response([80, 5, 136], &[9], 1, 2), 91),
    ]);
    assert_eq!(
        exchange_with_report_data_size(&t, &c, [80, 5, 136], &p, 1, 69, &|| Ok(())).unwrap(),
        vec![9]
    );
    assert_eq!(t.sends.get(), 2);
    assert_eq!(t.reads.get(), 2);
    t.finish();
}

#[test]
fn dynamic_exchange_rejects_truncated_count_mismatched_command_transaction_and_status() {
    let c = cap(170);
    let p = [3, 0, 0, 0, 0];
    for (reply, count) in [
        (response([80, 5, 136], &[], 1, 2), 90),
        (response([80, 5, 137], &[], 1, 2), 91),
        (response([80, 5, 136], &[], 2, 2), 91),
        (response([80, 5, 136], &[], 1, 5), 91),
    ] {
        let t = Transport::new([
            Step::Send(packet([80, 5, 136], 69, &p, 1), true),
            Step::Get(reply, count),
        ]);
        assert!(
            exchange_with_report_data_size(&t, &c, [80, 5, 136], &p, 1, 69, &|| Ok(())).is_err()
        );
        t.finish();
    }
}

#[test]
fn dynamic_exchange_propagates_read_failure_and_preserves_no_ack() {
    let c = cap(170);
    let t = Transport::new([
        Step::Send(packet([80, 5, 136], 69, &[3, 0, 0, 0, 0], 1), true),
        Step::ReadError,
    ]);
    assert!(
        exchange_with_report_data_size(&t, &c, [80, 5, 136], &[3, 0, 0, 0, 0], 1, 69, &|| Ok(()))
            .is_err()
    );
    t.finish();
}

#[test]
fn owned_extended_sleep_settings_do_not_mutate_caller_capability() {
    let mut c = cap(170);
    c.sleep_between_out_in_ms = 10;
    c.sleep_between_in_ms = 25;
    let t = Transport::new([]);
    let m = MouseOnboard::new(&t, &c).unwrap();
    assert_eq!(m.transport.sleep_between_out_in_ms, 10);
    assert_eq!(m.transport.sleep_between_in_ms, 25);
    assert_eq!(m.metadata_transport.sleep_between_out_in_ms, 20);
    assert_eq!(m.metadata_transport.sleep_between_in_ms, 25);
    assert_eq!(c.sleep_between_out_in_ms, 10);
    t.finish();
}

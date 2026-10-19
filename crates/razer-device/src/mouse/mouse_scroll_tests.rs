//! Source-pinned byte fixtures run the production executor with strict synthetic
//! Feature I/O. Every required exchange must be consumed; no hardware is opened.
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
    sent: Cell<usize>,
    reads: Cell<usize>,
}

impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
            sent: Cell::new(0),
            reads: Cell::new(0),
        }
    }
    fn finish(&self) {
        assert!(
            self.steps.borrow().is_empty(),
            "Required I/O was not consumed"
        );
    }
}

impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        let step = self.steps.borrow_mut().pop_front().expect("Unexpected OUT");
        let Step::Send(expected, success) = step else {
            panic!("Expected IN; received OUT");
        };
        assert_eq!(report, expected, "Exact source request and padding differ");
        self.sent.set(self.sent.get() + 1);
        ensure!(success, "Synthetic send failure");
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        // The prefix is the product's GET selector, checked in device_query_tests.
        assert_eq!(report.len(), 91);
        assert_eq!(&report[1..], [0; 90], "Payload buffer must start empty");
        self.reads.set(self.reads.get() + 1);
        match self.steps.borrow_mut().pop_front().expect("Unexpected IN") {
            Step::Get(response, count) => {
                assert!(response.len() <= report.len());
                report[..response.len()].copy_from_slice(&response);
                Ok(count)
            }
            Step::ReadError => anyhow::bail!("Synthetic read failure/timeout"),
            Step::Send(_, _) => panic!("Expected OUT; received IN"),
        }
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn packet(header: [u8; 3], payload: &[u8], transaction: u8) -> Vec<u8> {
    // Independent vendor layout: report ID, status, transaction, remaining[2],
    // reserved, payload size, class, command, data[80], checksum, reserved.
    assert!(payload.len() <= 80);
    let mut bytes = vec![0; 91];
    bytes[2] = transaction;
    bytes[6..9].copy_from_slice(&header);
    bytes[9..9 + payload.len()].copy_from_slice(payload);
    bytes[89] = bytes[3..89].iter().fold(0, |crc, byte| crc ^ byte);
    bytes
}

fn reply(header: [u8; 3], payload: &[u8], transaction: u8, status: u8) -> Vec<u8> {
    let mut bytes = packet(header, payload, transaction);
    bytes[1] = status;
    bytes[6] = payload.len() as u8;
    bytes[89] = bytes[3..89].iter().fold(0, |crc, byte| crc ^ byte);
    bytes
}

fn send(header: [u8; 3], payload: &[u8], transaction: u8) -> Step {
    Step::Send(packet(header, payload, transaction), true)
}
fn receive(header: [u8; 3], payload: &[u8], transaction: u8, status: u8) -> Step {
    Step::Get(reply(header, payload, transaction, status), 91)
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

fn current_cap(product_id: u32) -> DeviceReadCapability {
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
    let mut next = 0;
    move || {
        next += 1;
        Ok(next)
    }
}

fn observed() -> ScrollProfileObservation {
    ScrollProfileObservation {
        selected_obm_profile_id: Some(3),
        first_matching_obm_slot_id: Some(5),
        matching_obm_profile_ids: vec![1, 5, 7],
    }
}

fn single_change_script(set: [u8; 3], get: [u8; 3], profiles: &[u8], raw: u8) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut transaction = 1;
    for &profile in profiles {
        steps.push(send(set, &[profile, raw], transaction));
        steps.push(receive(set, &[profile, raw], transaction, 2));
        transaction += 1;
    }
    for &profile in profiles {
        steps.push(send(get, &[profile], transaction));
        steps.push(receive(get, &[profile, raw], transaction, 2));
        transaction += 1;
    }
    steps
}

fn audit() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../testdata/re/mouse-scroll-current-source.json"
    ))
    .unwrap()
}

#[test]
fn source_gates_do_not_expand_from_shared_methods() {
    let audit = audit();
    assert_eq!(audit["schema_version"], 2);
    assert_eq!(
        audit["implemented_product_ids"],
        serde_json::json!([153, 170, 203, 204, 214, 226, 45066])
    );
    let products = audit["products"].as_array().unwrap();
    assert_eq!(products.len(), 7);
    for product in products {
        let id = product["product_id"].as_u64().unwrap() as u32;
        let gated = capability(id);
        assert_eq!(
            gated.is_some(),
            [153, 170, 203, 204, 214, 226, 45066].contains(&id)
        );
        if let Some(gated) = gated {
            assert_eq!(gated.product_id(), id);
            assert_eq!(
                gated.supports_levels(),
                product["gate"]["levels"].as_bool().unwrap()
            );
            assert_eq!(gated.supports_mode_v2(), id == 226);
            for layer in ["mw", "ui"] {
                let layer = &product["layers"][layer];
                assert!(
                    layer["original_inputs"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|source| {
                            source["path"]
                                .as_str()
                                .unwrap()
                                .starts_with(".analysis/inputs/frontend/")
                                && source["sha256"].as_str().unwrap().len() == 64
                        })
                );
                // Capture failures remain explicit even when the independent
                // query successfully parses the retained source bytes.
                assert!(layer["capture_failures"].is_array());
            }
        }
    }
    assert!(capability(42).is_none());
    // Haptic scrolling has its own command and UI chain.
    assert!(capability(231).is_none());
}

#[test]
fn typed_commands_match_receipt_pinned_source_literals() {
    let pairs = [
        (
            ScrollReadKind::LegacyMode,
            "getScrollFreeNotchedMode",
            "setScrollFreeNotchedMode",
        ),
        (
            ScrollReadKind::ThreeMode,
            "getScrollTactileFreeSpinMicroTactileMode",
            "setScrollTactileFreeSpinMicroTactileMode",
        ),
        (
            ScrollReadKind::EnabledModes,
            "getLKScrollWheelModeSelectedInformation",
            "setLKScrollWheelModeSelectedInformation",
        ),
        (
            ScrollReadKind::VirtualAcceleration,
            "getScrollVirtualAccelerationState",
            "setScrollVirtualAccelerationState",
        ),
        (
            ScrollReadKind::SmartReel,
            "getScrollModeAutoSwitchState",
            "setScrollModeAutoSwitchState",
        ),
        (
            ScrollReadKind::VirtualAccelerationLevel,
            "getScrollVirtualAccelerationLevel",
            "setScrollVirtualAccelerationLevel",
        ),
        (
            ScrollReadKind::SmartReelThreshold,
            "getScrollWheelSmartReelModeThreshold",
            "setScrollWheelSmartReelModeThreshold",
        ),
    ];
    for product in audit()["products"].as_array().unwrap() {
        let Some(gated) = capability(product["product_id"].as_u64().unwrap() as u32) else {
            continue;
        };
        for (kind, get, set) in pairs {
            for (name, write) in [(get, false), (set, true)] {
                if let Ok(command) = gated.command(kind, write) {
                    let method = product["methods"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|method| method["name"] == name)
                        .unwrap();
                    assert_eq!(serde_json::to_value(command).unwrap(), method["command"]);
                    assert_eq!(method["receipt"]["sha256"].as_str().unwrap().len(), 64);
                }
            }
        }
        for (name, command) in [
            ("getMaxProfilesSupported", [1, 5, 138]),
            ("getNumberOfProfiles", [1, 5, 128]),
            ("getProfileIDList", [80, 5, 129]),
            ("getProfileName", [80, 5, 136]),
        ] {
            let method = product["methods"]
                .as_array()
                .unwrap()
                .iter()
                .find(|method| method["name"] == name)
                .unwrap();
            assert_eq!(method["command"], serde_json::json!(command));
            assert!(
                method["header_receipt"]["source"]
                    .as_str()
                    .unwrap()
                    .contains(&format!(
                        "new Uint8Array([{},{},{}])",
                        command[0], command[1], command[2]
                    ))
            );
        }
    }
}

#[test]
fn source_receipts_cover_profile_restore_and_actual_guid_association() {
    for product in audit()["products"].as_array().unwrap() {
        let product_id = product["product_id"].as_u64().unwrap();
        let chains = product["lifecycle_association_chains"].as_array().unwrap();
        assert_eq!(
            chains.len(),
            6 + usize::from(matches!(product_id, 226 | 45066))
        );
        let roles: std::collections::BTreeSet<_> = chains
            .iter()
            .map(|chain| chain["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles.len(), chains.len(), "Duplicate lifecycle owner");
        for chain in chains {
            let source = chain["source"].as_str().unwrap();
            let start = chain["byte_start"].as_u64().unwrap();
            let end = chain["byte_end"].as_u64().unwrap();
            assert_eq!(end - start, source.len() as u64);
            assert!(start < end && end <= chain["original"]["bytes"].as_u64().unwrap());
            assert_eq!(chain["source_sha256"].as_str().unwrap().len(), 64);
            assert!(
                product["layers"]["mw"]["original_inputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|original| original["path"] == chain["original"]["path"]
                        && original["sha256"] == chain["original"]["sha256"])
            );
        }
        let source = |role: &str| {
            chains.iter().find(|chain| chain["role"] == role).unwrap()["source"]
                .as_str()
                .unwrap()
        };
        let inventory = source("ordered_profile_inventory");
        let max = inventory.find("getMaxProfilesSupported()").unwrap();
        let count = inventory.find("getNumberOfProfiles()").unwrap();
        let ids = inventory.find("getProfileIDList()").unwrap();
        let metadata = inventory.find("getProfileNameStructure(").unwrap();
        assert!(max < count && count < ids && ids < metadata);
        assert!(inventory.contains("0!=="));
        let reader = source("profile_guid_reader");
        assert!(reader.contains("250;") && reader.contains("getProfileName("));
        assert!(reader.contains("00000000-0000-0000-0000-000000000000"));
        assert!(reader.contains("ffffffff-ffff-ffff-ffff-ffffffffffff"));
        let decoder = source("profile_guid_decoder");
        assert!(decoder.contains("e[3],e[2]") && decoder.contains("slice(8,16)"));
        let smart_reel = source("smart_reel_guid_maker");
        assert!(smart_reel.contains("obmJson.profiles.forEach"));
        assert!(smart_reel.contains(".guid===") && smart_reel.contains("1!==e.id"));
        assert!(smart_reel.contains("nextVersion") && smart_reel.contains("\"SmartReel\""));
        let restore = source("profile_restore");
        assert!(restore.contains("smartReelEnabled") && restore.contains("accelerationEnabled"));
        assert!(
            restore.rfind("restoreDeviceData").unwrap() > restore.find("smartReelEnabled").unwrap()
        );
        assert!(source("scroll_mode_maker").contains("nextVersion"));
        if product_id == 226 {
            let restore = source("mode_v2_restore");
            assert!(restore.contains("setScrollDisabledModes(r)"));
            assert!(restore.contains("updateScrollModeV2UI({scrollMode:a,disabledModes:r})"));
        }
        if product_id == 45066 {
            let matching = source("first_matching_slot");
            assert!(matching.contains("profiles.find((t=>t.guid===e))"));
            assert!(matching.contains(".slotId") && matching.contains("return null"));
            assert!(source("scroll_mode_maker").contains("[1,oo(r.guid)].filter"));
        }
    }
}

#[test]
fn enabled_modes_reject_empty_or_unknown_bits_and_handle_duplicate_disables() {
    for mask in 1..=7 {
        let modes = EnabledScrollModes::new(mask).unwrap();
        assert_eq!(modes.mask(), mask);
        assert_eq!(modes.contains(ScrollMode::Tactile), mask & 1 != 0);
        assert_eq!(modes.contains(ScrollMode::FreeSpin), mask & 2 != 0);
        assert_eq!(modes.contains(ScrollMode::MicroTactile), mask & 4 != 0);
    }
    for mask in [0, 8, 128, 255] {
        assert!(EnabledScrollModes::new(mask).is_err());
    }
    assert_eq!(
        EnabledScrollModes::from_disabled(&[ScrollMode::Tactile, ScrollMode::Tactile])
            .unwrap()
            .mask(),
        6
    );
    assert!(
        EnabledScrollModes::from_disabled(&[
            ScrollMode::Tactile,
            ScrollMode::FreeSpin,
            ScrollMode::MicroTactile
        ])
        .is_err()
    );
}

#[test]
fn constructor_rejects_unknown_products_and_incompatible_adapter() {
    let device = Transport::new([]);
    let mut transport = cap(170);
    for product in [42, 167, 231] {
        transport.product_id = product;
        assert!(MouseScroll::new(&device, &transport).is_err());
    }
    transport.product_id = 170;
    transport.source_class = "rzDevice30".into();
    assert!(MouseScroll::new(&device, &transport).is_err());
    device.finish();
}

#[test]
fn unsupported_or_incoherent_changes_fail_before_allocating_transactions() {
    let device = Transport::new([]);
    let mode_v2 = ScrollChange::ModeV2 {
        selected: ScrollMode::MicroTactile,
        enabled: EnabledScrollModes::new(3).unwrap(),
    };
    for (product, change) in [
        (170, ScrollChange::VirtualAccelerationLevel(2)),
        (170, ScrollChange::SmartReelThreshold(2)),
        (170, ScrollChange::LegacyMode(ScrollMode::MicroTactile)),
        (
            204,
            ScrollChange::ModeV2 {
                selected: ScrollMode::Tactile,
                enabled: EnabledScrollModes::new(7).unwrap(),
            },
        ),
        (226, ScrollChange::LegacyMode(ScrollMode::Tactile)),
        (226, mode_v2),
    ] {
        let transport = cap(product);
        let failure = MouseScroll::new(&device, &transport)
            .unwrap()
            .apply(
                change,
                &observed(),
                || panic!("Invalid changes must not allocate transactions"),
                || Ok(()),
            )
            .unwrap_err();
        assert_eq!(failure.phase, ScrollFailurePhase::Prepare);
        assert!(failure.progress.steps.is_empty());
        assert!(!failure.device_state_may_have_changed());
    }
    device.finish();
}

#[test]
fn actual_profile_ids_cannot_be_zero_or_duplicated() {
    let device = Transport::new([]);
    let transport = cap(204);
    let scroll = MouseScroll::new(&device, &transport).unwrap();
    for profiles in [
        ScrollProfileObservation {
            matching_obm_profile_ids: vec![0],
            ..observed()
        },
        ScrollProfileObservation {
            matching_obm_profile_ids: vec![3, 3],
            ..observed()
        },
        ScrollProfileObservation {
            first_matching_obm_slot_id: Some(0),
            ..observed()
        },
    ] {
        let failure = scroll
            .apply(
                ScrollChange::SmartReel(true),
                &profiles,
                || panic!("Invalid profiles must not allocate a transaction"),
                || Ok(()),
            )
            .unwrap_err();
        assert_eq!(failure.phase, ScrollFailurePhase::Prepare);
        assert!(!failure.device_state_may_have_changed());
    }
    device.finish();
}

#[test]
fn selected_slot_none_zero_or_one_means_only_the_active_profile() {
    for selected_obm_profile_id in [None, Some(0), Some(1)] {
        let profiles = ScrollProfileObservation {
            selected_obm_profile_id,
            ..observed()
        };
        let device = Transport::new(single_change_script([2, 2, 0x16], [2, 2, 0x96], &[1], 1));
        let transport = cap(170);
        let report = MouseScroll::new(&device, &transport)
            .unwrap()
            .apply(
                ScrollChange::VirtualAcceleration(true),
                &profiles,
                transactions(),
                || Ok(()),
            )
            .unwrap();
        assert!(report.verified());
        assert_eq!(report.steps.len(), 1);
        device.finish();
    }
}

#[test]
fn every_source_gated_getter_decodes_real_reply_bytes() {
    let values = [
        (
            170,
            ScrollReadKind::LegacyMode,
            [2, 2, 0x94],
            0,
            ScrollValue::LegacyMode(ScrollMode::Tactile),
        ),
        (
            204,
            ScrollReadKind::LegacyMode,
            [2, 2, 0x94],
            1,
            ScrollValue::LegacyMode(ScrollMode::FreeSpin),
        ),
        (
            226,
            ScrollReadKind::ThreeMode,
            [2, 2, 0xB0],
            2,
            ScrollValue::ThreeMode(ScrollMode::MicroTactile),
        ),
        (
            226,
            ScrollReadKind::EnabledModes,
            [2, 21, 0x8B],
            5,
            ScrollValue::EnabledModes(EnabledScrollModes::new(5).unwrap()),
        ),
        (
            170,
            ScrollReadKind::VirtualAcceleration,
            [2, 2, 0x96],
            1,
            ScrollValue::VirtualAcceleration(true),
        ),
        (
            45066,
            ScrollReadKind::SmartReel,
            [80, 2, 0x97],
            0,
            ScrollValue::SmartReel(false),
        ),
        (
            204,
            ScrollReadKind::VirtualAccelerationLevel,
            [2, 2, 0xA2],
            255,
            ScrollValue::VirtualAccelerationLevel(255),
        ),
        (
            226,
            ScrollReadKind::SmartReelThreshold,
            [2, 2, 0xA3],
            0,
            ScrollValue::SmartReelThreshold(0),
        ),
    ];
    for (product, kind, header, raw, expected) in values {
        let device = Transport::new([send(header, &[3], 1), receive(header, &[3, raw], 1, 2)]);
        let transport = cap(product);
        assert_eq!(
            MouseScroll::new(&device, &transport)
                .unwrap()
                .read(3, kind, transactions(), || Ok(()))
                .unwrap(),
            expected
        );
        device.finish();
    }
}

#[test]
fn legacy_mode_writes_active_and_the_first_selected_slot_then_reads_both() {
    for product in [170, 204] {
        let device = Transport::new(single_change_script([2, 2, 0x14], [2, 2, 0x94], &[1, 3], 1));
        let transport = cap(product);
        let report = MouseScroll::new(&device, &transport)
            .unwrap()
            .apply(
                ScrollChange::LegacyMode(ScrollMode::FreeSpin),
                &observed(),
                transactions(),
                || Ok(()),
            )
            .unwrap();
        assert!(report.verified());
        assert_eq!(
            report
                .steps
                .iter()
                .map(|s| s.profile_id)
                .collect::<Vec<_>>(),
            [1, 3]
        );
        device.finish();
    }
}

#[test]
fn smart_reel_uses_all_engine_matches_and_eighty_byte_padded_requests() {
    for product in [170, 204, 226, 45066] {
        let device = Transport::new(single_change_script(
            [80, 2, 0x17],
            [80, 2, 0x97],
            &[1, 5, 7],
            1,
        ));
        let transport = cap(product);
        let report = MouseScroll::new(&device, &transport)
            .unwrap()
            .apply(
                ScrollChange::SmartReel(true),
                &observed(),
                transactions(),
                || Ok(()),
            )
            .unwrap();
        assert!(report.verified());
        assert_eq!(
            report
                .steps
                .iter()
                .map(|s| s.profile_id)
                .collect::<Vec<_>>(),
            [1, 5, 7]
        );
        assert_eq!(device.sent.get(), 6);
        device.finish();
    }
}

#[test]
fn acceleration_and_levels_have_product_specific_profile_scope() {
    for (change, set, get, raw) in [
        (
            ScrollChange::VirtualAcceleration(true),
            [2, 2, 0x16],
            [2, 2, 0x96],
            1,
        ),
        (
            ScrollChange::VirtualAccelerationLevel(255),
            [2, 2, 0x22],
            [2, 2, 0xA2],
            255,
        ),
        (
            ScrollChange::SmartReelThreshold(0),
            [2, 2, 0x23],
            [2, 2, 0xA3],
            0,
        ),
    ] {
        for product in [204, 226, 45066] {
            let profiles: &[u8] = if product == 45066 { &[1] } else { &[1, 3] };
            let device = Transport::new(single_change_script(set, get, profiles, raw));
            let transport = cap(product);
            let report = MouseScroll::new(&device, &transport)
                .unwrap()
                .apply(change, &observed(), transactions(), || Ok(()))
                .unwrap();
            assert!(report.verified());
            assert_eq!(
                report
                    .steps
                    .iter()
                    .map(|s| s.profile_id)
                    .collect::<Vec<_>>(),
                profiles
            );
            device.finish();
        }
    }
}

#[test]
fn older_mode_uses_first_cached_slot_rather_than_selected_or_all_engine_matches() {
    let device = Transport::new(single_change_script([2, 2, 0x14], [2, 2, 0x94], &[1, 5], 0));
    let transport = cap(45066);
    let report = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::LegacyMode(ScrollMode::Tactile),
            &observed(),
            transactions(),
            || Ok(()),
        )
        .unwrap();
    assert!(report.verified());
    assert_eq!(
        report
            .steps
            .iter()
            .map(|s| s.profile_id)
            .collect::<Vec<_>>(),
        [1, 5]
    );
    device.finish();
}

#[test]
fn v2_applies_mask_then_mode_for_each_profile_before_any_confirmation_getter() {
    let device = Transport::new([
        send([2, 21, 0x0B], &[1, 5], 1),
        receive([2, 21, 0x0B], &[1, 5], 1, 2),
        send([2, 2, 0x30], &[1, 2], 2),
        receive([2, 2, 0x30], &[1, 2], 2, 2),
        send([2, 21, 0x0B], &[3, 5], 3),
        receive([2, 21, 0x0B], &[3, 5], 3, 2),
        send([2, 2, 0x30], &[3, 2], 4),
        receive([2, 2, 0x30], &[3, 2], 4, 2),
        send([2, 21, 0x8B], &[1], 5),
        receive([2, 21, 0x8B], &[1, 5], 5, 2),
        send([2, 2, 0xB0], &[1], 6),
        receive([2, 2, 0xB0], &[1, 2], 6, 2),
        send([2, 21, 0x8B], &[3], 7),
        receive([2, 21, 0x8B], &[3, 5], 7, 2),
        send([2, 2, 0xB0], &[3], 8),
        receive([2, 2, 0xB0], &[3, 2], 8, 2),
    ]);
    let transport = cap(226);
    let report = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::ModeV2 {
                selected: ScrollMode::MicroTactile,
                enabled: EnabledScrollModes::new(5).unwrap(),
            },
            &observed(),
            transactions(),
            || Ok(()),
        )
        .unwrap();
    assert!(report.verified());
    assert_eq!(report.steps.len(), 4);
    assert_eq!(
        report.steps[0].requested,
        ScrollValue::EnabledModes(EnabledScrollModes::new(5).unwrap())
    );
    assert_eq!(
        report.steps[1].requested,
        ScrollValue::ThreeMode(ScrollMode::MicroTactile)
    );
    assert_eq!(report.steps[2].profile_id, 3);
    device.finish();
}

#[test]
fn empty_source_success_ack_requires_a_separate_successful_getter() {
    let device = Transport::new([
        send([2, 2, 0x22], &[1, 2], 1),
        receive([2, 2, 0x22], &[], 1, 2),
        send([2, 2, 0xA2], &[1], 2),
        receive([2, 2, 0xA2], &[1, 2], 2, 2),
    ]);
    let transport = cap(45066);
    let report = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::VirtualAccelerationLevel(2),
            &observed(),
            transactions(),
            || Ok(()),
        )
        .unwrap();
    assert!(report.verified());
    assert!(report.steps[0].acknowledgment_received);
    assert_eq!(report.steps[0].acknowledged, None);
    assert_eq!(
        report.steps[0].observed,
        Some(ScrollValue::VirtualAccelerationLevel(2))
    );
    device.finish();
}

#[test]
fn a_different_setter_echo_is_preserved_without_replacing_readback() {
    let device = Transport::new([
        send([2, 2, 0x16], &[1, 1], 1),
        receive([2, 2, 0x16], &[1, 0], 1, 2),
        send([2, 2, 0x96], &[1], 2),
        receive([2, 2, 0x96], &[1, 1], 2, 2),
    ]);
    let transport = cap(45066);
    let report = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::VirtualAcceleration(true),
            &observed(),
            transactions(),
            || Ok(()),
        )
        .unwrap();
    assert!(report.verified());
    assert_eq!(
        report.steps[0].acknowledged,
        Some(ScrollValue::VirtualAcceleration(false))
    );
    assert_eq!(
        report.steps[0].observed,
        Some(ScrollValue::VirtualAcceleration(true))
    );
    device.finish();
}

#[test]
fn differing_readback_is_an_error_with_both_observations_and_no_extra_write() {
    let device = Transport::new([
        send([2, 2, 0x16], &[1, 1], 1),
        receive([2, 2, 0x16], &[1, 1], 1, 2),
        send([2, 2, 0x96], &[1], 2),
        receive([2, 2, 0x96], &[1, 0], 2, 2),
    ]);
    let transport = cap(45066);
    let failure = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::VirtualAcceleration(true),
            &observed(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(failure.phase, ScrollFailurePhase::Readback);
    assert_eq!(failure.failed_step, Some(0));
    assert_eq!(
        failure.progress.steps[0].acknowledged,
        Some(ScrollValue::VirtualAcceleration(true))
    );
    assert_eq!(
        failure.progress.steps[0].observed,
        Some(ScrollValue::VirtualAcceleration(false))
    );
    assert!(!failure.progress.verified());
    assert!(failure.device_state_may_have_changed());
    device.finish();
}

#[test]
fn getter_transport_failure_retains_prior_ack_but_has_no_observed_value() {
    let device = Transport::new([
        send([2, 2, 0x16], &[1, 1], 1),
        receive([2, 2, 0x16], &[1, 1], 1, 2),
        send([2, 2, 0x96], &[1], 2),
        Step::ReadError,
    ]);
    let transport = cap(45066);
    let failure = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::VirtualAcceleration(true),
            &observed(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(failure.phase, ScrollFailurePhase::Readback);
    assert!(failure.progress.steps[0].acknowledgment_received);
    assert_eq!(failure.progress.steps[0].observed, None);
    assert!(!failure.progress.verified());
    assert!(failure.to_string().contains("read failure/timeout"));
    device.finish();
}

#[test]
fn a_partially_written_second_profile_stops_and_retains_completed_acks() {
    let device = Transport::new([
        send([2, 2, 0x16], &[1, 1], 1),
        receive([2, 2, 0x16], &[1, 1], 1, 2),
        send([2, 2, 0x16], &[3, 1], 2),
        Step::ReadError,
    ]);
    let transport = cap(170);
    let failure = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::VirtualAcceleration(true),
            &observed(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(failure.phase, ScrollFailurePhase::Write);
    assert_eq!(failure.failed_step, Some(1));
    assert_eq!(failure.progress.steps.len(), 2);
    assert_eq!(
        failure.progress.steps[0].acknowledged,
        Some(ScrollValue::VirtualAcceleration(true))
    );
    assert_eq!(failure.progress.steps[1].acknowledged, None);
    assert!(!failure.progress.steps[1].acknowledgment_received);
    assert!(failure.progress.steps.iter().all(|s| s.observed.is_none()));
    assert!(failure.device_state_may_have_changed());
    device.finish();
}

#[test]
fn v2_mode_failure_retains_the_prior_mask_ack_and_never_starts_a_later_profile() {
    let device = Transport::new([
        send([2, 21, 0x0B], &[1, 7], 1),
        receive([2, 21, 0x0B], &[1, 7], 1, 2),
        send([2, 2, 0x30], &[1, 2], 2),
        receive([2, 2, 0x30], &[], 2, 5),
    ]);
    let transport = cap(226);
    let failure = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::ModeV2 {
                selected: ScrollMode::MicroTactile,
                enabled: EnabledScrollModes::new(7).unwrap(),
            },
            &observed(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(failure.phase, ScrollFailurePhase::Write);
    assert_eq!(failure.failed_step, Some(1));
    assert_eq!(failure.progress.steps.len(), 2);
    assert!(failure.progress.steps[0].acknowledgment_received);
    assert!(!failure.progress.steps[1].acknowledgment_received);
    assert!(failure.to_string().contains("unsupported"));
    device.finish();
}

#[test]
fn partial_readback_failure_retains_already_observed_and_unobserved_profiles() {
    let device = Transport::new([
        send([2, 2, 0x16], &[1, 1], 1),
        receive([2, 2, 0x16], &[1, 1], 1, 2),
        send([2, 2, 0x16], &[3, 1], 2),
        receive([2, 2, 0x16], &[3, 1], 2, 2),
        send([2, 2, 0x96], &[1], 3),
        receive([2, 2, 0x96], &[1, 1], 3, 2),
        send([2, 2, 0x96], &[3], 4),
        Step::ReadError,
    ]);
    let transport = cap(170);
    let failure = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::VirtualAcceleration(true),
            &observed(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(failure.phase, ScrollFailurePhase::Readback);
    assert_eq!(failure.failed_step, Some(1));
    assert_eq!(
        failure.progress.steps[0].observed,
        Some(ScrollValue::VirtualAcceleration(true))
    );
    assert_eq!(failure.progress.steps[1].observed, None);
    assert!(
        failure
            .progress
            .steps
            .iter()
            .all(|s| s.acknowledgment_received)
    );
    assert!(!failure.progress.verified());
    device.finish();
}

#[test]
fn cancellation_before_submission_allocates_nothing_and_does_not_report_a_write() {
    let device = Transport::new([]);
    let transport = cap(170);
    let failure = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::SmartReel(true),
            &observed(),
            || panic!("Canceled task allocated a transaction"),
            || anyhow::bail!("Canceled before submission"),
        )
        .unwrap_err();
    assert_eq!(failure.phase, ScrollFailurePhase::Prepare);
    assert!(!failure.device_state_may_have_changed());
    assert!(failure.progress.steps.is_empty());
    device.finish();
}

#[test]
fn cancellation_after_a_completed_ack_stops_before_the_next_profile_and_keeps_the_ack() {
    let device = Transport::new([
        send([2, 2, 0x16], &[1, 1], 1),
        receive([2, 2, 0x16], &[1, 1], 1, 2),
    ]);
    let calls = Cell::new(0);
    let transport = cap(170);
    let failure = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::VirtualAcceleration(true),
            &observed(),
            transactions(),
            || {
                // First validate after IN completes remains valid; cancellation is
                // observed before the next mutation is recorded or allocated.
                calls.set(calls.get() + 1);
                ensure!(calls.get() <= 6, "Canceled after completed ACK");
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(failure.phase, ScrollFailurePhase::Write);
    assert_eq!(failure.failed_step, None);
    assert_eq!(failure.progress.steps.len(), 1);
    assert!(failure.progress.steps[0].acknowledgment_received);
    assert_eq!(failure.progress.steps[0].observed, None);
    assert_eq!(device.sent.get(), 1);
    device.finish();
}

#[test]
fn cancellation_immediately_after_io_reports_uncertain_hardware_state() {
    for after_read in [false, true] {
        let mut steps = vec![send([2, 2, 0x16], &[1, 1], 1)];
        if after_read {
            steps.push(receive([2, 2, 0x16], &[1, 1], 1, 2));
        }
        let device = Transport::new(steps);
        let transport = cap(45066);
        let failure = MouseScroll::new(&device, &transport)
            .unwrap()
            .apply(
                ScrollChange::VirtualAcceleration(true),
                &observed(),
                transactions(),
                || {
                    ensure!(
                        if after_read {
                            device.reads.get() == 0
                        } else {
                            device.sent.get() == 0
                        },
                        "Session invalidated during I/O"
                    );
                    Ok(())
                },
            )
            .unwrap_err();
        assert_eq!(failure.phase, ScrollFailurePhase::Write);
        assert!(failure.device_state_may_have_changed());
        assert_eq!(failure.progress.steps.len(), 1);
        assert!(!failure.progress.steps[0].acknowledgment_received);
        assert_eq!(failure.progress.steps[0].observed, None);
        device.finish();
    }
}

#[test]
fn busy_replies_retry_input_only_then_complete_without_resending_the_write() {
    let device = Transport::new([
        send([2, 2, 0x16], &[1, 1], 1),
        receive([2, 2, 0x16], &[], 1, 1),
        receive([2, 2, 0x16], &[1, 1], 1, 2),
        send([2, 2, 0x96], &[1], 2),
        receive([2, 2, 0x96], &[1, 1], 2, 2),
    ]);
    let mut transport = cap(45066);
    transport.max_retry_in = 2;
    assert!(
        MouseScroll::new(&device, &transport)
            .unwrap()
            .apply(
                ScrollChange::VirtualAcceleration(true),
                &observed(),
                transactions(),
                || Ok(())
            )
            .unwrap()
            .verified()
    );
    assert_eq!(device.sent.get(), 2);
    assert_eq!(device.reads.get(), 3);
    device.finish();
}

#[test]
fn busy_retry_budget_is_finite_and_does_not_start_readback() {
    let device = Transport::new([
        send([2, 2, 0x16], &[1, 1], 1),
        receive([2, 2, 0x16], &[], 1, 1),
        receive([2, 2, 0x16], &[], 1, 1),
        send([2, 2, 0x16], &[1, 1], 1),
        receive([2, 2, 0x16], &[], 1, 1),
        receive([2, 2, 0x16], &[], 1, 1),
    ]);
    let mut transport = cap(45066);
    transport.max_retry_in = 2;
    transport.max_retry_out = 2;
    let failure = MouseScroll::new(&device, &transport)
        .unwrap()
        .apply(
            ScrollChange::VirtualAcceleration(true),
            &observed(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(failure.phase, ScrollFailurePhase::Write);
    assert!(failure.to_string().contains("Device is busy"));
    assert_eq!(device.sent.get(), 2);
    assert_eq!(device.reads.get(), 4);
    assert_eq!(failure.progress.steps.len(), 1);
    assert!(failure.device_state_may_have_changed());
    device.finish();
}

#[test]
fn wrong_transaction_or_command_retries_the_same_query_with_source_limits() {
    for bad_reply in [
        receive([2, 2, 0x94], &[3, 1], 9, 2),
        receive([2, 2, 0x96], &[3, 1], 1, 2),
    ] {
        let device = Transport::new([
            send([2, 2, 0x94], &[3], 1),
            bad_reply,
            send([2, 2, 0x94], &[3], 1),
            receive([2, 2, 0x94], &[3, 1], 1, 2),
        ]);
        let mut transport = cap(170);
        transport.max_retry_out = 2;
        assert_eq!(
            MouseScroll::new(&device, &transport)
                .unwrap()
                .read(3, ScrollReadKind::LegacyMode, transactions(), || Ok(()))
                .unwrap(),
            ScrollValue::LegacyMode(ScrollMode::FreeSpin)
        );
        assert_eq!(device.sent.get(), 2);
        device.finish();
    }
}

#[test]
fn malformed_report_lengths_never_decode_padded_bytes_as_observations() {
    let valid = reply([2, 2, 0x94], &[3, 1], 1, 2);
    let mut oversized_payload = valid.clone();
    oversized_payload[6] = 81;
    for (response, count) in [
        (valid[..90].to_vec(), 90),
        (valid.clone(), 92),
        (oversized_payload, 91),
        (reply([2, 2, 0x94], &[3], 1, 2), 91),
        (reply([2, 2, 0x94], &[], 1, 2), 91),
    ] {
        let device = Transport::new([send([2, 2, 0x94], &[3], 1), Step::Get(response, count)]);
        let transport = cap(170);
        assert!(
            MouseScroll::new(&device, &transport)
                .unwrap()
                .read(3, ScrollReadKind::LegacyMode, transactions(), || Ok(()))
                .is_err()
        );
        device.finish();
    }
}

#[test]
fn wrong_profile_or_invalid_mode_state_and_mask_are_explicit_errors() {
    for (product, kind, command, payload) in [
        (170, ScrollReadKind::LegacyMode, [2, 2, 0x94], [4, 1]),
        (170, ScrollReadKind::LegacyMode, [2, 2, 0x94], [3, 2]),
        (226, ScrollReadKind::ThreeMode, [2, 2, 0xB0], [3, 3]),
        (226, ScrollReadKind::EnabledModes, [2, 21, 0x8B], [3, 0]),
        (226, ScrollReadKind::EnabledModes, [2, 21, 0x8B], [3, 8]),
        (
            170,
            ScrollReadKind::VirtualAcceleration,
            [2, 2, 0x96],
            [3, 2],
        ),
        (204, ScrollReadKind::SmartReel, [80, 2, 0x97], [3, 255]),
    ] {
        let device = Transport::new([send(command, &[3], 1), receive(command, &payload, 1, 2)]);
        let transport = cap(product);
        assert!(
            MouseScroll::new(&device, &transport)
                .unwrap()
                .read(3, kind, transactions(), || Ok(()))
                .is_err()
        );
        device.finish();
    }
}

#[test]
fn malformed_setter_echo_does_not_become_success_or_trigger_readback() {
    for payload in [&[4, 1][..], &[1][..], &[1, 2][..]] {
        let device = Transport::new([
            send([2, 2, 0x16], &[1, 1], 1),
            receive([2, 2, 0x16], payload, 1, 2),
        ]);
        let transport = cap(45066);
        let failure = MouseScroll::new(&device, &transport)
            .unwrap()
            .apply(
                ScrollChange::VirtualAcceleration(true),
                &observed(),
                transactions(),
                || Ok(()),
            )
            .unwrap_err();
        assert_eq!(failure.phase, ScrollFailurePhase::Write);
        assert!(failure.progress.steps[0].acknowledgment_received);
        assert_eq!(failure.progress.steps[0].acknowledged, None);
        assert_eq!(failure.progress.steps[0].observed, None);
        assert!(!failure.progress.verified());
        device.finish();
    }
}

#[test]
fn transport_send_or_receive_errors_obey_the_finite_out_retry_budget() {
    for send_failure in [false, true] {
        let mut steps = Vec::new();
        for _ in 0..2 {
            steps.push(Step::Send(packet([2, 2, 0x16], &[1, 1], 1), !send_failure));
            if !send_failure {
                steps.push(Step::ReadError);
            }
        }
        let device = Transport::new(steps);
        let mut transport = cap(45066);
        transport.max_retry_out = 2;
        let failure = MouseScroll::new(&device, &transport)
            .unwrap()
            .apply(
                ScrollChange::VirtualAcceleration(true),
                &observed(),
                transactions(),
                || Ok(()),
            )
            .unwrap_err();
        assert_eq!(failure.phase, ScrollFailurePhase::Write);
        assert_eq!(device.sent.get(), 2);
        assert!(failure.device_state_may_have_changed());
        assert!(!failure.progress.steps[0].acknowledgment_received);
        device.finish();
    }
}

#[test]
fn failure_and_unknown_statuses_never_create_acknowledged_values() {
    for status in [0, 3, 4, 5, 9, 255] {
        let device = Transport::new([
            send([2, 2, 0x16], &[1, 1], 1),
            receive([2, 2, 0x16], &[1, 1], 1, status),
        ]);
        let transport = cap(45066);
        let failure = MouseScroll::new(&device, &transport)
            .unwrap()
            .apply(
                ScrollChange::VirtualAcceleration(true),
                &observed(),
                transactions(),
                || Ok(()),
            )
            .unwrap_err();
        assert_eq!(failure.phase, ScrollFailurePhase::Write);
        assert!(!failure.progress.steps[0].acknowledgment_received);
        assert_eq!(failure.progress.steps[0].acknowledged, None);
        device.finish();
    }
}

#[test]
fn unsupported_getters_transactions_and_invalid_packet_capabilities_send_nothing() {
    let device = Transport::new([]);
    let transport = cap(170);
    let scroll = MouseScroll::new(&device, &transport).unwrap();
    for kind in [
        ScrollReadKind::ThreeMode,
        ScrollReadKind::EnabledModes,
        ScrollReadKind::SmartReelThreshold,
        ScrollReadKind::VirtualAccelerationLevel,
    ] {
        assert!(
            scroll
                .read(
                    1,
                    kind,
                    || panic!("Unsupported getter allocated transaction"),
                    || Ok(())
                )
                .is_err()
        );
    }
    assert!(
        scroll
            .read(0, ScrollReadKind::LegacyMode, transactions(), || Ok(()))
            .is_err()
    );
    for transaction in [31, 32, 128, 255] {
        assert!(
            scroll
                .read(1, ScrollReadKind::LegacyMode, || Ok(transaction), || Ok(()))
                .is_err()
        );
    }
    for index in 0..4 {
        let mut transport = cap(170);
        match index {
            0 => transport.report_bytes = 90,
            1 => transport.transaction_modulus = 0,
            2 => transport.max_retry_in = 0,
            3 => transport.max_retry_out = 0,
            _ => unreachable!(),
        }
        assert!(
            MouseScroll::new(&device, &transport)
                .unwrap()
                .read(1, ScrollReadKind::LegacyMode, transactions(), || Ok(()))
                .is_err()
        );
    }
    device.finish();
}

#[test]
fn canceled_getter_before_and_after_receive_never_returns_a_hardware_observation() {
    let device = Transport::new([]);
    let transport = cap(170);
    assert!(
        MouseScroll::new(&device, &transport)
            .unwrap()
            .read(
                1,
                ScrollReadKind::LegacyMode,
                || panic!("Canceled getter allocated a transaction"),
                || anyhow::bail!("Canceled before getter")
            )
            .is_err()
    );
    device.finish();
    let device = Transport::new([
        send([2, 2, 0x94], &[3], 1),
        receive([2, 2, 0x94], &[3, 1], 1, 2),
    ]);
    assert!(
        MouseScroll::new(&device, &transport)
            .unwrap()
            .read(3, ScrollReadKind::LegacyMode, transactions(), || {
                ensure!(device.reads.get() == 0, "Canceled during receive");
                Ok(())
            })
            .is_err()
    );
    device.finish();
}

#[test]
fn empty_write_report_is_never_verified() {
    assert!(!ScrollWriteReport::default().verified());
}

#[test]
fn newly_audited_wired_products_execute_the_current_factory_commands() {
    for product_id in [153, 203, 214] {
        let mut transport = crate::mouse_current_dpi::capability(product_id)
            .unwrap()
            .transport()
            .for_physical_product(product_id)
            .unwrap();
        transport.sleep_between_out_ms = 0;
        transport.sleep_between_out_in_ms = 0;
        transport.sleep_between_in_ms = 0;
        transport.max_retry_out = 1;
        transport.max_retry_in = 1;
        let script = single_change_script([2, 2, 20], [2, 2, 148], &[1], 1);
        let device = Transport::new(script);
        let request = Submission::new(
            vec![ScrollChange::LegacyMode(ScrollMode::FreeSpin)],
            Default::default(),
        );
        let report = MouseScroll::new(&device, &transport)
            .unwrap()
            .submit(&request, transactions(), || Ok(()))
            .unwrap();
        assert!(report.verified_for(product_id));
        assert_eq!(report.requested, request);
        device.finish();
    }
}

#[test]
fn submission_confirmation_requires_the_exact_profiles_values_and_source_order() {
    let enabled = EnabledScrollModes::new(5).unwrap();
    let requested = Submission::new(
        vec![ScrollChange::ModeV2 {
            selected: ScrollMode::MicroTactile,
            enabled,
        }],
        ScrollProfileObservation {
            selected_obm_profile_id: Some(3),
            ..Default::default()
        },
    );
    let steps: Vec<_> = [1, 3]
        .into_iter()
        .flat_map(|profile_id| {
            [
                ScrollValue::EnabledModes(enabled),
                ScrollValue::ThreeMode(ScrollMode::MicroTactile),
            ]
            .into_iter()
            .map(move |requested| ScrollWriteStep {
                profile_id,
                requested,
                acknowledgment_received: true,
                acknowledged: None,
                observed: Some(requested),
            })
        })
        .collect();
    let original = SubmissionReport {
        requested,
        onboard_profiles: None,
        reports: vec![ScrollWriteReport { steps }],
        errors: Vec::new(),
    };
    assert!(original.verified_for(226));
    assert!(!original.verified_for(204));
    let mut different_echo = original.clone();
    different_echo.reports[0].steps[0].acknowledged = Some(ScrollValue::EnabledModes(
        EnabledScrollModes::new(7).unwrap(),
    ));
    assert!(
        different_echo.verified_for(226),
        "independent source readback confirms the requested value even when setter echo differs"
    );
    for count in 0..original.reports[0].steps.len() {
        let mut partial = original.clone();
        partial.reports[0].steps.truncate(count);
        assert!(partial.validate_for(226).is_ok());
        assert!(!partial.verified_for(226));
    }
    for failure in 0..7 {
        let mut report = original.clone();
        let steps = &mut report.reports[0].steps;
        match failure {
            0 => steps[1].profile_id = 3,
            1 => steps.swap(0, 1),
            2 => {
                steps.pop();
            }
            3 => {
                steps.push(steps[0].clone());
            }
            4 => {
                steps[0].observed = Some(ScrollValue::EnabledModes(
                    EnabledScrollModes::new(7).unwrap(),
                ))
            }
            5 => {
                steps[0].requested = ScrollValue::VirtualAcceleration(true);
                steps[0].observed = Some(steps[0].requested);
            }
            6 => steps[0].acknowledgment_received = false,
            _ => unreachable!(),
        }
        assert!(
            !report.verified_for(226),
            "accepted unrelated confirmation {failure}"
        );
        assert_eq!(
            report.validate_for(226).is_ok(),
            matches!(failure, 2 | 4),
            "malformed association and legitimate partial progress differ: {failure}"
        );
    }
}

#[test]
fn mouse_scroll_partial_validation_preserves_missing_and_mismatching_typed_observations() {
    let original = SubmissionReport {
        requested: Submission::new(
            vec![
                ScrollChange::VirtualAcceleration(true),
                ScrollChange::SmartReel(true),
            ],
            Default::default(),
        ),
        onboard_profiles: None,
        reports: vec![
            ScrollWriteReport {
                steps: vec![ScrollWriteStep {
                    profile_id: 1,
                    requested: ScrollValue::VirtualAcceleration(true),
                    acknowledgment_received: false,
                    acknowledged: None,
                    observed: None,
                }],
            },
            ScrollWriteReport {
                steps: vec![ScrollWriteStep {
                    profile_id: 1,
                    requested: ScrollValue::SmartReel(true),
                    acknowledgment_received: true,
                    acknowledged: None,
                    observed: Some(ScrollValue::SmartReel(true)),
                }],
            },
        ],
        errors: vec!["first task failed".into()],
    };
    assert!(original.validate_for(170).is_ok());
    assert!(!original.verified_for(170));
    assert!(original.reports[1].verified());
    let mut mismatch = original.clone();
    mismatch.reports[0].steps[0].acknowledgment_received = true;
    mismatch.reports[0].steps[0].observed = Some(ScrollValue::VirtualAcceleration(false));
    assert!(mismatch.validate_for(170).is_ok());
    assert!(!mismatch.verified_for(170));
    for failure in 0..3 {
        let mut malformed = original.clone();
        match failure {
            0 => {
                malformed.reports[0].steps[0].observed =
                    Some(ScrollValue::VirtualAcceleration(true))
            }
            1 => {
                malformed.reports[1].steps[0].observed =
                    Some(ScrollValue::VirtualAcceleration(true))
            }
            2 => malformed.reports[1].steps[0].requested = ScrollValue::SmartReel(false),
            _ => unreachable!(),
        }
        assert!(malformed.validate_for(170).is_err());
    }
}

#[test]
fn canceling_between_source_tasks_does_not_send_the_next_mutation() {
    let transport = cap(170);
    let device = Transport::new(single_change_script([2, 2, 22], [2, 2, 150], &[1], 1));
    let request = Submission::new(
        vec![
            ScrollChange::VirtualAcceleration(true),
            ScrollChange::SmartReel(true),
        ],
        Default::default(),
    );
    let result =
        MouseScroll::new(&device, &transport)
            .unwrap()
            .submit(&request, transactions(), || {
                ensure!(device.reads.get() < 2, "Canceled before the second task");
                Ok(())
            });
    assert!(result.is_err());
    assert_eq!(device.sent.get(), 2);
    device.finish();
}

#[test]
fn failed_source_task_preserves_progress_and_does_not_discard_an_independent_edit() {
    let transport = cap(170);
    let device = Transport::new([
        send([2, 2, 22], &[1, 1], 1),
        receive([2, 2, 22], &[], 1, 3),
        send([80, 2, 23], &[1, 1], 2),
        receive([80, 2, 23], &[1, 1], 2, 2),
        send([80, 2, 151], &[1], 3),
        receive([80, 2, 151], &[1, 1], 3, 2),
    ]);
    let request = Submission::new(
        vec![
            ScrollChange::VirtualAcceleration(true),
            ScrollChange::SmartReel(true),
        ],
        Default::default(),
    );
    let report = MouseScroll::new(&device, &transport)
        .unwrap()
        .submit(&request, transactions(), || Ok(()))
        .unwrap();
    assert!(!report.verified_for(170));
    assert_eq!(report.errors.len(), 1);
    assert_eq!(report.reports.len(), 2);
    assert!(!report.reports[0].steps[0].acknowledgment_received);
    assert!(report.reports[1].verified());
    assert!(report.validate_for(170).is_ok());
    device.finish();
}

fn profile_inventory_script() -> (Vec<Step>, u8) {
    let mut steps = Vec::new();
    let mut tx = 0;
    let mut exchange = |header: [u8; 3], payload: &[u8], response: &[u8]| {
        tx += 1;
        steps.push(send(header, payload, tx));
        steps.push(receive(header, response, tx, 2));
    };
    exchange([1, 5, 138], &[], &[8]);
    exchange([1, 5, 128], &[], &[4]);
    exchange([80, 5, 129], &[], &[4, 1, 7, 3, 5]);
    for profile_id in [7, 3, 5] {
        let mut metadata = [0; 250];
        metadata[..16].copy_from_slice(&[
            if profile_id == 3 { 0x79 } else { 0x78 },
            0x56,
            0x34,
            0x12,
            0xbc,
            0x9a,
            0xf0,
            0xde,
            0x12,
            0x34,
            0x56,
            0x78,
            0x9a,
            0xbc,
            0xde,
            0xf0,
        ]);
        for (offset, count) in [(0usize, 64usize), (64, 64), (128, 64), (192, 58)] {
            let offset_bytes = (offset as u16).to_be_bytes();
            let request = [profile_id, offset_bytes[0], offset_bytes[1], 0, 0];
            let mut response = vec![profile_id, offset_bytes[0], offset_bytes[1], 0, 250];
            response.extend_from_slice(&metadata[offset..offset + count]);
            exchange([(count + 5) as u8, 5, 136], &request, &response);
        }
    }
    (steps, tx)
}

fn append_change_script(
    steps: &mut Vec<Step>,
    tx: &mut u8,
    set: [u8; 3],
    get: [u8; 3],
    profiles: &[u8],
) {
    for profile in profiles {
        *tx += 1;
        steps.push(send(set, &[*profile, 1], *tx));
        steps.push(receive(set, &[], *tx, 2));
    }
    for profile in profiles {
        *tx += 1;
        steps.push(send(get, &[*profile], *tx));
        steps.push(receive(get, &[*profile, 1], *tx, 2));
    }
}

#[test]
fn actual_profile_guids_drive_all_smart_reel_matches_and_45066_first_mode_match() {
    for (product_id, change, set, get, expected) in [
        (
            153,
            ScrollChange::SmartReel(true),
            [80, 2, 23],
            [80, 2, 151],
            vec![1, 7, 5],
        ),
        (
            226,
            ScrollChange::SmartReel(true),
            [80, 2, 23],
            [80, 2, 151],
            vec![1, 7, 5],
        ),
        (
            45066,
            ScrollChange::LegacyMode(ScrollMode::FreeSpin),
            [2, 2, 20],
            [2, 2, 148],
            vec![1, 7],
        ),
    ] {
        let (mut steps, mut tx) = profile_inventory_script();
        append_change_script(&mut steps, &mut tx, set, get, &expected);
        let transport = Transport::new(steps);
        let capability = current_cap(product_id);
        let requested = Submission::for_profile(
            vec![change],
            "12345678-9abc-def0-1234-56789abcdef0".into(),
            Some(3),
        );
        let report = MouseScroll::new(&transport, &capability)
            .unwrap()
            .submit(&requested, transactions(), || Ok(()))
            .unwrap();
        assert_eq!(report.requested, requested);
        assert!(report.verified_for(product_id));
        assert_eq!(
            report.reports[0]
                .steps
                .iter()
                .map(|step| step.profile_id)
                .collect::<Vec<_>>(),
            expected
        );
        let observed = report.onboard_profiles.as_ref().unwrap();
        assert_eq!(
            observed
                .iter()
                .map(|profile| profile.profile_id)
                .collect::<Vec<_>>(),
            vec![7, 3, 5]
        );
        let mut stale_inventory = report.clone();
        stale_inventory.onboard_profiles.as_mut().unwrap()[0].guid = "other-profile".into();
        assert!(!stale_inventory.verified_for(product_id));
        let mut missing_inventory = report;
        missing_inventory.onboard_profiles = None;
        assert!(!missing_inventory.verified_for(product_id));
        transport.finish();
    }
}

#[test]
fn failed_obm_observation_does_not_fabricate_absence_or_block_independent_active_tasks() {
    let mut steps = vec![send([1, 5, 138], &[], 1), receive([1, 5, 138], &[], 1, 3)];
    append_change_script(&mut steps, &mut 1, [2, 2, 22], [2, 2, 150], &[1, 3]);
    let device = Transport::new(steps);
    let requested = Submission::for_profile(
        vec![
            ScrollChange::SmartReel(true),
            ScrollChange::VirtualAcceleration(true),
        ],
        "12345678-9abc-def0-1234-56789abcdef0".into(),
        Some(3),
    );
    let report = MouseScroll::new(&device, &cap(170))
        .unwrap()
        .submit(&requested, transactions(), || Ok(()))
        .unwrap();
    assert!(!report.verified_for(170));
    assert!(report.onboard_profiles.is_none());
    assert!(report.errors[0].contains("OBM profile observation"));
    assert!(report.reports[0].steps.is_empty());
    assert!(report.reports[1].verified());
    assert!(report.validate_for(170).is_ok());
    device.finish();
}

#[test]
fn profile_guid_requests_reject_fabricated_associations_and_empty_identity_before_io() {
    let mut request =
        Submission::for_profile(vec![ScrollChange::SmartReel(true)], String::new(), None);
    assert!(request.validate(226).is_err());
    request.profile_guid = Some("actual-profile".into());
    request.profiles.matching_obm_profile_ids = vec![7];
    assert!(request.validate(226).is_err());
}

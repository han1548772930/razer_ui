//! Strict synthetic Feature exchanges; no DLL, helper or hardware is opened.
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
        let Step::Send(expected, success) =
            self.steps.borrow_mut().pop_front().expect("Unexpected OUT")
        else {
            panic!("Expected IN, got OUT")
        };
        assert_eq!(
            report, expected,
            "Exact vendor header, payload, padding or CRC differs"
        );
        self.sent.set(self.sent.get() + 1);
        ensure!(success, "Synthetic send failure");
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        // The prefix is the product's GET selector, checked in device_query_tests.
        assert_eq!(report.len(), 91);
        assert_eq!(&report[1..], [0; 90]);
        self.reads.set(self.reads.get() + 1);
        match self.steps.borrow_mut().pop_front().expect("Unexpected IN") {
            Step::Get(response, count) => {
                assert!(response.len() <= report.len());
                report[..response.len()].copy_from_slice(&response);
                Ok(count)
            }
            Step::ReadError => anyhow::bail!("Synthetic read failure"),
            Step::Send(_, _) => panic!("Expected OUT, got IN"),
        }
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
fn packet(header: [u8; 3], payload: &[u8], transaction: u8) -> Vec<u8> {
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
    cap.max_retry_in = 1;
    cap.max_retry_out = 1;
    cap
}
fn transactions() -> impl FnMut() -> Result<u8> {
    let mut next = 0;
    move || {
        next = (next + 1) % 31;
        Ok(next)
    }
}
fn query(
    steps: &mut Vec<Step>,
    transaction: &mut u8,
    profile: u8,
    requested_region: u8,
    returned_region: u8,
    raw: u8,
) {
    if requested_region == 0 {
        steps.push(send([80, 15, 128], &[], *transaction));
        steps.push(receive(
            [80, 15, 128],
            &[returned_region, 7, 3, 1, 2],
            *transaction,
            2,
        ));
        *transaction += 1;
    }
    steps.push(send(
        [3, 15, 132],
        &[profile, returned_region],
        *transaction,
    ));
    steps.push(receive(
        [3, 15, 132],
        &[profile, returned_region, raw],
        *transaction,
        2,
    ));
    *transaction += 1;
}
fn mutation(steps: &mut Vec<Step>, transaction: &mut u8, profile: u8, region: u8, raw: u8) {
    steps.push(send([3, 15, 4], &[profile, region, raw], *transaction));
    steps.push(receive(
        [3, 15, 4],
        &[profile, region, raw],
        *transaction,
        2,
    ));
    *transaction += 1;
}
fn change(percent: u8, region_id: u8) -> BrightnessChange {
    BrightnessChange {
        is_enabled: true,
        percent,
        region_id,
    }
}
fn profiles(ids: &[u8]) -> BrightnessProfileObservation {
    BrightnessProfileObservation {
        active_guid: "active-guid".into(),
        observed_profiles: ids
            .iter()
            .map(|id| BrightnessProfileAssociation {
                guid: "active-guid".into(),
                slot_id: *id,
            })
            .collect(),
    }
}
fn audit() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../testdata/re/mouse-brightness-current-source.json"
    ))
    .unwrap()
}

#[test]
fn source_gates_and_typed_commands_match_current_receipts() {
    let evidence = audit();
    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(
        evidence["implemented_product_ids"],
        serde_json::json!([170, 204, 226, 45066])
    );
    assert_eq!(evidence["products"].as_array().unwrap().len(), 4);
    for product in evidence["products"].as_array().unwrap() {
        let id = product["product_id"].as_u64().unwrap() as u32;
        assert_eq!(capability(id).unwrap().product_id(), id);
        assert_eq!(product["source_class"], "rzDevice25DualLinkMouse");
        assert_eq!(
            product["gate"]["default_brightness"],
            serde_json::json!({"isEnabled":true,"value":33})
        );
        assert!(product["gate"]["ordinary_mouse_task"].as_bool().unwrap());
        assert!(product["gate"]["configured_brightness"].is_null());
        for (kind, header) in [("regions", REGIONS), ("get", GET), ("set", SET)] {
            let helper = product["helpers"]
                .as_array()
                .unwrap()
                .iter()
                .find(|h| h["kind"] == kind)
                .unwrap();
            assert_eq!(helper["command"], serde_json::to_value(header).unwrap());
            assert_eq!(helper["receipt"]["sha256"].as_str().unwrap().len(), 64);
        }
        assert!(
            product["task_runner_receipt"]["source"]
                .as_str()
                .unwrap()
                .contains("a.regionId!==s.regionId")
        );
        assert!(
            product["profileResolvers"][0]["receipt"]["source"]
                .as_str()
                .unwrap()
                .contains("t.push(n.slotId)")
        );
        assert!(
            product["obm_helper_receipt"]["source"]
                .as_str()
                .unwrap()
                .contains("\"ERROR\"")
        );
    }
    for id in [0, 42, 167, 175, 179, 231, 164, 241, 45057] {
        assert!(capability(id).is_none());
    }
}

#[test]
fn constructor_requires_the_audited_mouse_transport() {
    let device = Transport::new([]);
    let mut transport = cap(170);
    for id in [170, 204, 226, 45066] {
        transport = cap(id);
        assert_eq!(
            MouseBrightness::new(&device, &transport)
                .unwrap()
                .product_id(),
            id
        );
    }
    transport.product_id = 175;
    assert!(MouseBrightness::new(&device, &transport).is_err());
    transport.product_id = 170;
    transport.source_class = "rzDevice30".into();
    assert!(MouseBrightness::new(&device, &transport).is_err());
    device.finish();
}

#[test]
fn scaling_preserves_vendor_rounding_and_every_integral_percent_roundtrip() {
    for (percent, raw) in [
        (0, 0),
        (1, 2),
        (33, 84),
        (50, 127),
        (74, 188),
        (99, 252),
        (100, 255),
    ] {
        assert_eq!(percent_raw(percent), raw);
        assert_eq!(raw_percent(raw), percent);
    }
    for percent in 0..=100 {
        assert_eq!(raw_percent(percent_raw(percent)), percent);
    }
    for (raw, percent) in [(1, 1), (85, 34), (128, 51), (254, 100), (255, 100)] {
        assert_eq!(raw_percent(raw), percent);
    }
}

#[test]
fn guid_scope_preserves_all_matching_slots_in_source_order_including_one() {
    let observation = BrightnessProfileObservation {
        active_guid: "match".into(),
        observed_profiles: vec![
            BrightnessProfileAssociation {
                guid: "other".into(),
                slot_id: 9,
            },
            BrightnessProfileAssociation {
                guid: "match".into(),
                slot_id: 7,
            },
            BrightnessProfileAssociation {
                guid: "match".into(),
                slot_id: 1,
            },
            BrightnessProfileAssociation {
                guid: "match".into(),
                slot_id: 3,
            },
            BrightnessProfileAssociation {
                guid: "match".into(),
                slot_id: 7,
            },
        ],
    };
    assert_eq!(observation.matching_profile_ids().unwrap(), [7, 1, 3, 7]);
    assert!(profiles(&[0]).matching_profile_ids().is_err());
    let mut invalid = profiles(&[2]);
    invalid.active_guid.clear();
    assert!(invalid.matching_profile_ids().is_err());
    assert!(
        BrightnessProfileObservation::default()
            .matching_profile_ids()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn invalid_percent_profile_scope_and_cancelled_preparation_send_nothing() {
    let device = Transport::new([]);
    let transport = cap(170);
    let mouse = MouseBrightness::new(&device, &transport).unwrap();
    for (change, observation) in [
        (change(101, 0), profiles(&[])),
        (change(255, 4), profiles(&[])),
        (change(33, 0), profiles(&[0])),
    ] {
        let failure = mouse
            .apply(
                change,
                &observation,
                || panic!("Invalid preparation minted a transaction"),
                || Ok(()),
            )
            .unwrap_err();
        assert_eq!(failure.phase, BrightnessFailurePhase::Prepare);
        assert!(!failure.device_state_may_have_changed());
        assert!(failure.progress.steps.is_empty());
    }
    let failure = mouse
        .apply(
            change(33, 4),
            &profiles(&[]),
            || panic!("Cancelled task minted a transaction"),
            || anyhow::bail!("cancelled"),
        )
        .unwrap_err();
    assert_eq!(failure.phase, BrightnessFailurePhase::Prepare);
    device.finish();
}

#[test]
fn region_rows_keep_unknown_ids_types_and_empty_tables() {
    let device = Transport::new([
        send([80, 15, 128], &[], 1),
        receive([80, 15, 128], &[4, 7, 3, 1, 2, 250, 255, 254, 9, 8], 1, 2),
    ]);
    let transport = cap(170);
    let regions = MouseBrightness::new(&device, &transport)
        .unwrap()
        .read_regions(transactions(), || Ok(()))
        .unwrap();
    assert_eq!(
        regions,
        vec![
            BrightnessRegion {
                region_id: 4,
                update_rate: 7,
                data_type: 3,
                array_x: 1,
                array_y: 2
            },
            BrightnessRegion {
                region_id: 250,
                update_rate: 255,
                data_type: 254,
                array_x: 9,
                array_y: 8
            }
        ]
    );
    device.finish();
    let device = Transport::new([
        send([80, 15, 128], &[], 1),
        receive([80, 15, 128], &[], 1, 2),
    ]);
    assert!(
        MouseBrightness::new(&device, &transport)
            .unwrap()
            .read_regions(transactions(), || Ok(()))
            .unwrap()
            .is_empty()
    );
    device.finish();
}

#[test]
fn every_truncated_region_row_fails_without_brightness_query() {
    for length in [1, 2, 3, 4, 6, 7, 8, 9] {
        let payload = vec![4; length];
        let device = Transport::new([
            send([80, 15, 128], &[], 1),
            receive([80, 15, 128], &payload, 1, 2),
        ]);
        let transport = cap(204);
        assert!(
            MouseBrightness::new(&device, &transport)
                .unwrap()
                .read(1, 0, transactions(), || Ok(()))
                .is_err()
        );
        device.finish();
    }
}

#[test]
fn all_region_getter_queries_the_first_observed_region_for_every_gated_product() {
    for id in [170, 204, 226, 45066] {
        let device = Transport::new([
            send([80, 15, 128], &[], 1),
            receive([80, 15, 128], &[4, 7, 3, 1, 2, 1, 6, 2, 3, 4], 1, 2),
            send([3, 15, 132], &[7, 4], 2),
            receive([3, 15, 132], &[7, 4, 84], 2, 2),
        ]);
        let transport = cap(id);
        let observed = MouseBrightness::new(&device, &transport)
            .unwrap()
            .read(7, 0, transactions(), || Ok(()))
            .unwrap();
        assert_eq!(observed.requested_region_id, 0);
        assert_eq!(observed.queried_region_id, 4);
        assert_eq!(observed.value.percent, 33);
        let BrightnessRegionResolution::FirstAvailable { regions } = observed.resolution else {
            panic!("Missing real table")
        };
        assert_eq!(regions.len(), 2);
        device.finish();
    }
}

#[test]
fn explicit_region_read_skips_discovery() {
    let device = Transport::new([
        send([3, 15, 132], &[3, 250], 1),
        receive([3, 15, 132], &[3, 250, 254], 1, 2),
    ]);
    let transport = cap(226);
    let observed = MouseBrightness::new(&device, &transport)
        .unwrap()
        .read(3, 250, transactions(), || Ok(()))
        .unwrap();
    assert_eq!(observed.value.raw_value, 254);
    assert_eq!(observed.value.percent, 100);
    assert!(matches!(
        observed.resolution,
        BrightnessRegionResolution::Explicit
    ));
    device.finish();
}

#[test]
fn empty_region_list_keeps_the_original_all_region_query() {
    let device = Transport::new([
        send([80, 15, 128], &[], 1),
        receive([80, 15, 128], &[], 1, 2),
        send([3, 15, 132], &[1, 0], 2),
        receive([3, 15, 132], &[1, 0, 0], 2, 2),
    ]);
    let transport = cap(45066);
    let observed = MouseBrightness::new(&device, &transport)
        .unwrap()
        .read(1, 0, transactions(), || Ok(()))
        .unwrap();
    assert_eq!(observed.queried_region_id, 0);
    assert!(matches!(
        observed.resolution,
        BrightnessRegionResolution::EmptyRegionList
    ));
    device.finish();
}

#[test]
fn failed_region_query_uses_source_fallback_and_retains_its_error() {
    for query_failure in [Step::ReadError, receive([80, 15, 128], &[], 1, 3)] {
        let device = Transport::new([
            send([80, 15, 128], &[], 1),
            query_failure,
            send([3, 15, 132], &[1, 1], 2),
            receive([3, 15, 132], &[1, 1, 127], 2, 2),
        ]);
        let transport = cap(170);
        let observed = MouseBrightness::new(&device, &transport)
            .unwrap()
            .read(1, 0, transactions(), || Ok(()))
            .unwrap();
        assert_eq!(observed.queried_region_id, 1);
        assert_eq!(observed.value.percent, 50);
        let BrightnessRegionResolution::SourceFallback { error } = observed.resolution else {
            panic!("Fallback not recorded")
        };
        assert!(error.contains("retries exhausted"));
        device.finish();
    }
}

#[test]
fn cancelled_failed_region_query_never_starts_fallback_io() {
    let device = Transport::new([send([80, 15, 128], &[], 1), Step::ReadError]);
    let transport = cap(170);
    assert!(
        MouseBrightness::new(&device, &transport)
            .unwrap()
            .read(1, 0, transactions(), || {
                ensure!(device.reads.get() == 0, "cancelled during failed query");
                Ok(())
            })
            .is_err()
    );
    device.finish();
}

#[test]
fn explicit_active_skip_uses_observed_percent_without_requiring_canonical_raw() {
    let device = Transport::new([
        send([3, 15, 132], &[1, 4], 1),
        receive([3, 15, 132], &[1, 4, 85], 1, 2),
    ]);
    let transport = cap(170);
    let report = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(34, 4), &profiles(&[]), transactions(), || Ok(()))
        .unwrap();
    assert!(report.verified());
    assert!(report.source_completed);
    assert!(!report.steps[0].write_attempted);
    assert_eq!(
        report.steps[0].previous.as_ref().unwrap().value.raw_value,
        85
    );
    assert!(report.steps[0].observed.is_none());
    device.finish();
}

#[test]
fn default_active_region_difference_forces_all_region_write_even_at_equal_percent() {
    for id in [170, 204, 226, 45066] {
        let mut script = Vec::new();
        let mut tx = 1;
        query(&mut script, &mut tx, 1, 0, 4, 84);
        mutation(&mut script, &mut tx, 1, 0, 84);
        query(&mut script, &mut tx, 1, 0, 4, 84);
        let device = Transport::new(script);
        let transport = cap(id);
        let report = MouseBrightness::new(&device, &transport)
            .unwrap()
            .apply(change(33, 0), &profiles(&[]), transactions(), || Ok(()))
            .unwrap();
        assert!(report.verified());
        assert!(report.steps[0].write_attempted);
        assert_eq!(report.steps[0].requested_region_id, 0);
        device.finish();
    }
}

#[test]
fn default_active_can_skip_when_the_device_returns_all_region_and_same_percent() {
    let mut script = Vec::new();
    query(&mut script, &mut 1, 1, 0, 0, 84);
    let device = Transport::new(script);
    let transport = cap(204);
    let report = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(33, 0), &profiles(&[]), transactions(), || Ok(()))
        .unwrap();
    assert!(report.verified());
    assert!(!report.steps[0].write_attempted);
    device.finish();
}

#[test]
fn actual_guid_matches_use_all_region_and_finish_source_writes_before_any_readback() {
    let mut script = Vec::new();
    let mut tx = 1;
    query(&mut script, &mut tx, 1, 4, 4, 0);
    mutation(&mut script, &mut tx, 1, 4, 188);
    query(&mut script, &mut tx, 7, 0, 1, 0);
    mutation(&mut script, &mut tx, 7, 0, 188);
    query(&mut script, &mut tx, 1, 0, 1, 188); // Matching slot 1 is visited again; equal percent skips.
    query(&mut script, &mut tx, 3, 0, 1, 0);
    mutation(&mut script, &mut tx, 3, 0, 188);
    query(&mut script, &mut tx, 1, 4, 4, 188);
    query(&mut script, &mut tx, 7, 0, 1, 188);
    query(&mut script, &mut tx, 3, 0, 1, 188);
    let device = Transport::new(script);
    let transport = cap(226);
    let mut observation = profiles(&[7, 1, 3]);
    observation.observed_profiles.insert(
        1,
        BrightnessProfileAssociation {
            guid: "unrelated".into(),
            slot_id: 9,
        },
    );
    let report = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(74, 4), &observation, transactions(), || Ok(()))
        .unwrap();
    assert!(report.verified());
    assert_eq!(
        report
            .steps
            .iter()
            .map(|step| step.profile_id)
            .collect::<Vec<_>>(),
        [1, 7, 1, 3]
    );
    assert_eq!(report.steps[0].scope, BrightnessScope::Active);
    assert_eq!(report.steps[1].scope, BrightnessScope::Onboard);
    assert!(!report.steps[2].write_attempted);
    assert_eq!(report.steps[1].requested_region_id, 0);
    device.finish();
}

#[test]
fn disabled_brightness_writes_zero_and_never_persists_the_draft_percent() {
    let mut script = Vec::new();
    let mut tx = 1;
    query(&mut script, &mut tx, 1, 4, 4, 84);
    mutation(&mut script, &mut tx, 1, 4, 0);
    query(&mut script, &mut tx, 1, 4, 4, 0);
    let device = Transport::new(script);
    let transport = cap(45066);
    let report = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(
            BrightnessChange {
                is_enabled: false,
                percent: 33,
                region_id: 4,
            },
            &profiles(&[]),
            transactions(),
            || Ok(()),
        )
        .unwrap();
    assert!(report.verified());
    assert_eq!(report.steps[0].requested_percent, 0);
    assert_eq!(
        report.steps[0].observed.as_ref().unwrap().value.raw_value,
        0
    );
    device.finish();
}

#[test]
fn source_accepts_empty_setter_ack_and_independent_readback_confirms_the_value() {
    let device = Transport::new([
        send([3, 15, 132], &[1, 4], 1),
        receive([3, 15, 132], &[1, 4, 0], 1, 2),
        send([3, 15, 4], &[1, 4, 84], 2),
        receive([3, 15, 4], &[], 2, 2),
        send([3, 15, 132], &[1, 4], 3),
        receive([3, 15, 132], &[1, 4, 84], 3, 2),
    ]);
    let transport = cap(170);
    let report = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(33, 4), &profiles(&[]), transactions(), || Ok(()))
        .unwrap();
    assert!(report.verified());
    assert!(report.steps[0].acknowledgment_received);
    assert!(report.steps[0].acknowledged.is_none());
    device.finish();
}

#[test]
fn stale_setter_echo_does_not_replace_independent_hardware_confirmation() {
    let device = Transport::new([
        send([3, 15, 132], &[1, 4], 1),
        receive([3, 15, 132], &[1, 4, 0], 1, 2),
        send([3, 15, 4], &[1, 4, 84], 2),
        receive([3, 15, 4], &[1, 4, 0], 2, 2),
        send([3, 15, 132], &[1, 4], 3),
        receive([3, 15, 132], &[1, 4, 84], 3, 2),
    ]);
    let transport = cap(204);
    let report = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(33, 4), &profiles(&[]), transactions(), || Ok(()))
        .unwrap();
    assert!(report.verified());
    assert_eq!(report.steps[0].acknowledged.as_ref().unwrap().raw_value, 0);
    assert_eq!(
        report.steps[0].observed.as_ref().unwrap().value.raw_value,
        84
    );
    device.finish();
}

#[test]
fn exact_raw_readback_rejects_a_different_byte_with_the_same_rounded_percent() {
    let device = Transport::new([
        send([3, 15, 132], &[1, 4], 1),
        receive([3, 15, 132], &[1, 4, 0], 1, 2),
        send([3, 15, 4], &[1, 4, 86], 2),
        receive([3, 15, 4], &[1, 4, 86], 2, 2),
        send([3, 15, 132], &[1, 4], 3),
        receive([3, 15, 132], &[1, 4, 85], 3, 2),
    ]);
    let transport = cap(226);
    let failure = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(34, 4), &profiles(&[]), transactions(), || Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, BrightnessFailurePhase::Readback);
    assert!(failure.progress.source_completed);
    assert!(failure.device_state_may_have_changed());
    assert_eq!(
        failure.progress.steps[0]
            .observed
            .as_ref()
            .unwrap()
            .value
            .percent,
        34
    );
    assert_eq!(
        failure.progress.steps[0]
            .observed
            .as_ref()
            .unwrap()
            .value
            .raw_value,
        85
    );
    assert!(!failure.progress.verified());
    device.finish();
}

#[test]
fn onboard_failures_continue_later_profiles_and_retain_partial_acknowledgments() {
    let mut script = Vec::new();
    let mut tx = 1;
    query(&mut script, &mut tx, 1, 4, 4, 84);
    query(&mut script, &mut tx, 3, 0, 4, 0);
    script.push(send([3, 15, 4], &[3, 0, 84], tx));
    script.push(receive([3, 15, 4], &[], tx, 3));
    tx += 1;
    query(&mut script, &mut tx, 7, 0, 4, 0);
    mutation(&mut script, &mut tx, 7, 0, 84);
    // The source rejects after aggregate; Rust never invents a complete readback.
    let device = Transport::new(script);
    let transport = cap(170);
    let failure = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(33, 4), &profiles(&[3, 7]), transactions(), || Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, BrightnessFailurePhase::Onboard);
    assert_eq!(failure.failed_step, Some(1));
    assert_eq!(failure.progress.steps.len(), 3);
    assert!(failure.progress.steps[1].error.is_some());
    assert!(!failure.progress.steps[1].acknowledgment_received);
    assert!(failure.progress.steps[2].source_completed);
    assert!(failure.progress.steps[2].acknowledgment_received);
    assert!(failure.progress.steps[2].observed.is_none());
    assert!(!failure.progress.source_completed);
    assert!(failure.device_state_may_have_changed());
    device.finish();
}

#[test]
fn multiple_onboard_failures_are_all_reported_after_visiting_every_matching_slot() {
    let mut script = Vec::new();
    let mut tx = 1;
    query(&mut script, &mut tx, 1, 4, 4, 84);
    for profile in [3, 7] {
        query(&mut script, &mut tx, profile, 0, 4, 0);
        script.push(send([3, 15, 4], &[profile, 0, 84], tx));
        script.push(Step::ReadError);
        tx += 1;
    }
    let device = Transport::new(script);
    let transport = cap(45066);
    let failure = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(33, 4), &profiles(&[3, 7]), transactions(), || Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, BrightnessFailurePhase::Onboard);
    assert!(failure.to_string().contains("Profile 3"));
    assert!(failure.to_string().contains("Profile 7"));
    assert!(
        failure.progress.steps[1..]
            .iter()
            .all(|step| step.error.is_some() && step.write_attempted)
    );
    device.finish();
}

#[test]
fn active_read_failure_stops_before_obm_and_does_not_claim_a_write() {
    let device = Transport::new([send([3, 15, 132], &[1, 4], 1), Step::ReadError]);
    let transport = cap(170);
    let failure = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(33, 4), &profiles(&[7]), transactions(), || Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, BrightnessFailurePhase::Read);
    assert_eq!(failure.progress.steps.len(), 1);
    assert!(failure.progress.steps[0].previous.is_none());
    assert!(!failure.device_state_may_have_changed());
    device.finish();
}

#[test]
fn active_write_failure_stops_before_obm_and_retains_the_pre_read() {
    let device = Transport::new([
        send([3, 15, 132], &[1, 4], 1),
        receive([3, 15, 132], &[1, 4, 0], 1, 2),
        send([3, 15, 4], &[1, 4, 84], 2),
        Step::ReadError,
    ]);
    let transport = cap(204);
    let failure = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(33, 4), &profiles(&[7]), transactions(), || Ok(()))
        .unwrap_err();
    assert_eq!(failure.phase, BrightnessFailurePhase::Write);
    assert_eq!(failure.progress.steps.len(), 1);
    assert!(failure.progress.steps[0].previous.is_some());
    assert!(failure.device_state_may_have_changed());
    assert!(!failure.progress.steps[0].acknowledgment_received);
    device.finish();
}

#[test]
fn cancelled_onboard_write_stops_instead_of_following_the_ordinary_aggregate_branch() {
    let mut script = Vec::new();
    let mut tx = 1;
    query(&mut script, &mut tx, 1, 4, 4, 84);
    query(&mut script, &mut tx, 3, 0, 4, 0);
    script.push(send([3, 15, 4], &[3, 0, 84], tx));
    script.push(receive([3, 15, 4], &[3, 0, 84], tx, 2));
    let device = Transport::new(script);
    let transport = cap(226);
    let failure = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(33, 4), &profiles(&[3, 7]), transactions(), || {
            ensure!(device.reads.get() < 4, "cancelled after onboard receive");
            Ok(())
        })
        .unwrap_err();
    assert_eq!(failure.phase, BrightnessFailurePhase::Cancelled);
    assert_eq!(failure.progress.steps.len(), 2);
    assert!(failure.device_state_may_have_changed());
    assert!(!failure.progress.steps[1].acknowledgment_received);
    device.finish();
}

#[test]
fn cancelled_readback_keeps_source_completion_and_the_success_ack() {
    let device = Transport::new([
        send([3, 15, 132], &[1, 4], 1),
        receive([3, 15, 132], &[1, 4, 0], 1, 2),
        send([3, 15, 4], &[1, 4, 84], 2),
        receive([3, 15, 4], &[1, 4, 84], 2, 2),
        send([3, 15, 132], &[1, 4], 3),
        receive([3, 15, 132], &[1, 4, 84], 3, 2),
    ]);
    let transport = cap(170);
    let failure = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(33, 4), &profiles(&[]), transactions(), || {
            ensure!(device.reads.get() < 3, "cancelled during readback");
            Ok(())
        })
        .unwrap_err();
    assert_eq!(failure.phase, BrightnessFailurePhase::Cancelled);
    assert!(failure.progress.source_completed);
    assert!(failure.progress.steps[0].acknowledgment_received);
    assert!(failure.progress.steps[0].observed.is_none());
    device.finish();
}

#[test]
fn transaction_allocation_failure_before_setter_never_claims_an_attempted_write() {
    let device = Transport::new([
        send([3, 15, 132], &[1, 4], 1),
        receive([3, 15, 132], &[1, 4, 0], 1, 2),
    ]);
    let transport = cap(45066);
    let mut count = 0;
    let failure = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(
            change(33, 4),
            &profiles(&[]),
            || {
                count += 1;
                ensure!(count == 1, "allocator exhausted");
                Ok(1)
            },
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(failure.phase, BrightnessFailurePhase::Write);
    assert!(!failure.device_state_may_have_changed());
    assert!(failure.progress.steps[0].previous.is_some());
    device.finish();
}

#[test]
fn truncated_or_wrong_profile_and_region_reads_are_explicit_errors() {
    for payload in [
        &[][..],
        &[1][..],
        &[1, 4][..],
        &[2, 4, 84][..],
        &[1, 1, 84][..],
    ] {
        let device = Transport::new([
            send([3, 15, 132], &[1, 4], 1),
            receive([3, 15, 132], payload, 1, 2),
        ]);
        let transport = cap(170);
        assert!(
            MouseBrightness::new(&device, &transport)
                .unwrap()
                .read(1, 4, transactions(), || Ok(()))
                .is_err()
        );
        device.finish();
    }
}

#[test]
fn malformed_nonempty_setter_echo_keeps_success_status_without_claiming_completion() {
    for payload in [&[1][..], &[1, 4][..], &[2, 4, 84][..], &[1, 1, 84][..]] {
        let device = Transport::new([
            send([3, 15, 132], &[1, 4], 1),
            receive([3, 15, 132], &[1, 4, 0], 1, 2),
            send([3, 15, 4], &[1, 4, 84], 2),
            receive([3, 15, 4], payload, 2, 2),
        ]);
        let transport = cap(204);
        let failure = MouseBrightness::new(&device, &transport)
            .unwrap()
            .apply(change(33, 4), &profiles(&[]), transactions(), || Ok(()))
            .unwrap_err();
        assert_eq!(failure.phase, BrightnessFailurePhase::Write);
        assert!(failure.progress.steps[0].acknowledgment_received);
        assert!(failure.progress.steps[0].acknowledged.is_none());
        assert!(!failure.progress.source_completed);
        device.finish();
    }
}

#[test]
fn all_region_setter_can_echo_a_concrete_region() {
    let device = Transport::new([
        send([80, 15, 128], &[], 1),
        receive([80, 15, 128], &[4, 7, 3, 1, 2], 1, 2),
        send([3, 15, 132], &[1, 4], 2),
        receive([3, 15, 132], &[1, 4, 0], 2, 2),
        send([3, 15, 4], &[1, 0, 84], 3),
        receive([3, 15, 4], &[1, 4, 84], 3, 2),
        send([80, 15, 128], &[], 4),
        receive([80, 15, 128], &[4, 7, 3, 1, 2], 4, 2),
        send([3, 15, 132], &[1, 4], 5),
        receive([3, 15, 132], &[1, 4, 84], 5, 2),
    ]);
    let transport = cap(226);
    let report = MouseBrightness::new(&device, &transport)
        .unwrap()
        .apply(change(33, 0), &profiles(&[]), transactions(), || Ok(()))
        .unwrap();
    assert!(report.verified());
    assert_eq!(report.steps[0].acknowledged.as_ref().unwrap().region_id, 4);
    device.finish();
}

#[test]
fn busy_setter_retries_input_without_resending_the_mutation() {
    let device = Transport::new([
        send([3, 15, 132], &[1, 4], 1),
        receive([3, 15, 132], &[1, 4, 0], 1, 2),
        send([3, 15, 4], &[1, 4, 84], 2),
        receive([3, 15, 4], &[], 2, 1),
        receive([3, 15, 4], &[1, 4, 84], 2, 2),
        send([3, 15, 132], &[1, 4], 3),
        receive([3, 15, 132], &[1, 4, 84], 3, 2),
    ]);
    let mut transport = cap(170);
    transport.max_retry_in = 2;
    assert!(
        MouseBrightness::new(&device, &transport)
            .unwrap()
            .apply(change(33, 4), &profiles(&[]), transactions(), || Ok(()))
            .unwrap()
            .verified()
    );
    assert_eq!(device.sent.get(), 3);
    assert_eq!(device.reads.get(), 4);
    device.finish();
}

#[test]
fn wrong_transaction_or_command_retries_the_same_getter_with_source_limits() {
    for mismatch in [
        receive([3, 15, 132], &[1, 4, 84], 9, 2),
        receive([3, 15, 4], &[1, 4, 84], 1, 2),
    ] {
        let device = Transport::new([
            send([3, 15, 132], &[1, 4], 1),
            mismatch,
            send([3, 15, 132], &[1, 4], 1),
            receive([3, 15, 132], &[1, 4, 84], 1, 2),
        ]);
        let mut transport = cap(204);
        transport.max_retry_out = 2;
        assert_eq!(
            MouseBrightness::new(&device, &transport)
                .unwrap()
                .read(1, 4, transactions(), || Ok(()))
                .unwrap()
                .value
                .percent,
            33
        );
        device.finish();
    }
}

#[test]
fn transport_errors_obey_finite_out_retry_limits_and_do_not_start_readback() {
    for send_error in [false, true] {
        let mut script = vec![
            send([3, 15, 132], &[1, 4], 1),
            receive([3, 15, 132], &[1, 4, 0], 1, 2),
        ];
        for _ in 0..2 {
            script.push(Step::Send(packet([3, 15, 4], &[1, 4, 84], 2), !send_error));
            if !send_error {
                script.push(Step::ReadError);
            }
        }
        let device = Transport::new(script);
        let mut transport = cap(45066);
        transport.max_retry_out = 2;
        let failure = MouseBrightness::new(&device, &transport)
            .unwrap()
            .apply(change(33, 4), &profiles(&[]), transactions(), || Ok(()))
            .unwrap_err();
        assert_eq!(failure.phase, BrightnessFailurePhase::Write);
        assert!(failure.device_state_may_have_changed());
        assert_eq!(device.sent.get(), 3);
        assert!(!failure.progress.steps[0].acknowledgment_received);
        device.finish();
    }
}

#[test]
fn unsuccessful_or_unknown_setter_status_never_claims_acknowledgment() {
    for status in [0, 1, 3, 4, 5, 9, 255] {
        let device = Transport::new([
            send([3, 15, 132], &[1, 4], 1),
            receive([3, 15, 132], &[1, 4, 0], 1, 2),
            send([3, 15, 4], &[1, 4, 84], 2),
            receive([3, 15, 4], &[1, 4, 84], 2, status),
        ]);
        let transport = cap(226);
        let failure = MouseBrightness::new(&device, &transport)
            .unwrap()
            .apply(change(33, 4), &profiles(&[]), transactions(), || Ok(()))
            .unwrap_err();
        assert_eq!(failure.phase, BrightnessFailurePhase::Write);
        assert!(!failure.progress.steps[0].acknowledgment_received);
        assert!(failure.progress.steps[0].acknowledged.is_none());
        device.finish();
    }
}

#[test]
fn malformed_hid_lengths_and_oversized_payloads_never_decode_padding() {
    let valid = reply([3, 15, 132], &[1, 4, 84], 1, 2);
    let mut oversized = valid.clone();
    oversized[6] = 81;
    for (response, count) in [(valid[..90].to_vec(), 90), (valid, 92), (oversized, 91)] {
        let device = Transport::new([send([3, 15, 132], &[1, 4], 1), Step::Get(response, count)]);
        let transport = cap(170);
        assert!(
            MouseBrightness::new(&device, &transport)
                .unwrap()
                .read(1, 4, transactions(), || Ok(()))
                .is_err()
        );
        device.finish();
    }
}

#[test]
fn invalid_profiles_transactions_and_packet_capabilities_send_nothing() {
    let device = Transport::new([]);
    let transport = cap(170);
    let mouse = MouseBrightness::new(&device, &transport).unwrap();
    assert!(
        mouse
            .read(
                0,
                4,
                || panic!("Invalid profile minted transaction"),
                || Ok(())
            )
            .is_err()
    );
    for transaction in [31, 32, 128, 255] {
        assert!(mouse.read(1, 4, || Ok(transaction), || Ok(())).is_err());
    }
    for index in 0..4 {
        let mut transport = cap(170);
        match index {
            0 => transport.report_bytes = 90,
            1 => transport.transaction_modulus = 0,
            2 => transport.max_retry_in = 0,
            3 => transport.max_retry_out = 0,
            _ => unreachable!(),
        };
        assert!(
            MouseBrightness::new(&device, &transport)
                .unwrap()
                .read(1, 4, transactions(), || Ok(()))
                .is_err()
        );
    }
    device.finish();
}

#[test]
fn cancelled_reads_before_and_after_receive_do_not_return_observations() {
    let device = Transport::new([]);
    let transport = cap(170);
    assert!(
        MouseBrightness::new(&device, &transport)
            .unwrap()
            .read(
                1,
                4,
                || panic!("Cancelled read minted transaction"),
                || anyhow::bail!("cancelled before read")
            )
            .is_err()
    );
    device.finish();
    let device = Transport::new([
        send([3, 15, 132], &[1, 4], 1),
        receive([3, 15, 132], &[1, 4, 84], 1, 2),
    ]);
    assert!(
        MouseBrightness::new(&device, &transport)
            .unwrap()
            .read(1, 4, transactions(), || {
                ensure!(device.reads.get() == 0, "cancelled after receive");
                Ok(())
            })
            .is_err()
    );
    device.finish();
}

#[test]
fn empty_report_is_never_verified() {
    assert!(!BrightnessWriteReport::default().verified());
}

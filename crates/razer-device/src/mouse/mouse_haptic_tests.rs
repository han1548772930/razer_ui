//! Exact independent protocol fixtures with mandatory synthetic Feature I/O.
use super::*;
use crate::feature_exchange::tests::ExchangeProbe;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

#[derive(Debug)]
struct ReadOwnerRetired;

impl std::fmt::Display for ReadOwnerRetired {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("haptic owner retired after native GET")
    }
}

impl std::error::Error for ReadOwnerRetired {}

enum Step {
    Send(Vec<u8>, bool),
    Get(Vec<u8>, usize),
    UnprovenGet(Vec<u8>, usize),
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
            "Required fixture exchanges not consumed"
        );
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        let Step::Send(expected, success) =
            self.steps.borrow_mut().pop_front().expect("Unexpected OUT")
        else {
            panic!("OUT where IN was expected")
        };
        assert_eq!(
            report, expected,
            "Source command, payload, padding or CRC differs"
        );
        self.sent.set(self.sent.get() + 1);
        ensure!(success, "Synthetic send error");
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        // The prefix is the product's GET selector, checked in device_query_tests.
        assert_eq!(report.len(), 91);
        assert_eq!(&report[1..], [0; 90]);
        self.reads.set(self.reads.get() + 1);
        match self.steps.borrow_mut().pop_front().expect("Unexpected IN") {
            Step::Get(reply, count) => {
                report[..reply.len()].copy_from_slice(&reply);
                Ok(count)
            }
            Step::UnprovenGet(reply, byte_count) => {
                report[..reply.len()].copy_from_slice(&reply);
                Err(crate::backend::native_io_failure(
                    crate::backend::NativeIoOutcome::Returned { byte_count },
                    anyhow::anyhow!("returned count has no retained native input bytes"),
                ))
            }
            Step::ReadError => anyhow::bail!("Synthetic read failure/timeout"),
            Step::Send(..) => panic!("IN where OUT was expected"),
        }
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
fn packet(header: [u8; 3], payload: &[u8], transaction: u8) -> Vec<u8> {
    let mut report = vec![0; 91];
    report[2] = transaction;
    report[6..9].copy_from_slice(&header);
    report[9..9 + payload.len()].copy_from_slice(payload);
    report[89] = report[3..89].iter().fold(0, |a, b| a ^ b);
    report
}
fn reply(header: [u8; 3], payload: &[u8], transaction: u8, status: u8) -> Vec<u8> {
    let mut report = packet(header, payload, transaction);
    report[1] = status;
    report[6] = payload.len() as u8;
    report[89] = report[3..89].iter().fold(0, |a, b| a ^ b);
    report
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
    let mut t = 0;
    move || {
        t = (t + 1) % 31;
        Ok(t)
    }
}
fn profiles() -> HapticProfileObservation {
    HapticProfileObservation {
        first_matching_cached_slot: Some(5),
        selected_first_slot: Some(3),
    }
}
fn triangle() -> HapticCustomStage {
    let direction = HapticCustomDirection {
        scroll_steps: 15,
        scroll_tension: 100,
        coordinates: [
            HapticChartPoint { x: 0.0, y: 330.0 },
            HapticChartPoint { x: 130.0, y: 165.0 },
            HapticChartPoint { x: 260.0, y: 0.0 },
            HapticChartPoint { x: 390.0, y: 165.0 },
            HapticChartPoint { x: 520.0, y: 330.0 },
        ],
    };
    HapticCustomStage {
        linked_curves: true,
        up: direction.clone(),
        down: direction,
    }
}
fn layout() -> HapticStageLayout {
    HapticStageLayout::new(
        HapticPreset::Custom,
        vec![
            HapticPreset::SmoothScroll,
            HapticPreset::Standard,
            HapticPreset::Custom,
        ],
    )
    .unwrap()
}
fn audit() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../testdata/re/mouse-haptic-current-source.json"
    ))
    .unwrap()
}

#[test]
fn dedicated_product_startup_and_command_gates_match_current_source() {
    let evidence = audit();
    assert_eq!(evidence["product_ids"], serde_json::json!([167, 231]));
    let pairs = [
        (
            HapticRead::ActiveStage { profile: 1 },
            "getHapticScrollProfileActiveStage",
            "setHapticScrollProfileActiveStage",
        ),
        (
            HapticRead::StageInformation { profile: 1 },
            "getHapticScrollStageInformation",
            "setHapticScrollStageInformation",
        ),
        (
            HapticRead::GeneratorSetting {
                profile: 1,
                stage: 6,
            },
            "getHapticScrollProfileGeneratorSetting",
            "setHapticScrollProfileGeneratorSetting",
        ),
        (
            HapticRead::GeneratorTable {
                profile: 1,
                stage: 6,
                kind: HapticTableKind::Clockwise,
                offset: 0,
                count: 3,
            },
            "getHapticScrollProfileGeneratorTable",
            "setHapticScrollProfileGeneratorTable",
        ),
        (
            HapticRead::CustomMetadata { profile: 1 },
            "getHapticScrollGeneratorCustomMetaData",
            "setHapticScrollGeneratorCustomMetaData",
        ),
        (
            HapticRead::Acceleration { profile: 1 },
            "getScrollVirtualAccelerationState",
            "setScrollVirtualAccelerationState",
        ),
        (
            HapticRead::BrowserMode,
            "getHapticScrollBrowserModeEnable",
            "setHapticScrollBrowserModeEnable",
        ),
    ];
    for p in evidence["products"].as_array().unwrap() {
        assert_eq!(p["gate"]["defaults"]["activeStage"], "SW_STANDARD");
        assert!(p["gate"]["startup"].as_array().unwrap().iter().any(|s| {
            s["source"]
                .as_str()
                .unwrap()
                .contains("\"rzDeviceType\",\"rzDevice25\",{isDualLinkDevice:!0}")
        }));
        for (query, get, set) in pairs {
            for (name, write) in [(get, false), (set, true)] {
                let m = p["methods"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|m| m["name"] == name)
                    .unwrap();
                assert_eq!(
                    serde_json::to_value(query.command(write).unwrap()).unwrap(),
                    m["command"]
                );
            }
        }
        for (preset, name) in [
            (HapticPreset::Standard, "SW_STANDARD"),
            (HapticPreset::Distinct, "SW_DISTINCT"),
            (HapticPreset::UltraFine, "SW_ULTRA_FINE"),
            (HapticPreset::Adaptive, "SW_ADAPTIVE"),
            (HapticPreset::SmoothScroll, "SW_SMOOTH_SCROLL"),
            (HapticPreset::Custom, "SW_CUSTOM"),
        ] {
            assert_eq!(p["preset_codes"][name], preset.code());
        }
    }
    let p = &evidence["products"][1];
    assert_eq!(p["gate"]["extraOverrides"].as_array().unwrap().len(), 1);
    assert!(
        p["gate"]["extraOverrides"][0]["source"]
            .as_str()
            .unwrap()
            .contains("mappingOverrides")
    );
}

#[test]
fn haptic_adapter_never_expands_to_other_rzdevice25_products() {
    let device = Transport::new([]);
    let mut transport = cap(167);
    for product in [170, 204, 226, 45066, 42] {
        transport.product_id = product;
        assert!(MouseHaptic::new(&device, &transport).is_err());
    }
    transport.product_id = 231;
    transport.source_class = "rzDevice30".into();
    assert!(MouseHaptic::new(&device, &transport).is_err());
    device.finish();
}

#[test]
fn preset_codes_and_active_indexes_stay_distinct_for_reordered_enabled_stages() {
    let layout = layout();
    assert_eq!(layout.active_index(), 3);
    assert_eq!(layout.active_preset().code(), 134);
    assert_eq!(layout.stages()[0].code(), 133);
    assert!(HapticStageLayout::new(HapticPreset::Standard, vec![]).is_err());
    assert!(HapticStageLayout::new(HapticPreset::Custom, vec![HapticPreset::Standard]).is_err());
    assert!(
        HapticStageLayout::new(
            HapticPreset::Standard,
            vec![HapticPreset::Standard, HapticPreset::Standard]
        )
        .is_err()
    );
}

#[test]
fn hand_calculated_linear_curve_produces_three_points_and_exact_scaling() {
    let p = prepare_custom(&triangle()).unwrap();
    assert_eq!(
        p.metadata.up_points,
        [[0, 0], [25, 50], [50, 100], [75, 50], [100, 0]]
    );
    assert_eq!(p.clockwise, [[0.0, 0.0], [12.0, 0.2], [24.0, 0.0]]);
    assert_eq!(p.counterclockwise, p.clockwise);
    assert_eq!(p.generator.index_count_angle_cw, 12.0);
    assert_eq!(p.generator.index_count_angle_ccw, 12.0);
    assert_eq!(p.generator.current_table_size_cw, 3);
    assert_eq!(p.generator.velocity_factor_table_size, 3);
    assert_eq!(p.velocity_factor, [[-720.0, 1.0], [0.0, 1.0], [720.0, 1.0]]);
    assert!(p.metadata.active && p.metadata.linked);
    assert_eq!(p.metadata.order, 6);
}

#[test]
fn independent_directions_keep_their_own_steps_and_tension_even_when_linked() {
    let mut input = triangle();
    input.down.scroll_steps = 30;
    input.down.scroll_tension = 50;
    let p = prepare_custom(&input).unwrap();
    assert_eq!(p.counterclockwise, [[0.0, 0.0], [6.0, 0.1], [12.0, 0.0]]);
    assert_eq!(p.generator.index_count_angle_ccw, 6.0);
    assert_eq!(p.metadata.down_steps, 30);
    assert_eq!(p.metadata.down_tension, 50);
    assert_eq!(p.clockwise, [[0.0, 0.0], [12.0, 0.2], [24.0, 0.0]]);
}

#[test]
fn source_chart_conversion_rounds_before_sampling_and_does_not_wrap_invalid_bytes() {
    let mut custom = triangle();
    custom.up.coordinates[1].x = 132.7;
    assert_eq!(
        prepare_custom(&custom).unwrap().metadata.up_points[1][0],
        26
    );
    for invalid in [f64::NAN, f64::INFINITY, -1.0, 521.0] {
        let mut c = triangle();
        c.up.coordinates[1].x = invalid;
        assert!(prepare_custom(&c).is_err());
    }
    for invalid in [-1.0, 331.0, f64::NAN] {
        let mut c = triangle();
        c.down.coordinates[1].y = invalid;
        assert!(prepare_custom(&c).is_err());
    }
    let mut c = triangle();
    c.up.scroll_steps = 0;
    assert!(prepare_custom(&c).is_err());
    let mut c = triangle();
    c.up.coordinates[0].x = 5.2;
    assert!(prepare_custom(&c).is_err());
    let mut c = triangle();
    c.up.coordinates[1].x = 500.0;
    assert!(prepare_custom(&c).is_err());
}

#[test]
fn invalid_stage_custom_or_observed_profile_never_opens_the_transport() {
    let device = Transport::new([]);
    let transport = cap(167);
    let wheel = MouseHaptic::new(&device, &transport).unwrap();
    for index in [0, 7, 129, 134] {
        let f = wheel
            .apply(
                HapticChange::ActiveStage(index),
                &profiles(),
                || panic!("Invalid stage allocated transaction"),
                || Ok(()),
            )
            .unwrap_err();
        assert_eq!(f.phase, HapticFailurePhase::Prepare);
        assert!(!f.device_state_may_have_changed());
    }
    for id in [0, 6, 255] {
        let p = HapticProfileObservation {
            first_matching_cached_slot: Some(id),
            ..profiles()
        };
        assert!(
            wheel
                .apply(
                    HapticChange::StageInformation(layout()),
                    &p,
                    || panic!("Invalid profile allocated transaction"),
                    || Ok(())
                )
                .is_err()
        );
    }
    let mut c = triangle();
    c.up.scroll_steps = 0;
    assert!(
        wheel
            .apply(
                HapticChange::Custom(c),
                &profiles(),
                || panic!("Invalid curve allocated transaction"),
                || Ok(())
            )
            .is_err()
    );
    device.finish();
}

#[test]
fn stage_information_targets_167_cache_slot_and_only_active_231() {
    for product in [167, 231] {
        let targets: &[u8] = if product == 167 { &[1, 5] } else { &[1] };
        let mut steps = Vec::new();
        let mut t = 1;
        for &p in targets {
            steps.push(send([9, 21, 7], &[p, 3, 3, 133, 129, 134], t));
            steps.push(receive([9, 21, 7], &[], t, 2));
            t += 1;
        }
        for &p in targets {
            steps.push(send([9, 21, 135], &[p], t));
            steps.push(receive([9, 21, 135], &[p, 3, 3, 133, 129, 134], t, 2));
            t += 1;
        }
        let device = Transport::new(steps);
        let transport = cap(product);
        let report = MouseHaptic::new(&device, &transport)
            .unwrap()
            .apply(
                HapticChange::StageInformation(layout()),
                &profiles(),
                transactions(),
                || Ok(()),
            )
            .unwrap();
        assert!(report.verified());
        assert_eq!(report.steps.len(), targets.len());
        assert!(
            report
                .steps
                .iter()
                .all(|s| s.acknowledgment_received && s.acknowledged.is_none())
        );
        device.finish();
    }
}

#[test]
fn active_stage_runs_source_initial_getter_then_writes_and_confirms_each_target() {
    for product in [167, 231] {
        let targets: &[u8] = if product == 167 { &[1, 5] } else { &[1] };
        let mut steps = vec![
            send([2, 21, 128], &[1], 1),
            receive([2, 21, 128], &[1, 1], 1, 2),
        ];
        let mut t = 2;
        for &p in targets {
            steps.push(send([2, 21, 0], &[p, 3], t));
            steps.push(receive([2, 21, 0], &[p, 3], t, 2));
            t += 1;
        }
        for &p in targets {
            steps.push(send([2, 21, 128], &[p], t));
            steps.push(receive([2, 21, 128], &[p, 3], t, 2));
            t += 1;
        }
        let device = Transport::new(steps);
        let transport = cap(product);
        let report = MouseHaptic::new(&device, &transport)
            .unwrap()
            .apply(
                HapticChange::ActiveStage(3),
                &profiles(),
                transactions(),
                || Ok(()),
            )
            .unwrap();
        assert!(report.verified() && !report.skipped_source_write);
        assert_eq!(report.observations[0].value, HapticValue::ActiveStage(1));
        device.finish();
    }
}

#[test]
fn source_active_stage_noop_still_observes_all_targets_without_a_setter() {
    let device = Transport::new([
        send([2, 21, 128], &[1], 1),
        receive([2, 21, 128], &[1, 3], 1, 2),
        send([2, 21, 128], &[5], 2),
        receive([2, 21, 128], &[5, 3], 2, 2),
    ]);
    let transport = cap(167);
    let report = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::ActiveStage(3),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap();
    assert!(report.verified() && report.skipped_source_write);
    assert!(report.steps.is_empty());
    assert_eq!(report.observations.len(), 2);
    device.finish();
}

#[test]
fn divergent_noop_target_is_unconfirmed_and_never_replaced_with_a_write() {
    let device = Transport::new([
        send([2, 21, 128], &[1], 1),
        receive([2, 21, 128], &[1, 3], 1, 2),
        send([2, 21, 128], &[5], 2),
        receive([2, 21, 128], &[5, 2], 2, 2),
    ]);
    let transport = cap(167);
    let f = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::ActiveStage(3),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(f.phase, HapticFailurePhase::Readback);
    assert!(f.progress.skipped_source_write);
    assert!(!f.progress.verified());
    assert!(!f.device_state_may_have_changed());
    assert_eq!(f.progress.observations.len(), 2);
    device.finish();
}

#[test]
fn stage_observation_failure_prevents_all_setters() {
    let device = Transport::new([send([2, 21, 128], &[1], 1), Step::ReadError]);
    let transport = cap(167);
    let f = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::ActiveStage(3),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(f.phase, HapticFailurePhase::Observe);
    assert!(f.progress.steps.is_empty());
    assert!(!f.device_state_may_have_changed());
    device.finish();
}

#[test]
fn acceleration_uses_selected_slot_not_stage_cache_and_browser_is_global() {
    for product in [167, 231] {
        let device = Transport::new([
            send([2, 2, 22], &[1, 1], 1),
            receive([2, 2, 22], &[1, 1], 1, 2),
            send([2, 2, 22], &[3, 1], 2),
            receive([2, 2, 22], &[3, 1], 2, 2),
            send([2, 2, 150], &[1], 3),
            receive([2, 2, 150], &[1, 1], 3, 2),
            send([2, 2, 150], &[3], 4),
            receive([2, 2, 150], &[3, 1], 4, 2),
        ]);
        let transport = cap(product);
        assert!(
            MouseHaptic::new(&device, &transport)
                .unwrap()
                .apply(
                    HapticChange::Acceleration(true),
                    &profiles(),
                    transactions(),
                    || Ok(())
                )
                .unwrap()
                .verified()
        );
        device.finish();
        let device = Transport::new([
            send([1, 21, 8], &[1], 1),
            receive([1, 21, 8], &[1], 1, 2),
            send([1, 21, 136], &[], 2),
            receive([1, 21, 136], &[1], 2, 2),
        ]);
        assert!(
            MouseHaptic::new(&device, &transport)
                .unwrap()
                .apply(
                    HapticChange::BrowserMode(true),
                    &profiles(),
                    transactions(),
                    || Ok(())
                )
                .unwrap()
                .verified()
        );
        device.finish();
    }
}

// These full custom fixtures are hand-calculated from the linear triangle.
// They do not ask the production encoder/curve generator to make expectations.
fn custom_metadata(profile: u8) -> Vec<u8> {
    let mut data = vec![0; 40];
    data[..9].copy_from_slice(&[profile, 40, 6, 1, 1, 15, 100, 15, 100]);
    let points = [0, 0, 25, 50, 50, 100, 75, 50, 100, 0, 255, 255];
    data[9..21].copy_from_slice(&points);
    data[21..33].copy_from_slice(&points);
    data
}
fn custom_generator(profile: u8) -> Vec<u8> {
    let mut data = vec![profile, 6, 1, 1];
    for f in [0.0f32, 12.0, 0.0, 12.0] {
        data.extend_from_slice(&f.to_le_bytes());
    }
    data.extend_from_slice(&[0, 3, 0, 3, 0, 3]);
    data
}
fn table_payload(profile: u8, kind: u8, offset: u16, points: &[[f32; 2]]) -> Vec<u8> {
    let mut data = vec![profile, 6, kind];
    data.extend_from_slice(&offset.to_be_bytes());
    data.extend_from_slice(&(points.len() as u16).to_be_bytes());
    for point in points {
        for f in point {
            data.extend_from_slice(&f.to_le_bytes());
        }
    }
    data
}
fn custom_script(targets: &[u8]) -> Vec<Step> {
    let mut steps = vec![
        send([6, 21, 133], &[], 1),
        receive([6, 21, 133], &[0, 100, 0, 100, 0, 3], 1, 2),
    ];
    let mut writes = Vec::new();
    for &profile in targets {
        writes.push(([80, 21, 9], custom_metadata(profile), vec![profile]));
    }
    for &profile in targets {
        writes.push(([80, 21, 3], custom_generator(profile), vec![profile, 6]));
    }
    for (kind, points) in [
        (1, vec![[0.0, 0.0], [12.0, 0.2], [24.0, 0.0]]),
        (2, vec![[0.0, 0.0], [12.0, 0.2], [24.0, 0.0]]),
        (3, vec![[-720.0, 1.0], [0.0, 1.0], [720.0, 1.0]]),
    ] {
        for &profile in targets {
            writes.push((
                [80, 21, 4],
                table_payload(profile, kind, 0, &points),
                vec![profile, 6, kind, 0, 0, 0, 3],
            ));
        }
    }
    let mut transaction = 2;
    for (command, payload, _) in &writes {
        steps.push(send(*command, payload, transaction));
        steps.push(receive(*command, &[], transaction, 2));
        transaction = (transaction + 1) % 31;
    }
    for (mut command, payload, query) in writes {
        command[2] |= 128;
        steps.push(send(command, &query, transaction));
        steps.push(receive(command, &payload, transaction, 2));
        transaction = (transaction + 1) % 31;
    }
    steps
}

#[test]
fn custom_chain_keeps_metadata_generator_cw_ccw_velocity_order_and_real_getters() {
    for product in [167, 231] {
        let targets: &[u8] = if product == 167 { &[1, 5] } else { &[1] };
        let device = Transport::new(custom_script(targets));
        let transport = cap(product);
        let report = MouseHaptic::new(&device, &transport)
            .unwrap()
            .apply(
                HapticChange::Custom(triangle()),
                &profiles(),
                transactions(),
                || Ok(()),
            )
            .unwrap();
        assert!(report.verified());
        assert_eq!(report.steps.len(), targets.len() * 5);
        assert_eq!(report.observations.len(), 1);
        assert!(matches!(
            report.steps[0].query,
            HapticRead::CustomMetadata { profile: 1 }
        ));
        assert!(report.steps.iter().all(|s| s.acknowledgment_received
            && s.acknowledged.is_none()
            && s.observed.as_ref() == Some(&s.requested)));
        device.finish();
    }
}

#[test]
fn curved_custom_tables_use_eight_point_chunks_exact_offsets_and_a_short_final_chunk() {
    // For each half y = 100 * t^2 and x = 50 * t. At tolerance 0.2,
    // the source forward discretizer keeps four-sample intervals, the peak,
    // and the final endpoint. This is an independent analytic fixture.
    let mut custom = triangle();
    custom.up.coordinates[1].y = 330.0;
    custom.up.coordinates[3].y = 330.0;
    custom.down = custom.up.clone();
    let positions = [
        0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 50, 54, 58, 62, 66, 70, 74, 78, 82, 86,
        90, 94, 98, 100,
    ];
    let points = positions.map(|x| {
        let distance = f64::from(if x <= 50 { x } else { 100 - x });
        [
            (24.0 * f64::from(x) / 100.0) as f32,
            (0.2 * (0.04 * distance * distance / 100.0)) as f32,
        ]
    });
    let generated = prepare_custom(&custom).unwrap();
    assert_eq!(generated.clockwise, points);
    assert_eq!(generated.counterclockwise, points);
    let mut writes = Vec::new();
    for profile in [1, 5] {
        let mut metadata = custom_metadata(profile);
        for index in [12, 16, 24, 28] {
            metadata[index] = 0;
        }
        writes.push(([80, 21, 9], metadata, vec![profile]));
    }
    for profile in [1, 5] {
        let mut generator = custom_generator(profile);
        generator[20..24].copy_from_slice(&[0, 27, 0, 27]);
        writes.push(([80, 21, 3], generator, vec![profile, 6]));
    }
    for kind in [1, 2] {
        for (chunk, points) in points.chunks(8).enumerate() {
            let offset = (chunk * 8) as u16;
            for profile in [1, 5] {
                let payload = table_payload(profile, kind, offset, points);
                let query = payload[..7].to_vec();
                writes.push(([80, 21, 4], payload, query));
            }
        }
    }
    for profile in [1, 5] {
        writes.push((
            [80, 21, 4],
            table_payload(profile, 3, 0, &[[-720.0, 1.0], [0.0, 1.0], [720.0, 1.0]]),
            vec![profile, 6, 3, 0, 0, 0, 3],
        ));
    }
    let mut script = vec![
        send([6, 21, 133], &[], 1),
        receive([6, 21, 133], &[0, 100, 0, 100, 0, 3], 1, 2),
    ];
    let mut transaction = 2;
    for (command, payload, _) in &writes {
        script.push(send(*command, payload, transaction));
        script.push(receive(*command, &[], transaction, 2));
        transaction = (transaction + 1) % 31;
    }
    for (mut command, payload, query) in writes {
        command[2] |= 128;
        script.push(send(command, &query, transaction));
        script.push(receive(command, &payload, transaction, 2));
        transaction = (transaction + 1) % 31;
    }
    let device = Transport::new(script);
    let transport = cap(167);
    let report = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::Custom(custom),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap();
    assert!(report.verified());
    assert_eq!(report.steps.len(), 22);
    assert_eq!(device.sent.get(), 45);
    assert!(report.steps.iter().any(|step| matches!(
        step.query,
        HapticRead::GeneratorTable {
            offset: 24,
            count: 3,
            ..
        }
    )));
    device.finish();
}

#[test]
fn cancellation_after_actual_capacity_observation_prevents_the_first_custom_write() {
    let device = Transport::new([
        send([6, 21, 133], &[], 1),
        receive([6, 21, 133], &[0, 100, 0, 100, 0, 3], 1, 2),
    ]);
    let transport = cap(231);
    let canceled = Cell::new(false);
    let after_capacity = || canceled.set(true);
    let leased = ExchangeProbe::with_on_finish(&device, &after_capacity);
    let failure = MouseHaptic::new(&leased, &transport)
        .unwrap()
        .apply(
            HapticChange::Custom(triangle()),
            &profiles(),
            transactions(),
            || {
                ensure!(!canceled.get(), "Canceled after capacity query");
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(failure.phase, HapticFailurePhase::Observe);
    assert_eq!(failure.failed_step, None);
    assert!(crate::backend::exchange_release_completed(&failure.error));
    assert!(failure.progress.steps.is_empty());
    assert_eq!(failure.progress.observations.len(), 1);
    assert_eq!(
        failure.progress.observations[0].value,
        HapticValue::TableCapacity(HapticTableCapacity {
            clockwise: 100,
            counterclockwise: 100,
            velocity_factor: 3,
        })
    );
    assert!(!failure.progress.verified());
    assert!(!failure.device_state_may_have_changed());
    device.finish();
}

#[test]
fn completed_haptic_reads_use_the_domain_parser_even_when_release_fails() {
    let query = HapticRead::ActiveStage { profile: 1 };
    for (payload, expected) in [
        (vec![1, 3], Some(HapticValue::ActiveStage(3))),
        (vec![5, 3], None),
        (vec![1], None),
        (vec![1, 7], None),
    ] {
        let device = Transport::new([
            send([2, 21, 128], &[1], 1),
            receive([2, 21, 128], &payload, 1, 2),
        ]);
        let transport = cap(231);
        let leased = ExchangeProbe::failing_finish(&device);
        let error = MouseHaptic::new(&leased, &transport)
            .unwrap()
            .read(query, transactions(), || Ok(()))
            .unwrap_err();
        assert!(crate::backend::exchange_finalization_failed(&error));
        assert!(!crate::backend::exchange_release_completed(&error));
        let completed = CompletedHapticRead::from_error(&error);
        assert_eq!(completed.is_some(), expected.is_some());
        if let Some(completed) = completed {
            assert_eq!(completed.observation().query, query);
            assert_eq!(Some(completed.observation().value.clone()), expected);
        }
        device.finish();
    }
}

#[test]
fn canceled_baseline_keeps_actual_stage_and_never_marks_a_skipped_write_verified() {
    let device = Transport::new([
        send([2, 21, 128], &[1], 1),
        receive([2, 21, 128], &[1, 3], 1, 2),
    ]);
    let transport = cap(167);
    let failure = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::ActiveStage(3),
            &profiles(),
            transactions(),
            || {
                if device.reads.get() != 0 {
                    return Err(ReadOwnerRetired.into());
                }
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(failure.phase, HapticFailurePhase::Observe);
    assert_eq!(failure.failed_step, None);
    assert!(failure.error.is::<ReadOwnerRetired>());
    assert_eq!(failure.progress.observations.len(), 1);
    assert_eq!(
        failure.progress.observations[0].query,
        HapticRead::ActiveStage { profile: 1 }
    );
    assert_eq!(
        failure.progress.observations[0].value,
        HapticValue::ActiveStage(3)
    );
    assert!(failure.progress.steps.is_empty());
    assert!(!failure.progress.skipped_source_write);
    assert!(!failure.progress.verified());
    assert!(!failure.device_state_may_have_changed());
    assert_eq!(device.sent.get(), 1);
    assert_eq!(device.reads.get(), 1);
    device.finish();
}

#[test]
fn canceled_unwritten_profile_readback_keeps_both_observations_without_success() {
    let device = Transport::new([
        send([2, 21, 128], &[1], 1),
        receive([2, 21, 128], &[1, 3], 1, 2),
        send([2, 21, 128], &[5], 2),
        receive([2, 21, 128], &[5, 3], 2, 2),
    ]);
    let transport = cap(167);
    let failure = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::ActiveStage(3),
            &profiles(),
            transactions(),
            || {
                if device.reads.get() == 2 {
                    return Err(ReadOwnerRetired.into());
                }
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(failure.phase, HapticFailurePhase::Readback);
    assert_eq!(failure.failed_step, None);
    assert!(failure.error.is::<ReadOwnerRetired>());
    assert_eq!(failure.progress.observations.len(), 2);
    assert_eq!(
        failure.progress.observations[0].query,
        HapticRead::ActiveStage { profile: 1 }
    );
    assert_eq!(
        failure.progress.observations[1].query,
        HapticRead::ActiveStage { profile: 5 }
    );
    assert!(
        failure
            .progress
            .observations
            .iter()
            .all(|observation| observation.value == HapticValue::ActiveStage(3))
    );
    assert!(failure.progress.steps.is_empty());
    assert!(failure.progress.skipped_source_write);
    assert!(!failure.progress.verified());
    assert!(!failure.device_state_may_have_changed());
    assert_eq!(device.sent.get(), 2);
    assert_eq!(device.reads.get(), 2);
    device.finish();
}

#[test]
fn canceled_mutation_readback_keeps_the_actual_value_but_never_verifies() {
    let device = Transport::new([
        send([1, 21, 8], &[1], 1),
        receive([1, 21, 8], &[1], 1, 2),
        send([1, 21, 136], &[], 2),
        receive([1, 21, 136], &[0], 2, 2),
    ]);
    let transport = cap(231);
    let failure = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::BrowserMode(true),
            &profiles(),
            transactions(),
            || {
                if device.reads.get() == 2 {
                    return Err(ReadOwnerRetired.into());
                }
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(failure.phase, HapticFailurePhase::Readback);
    assert_eq!(failure.failed_step, Some(0));
    assert!(failure.error.is::<ReadOwnerRetired>());
    assert!(failure.progress.steps[0].acknowledgment_received);
    assert_eq!(
        failure.progress.steps[0].acknowledged,
        Some(HapticValue::BrowserMode(true))
    );
    assert_eq!(
        failure.progress.steps[0].observed,
        Some(HapticValue::BrowserMode(false))
    );
    assert!(!failure.progress.verified());
    assert_eq!(device.sent.get(), 2);
    assert_eq!(device.reads.get(), 2);
    device.finish();
}

#[test]
fn a_returned_get_count_without_retained_input_bytes_never_becomes_an_observation() {
    let device = Transport::new([
        send([2, 21, 128], &[1], 1),
        Step::UnprovenGet(reply([2, 21, 128], &[1, 3], 1, 2), 91),
    ]);
    let transport = cap(231);
    let failure = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::ActiveStage(3),
            &profiles(),
            transactions(),
            || {
                if device.reads.get() != 0 {
                    return Err(ReadOwnerRetired.into());
                }
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(failure.phase, HapticFailurePhase::Observe);
    assert_eq!(failure.failed_step, None);
    assert_eq!(
        crate::backend::native_io_outcome(&failure.error),
        Some(crate::backend::NativeIoOutcome::Returned { byte_count: 91 })
    );
    assert!(crate::backend::native_read_bytes(&failure.error).is_none());
    assert!(CompletedHapticRead::from_error(&failure.error).is_none());
    assert!(failure.progress.observations.is_empty());
    assert!(failure.progress.steps.is_empty());
    assert!(!failure.progress.verified());
    assert!(!failure.device_state_may_have_changed());
    device.finish();
}

#[test]
fn finish_failure_retains_the_haptic_ack_without_starting_confirmation() {
    let device = Transport::new([send([1, 21, 8], &[1], 1), receive([1, 21, 8], &[], 1, 2)]);
    let transport = cap(231);
    let leased = ExchangeProbe::failing_finish(&device);
    let failure = MouseHaptic::new(&leased, &transport)
        .unwrap()
        .apply(
            HapticChange::BrowserMode(true),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(failure.phase, HapticFailurePhase::Write);
    assert_eq!(failure.failed_step, Some(0));
    assert!(failure.progress.steps[0].acknowledgment_received);
    assert!(failure.progress.steps[0].observed.is_none());
    assert!(!failure.progress.verified());
    device.finish();
}

#[test]
fn cancellation_before_confirmation_keeps_the_completed_mutation_ack() {
    let device = Transport::new([send([1, 21, 8], &[1], 1), receive([1, 21, 8], &[], 1, 2)]);
    let transport = cap(231);
    let canceled = Cell::new(false);
    let after_ack = || canceled.set(true);
    let leased = ExchangeProbe::with_on_finish(&device, &after_ack);
    let failure = MouseHaptic::new(&leased, &transport)
        .unwrap()
        .apply(
            HapticChange::BrowserMode(true),
            &profiles(),
            transactions(),
            || {
                ensure!(!canceled.get(), "Canceled before readback");
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(failure.phase, HapticFailurePhase::Write);
    assert_eq!(failure.failed_step, Some(0));
    assert!(crate::backend::exchange_release_completed(&failure.error));
    assert!(failure.progress.steps[0].acknowledgment_received);
    assert_eq!(failure.progress.steps[0].observed, None);
    assert!(!failure.progress.verified());
    device.finish();
}

#[test]
fn actual_capacity_must_accommodate_every_generated_table_before_first_write() {
    for capacity in [
        vec![0, 2, 0, 3, 0, 3],
        vec![0, 3, 0, 2, 0, 3],
        vec![0, 3, 0, 3, 0, 2],
    ] {
        let device = Transport::new([
            send([6, 21, 133], &[], 1),
            receive([6, 21, 133], &capacity, 1, 2),
        ]);
        let transport = cap(231);
        let f = MouseHaptic::new(&device, &transport)
            .unwrap()
            .apply(
                HapticChange::Custom(triangle()),
                &profiles(),
                transactions(),
                || Ok(()),
            )
            .unwrap_err();
        assert_eq!(f.phase, HapticFailurePhase::Observe);
        assert!(!f.device_state_may_have_changed());
        assert_eq!(f.progress.observations.len(), 1);
        device.finish();
    }
}

#[test]
fn custom_generator_failure_retains_metadata_ack_and_stops_tables() {
    let device = Transport::new([
        send([6, 21, 133], &[], 1),
        receive([6, 21, 133], &[0, 3, 0, 3, 0, 3], 1, 2),
        send([80, 21, 9], &custom_metadata(1), 2),
        receive([80, 21, 9], &[], 2, 2),
        send([80, 21, 3], &custom_generator(1), 3),
        Step::ReadError,
    ]);
    let transport = cap(231);
    let f = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::Custom(triangle()),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(f.phase, HapticFailurePhase::Write);
    assert_eq!(f.failed_step, Some(1));
    assert_eq!(f.progress.steps.len(), 2);
    assert!(f.progress.steps[0].acknowledgment_received);
    assert!(!f.progress.steps[1].acknowledgment_received);
    assert!(f.device_state_may_have_changed());
    device.finish();
}

#[test]
fn partial_profile_failure_has_acks_without_fabricated_readback_or_rollback() {
    let device = Transport::new([
        send([9, 21, 7], &[1, 3, 3, 133, 129, 134], 1),
        receive([9, 21, 7], &[1, 3, 3, 133, 129, 134], 1, 2),
        send([9, 21, 7], &[5, 3, 3, 133, 129, 134], 2),
        Step::ReadError,
    ]);
    let transport = cap(167);
    let f = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::StageInformation(layout()),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(f.phase, HapticFailurePhase::Write);
    assert_eq!(f.failed_step, Some(1));
    assert_eq!(f.progress.steps.len(), 2);
    assert!(f.progress.steps[0].acknowledgment_received);
    assert_eq!(f.progress.steps[1].acknowledged, None);
    assert!(f.progress.steps.iter().all(|s| s.observed.is_none()));
    assert!(!f.progress.verified());
    device.finish();
}

#[test]
fn differing_setter_echo_is_kept_separate_from_matching_getter() {
    let device = Transport::new([
        send([1, 21, 8], &[1], 1),
        receive([1, 21, 8], &[0], 1, 2),
        send([1, 21, 136], &[], 2),
        receive([1, 21, 136], &[1], 2, 2),
    ]);
    let transport = cap(167);
    let report = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::BrowserMode(true),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap();
    assert!(report.verified());
    assert_eq!(
        report.steps[0].acknowledged,
        Some(HapticValue::BrowserMode(false))
    );
    assert_eq!(
        report.steps[0].observed,
        Some(HapticValue::BrowserMode(true))
    );
    device.finish();
}

#[test]
fn readback_difference_is_an_error_with_observations_preserved() {
    let device = Transport::new([
        send([1, 21, 8], &[1], 1),
        receive([1, 21, 8], &[1], 1, 2),
        send([1, 21, 136], &[], 2),
        receive([1, 21, 136], &[0], 2, 2),
    ]);
    let transport = cap(231);
    let f = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::BrowserMode(true),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(f.phase, HapticFailurePhase::Readback);
    assert_eq!(
        f.progress.steps[0].acknowledged,
        Some(HapticValue::BrowserMode(true))
    );
    assert_eq!(
        f.progress.steps[0].observed,
        Some(HapticValue::BrowserMode(false))
    );
    assert!(!f.progress.verified());
    device.finish();
}

#[test]
fn cancellation_before_any_request_has_no_attempted_mutations() {
    let device = Transport::new([]);
    let transport = cap(167);
    let f = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::Custom(triangle()),
            &profiles(),
            || panic!("Canceled task minted transaction"),
            || anyhow::bail!("Canceled before request"),
        )
        .unwrap_err();
    assert_eq!(f.phase, HapticFailurePhase::Prepare);
    assert!(!f.device_state_may_have_changed());
    device.finish();
}

#[test]
fn cancellation_after_completed_ack_keeps_it_and_stops_the_next_profile() {
    let device = Transport::new([
        send([9, 21, 7], &[1, 3, 3, 133, 129, 134], 1),
        receive([9, 21, 7], &[], 1, 2),
    ]);
    let transport = cap(167);
    let canceled = Cell::new(false);
    let after_ack = || canceled.set(true);
    let leased = ExchangeProbe::with_on_finish(&device, &after_ack);
    let f = MouseHaptic::new(&leased, &transport)
        .unwrap()
        .apply(
            HapticChange::StageInformation(layout()),
            &profiles(),
            transactions(),
            || {
                ensure!(!canceled.get(), "Canceled after completed ACK");
                Ok(())
            },
        )
        .unwrap_err();
    assert_eq!(f.phase, HapticFailurePhase::Write);
    assert_eq!(f.failed_step, Some(0));
    assert!(crate::backend::exchange_release_completed(&f.error));
    assert_eq!(f.progress.steps.len(), 1);
    assert!(f.progress.steps[0].acknowledgment_received);
    assert!(f.device_state_may_have_changed());
    device.finish();
}

#[test]
fn invalidated_session_after_io_retains_only_completed_ack_diagnostics() {
    for during_receive in [false, true] {
        let mut steps = vec![send([1, 21, 8], &[1], 1)];
        if during_receive {
            steps.push(receive([1, 21, 8], &[1], 1, 2));
        }
        let device = Transport::new(steps);
        let transport = cap(231);
        let f = MouseHaptic::new(&device, &transport)
            .unwrap()
            .apply(
                HapticChange::BrowserMode(true),
                &profiles(),
                transactions(),
                || {
                    ensure!(
                        if during_receive {
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
        assert!(f.device_state_may_have_changed());
        assert_eq!(f.phase, HapticFailurePhase::Write);
        assert_eq!(f.failed_step, Some(0));
        assert_eq!(f.progress.steps[0].acknowledgment_received, during_receive);
        assert_eq!(
            f.progress.steps[0].acknowledged,
            during_receive.then_some(HapticValue::BrowserMode(true))
        );
        assert_eq!(f.progress.steps[0].observed, None);
        assert!(!f.progress.verified());
        device.finish();
    }
}

#[test]
fn busy_replies_have_a_finite_input_and_output_retry_budget() {
    let device = Transport::new([
        send([1, 21, 8], &[1], 1),
        receive([1, 21, 8], &[], 1, 1),
        receive([1, 21, 8], &[], 1, 1),
        send([1, 21, 8], &[1], 1),
        receive([1, 21, 8], &[], 1, 1),
        receive([1, 21, 8], &[], 1, 1),
    ]);
    let mut transport = cap(167);
    transport.max_retry_in = 2;
    transport.max_retry_out = 2;
    let f = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::BrowserMode(true),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert_eq!(f.phase, HapticFailurePhase::Write);
    assert!(f.to_string().contains("busy"));
    assert_eq!(device.sent.get(), 2);
    assert_eq!(device.reads.get(), 4);
    device.finish();
}

#[test]
fn unsupported_status_or_transport_errors_never_become_an_ack() {
    for status in [0, 3, 4, 5, 255] {
        let device = Transport::new([
            send([1, 21, 8], &[1], 1),
            receive([1, 21, 8], &[1], 1, status),
        ]);
        let transport = cap(167);
        let f = MouseHaptic::new(&device, &transport)
            .unwrap()
            .apply(
                HapticChange::BrowserMode(true),
                &profiles(),
                transactions(),
                || Ok(()),
            )
            .unwrap_err();
        assert!(!f.progress.steps[0].acknowledgment_received);
        assert_eq!(f.phase, HapticFailurePhase::Write);
        device.finish();
    }
    let device = Transport::new([Step::Send(packet([1, 21, 8], &[1], 1), false)]);
    let transport = cap(231);
    let f = MouseHaptic::new(&device, &transport)
        .unwrap()
        .apply(
            HapticChange::BrowserMode(true),
            &profiles(),
            transactions(),
            || Ok(()),
        )
        .unwrap_err();
    assert!(f.to_string().contains("send error"));
    assert!(f.device_state_may_have_changed());
    device.finish();
}

#[test]
fn command_transaction_profile_and_actual_response_lengths_are_checked() {
    for response in [
        reply([2, 21, 128], &[1, 3], 9, 2),
        reply([2, 21, 129], &[1, 3], 1, 2),
        reply([2, 21, 128], &[5, 3], 1, 2),
        reply([2, 21, 128], &[1], 1, 2),
        reply([2, 21, 128], &[1, 7], 1, 2),
    ] {
        let device = Transport::new([send([2, 21, 128], &[1], 1), Step::Get(response, 91)]);
        let transport = cap(167);
        assert!(
            MouseHaptic::new(&device, &transport)
                .unwrap()
                .read(
                    HapticRead::ActiveStage { profile: 1 },
                    transactions(),
                    || Ok(())
                )
                .is_err()
        );
        device.finish();
    }
    let response = reply([2, 21, 128], &[1, 3], 1, 2);
    let device = Transport::new([
        send([2, 21, 128], &[1], 1),
        Step::Get(response[..90].to_vec(), 90),
    ]);
    let transport = cap(231);
    assert!(
        MouseHaptic::new(&device, &transport)
            .unwrap()
            .read(
                HapticRead::ActiveStage { profile: 1 },
                transactions(),
                || Ok(())
            )
            .is_err()
    );
    device.finish();
}

#[test]
fn getter_queries_with_invalid_profiles_stages_counts_or_transactions_never_send() {
    let device = Transport::new([]);
    let transport = cap(167);
    let wheel = MouseHaptic::new(&device, &transport).unwrap();
    for query in [
        HapticRead::ActiveStage { profile: 0 },
        HapticRead::ActiveStage { profile: 6 },
        HapticRead::GeneratorSetting {
            profile: 1,
            stage: 7,
        },
        HapticRead::GeneratorTable {
            profile: 1,
            stage: 6,
            kind: HapticTableKind::Clockwise,
            offset: 0,
            count: 9,
        },
        HapticRead::GeneratorTable {
            profile: 1,
            stage: 6,
            kind: HapticTableKind::Clockwise,
            offset: 65535,
            count: 1,
        },
    ] {
        assert!(
            wheel
                .read(
                    query,
                    || panic!("Invalid query allocated transaction"),
                    || Ok(())
                )
                .is_err()
        );
    }
    for txn in [31, 32, 128, 255] {
        assert!(
            wheel
                .read(HapticRead::BrowserMode, || Ok(txn), || Ok(()))
                .is_err()
        );
    }
    device.finish();
}

#[test]
fn typed_getters_keep_float32_values_and_big_endian_table_lengths() {
    let transport = cap(231);
    let device = Transport::new([
        send([80, 21, 131], &[5, 6], 1),
        receive([80, 21, 131], &custom_generator(5), 1, 2),
    ]);
    let v = MouseHaptic::new(&device, &transport)
        .unwrap()
        .read(
            HapticRead::GeneratorSetting {
                profile: 5,
                stage: 6,
            },
            transactions(),
            || Ok(()),
        )
        .unwrap();
    let HapticValue::GeneratorSetting(g) = v else {
        panic!()
    };
    assert_eq!(g.index_count_angle_cw, 12.0);
    assert_eq!(g.current_table_size_cw, 3);
    device.finish();
    let points = [[0.25, 0.125], [-1.0, 2.0]];
    let payload = table_payload(5, 2, 258, &points);
    let device = Transport::new([
        send([80, 21, 132], &[5, 6, 2, 1, 2, 0, 2], 1),
        receive([80, 21, 132], &payload, 1, 2),
    ]);
    assert_eq!(
        MouseHaptic::new(&device, &transport)
            .unwrap()
            .read(
                HapticRead::GeneratorTable {
                    profile: 5,
                    stage: 6,
                    kind: HapticTableKind::CounterClockwise,
                    offset: 258,
                    count: 2
                },
                transactions(),
                || Ok(())
            )
            .unwrap(),
        HapticValue::GeneratorTable(HapticTableChunk {
            kind: HapticTableKind::CounterClockwise,
            offset: 258,
            points: points.to_vec()
        })
    );
    device.finish();
}

#[test]
fn malformed_tables_generator_metadata_or_stage_information_are_errors() {
    let mut generator = custom_generator(1);
    generator[4..8].copy_from_slice(&f32::NAN.to_le_bytes());
    let table = table_payload(1, 1, 0, &[[0.0, 0.0]]);
    for (query, payload) in [
        (
            HapticRead::GeneratorSetting {
                profile: 1,
                stage: 6,
            },
            generator,
        ),
        (
            HapticRead::GeneratorSetting {
                profile: 1,
                stage: 6,
            },
            vec![1, 6, 1],
        ),
        (
            HapticRead::GeneratorTable {
                profile: 1,
                stage: 6,
                kind: HapticTableKind::Clockwise,
                offset: 0,
                count: 2,
            },
            table,
        ),
        (
            HapticRead::GeneratorTable {
                profile: 1,
                stage: 6,
                kind: HapticTableKind::Clockwise,
                offset: 0,
                count: 1,
            },
            table_payload(1, 2, 0, &[[0.0, 0.0]]),
        ),
        (
            HapticRead::CustomMetadata { profile: 1 },
            vec![1, 39, 6, 1, 1, 15, 100, 15, 100],
        ),
        (
            HapticRead::StageInformation { profile: 1 },
            vec![1, 1, 2, 129, 129],
        ),
        (
            HapticRead::StageInformation { profile: 1 },
            vec![1, 3, 1, 129],
        ),
        (
            HapticRead::StageInformation { profile: 1 },
            vec![1, 1, 1, 135],
        ),
    ] {
        let command = query.command(false).unwrap();
        let device = Transport::new([
            send(command, &query.payload().unwrap(), 1),
            receive(command, &payload, 1, 2),
        ]);
        let transport = cap(167);
        assert!(
            MouseHaptic::new(&device, &transport)
                .unwrap()
                .read(query, transactions(), || Ok(()))
                .is_err()
        );
        device.finish();
    }
}

#[test]
fn remaining_read_queries_use_their_actual_source_payload_and_parse_bytes() {
    let mut tick = vec![1, 2, 1, 3];
    for f in [24.0f32, 0.123456, 0.7, 12.0, 1.5] {
        tick.extend_from_slice(&f.to_le_bytes());
    }
    for (query, header, payload, response, expected) in [
        (
            HapticRead::LibraryVersion,
            [2, 21, 134],
            vec![],
            vec![2, 5],
            HapticValue::LibraryVersion { major: 2, minor: 5 },
        ),
        (
            HapticRead::MaxStage { profile: 5 },
            [2, 21, 129],
            vec![5],
            vec![5, 6],
            HapticValue::MaxStage(6),
        ),
        (
            HapticRead::CustomMetadata { profile: 1 },
            [80, 21, 137],
            vec![1],
            custom_metadata(1),
            HapticValue::CustomMetadata(prepare_custom(&triangle()).unwrap().metadata),
        ),
        (
            HapticRead::TickSetting {
                profile: 1,
                stage: 2,
            },
            [80, 21, 130],
            vec![1, 2],
            tick,
            HapticValue::TickSetting(HapticTickSetting {
                enabled: true,
                mode: 3,
                tick_angle: 24.0,
                tick_current: 0.123456,
                velocity_factor: 0.7,
                index_window_angle: 12.0,
                window_angle: 1.5,
            }),
        ),
    ] {
        let device = Transport::new([send(header, &payload, 1), receive(header, &response, 1, 2)]);
        let transport = cap(231);
        assert_eq!(
            MouseHaptic::new(&device, &transport)
                .unwrap()
                .read(query, transactions(), || Ok(()))
                .unwrap(),
            expected
        );
        device.finish();
    }
}

#[test]
fn max_stage_query_preserves_the_source_raw_byte_without_inventing_a_range() {
    for maximum in [0, 6, 7, 255] {
        let device = Transport::new([
            send([2, 21, 129], &[5], 1),
            receive([2, 21, 129], &[5, maximum], 1, 2),
        ]);
        let transport = cap(231);
        assert_eq!(
            MouseHaptic::new(&device, &transport)
                .unwrap()
                .read(HapticRead::MaxStage { profile: 5 }, transactions(), || Ok(
                    ()
                ))
                .unwrap(),
            HapticValue::MaxStage(maximum)
        );
        device.finish();
    }
}

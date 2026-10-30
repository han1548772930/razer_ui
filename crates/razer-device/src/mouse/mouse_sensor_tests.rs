use super::*;
use crate::feature_exchange::tests::ExchangeProbe;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

enum Io {
    Out(Vec<u8>),
    In(Vec<u8>, usize),
    InError,
    OutError(Vec<u8>),
}

struct Device {
    steps: RefCell<VecDeque<Io>>,
    sent: Cell<usize>,
    received: Cell<usize>,
}

impl Device {
    fn new(steps: impl IntoIterator<Item = Io>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
            sent: Cell::new(0),
            received: Cell::new(0),
        }
    }
    fn done(&self) {
        assert!(
            self.steps.borrow().is_empty(),
            "Required device requests were not made"
        );
    }
}

impl FeatureTransport for Device {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sent.set(self.sent.get() + 1);
        match self.steps.borrow_mut().pop_front() {
            Some(Io::Out(expected)) => {
                assert_eq!(report, expected);
                Ok(())
            }
            Some(Io::OutError(expected)) => {
                assert_eq!(report, expected);
                anyhow::bail!("synthetic OUT error")
            }
            _ => panic!("Unexpected device mutation/query"),
        }
    }
    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        self.received.set(self.received.get() + 1);
        match self.steps.borrow_mut().pop_front() {
            Some(Io::In(reply, count)) => {
                assert_eq!(report.len(), 91);
                report.copy_from_slice(&reply);
                Ok(count)
            }
            Some(Io::InError) => anyhow::bail!("synthetic IN error"),
            _ => panic!("Unexpected device response"),
        }
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

fn packet(header: [u8; 3], payload: &[u8], transaction: u8) -> Vec<u8> {
    let mut packet = vec![0u8; 91];
    packet[2] = transaction;
    packet[6..9].copy_from_slice(&header);
    packet[9..9 + payload.len()].copy_from_slice(payload);
    packet[89] = packet[3..89].iter().fold(0, |sum, byte| sum ^ byte);
    packet
}

fn outgoing(header: [u8; 3], payload: &[u8], transaction: u8) -> Io {
    Io::Out(packet(header, payload, transaction))
}
fn incoming(header: [u8; 3], payload: &[u8], transaction: u8) -> Io {
    let mut bytes = packet(header, payload, transaction);
    bytes[1] = 2;
    bytes[6] = payload.len() as u8;
    Io::In(bytes, 91)
}

fn cap() -> DeviceReadCapability {
    let mut cap = crate::device_reads::capability(226).unwrap().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_out = 1;
    cap.max_retry_in = 1;
    cap
}

fn transactions() -> impl FnMut() -> anyhow::Result<u8> {
    let mut transaction = 0;
    move || {
        transaction = (transaction + 1) % 31;
        Ok(transaction)
    }
}
fn rotation(enabled: bool, angle_degrees: i8) -> SensorChange {
    SensorChange::Rotation {
        enabled,
        angle_degrees,
        route: RotationRoute::DirectFeature,
    }
}
fn dynamic(enabled: bool, mode: AccelerationMode, curve: Vec<f32>) -> SensorChange {
    SensorChange::DynamicSensitivity {
        enabled,
        mode,
        custom_sensor_accelerations: curve,
    }
}

#[test]
fn commands_are_pinned_to_actual_current_sensor_methods() {
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../testdata/re/mouse-sensor-current-source.json"
    ))
    .unwrap();
    assert_eq!(
        source["products"].as_array().unwrap().len(),
        AUDITED_PRODUCTS.len()
    );
    let product = source["products"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["product_id"] == 226)
        .expect("current rotation and dynamic owner");
    let rows = product["queries"].as_array().unwrap();
    for (name, command) in [
        ("getProximitySensorAccelerationState", GET_STATE),
        ("setProximitySensorAccelerationState", SET_STATE),
        ("getProximitySensorAccelerationMode", GET_MODE),
        ("setProximitySensorAccelerationMode", SET_MODE),
        (
            "getProximitySensorAccelerationCurveSetting",
            GET_CURVE_SETTING,
        ),
        ("getProximitySensorAccelerationCurveData", GET_CURVE),
        ("setProximitySensorAccelerationCurveData", SET_CURVE),
        ("getProximitySensorAngleTune", GET_ROTATION),
        ("setProximitySensorAngleTune", SET_ROTATION),
    ] {
        assert_eq!(
            rows.iter().find(|r| r["name"] == name).unwrap()["command"],
            serde_json::json!(command)
        );
    }
    assert_eq!(source["semantics"]["custom_points"], 250);
    assert_eq!(source["semantics"]["chunk_points"], 8);
    assert_eq!(source["semantics"]["full_chunk_delay_ms"], 15);
}

#[test]
fn rotation_preserves_negative_twos_complement_and_disabling_does_not_zero_the_angle() {
    for enabled in [false, true] {
        let payload = [1, u8::from(enabled), 212];
        let device = Device::new([
            outgoing([3, 11, 20], &payload, 1),
            incoming([3, 11, 20], &[], 1),
        ]);
        let report = apply(
            &device,
            &cap(),
            &rotation(enabled, -44),
            transactions(),
            || Ok(()),
            |_| panic!("Rotation should not wait"),
        )
        .unwrap();
        validate_setter_report(&rotation(enabled, -44), &report, false).unwrap();
        assert_eq!(report.acknowledgements[0].data, Vec::<u8>::new());
        device.done();
    }
}

#[test]
fn mouse_rotation_tasks_keep_ack_and_actual_reading_distinct_for_every_current_factory() {
    use crate::mouse_rotation::{self as task, Report, Setting};
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../testdata/re/mouse-sensor-current-source.json"
    ))
    .unwrap();
    for &product in task::PRODUCTS {
        let source = fixture["products"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["product_id"] == product)
            .unwrap();
        for (name, command) in [
            ("setProximitySensorAngleTune", [3, 11, 20]),
            ("getProximitySensorAngleTune", [3, 11, 148]),
        ] {
            assert_eq!(
                source["queries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|row| row["name"] == name)
                    .unwrap()["command"],
                serde_json::json!(command)
            );
        }
        let mut cap = cap();
        cap.product_id = product;
        let setting = Setting {
            enabled: false,
            angle_degrees: -44,
        };
        let device = Device::new([
            outgoing([3, 11, 20], &[1, 0, 212], 1),
            incoming([3, 11, 20], &[], 1),
        ]);
        let report = task::execute(&device, &cap, Some(&setting), &mut transactions(), &|| {
            Ok(())
        })
        .unwrap();
        assert!(report.source_acknowledged());
        assert!(report.validate_reply(None).is_err());
        assert!(
            report
                .validate_reply(Some(&Setting {
                    enabled: true,
                    ..setting
                }))
                .is_err()
        );
        device.done();
        let device = Device::new([
            outgoing([3, 11, 148], &[1], 1),
            incoming([3, 11, 148], &[1, 255, 200], 1),
        ]);
        let reading = task::execute(&device, &cap, None, &mut transactions(), &|| Ok(())).unwrap();
        let Report::Read { reading } = reading else {
            panic!("getter must yield an observation")
        };
        assert_eq!((reading.raw_state, reading.raw_angle), (255, 200));
        assert_eq!(reading.enabled(), None);
        assert_eq!(reading.angle_degrees(), None);
        device.done();
    }
}

#[test]
fn mouse_rotation_task_preserves_failed_delivery_and_native_disable_semantics() {
    use crate::mouse_rotation::{self as task, Report, Setting};
    let setting = Setting {
        enabled: true,
        angle_degrees: -44,
    };
    assert_eq!(
        setting.native_request().unwrap(),
        crate::mapping_native_rotation::RotationRequest::SetAngle(316)
    );
    assert_eq!(
        Setting {
            enabled: false,
            ..setting
        }
        .native_request()
        .unwrap(),
        crate::mapping_native_rotation::RotationRequest::SetAngle(0)
    );
    let device = Device::new([Io::OutError(packet([3, 11, 20], &[1, 1, 212], 1))]);
    let report = task::execute(
        &device,
        &cap(),
        Some(&setting),
        &mut transactions(),
        &|| Ok(()),
    )
    .unwrap();
    assert!(!report.source_acknowledged());
    let Report::Write {
        delivery, failure, ..
    } = report
    else {
        panic!("write failure lost")
    };
    assert_eq!(delivery.attempted_writes, 1);
    assert!(delivery.acknowledgements.is_empty());
    assert!(failure.is_some());
    device.done();
}

#[test]
fn source_ack_only_strategy_never_claims_independent_confirmation() {
    let device = Device::new([
        outgoing([3, 11, 20], &[1, 1, 44], 1),
        incoming([3, 11, 20], &[1, 0, 0], 1),
    ]);
    let report = apply(
        &device,
        &cap(),
        &rotation(true, 44),
        transactions(),
        || Ok(()),
        |_| Ok(()),
    )
    .unwrap();
    validate_setter_report(&rotation(true, 44), &report, false).unwrap();
    assert_eq!(report.acknowledgements[0].data, [1, 0, 0]);
    device.done();
}

#[test]
fn off_stops_after_state_and_does_not_validate_an_unused_custom_curve() {
    let device = Device::new([
        outgoing([2, 11, 16], &[1, 0], 1),
        incoming([2, 11, 16], &[], 1),
    ]);
    let report = apply(
        &device,
        &cap(),
        &dynamic(false, AccelerationMode::Custom, vec![f32::NAN]),
        transactions(),
        || Ok(()),
        |_| panic!("Off should not wait"),
    )
    .unwrap();
    assert_eq!(report.acknowledgements.len(), 1);
    assert_eq!(report.attempted_writes, 1);
    device.done();
}

#[test]
fn each_preset_keeps_state_then_mode_before_any_separate_getter() {
    for mode in [
        AccelerationMode::Classic,
        AccelerationMode::Natural,
        AccelerationMode::Jump,
    ] {
        let raw = mode as u8;
        let device = Device::new([
            outgoing([2, 11, 16], &[1, 1], 1),
            incoming([2, 11, 16], &[], 1),
            outgoing([2, 11, 17], &[1, raw], 2),
            incoming([2, 11, 17], &[], 2),
        ]);
        let report = apply(
            &device,
            &cap(),
            &dynamic(true, mode, vec![]),
            transactions(),
            || Ok(()),
            |_| panic!("Preset should not wait"),
        )
        .unwrap();
        validate_setter_report(&dynamic(true, mode, vec![]), &report, false).unwrap();
        assert_eq!(report.acknowledgements.len(), 2);
        device.done();
    }
}

fn curve_payloads(values: &[f32]) -> Vec<Vec<u8>> {
    // Fixture uses a literal 250/8 source layout, not the production preparer.
    values
        .chunks(8)
        .enumerate()
        .map(|(index, points)| {
            let mut payload = vec![1, 3, 250, (index * 8) as u8, points.len() as u8];
            points
                .iter()
                .for_each(|value| payload.extend(value.to_le_bytes()));
            payload
        })
        .collect()
}

#[test]
fn custom_upload_sends_250_little_endian_floats_with_31_full_chunks_and_short_tail() {
    let values: Vec<f32> = (0..250).map(|x| x as f32 / 4.0).collect();
    let payloads = curve_payloads(&values);
    assert_eq!(&payloads[0][5..9], &[0, 0, 0, 0]);
    assert_eq!(&payloads[0][9..13], &[0, 0, 128, 62]);
    assert_eq!(payloads.len(), 32);
    assert_eq!(payloads[31].len(), 13);
    assert_eq!(&payloads[31][..5], &[1, 3, 250, 248, 2]);
    let mut io = vec![
        outgoing([2, 11, 16], &[1, 1], 1),
        incoming([2, 11, 16], &[], 1),
        outgoing([2, 11, 17], &[1, 3], 2),
        incoming([2, 11, 17], &[], 2),
    ];
    let mut txn = 2;
    for payload in &payloads {
        txn = (txn + 1) % 31;
        io.push(outgoing([37, 11, 19], payload, txn));
        io.push(incoming([37, 11, 19], &[], txn));
    }
    let device = Device::new(io);
    let mut delays = Vec::new();
    let report = apply(
        &device,
        &cap(),
        &dynamic(true, AccelerationMode::Custom, values),
        transactions(),
        || Ok(()),
        |time| {
            delays.push(time);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(report.attempted_writes, 34);
    assert_eq!(report.acknowledgements.len(), 34);
    assert_eq!(delays, vec![Duration::from_millis(15); 31]);
    device.done();
}

#[test]
fn all_read_queries_use_actual_source_profile_and_keep_unknown_raw_values() {
    let cases = [
        (
            SensorQuery::AccelerationState,
            [2, 11, 144],
            vec![1],
            vec![1, 255],
            SensorObservation::AccelerationState { raw_state: 255 },
        ),
        (
            SensorQuery::AccelerationMode,
            [2, 11, 145],
            vec![1],
            vec![1, 255],
            SensorObservation::AccelerationMode { raw_mode: 255 },
        ),
        (
            SensorQuery::CurveSetting {
                mode: AccelerationMode::Natural,
            },
            [3, 11, 146],
            vec![1, 1],
            vec![1, 1, 255],
            SensorObservation::CurveSetting {
                raw_mode: 1,
                input_offset: 255,
            },
        ),
        (
            SensorQuery::Rotation,
            [3, 11, 148],
            vec![1],
            vec![1, 254, 200],
            SensorObservation::Rotation {
                raw_state: 254,
                raw_angle: 200,
            },
        ),
    ];
    for (query, header, payload, response, expected) in cases {
        let device = Device::new([
            outgoing(header, &payload, 1),
            incoming(header, &response, 1),
        ]);
        assert_eq!(
            read(&device, &cap(), query, 1, || Ok(())).unwrap(),
            expected
        );
        device.done();
    }
}

#[test]
fn curve_reads_preserve_received_float_bits_and_returned_padding() {
    let bytes = [0, 0, 128, 191, 0, 0, 128, 62, 0, 0, 0, 0];
    let mut data = vec![1, 3, 250, 12, 2];
    data.extend(bytes);
    let device = Device::new([
        outgoing([37, 11, 147], &[1, 3, 250, 12, 2], 1),
        incoming([37, 11, 147], &data, 1),
    ]);
    assert_eq!(
        read(
            &device,
            &cap(),
            SensorQuery::CurveChunk {
                point_offset: 12,
                point_count: 2
            },
            1,
            || Ok(())
        )
        .unwrap(),
        SensorObservation::CurveChunk {
            point_offset: 12,
            points: vec![-1.0, 0.25],
            raw_point_data: bytes.to_vec()
        }
    );
    device.done();
}

#[test]
fn unsupported_products_routes_ranges_nonfinite_and_bad_curve_sizes_never_mutate() {
    let device = Device::new([]);
    let mut count = 0;
    for change in [
        rotation(true, 45),
        SensorChange::Rotation {
            enabled: true,
            angle_degrees: 0,
            route: RotationRoute::NativeMappingEngine,
        },
        dynamic(true, AccelerationMode::Custom, vec![1.0; 249]),
        dynamic(true, AccelerationMode::Custom, vec![f32::NAN; 250]),
        dynamic(true, AccelerationMode::Custom, vec![f32::INFINITY; 250]),
    ] {
        let failure = apply(
            &device,
            &cap(),
            &change,
            || {
                count += 1;
                Ok(1)
            },
            || Ok(()),
            |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(failure.phase, SensorFailurePhase::Prepare);
        assert!(!failure.device_state_may_have_changed());
    }
    assert_eq!(count, 0);
    for product in [170, 204, 45066, 167, 231, 227] {
        let mut invalid = cap();
        invalid.product_id = product;
        assert!(read(&device, &invalid, SensorQuery::Rotation, 1, || Ok(())).is_err());
        assert!(
            apply(
                &device,
                &invalid,
                &rotation(true, 0),
                transactions(),
                || Ok(()),
                |_| Ok(())
            )
            .is_err()
        );
    }
    device.done();
    assert_eq!(device.sent.get(), 0);
}

#[test]
fn malformed_or_mismatched_read_payloads_are_errors_not_padded_observations() {
    for (query, header, payload, response) in [
        (
            SensorQuery::AccelerationState,
            [2, 11, 144],
            vec![1],
            vec![1],
        ),
        (
            SensorQuery::AccelerationMode,
            [2, 11, 145],
            vec![1],
            vec![2, 0],
        ),
        (SensorQuery::Rotation, [3, 11, 148], vec![1], vec![1, 1]),
        (
            SensorQuery::CurveSetting {
                mode: AccelerationMode::Custom,
            },
            [3, 11, 146],
            vec![1, 3],
            vec![1, 2, 0],
        ),
        (
            SensorQuery::CurveChunk {
                point_offset: 248,
                point_count: 2,
            },
            [37, 11, 147],
            vec![1, 3, 250, 248, 2],
            vec![1, 3, 250, 248, 2, 0, 0, 0, 0],
        ),
        (
            SensorQuery::CurveChunk {
                point_offset: 0,
                point_count: 1,
            },
            [37, 11, 147],
            vec![1, 3, 250, 0, 1],
            vec![1, 3, 249, 0, 1, 0, 0, 0, 0],
        ),
    ] {
        let device = Device::new([
            outgoing(header, &payload, 1),
            incoming(header, &response, 1),
        ]);
        assert!(read(&device, &cap(), query, 1, || Ok(())).is_err());
        device.done();
    }
}

#[test]
fn short_report_wrong_transaction_command_and_unknown_status_cannot_acknowledge() {
    for variant in 0..5 {
        let mut bytes = packet([3, 11, 20], &[1, 1, 0], 1);
        bytes[1] = 2;
        let mut count = 91;
        match variant {
            0 => count = 90,
            1 => count = 92,
            2 => bytes[2] = 2,
            3 => bytes[8] = 21,
            _ => bytes[1] = 5,
        }
        let device = Device::new([outgoing([3, 11, 20], &[1, 1, 0], 1), Io::In(bytes, count)]);
        let failure = apply(
            &device,
            &cap(),
            &rotation(true, 0),
            transactions(),
            || Ok(()),
            |_| Ok(()),
        )
        .unwrap_err();
        assert!(failure.report.acknowledgements.is_empty());
        assert_eq!(failure.report.attempted_writes, 1);
        device.done();
    }
}

#[test]
fn different_readback_is_retained_without_retrying_a_successfully_acknowledged_write() {
    let device = Device::new([
        outgoing([3, 11, 20], &[1, 1, 12], 1),
        incoming([3, 11, 20], &[], 1),
        outgoing([3, 11, 148], &[1], 2),
        incoming([3, 11, 148], &[1, 1, 11], 2),
    ]);
    let report = apply(
        &device,
        &cap(),
        &rotation(true, 12),
        transactions(),
        || Ok(()),
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.acknowledgements.len(), 1);
    let observation = read(&device, &cap(), SensorQuery::Rotation, 2, || Ok(())).unwrap();
    assert_eq!(
        observation,
        SensorObservation::Rotation {
            raw_state: 1,
            raw_angle: 11
        }
    );
    device.done();
}

#[test]
fn failed_mode_write_stops_custom_chunks_and_keeps_completed_state_ack() {
    let device = Device::new([
        outgoing([2, 11, 16], &[1, 1], 1),
        incoming([2, 11, 16], &[], 1),
        Io::OutError(packet([2, 11, 17], &[1, 3], 2)),
    ]);
    let failure = apply(
        &device,
        &cap(),
        &dynamic(true, AccelerationMode::Custom, vec![1.0; 250]),
        transactions(),
        || Ok(()),
        |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(failure.phase, SensorFailurePhase::Write);
    assert_eq!(failure.report.attempted_writes, 2);
    assert_eq!(failure.report.acknowledgements.len(), 1);
    device.done();
}

#[test]
fn finish_failure_keeps_the_current_ack_and_stops_the_remaining_sensor_commands() {
    let device = Device::new([
        outgoing([2, 11, 16], &[1, 1], 1),
        incoming([2, 11, 16], &[7, 8], 1),
    ]);
    let leased = ExchangeProbe::failing_finish(&device);
    let change = dynamic(true, AccelerationMode::Natural, vec![]);
    let failure = apply(
        &leased,
        &cap(),
        &change,
        transactions(),
        || Ok(()),
        |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(failure.phase, SensorFailurePhase::Write);
    assert_eq!(failure.report.attempted_writes, 1);
    assert_eq!(failure.report.acknowledgements.len(), 1);
    assert_eq!(failure.report.acknowledgements[0].data, [7, 8]);
    validate_setter_report(&change, &failure.report, true).unwrap();
    assert!(format!("{:#}", failure.error).contains("release failed"));
    assert!(crate::backend::exchange_finalization_failed(&failure.error));
    assert_eq!((device.sent.get(), device.received.get()), (1, 1));
    device.done();
}

#[test]
fn cancellation_after_reply_or_release_keeps_the_ack_without_starting_mode_or_getter() {
    for cancel_in_reply in [false, true] {
        let device = Device::new([
            outgoing([2, 11, 16], &[1, 1], 1),
            incoming([2, 11, 16], &[], 1),
        ]);
        let canceled = Cell::new(false);
        let after_ack = || canceled.set(true);
        let leased = ExchangeProbe::with_on_finish(&device, &after_ack);
        let change = dynamic(true, AccelerationMode::Natural, vec![]);
        let failure = apply(
            &leased,
            &cap(),
            &change,
            transactions(),
            || {
                ensure!(
                    !canceled.get() && (!cancel_in_reply || device.received.get() == 0),
                    "canceled"
                );
                Ok(())
            },
            |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(failure.phase, SensorFailurePhase::Write);
        assert_eq!(failure.report.attempted_writes, 1);
        assert_eq!(failure.report.acknowledgements.len(), 1);
        assert_eq!(
            failure.report.acknowledgements[0],
            SensorAcknowledgement {
                command: [2, 11, 16],
                payload: vec![1, 1],
                data: vec![],
            }
        );
        validate_setter_report(&change, &failure.report, true).unwrap();
        assert!(validate_setter_report(&change, &failure.report, false).is_err());
        assert!(format!("{:#}", failure.error).contains("canceled"));
        assert!(failure.device_state_may_have_changed());
        assert_eq!((device.sent.get(), device.received.get()), (1, 1));
        device.done();
    }
}

#[test]
fn custom_wait_failure_keeps_first_chunk_ack_and_stops_later_packets() {
    let payload = curve_payloads(&vec![1.0; 250]).remove(0);
    let device = Device::new([
        outgoing([2, 11, 16], &[1, 1], 1),
        incoming([2, 11, 16], &[], 1),
        outgoing([2, 11, 17], &[1, 3], 2),
        incoming([2, 11, 17], &[], 2),
        outgoing([37, 11, 19], &payload, 3),
        incoming([37, 11, 19], &[], 3),
    ]);
    let failure = apply(
        &device,
        &cap(),
        &dynamic(true, AccelerationMode::Custom, vec![1.0; 250]),
        transactions(),
        || Ok(()),
        |time| {
            assert_eq!(time, Duration::from_millis(15));
            anyhow::bail!("canceled wait")
        },
    )
    .unwrap_err();
    assert_eq!(failure.phase, SensorFailurePhase::Wait);
    assert_eq!(failure.report.attempted_writes, 3);
    assert_eq!(failure.report.acknowledgements.len(), 3);
    device.done();
}

#[test]
fn busy_reply_polls_without_repeating_the_mutation_and_exhaustion_is_finite() {
    let mut bytes = packet([3, 11, 20], &[], 1);
    bytes[1] = 1;
    let mut cap = cap();
    cap.max_retry_in = 2;
    let device = Device::new([
        outgoing([3, 11, 20], &[1, 1, 0], 1),
        Io::In(bytes.clone(), 91),
        incoming([3, 11, 20], &[], 1),
    ]);
    let report = apply(
        &device,
        &cap,
        &rotation(true, 0),
        transactions(),
        || Ok(()),
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.acknowledgements.len(), 1);
    device.done();
    let device = Device::new([
        outgoing([3, 11, 20], &[1, 1, 0], 1),
        Io::In(bytes.clone(), 91),
        Io::In(bytes, 91),
    ]);
    let failure = apply(
        &device,
        &cap,
        &rotation(true, 0),
        transactions(),
        || Ok(()),
        |_| Ok(()),
    )
    .unwrap_err();
    assert!(failure.report.acknowledgements.is_empty());
    device.done();
}

#[test]
fn independent_getter_failure_keeps_write_ack_and_no_confirmation() {
    let device = Device::new([
        outgoing([3, 11, 20], &[1, 1, 0], 1),
        incoming([3, 11, 20], &[], 1),
        outgoing([3, 11, 148], &[1], 2),
        Io::InError,
    ]);
    let report = apply(
        &device,
        &cap(),
        &rotation(true, 0),
        transactions(),
        || Ok(()),
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.acknowledgements.len(), 1);
    assert!(read(&device, &cap(), SensorQuery::Rotation, 2, || Ok(())).is_err());
    device.done();
}

#[test]
fn invalid_transactions_and_out_of_range_queries_allocate_no_io() {
    let device = Device::new([]);
    for query in [
        SensorQuery::CurveChunk {
            point_offset: 250,
            point_count: 1,
        },
        SensorQuery::CurveChunk {
            point_offset: 249,
            point_count: 2,
        },
        SensorQuery::CurveChunk {
            point_offset: 0,
            point_count: 0,
        },
        SensorQuery::CurveChunk {
            point_offset: 0,
            point_count: 9,
        },
    ] {
        assert!(read(&device, &cap(), query, 1, || Ok(())).is_err());
    }
    for transaction in [31, 32, 255] {
        let failure = apply(
            &device,
            &cap(),
            &rotation(true, 0),
            || Ok(transaction),
            || Ok(()),
            |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(failure.report.attempted_writes, 0);
    }
    device.done();
    assert_eq!(device.sent.get(), 0);
}

//! Exact synthetic Feature reports exercise production code, never a device.
use super::*;
use crate::feature_exchange::tests::{ExchangeProbe, LeaseEvent};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

enum Io {
    Out(Vec<u8>),
    In(Vec<u8>, usize),
}
struct Transport {
    io: RefCell<VecDeque<Io>>,
    sends: Cell<usize>,
    reads: Cell<usize>,
}
impl Transport {
    fn new(io: impl IntoIterator<Item = Io>) -> Self {
        Self {
            io: RefCell::new(io.into_iter().collect()),
            sends: Cell::new(0),
            reads: Cell::new(0),
        }
    }
    fn finish(&self) {
        assert!(self.io.borrow().is_empty(), "Source exchanges not consumed");
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        let Some(Io::Out(expected)) = self.io.borrow_mut().pop_front() else {
            panic!("Unscripted Feature OUT");
        };
        assert_eq!(report, expected);
        self.sends.set(self.sends.get() + 1);
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        let Some(Io::In(bytes, count)) = self.io.borrow_mut().pop_front() else {
            panic!("Unscripted Feature IN");
        };
        assert_eq!(report.len(), 91);
        // Interface selection is checked across products in device_query_tests.
        assert_eq!(&report[1..], [0; 90]);
        report[..bytes.len()].copy_from_slice(&bytes);
        self.reads.set(self.reads.get() + 1);
        Ok(count)
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
fn request(header: [u8; 3], data: &[u8], txn: u8) -> Vec<u8> {
    let mut packet = vec![0; 91];
    packet[2] = txn;
    packet[6..9].copy_from_slice(&header);
    packet[9..9 + data.len()].copy_from_slice(data);
    packet[89] = packet[3..89].iter().fold(0, |sum, b| sum ^ b);
    packet
}
fn response(header: [u8; 3], data: &[u8], txn: u8, status: u8) -> Vec<u8> {
    let mut packet = request(header, data, txn);
    packet[1] = status;
    packet[6] = data.len() as u8;
    packet
}
fn exchange(header: [u8; 3], data: &[u8], txn: u8, ack: &[u8]) -> [Io; 2] {
    [
        Io::Out(request(header, data, txn)),
        Io::In(response(header, ack, txn, 2), 91),
    ]
}
fn cap(pid: u32) -> DeviceReadCapability {
    let mut cap = crate::mouse_current_dpi::capability(pid)
        .unwrap()
        .transport()
        .for_physical_product(pid)
        .unwrap();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_out = 1;
    cap.max_retry_in = 1;
    cap
}
fn transactions() -> impl FnMut() -> Result<u8> {
    let mut txn = 0;
    move || {
        txn += 1;
        Ok(txn)
    }
}
const SYMMETRIC: TrackingSetting = TrackingSetting::Symmetric {
    tracking_distance: 3,
};
const ASYMMETRIC: TrackingSetting = TrackingSetting::Asymmetric {
    landing_distance: 2,
    lift_off_distance: 26,
};

#[test]
fn finish_failure_keeps_the_current_tracking_response_and_prevents_later_writes() {
    let device = Transport::new(exchange([3, 11, 3], &[0, 4, 1], 1, &[7, 8]));
    let leased = ExchangeProbe::failing_finish(&device);
    let failure = apply(&leased, &cap(170), SYMMETRIC, transactions(), || Ok(())).unwrap_err();
    assert_eq!(failure.phase, TrackingFailurePhase::Command);
    assert_eq!(failure.acknowledgements.len(), 1);
    assert_eq!(failure.acknowledgements[0].payload, [7, 8]);
    assert_eq!((device.sends.get(), device.reads.get()), (1, 1));
    assert!(failure.error.contains("release failed"));
    device.finish();
}

#[test]
fn symmetric_uses_each_actual_products_sensor_state_before_lift_setting() {
    for (pid, enabled) in [(170, 1), (204, 1), (226, 1), (45066, 1)] {
        let io = exchange([3, 11, 3], &[0, 4, enabled], 1, &[])
            .into_iter()
            .chain(exchange([4, 11, 11], &[0, 4, 1, 2], 2, &[]));
        let device = Transport::new(io);
        let result = apply(&device, &cap(pid), SYMMETRIC, transactions(), || Ok(())).unwrap();
        assert!(result.source_chain_acknowledged);
        assert!(!result.verified);
        assert_eq!(result.acknowledgements.len(), 2);
        device.finish();
    }
}

#[test]
fn asymmetric_uses_state_then_hysteresis_then_lift_landing_minus_one() {
    for pid in [170, 204, 226, 45066] {
        let io = exchange([3, 11, 3], &[0, 4, 1], 1, &[])
            .into_iter()
            .chain(exchange([4, 11, 11], &[0, 4, 4, 0], 2, &[]))
            .chain(exchange(
                [10, 11, 5],
                &[0, 4, 25, 1, 0, 0, 0, 0, 0, 0],
                3,
                &[],
            ));
        let device = Transport::new(io);
        let result = apply(&device, &cap(pid), ASYMMETRIC, transactions(), || Ok(())).unwrap();
        assert_eq!(
            result
                .acknowledgements
                .iter()
                .map(|a| a.step)
                .collect::<Vec<_>>(),
            [
                TrackingStep::SensorState,
                TrackingStep::LiftSetting,
                TrackingStep::CutoffConfiguration
            ]
        );
        assert!(!result.verified);
        device.finish();
    }
}

#[test]
fn all_valid_distances_encode_current_source_units_and_bounds() {
    for distance in 1..=3 {
        let plan = prepare(
            capability(204).unwrap(),
            TrackingSetting::Symmetric {
                tracking_distance: distance,
            },
        )
        .unwrap();
        assert_eq!(plan[1].payload, [0, 4, 1, distance - 1]);
        assert_eq!(plan.len(), 2);
    }
    for lift in 2..=26 {
        for landing in 1..lift {
            let plan = prepare(
                capability(226).unwrap(),
                TrackingSetting::Asymmetric {
                    landing_distance: landing,
                    lift_off_distance: lift,
                },
            )
            .unwrap();
            assert_eq!(&plan[2].payload[..4], &[0, 4, lift - 1, landing - 1]);
            assert_eq!(&plan[2].payload[4..], &[0; 6]);
        }
    }
}

#[test]
fn raw_acknowledgements_remain_separate_from_observation() {
    let io = exchange([3, 11, 3], &[0, 4, 1], 1, &[0, 4, 0])
        .into_iter()
        .chain(exchange([4, 11, 11], &[0, 4, 1, 2], 2, &[0, 4, 4, 9]));
    let device = Transport::new(io);
    let result = apply(&device, &cap(204), SYMMETRIC, transactions(), || Ok(())).unwrap();
    assert_eq!(result.acknowledgements[0].payload, [0, 4, 0]);
    assert_eq!(result.acknowledgements[1].payload, [0, 4, 4, 9]);
    assert!(!result.verified);
    device.finish();
}

#[test]
fn second_command_failure_retains_first_ack_and_does_not_send_configuration() {
    let io = exchange([3, 11, 3], &[0, 4, 1], 1, &[]).into_iter().chain([
        Io::Out(request([4, 11, 11], &[0, 4, 4, 0], 2)),
        Io::In(response([4, 11, 11], &[], 2, 5), 91),
    ]);
    let device = Transport::new(io);
    let error = apply(&device, &cap(204), ASYMMETRIC, transactions(), || Ok(())).unwrap_err();
    assert_eq!(error.phase, TrackingFailurePhase::Command);
    assert_eq!(error.step, Some(TrackingStep::LiftSetting));
    assert_eq!(error.acknowledgements.len(), 1);
    assert!(error.command_may_have_reached_device);
    assert_eq!(device.sends.get(), 2);
    device.finish();
}

#[test]
fn cancellation_after_an_ack_keeps_it_and_stops_before_next_command() {
    let device = Transport::new(exchange([3, 11, 3], &[0, 4, 1], 1, &[0, 4, 1]));
    let canceled = Cell::new(false);
    let after_ack = || canceled.set(true);
    let leased = ExchangeProbe::with_on_finish(&device, &after_ack);
    let error = apply(&leased, &cap(204), ASYMMETRIC, transactions(), || {
        ensure!(!canceled.get(), "Synthetic post-ACK cancellation");
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error.phase, TrackingFailurePhase::Command);
    assert_eq!(error.step, Some(TrackingStep::SensorState));
    assert_eq!(error.acknowledgements.len(), 1);
    assert_eq!(error.acknowledgements[0].payload, [0, 4, 1]);
    assert!(error.command_may_have_reached_device);
    assert!(error.error.contains("Synthetic post-ACK cancellation"));
    assert!(matches!(
        leased.events().as_slice(),
        [
            LeaseEvent::Begin(_),
            LeaseEvent::Send,
            LeaseEvent::Get,
            LeaseEvent::Finish
        ]
    ));
    assert_eq!((device.sends.get(), device.reads.get()), (1, 1));
    device.finish();
}

#[test]
fn cancellation_during_reply_retains_only_diagnostic_ack_and_rejects_the_command() {
    let device = Transport::new(exchange([3, 11, 3], &[0, 4, 1], 1, &[0, 4, 1]));
    let error = apply(&device, &cap(204), ASYMMETRIC, transactions(), || {
        ensure!(
            device.reads.get() == 0,
            "Synthetic identity change while reading"
        );
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error.phase, TrackingFailurePhase::Command);
    assert_eq!(error.acknowledgements.len(), 1);
    assert_eq!(error.acknowledgements[0].payload, [0, 4, 1]);
    assert!(error.command_may_have_reached_device);
    device.finish();
}

#[test]
fn unsupported_products_bad_classes_distances_and_transactions_never_send() {
    let device = Transport::new([]);
    assert!(apply(&device, &cap(182), SYMMETRIC, transactions(), || Ok(())).is_err());
    let mut wrong = cap(204);
    wrong.source_class = "rzDevice30".into();
    assert!(apply(&device, &wrong, SYMMETRIC, transactions(), || Ok(())).is_err());
    for distance in [0, 4, 255] {
        assert!(
            apply(
                &device,
                &cap(204),
                TrackingSetting::Symmetric {
                    tracking_distance: distance
                },
                transactions(),
                || Ok(())
            )
            .is_err()
        );
    }
    for (landing, lift) in [(0, 3), (1, 1), (3, 2), (1, 27)] {
        assert!(
            apply(
                &device,
                &cap(204),
                TrackingSetting::Asymmetric {
                    landing_distance: landing,
                    lift_off_distance: lift
                },
                transactions(),
                || Ok(())
            )
            .is_err()
        );
    }
    for txn in [31, 128, 255] {
        assert!(apply(&device, &cap(204), SYMMETRIC, || Ok(txn), || Ok(())).is_err());
    }
    assert!(
        apply(
            &device,
            &cap(204),
            SYMMETRIC,
            transactions(),
            || anyhow::bail!("Canceled before OUT")
        )
        .is_err()
    );
    assert_eq!(device.sends.get(), 0);
    device.finish();
}

#[test]
fn busy_response_only_retries_input_and_not_the_mutation() {
    let mut capability = cap(204);
    capability.max_retry_in = 2;
    let io = [
        Io::Out(request([3, 11, 3], &[0, 4, 1], 1)),
        Io::In(response([3, 11, 3], &[], 1, 1), 91),
        Io::In(response([3, 11, 3], &[], 1, 2), 91),
    ]
    .into_iter()
    .chain(exchange([4, 11, 11], &[0, 4, 1, 2], 2, &[]));
    let device = Transport::new(io);
    assert!(
        apply(&device, &capability, SYMMETRIC, transactions(), || Ok(()))
            .unwrap()
            .source_chain_acknowledged
    );
    assert_eq!(device.sends.get(), 2);
    assert_eq!(device.reads.get(), 3);
    device.finish();
}

#[test]
fn state_reads_match_class_sensor_and_source_boolean() {
    for enabled in [0, 1] {
        let device = Transport::new(exchange([3, 11, 131], &[0, 4], 1, &[0, 4, enabled]));
        assert_eq!(
            read_state(&device, &cap(204), 1, || Ok(()))
                .unwrap()
                .enabled,
            enabled == 1
        );
        device.finish();
    }
    for bytes in [
        vec![],
        vec![0, 4],
        vec![1, 4, 1],
        vec![0, 5, 1],
        vec![0, 4, 2],
    ] {
        let device = Transport::new(exchange([3, 11, 131], &[0, 4], 1, &bytes));
        assert!(read_state(&device, &cap(204), 1, || Ok(())).is_err());
        device.finish();
    }
}

#[test]
fn configuration_reads_preserve_only_actual_bytes_and_do_not_pad_a_ninth_parameter() {
    for payload in [
        vec![0, 4, 25, 1, 2, 3, 4, 5, 6, 7, 8],
        vec![0, 4, 25, 1, 2, 3, 4, 5, 6, 7, 8, 9],
    ] {
        let device = Transport::new(exchange([10, 11, 133], &[0, 4], 1, &payload));
        assert_eq!(
            read_configuration(&device, &cap(226), 1, || Ok(()))
                .unwrap()
                .parameters,
            payload[2..]
        );
        device.finish();
    }
    for bytes in [vec![0, 4, 25, 1], vec![0, 4, 25, 1, 2, 3, 4, 5, 6, 7]] {
        let device = Transport::new(exchange([10, 11, 133], &[0, 4], 1, &bytes));
        assert!(read_configuration(&device, &cap(226), 1, || Ok(())).is_err());
        device.finish();
    }
}

#[test]
fn short_or_oversized_hid_report_stops_the_source_chain() {
    for count in [90, 92] {
        let mut bytes = response([3, 11, 3], &[], 1, 2);
        bytes[0] = 1; // The host discards this prefix; the length is invalid.
        let device = Transport::new([
            Io::Out(request([3, 11, 3], &[0, 4, 1], 1)),
            Io::In(bytes, count),
        ]);
        let error = apply(&device, &cap(204), SYMMETRIC, transactions(), || Ok(())).unwrap_err();
        assert!(error.acknowledgements.is_empty());
        assert!(error.command_may_have_reached_device);
        device.finish();
    }
}

#[test]
fn typed_products_and_flags_match_the_reverified_current_engine_evidence() {
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../testdata/re/mouse-tracking-current-source.json"
    ))
    .unwrap();
    let audited = source["products"].as_array().unwrap();
    assert_eq!(audited.len(), PRODUCTS.len());
    for (product, row) in PRODUCTS.iter().zip(audited) {
        assert_eq!(row["product_id"], product.product_id);
        assert_eq!(
            row["sensor_enabled_by_branch"]["smartTracking"],
            product.symmetric_sensor_enabled
        );
        assert_eq!(row["sensor_enabled_by_branch"]["presetTracking"], true);
        assert_eq!(
            row["commands"]["getProximitySensorState"],
            serde_json::json!(GET_STATE)
        );
        assert_eq!(
            row["commands"]["setProximitySensorState"],
            serde_json::json!(SET_STATE)
        );
        assert_eq!(
            row["commands"]["getProximitySensorConfiguration"],
            serde_json::json!(GET_CONFIG)
        );
        assert_eq!(
            row["commands"]["setProximitySensorConfiguration"],
            serde_json::json!(SET_CONFIG)
        );
        assert_eq!(
            row["commands"]["setProximitySensorLiftSetting"],
            serde_json::json!(SET_LIFT)
        );
        assert_eq!(row["selectors"]["SURFACE_CALIBRATION_CLASS_ID"], CLASS);
        assert_eq!(row["selectors"]["Avago_Proximity_sensor"], SENSOR);
    }
}

#[test]
fn tracking_typed_boundary_rejects_missing_wrong_type_and_out_of_range_values() {
    use serde_json::json;
    let valid = json!({"isAsymmetric":false,"trackingDistance":2,"liftOffDistance":13,"landingDistance":12});
    let value: SmartTracking = serde_json::from_value(valid.clone()).unwrap();
    assert_eq!(
        value.setting(),
        TrackingSetting::Symmetric {
            tracking_distance: 2
        }
    );
    for field in [
        "isAsymmetric",
        "trackingDistance",
        "liftOffDistance",
        "landingDistance",
    ] {
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<SmartTracking>(missing).is_err());
    }
    for (field, bad) in [
        ("isAsymmetric", json!(0)),
        ("trackingDistance", json!(4)),
        ("trackingDistance", json!(1.5)),
        ("landingDistance", json!(13)),
        ("liftOffDistance", json!(27)),
        ("landingDistance", json!(0)),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = bad;
        assert!(serde_json::from_value::<SmartTracking>(invalid).is_err());
    }
}

#[test]
fn tracking_local_storage_rebases_only_accepted_fields_and_preserves_extensions() {
    use serde_json::json;
    let smart: SmartTracking = serde_json::from_value(
        json!({"isAsymmetric":true,"trackingDistance":2,"liftOffDistance":26,"landingDistance":25}),
    )
    .unwrap();
    let mut root = Some(
        json!({"unknown":7,"version":91,"calibration":{"presetTracking":{"surfaceDistance":5},
        "profiles":[{"guid":"saved-profile"}],"smartTracking":{"futureSensorField":"retained"}}}),
    );
    smart.merge_local_settings(&mut root).unwrap();
    let root = root.unwrap();
    assert_eq!(root["version"], 91);
    assert_eq!(root["unknown"], 7);
    assert_eq!(
        root["calibration"]["profiles"],
        json!([{"guid":"saved-profile"}])
    );
    assert_eq!(root["calibration"]["presetTracking"]["surfaceDistance"], 5);
    assert_eq!(
        root["calibration"]["smartTracking"]["futureSensorField"],
        "retained"
    );
    assert_eq!(root["calibration"]["smartTracking"]["landingDistance"], 25);
    for invalid in [
        json!([]),
        json!({"calibration":[]}),
        json!({"calibration":{"smartTracking":null}}),
    ] {
        assert!(smart.merge_local_settings(&mut Some(invalid)).is_err());
    }
    let mut root =
        Some(json!({"calibration":{"futureCalibration":2,"smartTracking":{"deviceExtension":7}}}));
    smart
        .merge_local_settings_from(
            &mut root,
            Some(&json!({"futureCalibration":1,"profileExtension":3,
        "smartTracking":{"legacyExtension":9,"trackingDistance":1}})),
        )
        .unwrap();
    let calibration = &root.as_ref().unwrap()["calibration"];
    assert_eq!(calibration["futureCalibration"], 2);
    assert_eq!(calibration["profileExtension"], 3);
    assert_eq!(calibration["smartTracking"]["deviceExtension"], 7);
    assert_eq!(calibration["smartTracking"]["legacyExtension"], 9);
    assert_eq!(calibration["smartTracking"]["trackingDistance"], 2);
}

fn receiver_exchange(header: [u8; 3], data: &[u8], txn: u8, ack: &[u8], report_id: u8) -> [Io; 2] {
    let [Io::Out(mut out), reply] = exchange(header, data, txn, ack) else {
        unreachable!()
    };
    out[0] = report_id;
    [Io::Out(out), reply]
}

#[test]
fn tracking_receiver_post_write_disconnect_keeps_acknowledgements_and_both_real_observations() {
    let session = crate::mouse_current_dpi::capability(226)
        .unwrap()
        .receiver_session(227, 227)
        .unwrap();
    let cap = session.transport();
    let report_id = cap.report_id;
    let device = Transport::new(
        receiver_exchange([80, 0, 191], &[], 1, &[1, 1, 0, 227], report_id)
            .into_iter()
            .chain(receiver_exchange([3, 11, 3], &[0, 4, 1], 2, &[], report_id))
            .chain(receiver_exchange(
                [4, 11, 11],
                &[0, 4, 1, 2],
                3,
                &[],
                report_id,
            ))
            .chain(receiver_exchange(
                [80, 0, 191],
                &[],
                4,
                &[1, 0, 0, 227],
                report_id,
            )),
    );
    let report = submit(
        &device,
        cap,
        SYMMETRIC,
        Some(&session),
        transactions(),
        || Ok(()),
    );
    assert!(!report.acknowledged());
    assert_eq!(report.receiver_before, Some(vec![(227, 1)]));
    assert_eq!(report.receiver_after, Some(vec![(227, 0)]));
    assert!(report.observation_error.is_some());
    let outcome = report.outcome.unwrap().unwrap();
    assert_eq!(outcome.acknowledgements.len(), 2);
    assert!(outcome.source_chain_acknowledged && !outcome.verified);
    device.finish();
}

#[test]
fn tracking_offline_receiver_does_not_send_a_setter_or_invent_an_attempt() {
    let session = crate::mouse_current_dpi::capability(170)
        .unwrap()
        .receiver_session(171, 171)
        .unwrap();
    let cap = session.transport();
    let device = Transport::new(receiver_exchange(
        [80, 0, 191],
        &[],
        1,
        &[1, 0, 0, 171],
        cap.report_id,
    ));
    let report = submit(
        &device,
        cap,
        SYMMETRIC,
        Some(&session),
        transactions(),
        || Ok(()),
    );
    assert!(!report.acknowledged());
    assert_eq!(report.receiver_before, Some(vec![(171, 0)]));
    assert!(report.receiver_after.is_none() && report.outcome.is_none());
    assert!(report.observation_error.is_some());
    assert_eq!(device.sends.get(), 1);
    device.finish();
}

use super::*;
use crate::feature_exchange::tests::{ExchangeProbe, LeaseEvent};
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};
enum Io {
    Out(Vec<u8>),
    In(Vec<u8>),
}
struct Transport {
    io: RefCell<VecDeque<Io>>,
    sends: Cell<usize>,
}
impl Transport {
    fn new(io: impl IntoIterator<Item = Io>) -> Self {
        Self {
            io: RefCell::new(io.into_iter().collect()),
            sends: Cell::new(0),
        }
    }
    fn finish(&self) {
        assert!(self.io.borrow().is_empty());
    }
}
impl FeatureTransport for Transport {
    fn send_feature(&self, report: &[u8]) -> Result<()> {
        let Some(Io::Out(expected)) = self.io.borrow_mut().pop_front() else {
            panic!("Unscripted OUT");
        };
        assert_eq!(report, expected);
        self.sends.set(self.sends.get() + 1);
        Ok(())
    }
    fn get_feature(&self, report: &mut [u8]) -> Result<usize> {
        let Some(Io::In(bytes)) = self.io.borrow_mut().pop_front() else {
            panic!("Unscripted IN");
        };
        assert_eq!(report.len(), 91);
        report.copy_from_slice(&bytes);
        Ok(91)
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
fn exchange(header: [u8; 3], data: &[u8], txn: u8, status: u8) -> [Io; 2] {
    let out = request(header, data, txn);
    let mut reply = request(header, &[], txn);
    reply[1] = status;
    reply[6] = 0;
    [Io::Out(out), Io::In(reply)]
}
fn cap() -> DeviceReadCapability {
    let mut cap = crate::mouse_legacy_polling::read_capability().clone();
    cap.sleep_between_out_ms = 0;
    cap.sleep_between_out_in_ms = 0;
    cap.sleep_between_in_ms = 0;
    cap.max_retry_out = 1;
    cap.max_retry_in = 1;
    cap
}
fn next() -> impl FnMut() -> Result<u8> {
    let mut txn = 0;
    move || {
        txn += 1;
        Ok(txn)
    }
}
fn raw() -> serde_json::Value {
    json!({"xOffset":16909060,"xSwing":-2.75,"xTIAGain":3,"xReferenceHeightA":12000,
        "xReferenceHeightB":12001,"temperatureVarA":6,"temperatureVarB":7,"yOffset":8,
        "ySwing":9,"yTIAGain":10,"yReferenceHeightA":12002,"yReferenceHeightB":12003,
        "mean":0.75,"standardDeviation":250,"VPower":40000,"vendorExtension":{"keep":true}})
}
fn setting() -> Setting {
    serde_json::from_value::<RawCalibration>(raw())
        .unwrap()
        .preset(Surface::GoliathusSpeed, 3.try_into().unwrap())
        .unwrap()
}
fn configured(surface: &str, lift: u8) -> ConfigChange {
    ConfigChange::parse(json!({"profiles":[{"type":"default","surfaceId":"default"},
        {"type":"preset","guid":"measured-surface","surfaceId":surface,"name":"My surface","extension":4}],
        "selectedProfileGuid":"measured-surface","liftOffRangeValue":lift})).unwrap()
}
#[test]
fn finish_failure_keeps_the_philips_ack_and_stops_the_calibration_chain() {
    let device = Transport::new(exchange([3, 11, 3], &[0, 5, 1], 1, 2));
    let leased = ExchangeProbe::failing_finish(&device);
    let failure = apply(&leased, &cap(), setting(), next(), || Ok(())).unwrap_err();
    assert_eq!(failure.phase, FailurePhase::Command);
    assert_eq!(failure.acknowledgements.len(), 1);
    assert!(failure.acknowledgements[0].payload.is_empty());
    assert_eq!(device.sends.get(), 1);
    assert!(failure.error.contains("release failed"));
    device.finish();
}

#[test]
fn current_source_fixture_covers_all_fixed_surfaces_and_encoders() {
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../testdata/re/mouse-philips-current-source.json"
    ))
    .unwrap();
    assert_eq!(source["product"]["product_id"], 70);
    assert_eq!(
        source["default_identity"],
        json!({"type":"default","guid_is_undefined":true})
    );
    let apply = json!([
        "setProximitySensorState",
        "setProximitySensorCalibrationData",
        "setProximitySensorThreshold"
    ]);
    assert_eq!(source["apply_branches"]["preset"], apply);
    assert_eq!(source["apply_branches"]["manual"], apply);
    assert_eq!(
        source["apply_branches"]["mini"],
        json!([
            "setProximitySensorState",
            "setProximitySensorCalibrationData"
        ])
    );
    assert_eq!(source["calibration_fields"], json!(CALIBRATION_FIELDS));
    assert_eq!(
        source["commands"]["setProximitySensorState"],
        json!(SET_STATE)
    );
    assert_eq!(
        source["commands"]["setProximitySensorCalibrationData"],
        json!(SET_CALIBRATION)
    );
    assert_eq!(
        source["commands"]["setProximitySensorThreshold"],
        json!(SET_THRESHOLD)
    );
    for (helper, shift, mask) in [
        ("TK", 16, 65535),
        ("dV", 0, 65535),
        ("ft", 8, 255),
        ("Zz", 0, 255),
    ] {
        assert_eq!(
            source["numeric_helpers"][helper],
            json!({"shift":shift,"mask":mask})
        );
    }
    let mats = source["mats"].as_array().unwrap();
    assert_eq!(mats.len(), 16);
    for mat in mats {
        let surface: Surface = serde_json::from_value(mat["id"].clone()).unwrap();
        let (distance, thresholds) = surface.parameters();
        assert_eq!(json!(distance), mat["surfaceDistance"]);
        assert_eq!(json!(thresholds), mat["sqList"]);
    }
}
#[test]
fn preset_numeric_truncation_and_big_endian_word_order_match_source() {
    let Setting::Apply {
        calibration,
        lift_off_threshold,
    } = setting()
    else {
        panic!()
    };
    assert_eq!(calibration.x_offset, 0x01020304);
    assert_eq!(calibration.x_swing, 0xfffffffe);
    assert_eq!(calibration.x_reference_height_a, 450);
    assert_eq!(calibration.y_reference_height_b, 453);
    assert_eq!(lift_off_threshold, 357);
    let plan = prepare(setting());
    assert_eq!(
        &plan[1].payload[..10],
        &[0, 5, 1, 2, 3, 4, 255, 255, 255, 254]
    );
    assert_eq!(&plan[1].payload[14..22], &[0, 0, 1, 194, 0, 0, 1, 195]);
    assert_eq!(plan[1].payload.len(), 50);
    assert_eq!(plan[2].payload, [0, 5, 1, 101, 0, 0, 0, 0]);
}
#[test]
fn apply_waits_for_each_ack_and_never_claims_effective_readback() {
    let plan = prepare(setting());
    let io = plan.iter().enumerate().flat_map(|(index, command)| {
        exchange(command.header, &command.payload, index as u8 + 1, 2)
    });
    let device = Transport::new(io);
    let result = apply(&device, &cap(), setting(), next(), || Ok(())).unwrap();
    assert_eq!(
        result
            .acknowledgements
            .iter()
            .map(|ack| ack.step)
            .collect::<Vec<_>>(),
        [Step::SensorState, Step::CalibrationData, Step::Threshold]
    );
    assert!(result.source_chain_acknowledged && !result.verified);
    device.finish();
}
#[test]
fn failed_second_setter_retains_partial_ack_and_skips_threshold() {
    let plan = prepare(setting());
    let device = Transport::new(
        exchange(plan[0].header, &plan[0].payload, 1, 2)
            .into_iter()
            .chain(exchange(plan[1].header, &plan[1].payload, 2, 5)),
    );
    let failure = apply(&device, &cap(), setting(), next(), || Ok(())).unwrap_err();
    assert_eq!(failure.step, Some(Step::CalibrationData));
    assert_eq!(failure.acknowledgements.len(), 1);
    assert!(failure.command_may_have_reached_device);
    device.finish();
}
#[test]
fn cancel_after_ack_preserves_it_without_sending_another_setter() {
    let device = Transport::new(exchange(SET_STATE, &[0, 5, 1], 1, 2));
    let canceled = Cell::new(false);
    let after_ack = || canceled.set(true);
    let leased = ExchangeProbe::with_on_finish(&device, &after_ack);
    let failure = apply(&leased, &cap(), setting(), next(), || {
        ensure!(!canceled.get(), "Canceled after ACK");
        Ok(())
    })
    .unwrap_err();
    assert_eq!(failure.phase, FailurePhase::Command);
    assert_eq!(failure.step, Some(Step::SensorState));
    assert_eq!(failure.acknowledgements.len(), 1);
    assert!(failure.acknowledgements[0].payload.is_empty());
    assert!(failure.command_may_have_reached_device);
    assert!(failure.error.contains("Canceled after ACK"));
    assert!(matches!(
        leased.events().as_slice(),
        [
            LeaseEvent::Begin(_),
            LeaseEvent::Send,
            LeaseEvent::Get,
            LeaseEvent::Finish
        ]
    ));
    assert_eq!(device.sends.get(), 1);
    device.finish();
}
#[test]
fn disable_has_its_own_sensor_and_rejects_an_orca_transport() {
    let device = Transport::new(exchange(SET_STATE, &[0, 5, 0], 1, 2));
    assert_eq!(
        apply(&device, &cap(), Setting::Disabled, next(), || Ok(()))
            .unwrap()
            .acknowledgements
            .len(),
        1
    );
    device.finish();
    let orca = crate::mouse_current_dpi::capability(170)
        .unwrap()
        .transport();
    let device = Transport::new([]);
    assert!(apply(&device, orca, Setting::Disabled, next(), || Ok(())).is_err());
    device.finish();
}
#[test]
fn storage_save_preserves_baseline_extensions_and_wraps_current_source_version() {
    let mut root = Some(
        json!({"calibration":{"presetSensorData":raw(),"extra":{"keep":1}},"version":9007199254740991_u64,"other":true}),
    );
    let change = configured("Goliathus_Speed", 3);
    change.merge_local_settings(&mut root).unwrap();
    let saved = root.as_ref().unwrap();
    assert_eq!(saved["version"], 0);
    assert_eq!(saved["calibration"]["presetSensorData"], raw());
    assert_eq!(saved["calibration"]["extra"], json!({"keep":1}));
    assert_eq!(saved["other"], true);
    assert_eq!(change.setting(root.as_ref()).unwrap(), setting());
    assert!(change.setting(None).is_err());
}
#[test]
fn default_profile_uses_missing_guid_and_invalid_source_inputs_do_not_default() {
    let change = ConfigChange::parse(
        json!({"profiles":[{"type":"default","surfaceId":"default"}],"liftOffRangeValue":3}),
    )
    .unwrap();
    assert_eq!(change.setting(None).unwrap(), Setting::Disabled);
    let mut root = None;
    change.merge_local_settings(&mut root).unwrap();
    assert_eq!(root.unwrap()["calibration"]["selectedProfile"], json!({}));
    for payload in [
        json!({"profiles":[],"liftOffRangeValue":3}),
        json!({"profiles":[{"type":"default"}],"liftOffRangeValue":0}),
        json!({"profiles":[{"type":"preset","surfaceId":"Sphex"}],"liftOffRangeValue":3}),
        json!({"profiles":[{"type":"default"},{"type":"default"}],"liftOffRangeValue":3}),
        json!({"profiles":[{"type":"default"}],"selectedProfileGuid":null,"liftOffRangeValue":3}),
    ] {
        assert!(ConfigChange::parse(payload).is_err());
    }
    let mut malformed = raw();
    malformed.as_object_mut().unwrap().remove("mean");
    assert!(serde_json::from_value::<RawCalibration>(malformed).is_err());
}

#[test]
fn custom_reapply_uses_measured_statistics_and_source_lift_stages() {
    let mut measured = raw();
    measured["VPower"] = json!(0);
    // The source's repeated IEEE-754 subtraction ends at 2.6999999999999997;
    // its upper threshold is 399.9999999999999 before bitwise truncation.
    for (stage, expected) in [150, 175, 200, 228, 257, 285, 314, 342, 371, 399]
        .into_iter()
        .enumerate()
    {
        let change = ConfigChange::parse(
            json!({"profiles":[{"type":"custom","guid":"custom-measured","sensorData":measured}],
            "selectedProfileGuid":"custom-measured","liftOffRangeValue":stage+1}),
        )
        .unwrap();
        let Setting::Apply {
            calibration,
            lift_off_threshold,
        } = change.setting(None).unwrap()
        else {
            panic!()
        };
        assert_eq!(lift_off_threshold, expected);
        assert_eq!(calibration.x_reference_height_a, 12000);
        assert_eq!(calibration.y_reference_height_b, 12003);
    }
}

#[test]
fn projection_migration_keeps_existing_measurements_and_unknown_fields() {
    let mut stored =
        Some(json!({"calibration":{"presetSensorData":raw(),"rootExtension":1},"version":8}));
    let mut projected_raw = raw();
    projected_raw["mean"] = json!(42);
    capture_projection(&mut stored, &json!({"profiles":[{"type":"default","surfaceId":"default"}],
        "selectedProfile":{},"liftOffRangeValue":3,"presetSensorData":projected_raw,"projectedExtension":2})).unwrap();
    let stored = stored.unwrap();
    assert_eq!(stored["version"], 8);
    assert_eq!(stored["calibration"]["presetSensorData"], raw());
    assert_eq!(stored["calibration"]["rootExtension"], 1);
    assert_eq!(stored["calibration"]["projectedExtension"], 2);
}

fn measurement_save(
    destination: MeasurementDestination,
    mode: MeasurementMode,
    raw: serde_json::Value,
) -> MeasurementSave {
    MeasurementSave::new(
        destination,
        mode,
        "new-calibration-guid".to_owned().try_into().unwrap(),
        "18 Oct 2026".into(),
        serde_json::from_value(raw).unwrap(),
    )
    .unwrap()
}

#[test]
fn measurement_save_manual_adds_mean_once_and_keeps_durable_extensions() {
    let mut measured = raw();
    measured["mean"] = json!(64.0);
    let root = json!({"calibration":{"profiles":[{"type":"default","surfaceId":"default"}],"liftOffRangeValue":3,"extension":8},"version":9007199254740991_u64,"other":{"keep":true}});
    let prepared = measurement_save(
        MeasurementDestination::Custom,
        MeasurementMode::Add,
        measured,
    )
    .prepare(Some(&root))
    .unwrap();
    let profile = &prepared.root()["calibration"]["profiles"][1];
    assert_eq!(profile["name"], "MOUSE MAT");
    assert_eq!(profile["calibrationDate"], "18 Oct 2026");
    assert_eq!(profile["sensorData"]["xReferenceHeightA"], 12064.0);
    assert_eq!(profile["sensorData"]["yReferenceHeightB"], 12067.0);
    assert_eq!(
        profile["sensorData"]["vendorExtension"],
        json!({"keep":true})
    );
    let Setting::Apply { calibration, .. } = prepared.setting() else {
        panic!()
    };
    assert_eq!(calibration.x_reference_height_a, 12064);
    assert_eq!(calibration.y_reference_height_b, 12067);
    assert_eq!(prepared.root()["version"], 0);
    assert_eq!(prepared.root()["other"], root["other"]);
    assert_eq!(prepared.root()["calibration"]["extension"], 8);
    assert!(
        prepared.root()["calibration"]
            .get("presetSensorData")
            .is_none()
    );
}

#[test]
fn measurement_save_mini_keeps_raw_baseline_and_omits_threshold_write() {
    let save = measurement_save(
        MeasurementDestination::Preset {
            surface: Surface::GoliathusSpeed,
            name: "Razer Goliathus".into(),
        },
        MeasurementMode::Add,
        raw(),
    );
    let prepared = save.prepare(None).unwrap();
    let expected: RawCalibration = serde_json::from_value(raw()).unwrap();
    assert_eq!(
        serde_json::from_value::<RawCalibration>(
            prepared.root()["calibration"]["presetSensorData"].clone()
        )
        .unwrap(),
        expected
    );
    assert!(
        prepared.root()["calibration"]["profiles"][0]
            .get("sensorData")
            .is_none()
    );
    assert_eq!(
        prepared.root()["calibration"]["profiles"][0]["surfaceId"],
        "Goliathus_Speed"
    );
    assert_eq!(
        prepare(prepared.setting())
            .iter()
            .map(|command| command.header)
            .collect::<Vec<_>>(),
        [SET_STATE, SET_CALIBRATION]
    );
    let mut previous = raw();
    previous["mean"] = json!(999);
    let root = json!({"calibration":{"presetSensorData":previous},"version":4});
    let prepared = save.prepare(Some(&root)).unwrap();
    assert_eq!(
        prepared.root()["calibration"]["presetSensorData"],
        root["calibration"]["presetSensorData"]
    );
    assert_eq!(
        prepared.projection()["presetSensorData"],
        root["calibration"]["presetSensorData"]
    );
    assert_eq!(prepared.root()["version"], 5);
    let prepared = save
        .prepare(Some(&json!({"calibration":{"presetSensorData":null}})))
        .unwrap();
    assert_eq!(
        serde_json::from_value::<RawCalibration>(
            prepared.root()["calibration"]["presetSensorData"].clone()
        )
        .unwrap(),
        expected
    );
}

#[test]
fn measurement_save_recalibration_keeps_source_durable_and_emitted_profile_difference() {
    let root = json!({"calibration":{"profiles":[{"type":"custom","guid":"retained","name":"My mat","surfaceId":"Custom","calibrationDate":"old","sensorData":raw(),"extension":9}],"selectedProfile":{"guid":"retained"}}});
    let mut measured = raw();
    measured["mean"] = json!(128);
    let prepared = measurement_save(
        MeasurementDestination::Custom,
        MeasurementMode::Recalibrate {
            profile: "retained".to_owned().try_into().unwrap(),
        },
        measured,
    )
    .prepare(Some(&root))
    .unwrap();
    let durable = &prepared.root()["calibration"]["profiles"][0];
    assert_eq!(durable["sensorData"], raw());
    assert_eq!(durable["extension"], 9);
    assert_eq!(durable["calibrationDate"], "18 Oct 2026");
    let emitted = &prepared.projection()["profiles"][0];
    assert_eq!(emitted["guid"], "retained");
    assert_eq!(emitted["name"], "My mat");
    assert_eq!(emitted["sensorData"]["xReferenceHeightA"], 12128.0);
    assert_eq!(
        prepared.root()["calibration"]["selectedProfile"]["guid"],
        "retained"
    );
    let absent = measurement_save(
        MeasurementDestination::Custom,
        MeasurementMode::Recalibrate {
            profile: "missing".to_owned().try_into().unwrap(),
        },
        raw(),
    );
    assert!(absent.prepare(Some(&root)).is_err());
}

#[test]
fn measurement_save_names_follow_the_source_bare_then_numeric_collision_loop() {
    let save = measurement_save(MeasurementDestination::Custom, MeasurementMode::Add, raw());
    let mut profiles: Vec<_> = (0..=99).map(|index| json!({"type":"custom","guid":format!("old-{index}"),"name":if index==0 {"MOUSE MAT".into()} else {format!("MOUSE MAT {index}")},"sensorData":raw()})).collect();
    let prepared = save
        .prepare(Some(&json!({"calibration":{"profiles":profiles}})))
        .unwrap();
    assert_eq!(
        prepared.root()["calibration"]["profiles"][100]["name"],
        "MOUSE MAT 100"
    );
    profiles.remove(2);
    let prepared = save
        .prepare(Some(&json!({"calibration":{"profiles":profiles}})))
        .unwrap();
    assert_eq!(
        prepared.root()["calibration"]["profiles"][99]["name"],
        "MOUSE MAT 2"
    );
}

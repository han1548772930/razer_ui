use super::*;
use serde_json::{Value, json};

fn callback() -> Value {
    json!({
        "event":"device-container", "dllName":"RzSystemCommon",
        "dllFunc":"HardwareMonitorCallback", "dataType":0, "status":"SUCCESS",
        "CPUTemperature":37.25, "CPUClock":4100.5,
        "GPUTemperature":null, "GPUClock":null,
        "CPUTemperatureMin":30.0, "CPUTemperatureMax":50.0
    })
}

#[test]
fn system_monitor_callback_keeps_null_distinct_from_measured_zero() {
    let mut data = callback();
    data["CPUClock"] = json!(0);
    let sample = parse_callback(&serde_json::to_vec(&data).unwrap(), "device-container").unwrap();
    assert_eq!(sample.cpu_temperature, Some(37.25));
    assert_eq!(sample.cpu_clock_mhz, Some(0.0));
    assert_eq!(sample.gpu_temperature, None);
    assert_eq!(sample.gpu_clock_mhz, None);
}

#[test]
fn system_monitor_callback_requires_fields_and_routing_identity() {
    for field in [
        "event",
        "dllName",
        "dllFunc",
        "dataType",
        "status",
        "CPUTemperature",
        "CPUClock",
        "GPUTemperature",
        "GPUClock",
    ] {
        let mut data = callback();
        data.as_object_mut().unwrap().remove(field);
        assert!(
            parse_callback(&serde_json::to_vec(&data).unwrap(), "device-container").is_err(),
            "{field}"
        );
    }
    for (field, value) in [
        ("event", json!("previous-container")),
        ("dllName", json!("different-dll")),
        ("dllFunc", json!("different-function")),
        ("dataType", json!(1)),
        ("status", json!("FAILURE")),
        ("CPUClock", json!("4100")),
        ("GPUTemperature", json!(false)),
    ] {
        let mut data = callback();
        data[field] = value;
        assert!(
            parse_callback(&serde_json::to_vec(&data).unwrap(), "device-container").is_err(),
            "{field}"
        );
    }
}

#[test]
fn system_monitor_observation_rejects_old_owner_missing_values_and_non_finite_numbers() {
    let sample = parse_callback(
        &serde_json::to_vec(&callback()).unwrap(),
        "device-container",
    )
    .unwrap();
    let mut observation = MonitorObservation {
        owner_guid: "current-owner".into(),
        samples: vec![sample],
        diagnostics: vec![],
    };
    observation.validate_for("current-owner").unwrap();
    assert!(observation.validate_for("previous-owner").is_err());
    observation.samples[0].cpu_temperature = Some(f64::NAN);
    assert!(observation.validate_for("current-owner").is_err());
    assert!(
        serde_json::from_value::<SystemSample>(
            json!({"cpu_clock_mhz":null,"gpu_temperature":null,"gpu_clock_mhz":null})
        )
        .is_err()
    );
    assert!(parse_callback(br#"{"CPUTemperature":1e400}"#, "device-container").is_err());
    for identity in ["", " ", "a\0b"] {
        assert!(validate_identity(identity).is_err());
    }
}

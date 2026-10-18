//! Fixed wire fixtures; no child process, device, DLL or vendor service runs.
use super::*;
use serde_json::json;

fn observation() -> Value {
    json!({
        "node": {
            "path": [104, 97, 110, 98, 111],
            "vendor_id": 5426,
            "product_id": 3893,
            "usage_page": 65280,
            "usage": 1,
            "interface_number": 0,
            "release_number": 256,
            "serial_number": "sample",
            "manufacturer": "Razer",
            "product": "Razer Hanbo"
        },
        "product_id": 3893,
        "telemetry": {
            "fan": {
                "channel_number": 3,
                "presence": 1,
                "set_mode": 2,
                "actual_mode": 3,
                "rpm": 4660,
                "set_duty_cycle": 99,
                "actual_duty_cycle": 48
            },
            "pump": {
                "presence": 0,
                "set_mode": 2,
                "actual_mode": 1,
                "temperature_tenths_celsius": 367,
                "rpm": 22136,
                "set_duty_cycle": 80,
                "actual_duty_cycle": 79
            }
        }
    })
}

fn startup_observation() -> HanboStartupObservation {
    let mut packet = [0; 64];
    packet[..2].copy_from_slice(&[2, 2]);
    HanboStartupObservation {
        node: serde_json::from_value(observation()["node"].clone()).unwrap(),
        product_id: 3893,
        reading: razer_device::hanbo_cooler::StartupReading::DeviceInformation(
            razer_device::hanbo_cooler::decode_device_information(&packet).unwrap(),
        ),
    }
}

#[test]
fn hanbo_startup_request_keeps_independent_getter_identity() {
    let node = startup_observation().node;
    for (query, wire_query) in [
        (
            razer_device::hanbo_cooler::StartupQuery::DeviceInformation,
            "device_information",
        ),
        (
            razer_device::hanbo_cooler::StartupQuery::ErrorcodeInfo,
            "errorcode_info",
        ),
    ] {
        let request = ServiceRequest::HidNodeHanboStartupRead {
            node: node.clone(),
            product_id: 3893,
            query,
        };
        let frame = json!({"operation":"hid_node_hanbo_startup_read","node":node,"product_id":3893,"query":wire_query});
        assert_eq!(serde_json::to_value(request).unwrap(), frame);
        assert!(serde_json::from_value::<ServiceRequest>(frame).is_ok());
    }
    assert!(
        serde_json::from_value::<ServiceRequest>(
            json!({"operation":"hid_node_hanbo_startup_read","node":node,"product_id":3893})
        )
        .is_err()
    );
}

#[test]
fn hanbo_startup_observation_rejects_an_old_collection_or_different_getter() {
    use razer_device::hanbo_cooler::StartupQuery;
    let mut receipt = startup_observation();
    receipt
        .validate_for(&receipt.node, StartupQuery::DeviceInformation)
        .unwrap();
    assert!(
        receipt
            .validate_for(&receipt.node, StartupQuery::ErrorcodeInfo)
            .is_err()
    );
    let original = receipt.node.clone();
    receipt.node.serial_number = Some("replacement-device".into());
    assert!(
        receipt
            .validate_for(&original, StartupQuery::DeviceInformation)
            .is_err()
    );
    receipt = startup_observation();
    receipt.product_id = 123;
    assert!(
        receipt
            .validate_for(&receipt.node, StartupQuery::DeviceInformation)
            .is_err()
    );
}

#[test]
fn hanbo_startup_wire_requires_device_information_fields_and_all_identity_bytes() {
    let data = serde_json::to_value(startup_observation()).unwrap();
    for field in [
        "device_id",
        "edition",
        "mode",
        "firmware",
        "firmware_debug_flag",
        "hardware",
    ] {
        let mut frame = data.clone();
        frame["reading"]["data"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            serde_json::from_value::<HanboStartupObservation>(frame).is_err(),
            "{field}"
        );
    }
    for bytes in [json!([1, 2]), json!(vec![0; 16])] {
        let mut frame = data.clone();
        frame["reading"]["data"]["device_id"] = bytes;
        assert!(serde_json::from_value::<HanboStartupObservation>(frame).is_err());
    }
}

#[test]
fn hanbo_read_request_uses_the_fixed_operation_tag_and_full_collection() {
    let frame = observation();
    let node = serde_json::from_value(frame["node"].clone()).unwrap();
    let request = ServiceRequest::HidNodeHanboCoolerRead {
        node,
        product_id: 3893,
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({
            "operation": "hid_node_hanbo_cooler_read",
            "node": frame["node"],
            "product_id": 3893
        })
    );
}

#[test]
fn hanbo_observation_requires_both_getters_and_retains_disconnected_state() {
    let receipt: HanboCoolerObservation = serde_json::from_value(observation()).unwrap();
    receipt.validate_for(&receipt.node).unwrap();
    assert_eq!(receipt.telemetry.fan.rpm, 4660);
    assert_eq!(receipt.telemetry.pump.rpm, 22136);
    assert_eq!(receipt.telemetry.pump.temperature_celsius(), 36.7);
    assert_eq!(
        receipt.telemetry.pump.presence,
        razer_device::hanbo_cooler::ControllerPresence::Absent
    );
    for field in ["fan", "pump"] {
        let mut frame = observation();
        frame["telemetry"].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<HanboCoolerObservation>(frame).is_err());
    }
}

#[test]
fn hanbo_observation_rejects_stale_collection_and_wrong_product_identity() {
    let receipt: HanboCoolerObservation = serde_json::from_value(observation()).unwrap();
    for field in [
        "path",
        "vendor_id",
        "product_id",
        "usage_page",
        "usage",
        "interface_number",
        "release_number",
        "serial_number",
    ] {
        let mut frame = observation();
        frame["node"][field] = match field {
            "path" => json!([110, 101, 119]),
            "serial_number" => json!("replacement"),
            _ => json!(0),
        };
        if field == "interface_number" {
            frame["node"][field] = json!(1);
        }
        let stale: HanboCoolerObservation = serde_json::from_value(frame).unwrap();
        assert!(stale.validate_for(&receipt.node).is_err(), "{field}");
    }
    let mut wrong_product = receipt.clone();
    wrong_product.product_id = 182;
    assert!(wrong_product.validate_for(&receipt.node).is_err());
}

#[test]
fn hanbo_observation_rejects_missing_fields_bad_types_and_impossible_temperature() {
    for field in [
        "channel_number",
        "presence",
        "set_mode",
        "actual_mode",
        "rpm",
        "set_duty_cycle",
        "actual_duty_cycle",
    ] {
        let mut frame = observation();
        frame["telemetry"]["fan"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            serde_json::from_value::<HanboCoolerObservation>(frame).is_err(),
            "{field}"
        );
    }
    for value in [json!(null), json!("4660"), json!(-1), json!(65536)] {
        let mut frame = observation();
        frame["telemetry"]["fan"]["rpm"] = value;
        assert!(serde_json::from_value::<HanboCoolerObservation>(frame).is_err());
    }
    let mut frame = observation();
    frame["telemetry"]["pump"]["temperature_tenths_celsius"] = json!(2806);
    let receipt: HanboCoolerObservation = serde_json::from_value(frame).unwrap();
    assert!(receipt.validate_for(&receipt.node).is_err());
}

#[test]
fn hanbo_ipc_preserves_unknown_protocol_values_without_inventing_defaults() {
    let mut frame = observation();
    frame["telemetry"]["fan"]["presence"] = json!(129);
    frame["telemetry"]["fan"]["set_mode"] = json!(254);
    let receipt: HanboCoolerObservation = serde_json::from_value(frame.clone()).unwrap();
    receipt.validate_for(&receipt.node).unwrap();
    assert_eq!(
        receipt.telemetry.fan.presence,
        razer_device::hanbo_cooler::ControllerPresence::Unknown(129)
    );
    assert_eq!(
        receipt.telemetry.fan.set_mode,
        razer_device::hanbo_cooler::ControllerMode::Unknown(254)
    );
    assert_eq!(serde_json::to_value(receipt).unwrap(), frame);
}

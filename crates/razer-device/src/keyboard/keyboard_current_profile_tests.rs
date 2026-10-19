use super::*;
use serde_json::json;

#[test]
fn keyboard_current_profile_initialization_keeps_global_and_per_key_defaults_separate() {
    let profile = initial_profile();
    profile.validate().unwrap();
    assert_eq!(profile.guid, "af06d371-861f-4b98-8d78-bfaef5cfdebf");
    assert_eq!(profile.name, "Factory Default");
    assert_eq!(profile.settings.actuation_point.values(), [1.2, 1.2]);
    assert!(profile.settings.mappings.is_empty());
    let device = initial_device_settings();
    device.validate().unwrap();
    assert_eq!(device.analog_data.default_make_actuation, 19656);
    assert_eq!(device.analog_data.default_break_actuation, 18018);
    assert_eq!(device.analog_data.key_id_to_key_input[&59].key, 1);
    assert_eq!(
        device.report_ids.analog_keyboard_v2,
        ReportKind::AnalogKeyboardV2
    );
    assert_eq!(SCHEMA_VERSION, 13);
}

#[test]
fn keyboard_current_profile_decode_rejects_missing_and_malformed_settings() {
    let profile = serde_json::to_value(initial_profile()).unwrap();
    for field in [
        "guid",
        "name",
        "actuationPoint",
        "switchOffLighting",
        "brightness",
        "quickEffects",
        "gamingMode",
        "snapTap",
        "continuousRapidTrigger",
        "macro",
        "mappings",
    ] {
        let mut missing = profile.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(decode_profile(&missing).is_err(), "{field}");
    }
    for (field, value) in [
        ("brightness", json!({"isEnabled":true,"value":101})),
        ("gamingMode", json!({"state":0})),
        ("snapTap", json!({"isEnabled":false})),
        ("actuationPoint", json!([0.0, 1.2])),
        ("mappings", json!(null)),
    ] {
        let mut invalid = profile.clone();
        invalid[field] = value;
        assert!(decode_profile(&invalid).is_err(), "{field}");
    }
}

#[test]
fn keyboard_current_profile_preserves_extensions_and_analog_stage_identity() {
    let mut value = serde_json::to_value(initial_profile()).unwrap();
    value["actuationPoint"] = json!([2.0, 1.9]);
    value["unknownProfileExtension"] = json!({"value":77});
    value["brightness"]["unknownBrightnessExtension"] = json!("preserved");
    value["mappings"] = json!([{"inputID":"KEY_A","inputType":"AnalogInput","mapping":[
        {"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},"actuationPoint":[19656,256]},
        {"outputType":"defaultGroup"}]}]);
    let decoded = decode_profile(&value).unwrap();
    assert_eq!(decoded.settings.actuation_point.values(), [2.0, 1.9]);
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    value["mappings"][0]["mapping"][0]
        .as_object_mut()
        .unwrap()
        .remove("outputType");
    assert!(decode_profile(&value).is_err());
}

#[test]
fn keyboard_current_profile_device_settings_never_create_absent_analog_values() {
    let settings = serde_json::to_value(initial_device_settings()).unwrap();
    for field in ["analogData", "reportIDs", "selectedProfile"] {
        let mut missing = settings.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(decode_device_settings(&missing).is_err(), "{field}");
    }
    let mut changed = settings;
    changed["analogData"]["defaultMakeActuation"] = json!(29484);
    changed["analogData"]["defaultBreakActuation"] = json!(256);
    let decoded = decode_device_settings(&changed).unwrap();
    assert_eq!(decoded.analog_data.default_make_actuation, 29484);
    assert_eq!(decoded.analog_data.default_break_actuation, 256);
}

#[test]
fn keyboard_current_profile_accepts_source_analog_outer_record_without_output_type() {
    let mut profile = serde_json::to_value(initial_profile()).unwrap();
    profile["mappings"] = json!([{
        "inputID":"KEY_LEFT_CTRL","inputType":"AnalogInput","isHyperShift":false,
        "mapping":[
            {"outputType":"keyboardGroup","keyboardGroup":{},"actuationPoint":{"0":16380,"1":14742}},
            {"actuationPoint":{"0":19656,"1":18018},"isDefault":false,
             "outputType":"defaultGroup","isHyperShift":false}
        ]
    }]);
    let decoded = decode_profile(&profile).unwrap();
    assert!(decoded.settings.mappings[0].output_type.is_none());
    assert_eq!(serde_json::to_value(decoded).unwrap(), profile);
    profile["mappings"][0]["mapping"][0]
        .as_object_mut()
        .unwrap()
        .remove("outputType");
    assert!(decode_profile(&profile).is_err());
}

#[test]
fn keyboard_current_profile_high_speed_products_require_their_source_extensions() {
    for product in [740, 746, 756] {
        let mut profile = if product == 756 {
            crate::keyboard_latest_mapping::default_profile()
        } else {
            crate::keyboard_magnetic_mapping::default_profile(product).unwrap()
        };
        profile["guid"] = json!("typed-local-profile");
        high_speed::decode_profile_for_product(product, &profile).unwrap();
        for field in [
            "pollingRate",
            "bottomDeadzone",
            "calibratedKeys",
            "isEnableKeyboardKeys",
            "isGamepadOverride",
            "singleKeySnapTap",
            "interruptKey",
            "dualKeypressPriority",
        ] {
            let mut incomplete = profile.clone();
            incomplete.as_object_mut().unwrap().remove(field);
            assert!(
                high_speed::decode_profile_for_product(product, &incomplete).is_err(),
                "PID{product} {field}"
            );
        }
        profile["snapTap"]["keyList"][0]
            .as_object_mut()
            .unwrap()
            .remove("mode");
        assert!(high_speed::decode_profile_for_product(product, &profile).is_err());
    }
}

#[test]
fn keyboard_current_profile_latest_device_power_fields_match_source_numeric_enum() {
    let settings = serde_json::to_value(high_speed::initial_latest_device_settings()).unwrap();
    assert_eq!(settings["lowPowerMode"], 0);
    assert_eq!(
        settings["dimLighting"],
        json!({"isEnabled":true,"value":5.0})
    );
    assert_eq!(
        settings["powerSaving"],
        json!({"isEnabled":true,"value":15.0})
    );
    let decoded = high_speed::decode_latest_device_settings(&settings).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), settings);
    for field in high_speed::LatestDeviceSettings::FIELDS {
        let mut incomplete = settings.clone();
        incomplete.as_object_mut().unwrap().remove(*field);
        assert!(
            high_speed::decode_latest_device_settings(&incomplete).is_err(),
            "{field}"
        );
    }
    let mut invalid = settings;
    invalid["lowPowerMode"] = json!("Disable");
    assert!(high_speed::decode_latest_device_settings(&invalid).is_err());
    invalid["lowPowerMode"] = json!(2);
    assert!(high_speed::decode_latest_device_settings(&invalid).is_err());
}

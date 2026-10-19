use super::*;
use serde_json::{Value, json};

#[test]
#[ignore = "explicit maintenance-only read of the installed host's exact keyboard key"]
fn installed_keyboard679_profile_readonly() {
    let root = std::path::PathBuf::from(
        std::env::var_os("RAZER_PROFILE_AUDIT_DIRECTORY")
            .expect("maintenance supplies the platform-owned localStorage directory"),
    );
    let text = crate::host_local_profile::read_local_storage(&root, "synapse_679")
        .expect("strict coherent installed snapshot")
        .expect("maintenance target key must be present");
    let document: Document = serde_json::from_str(&text).expect("typed current keyboard catalog");
    let metadata = document
        .device_metadatas
        .as_ref()
        .expect("actual stored serial metadata");
    assert!(
        !metadata.is_empty(),
        "maintenance target needs an actual stored serial"
    );
    for (index, serial) in metadata.keys().enumerate() {
        let source =
            decode(679, serial, &text).expect("complete current catalog and exact serial selector");
        println!(
            "target=synapse_679 record={index} profiles={} selected={}",
            source.profiles().len(),
            source.active_guid().is_some()
        );
    }
}

fn profile(guid: &str, name: &str) -> Value {
    json!({
        "guid":guid,"name":name,"actuationPoint":{"0":1.2,"1":1.2},
        "switchOffLighting":{"isDisplayOn":false,"isIdleEnabled":false,"idleMinutes":1},
        "brightness":{"isEnabled":false,"value":42},"quickEffects":{"selectedEffectId":3},
        "gamingMode":{"state":0,"isWindowsKeyDisabled":false,"isAltF4Disabled":false,"isAltTabDisabled":false},
        "snapTap":{"isEnabled":false,"keyList":[{"key1":"KEY_A","key2":"KEY_D","id":1}]},
        "continuousRapidTrigger":false,"macro":{},"mappings":[]
    })
}

fn document() -> Value {
    json!({
        "version":17,"vendorId":5426,"productId":679,"category":"KEYBOARD","schemaVersion":13,
        "activeProfile":"first",
        "deviceMetadatas":{
            "real":{"activeProfileGuid":"selected","name":"User keyboard","futureSerial":{"keep":true}},
            "other":{"activeProfileGuid":"first","name":"Another keyboard"}
        },
        "profiles":[profile("first","First profile"),profile("selected","实际配置")],
        "analogData":{"defaultMakeActuation":19656,"defaultBreakActuation":18018},
        "reportIDs":{"11":"analogKeyboardV2ReportID","4":"razerKeyReportID"},
        "defaultMappings":[],"selectedProfile":{},"futureRoot":{"kept":7}
    })
}

fn decode_value(serial: &str, value: &Value) -> Result<LocalProfiles> {
    decode(679, serial, &value.to_string())
}

#[test]
fn serial_selection_overrides_the_other_serials_root_active_profile() {
    let loaded = decode_value("real", &document()).unwrap();
    assert_eq!(loaded.product_id(), 679);
    assert_eq!(loaded.serial(), "real");
    assert_eq!(loaded.profiles().len(), 2);
    assert_eq!(loaded.active_guid(), Some("selected"));
    assert_eq!(loaded.active_profile().unwrap().name(), "实际配置");
    assert_eq!(loaded.selection_origin(), SelectionOrigin::SerialMetadata);
    let source = loaded.source_document().unwrap();
    assert_eq!(source["activeProfile"], "selected");
    assert_eq!(source["profiles"][1]["brightness"]["value"], 42);
    assert_eq!(source["profiles"][1]["brightness"]["isEnabled"], false);
}

#[test]
fn existing_source_document_without_serial_metadata_uses_its_first_profile() {
    let source = document();
    let loaded = decode_value("new-real-serial", &source).unwrap();
    assert_eq!(loaded.active_guid(), Some("first"));
    assert_eq!(
        loaded.selection_origin(),
        SelectionOrigin::FirstProfileWithoutSerialMetadata
    );
    let mut without_metadata = source.clone();
    without_metadata
        .as_object_mut()
        .unwrap()
        .remove("deviceMetadatas");
    let loaded = decode_value("real", &without_metadata).unwrap();
    assert_eq!(loaded.active_guid(), Some("first"));
    assert!(
        loaded
            .source_document()
            .unwrap()
            .get("deviceMetadatas")
            .is_none()
    );
}

#[test]
fn explicit_empty_or_dangling_serial_selection_does_not_choose_another_profile() {
    for guid in ["", "absent"] {
        let mut source = document();
        source["deviceMetadatas"]["real"]["activeProfileGuid"] = guid.into();
        assert!(decode_value("real", &source).is_err());
    }
}

#[test]
fn valid_empty_source_collection_stays_empty_without_initialization() {
    let mut source = document();
    source["profiles"] = json!([]);
    source.as_object_mut().unwrap().remove("deviceMetadatas");
    let loaded = decode_value("real", &source).unwrap();
    assert!(loaded.profiles().is_empty());
    assert!(loaded.active_profile().is_none());
    assert_eq!(loaded.selection_origin(), SelectionOrigin::EmptyCollection);
    let saved = loaded.source_document().unwrap();
    assert_eq!(saved["profiles"], json!([]));
    assert!(saved.get("activeProfile").is_none());
}

#[test]
fn external_settings_and_other_serial_metadata_are_retained() {
    let mut source = document();
    source["profiles"][1]["futureProfile"] = json!({"unknownMode":23});
    source["profiles"][1]["brightness"]["futureBrightness"] = json!({"retained":false});
    source["profiles"][1]["mappings"] = json!([{
        "inputID":"KEY_A","inputType":"AnalogInput","outputType":"keyboardGroup","isHyperShift":false,
        "mapping":[{"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},"actuationPoint":{"0":19656,"1":18018}}],
        "futureMapping":{"preserved":[1,2]}
    }]);
    let loaded = decode_value("real", &source).unwrap();
    let mut expected = source.clone();
    expected["activeProfile"] = "selected".into();
    assert_eq!(loaded.source_document().unwrap(), expected);
    assert_eq!(
        loaded.active_profile().unwrap().source_settings().unwrap(),
        expected["profiles"][1]
    );
    let settings = loaded.device_settings().unwrap();
    expected.as_object_mut().unwrap().remove("profiles");
    assert_eq!(settings, expected);
}

#[test]
fn model_projection_keeps_source_guids_and_never_synthesizes_observed_settings() {
    let loaded = decode_value("real", &document()).unwrap();
    let projection = loaded.project().unwrap();
    assert_eq!(projection.active_profile.as_deref(), Some("selected"));
    assert_eq!(projection.profiles.len(), 2);
    for (source, projected) in loaded.profiles().iter().zip(&projection.profiles) {
        assert_eq!(projected.id, source.guid());
        assert_eq!(projected.guid, source.guid());
        assert_eq!(projected.name, source.name());
        assert_eq!(
            projected.source_settings.as_ref(),
            Some(&source.source_settings().unwrap())
        );
        assert!(projected.settings.is_none());
        assert!(projected.dpi_stages.is_none());
    }
    assert_eq!(
        projection.source_device_settings,
        loaded.device_settings().unwrap()
    );
    assert!(projection.source_device_settings.get("profiles").is_none());

    let mut empty = document();
    empty["profiles"] = json!([]);
    empty.as_object_mut().unwrap().remove("deviceMetadatas");
    let projection = decode_value("real", &empty).unwrap().project().unwrap();
    assert!(projection.profiles.is_empty());
    assert!(projection.active_profile.is_none());
    assert!(
        projection
            .source_device_settings
            .get("activeProfile")
            .is_none()
    );
}

fn keyboard() -> razer_model::model::Device {
    let mut device = razer_model::product_preview::registered_preview(679).unwrap();
    device.serial_number = "real".into();
    device.device_container_id = "physical-container".into();
    device
}

#[test]
fn local_import_only_changes_local_fields_and_retains_new_observations() {
    let source = decode_value("real", &document()).unwrap();
    let mut device = keyboard();
    let baseline = device.clone();
    device.observe_connection(Some(
        razer_model::model::DeviceConnectionObservation::HidPresent,
    ));
    device.mount_local_runtime();
    device.dashboard.device_state = Some(json!({"observed":"device-state"}));
    device.dashboard.readonly_pairing_is_dongle = Some(false);
    let mut before = serde_json::to_value(&device).unwrap();
    assert!(import(&mut device, &baseline, &source).unwrap());
    assert_eq!(device.active_profile, "selected");
    assert_eq!(device.profiles[1].name, "实际配置");
    assert_eq!(
        device.dashboard.connection_observation,
        Some(razer_model::model::DeviceConnectionObservation::HidPresent)
    );
    assert!(device.dashboard.local_runtime_mounted);
    assert_eq!(device.dashboard.readonly_pairing_is_dongle, Some(false));
    assert_eq!(
        device.dashboard.device_state,
        Some(json!({"observed":"device-state"}))
    );
    let mut after = serde_json::to_value(&device).unwrap();
    for value in [&mut before, &mut after] {
        let object = value.as_object_mut().unwrap();
        object.remove("profiles");
        object.remove("active_profile");
        object.remove("source_device_settings");
    }
    assert_eq!(before, after);
    let baseline = device.clone();
    assert!(!import(&mut device, &baseline, &source).unwrap());
}

#[test]
fn local_import_rejects_draft_changes_and_wrong_identity_before_mutating() {
    let source = decode_value("real", &document()).unwrap();
    let mut device = keyboard();
    let empty = device.clone();
    import(&mut device, &empty, &source).unwrap();
    let baseline = device.clone();
    for field in [
        "name",
        "active",
        "settings",
        "serial",
        "container",
        "product",
    ] {
        let mut edited = device.clone();
        match field {
            "name" => edited.profiles[0].name = "new draft name".into(),
            "active" => edited.active_profile = "first".into(),
            "settings" => {
                edited.source_device_settings.as_mut().unwrap()["futureRoot"] =
                    json!({"userEdit":true})
            }
            "serial" => edited.serial_number = "another-real-device".into(),
            "container" => edited.device_container_id = "another-container".into(),
            "product" => edited.product_id = 688,
            _ => unreachable!(),
        }
        let before = serde_json::to_value(&edited).unwrap();
        assert!(
            import(&mut edited, &baseline, &source).is_err(),
            "changed {field}"
        );
        assert_eq!(serde_json::to_value(&edited).unwrap(), before);
    }
}

#[test]
fn empty_import_keeps_selection_absent_and_a_dangling_selection_never_imports() {
    let mut value = document();
    value["profiles"] = json!([]);
    value.as_object_mut().unwrap().remove("deviceMetadatas");
    let source = decode_value("real", &value).unwrap();
    let mut device = keyboard();
    let baseline = device.clone();
    import(&mut device, &baseline, &source).unwrap();
    assert!(device.profiles.is_empty());
    assert!(device.active_profile.is_empty());
    let mut invalid = document();
    invalid["deviceMetadatas"]["real"]["activeProfileGuid"] = "absent".into();
    assert!(decode_value("real", &invalid).is_err());
}

#[test]
fn duplicate_guids_and_invalid_unselected_settings_are_document_errors() {
    let mut source = document();
    source["profiles"][0]["guid"] = "selected".into();
    assert!(decode_value("real", &source).is_err());
    let mut source = document();
    source["profiles"][0]["brightness"]["value"] = 101.into();
    assert!(decode_value("real", &source).is_err());
    let mut source = document();
    source["profiles"][0]["actuationPoint"]["0"] = 5.into();
    assert!(decode_value("real", &source).is_err());
    let mut source = document();
    source["profiles"][0]["gamingMode"]["state"] = 3.into();
    assert!(decode_value("real", &source).is_err());
}

#[test]
fn missing_required_fields_and_wrong_types_are_not_synthetic_defaults() {
    for key in [
        "guid",
        "name",
        "brightness",
        "actuationPoint",
        "switchOffLighting",
        "quickEffects",
        "gamingMode",
        "snapTap",
        "continuousRapidTrigger",
        "macro",
        "mappings",
    ] {
        let mut source = document();
        source["profiles"][1].as_object_mut().unwrap().remove(key);
        assert!(decode_value("real", &source).is_err(), "missing {key}");
    }
    let mut source = document();
    source["profiles"][1]["brightness"]["isEnabled"] = "false".into();
    assert!(decode_value("real", &source).is_err());
    let mut source = document();
    source["profiles"][1]["mappings"] = json!([{"inputID":"KEY_A","inputType":"AnalogInput"}]);
    assert!(decode_value("real", &source).is_err());
}

#[test]
fn explicit_null_is_distinct_from_an_absent_optional_source_field() {
    for key in ["deviceMetadatas", "activeProfile", "isDefault"] {
        let mut source = document();
        source[key] = Value::Null;
        assert!(decode_value("real", &source).is_err(), "null {key}");
    }
    let mut source = document();
    source["deviceMetadatas"]["real"]["activeProfileGuid"] = Value::Null;
    assert!(decode_value("real", &source).is_err());
}

#[test]
fn incompatible_source_identity_and_version_are_errors_before_import() {
    for (key, value) in [
        ("productId", json!(756)),
        ("vendorId", json!(1)),
        ("category", json!("MOUSE")),
        ("schemaVersion", json!(12)),
        ("schemaVersion", json!(14)),
        ("version", json!(1u64 << 53)),
    ] {
        let mut source = document();
        source[key] = value;
        assert!(decode_value("real", &source).is_err(), "invalid {key}");
    }
    assert!(decode(756, "real", &document().to_string()).is_err());
    assert!(decode_value("", &document()).is_err());
    assert!(decode_value("PREVIEW-679", &document()).is_err());
    assert!(decode(679, "real", "{").is_err());
    assert!(decode(679, "real", "null").is_err());
}

#[test]
fn full_size_and_mini_documents_keep_their_product_specific_source_settings() {
    for product_id in [678, 688] {
        let mut source = document();
        source["productId"] = product_id.into();
        source["analogData"]["defaultBreakActuation"] = 1638.into();
        if product_id == 688 {
            for profile in source["profiles"].as_array_mut().unwrap() {
                profile["actuationPoint"] = json!({"0":1.5,"1":1.5});
                profile["modTap"] = true.into();
            }
        }
        let loaded = decode(product_id, "real", &source.to_string()).unwrap();
        assert_eq!(loaded.product_id(), product_id);
        assert_eq!(loaded.active_guid(), Some("selected"));
        let mut expected = source.clone();
        expected["activeProfile"] = "selected".into();
        assert_eq!(loaded.source_document().unwrap(), expected);
        let projection = loaded.project().unwrap();
        assert_eq!(
            projection.profiles()[1].source_settings,
            Some(expected["profiles"][1].clone())
        );
        assert!(projection.profiles()[1].settings.is_none());
    }
}

#[test]
fn mini_mod_tap_is_required_and_is_not_inferred_from_other_settings() {
    let mut source = document();
    source["productId"] = 688.into();
    for value in [None, Some(Value::Null), Some(json!(1)), Some(json!("true"))] {
        let mut candidate = source.clone();
        for profile in candidate["profiles"].as_array_mut().unwrap() {
            if let Some(value) = &value {
                profile["modTap"] = value.clone();
            }
        }
        assert!(decode(688, "real", &candidate.to_string()).is_err());
    }
    for profile in source["profiles"].as_array_mut().unwrap() {
        profile["modTap"] = false.into();
    }
    let loaded = decode(688, "real", &source.to_string()).unwrap();
    assert_eq!(
        loaded.source_document().unwrap()["profiles"][1]["modTap"],
        false
    );
}

fn high_speed_document() -> Value {
    let mut source = document();
    source["productId"] = 719.into();
    source["analogData"]["defaultBreakActuation"] = 1638.into();
    source["turnOffAnalogToKey"] = true.into();
    for profile in source["profiles"].as_array_mut().unwrap() {
        profile["pollingRate"] = 8000.into();
        profile["bottomDeadzone"] = json!({"value":"medium","futureDeadzone":17});
        profile["calibratedKeys"] =
            json!([{"inputID":"KEY_C","inputType":"KeyInput","futureCalibration":false}]);
        profile["isEnableKeyboardKeys"] = false.into();
        profile["isGamepadOverride"] = true.into();
        profile["singleKeySnapTap"] = json!({"isEnabled":false,"keyList":[{"keyMapMake":"KEY_Q","keyMapBreak":"KEY_E","id":1,"futurePair":true}]});
        profile["interruptKey"] =
            json!({"isEnabled":false,"interrupts":["KEY_ANY"],"futureInterrupt":42});
        profile["dualKeypressPriority"] =
            json!({"isEnabled":false,"keys":["KEY_A","KEY_D"],"futurePriority":"kept"});
        profile["snapTap"]["keyList"][0]["mode"] = "LAST_INPUT".into();
    }
    source
}

#[test]
fn high_speed_profile_settings_and_extensions_survive_projection() {
    let source = high_speed_document();
    let loaded = decode(719, "real", &source.to_string()).unwrap();
    assert_eq!(loaded.product_id(), 719);
    let mut expected = source;
    expected["activeProfile"] = "selected".into();
    assert_eq!(loaded.source_document().unwrap(), expected);
    let projection = loaded.project().unwrap();
    assert_eq!(
        projection.profiles()[1].source_settings,
        Some(expected["profiles"][1].clone())
    );
    assert!(projection.profiles()[1].settings.is_none());
    let interrupt = &projection.profiles()[1].source_settings.as_ref().unwrap()["interruptKey"];
    assert!(interrupt.get("interruptionKey").is_none());
}

#[test]
fn high_speed_required_settings_are_not_inferred_or_replaced() {
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
        for replacement in [None, Some(Value::Null), Some(json!("invalid"))] {
            let mut source = high_speed_document();
            let profile = source["profiles"][0].as_object_mut().unwrap();
            if let Some(value) = replacement {
                profile.insert(field.into(), value);
            } else {
                profile.remove(field);
            }
            assert!(
                decode(719, "real", &source.to_string()).is_err(),
                "invalid {field}"
            );
        }
    }
    for (field, value) in [
        ("pollingRate", json!(9000)),
        ("bottomDeadzone", json!({"value":"unknown"})),
        ("calibratedKeys", json!([{"inputID":"KEY_Q"}])),
        (
            "singleKeySnapTap",
            json!({"isEnabled":true,"keyList":[{"id":1,"keyMapMake":"KEY_Q"}]}),
        ),
        (
            "interruptKey",
            json!({"isEnabled":true,"interruptionKey":"unknown","interrupts":[]}),
        ),
        ("dualKeypressPriority", json!({"isEnabled":true,"keys":[4]})),
    ] {
        let mut source = high_speed_document();
        source["profiles"][0][field] = value;
        assert!(
            decode(719, "real", &source.to_string()).is_err(),
            "invalid {field}"
        );
    }
    let mut source = high_speed_document();
    source["profiles"][0]["snapTap"]["keyList"][0]
        .as_object_mut()
        .unwrap()
        .remove("mode");
    assert!(decode(719, "real", &source.to_string()).is_err());
}

#[test]
fn source_key_absence_and_a_corrupt_database_are_distinct() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "razer-keyboard-profile-{}-{unique}",
        std::process::id()
    ));
    assert!(read(&root, 679, "real").unwrap().is_none());
    std::fs::create_dir(&root).unwrap();
    let result = read(&root, 679, "real");
    std::fs::remove_dir(&root).unwrap();
    assert!(result.is_err());
}

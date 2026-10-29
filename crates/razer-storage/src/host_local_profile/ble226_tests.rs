use super::*;
use serde_json::json;

fn document() -> Value {
    json!({
        "version":7,"productId":226,"editionId":128,"name":"Razer Basilisk V4 Pro",
        "category":"MOUSE","schemaVersion":13,
        "reportIDs":{"4":"razerKeyReportID","5":"hardwareEventReportID"},
        "calibration":{"smartTracking":{"isAsymmetric":false}},
        "sensitivityMatcher":{"profiles":[]},"selectedSensitivityMatcher":"",
        "activeProfile":"stale-other-serial",
        "deviceMetadatas":{
            "real-serial":{"activeProfileGuid":"selected","name":"Current device","metadataExtension":9},
            "other-serial":{"activeProfileGuid":"first"}
        },
        "rootExtension":{"keep":true},
        "profiles":[
            {"guid":"first","name":"Z-first","lowBatteryEffects":5,
             "ledPowerSettings":{"idleState":1,"ileStateValue":60},"custom":{"keep":"first"}},
            {"guid":"selected","name":"A-selected","obmSlotId":[4,2],
             "dpiStages":{"enable":false,"active":2,"count":2,"stageExtension":7,"stages":[
                 {"x":400,"y":400,"visible":true,"independent":false},
                 {"x":1234,"y":1678,"visible":true,"independent":true,"rowExtension":"keep"}
             ]},"lowBatteryEffects":12,"custom":{"keep":"selected"}}
        ]
    })
}

fn encoded(value: &Value) -> leveldb::Value {
    let mut bytes = vec![1];
    bytes.extend(value.to_string().bytes());
    leveldb::Value {
        sequence: 19,
        bytes,
    }
}

fn catalog() -> Catalog {
    decode("real-serial", encoded(&document())).unwrap()
}

fn device() -> Device {
    let mut device = razer_model::product_preview::registered_preview(PRODUCT_ID).unwrap();
    device.device_container_id = "physical-container".into();
    device.serial_number = "real-serial".into();
    device.profiles = document()["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|source| razer_model::model::Profile {
            id: format!("local-{}", source["guid"].as_str().unwrap()),
            guid: source["guid"].as_str().unwrap().into(),
            name: source["name"].as_str().unwrap().into(),
            source_settings: Some(source.clone()),
            settings: None,
            dpi_stages: None,
        })
        .collect();
    device.active_profile = "local-selected".into();
    let mut root = document();
    root.as_object_mut().unwrap().remove("profiles");
    device.source_device_settings = Some(root);
    device
}

#[test]
fn ble226_catalog_selects_actual_serial_without_sorting_or_losing_external_fields() {
    let source = catalog();
    assert_eq!(source.serial(), "real-serial");
    assert_eq!(source.sequence(), 19);
    assert_eq!(source.sha256().len(), 64);
    assert_eq!(source.source_document().unwrap(), document());
    assert_eq!(
        source
            .profiles()
            .iter()
            .map(Profile::guid)
            .collect::<Vec<_>>(),
        ["first", "selected"]
    );
    assert_eq!(source.profiles()[0].name(), "Z-first");
    assert_eq!(
        source.profiles()[0].source_settings().unwrap()["ledPowerSettings"]["ileStateValue"],
        60
    );
    let request = source
        .initialization_request("local-owner")
        .unwrap()
        .unwrap();
    assert_eq!(request.active_profile_guid, "selected");
    assert_eq!(request.profiles[0].guid, "first");
    assert!(request.profiles[0].obm_slot_ids.is_empty());
    assert!(request.profiles[0].dpi_stages.is_none());
    assert_eq!(request.profiles[1].obm_slot_ids, [4, 2]);
    let stages = request.profiles[1].dpi_stages.as_ref().unwrap();
    assert!(!stages.enabled());
    assert_eq!(stages.active_stage(), 2);
    assert_eq!(
        stages.source_file_value()["stages"][1]["rowExtension"],
        "keep"
    );
    assert!(source.initialization_request("").is_err());
    let mut changed = encoded(&document());
    changed.bytes.push(b' ');
    assert_ne!(
        source.sha256(),
        decode("real-serial", changed).unwrap().sha256()
    );
}

#[test]
fn ble226_missing_store_missing_metadata_and_read_errors_remain_separate() {
    let absent = read_with("real-serial", |key| {
        assert_eq!(key, b"_https://apps.razer.com\0\x01synapse_226");
        Ok(None)
    })
    .unwrap();
    assert!(absent.is_none());
    assert!(read_with("real-serial", |_| anyhow::bail!("corrupt snapshot")).is_err());
    let mut value = document();
    value.as_object_mut().unwrap().remove("deviceMetadatas");
    let source = decode("real-serial", encoded(&value)).unwrap();
    assert_eq!(
        source.selection(),
        &SourceSelection::NeedsSerialInitialization {
            reason: SerialInitialization::MissingMetadata
        }
    );
    assert!(source.initialization_request("owner").unwrap().is_none());
    assert_eq!(source.source_document().unwrap(), value);
    let source = decode("unknown-serial", encoded(&document())).unwrap();
    assert!(source.initialization_request("owner").unwrap().is_none());
    let mut empty = document();
    empty["deviceMetadatas"]["real-serial"]["activeProfileGuid"] = "".into();
    let source = decode("real-serial", encoded(&empty)).unwrap();
    assert_eq!(
        source.selection(),
        &SourceSelection::NeedsSerialInitialization {
            reason: SerialInitialization::EmptyActiveGuid
        }
    );
    assert!(source.initialization_request("owner").unwrap().is_none());
}

#[test]
fn ble226_rejects_null_types_wrong_products_versions_and_unselected_profile_ids() {
    for (pointer, invalid) in [
        ("/productId", json!(182)),
        ("/productId", json!(227)),
        ("/productId", json!(228)),
        ("/editionId", json!(129)),
        ("/category", json!("KEYBOARD")),
        ("/schemaVersion", json!(14)),
        ("/version", json!(1_u64 << 53)),
        ("/reportIDs/4", json!("hardwareEventReportID")),
        ("/deviceMetadatas", Value::Null),
        ("/deviceMetadatas", json!([])),
        (
            "/deviceMetadatas/real-serial/activeProfileGuid",
            json!("absent"),
        ),
        (
            "/deviceMetadatas/real-serial/activeProfileGuid",
            Value::Null,
        ),
        ("/profiles", Value::Null),
        ("/profiles/0/guid", json!("selected")),
        ("/profiles/0/name", Value::Null),
        ("/profiles/1/obmSlotId", Value::Null),
        ("/profiles/1/obmSlotId", json!([0])),
        ("/profiles/1/obmSlotId", json!([2, 2])),
        ("/profiles/1/dpiStages", Value::Null),
        ("/profiles/1/dpiStages/enable", json!("false")),
    ] {
        let mut value = document();
        *value.pointer_mut(pointer).unwrap() = invalid;
        assert!(
            decode("real-serial", encoded(&value)).is_err(),
            "accepted {pointer}"
        );
    }
    for field in [
        "version",
        "productId",
        "editionId",
        "category",
        "profiles",
        "reportIDs",
    ] {
        let mut value = document();
        value.as_object_mut().unwrap().remove(field);
        assert!(
            decode("real-serial", encoded(&value)).is_err(),
            "accepted missing {field}"
        );
    }
}

#[test]
fn ble226_requires_observed_serial_before_any_storage_read_or_device_projection() {
    for serial in [
        "",
        "PREVIEW-226",
        "bad\0serial",
        "NOSERIALNUMBER",
        "DEFERSERIALNUMBER",
    ] {
        assert!(read_with(serial, |_| panic!("invalid serial reached storage")).is_err());
        assert!(from_device(&device(), serial, "owner").is_err());
    }
    for product in [182, 190, 227, 228] {
        let mut local = device();
        local.product_id = product;
        assert!(from_device(&local, "real-serial", "owner").is_err());
    }
    assert!(!super::super::supported(226));
}

#[test]
fn ble226_device_projection_checks_source_identity_and_optional_field_types() {
    let local = device();
    let request = from_device(&local, "real-serial", "owner")
        .unwrap()
        .unwrap();
    assert_eq!(
        request
            .profiles
            .iter()
            .map(|profile| profile.guid.as_str())
            .collect::<Vec<_>>(),
        ["first", "selected"]
    );
    assert_eq!(request.active_profile_guid, "selected");
    assert!(
        from_device(&local, "another-serial", "owner")
            .unwrap()
            .is_none()
    );
    for mutation in 0..7 {
        let mut local = local.clone();
        match mutation {
            0 => local.profiles[0].guid = "different".into(),
            1 => local.profiles[0].name = "different".into(),
            2 => local.profiles[0].source_settings = None,
            3 => local.profiles[0].source_settings.as_mut().unwrap()["obmSlotId"] = Value::Null,
            4 => local.profiles[0].source_settings.as_mut().unwrap()["dpiStages"] = Value::Null,
            5 => local.source_device_settings.as_mut().unwrap()["productId"] = json!(190),
            _ => local.source_device_settings.as_mut().unwrap()["deviceMetadatas"] = Value::Null,
        }
        assert!(from_device(&local, "real-serial", "owner").is_err());
    }
    let mut local = local;
    local
        .source_device_settings
        .as_mut()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("deviceMetadatas");
    assert!(
        from_device(&local, "real-serial", "owner")
            .unwrap()
            .is_none()
    );
}

#[test]
fn ble226_utf16_source_retains_unicode_and_rejects_damaged_payloads() {
    let mut value = document();
    value["profiles"][0]["name"] = "本地名称".into();
    let mut bytes = vec![0];
    bytes.extend(value.to_string().encode_utf16().flat_map(u16::to_le_bytes));
    let source = decode(
        "real-serial",
        leveldb::Value {
            sequence: 8,
            bytes: bytes.clone(),
        },
    )
    .unwrap();
    assert_eq!(source.profiles()[0].name(), "本地名称");
    assert_eq!(source.source_document().unwrap(), value);
    bytes.pop();
    assert!(decode("real-serial", leveldb::Value { sequence: 8, bytes }).is_err());
    assert!(
        decode(
            "real-serial",
            leveldb::Value {
                sequence: 8,
                bytes: vec![1, b'{']
            }
        )
        .is_err()
    );
}

#[test]
fn ble226_import_keeps_observations_local_fields_source_revision_and_profile_order() {
    let source = catalog();
    let mut local = device();
    local.source_device_settings.as_mut().unwrap()["localExtension"] = json!({"keep":true});
    local.profiles[0].source_settings.as_mut().unwrap()["localExtension"] = json!(9);
    local.profiles.reverse();
    let baseline = local.clone();
    let observations = serde_json::to_value(&local.dashboard).unwrap();
    assert!(source.import_into(&mut local, &baseline).unwrap());
    assert_eq!(local.active_profile, "local-selected");
    assert_eq!(
        local
            .profiles
            .iter()
            .map(|profile| profile.guid.as_str())
            .collect::<Vec<_>>(),
        ["first", "selected"]
    );
    assert_eq!(
        local.profiles[0].source_settings.as_ref().unwrap()["localExtension"],
        9
    );
    assert_eq!(
        local.source_device_settings.as_ref().unwrap()["localExtension"],
        json!({"keep":true})
    );
    let revision = &local.source_device_settings.as_ref().unwrap()[REVISION_KEY];
    assert_eq!(revision["sequence"], 19);
    assert_eq!(revision["sha256"], source.sha256());
    assert_eq!(revision["serial"], "real-serial");
    assert_eq!(
        serde_json::to_value(&local.dashboard).unwrap(),
        observations
    );
    let baseline = local.clone();
    assert!(!source.import_into(&mut local, &baseline).unwrap());
    assert!(super::super::owns_field(REVISION_KEY));
}

#[test]
fn ble226_import_rejects_replaced_connections_and_new_drafts_without_partial_mutation() {
    let source = catalog();
    let baseline = device();
    for mutation in 0..7 {
        let mut local = baseline.clone();
        match mutation {
            0 => local.product_id = 190,
            1 => local.device_container_id = "other-container".into(),
            2 => local.serial_number = "other-serial".into(),
            3 => local.profiles[0].source_settings.as_mut().unwrap()["userEdit"] = true.into(),
            4 => local.active_profile = "local-first".into(),
            5 => local.source_device_settings.as_mut().unwrap()["localEdit"] = 1.into(),
            _ => local.device_container_id = "preview-226".into(),
        }
        let before = serde_json::to_value(&local).unwrap();
        assert!(source.import_into(&mut local, &baseline).is_err());
        assert_eq!(serde_json::to_value(&local).unwrap(), before);
    }
    let mut value = document();
    value.as_object_mut().unwrap().remove("deviceMetadatas");
    let source = decode("real-serial", encoded(&value)).unwrap();
    let mut local = baseline.clone();
    assert!(!source.import_into(&mut local, &baseline).unwrap());
    assert_eq!(
        serde_json::to_value(local).unwrap(),
        serde_json::to_value(baseline).unwrap()
    );
}

#[test]
fn ble226_legacy_store_read_migrates_before_typed_validation_and_retains_raw_provenance() {
    let raw = r#"{
        "version":7,"productId":226,"editionId":128,"name":"Razer Basilisk V4 Pro",
        "category":"MOUSE","schemaVersion":0,
        "reportIDs":{"4":"razerKeyReportID","5":"hardwareEventReportID"},
        "rootExtension":{"nested":[null,false,0]},"bossKey":5,"indicatorLedStatus":2,
        "samplingRate":null,"isPreview":false,
        "analogData":{"keep":9,"rapidTrigger":{"discard":true}},
        "sensitivityMatcher":{"profiles":[{"name":"None","guid":"drop"},
            {"name":"Keep","guid":"matcher","extra":null}],"keep":false},
        "activeProfile":"selected",
        "deviceMetadatas":{
            "real-serial":{"activeProfileGuid":"selected","metadataExtension":{"keep":null}},
            "other-serial":{"activeProfileGuid":"first"}
        },
        "profiles":[
            {"guid":"first","name":"Z-first","volume":0,"micVolume":null,
             "streamMixVolume":0,"streamMixerSettings":null,"samplingRate":44100,
             "isHighPassFilterEnabled":false,
             "appEngine":{"rapidTriggers":{"a":{"continuous":[],"keep":1}}},
             "gamingMode":{"isFnZDisabled":null,"keep":true},
             "camera":{"antiFlicker":"1","keep":false},"custom":{"keep":"first"}},
            {"guid":"selected","name":"A-selected","obmSlotId":[4,2],
             "dpiStages":{"enable":false,"active":2,"count":2,"stageExtension":7,"stages":[
                 {"x":400,"y":400,"visible":true,"independent":false},
                 {"x":1234,"y":1678,"visible":true,"independent":true,"rowExtension":"keep"}
             ]},"volume":12,"isStreamMixerEnabled":false,"samplingRate":48000,
             "isHighPassFilterEnabled":true,"isAnalogGainLimiterEnabled":null,
             "appEngine":{"rapidTriggers":{"b":{"continuous":false,"keep":2}}},
             "snapTap":{"keyList":{"key":"A","keep":1}},
             "gamingMode":{"isFnZDisabled":false,"isFnSleepEnabled":true},
             "camera":{"antiFlicker":2}}
        ]
    }"#;
    let mut bytes = vec![1];
    bytes.extend_from_slice(raw.as_bytes());
    let original_bytes = bytes.clone();
    let source = read_with("real-serial", |key| {
        assert_eq!(key, b"_https://apps.razer.com\0\x01synapse_226");
        Ok(Some(leveldb::Value {
            sequence: 23,
            bytes,
        }))
    })
    .unwrap()
    .unwrap();
    assert_eq!(source.sequence(), 23);
    assert_eq!(
        source.sha256(),
        "ea904977d6571896de23b09fe15751756ddafff868de54999fe6e4432c6c6d82"
    );
    assert_eq!(
        source.source_document().unwrap(),
        json!({
            "version":7,"productId":226,"editionId":128,"name":"Razer Basilisk V4 Pro",
            "category":"MOUSE","schemaVersion":13,
            "reportIDs":{"4":"razerKeyReportID","5":"hardwareEventReportID"},
            "rootExtension":{"nested":[null,false,0]},"samplingRate":null,
            "isHighPassFilterEnabled":false,"isAnalogGainLimiterEnabled":null,
            "analogData":{"keep":9,"rapidTrigger":{"continuous":false}},
            "sensitivityMatcher":{"profiles":[{"name":"Keep","guid":"matcher","extra":null}],"keep":false},
            "activeProfile":"selected",
            "deviceMetadatas":{
                "real-serial":{"activeProfileGuid":"selected","metadataExtension":{"keep":null}},
                "other-serial":{"activeProfileGuid":"first"}
            },
            "profiles":[
                {"guid":"first","name":"Z-first","streamMixVolume":0,"bossKey":5,
                 "continuousRapidTrigger":true,
                 "appEngine":{"rapidTriggers":{"a":{"keep":1}}},
                 "gamingMode":{"isFnSleepEnabled":null,"keep":true},
                 "camera":{"antiFlicker":0,"keep":false},"custom":{"keep":"first"}},
                {"guid":"selected","name":"A-selected","obmSlotId":[4,2],"bossKey":5,
                 "dpiStages":{"enable":false,"active":2,"count":2,"stageExtension":7,"stages":[
                     {"x":400,"y":400,"visible":true,"independent":false},
                     {"x":1234,"y":1678,"visible":true,"independent":true,"rowExtension":"keep"}
                 ]},"continuousRapidTrigger":false,
                 "appEngine":{"rapidTriggers":{"b":{"keep":2}}},
                 "snapTap":{"keyList":[{"key":"A","keep":1,"id":1}]},
                 "gamingMode":{"isFnSleepEnabled":false},"camera":{"antiFlicker":2}}
            ]
        })
    );
    let request = source.initialization_request("owner").unwrap().unwrap();
    assert_eq!(request.active_profile_guid, "selected");
    assert_eq!(request.profiles[0].guid, "first");
    assert_eq!(request.profiles[1].guid, "selected");
    assert_eq!(request.profiles[1].obm_slot_ids, [4, 2]);
    assert_eq!(
        decode(
            "real-serial",
            leveldb::Value {
                sequence: 23,
                bytes: original_bytes
            }
        )
        .unwrap()
        .sha256(),
        source.sha256()
    );
}

#[test]
fn ble226_catalog_current_schema_preserves_legacy_looking_extensions_without_migration() {
    let mut source = document();
    source["bossKey"] = json!(7);
    source["sensitivityMatcher"] = Value::Null;
    source["profiles"][0]["volume"] = Value::Null;
    source["profiles"][0]["streamMixerSettings"] = Value::Null;
    source["profiles"][0]["snapTap"] = json!({"keyList":null});
    source["profiles"][0]["gamingMode"] = json!({"isFnZDisabled":false});
    source["profiles"][0]["camera"] = json!({"antiFlicker":"invalid"});
    assert_eq!(
        decode("real-serial", encoded(&source))
            .unwrap()
            .source_document()
            .unwrap(),
        source
    );
}

#[test]
fn ble226_migrated_read_retains_typed_validation_and_never_masks_read_failure() {
    for (pointer, invalid) in [
        ("/productId", json!(182)),
        ("/editionId", json!(129)),
        ("/version", json!(1_u64 << 53)),
        ("/reportIDs/4", json!("hardwareEventReportID")),
        ("/deviceMetadatas", Value::Null),
        (
            "/deviceMetadatas/real-serial/activeProfileGuid",
            json!("absent"),
        ),
        ("/profiles/0/guid", json!("selected")),
        ("/profiles/1/obmSlotId", json!([0])),
        ("/profiles/1/dpiStages", Value::Null),
    ] {
        let mut source = document();
        source["schemaVersion"] = json!(0);
        *source.pointer_mut(pointer).unwrap() = invalid;
        let original = source.clone();
        assert!(
            read_with("real-serial", |_| Ok(Some(encoded(&source)))).is_err(),
            "accepted {pointer}"
        );
        assert_eq!(source, original);
    }
    let error = read_with("real-serial", |_| {
        anyhow::bail!("legacy snapshot read failed")
    })
    .unwrap_err();
    assert!(error.to_string().contains("legacy snapshot read failed"));
    let mut source = document();
    source["schemaVersion"] = json!(0);
    source["sensitivityMatcher"] = Value::Null;
    let before = encoded(&source).bytes;
    assert!(read_with("real-serial", |_| Ok(Some(encoded(&source)))).is_err());
    assert_eq!(encoded(&source).bytes, before);
    source["sensitivityMatcher"] = json!({"profiles":[{"name":"None"},{"name":"kept"}]});
    let recovered = read_with("real-serial", |_| Ok(Some(encoded(&source))))
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered.source_document().unwrap()["sensitivityMatcher"],
        json!({"profiles":[{"name":"kept"}]})
    );
}

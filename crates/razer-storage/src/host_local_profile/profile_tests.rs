use super::*;

fn import(device: &mut Device, source: &LocalProfiles) -> Result<bool> {
    let baseline = device.clone();
    super::import(device, &baseline, source)
}

fn device_with_local_profile() -> Device {
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    device.profiles.push(razer_model::model::Profile {
        source_settings: None,
        settings: None,
        name: "Test local profile".into(),
        guid: "selected".into(),
        id: "test-local-profile".into(),
        dpi_stages: None,
    });
    device.active_profile = "test-local-profile".into();
    device
}

#[test]
#[ignore = "explicit maintenance-only read of the installed host's one target key"]
fn installed_mouse190_profile_readonly() {
    let root = std::path::PathBuf::from(
        std::env::var_os("RAZER_PROFILE_AUDIT_DIRECTORY")
            .expect("maintenance supplies the platform-owned localStorage directory"),
    );
    let key = format!("_{ORIGIN}\0\u{1}synapse_190");
    let value = leveldb::read(&root, key.as_bytes()).expect("strict coherent installed snapshot");
    let Some(value) = value else {
        println!("target=synapse_190 present=false");
        return;
    };
    let json: serde_json::Value = serde_json::from_str(&text(&value.bytes).unwrap()).unwrap();
    println!(
        "target=synapse_190 present=true root_keys={:?}",
        json.as_object().unwrap().keys().collect::<Vec<_>>()
    );
    println!(
        "product_id={} schema_version={}",
        json["productId"], json["schemaVersion"]
    );
    let metadata = json
        .get("deviceMetadatas")
        .and_then(serde_json::Value::as_object)
        .unwrap();
    println!("metadata_records={}", metadata.len());
    for (index, serial) in metadata.keys().enumerate() {
        let profile =
            decode(190, serial, value.clone()).expect("exact current serial/GUID selection");
        println!(
            "record={index} active_guid_matched=true stages_enabled={}",
            stages(&profile).enabled
        );
    }
}
fn document(serial: &str, guid: &str, enabled: bool) -> serde_json::Value {
    serde_json::json!({
        "version":7,"vendorId":5426,"productId":190,"category":"MOUSE","schemaVersion":13,
        "reportIDs":{"4":"razerKeyReportID","5":"hardwareEventReportID"},
        "calibration":{"smartTracking":{"isAsymmetric":false,"trackingDistance":2,"liftOffDistance":13,"landingDistance":12},"profiles":[{"type":"default"}],"selectedProfile":{}},
        "lowPowerMode":30,"sensitivityMatcher":{"profiles":[]},"selectedSensitivityMatcher":"",
        "deviceMetadatas":{serial:{"activeProfileGuid":guid}},"activeProfile":"stale-other-serial",
        "profiles":[{
            "guid":"selected","name":"source profile","dpi":{"x":1200,"y":1600},"mappings":[],
            "pollingRate":1000,"pollingRateWireless":2000,"lowPowerMode":30,"quickEffects":{"selectedEffectId":3},
            "inGamePollingRate":{"state":0,"wired":1000,"wireless":2000},"selectedSensitivityMatcher":"",
            "rotation":{"isEnabled":false,"value":0},
            "indicatorLedConfiguration":[{"ledStatus":2,"index":0},{"ledStatus":1,"index":1},{"ledStatus":3,"index":2}],
            "dynamicSensitivity":{"mode":0,"state":0,"customSensorAccelerations":[],"templateId":0},
            "dpiStages":{"enable":enabled,"active":2,"count":5,"stages":[
                {"x":400,"y":400,"visible":true,"independent":false},
                {"x":1200,"y":1600,"visible":true,"independent":true},
                {"x":2400,"y":2400,"visible":true,"independent":false},
                {"x":3200,"y":3200,"visible":true,"independent":false},
                {"x":6400,"y":6400,"visible":true,"independent":false}
            ]}
        }]
    })
}

fn encoded(document: &serde_json::Value) -> leveldb::Value {
    let mut bytes = vec![1];
    bytes.extend(document.to_string().bytes());
    leveldb::Value {
        sequence: 19,
        bytes,
    }
}

fn stages(source: &LocalProfiles) -> DpiStagesDraft {
    source.active_profile().unwrap().stages().unwrap()
}

fn value(serial: &str, guid: &str, enabled: bool) -> leveldb::Value {
    let json = document(serial, guid, enabled);
    let mut bytes = vec![1];
    bytes.extend(json.to_string().bytes());
    leveldb::Value {
        sequence: 19,
        bytes,
    }
}
#[test]
fn exact_serial_and_guid_are_required_and_false_is_preserved() {
    let local = decode(190, "real", value("real", "selected", false)).unwrap();
    assert!(!stages(&local).enabled);
    assert_eq!(stages(&local).stages[1].y, 1600);
    let other = decode(190, "other", value("real", "selected", false)).unwrap();
    assert_eq!(other.profiles().len(), 1);
    assert_eq!(other.active_guid(), None);
    assert_eq!(
        other.selection(),
        &SourceSelection::NeedsSerialInitialization {
            reason: SerialInitialization::MissingMetadata
        }
    );
    assert!(decode(190, "real", value("real", "missing", false)).is_err());
}
#[test]
fn chromium_encodings_reject_truncated_utf16_and_preserve_non_ascii() {
    assert_eq!(text(&[1, 0xe9]).unwrap(), "é");
    let mut utf16 = vec![0];
    utf16.extend("中文".encode_utf16().flat_map(u16::to_le_bytes));
    assert_eq!(text(&utf16).unwrap(), "中文");
    assert!(text(&[0, 0]).is_err());
    assert!(text(&[2, b'x']).is_err());
    assert!(text(&[0, 0, 0xd8]).is_err());
}
#[test]
fn import_does_not_infer_defaults_and_later_edits_are_not_overwritten() {
    let mut device = device_with_local_profile();
    device.serial_number = "real".into();
    assert!(known_stages(&device).unwrap().is_none());
    let local = decode(190, "real", value("real", "selected", false)).unwrap();
    assert!(import(&mut device, &local).unwrap());
    assert!(!known_stages(&device).unwrap().unwrap().enabled);
    let profile = device.active_profile.clone();
    let mut edited = stage_value(&stages(&local));
    edited["enable"] = true.into();
    mark_edited(
        &mut device,
        &profile,
        &serde_json::json!({"dpiStages":edited}),
    )
    .unwrap();
    assert!(!import(&mut device, &local).unwrap());
    assert!(known_stages(&device).unwrap().unwrap().enabled);
    let changed = decode(190, "real", value("real", "selected", true)).unwrap();
    assert!(import(&mut device, &changed).is_err());
    assert!(known_stages(&device).unwrap().unwrap().enabled);
}
#[test]
fn removed_host_profile_is_unknown_until_a_real_user_edit_has_its_own_ownership() {
    let mut device = device_with_local_profile();
    device.serial_number = "real".into();
    assert!(stages_without_host(&device).unwrap().is_none());
    let source = decode(190, "real", value("real", "selected", false)).unwrap();
    import(&mut device, &source).unwrap();
    assert!(stages_without_host(&device).is_err());
    let id = device.active_profile.clone();
    mark_edited(
        &mut device,
        &id,
        &serde_json::json!({"dpiStages":stage_value(&stages(&source))}),
    )
    .unwrap();
    assert!(!stages_without_host(&device).unwrap().unwrap().enabled);
    device.serial_number = "another-connection".into();
    assert!(known_stages(&device).unwrap().is_none());
    assert!(import(&mut device, &source).is_err());
}

#[test]
fn unchanged_local_ownership_accepts_a_new_actual_host_revision() {
    let mut device = device_with_local_profile();
    device.serial_number = "real".into();
    let old = decode(190, "real", value("real", "selected", false)).unwrap();
    let new = decode(190, "real", value("real", "selected", true)).unwrap();
    import(&mut device, &old).unwrap();
    assert!(import(&mut device, &new).unwrap());
    assert!(known_stages(&device).unwrap().unwrap().enabled);
    assert!(!import(&mut device, &new).unwrap());
}
#[test]
fn durable_ownership_follows_accepted_saves_without_replacing_a_newer_owner() {
    let mut previous = device_with_local_profile();
    previous.serial_number = "real".into();
    let source = decode(190, "real", value("real", "selected", false)).unwrap();
    import(&mut previous, &source).unwrap();
    let id = previous.active_profile.clone();
    let mut saved = previous.clone();
    let mut edited = stages(&source);
    edited.enabled = true;
    mark_edited(
        &mut saved,
        &id,
        &serde_json::json!({"dpiStages":stage_value(&edited)}),
    )
    .unwrap();
    let mut live = previous.clone();
    accept_saved(&mut live, &previous, &saved).unwrap();
    assert!(known_stages(&live).unwrap().unwrap().enabled);
    let restored: Device = serde_json::from_value(serde_json::to_value(&live).unwrap()).unwrap();
    assert!(stages_without_host(&restored).unwrap().unwrap().enabled);
    edited.enabled = false;
    edited.stages[0].x = 900;
    mark_edited(
        &mut live,
        &id,
        &serde_json::json!({"dpiStages":stage_value(&edited)}),
    )
    .unwrap();
    accept_saved(&mut live, &previous, &saved).unwrap();
    assert_eq!(known_stages(&live).unwrap().unwrap().stages[0].x, 900);
}
#[test]
fn mismatched_product_and_unknown_schema_are_not_missing_profiles() {
    assert!(decode(182, "real", value("real", "selected", false)).is_err());
    let mut source = value("real", "selected", false);
    let mut json: serde_json::Value = serde_json::from_str(&text(&source.bytes).unwrap()).unwrap();
    json["schemaVersion"] = 14.into();
    source.bytes = vec![1];
    source.bytes.extend(json.to_string().bytes());
    assert!(decode(190, "real", source).is_err());
}

#[test]
fn ownership_preserves_extensions_and_rejects_corrupt_provenance() {
    let mut device = device_with_local_profile();
    device.serial_number = "real".into();
    let source = decode(190, "real", value("real", "selected", false)).unwrap();
    import(&mut device, &source).unwrap();
    let id = device.active_profile.clone();
    let ownership = &mut device.source_device_settings.as_mut().unwrap()[OWNERSHIP_KEY];
    ownership["futureRoot"] = "retained".into();
    ownership["profiles"][&id]["futureProfile"] = 17.into();
    ownership["profiles"][&id]["imported"]["futureSource"] = true.into();
    mark_edited(
        &mut device,
        &id,
        &serde_json::json!({"dpiStages":stage_value(&stages(&source))}),
    )
    .unwrap();
    let ownership = &device.source_device_settings.as_ref().unwrap()[OWNERSHIP_KEY];
    assert_eq!(ownership["futureRoot"], "retained");
    assert_eq!(ownership["profiles"][&id]["futureProfile"], 17);
    assert_eq!(ownership["profiles"][&id]["imported"]["futureSource"], true);
    device.source_device_settings.as_mut().unwrap()[OWNERSHIP_KEY]["profiles"][&id]["imported"]["serial"] =
        "other".into();
    assert!(known_stages(&device).is_err());
}

#[test]
fn removed_profiles_release_their_ownership_when_the_remaining_profile_is_saved() {
    let mut device = device_with_local_profile();
    device.serial_number = "real".into();
    let source = decode(190, "real", value("real", "selected", false)).unwrap();
    import(&mut device, &source).unwrap();
    let removed_id = device.active_profile.clone();
    let mut profile = device.profiles[0].clone();
    profile.id = "replacement".into();
    device.profiles = vec![profile];
    device.active_profile = "replacement".into();
    mark_edited(
        &mut device,
        "replacement",
        &serde_json::json!({"dpiStages":stage_value(&stages(&source))}),
    )
    .unwrap();
    let state = ownership(&device).unwrap();
    assert!(!state.profiles.contains_key(&removed_id));
    assert!(state.profiles.contains_key("replacement"));
}

#[test]
fn import_creates_real_guid_profiles_without_a_preexisting_default_or_observation() {
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    assert!(device.profiles.is_empty());
    device.serial_number = "real".into();
    let source = decode(190, "real", value("real", "selected", false)).unwrap();
    device.dashboard.connection_observation =
        Some(razer_model::model::DeviceConnectionObservation::UsbPresent);
    let observation = device.dashboard.connection_observation;
    assert!(import(&mut device, &source).unwrap());
    assert_eq!(device.profiles.len(), 1);
    assert_eq!(device.active_profile, "selected");
    assert_eq!(device.profiles[0].id, "selected");
    assert_eq!(device.profiles[0].guid, "selected");
    assert_eq!(device.profiles[0].name, "source profile");
    assert_eq!(
        device.profiles[0].source_settings.as_ref().unwrap()["pollingRateWireless"],
        2000
    );
    assert!(device.profiles[0].settings.is_none());
    assert!(device.profiles[0].dpi_stages.is_none());
    assert_eq!(device.dashboard.connection_observation, observation);
    assert!(!known_stages(&device).unwrap().unwrap().enabled);
}

#[test]
fn complete_projection_preserves_all_profiles_order_and_nested_extensions() {
    let mut source = document("real", "second", true);
    source["futureRoot"] = serde_json::json!({"value":9});
    source["profiles"][0]["futureProfile"] = serde_json::json!([1, 2]);
    source["profiles"][0]["dpiStages"]["stages"][0]["futureStage"] = "kept".into();
    source["profiles"][0]["rotation"]["futureRotation"] = false.into();
    let mut second = source["profiles"][0].clone();
    second["guid"] = "second".into();
    second["name"] = "actual second".into();
    source["profiles"].as_array_mut().unwrap().push(second);
    let decoded = decode(190, "real", encoded(&source)).unwrap();
    let projected = decoded.source_document().unwrap();
    source["activeProfile"] = "second".into();
    assert_eq!(projected, source);
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    device.serial_number = "real".into();
    import(&mut device, &decoded).unwrap();
    assert_eq!(
        device
            .profiles
            .iter()
            .map(|profile| profile.guid.as_str())
            .collect::<Vec<_>>(),
        ["selected", "second"]
    );
    assert_eq!(device.active_profile, "second");
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["futureRoot"]["value"],
        9
    );
}

#[test]
fn every_unselected_profile_is_validated_and_an_empty_library_needs_serial_initialization() {
    let mut source = document("real", "selected", true);
    let mut bad = source["profiles"][0].clone();
    bad["guid"] = "unselected".into();
    bad["pollingRate"] = 71.into();
    source["profiles"].as_array_mut().unwrap().push(bad);
    assert!(decode(190, "real", encoded(&source)).is_err());
    source["profiles"] = serde_json::json!([]);
    source.as_object_mut().unwrap().remove("deviceMetadatas");
    let decoded = decode(190, "real", encoded(&source)).unwrap();
    assert!(decoded.profiles().is_empty());
    assert!(matches!(
        decoded.selection(),
        SourceSelection::NeedsSerialInitialization {
            reason: SerialInitialization::MissingMetadata
        }
    ));
}

#[test]
fn current182_uses_its_real_full_profile_and_calibration_branch() {
    let mut source = razer_device::profile_transfer::default_device_182();
    let mut profile = razer_device::profile_transfer::default_profile_182();
    profile["guid"] = "mouse182-real-guid".into();
    profile["name"] = "source182".into();
    source["schemaVersion"] = 13.into();
    source["profiles"] = serde_json::json!([profile]);
    source["deviceMetadatas"] =
        serde_json::json!({"serial182":{"activeProfileGuid":"mouse182-real-guid"}});
    let decoded = decode(182, "serial182", encoded(&source)).unwrap();
    assert_eq!(decoded.active_guid(), Some("mouse182-real-guid"));
    source["profiles"][0]
        .as_object_mut()
        .unwrap()
        .remove("ledPowerSettings");
    assert!(decode(182, "serial182", encoded(&source)).is_err());
}

#[test]
fn missing_empty_and_invalid_serial_selections_have_different_meanings() {
    let source = decode(190, "real", value("real", "", false)).unwrap();
    assert_eq!(
        source.selection(),
        &SourceSelection::NeedsSerialInitialization {
            reason: SerialInitialization::EmptyActiveGuid
        }
    );
    assert_eq!(source.active_guid(), None);
    assert!(decode(190, "real", value("real", "dangling", false)).is_err());
    let mut source = document("real", "selected", false);
    source["deviceMetadatas"]["real"]["activeProfileGuid"] = serde_json::Value::Null;
    assert!(decode(190, "real", encoded(&source)).is_err());
}

#[test]
fn wrong_missing_and_null_full_profile_fields_are_errors() {
    let base = document("real", "selected", false);
    for field in [
        "dpi",
        "mappings",
        "pollingRate",
        "rotation",
        "dynamicSensitivity",
    ] {
        let mut source = base.clone();
        source["profiles"][0].as_object_mut().unwrap().remove(field);
        assert!(
            decode(190, "real", encoded(&source)).is_err(),
            "missing {field}"
        );
        source["profiles"][0][field] = serde_json::Value::Null;
        assert!(
            decode(190, "real", encoded(&source)).is_err(),
            "null {field}"
        );
    }
    let mut source = base.clone();
    source["profiles"]
        .as_array_mut()
        .unwrap()
        .push(base["profiles"][0].clone());
    assert!(decode(190, "real", encoded(&source)).is_err());
    assert!(decode(190, "PREVIEW-190", encoded(&base)).is_err());
}

#[test]
fn captured_local_revision_rejects_non_dpi_edits_before_the_catalog_can_replace_them() {
    let mut device = device_with_local_profile();
    device.serial_number = "real".into();
    let source = decode(190, "real", value("real", "selected", false)).unwrap();
    import(&mut device, &source).unwrap();
    let baseline = device.clone();
    device.profiles[0].source_settings.as_mut().unwrap()["rotation"]["value"] = 12.into();
    device.profiles[0].name = "user's pending name".into();
    let before = serde_json::to_value(&device).unwrap();
    assert!(super::import(&mut device, &baseline, &source).is_err());
    assert_eq!(serde_json::to_value(&device).unwrap(), before);
}

#[test]
fn a_new_serial_and_actual_connection_observations_do_not_invalidate_the_local_revision() {
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    device.device_container_id = "actual-container".into();
    let baseline = device.clone();
    device.serial_number = "real".into();
    device.dashboard.connection_observation =
        Some(razer_model::model::DeviceConnectionObservation::ReceiverPeer(1));
    let source = decode(190, "real", value("real", "selected", false)).unwrap();
    assert!(super::import(&mut device, &baseline, &source).unwrap());
    assert_eq!(device.serial_number, "real");
    assert_eq!(
        device.dashboard.connection_observation,
        Some(razer_model::model::DeviceConnectionObservation::ReceiverPeer(1))
    );
    let baseline = device.clone();
    device.device_container_id = "successor-container".into();
    let before = serde_json::to_value(&device).unwrap();
    assert!(super::import(&mut device, &baseline, &source).is_err());
    assert_eq!(serde_json::to_value(&device).unwrap(), before);
}

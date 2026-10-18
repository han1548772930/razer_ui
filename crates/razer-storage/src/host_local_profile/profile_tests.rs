use super::*;
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
    let document: Document = serde_json::from_value(json).expect("verified current schema");
    println!("metadata_records={}", document.metadata.len());
    for (index, serial) in document.metadata.keys().enumerate() {
        let profile = decode(190, serial, value.clone())
            .expect("exact current serial/GUID selection")
            .unwrap();
        println!(
            "record={index} active_guid_matched=true stages_enabled={}",
            profile.stages.enabled
        );
    }
}
fn value(serial: &str, guid: &str, enabled: bool) -> leveldb::Value {
    let json = serde_json::json!({"productId":190,"schemaVersion":13,"deviceMetadatas":{serial:{"activeProfileGuid":guid}},"profiles":[{"guid":"selected","name":"source profile","dpiStages":{"enable":enabled,"active":2,"count":5,"stages":[{"x":400,"y":400,"visible":true,"independent":false},{"x":1200,"y":1600,"visible":true,"independent":true},{"x":2400,"y":2400,"visible":true,"independent":false},{"x":3200,"y":3200,"visible":true,"independent":false},{"x":6400,"y":6400,"visible":true,"independent":false}]}}]});
    let mut bytes = vec![1];
    bytes.extend(json.to_string().bytes());
    leveldb::Value {
        sequence: 19,
        bytes,
    }
}
#[test]
fn exact_serial_and_guid_are_required_and_false_is_preserved() {
    let local = decode(190, "real", value("real", "selected", false))
        .unwrap()
        .unwrap();
    assert!(!local.stages.enabled);
    assert_eq!(local.stages.stages[1].y, 1600);
    assert!(
        decode(190, "other", value("real", "selected", false))
            .unwrap()
            .is_none()
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
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    device.serial_number = "real".into();
    assert!(known_stages(&device).unwrap().is_none());
    let local = decode(190, "real", value("real", "selected", false))
        .unwrap()
        .unwrap();
    assert!(import(&mut device, &local).unwrap());
    assert!(!known_stages(&device).unwrap().unwrap().enabled);
    let profile = device.active_profile.clone();
    let mut edited = stage_value(&local.stages);
    edited["enable"] = true.into();
    mark_edited(
        &mut device,
        &profile,
        &serde_json::json!({"dpiStages":edited}),
    )
    .unwrap();
    assert!(!import(&mut device, &local).unwrap());
    assert!(known_stages(&device).unwrap().unwrap().enabled);
    let changed = decode(190, "real", value("real", "selected", true))
        .unwrap()
        .unwrap();
    assert!(import(&mut device, &changed).is_err());
    assert!(known_stages(&device).unwrap().unwrap().enabled);
}
#[test]
fn removed_host_profile_is_unknown_until_a_real_user_edit_has_its_own_ownership() {
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    device.serial_number = "real".into();
    assert!(stages_without_host(&device).unwrap().is_none());
    let source = decode(190, "real", value("real", "selected", false))
        .unwrap()
        .unwrap();
    import(&mut device, &source).unwrap();
    assert!(stages_without_host(&device).is_err());
    let id = device.active_profile.clone();
    mark_edited(
        &mut device,
        &id,
        &serde_json::json!({"dpiStages":stage_value(&source.stages)}),
    )
    .unwrap();
    assert!(!stages_without_host(&device).unwrap().unwrap().enabled);
    device.serial_number = "another-connection".into();
    assert!(known_stages(&device).unwrap().is_none());
    assert!(import(&mut device, &source).is_err());
}

#[test]
fn unchanged_local_ownership_accepts_a_new_actual_host_revision() {
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    device.serial_number = "real".into();
    let old = decode(190, "real", value("real", "selected", false))
        .unwrap()
        .unwrap();
    let new = decode(190, "real", value("real", "selected", true))
        .unwrap()
        .unwrap();
    import(&mut device, &old).unwrap();
    assert!(import(&mut device, &new).unwrap());
    assert!(known_stages(&device).unwrap().unwrap().enabled);
    assert!(!import(&mut device, &new).unwrap());
}
#[test]
fn durable_ownership_follows_accepted_saves_without_replacing_a_newer_owner() {
    let mut previous = razer_model::product_preview::registered_preview(190).unwrap();
    previous.serial_number = "real".into();
    let source = decode(190, "real", value("real", "selected", false))
        .unwrap()
        .unwrap();
    import(&mut previous, &source).unwrap();
    let id = previous.active_profile.clone();
    let mut saved = previous.clone();
    let mut edited = source.stages.clone();
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
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    device.serial_number = "real".into();
    let source = decode(190, "real", value("real", "selected", false))
        .unwrap()
        .unwrap();
    import(&mut device, &source).unwrap();
    let id = device.active_profile.clone();
    let ownership = &mut device.source_device_settings.as_mut().unwrap()[OWNERSHIP_KEY];
    ownership["futureRoot"] = "retained".into();
    ownership["profiles"][&id]["futureProfile"] = 17.into();
    ownership["profiles"][&id]["imported"]["futureSource"] = true.into();
    mark_edited(
        &mut device,
        &id,
        &serde_json::json!({"dpiStages":stage_value(&source.stages)}),
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
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    device.serial_number = "real".into();
    let source = decode(190, "real", value("real", "selected", false))
        .unwrap()
        .unwrap();
    import(&mut device, &source).unwrap();
    let removed_id = device.active_profile.clone();
    let mut profile = device.profiles[0].clone();
    profile.id = "replacement".into();
    device.profiles = vec![profile];
    device.active_profile = "replacement".into();
    mark_edited(
        &mut device,
        "replacement",
        &serde_json::json!({"dpiStages":stage_value(&source.stages)}),
    )
    .unwrap();
    let state = ownership(&device).unwrap();
    assert!(!state.profiles.contains_key(&removed_id));
    assert!(state.profiles.contains_key("replacement"));
}

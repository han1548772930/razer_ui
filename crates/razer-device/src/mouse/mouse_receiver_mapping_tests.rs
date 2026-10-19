use super::*;
use serde_json::json;

fn guids() -> impl Iterator<Item = String> {
    (0..64).map(|index| format!("00000000-0000-4000-8000-{index:012}"))
}

fn profile(mappings: Value) -> Value {
    json!({"guid":"profile-a","name":"Local","mappings":mappings,"dpi":{"x":800,"y":800}})
}

#[test]
fn defaults_use_only_current_device_dkm_identifiers() {
    let document = prepare_document(&[profile(json!([]))], "profile-a", None, guids()).unwrap();
    let mappings = document["defaultMappings"]["appEngine"]["mappings"]
        .as_array()
        .unwrap();
    assert_eq!(mappings.len(), 12);
    for mapping in mappings {
        assert_eq!(mapping["input"]["type"], "razerKey");
        assert!(matches!(
            mapping["input"]["key"].as_u64(),
            Some(32 | 33 | 82)
        ));
    }
    assert_eq!(document["productId"], 158);
    assert_eq!(document["profiles"][0]["dpi"], json!({"x":800,"y":800}));
}

#[test]
fn saved_records_keep_unregistered_keys_without_inventing_native_inputs() {
    let unknown = json!({"inputType":"DKMInput","inputID":"DKM_SB_01","outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[]}});
    let known = json!({"inputType":"MouseInput","inputID":"Button4","outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B","modifiers":[]}});
    let document = prepare_document(
        &[profile(json!([unknown, known]))],
        "profile-a",
        None,
        guids(),
    )
    .unwrap();
    assert_eq!(
        document["profiles"][0]["mappings"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let mappings = document["profiles"][0]["appEngine"]["mappings"]
        .as_array()
        .unwrap();
    assert_eq!(mappings.len(), 2);
    assert!(
        mappings
            .iter()
            .all(|mapping| mapping["input"]["type"] == "mouse")
    );
}

#[test]
fn compilation_preserves_turbos_extensions_and_rejects_projection_overwrite() {
    let input = profile(json!([]));
    let mut previous = prepare_document(&[input.clone()], "profile-a", None, guids()).unwrap();
    previous["extension"] = json!({"value":7});
    let next = prepare_document(&[input.clone()], "profile-a", Some(&previous), []).unwrap();
    assert_eq!(next["defaultTurbos"], previous["defaultTurbos"]);
    assert_eq!(next["extension"], previous["extension"]);
    assert!(next["version"].as_u64().unwrap() > previous["version"].as_u64().unwrap());
    previous["dualLinked"] = json!(true);
    assert!(prepare_document(&[input], "profile-a", Some(&previous), []).is_err());
}

#[test]
fn malformed_or_ambiguous_profiles_fail_without_mutating_input() {
    let input = profile(json!([]));
    let snapshot = input.clone();
    assert!(prepare_document(&[input.clone(), input.clone()], "profile-a", None, guids()).is_err());
    assert!(prepare_document(&[input.clone()], "absent", None, guids()).is_err());
    assert!(prepare_document(&[input.clone()], "profile-a", Some(&json!({})), guids()).is_err());
    assert_eq!(input, snapshot);
}

#[test]
fn receiver_projection_uses_observed_serial_and_keeps_local_documents_unchanged() {
    let first = profile(json!([]));
    let mut second = profile(json!([]));
    second["guid"] = json!("profile-b");
    let master = prepare_document(&[first, second], "profile-b", None, guids()).unwrap();
    let master = select_profile_for_serial(&master, "master-serial").unwrap();
    let mut slave = prepare_document(&[profile(json!([]))], "profile-a", None, guids()).unwrap();
    slave["productId"] = json!(515);
    slave["category"] = json!("KEYBOARD");
    slave["profiles"][0]["appEngine"] = json!({"hash":"old","gamemode":3,"mappings":[{
        "input":{"type":"keyboard","key":30},"output":{"type":"cascade","cascade":[
            {"input":{"type":"razerKey","key":9},"output":{"type":"none"}}
        ]}
    }]});
    let saved_master = master.clone();
    let saved_slave = slave.clone();
    let merged = compose_receiver(
        &master,
        159,
        "master-serial",
        ReceiverRoute::Normal,
        Some(SlaveSnapshot {
            document: &slave,
            serial: "slave-serial",
        }),
    )
    .unwrap();
    assert_eq!(merged["productId"], 159);
    assert_eq!(merged["dualLinked"], true);
    assert_eq!(merged["profiles"][0]["appEngine"]["mappings"], json!([]));
    let active = &merged["profiles"][1]["appEngine"];
    assert_eq!(active["mappings"][0]["input"]["unitId"], 43981);
    assert_eq!(
        active["mappings"][0]["output"]["cascade"][0]["input"]["reportId"],
        8
    );
    assert_eq!(active["gamemode"], 3);
    assert_eq!(active["hash"], source_hash(active).unwrap());
    assert_eq!(merged["reportIDs"]["8"], "razerKeyReportID");
    assert_eq!(master, saved_master);
    assert_eq!(slave, saved_slave);
    assert!(compose_receiver(&master, 159, "", ReceiverRoute::Normal, None).is_err());
    assert!(compose_receiver(&master, 160, "master-serial", ReceiverRoute::Normal, None).is_err());
}

#[test]
fn linker_projection_overrides_slave_report_identity_last() {
    let master = prepare_document(&[profile(json!([]))], "profile-a", None, guids()).unwrap();
    let mut slave = master.clone();
    slave["reportIDs"]["4"] = json!("hardwareEventReportID");
    let merged = compose_receiver(
        &master,
        159,
        "master",
        ReceiverRoute::Linker,
        Some(SlaveSnapshot {
            document: &slave,
            serial: "slave",
        }),
    )
    .unwrap();
    assert_eq!(merged["reportIDs"]["4"], "razerKeyReportID");
    assert!(
        merged["defaultMappings"]["appEngine"]["mappings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|mapping| mapping["input"]["reportId"] == 4)
    );
}

#[test]
fn interdevice_otfs_uses_source_scroll_controls_without_changing_saved_profiles() {
    let first = profile(json!([
        {"inputType":"MouseInput","inputID":"Button4","outputType":"keyboardGroup",
         "keyboardGroup":{"key":"KEY_A","modifiers":[]}},
        {"inputType":"MouseInput","inputID":"Button5","outputType":"sensitivityGroup",
         "sensitivityGroup":{"sensitivityAssignment":"DPI_OnTheFly"}}
    ]));
    let mut second = profile(json!([]));
    second["guid"] = json!("profile-b");
    let mut ordinary = prepare_document(&[first, second], "profile-a", None, guids()).unwrap();
    ordinary["deviceMetadatas"] = json!({"observed-serial":{"activeProfileGuid":"profile-a"}});
    ordinary["unknown-extension"] = json!({"nested":17});
    let original = ordinary.clone();
    let temporary = interdevice_otfs_document(&ordinary).unwrap();
    assert_eq!(ordinary, original);
    for field in ["version", "defaultTurbos", "synapseTurbos", "deviceMetadatas", "unknown-extension"] {
        assert_eq!(temporary[field], ordinary[field]);
    }
    assert_eq!(temporary["profiles"][0]["mappings"], ordinary["profiles"][0]["mappings"]);
    assert_eq!(temporary["profiles"][1], ordinary["profiles"][1]);
    let mappings = temporary["profiles"][0]["appEngine"]["mappings"].as_array().unwrap();
    assert!(mappings.iter().all(|mapping| mapping["input"]["otfs"] != true));
    let up = mappings.iter().find(|mapping| mapping["input"]["id"] == 1024 && mapping["input"]["data"] == 120).unwrap();
    let down = mappings.iter().find(|mapping| mapping["input"]["id"] == 1024 && mapping["input"]["data"] == 65416).unwrap();
    assert_eq!(up["output"]["type"], "sensitivity");
    assert_eq!(up["output"]["sensitivityGroup"],
        json!({"sensitivityAssignment":"DPI_Value_Up","value":50,"independent":false}));
    assert_eq!(down["output"]["sensitivityGroup"],
        json!({"sensitivityAssignment":"DPI_Value_Down","value":50,"independent":false}));
    assert!(!mappings.iter().any(|mapping| mapping["output"]["type"] == "keyboard"));
    assert_eq!(temporary["profiles"][0]["defaultMappings"], temporary["defaultMappings"]["appEngine"]);
    assert_eq!(temporary["profiles"][0]["appEngine"]["hash"],
        source_hash(&temporary["profiles"][0]["appEngine"]).unwrap());
}

#[test]
fn interdevice_otfs_rejects_wrong_product_projection_and_invalid_profile() {
    let ordinary = prepare_document(&[profile(json!([]))], "profile-a", None, guids()).unwrap();
    for (field, value) in [
        ("productId", json!(70)),
        ("dualLinked", json!(true)),
        ("activeProfile", json!("absent")),
        ("defaultTurbos", Value::Null),
    ] {
        let mut invalid = ordinary.clone();
        invalid[field] = value;
        assert!(interdevice_otfs_document(&invalid).is_err(), "{field}");
    }
    let mut invalid = ordinary;
    invalid["profiles"][0]["mappings"] = json!([{"inputType":"MouseInput","inputID":"Button4"}]);
    assert!(interdevice_otfs_document(&invalid).is_err());
}

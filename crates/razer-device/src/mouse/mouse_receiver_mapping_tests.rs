use super::*;

#[test]
fn ordinary_shared_compiler_batch_keeps_actual_default_inputs_and_report_ids() {
    for (pid, defaults, keys) in [
        (89, 10, 5),
        (104, 10, 3),
        (115, 4, 3),
        (140, 4, 1),
        (145, 6, 3),
        (148, 6, 3),
        (161, 4, 1),
        (178, 6, 3),
        (185, 8, 2),
        (194, 8, 3),
        (196, 2, 1),
        (224, 8, 3),
    ] {
        assert_eq!(default_bindings_for(pid).unwrap().len(), defaults);
        assert_eq!(dkm_keys(pid).len(), keys);
        let doc = prepare_document_for(
            pid,
            &[profile(serde_json::json!([]))],
            "profile-a",
            None,
            None,
            guids(),
        )
        .unwrap();
        assert_eq!(doc["productId"], pid);
        assert_eq!(
            doc["reportIDs"].as_object().unwrap().len(),
            if pid == 145 { 1 } else { 2 }
        );
        assert_eq!(doc["reportIDs"]["4"], "razerKeyReportID");
        let mappings = doc["defaultMappings"]["appEngine"]["mappings"]
            .as_array()
            .unwrap();
        assert!(!mappings.is_empty());
        if matches!(pid, 140 | 161 | 224) {
            assert_eq!(
                mappings
                    .iter()
                    .filter(|row| row["input"]["otfs"] == true)
                    .count(),
                20
            );
        }
        if pid == 185 {
            assert!(mappings.iter().all(|row| row["input"]["type"] != "razerKey"
                || !matches!(row["input"]["key"].as_u64(), Some(34 | 35))));
        }
        let snapshot = crate::mouse_otfs::Snapshot::capture(&doc).unwrap();
        let temporary = crate::mouse_otfs::project(&doc, &snapshot, true)
            .unwrap()
            .unwrap();
        let restored = crate::mouse_otfs::project(&temporary, &snapshot, false)
            .unwrap()
            .unwrap();
        assert_eq!(
            restored["profiles"][0]["appEngine"]["hash"],
            doc["profiles"][0]["appEngine"]["hash"]
        );
    }
    let rows = default_bindings_for(104).unwrap();
    assert_eq!(rows[6]["mouseGroup"]["mouseAssignment"], "RepeatScrollDown");
    assert_eq!(rows[8]["mouseGroup"]["mouseAssignment"], "RepeatScrollUp");
    assert!(dkm_keys(104).iter().all(|key| key.button_key.is_none()));
}

#[test]
fn ordinary_defaults_keep_initialized_reports_and_capitalized_navigation() {
    for pid in [96, 112] {
        let doc =
            prepare_document_for(pid, &[profile(json!([]))], "profile-a", None, None, guids())
                .unwrap();
        assert_eq!(default_bindings_for(pid).unwrap().len(), 10);
        assert_eq!(dkm_keys(pid).len(), 5);
        assert_eq!(
            doc["reportIDs"],
            json!({"4":"razerKeyReportID","5":"hardwareEventReportID"})
        );
        let mappings = doc["defaultMappings"]["appEngine"]["mappings"]
            .as_array()
            .unwrap();
        let navigations: Vec<_> = mappings
            .iter()
            .filter(|row| row["output"]["type"] == "navigateProfile")
            .collect();
        assert_eq!(navigations.len(), 2);
        for row in navigations {
            assert_eq!(row["input"]["key"], 80);
            if pid == 112 {
                assert!(row["output"].get("name").is_none());
            } else {
                assert_eq!(row["output"]["name"], "CycleUp");
            }
        }
        let snapshot = crate::mouse_otfs::Snapshot::capture(&doc).unwrap();
        let temporary = crate::mouse_otfs::project(&doc, &snapshot, true)
            .unwrap()
            .unwrap();
        let restored = crate::mouse_otfs::project(&temporary, &snapshot, false)
            .unwrap()
            .unwrap();
        assert_eq!(restored.get("reportIDs"), doc.get("reportIDs"));
        assert_eq!(
            restored["profiles"][0]["appEngine"]["hash"],
            doc["profiles"][0]["appEngine"]["hash"]
        );
        let mut malformed = doc;
        malformed["reportIDs"] = Value::Null;
        assert!(Document::parse(&malformed).is_err());
    }
    assert!(dkm_keys(96).iter().all(|key| key.button_key.is_none()));
    let defaults = default_bindings_for(112).unwrap();
    assert_eq!(defaults[4]["inputID"], "DKM_M_01");
    assert_eq!(
        defaults[5]["profileNavigationGroup"],
        json!({"ProfileNavigationAssignment":"CycleUp"})
    );
    assert_eq!(
        defaults[9]["profileNavigationGroup"],
        json!({"ProfileNavigationAssignment":"CycleUp"})
    );
}
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
fn ordinary_product_batch_uses_actual_dkm_defaults_and_retains_profile_fields() {
    for pid in [190, 192, 222, 229, 239] {
        let input = profile(json!([{"inputType":"DKMInput","inputID":"DKM_SB_03",
            "outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[]}}]));
        let document =
            prepare_document_for(pid, &[input.clone()], "profile-a", None, None, guids()).unwrap();
        assert_eq!(document["productId"], pid);
        assert_eq!(document["profiles"][0]["dpi"], input["dpi"]);
        let defaults = document["defaultMappings"]["appEngine"]["mappings"]
            .as_array()
            .unwrap();
        // Each source wheel mapping expands into ten report magnitudes.
        assert_eq!(defaults.len(), if pid == 239 { 24 } else { 4 });
        assert!(
            defaults
                .iter()
                .filter(|row| row["input"]["type"] == "razerKey")
                .all(|row| row["input"]["key"] == 82)
        );
        assert!(defaults.iter().any(|row| row["input"]["otfs"] == true) == (pid == 239));
        let next =
            prepare_document_for(pid, &[input], "profile-a", Some(&document), None, []).unwrap();
        assert_eq!(next["defaultTurbos"], document["defaultTurbos"]);
        let temporary = interdevice_otfs_document(&next).unwrap();
        assert_eq!(temporary["productId"], pid);
        let snapshot = crate::mouse_otfs::Snapshot::capture(&next).unwrap();
        let restored = crate::mouse_otfs::project(&temporary, &snapshot, false)
            .unwrap()
            .unwrap();
        assert_eq!(
            restored["profiles"][0]["mappings"],
            next["profiles"][0]["mappings"]
        );
    }
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
fn unregistered_dkm_output_keeps_turbo_identity_in_the_saved_document() {
    let mapping = json!({
        "inputType":"DKMInput","inputID":"DKM_SB_01","outputType":"keyboardGroup",
        "keyboardGroup":{"key":"KEY_A","modifiers":[],"turboMode":{"keysPerSecond":10}}
    });
    let document = prepare_document(
        &[profile(json!([mapping.clone()]))],
        "profile-a",
        None,
        guids(),
    )
    .unwrap();
    assert_eq!(document["profiles"][0]["mappings"], json!([mapping]));
    assert_eq!(document["profiles"][0]["appEngine"]["mappings"], json!([]));
    assert_eq!(document["synapseTurbos"].as_array().unwrap().len(), 1);
    let next = prepare_document(
        &[document["profiles"][0].clone()],
        "profile-a",
        Some(&document),
        [],
    )
    .unwrap();
    assert_eq!(next["synapseTurbos"], document["synapseTurbos"]);
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
fn multiple_peer_projection_keeps_category_channels_and_exact_output_owners() {
    let mut master = prepare_document(&[profile(json!([]))], "profile-a", None, guids()).unwrap();
    master["profiles"][0]["appEngine"]["mappings"] = json!([{
        "input":{"type":"mouse","id":1},
        "output":{"type":"cascade","cascade":[{
            "input":{"type":"keyboard","scancode":30},
            "output":{"type":"mouse","id":4}
        }]}
    }]);
    let mut keyboard = master.clone();
    keyboard["productId"] = json!(679);
    keyboard["category"] = json!("KEYBOARD");
    keyboard["profiles"][0]["appEngine"]["gamemode"] = json!(true);
    let mut mouse = master.clone();
    mouse["productId"] = json!(190);
    mouse["profiles"][0]["appEngine"]["gamemode"] = json!(false);
    let original = master.clone();
    let peers = [
        ReceiverPeer::new(
            PeerIdentity::new(679, 680).unwrap(),
            Some(&keyboard),
            Some("keyboard-serial"),
        ),
        ReceiverPeer::new(
            PeerIdentity::new(190, 191).unwrap(),
            Some(&mouse),
            Some("mouse-serial"),
        ),
    ];
    let merged = ReceiverProjection::new(159, ReceiverRoute::Normal, true)
        .unwrap()
        .project(&master, None, &peers)
        .unwrap();
    let rows = merged["profiles"][0]["appEngine"]["mappings"]
        .as_array()
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["input"]["unitId"], 0);
    assert_eq!(rows[0]["output"]["multiDevicesInfo"], json!({}));
    assert_eq!(rows[0]["output"]["cascade"][0]["input"]["unitId"], 0);
    assert_eq!(rows[1]["input"]["unitId"], 43981);
    assert_eq!(
        rows[1]["output"]["multiDevicesInfo"],
        json!({"productId":679,"dongleId":680})
    );
    assert_eq!(
        rows[1]["output"]["cascade"][0]["output"]["multiDevicesInfo"],
        json!({"productId":679,"dongleId":680})
    );
    assert_eq!(rows[2]["input"]["unitId"], 0);
    assert_eq!(
        rows[2]["output"]["multiDevicesInfo"],
        json!({"productId":190,"dongleId":191})
    );
    assert_eq!(merged["profiles"][0]["appEngine"]["gamemode"], true);
    assert_eq!(master, original);
    let defaults = merged["defaultMappings"]["appEngine"]["mappings"]
        .as_array()
        .unwrap();
    assert!(
        defaults[..12]
            .iter()
            .all(|row| row["output"].get("multiDevicesInfo").is_none())
    );
    assert!(
        defaults[12..24]
            .iter()
            .all(|row| row["input"]["reportId"] == 8)
    );
    assert!(
        defaults[24..]
            .iter()
            .all(|row| row["input"]["reportId"] == 4)
    );
    assert_eq!(
        merged["profiles"][0]["appEngine"]["hash"],
        source_hash(&merged["profiles"][0]["appEngine"]).unwrap()
    );
}

#[test]
fn receiver_projection_keeps_unobserved_serial_separate_from_an_empty_peer_list() {
    let master = prepare_document(&[profile(json!([]))], "profile-a", None, guids()).unwrap();
    let peer = PeerIdentity::new(158, 159).unwrap();
    let peers = [
        ReceiverPeer::new(peer, Some(&master), None),
        ReceiverPeer::new(peer, Some(&master), Some("serial")),
    ];
    let single = ReceiverProjection::new(179, ReceiverRoute::Normal, false)
        .unwrap()
        .project(&master, Some("master"), &peers)
        .unwrap();
    let multiple = ReceiverProjection::new(179, ReceiverRoute::Normal, true)
        .unwrap()
        .project(&master, Some("master"), &peers)
        .unwrap();
    assert_eq!(
        single["defaultMappings"]["appEngine"]["mappings"]
            .as_array()
            .unwrap()
            .len(),
        12
    );
    assert_eq!(
        multiple["defaultMappings"]["appEngine"]["mappings"]
            .as_array()
            .unwrap()
            .len(),
        24
    );
    for route in [ReceiverRoute::Linker, ReceiverRoute::Mika] {
        let merged = ReceiverProjection::new(179, route, true)
            .unwrap()
            .project(&master, None, &peers)
            .unwrap();
        assert!(
            merged["defaultMappings"]["appEngine"]["mappings"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["input"]["reportId"] == 4)
        );
    }
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
    for field in [
        "version",
        "defaultTurbos",
        "synapseTurbos",
        "deviceMetadatas",
        "unknown-extension",
    ] {
        assert_eq!(temporary[field], ordinary[field]);
    }
    assert_eq!(
        temporary["profiles"][0]["mappings"],
        ordinary["profiles"][0]["mappings"]
    );
    assert_eq!(temporary["profiles"][1], ordinary["profiles"][1]);
    let mappings = temporary["profiles"][0]["appEngine"]["mappings"]
        .as_array()
        .unwrap();
    assert!(
        mappings
            .iter()
            .all(|mapping| mapping["input"]["otfs"] != true)
    );
    let up = mappings
        .iter()
        .find(|mapping| mapping["input"]["id"] == 1024 && mapping["input"]["data"] == 120)
        .unwrap();
    let down = mappings
        .iter()
        .find(|mapping| mapping["input"]["id"] == 1024 && mapping["input"]["data"] == 65416)
        .unwrap();
    assert_eq!(up["output"]["type"], "sensitivity");
    assert_eq!(
        up["output"]["sensitivityGroup"],
        json!({"sensitivityAssignment":"DPI_Value_Up","value":50,"independent":false})
    );
    assert_eq!(
        down["output"]["sensitivityGroup"],
        json!({"sensitivityAssignment":"DPI_Value_Down","value":50,"independent":false})
    );
    assert!(
        !mappings
            .iter()
            .any(|mapping| mapping["output"]["type"] == "keyboard")
    );
    assert_eq!(
        temporary["profiles"][0]["defaultMappings"],
        temporary["defaultMappings"]["appEngine"]
    );
    assert_eq!(
        temporary["profiles"][0]["appEngine"]["hash"],
        source_hash(&temporary["profiles"][0]["appEngine"]).unwrap()
    );
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

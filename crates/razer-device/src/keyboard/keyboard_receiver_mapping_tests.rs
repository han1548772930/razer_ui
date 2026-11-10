use super::*;

fn guids() -> impl Iterator<Item = String> {
    (0..512).map(|n| format!("00000000-0000-4000-8000-{n:012x}"))
}

fn document(product: u32) -> Value {
    let recipe =
        crate::keyboard_current_profile::local::initial_template(product, 0, "Desktop").unwrap();
    let settings = serde_json::to_value(recipe.device).unwrap();
    let mut profile = serde_json::to_value(&recipe.profiles[0]).unwrap();
    profile["guid"] = "profile-a".into();
    prepare_document(
        product,
        &[profile],
        "profile-a",
        1,
        None,
        Some(&settings),
        guids(),
    )
    .unwrap()
}

#[test]
fn all_receiver_keyboards_compile_actual_defaults_without_saving_runtime_gaming() {
    for (product, count, dkm_count) in [
        (600, 30, 0),
        (602, 18, 4),
        (658, 15, 4),
        (664, 18, 4),
        (697, 31, 0),
        (716, 26, 11),
        (717, 22, 2),
        (724, 32, 10),
        (727, 21, 5),
    ] {
        let rows = defaults::mappings(product).unwrap();
        assert_eq!(rows.len(), count, "{product}");
        // Independently computed from each current DEFAULTMAPPINGS owner,
        // including property absence, source array order and dial-zero spread.
        let expected = match product {
            600 => "9e46e97a03fea52233eac9bba624e95d",
            602 => "c78108d0dc409a34f786f49e0d54f797",
            658 => "8cf6a4c41a60628172b8ea13a90d2285",
            664 => "8cca8e62119712474e7dfabd745e766f",
            697 => "ddd10e306b41474f71d478042402dc2a",
            716 => "2197e97bff553ab60bcfe3bb23dc3024",
            717 => "e1dd484437c7386c23f6da51c53fafc6",
            724 => "6a3acc204da86361a4a521b3e1b2e4d5",
            727 => "1067a7a2d64b9a2a9e5416fc8a208f70",
            _ => unreachable!(),
        };
        assert_eq!(
            source_hash(&json!(rows)).unwrap(),
            expected,
            "source defaults {product}"
        );
        assert_eq!(defaults::dkm_keys(product).len(), dkm_count, "{product}");
        let saved = document(product);
        assert_eq!(saved["productId"], product);
        assert_eq!(saved["category"], "KEYBOARD");
        assert!(saved["profiles"][0]["appEngine"].get("gamemode").is_none());
        assert_eq!(
            saved["reportIDs"],
            json!({"4":"razerKeyReportID","5":"hardwareEventReportID"})
        );
        assert_eq!(
            saved["defaultMappings"]["appEngine"]["mappings"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["input"]["key"] == 1 && row["output"]["type"] == "hypershift")
                .count(),
            4
        );
    }
    let mini = defaults::mappings(600).unwrap();
    let latest = defaults::mappings(697).unwrap();
    assert!(mini[1]["multimediaGroup"].get("useTurbo").is_none());
    assert_eq!(latest[1]["multimediaGroup"]["useTurbo"], true);
    assert_eq!(latest[30]["outputType"], "disableGroup");
    let joro = defaults::mappings(717).unwrap();
    assert_eq!(joro[0]["outputType"], "layoutSwitchGroup");
    assert!(joro[12]["keyboardGroup"].get("keyAssignment").is_none());
    assert_eq!(
        joro[15]["win8ShortcutsGroup"]["windowsShortcutAssignment"],
        "LockComputer"
    );
    let dial = defaults::mappings(724).unwrap();
    assert_eq!(dial[0]["inputID"], "ScrollRight");
    assert_eq!(
        dial[0]["backlightGroup"]["backlightAssignment"],
        "BrightnessUp"
    );
    assert_eq!(
        dial[1]["backlightGroup"]["backlightAssignment"],
        "BrightnessDown"
    );
}

#[test]
fn receiver_runtime_keeps_unknown_games_and_serial_profile_selection_out_of_saved_data() {
    let mut saved = document(600);
    saved["profiles"][0]["gamingMode"]["state"] = 2.into();
    saved["profiles"][0]["gamingMode"]["isWindowsKeyDisabled"] = true.into();
    saved["profiles"][0]["snapTap"]["isEnabled"] = true.into();
    let mut second = saved["profiles"][0].clone();
    second["guid"] = "profile-b".into();
    second["name"] = "Second".into();
    second["snapTap"]["isEnabled"] = false.into();
    saved["profiles"].as_array_mut().unwrap().push(second);
    let original = saved.clone();
    for count in [None, Some(0), Some(1)] {
        let projected = prepare_peer_document(158, &saved, "real-serial", count).unwrap();
        // Enabled Gaming moves A before the serial fallback is resolved;
        // Snap Tap moving A later must not change the already chosen GUID.
        assert_eq!(
            projected.selected_profile(),
            Some(if count == Some(1) { "profile-b" } else { "profile-a" })
        );
        assert_eq!(saved, original);
        let profiles = projected.document()["profiles"].as_array().unwrap();
        let a = profiles.iter().find(|p| p["guid"] == "profile-a").unwrap();
        assert_eq!(
            a["appEngine"].get("gamemode"),
            if count == Some(1) {
                Some(&Value::Bool(true))
            } else {
                None
            }
        );
    }
    saved["deviceMetadatas"] = json!({"real-serial":{"activeProfileGuid":"not-present"}});
    assert!(
        prepare_peer_document(158, &saved, "real-serial", None)
            .unwrap()
            .selected_profile()
            .is_none()
    );
    assert!(prepare_peer_document(158, &saved, "", None).is_err());
}

#[test]
fn persisted_peer_runtime_inputs_are_validated_before_any_projection() {
    let original = document(600);
    let mut corruptions = Vec::new();
    let mut invalid = original.clone();
    invalid["profiles"][0]["gamingMode"]["state"] = 3.into();
    corruptions.push(invalid);
    let mut invalid = original.clone();
    invalid["profiles"][0]["gamingMode"]["isAltEnterDisabled"] = "false".into();
    corruptions.push(invalid);
    let mut invalid = original.clone();
    invalid["profiles"][0]["modTap"] = "false".into();
    corruptions.push(invalid);
    let mut invalid = original.clone();
    invalid["gameModeEnabled"] = Value::Null;
    corruptions.push(invalid);
    let mut invalid = original.clone();
    invalid["profiles"][0]["snapTap"]["keyList"] =
        json!([{"id":1,"key1":"KEY_A","key2":"KEY_D"},{"id":1,"key1":"KEY_Q","key2":"KEY_E"}]);
    corruptions.push(invalid);
    for invalid in corruptions {
        let before = invalid.clone();
        assert!(prepare_peer_document(158, &invalid, "real-serial", Some(1)).is_err());
        assert_eq!(invalid, before);
    }
}

#[test]
fn selected_keyboard_serial_updates_only_its_local_profile_reference() {
    let settings = json!({"deviceMetadatas":{"first":{"activeProfileGuid":"a","name":"Retained"},"other":{"activeProfileGuid":"b"}},"extension":41});
    let selected = select_profile_for_serial(&settings, "first", "c").unwrap();
    assert_eq!(
        selected["deviceMetadatas"]["first"],
        json!({"activeProfileGuid":"c","name":"Retained"})
    );
    assert_eq!(
        selected["deviceMetadatas"]["other"],
        settings["deviceMetadatas"]["other"]
    );
    assert_eq!(selected["extension"], 41);
    assert_eq!(
        settings["deviceMetadatas"]["first"]["activeProfileGuid"],
        "a"
    );
    assert!(select_profile_for_serial(&settings, "", "c").is_err());
}

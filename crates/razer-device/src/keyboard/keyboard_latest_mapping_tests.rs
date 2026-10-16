use super::*;
fn copilot(input_type: &str) -> Value {
    json!({"inputID":"KEY_RIGHT_CTRL","inputType":input_type,"isHyperShift":true,"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_RIGHT_CTRL","modifiers":[]}})
}
#[test]
fn copilot_repairs_only_source_candidates_and_round_trips_the_ui_default() {
    let original = copilot("KeyInput");
    let fixed = materialize(&[original.clone()], false).unwrap();
    assert_eq!(
        fixed[0]["keyboardGroup"],
        json!({"key":"KEY_F23","modifiers":["KEY_LEFT_GUI","KEY_LEFT_SHIFT"]})
    );
    let ui = ui_mappings(&fixed);
    assert_eq!(ui[0]["outputType"], "defaultGroup");
    assert!(ui[0].get("keyboardGroup").is_none());
    assert_eq!(materialize(&ui, false).unwrap(), fixed);
    let mut unrelated = original.clone();
    unrelated["isHyperShift"] = json!(false);
    assert_eq!(
        materialize(&[unrelated.clone()], false).unwrap(),
        [unrelated]
    );
    let mut custom = fixed[0].clone();
    custom["keyboardGroup"]["key"] = json!("KEY_A");
    assert_eq!(ui_mappings(&[custom.clone()]), [custom]);
    assert_eq!(original["keyboardGroup"]["key"], "KEY_RIGHT_CTRL");
}
#[test]
fn recovery_fails_closed_on_missing_cache_and_preserves_unrelated_engine_outputs() {
    let row = copilot("KeyInput");
    let mut context = EngineContext::default();
    let mut cached = MappingEngine::new(756)
        .unwrap()
        .generate(&[row.clone()], &mut context)
        .unwrap()
        .app_engine;
    cached["mappings"]
        .as_array_mut()
        .unwrap()
        .push(json!({"input":{"type":"razerKey","key":77},"output":{"type":"disabled"}}));
    cached["retained"] = json!(true);
    let doc = json!({"activeProfile":"profile","version":9,"profiles":[{"guid":"profile","mappings":[row],"appEngine":cached}]});
    let fixed = recover_document(&doc, &mut context).unwrap();
    assert_eq!(fixed["version"], 10);
    assert_eq!(fixed["profiles"][0]["appEngine"]["retained"], true);
    assert_eq!(
        fixed["profiles"][0]["appEngine"]["mappings"]
            .as_array()
            .unwrap()
            .last(),
        doc["profiles"][0]["appEngine"]["mappings"]
            .as_array()
            .unwrap()
            .last()
    );
    assert_ne!(
        fixed["profiles"][0]["appEngine"]["mappings"][0]["output"],
        doc["profiles"][0]["appEngine"]["mappings"][0]["output"]
    );
    let mut missing = doc.clone();
    missing["profiles"][0]["appEngine"] = Value::Null;
    assert!(recover_document(&missing, &mut context).is_err());
}
#[test]
fn firmware_duplicates_incompatible_analog_and_retains_plain_analog_metadata() {
    let rows = json!([{"inputID":"KEY_A","inputType":"AnalogInput","isHyperShift":false,"interrupts":["KEY_ANY"],"mapping":[{"outputType":"controllerGroup","controllerGroup":{"controllerMode":{"isAnalog":true}}},{"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"}}]},
        {"inputID":"KEY_C","inputType":"AnalogInput","mapping":[{"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_D"}},{"outputType":"defaultGroup"}]}]);
    let fixed = firmware_mappings(rows.as_array().unwrap());
    assert_eq!(fixed.len(), 3);
    assert_eq!(fixed[0], rows[0]);
    assert_eq!(fixed[1]["inputType"], "KeyInput");
    assert_eq!(fixed[1]["keyboardGroup"]["key"], "KEY_D");
    assert!(fixed[1].get("mapping").is_none());
    assert_eq!(
        fixed[2]["disableGroup"],
        json!({"disableGroup":{"disableAssignment":"Disable"}})
    );
    assert!(fixed[2].get("interrupts").is_none());
}
#[test]
fn analog_pair_uses_firmware_ids_last_mapping_wins_and_removed_keys_reset_break() {
    let row = json!({"inputID":"KEY_A","inputType":"AnalogInput","isHyperShift":false,"mapping":[{"actuationPoint":{"0":3276,"1":256},"rapidTrigger":{"makeSensitivity":0.2,"breakSensitivity":0.4}},{"outputType":"defaultGroup"}]});
    let mut last = row.clone();
    last["mapping"][0]["actuationPoint"]["0"] = json!(4914);
    let pair = analog_pair(6, "custom", &[row.clone()], &[row.clone(), last]).unwrap();
    let key = source()["firmware_keyboard"]["NormalMode"]["KEY_A"]["fwID"]
        .as_u64()
        .unwrap() as u8;
    assert_eq!(
        pair.actuation
            .iter()
            .filter(|point| point.key_id == key)
            .count(),
        1
    );
    assert_eq!(
        pair.actuation
            .iter()
            .find(|point| point.key_id == key)
            .unwrap()
            .threshold_low,
        4914
    );
    assert_eq!(
        pair.rapid_trigger
            .iter()
            .find(|point| point.key_id == key)
            .unwrap()
            .up_sensitivity,
        655
    );
    let removed = analog_pair(1, "custom", &[row], &[]).unwrap();
    let reset = removed
        .actuation
        .iter()
        .find(|point| point.key_id == key)
        .unwrap();
    assert_eq!((reset.threshold_low, reset.threshold_high), (19656, 256));
    let factory = analog_pair(2, "af06d371-861f-4b98-8d78-bfaef5cfdebf", &[], &[]).unwrap();
    assert!(factory.rapid_trigger.iter().all(|key| key.enabled == 1));
}

#[test]
fn global_device_toggle_keeps_saved_preference_and_only_ui_options_version() {
    let saved =
        json!({"state":1,"enableInGame":false,"isWindowsKeyDisabled":true,"isAltTabDisabled":true});
    let mut document = json!({"version":9,"activeProfile":"p","gameModeEnabled":true,"profiles":[{"guid":"p","gamingMode":saved,"appEngine":{"hash":"old","mappings":[],"other":42}}]});
    let effective = features::set_global_gaming_mode(
        &mut document,
        &json!({"state":0,"isWindowsKeyDisabled":false}),
        false,
    )
    .unwrap();
    assert_eq!(document["gameModeEnabled"], false);
    assert_eq!(document["version"], 9);
    assert_eq!(document["profiles"][0]["gamingMode"], saved);
    assert_eq!(effective["state"], 0);
    assert_eq!(document["profiles"][0]["appEngine"]["other"], 42);
    assert_eq!(
        document["profiles"][0]["appEngine"]["hash"],
        source_hash(&document["profiles"][0]["appEngine"]).unwrap()
    );
    features::set_global_gaming_mode(
        &mut document,
        &json!({"state":2,"gameModeEnabled":true}),
        true,
    )
    .unwrap();
    assert_eq!(document["version"], 9);
    assert_eq!(document["profiles"][0]["gamingMode"]["state"], 2);
    features::set_global_gaming_mode(
        &mut document,
        &json!({"state":2,"isAltTabDisabled":false}),
        true,
    )
    .unwrap();
    assert_eq!(document["version"], 10);
    let before = document.clone();
    features::set_global_gaming_mode(&mut document, &json!({"state":2}), false).unwrap();
    assert_eq!(document, before);
}

#[test]
fn restore_projection_and_led_use_global_and_running_games_independently() {
    let mut profile = default_profile();
    profile["guid"] = json!("p");
    profile["gamingMode"] = json!({"state":2,"enableInGame":true});
    profile["mappings"] = json!([]);
    let document = json!({"activeProfile":"p","gameModeEnabled":false,"profiles":[profile]});
    let projected = features::effective_gaming_mode(&document["profiles"][0], Some(false));
    assert_eq!(projected["state"], 0);
    assert_eq!(document["profiles"][0]["gamingMode"]["state"], 2);
    let enabled = |document: &Value, games| {
        features::actions(document, None, &[], games, false)
            .unwrap()
            .into_iter()
            .find_map(|action| match action {
                features::Action::Write(crate::keyboard_latest_protocol::Setting::LedState {
                    enabled,
                    ..
                }) => Some(enabled),
                _ => None,
            })
            .unwrap()
    };
    assert!(!enabled(&document, Some(1)));
    let mut on = document.clone();
    on["gameModeEnabled"] = json!(true);
    assert!(!enabled(&on, None));
    assert!(enabled(&on, Some(1)));
    on["profiles"][0]["gamingMode"]["state"] = json!(1);
    assert!(enabled(&on, None));
}

#[test]
fn device_settings_reach_compiler_and_global_only_edits_keep_version() {
    let mut profile = default_profile();
    profile["guid"] = json!("p");
    profile["name"] = json!("Local");
    let guids = || (0..512).map(|id| format!("00000000-0000-4000-8000-{id:012}"));
    let old = prepare_document(&[profile.clone()], "p", 0, None, guids()).unwrap();
    let old_profile = old["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|profile| profile["guid"] == "p")
        .unwrap();
    let mut updated = old_profile.clone();
    updated["gamingMode"]["state"] = json!(1);
    let document = prepare_document_with_device_settings(
        &[updated.clone()],
        "p",
        0,
        Some(&old),
        Some(&json!({"gameModeEnabled":true})),
        guids(),
    )
    .unwrap();
    assert_eq!(document["version"], old["version"]);
    assert_eq!(document["gameModeEnabled"], true);
    assert_eq!(
        features::profile(&document).unwrap()["appEngine"]["gamemode"],
        true
    );
    let settings = json!({"gameModeEnabled":true,"dimLighting":{"isEnabled":false,"value":3},"powerSaving":{"isEnabled":true,"value":10}});
    let changed = prepare_document_with_device_settings(
        &[updated],
        "p",
        0,
        Some(&old),
        Some(&settings),
        guids(),
    )
    .unwrap();
    assert_eq!(changed["dimLighting"], settings["dimLighting"]);
    assert_eq!(changed["powerSaving"], settings["powerSaving"]);
    assert_ne!(changed["version"], old["version"]);
    assert!(
        !features::actions(&changed, None, &[], None, false)
            .unwrap()
            .iter()
            .any(|action| matches!(
                action,
                features::Action::Dim { .. } | features::Action::Sleep { .. }
            ))
    );
    let changes = features::actions(&changed, Some(&old), &[], None, false).unwrap();
    assert!(changes.iter().any(
        |action| matches!(action,features::Action::Dim{enabled:false,minutes} if *minutes==3.)
    ));
    assert!(changes.iter().any(
        |action| matches!(action,features::Action::Sleep{enabled:true,minutes} if *minutes==10.)
    ));
}

#[test]
fn global_sync_preserves_mapped_keys_and_only_targets_matching_obm_slots() {
    use crate::{keyboard_latest_protocol::Operation, keyboard_latest_restore::Slot};
    let mappings = json!([
        {"inputID":"KEY_A","inputType":"AnalogInput","isHyperShift":false,"mapping":[{"outputType":"keyboardGroup","actuationPoint":[4914,256],"rapidTrigger":{"makeSensitivity":0.4,"breakSensitivity":0.9,"isEnableUpStroke":false}},{"outputType":"defaultGroup"}]},
        {"inputID":"KEY_B","inputType":"AnalogInput","isHyperShift":false,"mapping":[{"outputType":"controllerGroup","actuationPoint":[9000,256]},{"outputType":"defaultGroup"}]}
    ]);
    let document = json!({"activeProfile":"p","_source_layout_id":0,"profiles":[{"guid":"p","mappings":mappings}]});
    let observed = [
        Slot {
            slot_id: 6,
            guid: "p".into(),
        },
        Slot {
            slot_id: 4,
            guid: "other".into(),
        },
    ];
    let key = |input: &str| {
        source()["firmware_keyboard"]["NormalMode"][input]["fwID"]
            .as_u64()
            .unwrap() as u8
    };
    let act = features::global_sync_operations(
        &document,
        &observed,
        &ObmButtonAssignments::from_source().unwrap(),
        &json!({"type":"ON_SYNC_GLOBAL_ACTUATION","payload":{"value":{"0":32760,"1":256}}}),
        "actual-serial",
    )
    .unwrap();
    let points = act
        .iter()
        .filter_map(|operation| {
            if let Operation::Actuation { slot, points } = operation {
                assert!([1, 6].contains(slot));
                assert!(points.len() <= 15);
                (*slot == 6).then_some(points)
            } else {
                panic!()
            }
        })
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(points[0].key_id, 255);
    assert_eq!(points[0].threshold_low, 32760);
    assert_eq!(
        points
            .iter()
            .find(|point| point.key_id == key("KEY_A"))
            .unwrap()
            .threshold_low,
        4914
    );
    assert_eq!(
        points
            .iter()
            .find(|point| point.key_id == key("KEY_B"))
            .unwrap()
            .threshold_low,
        19656
    );
    let rapid=features::global_sync_operations(&document,&observed,&ObmButtonAssignments::from_source().unwrap(),&json!({"type":"ON_SYNC_GLOBAL_RAPID_TRIGGER","payload":{"value":{"makeSensitivity":0.8,"breakSensitivity":0.9,"isEnableUpStroke":true}}}),"actual-serial").unwrap();
    let rows = rapid
        .iter()
        .filter_map(|operation| {
            if let Operation::RapidTrigger { slot, keys } = operation {
                assert!([1, 6].contains(slot));
                assert!(keys.len() <= 13);
                (*slot == 6).then_some(keys)
            } else {
                panic!()
            }
        })
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(rows[0].key_id, 255);
    let a = rows.iter().find(|row| row.key_id == key("KEY_A")).unwrap();
    assert_eq!(
        (a.enabled, a.down_sensitivity, a.up_sensitivity),
        (1, 655, 256)
    );
    assert_eq!(document["profiles"][0]["mappings"], mappings);
}

#[test]
fn navigation_feedback_uses_input_output_flag_actual_slot_and_slot1_fallback() {
    use crate::keyboard_latest_protocol::{ProfileMetadata, Setting};
    let observed = [ProfileMetadata {
        slot_id: 6,
        guid: "p".into(),
        name: "Onboard".into(),
        owner: String::new(),
        key_mapping_hash: [0; 6],
    }];
    let input = json!(r#"{"scancode":72,"flag":3}"#);
    let key = source()["firmware_keyboard"]["NormalMode"]["KEY_UP_ARROW"]["fwID"]
        .as_u64()
        .unwrap() as u8;
    assert!(
        matches!(features::navigation_feedback("p", &input, &observed).unwrap(),
        Some(Setting::TriggerPresetEffect{slot:6,key:id}) if id == key)
    );
    assert!(
        matches!(features::navigation_feedback("local", &input, &observed).unwrap(),
        Some(Setting::TriggerPresetEffect{slot:1,key:id}) if id == key)
    );
    // The same scan code with an ordinary flag resolves to Num8, not Up.
    let num8 = source()["firmware_keyboard"]["NormalMode"]["KEY_NUMPAD_8"]["fwID"]
        .as_u64()
        .unwrap() as u8;
    assert!(
        matches!(features::navigation_feedback("p", &json!(r#"{"scancode":72,"flag":1}"#), &observed).unwrap(),
        Some(Setting::TriggerPresetEffect{slot:6,key:id}) if id == num8 && id != key)
    );
    assert!(
        features::navigation_feedback("p", &json!(r#"{"scancode":9999,"flag":1}"#), &observed)
            .unwrap()
            .is_none()
    );
}

#[test]
fn navigation_reset_uses_preset_rest_preserves_draft_and_has_separate_indicator_tasks() {
    use crate::keyboard_latest_protocol::Operation;
    let guid = source()["preset_guids"]["ANALOG_WASD"].as_str().unwrap();
    let document = json!({"activeProfile":guid,"profiles":[{"guid":guid,"mappings":[
        {"inputID":"KEY_A","inputType":"AnalogInput","mapping":[{"actuationPoint":{"0":1000,"1":256}}]}
    ]}]});
    let before = document.clone();
    let operations = features::navigation_reset_operations(&document, "actual-serial").unwrap();
    assert!(matches!(&operations[0],Operation::Actuation{slot:1,points}
        if points.len()==1 && points[0].key_id==255));
    let a_key = source()["firmware_keyboard"]["NormalMode"]["KEY_A"]["fwID"]
        .as_u64()
        .unwrap() as u8;
    let a = operations
        .iter()
        .filter_map(|operation| match operation {
            Operation::Actuation { points, .. } => Some(points),
            _ => None,
        })
        .flatten()
        .find(|point| point.key_id == a_key)
        .unwrap();
    assert_eq!((a.threshold_low, a.threshold_high), (19656, 18018));
    let rapid = operations
        .iter()
        .find_map(|operation| match operation {
            Operation::RapidTrigger { slot: 1, keys } => Some(keys),
            _ => None,
        })
        .unwrap();
    assert_eq!(rapid.len(), 1);
    assert_eq!(rapid[0].key_id, 255);
    assert_eq!(document, before);
}

#[test]
fn generic_keyboard_disabled_pass_preserves_navigation_and_constrains_controller_secondary() {
    let rows = json!([
        {"inputID":"KEY_A","inputType":"AnalogInput","isHyperShift":false,"mapping":[{"outputType":"controllerGroup","controllerGroup":{"controllerMode":{"isAnalog":true}},"actuationPoint":[10000,256]},{"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},"actuationPoint":[20000,18000]}]},
        {"inputID":"KEY_C","inputType":"AnalogInput","isHyperShift":false,"mapping":[{"outputType":"profileNavigationGroup","profileNavigationGroup":{"profileNavigationAssignment":"CycleUp"}}]},
        {"inputID":"KEY_D","inputType":"AnalogInput","isHyperShift":false,"mapping":[{"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_E"}}]}
    ]);
    let mut profile =
        json!({"guid":"p","isEnableKeyboardKeys":false,"isGamepadOverride":true,"mappings":rows});
    let disabled = engine_runtime::gamepad_rows(&profile).unwrap().unwrap();
    let controller = disabled
        .iter()
        .find(|row| row["inputID"] == "KEY_A" && row["inputType"] == "AnalogInput")
        .unwrap();
    assert_eq!(controller["mapping"][1]["outputType"], "defaultGroup");
    assert_eq!(
        controller["mapping"][1]["actuationPoint"],
        json!([20000, 18000])
    );
    assert!(disabled.iter().any(|row| row == &rows[1]));
    assert!(disabled.iter().any(|row| row["inputID"] == "KEY_D"
        && row["inputType"] == "KeyInput"
        && row["outputType"] == "disableGroup"));
    profile["isEnableKeyboardKeys"] = json!(true);
    let enabled = engine_runtime::gamepad_rows(&profile).unwrap().unwrap();
    assert!(enabled.iter().any(|row| row["inputID"] == "KEY_A"
        && row["outputType"] == "disableGroup"
        && row["mapping"].is_array()));
    assert!(enabled.iter().any(|row| row["inputID"] == "KEY_D"
        && row["inputType"] == "KeyInput"
        && row["keyboardGroup"]["key"] == "KEY_E"));
    profile
        .as_object_mut()
        .unwrap()
        .remove("isEnableKeyboardKeys");
    assert_eq!(
        engine_runtime::gamepad_rows(&profile).unwrap().unwrap(),
        enabled
    );
    assert_eq!(profile["mappings"], rows);
}
#[test]
fn typed_restore_source_preserves_every_interchange_field_and_numeric_kind() {
    let encoded = serde_json::to_vec(source()).unwrap();
    assert_eq!(
        format!("{:x}", md5::compute(encoded)),
        "4227ad82d0cd6ef9976e62a92ae0fd33"
    );
}

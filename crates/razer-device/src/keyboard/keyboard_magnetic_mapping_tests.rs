use super::*;

fn row(input: &str, hyper: bool, low: u16, output: &str) -> Value {
    json!({"inputID":input,"inputType":"AnalogInput","isHyperShift":hyper,
        "mapping":[{"outputType":output,output:{},"actuationPoint":{"0":low,"1":256}},
            {"outputType":"defaultGroup","actuationPoint":{"0":low,"1":256}}]})
}

#[test]
fn magnetic_global_actuation_preserves_indicator_last_hypershift_and_blocked_preset() {
    let profile = json!({"guid":source::FPS,"mappings":[
        row("KEY_A",false,3276,"keyboardGroup"),
        row("KEY_A",true,6552,"keyboardGroup"),
        row("KEY_B",false,3276,"disableGroup")
    ]});
    let points = global_actuation(740, 1, &profile, &json!({"0":1000,"1":999})).unwrap();
    assert_eq!(
        (
            points[0].key_id,
            points[0].threshold_low,
            points[0].threshold_high
        ),
        (255, 1000, 256)
    );
    let point = |input| {
        points
            .iter()
            .find(|point| point.key_id == source::firmware_key(740, input).unwrap())
            .unwrap()
    };
    assert_eq!(
        (point("KEY_A").threshold_low, point("KEY_A").threshold_high),
        (6552, 4914)
    );
    assert_eq!(
        (point("KEY_B").threshold_low, point("KEY_B").threshold_high),
        (19656, 18018)
    );
}

#[test]
fn magnetic_global_rapid_uses_common_preset_and_independent_last_key_override() {
    let mut normal = row("KEY_A", false, 3276, "keyboardGroup");
    normal["mapping"][0]["rapidTrigger"] =
        json!({"makeSensitivity":2,"breakSensitivity":3,"isEnableUpStroke":true});
    let mut hyper = normal.clone();
    hyper["isHyperShift"] = json!(true);
    hyper["mapping"][0]["rapidTrigger"] =
        json!({"makeSensitivity":11,"breakSensitivity":1,"isEnableUpStroke":false});
    let keys = global_rapid_trigger(
        746,
        1,
        &json!({"guid":source::FPS,"mappings":[normal,hyper]}),
        &json!({"makeSensitivity":0,"breakSensitivity":5,"isEnableUpStroke":false}),
    )
    .unwrap();
    assert_eq!(
        (
            keys[0].key_id,
            keys[0].enabled,
            keys[0].down_sensitivity,
            keys[0].up_sensitivity
        ),
        (255, 1, 1638, 1638)
    );
    let key = |input| {
        keys.iter()
            .find(|key| key.key_id == source::firmware_key(746, input).unwrap())
            .unwrap()
    };
    assert_eq!(
        (
            key("KEY_A").enabled,
            key("KEY_A").down_sensitivity,
            key("KEY_A").up_sensitivity
        ),
        (1, 16380, 14742)
    );
    assert_eq!(
        (
            key("KEY_W").enabled,
            key("KEY_W").down_sensitivity,
            key("KEY_W").up_sensitivity
        ),
        (0, 4914, 4914)
    );
}

#[test]
fn magnetic_mapping_change_retains_old_default_then_new_threshold_for_the_same_key() {
    let old = json!({"guid":"custom","mappings":[row("KEY_A",false,3276,"keyboardGroup")]});
    let current = json!({"guid":"custom","mappings":[row("KEY_A",false,6552,"keyboardGroup")]});
    let (points, keys) = per_key_changes(740, &current, &old).unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(points[0].key_id, points[1].key_id);
    assert_eq!(
        (points[0].threshold_low, points[0].threshold_high),
        (19656, 256)
    );
    assert_eq!(
        (points[1].threshold_low, points[1].threshold_high),
        (6552, 256)
    );
    assert_eq!(keys.len(), 2);
}

#[test]
fn magnetic_normalizer_retains_local_fields_but_engine_owns_interruption_keys() {
    let profile = json!({"guid":"custom","interruptKey":{"isEnabled":true,"interruptionKey":"any","interrupts":["KEY_ANY"]},"mappings":[]});
    let input = json!({"inputID":"KEY_A","inputType":"KeyInput","outputType":"macroGroup","macroGroup":{"guid":"macro","macroPlaybackOption":"Once"},"interrupts":["KEY_B"]});
    let rows = normalize_mappings(740, 1, &profile, &profile, &[input]).unwrap();
    assert_eq!(rows[0]["inputType"], "AnalogInput");
    assert_eq!(
        rows[0]["mapping"][0]["actuationPoint"],
        json!({"0":19656,"1":18018})
    );
    assert!(rows[0].get("interrupts").is_none());
    let engine = controls::engine_rows(&profile, &rows, EnginePreparation::Firmware).unwrap();
    assert_eq!(engine[0]["interrupts"], json!(["KEY_ANY"]));
    assert!(rows[0].get("interrupts").is_none());
}

#[test]
fn magnetic_obm_keyboard_remap_restores_default_before_current_nkro_assignment() {
    let old = json!({"guid":"custom","mappings":[{"inputID":"KEY_A","inputType":"KeyInput","outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B","modifiers":[]}}]});
    let current = json!({"guid":"custom","mappings":[{"inputID":"KEY_A","inputType":"KeyInput","outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_LEFT_SHIFT","modifiers":["KEY_LEFT_CTRL"]}}]});
    let rows = prepare_assignments(740, 1, &current, &old).unwrap();
    assert_eq!(rows.len(), 2);
    let PreparedAssignment::Button(first) = rows[0] else {
        panic!("default button expected")
    };
    let PreparedAssignment::Button(last) = rows[1] else {
        panic!("NKRO button expected")
    };
    assert_eq!(
        (first.function_id, first.data_size, first.data),
        (2, 2, [0, 4, 0, 0, 0])
    );
    assert_eq!(
        (last.function_id, last.data_size, last.data),
        (2, 2, [2, 0, 0, 0, 0])
    );
}

#[test]
fn magnetic_engine_compiles_both_products_with_retained_turbo_identifiers() {
    for product in [740, 746] {
        let mut profile = default_profile(product).unwrap();
        profile["guid"] = json!("11223344-5566-7788-99aa-bbccddeeff12");
        profile["name"] = json!("Local");
        let profiles = vec![profile];
        let settings = serde_json::to_value(
            crate::keyboard_current_profile::high_speed::initial_device_settings(),
        )
        .unwrap();
        let ids = || (0..256).map(|index| format!("11223344-5566-4788-99aa-{index:012x}"));
        let initial = prepare_document_with_device_settings(
            product,
            &profiles,
            "11223344-5566-7788-99aa-bbccddeeff12",
            1,
            None,
            Some(&settings),
            ids(),
        )
        .unwrap();
        let compiled = compile(&initial, Some(0), std::iter::empty::<String>()).unwrap();
        assert_eq!(compiled["productId"], product);
        assert_eq!(compiled["defaultTurbos"], initial["defaultTurbos"]);
        assert!(compiled["profiles"][0]["appEngine"]["mappings"].is_array());
        assert_eq!(
            compiled["defaultMappings"]["appEngine"]["hash"],
            source_hash(&compiled["defaultMappings"]["appEngine"]).unwrap()
        );
    }
}

#[test]
fn magnetic_engine_preparation_retains_source_any_interrupt_difference_and_rejects_unknown_mode() {
    let row = json!({"inputID":"KEY_A","inputType":"AnalogInput","mapping":[{"outputType":"macroGroup","macroGroup":{"guid":"macro","macroPlaybackOption":"Once"},"actuationPoint":{"0":3276,"1":256}}]});
    let profile =
        json!({"interruptKey":{"isEnabled":true,"interruptionKey":"any","interrupts":["KEY_C"]}});
    let firmware =
        controls::engine_rows(&profile, &[row.clone()], EnginePreparation::Firmware).unwrap();
    let gamepad =
        controls::engine_rows(&profile, &[row.clone()], EnginePreparation::Gamepad).unwrap();
    assert_eq!(firmware[0]["interrupts"], json!(["KEY_ANY"]));
    assert_eq!(gamepad[0]["interrupts"], json!([]));
    assert!(row.get("interrupts").is_none());
    assert_eq!(
        controls::preparation(&json!({})).unwrap(),
        EnginePreparation::Gamepad
    );
    assert!(controls::preparation(&json!({"_magneticEnginePreparation":"unknown"})).is_err());
    assert!(controls::preparation(&json!({"_magneticEnginePreparation":null})).is_err());
    let disabled = json!({"interruptKey":{"isEnabled":false,"interrupts":["KEY_ANY"]}});
    assert!(
        controls::engine_rows(&disabled, &firmware, EnginePreparation::Firmware).unwrap()[0]
            .get("interrupts")
            .is_none()
    );
}

#[test]
fn magnetic_gamepad_preparation_preserves_navigation_and_controller_secondary_without_editing_draft()
 {
    let rows = vec![
        json!({"inputID":"KEY_A","inputType":"AnalogInput","isHyperShift":false,"mapping":[{"outputType":"controllerGroup","controllerGroup":{"controllerMode":{"isAnalog":true}},"actuationPoint":{"0":10000,"1":256}},{"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},"actuationPoint":{"0":20000,"1":18000}}]}),
        json!({"inputID":"KEY_C","inputType":"AnalogInput","isHyperShift":false,"mapping":[{"outputType":"profileNavigationGroup","profileNavigationGroup":{"profileNavigationAssignment":"CycleUp"}}]}),
    ];
    let profile =
        json!({"guid":"p","isEnableKeyboardKeys":false,"isGamepadOverride":true,"mappings":rows});
    let disabled = crate::keyboard_firmware_mapping::gamepad_rows(
        &profile,
        &rows,
        &source::full_mapping_list(740).unwrap(),
    )
    .unwrap()
    .unwrap();
    let controller = disabled
        .iter()
        .find(|row| row["inputID"] == "KEY_A" && row["inputType"] == "AnalogInput")
        .unwrap();
    assert_eq!(
        controller["mapping"][1],
        json!({"actuationPoint":{"0":20000,"1":18000},"isDefault":false,"outputType":"defaultGroup","isHyperShift":false})
    );
    assert!(disabled.contains(&rows[1]));
    assert!(disabled.iter().any(|row| row["inputID"] == "KEY_D"
        && row["inputType"] == "KeyInput"
        && row["disableGroup"] == json!({"disableAssignment":"Disable"})));
    assert_eq!(profile["mappings"], json!(rows));
}

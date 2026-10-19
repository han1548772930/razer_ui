use super::*;

#[test]
fn interdevice_otfs_failed_projection_keeps_available_turbo_identities() {
    for product_id in [70, 158] {
        let engine = MappingEngine::new(product_id).unwrap();
        let mut state = context();
        let before = state.clone();
        let defaults = [json!({
            "inputType":"MouseInput", "inputID":"ScrollUp", "outputType":"keyboardGroup",
            "keyboardGroup":{"key":"KEY_A","modifiers":[]}
        })];
        let invalid = [json!({"outputType":"sensitivityGroup"})];
        assert!(
            engine
                .generate_mouse_otfs(&invalid, &defaults, &mut state)
                .is_err()
        );
        assert_eq!(state.fresh_guids, before.fresh_guids);
        assert_eq!(state.default_turbos, before.default_turbos);
        assert_eq!(state.synapse_turbos, before.synapse_turbos);
        assert_eq!(state.dkm_keys, before.dkm_keys);
    }
}

fn context() -> EngineContext {
    EngineContext {
        default_turbos: Some(vec![
            json!({"name":"VolumeUp","guid":"media-volume"}),
            json!({"name":"DoubleClick","guid":"double-click"}),
        ]),
        synapse_turbos: Some(Vec::new()),
        fresh_guids: VecDeque::from(["12345678-1234-4234-8234-123456789abc".into()]),
        ..Default::default()
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceSummary {
    schema_version: u32,
    products: Vec<SourceProduct>,
    tables: Vec<SourceTable>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceProduct {
    product_id: u32,
    table: usize,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceTable {
    key_count: usize,
    keys_without_input_id: usize,
    keys_md5: String,
    key_events_md5: String,
    constants_md5: String,
    default_turbos: Vec<SourceTurbo>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceTurbo {
    id: String,
    event_count: usize,
    events_md5: String,
    source_hash: String,
}

impl SourceSummary {
    fn for_product(&self, product_id: u32) -> &SourceTable {
        let product = self
            .products
            .iter()
            .find(|product| product.product_id == product_id)
            .expect("source summary covers each product");
        &self.tables[product.table]
    }
}

/// Digests are projected from the complete current JavaScript literals by the
/// static acquisition tool, independently of the generated Rust tables.
fn source_summary() -> SourceSummary {
    serde_json::from_str(include_str!(
        "../../testdata/re/mapping-engine-source-summary.json"
    ))
    .unwrap()
}

#[test]
fn typed_tables_preserve_every_audited_key_field_and_used_constant() {
    let source = source_summary();
    assert_eq!(source.schema_version, 1);
    assert_eq!(
        data::PRODUCTS
            .iter()
            .map(|product| product.product_id)
            .collect::<Vec<_>>(),
        source
            .products
            .iter()
            .map(|product| product.product_id)
            .collect::<Vec<_>>()
    );
    for product in data::PRODUCTS {
        let original = source.for_product(product.product_id);
        let table = product.data;
        assert_eq!(table.keys.len(), original.key_count);
        assert_eq!(
            source_hash(&serde_json::to_value(table.keys).unwrap()).unwrap(),
            original.keys_md5
        );
        assert_eq!(
            source_hash(&json!({
                "SINGLE_STATE_INPUT_ID": table.single_state_input_ids,
                "SCROLL_INPUT_ID": table.scroll_input_ids,
                "SCROLL_TURBO_DELAY_MS": table.scroll_turbo_delay_ms,
            }))
            .unwrap(),
            original.constants_md5
        );
        // Symbols without a physical input identifier must remain absent.
        assert_eq!(
            table
                .keys
                .iter()
                .filter(|key| key.input_id.is_none())
                .count(),
            original.keys_without_input_id
        );
        assert_eq!(original.keys_without_input_id, 19);
        let a = table
            .keys
            .iter()
            .find(|key| key.input_id == Some("KEY_A"))
            .unwrap();
        let serialized = serde_json::to_value(a).unwrap();
        assert_eq!(serialized["virtualKey"], "65");
        assert_eq!(serialized["keyCode"], "65");
        assert_eq!(serialized["HID"], "4");
        assert!(serialized.get("outputFlag").is_none());
        assert!(serialized.get("ExtendedKey").is_none());
    }
}

#[test]
fn default_turbos_preserve_source_uuid_order_events_and_hashes() {
    let source = source_summary();
    for product in data::PRODUCTS {
        let original = source.for_product(product.product_id);
        let expected = &original.default_turbos;
        assert_eq!(expected.len(), product.data.default_turbos.len());
        let mut c = context();
        let guids = (0..expected.len())
            .map(|index| format!("12345678-1234-4234-8234-{index:012x}"))
            .collect::<Vec<_>>();
        c.fresh_guids = guids.clone().into();
        let generated = MappingEngine::new(product.product_id)
            .unwrap()
            .generate_default_turbos(&mut c)
            .unwrap();
        let double_click = product
            .data
            .default_turbos
            .iter()
            .find(|turbo| turbo.id == "DoubleClick")
            .unwrap();
        assert_eq!(
            serde_json::to_value(double_click.events).unwrap(),
            json!([
                {"type":"mouse","id":1,"data":0},
                {"type":"mouse","id":2,"data":0},
                {"type":"delay","ms":10},
                {"type":"mouse","id":1,"data":0},
                {"type":"mouse","id":2,"data":0},
            ])
        );
        for (index, expected) in expected.iter().enumerate() {
            let name = expected.id.as_str();
            let turbo = product.data.default_turbos[index];
            assert_eq!(turbo.id, name);
            assert_eq!(turbo.events.len(), expected.event_count);
            assert_eq!(
                source_hash(&serde_json::to_value(turbo.events).unwrap()).unwrap(),
                expected.events_md5
            );
            assert_eq!(generated[index]["name"], name);
            assert_eq!(generated[index]["id"], name);
            assert_eq!(generated[index]["guid"], guids[index]);
            assert_eq!(
                generated[index]["appEngine"]["events"],
                serde_json::to_value(turbo.events).unwrap()
            );
            assert_eq!(generated[index]["appEngine"]["hash"], expected.source_hash);
        }
        assert_eq!(c.default_turbos.as_ref().unwrap(), &generated);
        assert!(c.fresh_guids.is_empty());
    }
}

#[test]
fn every_known_key_keeps_audited_make_and_release_flags() {
    let source = source_summary();
    for product in data::PRODUCTS {
        let engine = MappingEngine::new(product.product_id).unwrap();
        let mut actual = Vec::new();
        for key in product.data.keys {
            let Some(id) = key.input_id else {
                continue;
            };
            let mut events = Vec::new();
            for release in [false, true] {
                let event = engine.key_event(&json!(id), release).unwrap();
                assert_eq!(
                    engine.key_event(&json!({"inputID":id}), release).unwrap(),
                    event
                );
                events.push(event);
            }
            actual.push(json!({"inputID":id,"events":events}));
        }
        assert_eq!(
            source_hash(&json!(actual)).unwrap(),
            source.for_product(product.product_id).key_events_md5
        );
        for (id, scancode, flag) in [("KEY_A", 30, 0), ("KEY_RIGHT_CTRL", 29, 2)] {
            for release in [false, true] {
                assert_eq!(
                    engine.key_event(&json!(id), release).unwrap(),
                    json!({"type":"keyboard","scancode":scancode,"flag":flag+u32::from(release)})
                );
            }
        }
        for unknown in [
            Value::Null,
            json!({"inputID":null}),
            json!("UNKNOWN"),
            json!(""),
        ] {
            assert!(engine.key(&unknown).is_err());
        }
    }
}

#[test]
fn all_products_use_independently_decoded_current_keys_and_events() {
    for pid in [70, 190, 515, 678, 679, 688] {
        let engine = MappingEngine::new(pid).unwrap();
        let r = json!({"inputType":"KeyInput","inputID":"KEY_RIGHT_CTRL","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[]}});
        let entries = engine.record(&r, &mut context()).unwrap();
        assert_eq!(
            entries[0],
            json!({"input":{"type":"keyboard","scancode":29,"hypershift":false,"flag":2},"output":{"type":"keyboard","scancode":30,"flag":0}})
        );
        assert_eq!(entries[1]["input"]["flag"], 3);
        assert_eq!(
            engine
                .data()
                .default_turbos
                .iter()
                .find(|turbo| turbo.id == "DoubleClick")
                .unwrap()
                .events
                .len(),
            5
        );
    }
    assert!(MappingEngine::new(99999).is_err());
}

#[test]
fn modifiers_keep_source_release_order_and_bitmask() {
    let engine = MappingEngine::new(679).unwrap();
    let r = json!({"inputType":"KeyInput","inputID":"KEY_A","inputModifiers":["CTRL","KEY_RIGHT_SHIFT"],"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B","modifiers":["KEY_LEFT_CTRL","KEY_RIGHT_SHIFT"]}});
    let entries = engine.record(&r, &mut context()).unwrap();
    assert_eq!(entries[0]["input"]["modifiers"], 66);
    assert_eq!(
        entries[0]["output"]["outputs"],
        json!([{"type":"keyboard","scancode":29,"flag":0},{"type":"keyboard","scancode":54,"flag":0},{"type":"keyboard","scancode":48,"flag":0}])
    );
    assert_eq!(entries[1]["output"]["outputs"][0]["scancode"], 48);
    assert_eq!(entries[1]["output"]["outputs"][1]["scancode"], 29);
}

#[test]
fn scroll_input_expands_ten_distinct_wheel_deltas_and_has_no_release() {
    let engine = MappingEngine::new(190).unwrap();
    let r = json!({"inputType":"MouseInput","inputID":"ScrollDown","outputType":"sensitivityGroup","sensitivityGroup":{"sensitivityAssignment":"OTFS_Scroll"}});
    let entries = engine.record(&r, &mut context()).unwrap();
    assert_eq!(entries.len(), 10);
    assert_eq!(
        entries[0]["input"],
        json!({"type":"mouse","id":1024,"data":65416,"otfs":true})
    );
    assert_eq!(entries[9]["input"]["data"], 64336);
    assert_eq!(
        entries[9]["output"],
        json!({"type":"sensitivity","sensitivityGroup":{"sensitivityAssignment":"DPI_Value_Down"},"flag":0})
    );
}

#[test]
fn single_scroll_keyboard_creates_real_event_turbo_and_hash() {
    let engine = MappingEngine::new(190).unwrap();
    let mut c = context();
    let r = json!({"inputType":"MouseInput","inputID":"ScrollUp","outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[]}});
    let result = engine.generate(&[r], &mut c).unwrap();
    assert_eq!(result.app_engine["mappings"].as_array().unwrap().len(), 10);
    assert_eq!(result.app_engine["mappings"][0]["output"]["repeat"], 1);
    assert_eq!(
        result.synapse_turbos[0]["appEngine"]["events"],
        json!([{"type":"keyboard","scancode":30,"flag":0},{"type":"delay","ms":30},{"type":"keyboard","scancode":30,"flag":1}])
    );
    let app = &result.synapse_turbos[0]["appEngine"];
    assert_eq!(app["hash"], source_hash(app).unwrap());
    assert!(c.fresh_guids.is_empty());
}

#[test]
fn generation_failure_does_not_consume_guids_or_change_turbo_state() {
    let engine = MappingEngine::new(679).unwrap();
    let mut c = context();
    let turbo = json!({"inputType":"KeyInput","inputID":"KEY_A","outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B","modifiers":[],"turboMode":{"keysPerSecond":10}}});
    let gap =
        json!({"inputType":"KeyInput","inputID":"KEY_C","outputType":"dynamicKeyStrokeGroup"});
    let before = c.clone();
    assert!(engine.generate(&[turbo, gap], &mut c).is_err());
    assert_eq!(c.fresh_guids, before.fresh_guids);
    assert_eq!(c.synapse_turbos, before.synapse_turbos);
}

#[test]
fn default_turbos_are_complete_source_events_not_placeholder_success() {
    let engine = MappingEngine::new(688).unwrap();
    let mut c = context();
    let count = engine.data().default_turbos.len();
    c.fresh_guids = (0..count)
        .map(|i| format!("12345678-1234-4234-8234-{i:012x}"))
        .collect();
    let turbos = engine.generate_default_turbos(&mut c).unwrap();
    assert_eq!(turbos.len(), 38);
    assert!(turbos.iter().all(|t| {
        t["appEngine"]["events"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
    }));
    assert_eq!(
        turbos
            .iter()
            .find(|t| t["name"] == "RepeatScrollDown")
            .unwrap()["appEngine"]["events"],
        json!([{"type":"mouse","id":1024,"data":65416}])
    );
    assert!(c.fresh_guids.is_empty());
}

#[test]
fn dkm_double_click_release_is_disabled_and_multi_filters_only_down_disabled() {
    let engine = MappingEngine::new(190).unwrap();
    let mut c = context();
    c.dkm_keys.insert("DKM_KBMK_01".into(), json!({"key":2}));
    let r = json!({"inputType":"DKMInput","inputID":"DKM_KBMK_01","outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"DoubleClick"}});
    let entries = engine.record(&r, &mut c).unwrap();
    assert_eq!(entries[1]["output"], disabled());
    let result = engine.generate_multi(&[r], &mut c).unwrap();
    assert_eq!(
        result.app_engine["mappings"][1]["output"]["outputs"],
        json!([{"type":"disabled"}])
    );
}

#[test]
fn macro_options_profile_release_and_source_disable_spelling_are_retained() {
    let engine = MappingEngine::new(679).unwrap();
    let mut c = context();
    let mut r = json!({"inputType":"KeyInput","inputID":"KEY_A","outputType":"macroGroup","macroGroup":{"guid":"macro-guid","macroPlaybackOption":"PhasedMacro"}});
    let entries = engine.record(&r, &mut c).unwrap();
    assert_eq!(
        entries[0]["output"],
        json!({"type":"macro","guid":"macro-guid","flag":5,"toggle":5})
    );
    r["outputType"] = json!("profileNavigationGroup");
    r["profileNavigationGroup"] =
        json!({"profileNavigationAssignment":"Specific","guid":"profile","lightpacId":"light"});
    let entries = engine.record(&r, &mut c).unwrap();
    assert_eq!(entries[0]["output"], json!({"type":"disable"}));
    assert_eq!(
        entries[1]["output"],
        json!({"type":"switchProfile","guid":"profile","lightpacId":"light"})
    );
}

#[test]
fn media_use_turbo_false_preserves_wheel_pulse_sequence() {
    let engine = MappingEngine::new(190).unwrap();
    let mut c = context();
    let r = json!({"inputType":"MouseInput","inputID":"ScrollUp","outputType":"multimediaGroup","multimediaGroup":{"multimediaAssignment":"VolumeUp","useTurbo":false}});
    let entries = engine.record(&r, &mut c).unwrap();
    assert_eq!(
        entries[0]["output"],
        json!({"type":"multi","outputs":[{"type":"keyboard","scancode":48,"flag":2},{"type":"delay","ms":30},{"type":"keyboard","scancode":48,"flag":3}]})
    );
}

#[test]
fn windows_shortcuts_use_the_observed_default_turbo_without_allocating_a_new_one() {
    let engine = MappingEngine::new(679).unwrap();
    let mut c = context();
    c.default_turbos
        .as_mut()
        .unwrap()
        .push(json!({"name":"XboxGameBar","guid":"observed-game-bar"}));
    let before = c.fresh_guids.clone();
    let record = json!({"inputType":"KeyInput","inputID":"KEY_A","outputType":"win8ShortcutsGroup",
        "win8ShortcutsGroup":{"windowsShortcutAssignment":"XboxGameBar"}});
    let entries = engine.record(&record, &mut c).unwrap();
    assert_eq!(
        entries[0]["output"],
        json!({"type":"turbo","guid":"observed-game-bar","flag":0,"repeat":1})
    );
    assert_eq!(entries[1]["output"], disabled());
    assert_eq!(c.fresh_guids, before);
    assert!(c.synapse_turbos.unwrap().is_empty());
}

#[test]
fn switch_apps_uses_timed_alt_for_wheel_and_pod_but_held_alt_for_keyboard() {
    let engine = MappingEngine::new(679).unwrap();
    let mut c = context();
    for (kind, input) in [
        ("MouseInput", "ScrollUp"),
        ("ControlPodInput", "ControlPodClick"),
        ("CoolingPadInput", "CycleChromaLighting"),
    ] {
        let record = json!({"inputType":kind,"inputID":input,"outputType":"win8ShortcutsGroup",
            "win8ShortcutsGroup":{"windowsShortcutAssignment":"SwitchApps"}});
        let (down, up) = engine.windows_shortcut_outputs(&record, &c).unwrap();
        assert_eq!(
            down,
            json!({"type":"multi","outputs":[
            {"type":"keyboard","scancode":56,"flag":0,"releaseAfterMs":1000},
            {"type":"keyboard","scancode":15,"flag":0},{"type":"keyboard","scancode":15,"flag":1}]})
        );
        assert_eq!(
            up,
            json!({"type":"multi","outputs":[
            {"type":"keyboard","scancode":15,"flag":1},{"type":"delay","ms":10},
            {"type":"keyboard","scancode":56,"flag":1}]})
        );
    }
    let record = json!({"inputType":"KeyInput","inputID":"KEY_A","outputType":"win8ShortcutsGroup",
        "win8ShortcutsGroup":{"windowsShortcutAssignment":"SwitchApps"}});
    let entries = engine.record(&record, &mut c).unwrap();
    assert_eq!(
        entries[0]["output"],
        json!({"type":"multi","outputs":[
        {"type":"keyboard","scancode":56,"flag":0},{"type":"delay","ms":10},
        {"type":"keyboard","scancode":15,"flag":0}]})
    );
}

#[test]
fn lock_computer_preserves_mouse_release_delay_and_global_turbo_precedence() {
    let engine = MappingEngine::new(190).unwrap();
    let mut c = context();
    let mut record = json!({"inputType":"DKMInput","inputID":"DKM_M_01","outputType":"win8ShortcutsGroup",
        "win8ShortcutsGroup":{"windowsShortcutAssignment":"LockComputer"}});
    let (down, up) = engine.windows_shortcut_outputs(&record, &c).unwrap();
    assert_eq!(down, disabled());
    assert_eq!(
        up,
        json!({"type":"multi","outputs":[
        {"type":"keyboard","scancode":91,"flag":2},{"type":"keyboard","scancode":38,"flag":0},
        {"type":"delay","ms":50},{"type":"keyboard","scancode":38,"flag":1},
        {"type":"keyboard","scancode":91,"flag":3}]})
    );
    record["inputType"] = json!("KeyInput");
    record["inputID"] = json!("KEY_A");
    let (down, up) = engine.windows_shortcut_outputs(&record, &c).unwrap();
    assert_eq!(down["outputs"][2]["ms"], 10);
    assert_eq!(up, disabled());
    c.is_global_shortcuts = true;
    c.default_turbos
        .as_mut()
        .unwrap()
        .push(json!({"name":"LockComputer","guid":"global-lock"}));
    assert_eq!(
        engine.windows_shortcut_outputs(&record, &c).unwrap().0,
        json!({"type":"turbo","guid":"global-lock","flag":0,"repeat":1})
    );
}

#[test]
fn windows_shortcuts_keep_brightness_pulses_and_local_launch_visibility() {
    let engine = MappingEngine::new(679).unwrap();
    let c = context();
    let mut record = json!({"inputType":"MouseInput","inputID":"ScrollDown","outputType":"win8ShortcutsGroup",
        "win8ShortcutsGroup":{"windowsShortcutAssignment":"DisplayBrightnessDown"}});
    assert_eq!(
        engine.windows_shortcut_outputs(&record, &c).unwrap(),
        (
            json!({"type":"multi","outputs":[{"type":"display","id":"driverBrightnessDown"},
            {"type":"delay","ms":10},{"type":"display","id":"driverBrightnessStop"}]}),
            json!({"type":"display","id":"driverBrightnessStop"})
        )
    );
    for (assignment, path, hidden) in [
        ("Calculator", "calc", false),
        ("LaunchTaskManager", "cmd /c start taskmgr", true),
        (
            "User_Directory",
            "cmd /c start explorer %USERPROFILE%",
            true,
        ),
    ] {
        record["win8ShortcutsGroup"]["windowsShortcutAssignment"] = json!(assignment);
        let (down, up) = engine.windows_shortcut_outputs(&record, &c).unwrap();
        assert_eq!(down["type"], "launch");
        assert_eq!(down["path"], path);
        assert_eq!(
            down.get("startHidden"),
            hidden.then_some(&Value::Bool(true))
        );
        assert_eq!(up, disabled());
    }
    record["win8ShortcutsGroup"]["windowsShortcutAssignment"] = json!("MSCopilot");
    assert!(engine.windows_shortcut_outputs(&record, &c).is_err());
}

#[test]
fn control_pod_and_cooling_pad_inputs_match_current_source_shapes() {
    let engine = MappingEngine::new(190).unwrap();
    let mut c = context();
    let pod = json!({"inputType":"ControlPodInput","inputID":"ControlPodClockwise",
        "outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[]}});
    let entries = engine.record(&pod, &mut c).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0]["input"],
        json!({"type":"controlPod","id":"dial","rotation":"clockwise","repeat":true})
    );
    assert_eq!(entries[0]["output"]["type"], "multi");
    assert_eq!(entries[0]["output"]["outputs"][0]["type"], "keyboard");
    assert_eq!(entries[0]["output"]["outputs"][1]["type"], "keyboard");

    let cooling = json!({"inputType":"CoolingPadInput","inputID":"PowerOnFan",
        "outputType":"disableGroup"});
    let entries = engine.record(&cooling, &mut c).unwrap();
    assert_eq!(
        entries[0]["input"],
        json!({"type":"coolingPad","id":9,"byte1":3,"byte2":8})
    );
    assert_eq!(entries[0]["output"], disabled());
}

#[test]
fn scrolling_output_keeps_group_and_make_release_flags() {
    let engine = MappingEngine::new(190).unwrap();
    let mut c = context();
    let record = json!({"inputType":"KeyInput","inputID":"KEY_A",
        "outputType":"scrollingGroup","scrollingGroup":{"direction":"up","amount":2}});
    let entries = engine.record(&record, &mut c).unwrap();
    assert_eq!(
        entries[0]["output"],
        json!({"type":"scrolling",
        "scrollingGroup":{"direction":"up","amount":2},"flag":0})
    );
    assert_eq!(entries[1]["output"]["flag"], 1);
}

#[test]
fn analog_actuation_only_and_rt_follow_generation_and_upstroke() {
    let engine = MappingEngine::new(678).unwrap();
    let mut c = context();
    c.analog_generation = Some("analogV1".into());
    let r = json!({"inputType":"AnalogInput","inputID":"KEY_A","isHyperShift":true,"mapping":[{"outputType":"keyboardGroup","keyboardGroup":{},"actuationPoint":[8,12],"rapidTrigger":{"makeSensitivity":0.2,"breakSensitivity":0.3,"isEnableUpStroke":true}},{"outputType":"defaultGroup","actuationPoint":[9,13]}]});
    let result = engine.generate_analog(&[r.clone()], &mut c).unwrap();
    assert_eq!(result.app_engine["mappings"], json!([]));
    assert_eq!(
        result.app_engine["actuations"],
        json!([{"keyId":31,"make":16.0,"break":16.0,"hypershift":true}])
    );
    assert_eq!(
        result.app_engine["rapidTriggers"]["31"],
        json!({"makeSensitivity":2,"breakSensitivity":4})
    );
    c.is_analog_gen2 = true;
    c.analog_generation = Some("analogV2".into());
    let result = engine.generate_analog(&[r], &mut c).unwrap();
    assert_eq!(result.app_engine["actuations"][0]["make"], 8);
    assert_eq!(
        result.app_engine["rapidTriggers"]["31"],
        json!({"makeSensitivity":326,"breakSensitivity":488})
    );
}

#[test]
fn rapid_trigger_source_rounding_serializes_native_integer_fields() {
    use crate::mapping_native_hid::NativeHidDeviceConfig;

    let engine = MappingEngine::new(679).unwrap();
    for (generation2, make, release, expected_make, expected_release) in [
        (false, 0.125, 0.375, 2, 5),
        (false, -0.125, -0.375, -1, -4),
        (true, 0.2, 0.3, 326, 488),
        (true, 0.4, 0.9, 651, 1465),
    ] {
        let mut c = context();
        c.is_analog_gen2 = generation2;
        let record = json!({"inputID":"KEY_A","mapping":[{"rapidTrigger":{
            "makeSensitivity":make,"breakSensitivity":release,"isEnableUpStroke":true}}]});
        let rapid = engine.rapid_trigger(&record, &c).unwrap();
        assert_eq!(rapid["31"]["makeSensitivity"].as_i64(), Some(expected_make));
        assert_eq!(
            rapid["31"]["breakSensitivity"].as_i64(),
            Some(expected_release)
        );

        let document = json!({"activeProfile":"actual-profile","profiles":[{
            "guid":"actual-profile","appEngine":{"rapidTriggers":rapid}}]});
        let wire = serde_json::to_vec(&document).unwrap();
        let received = serde_json::from_slice(&wire).unwrap();
        let config = NativeHidDeviceConfig::from_document(&received).unwrap();
        assert_eq!(
            config.profile.rapid_triggers[&31].make_sensitivity,
            Some(expected_make as u16)
        );
        assert_eq!(
            config.profile.rapid_triggers[&31].break_sensitivity,
            Some(expected_release as u16)
        );
    }
}

#[test]
fn analog_nonkeyboard_retains_nested_mapping_arrays_and_interrupts() {
    let engine = MappingEngine::new(679).unwrap();
    let mut c = context();
    c.analog_generation = Some("analogV2".into());
    let r = json!({"inputType":"AnalogInput","inputID":"KEY_A","interrupts":["KEY_B","UNKNOWN"],"mapping":[{"outputType":"macroGroup","macroGroup":{"guid":"macro","macroPlaybackOption":"ContinuousHeld"},"actuationPoint":[500,450]},{"outputType":"defaultGroup","actuationPoint":[600,550]}]});
    let result = engine.generate_analog(&[r], &mut c).unwrap();
    assert_eq!(
        result.app_engine["mappings"][0][0]["input"]["interrupts"],
        json!([{"type":"keyboard","scancode":48,"flag":0}])
    );
    assert_eq!(
        result.app_engine["mappings"][0][1]["output"],
        json!({"type":"macro","guid":"macro","flag":1})
    );
    assert_eq!(result.app_engine["actuations"][0]["make"], 500);
}

#[test]
fn double_mapping_keeps_two_thresholded_digital_outputs() {
    let engine = MappingEngine::new(688).unwrap();
    let mut c = context();
    c.analog_generation = Some("analogV2".into());
    let r = json!({"inputType":"AnalogInput","inputID":"KEY_A","mapping":[{"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},"actuationPoint":[500,450]},{"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"LeftClick"},"actuationPoint":[600,550]}]});
    let result = engine.generate_analog(&[r], &mut c).unwrap();
    let mapping = &result.app_engine["mappings"][0][0];
    assert_eq!(mapping["output"]["type"], "triggers");
    assert_eq!(mapping["output"]["outputs"]["1"]["actuation"], 500);
    assert_eq!(mapping["output"]["outputs"]["2"]["actuation"], 600);
    // Current 688/679 kt receives the wrapper, which has no actuationPoint.
    // The two per-stage thresholds belong only to the triggers outputs.
    assert_eq!(result.app_engine["actuations"], json!([]));
}

#[test]
fn current_dual_key_default_keeps_numeric_key_points_and_rapid_trigger() {
    let engine = MappingEngine::new(679).unwrap();
    let mut c = context();
    c.is_analog_gen2 = true;
    let r = json!({"inputType":"AnalogInput","inputID":"KEY_A","mapping":[
        {"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},
         "actuationPoint":{"0":19656,"1":256},"rapidTrigger":{"makeSensitivity":0.2}},
        {"outputType":"defaultGroup","actuationPoint":{"0":65520,"1":256}}]});
    let generated = engine.generate_analog(&[r], &mut c).unwrap().app_engine;
    assert_eq!(
        generated["actuations"],
        json!([{"keyId":31,"make":19656,"break":256}])
    );
    assert_eq!(generated["mappings"][0][0]["input"]["type"], "analogKey");
    assert_eq!(
        generated["mappings"][0][0]["output"],
        json!({"type":"keyboard","scancode":48,"flag":0})
    );
    assert_eq!(generated["mappings"][0][1]["output"]["flag"], 1);
    assert_eq!(
        generated["rapidTriggers"]["31"],
        json!({"makeSensitivity":326,"breakSensitivity":326})
    );
}

#[test]
fn current_dual_key_multimedia_uses_triggers_instead_of_double_mapping() {
    let engine = MappingEngine::new(679).unwrap();
    let r = json!({"inputType":"AnalogInput","inputID":"KEY_A","mapping":[
        {"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},"actuationPoint":{"0":19656,"1":256}},
        {"outputType":"multimediaGroup","multimediaGroup":{"multimediaAssignment":"MuteVolume"},"actuationPoint":{"0":60000,"1":256}}]});
    let generated = engine
        .generate_analog(&[r], &mut context())
        .unwrap()
        .app_engine;
    let output = &generated["mappings"][0][0]["output"];
    assert_eq!(output["type"], "triggers");
    assert_eq!(output["outputs"]["1"]["mapping"]["scancode"], 48);
    assert_eq!(output["outputs"]["2"]["actuation"], 60000);
    assert!(output.get("analog").is_none());
}

#[test]
fn explicit_null_analog_id_does_not_fall_back_to_the_physical_key_table() {
    let engine = MappingEngine::new(678).unwrap();
    let mut record = json!({"inputID":"KEY_A","actuationPoint":[500,450]});
    assert_eq!(engine.actuation(&record).unwrap()["keyId"], 31);
    record["analogKeyID"] = Value::Null;
    assert_eq!(engine.actuation(&record).unwrap()["keyId"], Value::Null);
    let mut rapid = json!({"inputID":"KEY_A","mapping":[{"rapidTrigger":{"makeSensitivity":0.2}}]});
    assert!(
        engine
            .rapid_trigger(&rapid, &context())
            .unwrap()
            .get("31")
            .is_some()
    );
    rapid["analogKeyID"] = Value::Null;
    assert!(engine.rapid_trigger(&rapid, &context()).is_err());
}

#[test]
fn joystick_serialization_preserves_js_whole_numbers_for_native_integer_arrays() {
    let engine = MappingEngine::new(679).unwrap();
    let record = json!({"joystickGroup":{"joystickMode":"axes",
        "joystickAxisAssignment":{"X":"500","Y":-128.0,"Z":null}}});
    for gen2 in [false, true] {
        let c = EngineContext {
            is_analog_gen2: gen2,
            analog_binary_range: Some(65535),
            ..context()
        };
        let (down, up) = engine.joystick_outputs(&record, &c, true).unwrap();
        assert_eq!(down, up);
        let endpoint = if gen2 { "65535" } else { "255" };
        let xyz = down["actuations"][endpoint].as_array().unwrap();
        assert_eq!(xyz, &vec![json!(127), json!(-128), json!(0)]);
        assert!(xyz.iter().all(Value::is_i64));
        assert_eq!(serde_json::to_string(xyz).unwrap(), "[127,-128,0]");
        let serialized = serde_json::to_string(&down).unwrap();
        let native_json: Value = serde_json::from_str(&serialized).unwrap();
        let native =
            crate::mapping_native_analog::NativeAnalogOutput::from_native_json(&native_json)
                .expect(
                    "Current generator output must survive standard JSON into the native parser",
                );
        let mut state = crate::mapping_native_analog::AnalogMappingState::default();
        let endpoint_value = if gen2 { 65535 } else { 255 };
        assert_eq!(
            state.apply("KEY_A", &native, endpoint_value).axes,
            [127, -128, 0]
        );
        assert_eq!(state.apply("KEY_A", &native, 0).axes, [0, 0, 0]);
        let floating = json!({"type":down["type"],"id":"axes",
            "actuations":{"0":[0.0,0.0,0.0],endpoint:[127.0,-128.0,0.0]}});
        assert_eq!(source_hash(&down).unwrap(), source_hash(&floating).unwrap());
    }
    let (down, up) = engine.joystick_outputs(&record, &context(), false).unwrap();
    assert_eq!(down["xyz"], json!([127, -128, 0]));
    assert_eq!(up["xyz"], json!([0, 0, 0]));
    let button = json!({"joystickGroup":{"joystickMode":"button","joystickButtonAssignment":"3"}});
    let (down, _) = engine.joystick_outputs(&button, &context(), false).unwrap();
    assert!(down["button"].is_i64());
    assert_eq!(down["button"], 3);
}

#[test]
fn joystick_fractional_values_remain_explicit_for_native_boundary_rejection() {
    let engine = MappingEngine::new(679).unwrap();
    let record = json!({"joystickGroup":{"joystickMode":"raxes",
        "joystickAxisAssignment":{"X":12.5,"Y":-0.0}}});
    let (down, _) = engine.joystick_outputs(&record, &context(), true).unwrap();
    let xyz = down["actuations"]["255"].as_array().unwrap();
    assert_eq!(xyz[0], 12.5);
    assert!(xyz[0].is_f64());
    assert_eq!(xyz[1], 0);
    assert!(xyz[1].is_i64());
    assert_eq!(serde_json::to_string(xyz).unwrap(), "[12.5,0,0]");
    let native_json: Value = serde_json::from_str(&serde_json::to_string(&down).unwrap()).unwrap();
    assert!(
        crate::mapping_native_analog::NativeAnalogOutput::from_native_json(&native_json).is_err(),
        "A fractional generator value must remain an explicit native compatibility failure"
    );
    let non_finite = json!({"joystickGroup":{"joystickMode":"axes",
        "joystickAxisAssignment":{"X":"NaN"}}});
    assert!(
        engine
            .joystick_outputs(&non_finite, &context(), true)
            .is_err()
    );
}

#[test]
fn current_mouse_local_chroma_and_interdevice_hold_release_outputs() {
    let engine = MappingEngine::new(70).unwrap();
    let mut context = context();
    let lighting =
        json!({"outputType":"lightPacGroup","lightPacGroup":{"ID":"local-chroma","Name":"Local"}});
    let (down, up) = engine.outputs(&lighting, &mut context, 0).unwrap();
    assert_eq!(down, json!({"type":"lightpac","id":"local-chroma"}));
    assert_eq!(up, json!({"type":"disabled"}));
    let target = json!({"outputType":"interDeviceGroup","interDeviceGroup":{"interPID":70,"interDeviceType":"sensitivityGroup","sensitivityGroup":{"sensitivityAssignment":"DPI_Clutch","x":800,"y":900,"independent":true}}});
    let (down, up) = engine.outputs(&target, &mut context, 0).unwrap();
    assert_eq!(down["value"]["flag"], 0);
    assert_eq!(up["value"]["flag"], 1);
    assert_eq!(up["pid"], 70);
    assert_eq!(up["value"]["sensitivityGroup"]["y"], 900);
    let profile = json!({"outputType":"interDeviceGroup","interDeviceGroup":{"interPID":515,"deviceContainerId":"target","interDeviceType":"profileNavigationGroup","profileNavigationGroup":{"profileNavigationAssignment":"Specific","guid":"target-profile","lightpacId":"local-chroma"}}});
    let (down, up) = engine.outputs(&profile, &mut context, 0).unwrap();
    assert_eq!(
        down["value"],
        json!({"type":"switchProfile","deviceContainerId":"target","guid":"target-profile","lightpacId":"local-chroma"})
    );
    assert_eq!(up, json!({"type":"disabled"}));
}

#[test]
fn system_assignments_emit_only_source_release_actions() {
    let engine = MappingEngine::new(515).unwrap();
    for (assignment, expected) in [
        ("Sleep", json!({"type":"system","id":"sleep"})),
        ("Hibernate", json!({"type":"system","id":"hibernate"})),
        ("Unknown", json!({"type":"system"})),
    ] {
        let record = json!({"inputType":"KeyInput","inputID":"KEY_PAUSE",
            "outputType":"systemGroup","systemGroup":{"systemAssignment":assignment}});
        let entries = engine.record(&record, &mut context()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["output"], json!({"type":"disabled"}));
        assert_eq!(entries[1]["output"], expected);
    }
}

#[test]
fn source_scroll_turbo_without_rate_preserves_explicit_json_null() {
    let engine = MappingEngine::new(190).unwrap();
    for assignment in ["ScrollLeft", "ScrollRight", "DoubleClick"] {
        let record = json!({"inputType":"KeyInput","inputID":"KEY_A",
            "outputType":"mouseGroup","mouseGroup":{
                "mouseAssignment":assignment,"turboMode":{"isTurbo":true}}});
        let entries = engine.record(&record, &mut context()).unwrap();
        assert_eq!(entries[0]["output"]["type"], "turbo");
        assert_eq!(entries[0]["output"].get("delay"), Some(&Value::Null));
        assert!(entries[0]["output"].get("repeat").is_none());
        assert_eq!(entries[1]["output"]["flag"], 1);
        let serialized = serde_json::to_string(&entries[0]["output"]).unwrap();
        assert!(serialized.contains("\"delay\":null"));
        let mut without_turbo = record.clone();
        without_turbo["mouseGroup"]
            .as_object_mut()
            .unwrap()
            .remove("turboMode");
        let ordinary = engine.record(&without_turbo, &mut context()).unwrap();
        assert!(ordinary[0]["output"].get("delay").is_none());
    }
}

use super::*;

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

#[test]
fn all_products_use_independently_decoded_current_keys_and_events() {
    for pid in [190, 678, 679, 688] {
        let engine = MappingEngine::new(pid).unwrap();
        let r = json!({"inputType":"KeyInput","inputID":"KEY_RIGHT_CTRL","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[]}});
        let entries = engine.record(&r, &mut context()).unwrap();
        assert_eq!(
            entries[0],
            json!({"input":{"type":"keyboard","scancode":29,"hypershift":false,"flag":2},"output":{"type":"keyboard","scancode":30,"flag":0}})
        );
        assert_eq!(entries[1]["input"]["flag"], 3);
        assert_eq!(
            engine.data()["turbo_events"]["DoubleClick"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
    }
    assert!(MappingEngine::new(70).is_err());
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
    let count = engine.data()["default_turbo_ids"].as_array().unwrap().len();
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
    assert!(
        (result.app_engine["rapidTriggers"]["31"]["makeSensitivity"]
            .as_f64()
            .unwrap()
            - 2.4)
            .abs()
            < 1e-9
    );
    c.is_analog_gen2 = true;
    c.analog_generation = Some("analogV2".into());
    let result = engine.generate_analog(&[r], &mut c).unwrap();
    assert_eq!(result.app_engine["actuations"][0]["make"], 8);
    assert!(
        (result.app_engine["rapidTriggers"]["31"]["breakSensitivity"]
            .as_f64()
            .unwrap()
            - 488.4)
            .abs()
            < 1e-9
    );
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
fn wrappers_and_complex_analog_branches_do_not_silently_flatten() {
    let engine = MappingEngine::new(688).unwrap();
    let mut c = context();
    c.analog_generation = Some("analogV2".into());
    let r = json!({"inputType":"AnalogInput","inputID":"KEY_A","mapping":[{"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"},"actuationPoint":[500,450]},{"outputType":"mouseGroup","mouseGroup":{"mouseAssignment":"LeftClick"},"actuationPoint":[600,550]}]});
    assert!(engine.record(&r, &mut c).is_err());
    assert!(engine.generate_analog(&[r], &mut c).is_err());
}

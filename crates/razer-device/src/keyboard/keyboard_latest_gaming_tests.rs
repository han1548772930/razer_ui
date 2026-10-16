use super::*;

fn mapped(kind: &str, key: u32, output: u32) -> Vec<Value> {
    [0,1].map(|flag|json!({"input":input(kind,key,flag,Some(false),false),"output":keyboard(output,flag)})).to_vec()
}
fn document(settings: Value) -> Value {
    json!({"productId":756,"version":7,"activeProfile":"p","profiles":[
        {"guid":"p","gamingMode":settings,"mappings":[],"appEngine":{"hash":"before","mappings":mapped("analogKey",15,30),"preserve":7}},
        {"guid":"other","appEngine":{"mappings":[]}}
    ],"defaultMappings":{"appEngine":{"hash":"before","mappings":[]}},"retained":42})
}
fn profile(document: &Value) -> &Value {
    document["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|profile| profile["guid"] == "p")
        .unwrap()
}
fn mapping<'a>(profile: &'a Value, kind: &str, key: u32, flag: u32) -> &'a Value {
    profile["appEngine"]["mappings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["input"]["type"] == kind
                && row["input"]["scancode"] == key
                && row["input"]["flag"] == flag
                && row["input"]["hypershift"] != true
        })
        .unwrap()
}

#[test]
fn dual_pass_reuses_consumed_outputs_and_restores_exact_app_engine_rows() {
    let original = document(json!({"state":1,"isAltTabDisabled":true}));
    let mut context = EngineContext::default();
    let active = apply(&original, None, true, &mut context).unwrap();
    let p = profile(&active);
    assert_eq!(p["appEngine"]["preserve"], 7);
    assert_eq!(active["profiles"][1]["guid"], "p");
    for kind in ["analogKey", "keyboard"] {
        let down = mapping(p, kind, 15, 0);
        assert_eq!(down["output"]["cascade"][0]["output"]["type"], "disabled");
        assert_eq!(down["output"]["cascade"][2]["output"], keyboard(30, 0));
    }
    // The second pass reuses the source mapping without consuming it again.
    assert_eq!(
        p["appEngine"][CONSUMED]["mappings"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(p["appEngine"][CONSUMED]["mappings"][0]["position"], 0);
    let mut off = active.clone();
    off["gameModeEnabled"] = json!(false);
    let restored = apply(&off, Some(2), true, &mut context).unwrap();
    assert_eq!(
        profile(&restored)["appEngine"]["mappings"],
        original["profiles"][0]["appEngine"]["mappings"]
    );
    assert!(profile(&restored)["appEngine"].get(CONSUMED).is_none());
    assert_eq!(
        restored["defaultMappings"]["appEngine"]["mappings"],
        json!([])
    );
    assert_eq!(restored["version"], 7);
    assert_eq!(restored["retained"], 42);
    assert!(
        !original
            .as_object()
            .unwrap()
            .contains_key("gameModeEnabled")
    );
}

#[test]
fn restore_prefers_current_raw_mapping_and_never_reuses_removed_raw_outputs() {
    let mut original = document(json!({"state":1,"isAltTabDisabled":true}));
    original["profiles"][0]["appEngine"]["mappings"] = json!(mapped("keyboard", 15, 30));
    original["profiles"][0]["mappings"] = json!([{"inputID":"KEY_TAB","inputType":"KeyInput","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_A","modifiers":[]}}]);
    let mut context = EngineContext::default();
    let mut active = apply(&original, None, true, &mut context).unwrap();
    let p = active["profiles"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["guid"] == "p")
        .unwrap();
    assert_eq!(p["appEngine"][CONSUMED]["mappings"][0]["source"], "raw");
    p["mappings"][0]["keyboardGroup"]["key"] = json!("KEY_C");
    active["gameModeEnabled"] = json!(false);
    let restored = apply(&active, None, true, &mut context).unwrap();
    assert_eq!(
        mapping(profile(&restored), "keyboard", 15, 0)["output"],
        keyboard(46, 0)
    );
    active["profiles"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|p| p["guid"] == "p")
        .unwrap()["mappings"] = json!([]);
    let removed = apply(&active, None, true, &mut context).unwrap();
    assert_eq!(profile(&removed)["appEngine"]["mappings"], json!([]));
}

#[test]
fn game_observation_and_global_override_follow_original_state_precedence() {
    let original = document(json!({"state":2}));
    let mut context = EngineContext::default();
    assert_eq!(
        profile(&apply(&original, None, true, &mut context).unwrap())["appEngine"]["gamemode"],
        false
    );
    assert_eq!(
        profile(&apply(&original, Some(1), true, &mut context).unwrap())["appEngine"]["gamemode"],
        true
    );
    let mut global = original.clone();
    global["gameModeEnabled"] = json!(true);
    global["profiles"][0]["gamingMode"]["state"] = json!(0);
    assert_eq!(
        profile(&apply(&global, None, true, &mut context).unwrap())["appEngine"]["gamemode"],
        true
    );
    global["gameModeEnabled"] = json!(false);
    assert_eq!(
        profile(&apply(&global, Some(1), true, &mut context).unwrap())["appEngine"]["gamemode"],
        false
    );
    let mut malformed = original;
    malformed["profiles"][0]["gamingMode"]["state"] = json!("1");
    assert_eq!(
        profile(&apply(&malformed, None, true, &mut context).unwrap())["appEngine"]["gamemode"],
        false
    );
}

#[test]
fn custom_direct_replaces_fallback_while_alt_guards_remain_protected() {
    let original = document(
        json!({"state":1,"isAltTabDisabled":true,"customMappingSet":2,"customMappingSets":[[],{"mappings":[
            {"inputID":"KEY_TAB","inputType":"KeyInput","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_C","modifiers":[]}}
        ]}]}),
    );
    let active = apply(&original, None, true, &mut EngineContext::default()).unwrap();
    let output = &mapping(profile(&active), "keyboard", 15, 0)["output"]["cascade"];
    assert_eq!(output[0]["input"], keyboard(56, 0));
    assert_eq!(output[0]["output"], disabled());
    assert_eq!(output[1]["output"], disabled());
    assert_eq!(output[2]["output"], keyboard(46, 0));
}

#[test]
fn conditional_custom_mapping_cannot_override_source_disabled_alt_path() {
    let original = document(
        json!({"state":1,"isAltTabDisabled":true,"customMappingSets":[[
            {"inputID":"KEY_TAB","inputType":"KeyInput","isHyperShift":false,"inputModifiers":["ALT"],"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_C","modifiers":[]}}
        ]]}),
    );
    let active = apply(&original, None, true, &mut EngineContext::default()).unwrap();
    let output = &mapping(profile(&active), "keyboard", 15, 0)["output"]["cascade"];
    assert_eq!(output[0]["output"], disabled());
    assert_eq!(output[1]["output"], disabled());
    assert_eq!(output[2]["output"], keyboard(30, 0));
}

#[test]
fn custom_combo_disable_and_partner_guards_have_current_source_precedence() {
    let original = document(json!({"state":1,"customGameMode":{"payload":{"combos":[
        {"enabled":true,"key1":"KEY_A","key2":"KEY_B"},
        {"enabled":true,"key1":"KEY_C","key2":""}
    ]}}}));
    let active = apply(&original, None, true, &mut EngineContext::default()).unwrap();
    let p = profile(&active);
    assert_eq!(mapping(p, "keyboard", 30, 0)["output"], keyboard(30, 0));
    assert_eq!(
        mapping(p, "keyboard", 48, 0)["output"]["cascade"][0],
        entry(Some(keyboard(30, 0)), disabled(), false)
    );
    assert_eq!(mapping(p, "keyboard", 46, 1)["output"], disabled());
    assert!(
        !p["appEngine"]["mappings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["input"]["type"] == "analogKey" && row["input"]["scancode"] == 48)
    );
}

#[test]
fn firmware_defaults_include_both_input_passes_and_partial_outputs_do_not_abort() {
    let mut original =
        document(json!({"state":1,"isWindowsKeyDisabled":true,"isAltTabDisabled":true}));
    original["profiles"][0]["appEngine"]["mappings"]
        .as_array_mut()
        .unwrap()
        .pop();
    let active = apply(&original, None, true, &mut EngineContext::default()).unwrap();
    let defaults = active["defaultMappings"]["appEngine"]["mappings"]
        .as_array()
        .unwrap();
    assert_eq!(defaults.len(), 20); // menu 4x2 + Windows 2x2 + razer 8
    assert_eq!(
        mapping(profile(&active), "analogKey", 15, 1)["output"]["cascade"][2]["output"],
        keyboard(15, 1)
    );
    assert_eq!(
        mapping(profile(&active), "keyboard", 15, 1)["output"]["cascade"][0]["output"],
        keyboard(15, 1)
    );
}

use super::*;
use std::cell::Cell;

fn engine(mappings: Vec<Value>) -> Value {
    json!({"hash":"old","mappings":mappings})
}
fn record(kind: &str, scancode: u16, flag: u8, output: Value) -> Value {
    json!({"input":{"type":kind,"scancode":scancode,"flag":flag},"output":output})
}
fn pair(scancode: u16, target: u16) -> Vec<Value> {
    [0, 1]
        .map(|flag| record("keyboard", scancode, flag, keyboard(target, flag)))
        .into()
}
fn prepare(
    active: &Value,
    defaults: &Value,
    settings: GamingModeSettings,
    context: GamingMappingContext,
) -> Result<KeyboardGamingMappingPlan> {
    KeyboardGamingMappingPlan::prepare(600, active, defaults, settings, context, || Ok(()))
}
fn find<'a>(engine: &'a Value, scancode: u16, flag: u8, hypershift: Option<bool>) -> &'a Value {
    engine["mappings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| {
            m["input"]["scancode"] == scancode
                && m["input"]["flag"] == flag
                && m["input"]["gamemode"] == true
                && m["input"].get("hypershift").and_then(Value::as_bool) == hypershift
        })
        .unwrap()
}

#[test]
fn settings_bit_and_camel_case_match_current_defaults() {
    let parsed: GamingModeSettings = serde_json::from_value(json!({"state":2,"isWindowsKeyDisabled":true,"isAltTabDisabled":true,"isAltF4Disabled":true,"isAltSpaceDisabled":true,"isAltEnterDisabled":true})).unwrap();
    assert_eq!(parsed.game_mode_bit(), 126);
    let defaults: GamingModeSettings = serde_json::from_value(json!({"state":0,"isWindowsKeyDisabled":false,"isAltTabDisabled":false,"isAltF4Disabled":false})).unwrap();
    assert_eq!(defaults, GamingModeSettings::default());
    assert!(
        serde_json::from_value::<GamingModeSettings>(json!({"customMappingSets":[{}]})).is_err()
    );
}

#[test]
fn state_zero_returns_exact_original_even_with_filter_booleans() {
    let active = engine(pair(15, 30));
    let defaults = engine(vec![]);
    let plan = prepare(
        &active,
        &defaults,
        GamingModeSettings {
            is_alt_tab_disabled: true,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.active_engine(), &active);
    assert_eq!(plan.default_engine(), &defaults);
    assert!(!plan.is_temporary());
}

#[test]
fn automatic_game_observation_and_global_override_match_source() {
    let settings = GamingModeSettings {
        state: 2,
        ..Default::default()
    };
    for games in [None, Some(0)] {
        assert!(
            !prepare(
                &engine(vec![]),
                &engine(vec![]),
                settings,
                GamingMappingContext {
                    running_games: games,
                    ..Default::default()
                }
            )
            .unwrap()
            .is_temporary()
        );
    }
    assert!(
        prepare(
            &engine(vec![]),
            &engine(vec![]),
            settings,
            GamingMappingContext {
                running_games: Some(1),
                ..Default::default()
            }
        )
        .unwrap()
        .is_temporary()
    );
    assert!(
        !prepare(
            &engine(vec![]),
            &engine(vec![]),
            GamingModeSettings {
                state: 1,
                ..Default::default()
            },
            GamingMappingContext {
                game_mode_enabled: Some(false),
                ..Default::default()
            }
        )
        .unwrap()
        .is_temporary()
    );
    assert!(
        prepare(
            &engine(vec![]),
            &engine(vec![]),
            GamingModeSettings::default(),
            GamingMappingContext {
                game_mode_enabled: Some(true),
                ..Default::default()
            }
        )
        .unwrap()
        .is_temporary()
    );
}

#[test]
fn indicator_caller_applies_global_override_and_current_factory_preset_exception() {
    let mut document =
        json!({"activeProfile":"profile","profiles":[{"guid":"profile","gamingMode":{"state":2}}]});
    assert!(!indicator_enabled(&document, 679, None).unwrap());
    assert!(!indicator_enabled(&document, 679, Some(0)).unwrap());
    assert!(indicator_enabled(&document, 679, Some(1)).unwrap());
    document["gameModeEnabled"] = json!(false);
    assert!(!indicator_enabled(&document, 679, Some(1)).unwrap());
    document["gameModeEnabled"] = json!(true);
    assert!(indicator_enabled(&document, 679, Some(1)).unwrap());
    document["profiles"][0]["gamingMode"]["state"] = json!(0);
    assert!(indicator_enabled(&document, 679, None).unwrap());
    document["profiles"][0]["guid"] = json!("af06d371-861f-4b98-8d78-bfaef5cfdebf");
    document["activeProfile"] = document["profiles"][0]["guid"].clone();
    assert!(!indicator_enabled(&document, 679, Some(1)).unwrap());
    assert!(indicator_enabled(&document, 515, Some(1)).unwrap());
}

#[test]
fn windows_menu_and_function_disables_are_in_default_engine_only() {
    let active = engine(pair(91, 30));
    let defaults = engine(pair(91, 31));
    let plan = prepare(
        &active,
        &defaults,
        GamingModeSettings {
            state: 1,
            is_windows_key_disabled: true,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.active_engine()["mappings"], active["mappings"]);
    for flag in [2, 3] {
        assert_eq!(
            find(plan.default_engine(), 91, flag, None)["output"],
            disabled()
        );
    }
    for hypershift in [None, Some(true)] {
        for flag in [2, 3] {
            assert_eq!(
                find(plan.default_engine(), 93, flag, hypershift)["output"],
                disabled()
            );
        }
    }
    assert_eq!(
        plan.default_engine()["mappings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["input"]["type"] == "razerKey" && m["input"]["gamemode"] == true)
            .count(),
        8
    );
}

#[test]
fn keyboard_tab_down_blocks_alt_and_up_preserves_saved_output() {
    let plan = prepare(
        &engine(pair(15, 30)),
        &engine(vec![]),
        GamingModeSettings {
            state: 1,
            is_alt_tab_disabled: true,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        find(plan.active_engine(), 15, 0, None)["output"],
        json!({"type":"cascade","cascade":[
            {"input":{"type":"keyboard","scancode":56,"flag":0},"output":{"type":"disabled"}},
            {"input":{"type":"keyboard","scancode":56,"flag":2},"output":{"type":"disabled"}},
            {"track":true,"output":{"type":"keyboard","scancode":30,"flag":0}}
        ]})
    );
    assert_eq!(
        find(plan.active_engine(), 15, 1, None)["output"],
        json!({"type":"cascade","cascade":[{"track":true,"output":{"type":"keyboard","scancode":30,"flag":1}}]})
    );
    for flag in [0, 1, 2, 3] {
        assert_eq!(
            find(plan.active_engine(), 56, flag, None)["output"],
            cascade(vec![entry(None, keyboard(56, flag), true)])
        );
    }
}

#[test]
fn take_latest_mapping_pair_preserves_earlier_records() {
    let mut base = pair(15, 30);
    base.extend(pair(15, 31));
    let plan = prepare(
        &engine(base),
        &engine(vec![]),
        GamingModeSettings {
            state: 1,
            is_alt_tab_disabled: true,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    assert!(
        plan.active_engine()["mappings"]
            .as_array()
            .unwrap()
            .contains(&pair(15, 30)[0])
    );
    assert_eq!(
        find(plan.active_engine(), 15, 0, None)["output"]["cascade"][2]["output"],
        keyboard(31, 0)
    );
}

#[test]
fn analog_tab_up_retains_analog_alt_guards() {
    let defaults = engine(vec![record("analogKey", 4, 0, keyboard(4, 0))]);
    let plan = prepare(
        &engine(vec![]),
        &defaults,
        GamingModeSettings {
            state: 1,
            is_alt_tab_disabled: true,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    let up = find(plan.active_engine(), 15, 1, None);
    assert_eq!(up["input"]["type"], "analogKey");
    assert_eq!(up["output"]["cascade"][0]["input"]["type"], "analogKey");
    assert_eq!(up["output"]["cascade"].as_array().unwrap().len(), 3);
}

#[test]
fn mod_tap_adds_source_extra_guard_and_untracked_keyboard_modifier_records() {
    let plan = prepare(
        &engine(vec![]),
        &engine(vec![]),
        GamingModeSettings {
            state: 1,
            is_alt_space_disabled: true,
            ..Default::default()
        },
        GamingMappingContext {
            mod_tap: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        find(plan.active_engine(), 57, 0, None)["output"]["cascade"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        plan.active_engine()["mappings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["input"]["scancode"] == 56 && m["input"]["flag"] == 2)
            .count(),
        2
    );
}

#[test]
fn source_scancodes_for_space_enter_and_f4_are_distinct() {
    let plan = prepare(
        &engine(vec![]),
        &engine(vec![]),
        GamingModeSettings {
            state: 1,
            is_alt_space_disabled: true,
            is_alt_enter_disabled: true,
            is_alt_f4_disabled: true,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    for scancode in [57, 28, 62] {
        assert_eq!(
            find(plan.active_engine(), scancode, 0, None)["input"]["scancode"],
            scancode
        );
    }
}

#[test]
fn multi_function_key_primary_uses_first_default_f4_pair_and_keeps_active() {
    let mut active = engine(pair(62, 30));
    active["functionKeyPrimary"] = json!("multi");
    let mut defaults = pair(62, 31);
    defaults.extend(pair(62, 32));
    let plan = prepare(
        &active,
        &engine(defaults),
        GamingModeSettings {
            state: 1,
            is_alt_f4_disabled: true,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    assert!(
        plan.active_engine()["mappings"]
            .as_array()
            .unwrap()
            .contains(&pair(62, 30)[0])
    );
    assert_eq!(
        find(plan.active_engine(), 62, 0, None)["output"]["cascade"][2]["output"],
        keyboard(31, 0)
    );
}

#[test]
fn f4_hypershift_and_compose_f4_track_razer_modifier() {
    let plan = prepare(
        &engine(vec![]),
        &engine(vec![]),
        GamingModeSettings {
            state: 1,
            is_alt_f4_disabled: true,
            ..Default::default()
        },
        GamingMappingContext {
            compose_f4: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        find(plan.active_engine(), 62, 0, Some(true))["output"]["cascade"][2],
        entry(Some(razer_modifier()), disabled(), true)
    );
    assert_eq!(
        find(plan.active_engine(), 62, 1, Some(true))["output"],
        cascade(vec![entry(None, keyboard(62, 1), true)])
    );
    assert_eq!(
        find(plan.active_engine(), 5, 0, Some(true))["output"]["cascade"][0]["input"],
        razer_modifier()
    );
    assert_eq!(
        find(plan.active_engine(), 5, 1, Some(true))["output"],
        cascade(vec![entry(None, keyboard(5, 1), true)])
    );
}

#[test]
fn fn_sleep_requires_compose_and_uses_default_or_observed_scancode() {
    let settings = GamingModeSettings {
        state: 1,
        is_fn_sleep_enabled: true,
        ..Default::default()
    };
    let plan = prepare(
        &engine(vec![]),
        &engine(vec![]),
        settings,
        GamingMappingContext {
            compose_f4: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        find(plan.active_engine(), 44, 1, Some(true))["output"],
        cascade(vec![entry(None, keyboard(44, 1), true)])
    );
    let plan = prepare(
        &engine(vec![]),
        &engine(vec![]),
        GamingModeSettings {
            scancode: Some(46),
            ..settings
        },
        GamingMappingContext {
            compose_f4: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        find(plan.active_engine(), 46, 0, Some(true))["input"]["scancode"],
        46
    );
}

#[test]
fn missing_default_or_partial_saved_pair_is_explicit_error() {
    let settings = GamingModeSettings {
        state: 1,
        is_alt_tab_disabled: true,
        ..Default::default()
    };
    assert!(prepare(&engine(vec![]), &json!({}), settings, Default::default()).is_err());
    assert!(
        prepare(
            &engine(vec![record("keyboard", 15, 0, keyboard(30, 0))]),
            &engine(vec![]),
            settings,
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn restore_and_hash_are_separate_from_device_save() {
    let active = engine(pair(15, 30));
    let defaults = engine(vec![]);
    let plan = prepare(
        &active,
        &defaults,
        GamingModeSettings {
            state: 1,
            is_alt_tab_disabled: true,
            ..Default::default()
        },
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.restore_engine(), &active);
    assert_eq!(plan.restore_default_engine(), &defaults);
    assert_eq!(
        plan.active_engine()["hash"],
        source_hash(plan.active_engine()).unwrap()
    );
    assert_eq!(
        plan.default_engine()["hash"],
        source_hash(plan.default_engine()).unwrap()
    );
    assert_eq!(active["hash"], "old");
}

#[test]
fn cancellation_during_preparation_leaves_both_inputs_unchanged() {
    let active = engine(pair(15, 30));
    let defaults = engine(vec![]);
    let count = Cell::new(0);
    let result = KeyboardGamingMappingPlan::prepare(
        600,
        &active,
        &defaults,
        GamingModeSettings {
            state: 1,
            is_alt_tab_disabled: true,
            ..Default::default()
        },
        Default::default(),
        || {
            count.set(count.get() + 1);
            ensure!(count.get() < 3, "cancelled");
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(active, engine(pair(15, 30)));
    assert_eq!(defaults, engine(vec![]));
}

#[test]
fn unsupported_products_firmware_branch_and_preexisting_temporary_records_fail() {
    assert!(supports_product(708));
    assert!(!supports_product(999));
    assert!(
        KeyboardGamingMappingPlan::prepare(
            999,
            &engine(vec![]),
            &engine(vec![]),
            GamingModeSettings {
                state: 1,
                ..Default::default()
            },
            Default::default(),
            || Ok(())
        )
        .is_err()
    );
    assert!(
        prepare(
            &engine(vec![]),
            &engine(vec![]),
            GamingModeSettings {
                state: 1,
                ..Default::default()
            },
            GamingMappingContext {
                firmware_analog_mapping: true,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(prepare(&engine(vec![json!({"input":{"type":"keyboard","scancode":15,"flag":0,"gamemode":true},"output":disabled()})]),&engine(vec![]),GamingModeSettings{state:1,..Default::default()},Default::default()).is_err());
}

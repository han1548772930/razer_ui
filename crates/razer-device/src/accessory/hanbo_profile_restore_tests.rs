use super::*;
use crate::hanbo_cooler::ThermalMode;
use serde_json::json;

fn profile() -> Value {
    hanbo_settings::default_profile_settings()
}

fn normal() -> Vec<CoolerCommand> {
    vec![
        CoolerCommand::PumpParameter { mode: 2, speed: 50 },
        CoolerCommand::FanParameter { mode: 2, speed: 50 },
    ]
}

#[test]
fn absent_profile_is_empty_but_equal_and_root_only_diffs_restore_thermal_ports() {
    assert!(commands(None, None).unwrap().is_empty());
    let previous = profile();
    assert_eq!(
        commands(Some(&previous), Some(&previous)).unwrap(),
        normal()
    );
    let mut current = previous.clone();
    current["brightness"]["value"] = json!(35);
    assert_eq!(commands(Some(&previous), Some(&current)).unwrap(), normal());
    current["name"] = json!("Renamed profile");
    assert_eq!(commands(Some(&previous), Some(&current)).unwrap(), normal());
}

#[test]
fn thermal_field_diff_selects_current_fan_once() {
    let previous = profile();
    let mut current = previous.clone();
    current["ports"][1]["thermal"]["activeThermalIndicator"]["modeSelected"] =
        serde_json::to_value(ThermalMode::Quiet).unwrap();
    assert_eq!(
        commands(Some(&previous), Some(&current)).unwrap(),
        [CoolerCommand::FanParameter { mode: 1, speed: 20 }]
    );
}

#[test]
fn port_lighting_fields_do_not_select_thermal() {
    let previous = profile();
    let mut current = previous.clone();
    current["ports"][0]["brightness"]["value"] = json!(25);
    current["ports"][2]["quickEffects"]["selectedEffectId"] = json!(4);
    assert!(
        commands(Some(&previous), Some(&current))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn source_array_scoring_reassigns_a_removed_thermal_row_before_runtime_selection() {
    let previous = profile();
    let mut current = previous.clone();
    current["ports"][0]
        .as_object_mut()
        .unwrap()
        .remove("thermal");
    // The old Pump scores 90 against the current Pump without thermal, but
    // 113.6 against the current Fan. The Fan's exact match wins that token,
    // leaving '-' and '+' before equal rows. Mn selects runtime Fan index1.
    assert_eq!(
        commands(Some(&previous), Some(&current)).unwrap(),
        [CoolerCommand::FanParameter { mode: 2, speed: 50 }]
    );
    // The reverse direction emits thermal__deleted on the matched Pump row;
    // that suffixed field does not select a thermal write.
    assert!(
        commands(Some(&current), Some(&previous))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn active_transition_uses_truthy_scalar_diff_but_current_inactive_port_is_skipped() {
    let mut previous = profile();
    previous["ports"][0]["isActive"] = json!(false);
    let mut current = previous.clone();
    current["ports"][0]["isActive"] = json!(true);
    assert_eq!(
        commands(Some(&previous), Some(&current)).unwrap(),
        [CoolerCommand::PumpParameter { mode: 2, speed: 50 }]
    );
    assert!(
        commands(Some(&current), Some(&previous))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn array_insert_direction_and_runtime_index_follow_source() {
    let previous = profile();
    let mut current = previous.clone();
    current["ports"].as_array_mut().unwrap().rotate_left(1);
    // diff(current, previous) inserts the previous Pump row at position zero.
    // Mn consumes that index and writes the current Pump value.
    current["ports"][5]["thermal"]["activeThermalIndicator"]["modeSelected"] =
        serde_json::to_value(ThermalMode::Performance).unwrap();
    assert_eq!(
        commands(Some(&previous), Some(&current)).unwrap(),
        [CoolerCommand::PumpParameter { mode: 3, speed: 80 }]
    );
}

#[test]
fn malformed_profile_or_diff_does_not_report_empty_success() {
    let previous = profile();
    for invalid in [
        json!([]),
        json!({"ports":{}}),
        json!({"ports":[{"name":"Pump Cap"}]}),
    ] {
        assert!(commands(Some(&previous), Some(&invalid)).is_err());
    }
    let mut current = previous.clone();
    current["ports"].as_array_mut().unwrap().pop();
    // The source inserts the previous row into diff(current, previous).
    assert!(commands(Some(&previous), Some(&current)).is_ok());
}

#[test]
fn sequence_matcher_keeps_earliest_equal_length_block_and_popular_extension() {
    let left = [json!("b"), json!("a")];
    let right = [json!("a"), json!("b")];
    let left = scalarize(&left, None).unwrap();
    let right = scalarize(&right, None).unwrap();
    assert_eq!(
        matching_blocks(&left.tokens, &right.tokens),
        [(0, 1, 1), (2, 2, 0)]
    );
    let values = vec![json!("same"); 201];
    let tokens = scalarize(&values, None).unwrap();
    assert_eq!(
        matching_blocks(&tokens.tokens, &tokens.tokens),
        [(0, 0, 201), (201, 201, 0)]
    );
}

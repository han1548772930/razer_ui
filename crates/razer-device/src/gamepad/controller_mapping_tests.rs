use super::*;
use crate::controller_profile::{AssignmentRead, ButtonRead};
use serde_json::{Value, json};

fn assignment(button_id: u8, function_id: u8, data: [u8; 5]) -> ButtonAssignment {
    ButtonAssignment {
        profile_slot: 3,
        button_id,
        mode: 0,
        function_id,
        data_size: 5,
        data,
    }
}
fn observed(rows: Vec<ButtonAssignment>) -> MappingObservation {
    MappingObservation {
        profile_slot: 3,
        buttons: rows
            .into_iter()
            .map(|assignment| ButtonRead {
                button_id: assignment.button_id,
                result: AssignmentRead::Observed { assignment },
            })
            .collect(),
        canceled: false,
    }
}
fn output(product: u32, function: u8, data: [u8; 5]) -> Value {
    serde_json::to_value(convert_assignment(product, &assignment(16, function, data)).unwrap())
        .unwrap()
}

#[test]
fn controller_mapping_keyboard_modifiers_extended_keys_and_layout_metadata_have_independent_expectations()
 {
    let actual = output(2636, 2, [0xff, 70, 0, 0, 0]);
    assert_eq!(
        actual,
        json!({"inputID":"BUTTON_M1","inputType":"ControllerInput","controllerInput":"BUTTON_M1","isHyperShift":false,"outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_PRINT_SCREEN","modifiers":["Extended_Key","Left_Alt","Left_Ctrl","Left_Shift","Right_Alt","Right_Ctrl","Right_Shift","Left_GUI","Right_GUI"],"extendedKey":"Extended_00"}})
    );
    let old:LocalKeyboardMetadata=serde_json::from_value(json!({"inputID":"BUTTON_M1","isHyperShift":false,"keyboardGroup":{"key":"KEY_PRINT_SCREEN","keyGroup":5,"modifiers":["Left_Alt"]}})).unwrap();
    let projection = project_mappings(
        2636,
        &observed(vec![assignment(16, 2, [1, 70, 0, 0, 0])]),
        &[old],
    )
    .unwrap();
    let saved = serde_json::to_value(projection.source_mappings().unwrap()).unwrap();
    assert_eq!(saved[0]["keyboardGroup"]["keyGroup"], 5);
    assert_eq!(
        saved[0]["keyboardGroup"]["modifiers"],
        json!(["Extended_Key", "Left_Ctrl"])
    );
}

#[test]
fn controller_mapping_modifier_only_has_seven_source_cases_and_turbo_keeps_distinct_rules() {
    for (mask, name) in [
        (1, "KEY_LEFT_CTRL"),
        (2, "KEY_LEFT_SHIFT"),
        (4, "KEY_LEFT_ALT"),
        (8, "KEY_LEFT_GUI"),
        (16, "KEY_RIGHT_CTRL"),
        (32, "KEY_RIGHT_SHIFT"),
        (64, "KEY_RIGHT_ALT"),
    ] {
        let result = output(2676, 2, [mask, 0, 0, 0, 0]);
        assert_eq!(result["keyboardGroup"]["key"], name);
        let expected = if [8, 16, 64].contains(&mask) {
            json!(["Extended_Key"])
        } else {
            json!([])
        };
        assert_eq!(result["keyboardGroup"]["modifiers"], expected);
    }
    for mask in [3, 128, 255] {
        assert!(convert_assignment(2676, &assignment(16, 2, [mask, 0, 0, 0, 0])).is_err());
    }
    assert!(convert_assignment(2676, &assignment(16, 13, [1, 0, 0, 100, 0])).is_err());
    assert_eq!(
        output(2676, 13, [1, 4, 0, 150, 0])["keyboardGroup"],
        json!({"key":"KEY_A","modifiers":["Left_Ctrl"],"turboMode":{"isTurbo":true,"keysPerSecond":7}})
    );
    assert!(convert_assignment(2676, &assignment(16, 13, [0, 4, 0, 0, 0])).is_err());
    assert_eq!(
        output(2676, 13, [0, 4, 255, 255, 0])["keyboardGroup"]["turboMode"]["keysPerSecond"],
        0
    );
}

#[test]
fn controller_mapping_media_swaps_protocol_bytes_before_generic_big_endian_conversion() {
    assert_eq!(
        output(2636, 10, [159, 2, 0, 0, 0])["multimediaGroup"],
        json!({"multimediaAssignment":"CycleApps"})
    );
    assert_eq!(
        output(2676, 10, [205, 0, 0, 0, 0])["multimediaGroup"],
        json!({"multimediaAssignment":"Play"})
    );
    assert!(convert_assignment(2676, &assignment(16, 10, [2, 159, 0, 0, 0])).is_err());
}

#[test]
fn controller_mapping_source_omission_is_separate_from_failed_and_nondefault_rows() {
    let reads = observed(vec![
        assignment(16, 16, [16, 0, 0, 0, 0]),
        assignment(17, 16, [0, 0, 0, 0, 0]),
        assignment(18, 16, [32, 0, 0, 0, 0]),
        assignment(61, 2, [0, 4, 0, 0, 0]),
    ]);
    let projection = project_mappings(2676, &reads, &[]).unwrap();
    assert!(matches!(
        projection.rows[0],
        MappingRow::OmittedSelf { button_id: 16 }
    ));
    assert!(matches!(
        projection.rows[1],
        MappingRow::OmittedSelf { button_id: 17 }
    ));
    assert!(matches!(
        projection.rows[3],
        MappingRow::OmittedDefault { .. }
    ));
    let saved = serde_json::to_value(projection.source_mappings().unwrap()).unwrap();
    assert_eq!(
        saved,
        json!([{"inputID":"BUTTON_M3","inputType":"ControllerInput","controllerInput":"BUTTON_M3","isHyperShift":false,"outputType":"controllerGroup","controllerGroup":{"controllerAssignment":"A_BUTTON"}}])
    );
    let share = project_mappings(
        2636,
        &observed(vec![assignment(61, 2, [0, 4, 0, 0, 0])]),
        &[],
    )
    .unwrap();
    assert_eq!(share.source_mappings().unwrap().len(), 1);
    assert!(convert_assignment(2676, &assignment(16, 16, [61, 0, 0, 0, 0])).is_err());
}

#[test]
fn controller_mapping_layout_metadata_requires_same_key_input_and_mode() {
    for (input, key, mode) in [
        ("BUTTON_M2", "KEY_A", false),
        ("BUTTON_M1", "KEY_B", false),
        ("BUTTON_M1", "KEY_A", true),
    ] {
        let local = serde_json::from_value::<LocalKeyboardMetadata>(
            json!({"inputID":input,"isHyperShift":mode,"keyboardGroup":{"key":key,"keyGroup":4}}),
        )
        .unwrap();
        let mappings = project_mappings(
            2636,
            &observed(vec![assignment(16, 2, [0, 4, 0, 0, 0])]),
            &[local],
        )
        .unwrap()
        .source_mappings()
        .unwrap();
        assert!(
            serde_json::to_value(&mappings).unwrap()[0]["keyboardGroup"]
                .get("keyGroup")
                .is_none()
        );
    }
}

#[test]
fn controller_mapping_partial_source_cache_keeps_errors_distinct_from_observed_defaults() {
    let mut reads = observed(vec![
        assignment(16, 2, [0, 4, 0, 0, 0]),
        assignment(17, 2, [0, 127, 0, 0, 0]),
    ]);
    reads.buttons.push(ButtonRead {
        button_id: 18,
        result: AssignmentRead::Failed {
            error: "real transfer failed".into(),
        },
    });
    let converted = project_mappings(2676, &reads, &[]).unwrap();
    assert_eq!(converted.rows.len(), 3);
    assert!(
        matches!(&converted.rows[1],MappingRow::Failed{button_id:17,error} if error.contains("127"))
    );
    assert!(
        matches!(&converted.rows[2],MappingRow::Failed{button_id:18,error} if error=="real transfer failed")
    );
    assert!(!converted.all_rows_observed());
    let partial = serde_json::to_value(converted.source_mappings().unwrap()).unwrap();
    assert_eq!(partial.as_array().unwrap().len(), 1);
    assert_eq!(partial[0]["keyboardGroup"]["key"], "KEY_A");
    reads.buttons.truncate(1);
    reads.canceled = true;
    let canceled = project_mappings(2676, &reads, &[]).unwrap();
    assert_eq!(canceled.rows.len(), 1);
    assert!(canceled.source_mappings().is_err());
}

#[test]
fn controller_mapping_source_output_groups_preserve_units_omissions_and_unresolved_macros() {
    assert_eq!(
        output(2636, 0, [0; 5]),
        json!({"inputID":"BUTTON_M1","inputType":"ControllerInput","controllerInput":"BUTTON_M1","isHyperShift":false,"outputType":"disableGroup"})
    );
    assert_eq!(
        output(2636, 6, [5, 3, 32, 0, 0])["sensitivityGroup"],
        json!({"sensitivityAssignment":"DPI_Clutch","x":800})
    );
    assert_eq!(
        output(2676, 7, [3, 0, 0, 0, 0])["profileNavigationGroup"],
        json!({"profileNavigationAssignment":"Specific"})
    );
    assert_eq!(
        output(2676, 9, [131, 0, 0, 0, 0])["powerKeyGroup"],
        json!({"powerKeyAssignment":"SystemWake"})
    );
    assert_eq!(
        output(2676, 17, [89, 0, 0, 0, 0])["razerKeyGroup"],
        json!({"razerKeyAssignment":"HyperShiftButton"})
    );
    assert_eq!(
        output(2676, 21, [100, 0, 0, 0, 0])["win8ShortcutsGroup"],
        json!({"windowsShortcutAssignment":"XboxGameBar"})
    );
    assert_eq!(
        output(2636, 12, [1, 0, 0, 0, 0]),
        json!({"inputID":"BUTTON_M1","inputType":"ControllerInput","controllerInput":"BUTTON_M1","isHyperShift":false,"outputType":"hyperShiftGroup","":{}})
    );
    let unresolved = convert_assignment(2636, &assignment(16, 3, [0x12, 0x34, 4, 0, 0])).unwrap();
    assert!(!unresolved.has_resolved_output());
    assert_eq!(
        serde_json::to_value(&unresolved).unwrap()["macroGroup"],
        json!({"id":"","macroPlaybackOption":"NTimes","repeatCount":4})
    );
}

#[test]
fn controller_mapping_perfect180_only_exists_in_the_current_2676_branch_and_keeps_numeric_enabled()
{
    assert_eq!(
        output(2676, 22, [1, 0x12, 0x34, 7, 0])["perfect180Group"],
        json!({"mouseAssignment":"Perfect180","direction":"left","reportCount":4660,"isEnabled":7})
    );
    assert!(convert_assignment(2636, &assignment(16, 22, [1, 0, 30, 1, 0])).is_err());
    for function in [8, 15, 18, 19, 20, 255] {
        assert!(convert_assignment(2676, &assignment(16, function, [1, 0, 0, 0, 0])).is_err());
    }
    assert!(convert_assignment(2676, &assignment(127, 2, [0, 4, 0, 0, 0])).is_err());
    assert_eq!(
        serde_json::to_value(
            convert_assignment(2676, &assignment(128, 2, [0, 4, 0, 0, 0])).unwrap()
        )
        .unwrap()["inputID"],
        "LEFT_JS_UP"
    );
    let mut oversized = assignment(16, 2, [0, 4, 0, 0, 0]);
    oversized.data_size = 6;
    assert!(convert_assignment(2676, &oversized).is_err());
}

#[test]
fn controller_mapping_known_source_cache_shapes_round_trip_through_typed_boundaries() {
    for (function, data) in [
        (0, [0; 5]),
        (1, [1, 0, 0, 0, 0]),
        (2, [0, 70, 0, 0, 0]),
        (3, [0, 2, 1, 0, 0]),
        (6, [5, 0, 10, 0, 20]),
        (7, [3, 4, 0, 0, 0]),
        (9, [129, 0, 0, 0, 0]),
        (10, [205, 0, 0, 0, 0]),
        (11, [1, 0, 0, 0, 0]),
        (12, [1, 0, 0, 0, 0]),
        (16, [32, 0, 0, 0, 0]),
        (17, [117, 0, 0, 0, 0]),
        (21, [99, 0, 0, 0, 0]),
        (22, [0, 0, 10, 1, 0]),
    ] {
        let mapping = convert_assignment(2676, &assignment(16, function, data)).unwrap();
        let document = serde_json::to_value(&mapping).unwrap();
        assert_eq!(
            serde_json::from_value::<SourceMapping>(document).unwrap(),
            mapping
        );
    }
}

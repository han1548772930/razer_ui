use super::super::deadzone_state::{DeadzoneState, Side};
use super::{TriggerOperation, TriggerRuntime, prepare_thumbstick_change};
use razer_device::controller_triggers::ThumbstickValue;
use serde_json::{Value, json};
use std::prelude::v1::test;

// The six class components retain mount/selection values; the two widgets
// additionally observe nonlow Redux values in their left/right effects.
const PRODUCTS: [(u32, bool); 8] = [
    (2629, false),
    (2636, false),
    (2647, false),
    (2650, false),
    (2676, true),
    (2684, true),
    (4133, false),
    (4144, false),
];

#[test]
fn source_close_dispatches_even_when_the_mounted_low_value_is_unchanged() {
    for side in [Side::Left, Side::Right] {
        for value in [0, 3, 5] {
            let runtime = TriggerRuntime::default();
            let owner = runtime.guard();
            let mut state = DeadzoneState::default();
            state.enter([value, value], owner.clone());
            assert!(state.select(side, value, [value, value], owner.clone()));
            assert_eq!(state.warning_side(), Some(side));
            let restore = state.dismiss(true, &owner).unwrap();
            assert_eq!(restore.path(), side.path());
            assert_eq!(restore.value(), value);
            assert_eq!(state.warning_side(), None);
            assert!(
                state.dismiss(true, &owner).is_none(),
                "one close, one dispatch"
            );
        }
    }
}

#[test]
fn source_continue_and_recalibrate_keep_the_mount_baseline_without_a_restore() {
    let runtime = TriggerRuntime::default();
    let owner = runtime.guard();
    let mut state = DeadzoneState::default();
    state.enter([9, 11], owner.clone());
    for side in [Side::Left, Side::Right] {
        for action in ["Continue", "Recalibrate", "PageLeave", "ProfileRestore"] {
            assert!(state.select(side, 3, [3, 3], owner.clone()));
            assert!(state.dismiss(false, &owner).is_none(), "{action}");
            assert_eq!(state.warning_side(), None);
        }
        assert!(state.select(side, 0, [0, 0], owner.clone()));
        let restore = state.dismiss(true, &owner).unwrap();
        assert_eq!(restore.value(), if side == Side::Left { 9 } else { 11 });
    }
}

#[test]
fn only_a_nonlow_selection_replaces_its_own_previous_value() {
    let runtime = TriggerRuntime::default();
    let owner = runtime.guard();
    let mut state = DeadzoneState::default();
    state.enter([5, 3], owner.clone());
    assert!(!state.select(Side::Left, 7, [5, 3], owner.clone()));
    assert_eq!(state.warning_side(), None);
    assert!(state.select(Side::Left, 3, [7, 3], owner.clone()));
    assert!(state.dismiss(false, &owner).is_none());
    assert!(state.select(Side::Left, 0, [3, 3], owner.clone()));
    assert_eq!(state.dismiss(true, &owner).unwrap().value(), 7);
    assert!(state.select(Side::Right, 0, [7, 3], owner.clone()));
    assert_eq!(state.dismiss(true, &owner).unwrap().value(), 3);
}

#[test]
fn reentering_the_component_uses_the_new_displayed_values_and_closes_old_warning() {
    let runtime = TriggerRuntime::default();
    let owner = runtime.guard();
    let mut state = DeadzoneState::default();
    state.enter([9, 11], owner.clone());
    assert!(state.select(Side::Left, 3, [9, 11], owner.clone()));
    state.enter([3, 5], owner.clone());
    assert_eq!(state.warning_side(), None);
    assert!(state.dismiss(true, &owner).is_none());
    assert!(state.select(Side::Left, 0, [3, 5], owner.clone()));
    assert_eq!(state.dismiss(true, &owner).unwrap().value(), 3);
}

#[test]
fn old_warning_never_restores_into_another_or_invalidated_trigger_owner() {
    for invalidate in [false, true] {
        let mut runtime = TriggerRuntime::default();
        let owner = runtime.guard();
        let mut state = DeadzoneState::default();
        state.enter([9, 11], owner.clone());
        assert!(state.select(Side::Left, 3, [9, 11], owner.clone()));
        let current = if invalidate {
            runtime.invalidate();
            assert!(!owner.load(std::sync::atomic::Ordering::Acquire));
            runtime.guard()
        } else {
            TriggerRuntime::default().guard()
        };
        assert!(state.dismiss(true, &current).is_none());
        assert_eq!(state.warning_side(), None);
    }
}

#[test]
fn dropping_the_trigger_owner_cancels_a_retained_warning() {
    let runtime = TriggerRuntime::default();
    let owner = runtime.guard();
    let mut state = DeadzoneState::default();
    state.enter([9, 11], owner.clone());
    assert!(state.select(Side::Right, 3, [9, 11], owner.clone()));
    drop(runtime);
    assert!(state.dismiss(true, &owner).is_none());
}

#[test]
fn selection_after_connection_replacement_never_reuses_the_old_previous_values() {
    let mut runtime = TriggerRuntime::default();
    let owner = runtime.guard();
    let mut state = DeadzoneState::default();
    state.enter([5, 3], owner.clone());
    assert!(state.select(Side::Left, 0, [5, 3], owner));
    runtime.invalidate();
    let owner = runtime.guard();
    assert!(state.select(Side::Right, 0, [11, 13], owner.clone()));
    assert_eq!(state.dismiss(true, &owner).unwrap().value(), 13);
    assert!(state.select(Side::Left, 3, [11, 13], owner.clone()));
    assert_eq!(state.dismiss(true, &owner).unwrap().value(), 11);
}

#[test]
fn current_product_branches_observe_only_their_source_previous_value_updates() {
    for (product_id, follows_displayed_values) in PRODUCTS {
        let runtime = TriggerRuntime::default();
        let owner = runtime.guard();
        let mut state = DeadzoneState::default();
        state.enter([5, 9], owner.clone());
        state.synchronize([13, 11], follows_displayed_values, owner.clone());
        state.synchronize([3, 0], follows_displayed_values, owner.clone());
        for (side, expected) in [
            (Side::Left, if follows_displayed_values { 13 } else { 5 }),
            (Side::Right, if follows_displayed_values { 11 } else { 9 }),
        ] {
            assert!(state.select(side, 0, [3, 0], owner.clone()));
            assert_eq!(
                state.dismiss(true, &owner).unwrap().value(),
                expected,
                "product {product_id}, side {side:?}"
            );
        }
    }
}

#[test]
fn close_uses_the_current_previous_value_after_a_display_update_while_open() {
    for (product_id, follows_displayed_values) in PRODUCTS {
        for side in [Side::Left, Side::Right] {
            let runtime = TriggerRuntime::default();
            let owner = runtime.guard();
            let mut state = DeadzoneState::default();
            state.enter([9, 11], owner.clone());
            assert!(state.select(side, 0, [9, 11], owner.clone()));
            state.synchronize([13, 15], follows_displayed_values, owner.clone());
            assert_eq!(state.warning_side(), Some(side));
            let expected = match (follows_displayed_values, side) {
                (false, Side::Left) => 9,
                (false, Side::Right) => 11,
                (true, Side::Left) => 13,
                (true, Side::Right) => 15,
            };
            assert_eq!(
                state.dismiss(true, &owner).unwrap().value(),
                expected,
                "product {product_id}, side {side:?}"
            );
        }
    }
}

#[test]
fn displayed_values_from_a_new_owner_clear_the_previous_warning_and_baseline() {
    for (product_id, follows_displayed_values) in PRODUCTS {
        let mut runtime = TriggerRuntime::default();
        let owner = runtime.guard();
        let mut state = DeadzoneState::default();
        state.enter([13, 15], owner.clone());
        assert!(state.select(Side::Left, 0, [13, 15], owner.clone()));
        runtime.invalidate();
        let current = runtime.guard();
        state.synchronize([3, 5], follows_displayed_values, current.clone());
        assert_eq!(state.warning_side(), None, "product {product_id}");
        assert!(state.dismiss(true, &owner).is_none());
        assert!(state.select(Side::Right, 0, [3, 5], current.clone()));
        assert_eq!(state.dismiss(true, &current).unwrap().value(), 5);
    }
}

fn draft() -> Value {
    json!({
        "profile": {
            "guid": "current-profile",
            "obmSlotId": [3],
            "controllerDeadzone": {
                "deadzones": [{"controllerId":2,"value":9},{"controllerId":1,"value":5}],
                "preventDoubleDeadzone": true
            }
        },
        "controller": {
            "leftStick": {"deadzoneValue":5,"preventDoubleDeadzone":true},
            "rightStick": {"deadzoneValue":9,"preventDoubleDeadzone":false}
        }
    })
}

fn enqueue_selection(
    runtime: &mut TriggerRuntime,
    draft: &mut Value,
    path: &str,
    value: i64,
    legacy: bool,
) {
    *draft.pointer_mut(path).unwrap() = json!(value);
    let operation = prepare_thumbstick_change(draft, path, 0, legacy).unwrap();
    runtime.enqueue(operation, draft["profile"].clone());
}

#[test]
fn every_product_keeps_close_dispatch_order_and_its_source_prevention_field() {
    for (product_id, modern) in PRODUCTS {
        for side in [Side::Left, Side::Right] {
            let mut runtime = TriggerRuntime::default();
            let owner = runtime.guard();
            let mut state = DeadzoneState::default();
            let mut draft = draft();
            draft["controller"]["rightStick"]["deadzoneValue"] = json!(3);
            draft["profile"]["controllerDeadzone"]["deadzones"][0]["value"] = json!(3);
            state.enter([5, 3], owner.clone());
            let selected = if side == Side::Left { 5 } else { 3 };
            assert!(state.select(side, selected, [5, 3], owner.clone()));
            enqueue_selection(&mut runtime, &mut draft, side.path(), selected, !modern);
            let restore = state.dismiss(true, &owner).unwrap();
            enqueue_selection(
                &mut runtime,
                &mut draft,
                restore.path(),
                restore.value(),
                !modern,
            );
            let expected = TriggerOperation::ThumbstickDeadzone {
                profile_slot: 3,
                settings: vec![
                    ThumbstickValue::new(2, 3).unwrap(),
                    ThumbstickValue::new(1, 5).unwrap(),
                ],
                prevent_double_deadzone: !modern || side == Side::Left,
            };
            assert_eq!(
                runtime.pending.len(),
                2,
                "product {product_id}, side {side:?}"
            );
            for (index, request) in runtime.pending.iter().enumerate() {
                assert_eq!(request.operation(), &expected, "product {product_id}");
                assert_eq!(request.profile()["guid"], "current-profile");
                assert_eq!(request.revision, index as u64 + 1);
                assert!(request.is_valid());
            }
            assert!(runtime.last_result.is_none());
        }
    }
}

#[test]
fn equal_close_retains_a_distinct_ordered_profile_request_without_fabricating_an_ack() {
    let mut runtime = TriggerRuntime::default();
    let owner = runtime.guard();
    let mut state = DeadzoneState::default();
    let mut draft = draft();
    state.enter([5, 9], owner.clone());
    assert!(state.select(Side::Left, 5, [5, 9], owner.clone()));
    enqueue_selection(&mut runtime, &mut draft, Side::Left.path(), 5, true);
    let restore = state.dismiss(true, &owner).unwrap();
    enqueue_selection(
        &mut runtime,
        &mut draft,
        restore.path(),
        restore.value(),
        true,
    );
    assert_eq!(runtime.pending.len(), 2);
    let expected = TriggerOperation::ThumbstickDeadzone {
        profile_slot: 3,
        settings: vec![
            ThumbstickValue::new(2, 9).unwrap(),
            ThumbstickValue::new(1, 5).unwrap(),
        ],
        prevent_double_deadzone: true,
    };
    for (index, request) in runtime.pending.iter().enumerate() {
        assert_eq!(request.operation(), &expected);
        assert_eq!(request.profile()["guid"], "current-profile");
        assert_eq!(request.revision, index as u64 + 1);
        assert!(request.is_valid());
    }
    assert!(runtime.last_result.is_none());
    let first = runtime.next().unwrap();
    assert!(runtime.matches(&first));
    assert!(runtime.next().is_none());
    runtime.invalidate();
    assert!(!runtime.matches(&first));
    assert!(!first.is_valid());
    assert!(runtime.next().is_none());
}

#[test]
fn equal_close_advances_the_revision_so_an_inflight_get_cannot_replace_its_draft() {
    let mut runtime = TriggerRuntime::default();
    runtime.enqueue(
        TriggerOperation::ReadThumbsticks { profile_slot: 3 },
        json!({}),
    );
    let read = runtime.next().unwrap();
    let owner = runtime.guard();
    let mut state = DeadzoneState::default();
    let mut draft = draft();
    state.enter([5, 9], owner.clone());
    assert!(state.select(Side::Left, 5, [5, 9], owner.clone()));
    enqueue_selection(&mut runtime, &mut draft, Side::Left.path(), 5, true);
    let restore = state.dismiss(true, &owner).unwrap();
    enqueue_selection(
        &mut runtime,
        &mut draft,
        restore.path(),
        restore.value(),
        true,
    );
    assert!(runtime.matches(&read));
    assert_ne!(read.revision, runtime.revision);
    assert_eq!(draft["controller"]["leftStick"]["deadzoneValue"], 5);
    assert_eq!(runtime.pending.len(), 2);
    assert!(runtime.last_result.is_none());
}

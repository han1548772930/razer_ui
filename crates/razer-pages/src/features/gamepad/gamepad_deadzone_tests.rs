use super::{TriggerOperation, TriggerRuntime, prepare_thumbstick_change};
use super::super::deadzone_state::{DeadzoneState, Side};
use razer_device::controller_triggers::ThumbstickValue;
use serde_json::{Value, json};
use std::prelude::v1::test;

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
            assert!(state.dismiss(true, &owner).is_none(), "one close, one dispatch");
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

fn enqueue_selection(runtime: &mut TriggerRuntime, draft: &mut Value, path: &str, value: i64) {
    *draft.pointer_mut(path).unwrap() = json!(value);
    let operation = prepare_thumbstick_change(draft, path, 0, true).unwrap();
    runtime.enqueue(operation, draft["profile"].clone());
}

#[test]
fn equal_close_retains_a_distinct_ordered_profile_request_without_fabricating_an_ack() {
    let mut runtime = TriggerRuntime::default();
    let owner = runtime.guard();
    let mut state = DeadzoneState::default();
    let mut draft = draft();
    state.enter([5, 9], owner.clone());
    assert!(state.select(Side::Left, 5, [5, 9], owner.clone()));
    enqueue_selection(&mut runtime, &mut draft, Side::Left.path(), 5);
    let restore = state.dismiss(true, &owner).unwrap();
    enqueue_selection(&mut runtime, &mut draft, restore.path(), restore.value());
    assert_eq!(runtime.pending.len(), 2);
    let expected = TriggerOperation::ThumbstickDeadzone {
        profile_slot: 3,
        settings: vec![ThumbstickValue::new(2, 9).unwrap(), ThumbstickValue::new(1, 5).unwrap()],
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
    runtime.enqueue(TriggerOperation::ReadThumbsticks { profile_slot: 3 }, json!({}));
    let read = runtime.next().unwrap();
    let owner = runtime.guard();
    let mut state = DeadzoneState::default();
    let mut draft = draft();
    state.enter([5, 9], owner.clone());
    assert!(state.select(Side::Left, 5, [5, 9], owner.clone()));
    enqueue_selection(&mut runtime, &mut draft, Side::Left.path(), 5);
    let restore = state.dismiss(true, &owner).unwrap();
    enqueue_selection(&mut runtime, &mut draft, restore.path(), restore.value());
    assert!(runtime.matches(&read));
    assert_ne!(read.revision, runtime.revision);
    assert_eq!(draft["controller"]["leftStick"]["deadzoneValue"], 5);
    assert_eq!(runtime.pending.len(), 2);
    assert!(runtime.last_result.is_none());
}

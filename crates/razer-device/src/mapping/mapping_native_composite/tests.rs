use super::*;
use crate::{
    mapping_native_digital::{JoystickGroup, NativeDigitalOutput},
    mapping_native_lookup::{Input, InputKind, Selectors},
};
fn input(code: u16) -> Input {
    Input::new(InputKind::Keyboard {
        scancode: code,
        flag: 0,
    })
}
fn resolved(input: Input, output: Output) -> Resolved<Output> {
    Resolved {
        input,
        output: ResolvedOutput::Mapped(output),
        selectors: Selectors::default(),
        mapping_enabled: true,
        global_shortcut: false,
    }
}
fn marker(value: u32) -> Output {
    Output::Hypershift { flag: value }
}
fn flag(dispatch: &Dispatch) -> u32 {
    let ResolvedOutput::Mapped(Output::Hypershift { flag }) = dispatch.resolved.output else {
        panic!("Expected marker")
    };
    flag
}
fn pair(key: u16, make: Output, release: Output) -> HeldMapping<Output> {
    HeldMapping {
        make: resolved(input(key), make),
        release: resolved(input(key).released(), release),
    }
}
fn branches(condition: Option<u16>, first: Output, last: Output) -> Vec<Branch> {
    vec![
        Branch {
            input: condition.map(input),
            output: first,
        },
        Branch {
            input: None,
            output: last,
        },
    ]
}

#[test]
fn cascade_selects_first_held_condition_and_suppresses_same_pair_make() {
    let mut state = State::default();
    let output = Output::Cascade(branches(Some(31), marker(1), marker(2)));
    let release = Output::Cascade(branches(Some(31), marker(3), marker(4)));
    let held = BTreeMap::from([
        (input(30).stateless_id(), pair(30, output.clone(), release)),
        (input(31).stateless_id(), pair(31, marker(5), marker(6))),
    ]);
    let selected = state
        .apply(resolved(input(30), output.clone()), |id| {
            held.get(id).cloned()
        })
        .unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(flag(&selected[0]), 1);
    let selected = state
        .apply(resolved(input(30), output), |id| held.get(id).cloned())
        .unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(flag(&selected[0]), 2);
}

#[test]
fn cascade_release_uses_press_bound_counterpart_when_other_key_was_released() {
    let mut state = State::default();
    let make = Output::Cascade(branches(Some(31), marker(1), marker(2)));
    let release = Output::Cascade(branches(Some(31), marker(3), marker(4)));
    let mut held = BTreeMap::from([
        (
            input(30).stateless_id(),
            pair(30, make.clone(), release.clone()),
        ),
        (input(31).stateless_id(), pair(31, marker(5), marker(6))),
    ]);
    state
        .apply(resolved(input(30), make), |id| held.get(id).cloned())
        .unwrap();
    held.remove(&input(31).stateless_id());
    held.remove(&input(30).stateless_id());
    let selected = state
        .apply(resolved(input(30).released(), release), |id| {
            held.get(id).cloned()
        })
        .unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(flag(&selected[0]), 3);
    assert_eq!(selected[0].resolved.input.phase(), Phase::Release);
}

#[test]
fn dpad_releases_previous_combination_before_new_output_then_restores_held_single() {
    let mut state = State::default();
    let am = Output::Dpad(branches(Some(31), marker(1), marker(2)));
    let ar = Output::Dpad(branches(Some(31), marker(3), marker(4)));
    let bm = Output::Dpad(branches(Some(30), marker(5), marker(6)));
    let br = Output::Dpad(branches(Some(30), marker(7), marker(8)));
    let mut held = BTreeMap::from([(input(30).stateless_id(), pair(30, am.clone(), ar))]);
    assert_eq!(
        flag(
            &state
                .apply(resolved(input(30), am), |id| held.get(id).cloned())
                .unwrap()[0]
        ),
        2
    );
    held.insert(input(31).stateless_id(), pair(31, bm.clone(), br.clone()));
    let selected = state
        .apply(resolved(input(31), bm), |id| held.get(id).cloned())
        .unwrap();
    assert_eq!(selected.iter().map(flag).collect::<Vec<_>>(), [4, 5]);
    held.remove(&input(31).stateless_id());
    let selected = state
        .apply(resolved(input(31).released(), br), |id| {
            held.get(id).cloned()
        })
        .unwrap();
    assert_eq!(selected.iter().map(flag).collect::<Vec<_>>(), [7, 2]);
    assert_eq!(selected[1].resolved.input, input(30));
}

#[test]
fn dpad_digital_pair_identity_is_sorted_and_shared_across_input_order() {
    let axis = Output::Digital(NativeDigitalOutput::JoystickAxes {
        group: JoystickGroup::Axes,
        value: [1, 0, 0],
    });
    let am = Output::Dpad(branches(Some(31), axis.clone(), axis.clone()));
    let bm = Output::Dpad(branches(Some(30), axis.clone(), axis));
    let held = BTreeMap::from([
        (input(30).stateless_id(), pair(30, am.clone(), am.clone())),
        (input(31).stateless_id(), pair(31, bm.clone(), bm.clone())),
    ]);
    let a = State::default()
        .apply(resolved(input(30), am), |id| held.get(id).cloned())
        .unwrap();
    let b = State::default()
        .apply(resolved(input(31), bm), |id| held.get(id).cloned())
        .unwrap();
    assert_eq!(a[0].contribution_id, Some("t21s30f0t21s31f0".into()));
    assert_eq!(a[0].contribution_id, b[0].contribution_id);
}

#[test]
fn dpad_no_matching_condition_produces_no_output() {
    let mut state = State::default();
    let output = Output::Dpad(vec![Branch {
        input: Some(input(31)),
        output: marker(1),
    }]);
    assert!(
        state
            .apply(resolved(input(30), output), |_| None)
            .unwrap()
            .is_empty()
    );
}

use super::*;
use crate::{
    mapping_native_action::parse_output,
    mapping_native_lookup::{Input, InputKind, Selectors},
};
use serde_json::json;

fn key(group: u32, code: u16, phase: Phase) -> Resolved<Output> {
    let flag = u16::from(phase == Phase::Release);
    Resolved {
        input: Input::new(InputKind::Keyboard {
            scancode: code,
            flag,
        }),
        output: ResolvedOutput::Mapped(
            parse_output(&json!({"type":"keyboard","scancode":code + 10,"flag":flag})).unwrap(),
        ),
        selectors: Selectors {
            dynamic_key: Some(group),
            ..Default::default()
        },
        mapping_enabled: true,
        global_shortcut: false,
    }
}
fn run(state: &mut State, mapping: Resolved<Output>) -> (bool, Vec<Effect>) {
    let Some(mut transition) = state.begin(&mapping) else {
        return (false, Vec::new());
    };
    let mut effects = Vec::new();
    while let Some(effect) = state.next(&mut transition) {
        effects.push(effect);
    }
    (transition.handled(), effects)
}
#[test]
fn first_two_presses_dispatch_and_later_keys_wait_in_insertion_order() {
    let mut state = State::default();
    for code in [30, 31] {
        assert_eq!(
            run(&mut state, key(7, code, Phase::Press)),
            (true, vec![Effect::Current])
        );
    }
    for code in [32, 33] {
        assert_eq!(run(&mut state, key(7, code, Phase::Press)), (true, vec![]));
    }
    assert_eq!(
        run(&mut state, key(7, 30, Phase::Release)),
        (
            true,
            vec![
                Effect::Current,
                Effect::Pause,
                Effect::Replay(key(7, 32, Phase::Press))
            ]
        )
    );
    assert_eq!(
        run(&mut state, key(7, 32, Phase::Release)),
        (
            true,
            vec![
                Effect::Current,
                Effect::Pause,
                Effect::Replay(key(7, 33, Phase::Press))
            ]
        )
    );
    assert_eq!(
        run(&mut state, key(7, 31, Phase::Release)),
        (true, vec![Effect::Current])
    );
    assert_eq!(
        run(&mut state, key(7, 33, Phase::Release)),
        (true, vec![Effect::Current])
    );
}
#[test]
fn tail_and_missing_releases_replay_second_retained_make_with_distinct_consumption() {
    let mut state = State::default();
    for code in [30, 31, 32, 33] {
        run(&mut state, key(7, code, Phase::Press));
    }
    let replay = vec![Effect::Pause, Effect::Replay(key(7, 31, Phase::Press))];
    assert_eq!(
        run(&mut state, key(7, 33, Phase::Release)),
        (true, replay.clone())
    );
    assert_eq!(run(&mut state, key(7, 99, Phase::Release)), (false, replay));
}
#[test]
fn state_mutation_happens_after_current_callback_and_duplicates_remove_first_match() {
    let mut state = State::default();
    let mut press = state.begin(&key(0, 30, Phase::Press)).unwrap();
    assert_eq!(state.next(&mut press), Some(Effect::Current));
    assert!(state.groups[&0].is_empty());
    assert_eq!(state.next(&mut press), None);
    assert!(press.handled());
    run(&mut state, key(0, 30, Phase::Press));
    run(&mut state, key(0, 31, Phase::Press));
    let mut release = state.begin(&key(0, 30, Phase::Release)).unwrap();
    assert_eq!(state.next(&mut release), Some(Effect::Current));
    assert_eq!(state.groups[&0].len(), 3);
    assert_eq!(state.next(&mut release), Some(Effect::Pause));
    assert_eq!(state.groups[&0].len(), 2);
    assert_eq!(
        state.next(&mut release),
        Some(Effect::Replay(key(0, 31, Phase::Press)))
    );
    assert_eq!(state.next(&mut release), None);
}
#[test]
fn independent_groups_and_retained_mapping_snapshots_survive_table_changes() {
    let mut state = State::default();
    let mut original = key(u32::MAX, 31, Phase::Press);
    original.input.modifiers = 64;
    for mapping in [
        key(u32::MAX, 30, Phase::Press),
        original.clone(),
        key(u32::MAX, 32, Phase::Press),
    ] {
        run(&mut state, mapping);
    }
    assert_eq!(
        run(&mut state, key(0, 40, Phase::Press)),
        (true, vec![Effect::Current])
    );
    let mut release = key(u32::MAX, 32, Phase::Release);
    release.input.modifiers = 128;
    assert_eq!(
        run(&mut state, release),
        (true, vec![Effect::Pause, Effect::Replay(original)])
    );
    state.clear();
    assert_eq!(
        run(&mut state, key(u32::MAX, 31, Phase::Release)),
        (false, vec![])
    );
}
#[test]
fn only_direct_keyboard_outputs_with_dkp_id_are_eligible() {
    let mut state = State::default();
    let mut mapping = key(7, 30, Phase::Press);
    mapping.selectors.dynamic_key = None;
    assert!(state.begin(&mapping).is_none());
    mapping.selectors.dynamic_key = Some(7);
    for output in [
        Output::Disabled,
        Output::Multi(vec![
            parse_output(&json!({"type":"keyboard","scancode":40,"flag":0})).unwrap(),
        ]),
        Output::Macro {
            guid: "test".into(),
            flag: 0,
            repeat: 1,
        },
    ] {
        mapping.output = ResolvedOutput::Mapped(output);
        assert!(state.begin(&mapping).is_none());
    }
    mapping.output = ResolvedOutput::Passthrough(mapping.input.clone());
    assert!(state.begin(&mapping).is_none());
    mapping = key(7, 30, Phase::Press);
    mapping.input = Input::new(InputKind::ControlPodDial { clockwise: true });
    assert_eq!(run(&mut state, mapping), (false, vec![]));
}

#[test]
fn current_native_dkp_id_boundary_selects_dynamic_before_snaptap() {
    use crate::mapping_native_lookup::{Lookup, Modes, Tables, parse_mappings};
    let engine = json!({"mappings":[
        {"input":{"type":"keyboard","scancode":30,"flag":0,"dkpId":-1,"snaptapId":7},"output":{"type":"keyboard","scancode":40,"flag":0}},
        {"input":{"type":"keyboard","scancode":30,"flag":1,"dkpId":-1,"snaptapId":7},"output":{"type":"keyboard","scancode":40,"flag":1}}
    ]});
    let mappings = parse_mappings(&engine, parse_output).unwrap();
    assert!(
        mappings
            .iter()
            .all(|mapping| mapping.selectors.dynamic_key == Some(u32::MAX)
                && mapping.selectors.snap_tap.is_none())
    );
    let mut lookup = Lookup::new(Tables::new(vec![], mappings, vec![]).unwrap());
    let modes = Modes {
        mapping_enabled: true,
        global_shortcuts_enabled: true,
        hypershift: false,
        gaming: false,
        otfs: false,
    };
    let mut state = State::default();
    let physical = Input::new(InputKind::Keyboard {
        scancode: 30,
        flag: 0,
    });
    assert_eq!(
        run(&mut state, lookup.resolve(physical.clone(), modes).unwrap()),
        (true, vec![Effect::Current])
    );
    assert_eq!(
        run(
            &mut state,
            lookup.resolve(physical.released(), modes).unwrap()
        ),
        (true, vec![Effect::Current])
    );
}

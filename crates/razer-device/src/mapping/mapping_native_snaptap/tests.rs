use super::*;
use crate::{
    mapping_native_action::parse_output,
    mapping_native_hid::NativeHidDeviceConfig,
    mapping_native_lookup::{Input, InputKind, Selectors},
};
use serde_json::json;

fn key(scancode: u16, phase: Phase, priority: bool) -> Resolved<Output> {
    let flag = u16::from(phase == Phase::Release);
    Resolved {
        input: Input::new(InputKind::Keyboard { scancode, flag }),
        output: ResolvedOutput::Mapped(
            parse_output(&json!({"type":"keyboard","scancode":scancode,"flag":flag})).unwrap(),
        ),
        selectors: Selectors {
            snap_tap: Some(7),
            snap_tap_prioritize: priority,
            ..Default::default()
        },
        mapping_enabled: true,
        global_shortcut: false,
    }
}

fn replay(scancode: u16, phase: Phase) -> Effect {
    Effect::Replay {
        identity: key(scancode, phase, false).input.stateless_id(),
        phase,
    }
}

fn run(state: &mut DigitalState, input: Resolved<Output>, policy: Policy) -> Vec<Effect> {
    let mut transition = state.begin(&input, policy).unwrap().unwrap();
    let mut effects = Vec::new();
    while let Some(effect) = state.next(&mut transition) {
        if let Effect::Replay { identity, phase } = &effect {
            assert!(state.replayed(identity, *phase));
        }
        effects.push(effect);
    }
    effects
}

#[test]
fn last_input_releases_previous_before_pause_and_restores_latest_remaining_key() {
    let mut state = DigitalState::default();
    assert_eq!(
        run(&mut state, key(30, Phase::Press, false), Policy::LastInput),
        [Effect::Current]
    );
    let mut incoming = state
        .begin(&key(31, Phase::Press, false), Policy::LastInput)
        .unwrap()
        .unwrap();
    assert_eq!(state.next(&mut incoming), Some(replay(30, Phase::Release)));
    assert!(
        !state
            .active
            .contains_key(&key(31, Phase::Press, false).input.stateless_id())
    );
    assert!(state.replayed(
        &key(30, Phase::Press, false).input.stateless_id(),
        Phase::Release
    ));
    assert_eq!(state.next(&mut incoming), Some(Effect::Pause));
    assert_eq!(state.next(&mut incoming), Some(Effect::Current));
    assert_eq!(state.next(&mut incoming), None);
    assert_eq!(
        run(&mut state, key(32, Phase::Press, false), Policy::LastInput),
        [replay(31, Phase::Release), Effect::Pause, Effect::Current]
    );
    assert!(!state.physical_release(&key(32, Phase::Press, false).input.stateless_id()));
    assert_eq!(
        run(
            &mut state,
            key(32, Phase::Release, false),
            Policy::LastInput
        ),
        [Effect::Current, Effect::Pause, replay(31, Phase::Press)]
    );
    assert!(state.physical_release(&key(30, Phase::Press, false).input.stateless_id()));
    assert_eq!(
        run(
            &mut state,
            key(30, Phase::Release, false),
            Policy::LastInput
        ),
        [Effect::Current]
    );
}

#[test]
fn prioritized_keys_suppress_normal_keys_and_restore_them_in_press_order() {
    let mut state = DigitalState::default();
    run(&mut state, key(30, Phase::Press, false), Policy::Priority);
    assert_eq!(
        run(&mut state, key(31, Phase::Press, true), Policy::Priority),
        [replay(30, Phase::Release), Effect::Pause, Effect::Current]
    );
    assert!(run(&mut state, key(32, Phase::Press, false), Policy::Priority).is_empty());
    assert_eq!(
        run(&mut state, key(33, Phase::Press, true), Policy::Priority),
        [replay(31, Phase::Release), Effect::Pause, Effect::Current]
    );
    assert!(run(&mut state, key(31, Phase::Release, true), Policy::Priority).is_empty());
    assert_eq!(
        run(&mut state, key(33, Phase::Release, true), Policy::Priority),
        [Effect::Current, Effect::Pause, replay(32, Phase::Press)]
    );
    assert_eq!(
        run(&mut state, key(32, Phase::Release, false), Policy::Priority),
        [Effect::Current, Effect::Pause, replay(30, Phase::Press)]
    );
}

#[test]
fn neutral_emits_only_when_one_key_remains_and_has_no_digital_pause() {
    let mut state = DigitalState::default();
    assert_eq!(
        run(&mut state, key(30, Phase::Press, false), Policy::Neutral),
        [Effect::Current]
    );
    assert_eq!(
        run(&mut state, key(31, Phase::Press, false), Policy::Neutral),
        [replay(30, Phase::Release)]
    );
    assert_eq!(
        run(&mut state, key(32, Phase::Press, false), Policy::Neutral),
        [replay(31, Phase::Release)]
    );
    assert!(run(&mut state, key(31, Phase::Release, false), Policy::Neutral).is_empty());
    assert_eq!(
        run(&mut state, key(32, Phase::Release, false), Policy::Neutral),
        [replay(30, Phase::Press)]
    );
    assert_eq!(
        run(&mut state, key(30, Phase::Release, false), Policy::Neutral),
        [Effect::Current]
    );
}

#[test]
fn native_exclusions_and_unsupported_digital_policy_remain_distinct() {
    let state = DigitalState::default();
    let input = key(30, Phase::Press, false);
    assert!(state.begin(&input, Policy::CompareLevel).is_err());
    for output in [
        Output::Event(Event::Clipboard("text".into())),
        Output::Dpad(vec![]),
        Output::Hypershift { flag: 0 },
        Output::ModTap {
            milliseconds: 1,
            tap: Box::new(Output::Disabled),
            hold: Box::new(Output::Disabled),
        },
    ] {
        let mut excluded = input.clone();
        excluded.output = ResolvedOutput::Mapped(output);
        assert!(
            state
                .begin(&excluded, Policy::CompareLevel)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn profile_parses_all_policies_rejects_duplicates_and_invalid_values() {
    let mut doc = json!({"activeProfile":"p","profiles":[{"guid":"p","appEngine":{"mappings":[],"snaptaps":{"0":0,"+1":1,"02":2,"3":3}}}]});
    let config = NativeHidDeviceConfig::from_document(&doc).unwrap();
    assert_eq!(
        config.profile.snaptaps.into_values().collect::<Vec<_>>(),
        [
            Policy::LastInput,
            Policy::Priority,
            Policy::CompareLevel,
            Policy::Neutral
        ]
    );
    for value in [
        json!({"1":0,"01":1}),
        json!({"1":4}),
        json!({"1":-1}),
        json!({"1":0.5}),
        json!({" 1":1}),
        json!({"4294967296":1}),
    ] {
        doc["profiles"][0]["appEngine"]["snaptaps"] = value;
        assert!(NativeHidDeviceConfig::from_document(&doc).is_err());
    }
    doc["profiles"][0]["appEngine"]["snaptaps"] = json!([]);
    assert!(
        NativeHidDeviceConfig::from_document(&doc)
            .unwrap()
            .profile
            .snaptaps
            .is_empty()
    );
}

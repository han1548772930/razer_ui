use super::*;

fn replay(key_id: u8, phase: Phase) -> AnalogEffect {
    AnalogEffect::Replay { key_id, phase }
}
fn current() -> AnalogEffect {
    AnalogEffect::Current { execute: true }
}
fn levels(values: &[(u8, u16)]) -> BTreeMap<u8, u16> {
    values.iter().copied().collect()
}
fn run(
    state: &mut AnalogState,
    mut transition: AnalogTransition,
    levels: &BTreeMap<u8, u16>,
) -> Vec<AnalogEffect> {
    let mut effects = Vec::new();
    while let Some(effect) = state.next(&mut transition, |key| levels.get(&key).copied()) {
        if let AnalogEffect::Replay { key_id, phase } = effect {
            state.replayed(key_id, phase);
        }
        effects.push(effect);
    }
    effects
}
fn edge(
    state: &mut AnalogState,
    key: u8,
    priority: bool,
    policy: Policy,
    phase: Phase,
    levels: &BTreeMap<u8, u16>,
) -> Vec<AnalogEffect> {
    let transition = state
        .begin(key, 7, priority, policy, phase)
        .unwrap()
        .unwrap();
    run(state, transition, levels)
}

#[test]
fn last_input_notifies_current_before_appending_and_replays_without_eager_mutation() {
    let mut state = AnalogState::default();
    let mut first = state
        .begin(30, 7, false, Policy::LastInput, Phase::Press)
        .unwrap()
        .unwrap();
    assert_eq!(state.next(&mut first, |_| None), Some(current()));
    assert!(state.key_state(30).unwrap().active);
    assert!(state.groups[&7].normal.is_empty());
    assert_eq!(state.next(&mut first, |_| None), None);
    assert_eq!(state.groups[&7].normal, [30]);

    let mut second = state
        .begin(31, 7, false, Policy::LastInput, Phase::Press)
        .unwrap()
        .unwrap();
    assert_eq!(
        state.next(&mut second, |_| None),
        Some(replay(30, Phase::Release))
    );
    // A missing retained pair prevents replayed(); merely producing an effect
    // cannot change the original key's state.
    assert!(state.key_state(30).unwrap().active);
    assert_eq!(state.next(&mut second, |_| None), Some(AnalogEffect::Pause));
    assert_eq!(state.next(&mut second, |_| None), Some(current()));
    assert_eq!(state.groups[&7].normal, [30]);
    assert_eq!(state.next(&mut second, |_| None), None);
    assert_eq!(state.groups[&7].normal, [30, 31]);
}

#[test]
fn last_input_inactive_release_notifies_without_dispatch_and_preserves_latch() {
    let mut state = AnalogState::default();
    let values = BTreeMap::new();
    edge(
        &mut state,
        30,
        false,
        Policy::LastInput,
        Phase::Press,
        &values,
    );
    assert_eq!(
        edge(
            &mut state,
            31,
            false,
            Policy::LastInput,
            Phase::Press,
            &values
        ),
        [replay(30, Phase::Release), AnalogEffect::Pause, current()]
    );
    assert_eq!(
        edge(
            &mut state,
            30,
            false,
            Policy::LastInput,
            Phase::Release,
            &values
        ),
        [AnalogEffect::Current { execute: false }]
    );
    assert_eq!(
        state.key_state(30),
        Some(AnalogKeyState {
            group: 7,
            active: false,
            released_by_arbitration: true,
        })
    );
    assert_eq!(state.groups[&7].normal, [31]);
    assert_eq!(
        edge(
            &mut state,
            31,
            false,
            Policy::LastInput,
            Phase::Release,
            &values
        ),
        [current()]
    );
    assert_eq!(state.key_state(31).unwrap().released_by_arbitration, false);
}

#[test]
fn last_input_release_current_precedes_list_removal_pause_and_restoration() {
    let mut state = AnalogState::default();
    let values = BTreeMap::new();
    edge(
        &mut state,
        30,
        false,
        Policy::LastInput,
        Phase::Press,
        &values,
    );
    edge(
        &mut state,
        31,
        false,
        Policy::LastInput,
        Phase::Press,
        &values,
    );
    let mut transition = state
        .begin(31, 7, false, Policy::LastInput, Phase::Release)
        .unwrap()
        .unwrap();
    assert_eq!(state.next(&mut transition, |_| None), Some(current()));
    assert_eq!(state.groups[&7].normal, [30, 31]);
    assert_eq!(
        state.next(&mut transition, |_| None),
        Some(AnalogEffect::Pause)
    );
    assert_eq!(state.groups[&7].normal, [30]);
    assert_eq!(
        state.next(&mut transition, |_| None),
        Some(replay(30, Phase::Press))
    );
    assert!(!state.key_state(30).unwrap().active);
    assert!(state.replayed(30, Phase::Press));
    assert_eq!(state.key_state(30).unwrap().released_by_arbitration, false);
    assert_eq!(state.next(&mut transition, |_| None), None);
}

#[test]
fn ordinary_make_retains_arbitration_latch_while_replayed_make_clears_it() {
    let mut state = AnalogState::default();
    let values = BTreeMap::new();
    edge(
        &mut state,
        30,
        false,
        Policy::LastInput,
        Phase::Press,
        &values,
    );
    edge(
        &mut state,
        31,
        false,
        Policy::LastInput,
        Phase::Press,
        &values,
    );
    assert!(state.key_state(30).unwrap().released_by_arbitration);
    edge(
        &mut state,
        30,
        false,
        Policy::LastInput,
        Phase::Press,
        &values,
    );
    assert!(state.key_state(30).unwrap().active);
    assert!(state.key_state(30).unwrap().released_by_arbitration);
    assert!(state.replayed(30, Phase::Press));
    assert!(!state.key_state(30).unwrap().released_by_arbitration);
    assert!(!state.replayed(99, Phase::Release));
    assert!(state.key_state(99).is_none());
}

#[test]
fn priority_updates_lists_before_current_and_restores_latest_normal() {
    let mut state = AnalogState::default();
    let values = BTreeMap::new();
    edge(
        &mut state,
        30,
        false,
        Policy::Priority,
        Phase::Press,
        &values,
    );
    let mut transition = state
        .begin(31, 7, true, Policy::Priority, Phase::Press)
        .unwrap()
        .unwrap();
    assert_eq!(
        state.next(&mut transition, |_| None),
        Some(replay(30, Phase::Release))
    );
    state.replayed(30, Phase::Release);
    assert_eq!(
        state.next(&mut transition, |_| None),
        Some(AnalogEffect::Pause)
    );
    assert_eq!(state.next(&mut transition, |_| None), Some(current()));
    assert_eq!(state.groups[&7].priority, [31]);
    assert_eq!(state.next(&mut transition, |_| None), None);
    assert!(
        edge(
            &mut state,
            32,
            false,
            Policy::Priority,
            Phase::Press,
            &values
        )
        .is_empty()
    );
    assert!(!state.key_state(32).unwrap().active);
    let mut release = state
        .begin(31, 7, true, Policy::Priority, Phase::Release)
        .unwrap()
        .unwrap();
    assert_eq!(state.next(&mut release, |_| None), Some(current()));
    assert!(state.groups[&7].priority.is_empty());
    assert_eq!(
        run(&mut state, release, &values),
        [AnalogEffect::Pause, replay(32, Phase::Press)]
    );
}

#[test]
fn missing_key_cleans_normal_lists_and_key_state_but_preserves_priority_list() {
    let mut state = AnalogState::default();
    let values = BTreeMap::new();
    edge(
        &mut state,
        30,
        false,
        Policy::Priority,
        Phase::Press,
        &values,
    );
    edge(
        &mut state,
        31,
        true,
        Policy::Priority,
        Phase::Press,
        &values,
    );
    state.physical_missing(30);
    state.physical_missing(31);
    assert!(state.key_state(30).is_none());
    assert!(state.key_state(31).is_none());
    assert!(state.groups[&7].normal.is_empty());
    assert_eq!(state.groups[&7].priority, [31]);
    assert!(
        edge(
            &mut state,
            32,
            false,
            Policy::Priority,
            Phase::Press,
            &values
        )
        .is_empty()
    );
    assert_eq!(
        edge(
            &mut state,
            31,
            true,
            Policy::Priority,
            Phase::Release,
            &values
        ),
        [current(), AnalogEffect::Pause, replay(32, Phase::Press)]
    );
    assert!(state.groups[&7].priority.is_empty());
    state.clear();
    assert!(state.groups.is_empty());
    assert!(state.keys.is_empty());
}

#[test]
fn compare_make_uses_low_byte_of_physical_levels_and_equal_depth_displaces() {
    let mut state = AnalogState::default();
    let mut values = levels(&[(30, 200), (31, 356)]);
    assert_eq!(
        edge(
            &mut state,
            30,
            false,
            Policy::CompareLevel,
            Phase::Press,
            &values
        ),
        [current()]
    );
    // 356 is compared as 100, not as a larger V2 WORD value.
    assert!(
        edge(
            &mut state,
            31,
            false,
            Policy::CompareLevel,
            Phase::Press,
            &values
        )
        .is_empty()
    );
    assert!(!state.key_state(31).unwrap().active);
    values.insert(31, 456);
    assert_eq!(
        edge(
            &mut state,
            31,
            false,
            Policy::CompareLevel,
            Phase::Press,
            &values
        ),
        [replay(30, Phase::Release), AnalogEffect::Pause, current()]
    );
    assert!(state.key_state(31).unwrap().active);
    assert_eq!(state.groups[&7].normal, [30, 31]);
}

#[test]
fn compare_release_keeps_active_until_missing_and_removes_list_after_replay() {
    let mut state = AnalogState::default();
    let mut values = levels(&[(30, 100), (31, 90)]);
    edge(
        &mut state,
        30,
        false,
        Policy::CompareLevel,
        Phase::Press,
        &values,
    );
    edge(
        &mut state,
        31,
        false,
        Policy::CompareLevel,
        Phase::Press,
        &values,
    );
    values.insert(30, 50);
    let mut transition = state
        .begin(30, 7, false, Policy::CompareLevel, Phase::Release)
        .unwrap()
        .unwrap();
    assert_eq!(
        state.next(&mut transition, |key| values.get(&key).copied()),
        Some(current())
    );
    assert!(state.key_state(30).unwrap().active);
    assert_eq!(state.groups[&7].normal, [30, 31]);
    assert_eq!(
        state.next(&mut transition, |key| values.get(&key).copied()),
        Some(AnalogEffect::Pause)
    );
    assert_eq!(
        state.next(&mut transition, |key| values.get(&key).copied()),
        Some(replay(31, Phase::Press))
    );
    assert_eq!(state.groups[&7].normal, [30, 31]);
    state.replayed(31, Phase::Press);
    assert_eq!(
        state.next(&mut transition, |key| values.get(&key).copied()),
        None
    );
    assert_eq!(state.groups[&7].normal, [31]);
    assert!(state.key_state(30).unwrap().active);
    state.physical_missing(30);
    assert!(state.key_state(30).is_none());
}

#[test]
fn compare_release_reads_competitor_levels_after_current_callback_and_preserves_ties() {
    let mut state = AnalogState::default();
    let mut values = levels(&[(30, 100), (31, 80), (32, 70)]);
    for key in [30, 31, 32] {
        edge(
            &mut state,
            key,
            false,
            Policy::CompareLevel,
            Phase::Press,
            &values,
        );
    }
    values.insert(30, 50);
    let mut transition = state
        .begin(30, 7, false, Policy::CompareLevel, Phase::Release)
        .unwrap()
        .unwrap();
    assert_eq!(
        state.next(&mut transition, |key| values.get(&key).copied()),
        Some(current())
    );
    // The current level was captured on entry; competitors are read afterward.
    values.insert(30, 200);
    values.insert(32, 80);
    assert_eq!(
        run(&mut state, transition, &values),
        [AnalogEffect::Pause, replay(32, Phase::Press)]
    );
}

#[test]
fn continuous_depth_switches_without_current_callback_or_list_changes() {
    let mut state = AnalogState::default();
    let mut values = levels(&[(30, 100), (31, 90)]);
    edge(
        &mut state,
        30,
        false,
        Policy::CompareLevel,
        Phase::Press,
        &values,
    );
    edge(
        &mut state,
        31,
        false,
        Policy::CompareLevel,
        Phase::Press,
        &values,
    );
    values.insert(31, 356); // Equal low-byte depth wins in the increasing branch.
    let increasing = state
        .begin_depth(31, 7, Policy::CompareLevel, Phase::Press)
        .unwrap();
    assert_eq!(
        run(&mut state, increasing, &values),
        [
            replay(30, Phase::Release),
            AnalogEffect::Pause,
            replay(31, Phase::Press)
        ]
    );
    assert_eq!(state.groups[&7].normal, [30, 31]);
    values.insert(31, 80);
    let decreasing = state
        .begin_depth(31, 7, Policy::CompareLevel, Phase::Release)
        .unwrap();
    assert_eq!(
        run(&mut state, decreasing, &values),
        [
            replay(31, Phase::Release),
            AnalogEffect::Pause,
            replay(30, Phase::Press)
        ]
    );
    assert_eq!(state.groups[&7].normal, [30, 31]);
    assert!(state.key_state(30).unwrap().active);
    assert!(state.key_state(31).unwrap().released_by_arbitration);
}

#[test]
fn continuous_depth_release_requires_strictly_deeper_inactive_competitor() {
    let mut state = AnalogState::default();
    let values = levels(&[(30, 100), (31, 90)]);
    edge(
        &mut state,
        30,
        false,
        Policy::CompareLevel,
        Phase::Press,
        &values,
    );
    edge(
        &mut state,
        31,
        false,
        Policy::CompareLevel,
        Phase::Press,
        &values,
    );
    let transition = state
        .begin_depth(30, 7, Policy::CompareLevel, Phase::Release)
        .unwrap();
    assert!(run(&mut state, transition, &levels(&[(30, 90), (31, 90)])).is_empty());
    state.replayed(31, Phase::Press);
    let transition = state
        .begin_depth(30, 7, Policy::CompareLevel, Phase::Release)
        .unwrap();
    assert!(run(&mut state, transition, &levels(&[(30, 80), (31, 100)])).is_empty());
    assert!(
        state
            .begin_depth(30, 7, Policy::LastInput, Phase::Press)
            .is_none()
    );
}

#[test]
fn neutral_analog_waits_after_displacement_and_restores_without_wait_on_release() {
    let mut state = AnalogState::default();
    let values = BTreeMap::new();
    assert_eq!(
        edge(
            &mut state,
            30,
            false,
            Policy::Neutral,
            Phase::Press,
            &values
        ),
        [current()]
    );
    assert_eq!(
        edge(
            &mut state,
            31,
            false,
            Policy::Neutral,
            Phase::Press,
            &values
        ),
        [replay(30, Phase::Release), AnalogEffect::Pause]
    );
    assert!(!state.key_state(30).unwrap().active);
    assert!(!state.key_state(31).unwrap().active);
    assert_eq!(
        edge(
            &mut state,
            31,
            false,
            Policy::Neutral,
            Phase::Release,
            &values
        ),
        [replay(30, Phase::Press)]
    );
    let mut release = state
        .begin(30, 7, false, Policy::Neutral, Phase::Release)
        .unwrap()
        .unwrap();
    assert_eq!(state.next(&mut release, |_| None), Some(current()));
    assert_eq!(state.groups[&7].normal, [30]);
    assert_eq!(state.next(&mut release, |_| None), None);
    assert!(state.groups[&7].normal.is_empty());
}

#[test]
fn unknown_policy_and_instant_edges_do_not_create_analog_state() {
    let state = AnalogState::default();
    assert!(
        state
            .begin(30, 7, false, Policy::Unknown(9), Phase::Press)
            .is_err()
    );
    assert!(
        state
            .begin(30, 7, false, Policy::Neutral, Phase::Instant)
            .unwrap()
            .is_none()
    );
    assert!(state.keys.is_empty());
    assert!(state.groups.is_empty());
}

#[test]
fn missing_physical_level_compares_as_zero_and_zero_key_sentinel_is_preserved() {
    let mut state = AnalogState::default();
    let values = levels(&[(0, 100), (30, 90)]);
    edge(
        &mut state,
        0,
        false,
        Policy::CompareLevel,
        Phase::Press,
        &values,
    );
    edge(
        &mut state,
        30,
        false,
        Policy::CompareLevel,
        Phase::Press,
        &values,
    );
    let transition = state
        .begin_depth(30, 7, Policy::CompareLevel, Phase::Press)
        .unwrap();
    assert!(run(&mut state, transition, &BTreeMap::new()).is_empty());
    // A zero previous key suppresses the synthetic displacement in 9a4ce,
    // but the current make still executes when its low-byte level wins.
    assert_eq!(
        edge(
            &mut state,
            30,
            false,
            Policy::CompareLevel,
            Phase::Press,
            &BTreeMap::new()
        ),
        [current()]
    );
}

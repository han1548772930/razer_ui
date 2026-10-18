use super::*;
use crate::{
    mapping_native_digital::{JoystickGroup, NativeDigitalOutput},
    mapping_native_lookup::{Input, InputKind, Selectors},
};
impl State {
    fn apply_analog(
        &mut self,
        sample: AnalogSample,
        parameters: AnalogParameters,
        pair: &HeldMapping<Output>,
        held: impl Fn(&str) -> Option<HeldMapping<Output>>,
        now: Duration,
    ) -> Result<Vec<AnalogDispatch>> {
        let mut outputs = Vec::new();
        for operation in Self::prepare_analog(sample, parameters, pair) {
            match self.advance_analog(operation)? {
                Some(AnalogStep::Transition(transition)) => outputs.extend(
                    self.dispatch_analog(transition, &held, now)?
                        .into_iter()
                        .map(AnalogDispatch::Action),
                ),
                Some(AnalogStep::Continuous {
                    resolved,
                    actuation,
                    ..
                }) => {
                    outputs.push(AnalogDispatch::Analog {
                        resolved,
                        actuation,
                    });
                }
                _ => {}
            }
        }
        Ok(outputs)
    }

    fn apply(
        &mut self,
        resolved: Resolved<Output>,
        held: impl Fn(&str) -> Option<HeldMapping<Output>>,
    ) -> Result<Vec<Dispatch>> {
        self.apply_at(resolved, held, Duration::ZERO)
    }
}
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

fn modtap(milliseconds: u32, tap: u32, hold: u32) -> Output {
    Output::ModTap {
        milliseconds,
        tap: Box::new(marker(tap)),
        hold: Box::new(marker(hold)),
    }
}

fn analog(key_id: u8, previous: u16, value: u16) -> AnalogSample {
    AnalogSample {
        key_id,
        previous,
        value,
        maximum: 255,
        phase: if value >= previous {
            Phase::Press
        } else {
            Phase::Release
        },
    }
}

fn analog_parameters(actuation: u16) -> AnalogParameters {
    AnalogParameters {
        actuation,
        rapid: None,
        default_continuous: false,
    }
}
fn analog_flags(dispatches: Vec<AnalogDispatch>) -> Vec<u32> {
    dispatches
        .iter()
        .map(|dispatch| match dispatch {
            AnalogDispatch::Action(dispatch) => flag(dispatch),
            _ => panic!("Expected gated action"),
        })
        .collect()
}

#[test]
fn analog_dispatch_keeps_release_until_crossing_and_cleans_only_made_outputs() {
    let mut state = State::default();
    let retained = pair(30, marker(11), marker(12));
    assert!(
        state
            .apply_analog(
                analog(1, 0, 99),
                analog_parameters(100),
                &retained,
                |_| None,
                Duration::ZERO
            )
            .unwrap()
            .is_empty()
    );
    assert!(
        state
            .drain_analog_dispatches(|_| None, Duration::ZERO)
            .unwrap()
            .is_empty()
    );
    let made = state
        .apply_analog(
            analog(1, 99, 100),
            analog_parameters(100),
            &retained,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(analog_flags(made), [11]);
    let changed = pair(30, marker(21), marker(22));
    let released = state
        .apply_analog(
            analog(1, 100, 99),
            analog_parameters(100),
            &changed,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(analog_flags(released), [12]);
    assert!(
        state
            .drain_analog_dispatches(|_| None, Duration::ZERO)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn analog_trigger_dispatches_make_in_id_order_and_release_in_reverse_order() {
    use crate::mapping_native_action::Trigger;
    let make = Output::Triggers(BTreeMap::from([
        (
            1,
            Trigger {
                actuation: 50,
                output: marker(1),
            },
        ),
        (
            2,
            Trigger {
                actuation: 100,
                output: marker(2),
            },
        ),
    ]));
    let release = Output::Triggers(BTreeMap::from([
        (
            1,
            Trigger {
                actuation: 50,
                output: marker(11),
            },
        ),
        (
            2,
            Trigger {
                actuation: 100,
                output: marker(12),
            },
        ),
    ]));
    let retained = pair(30, make, release);
    let mut state = State::default();
    let make = state
        .apply_analog(
            analog(1, 0, 120),
            analog_parameters(1),
            &retained,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(analog_flags(make), [1, 2]);
    let release = state
        .apply_analog(
            analog(1, 120, 0),
            analog_parameters(1),
            &retained,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(analog_flags(release), [12, 11]);
}

#[test]
fn analog_opposite_edges_can_have_different_wrapper_shapes() {
    use crate::mapping_native_action::Trigger;
    let retained = pair(
        30,
        Output::Triggers(BTreeMap::from([(
            1,
            Trigger {
                actuation: 100,
                output: marker(1),
            },
        )])),
        marker(2),
    );
    let mut state = State::default();
    let make = state
        .apply_analog(
            analog(1, 0, 120),
            analog_parameters(100),
            &retained,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(analog_flags(make), [1]);
    let release = state
        .apply_analog(
            analog(1, 120, 0),
            analog_parameters(100),
            &retained,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(analog_flags(release), [2]);
}

#[test]
fn analog_cascade_ignores_digital_held_conditions_and_uses_analog_gate_activity() {
    let condition = pair(30, marker(1), marker(2));
    let cascade = pair(
        31,
        Output::Cascade(branches(Some(30), marker(11), marker(12))),
        Output::Cascade(branches(Some(30), marker(21), marker(22))),
    );
    let mut state = State::default();
    let output = state
        .apply_analog(
            analog(2, 0, 120),
            analog_parameters(100),
            &cascade,
            |_| Some(condition.clone()),
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(analog_flags(output), [12]);
    state
        .apply_analog(
            analog(2, 120, 0),
            analog_parameters(100),
            &cascade,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    state
        .apply_analog(
            analog(1, 0, 120),
            analog_parameters(100),
            &condition,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    let output = state
        .apply_analog(
            analog(2, 0, 120),
            analog_parameters(100),
            &cascade,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(analog_flags(output), [11]);
    state
        .apply_analog(
            analog(1, 120, 0),
            analog_parameters(100),
            &condition,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    let output = state
        .apply_analog(
            analog(2, 120, 0),
            analog_parameters(100),
            &cascade,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(analog_flags(output), [21]);
}

#[test]
fn one_trigger_break_removes_analog_cascade_condition_even_with_another_trigger_held() {
    use crate::mapping_native_action::Trigger;
    let trigger = |flag| {
        Output::Triggers(BTreeMap::from([
            (
                1,
                Trigger {
                    actuation: 50,
                    output: marker(flag),
                },
            ),
            (
                2,
                Trigger {
                    actuation: 100,
                    output: marker(flag + 1),
                },
            ),
        ]))
    };
    let condition = pair(30, trigger(1), trigger(3));
    let cascade = pair(
        31,
        Output::Cascade(branches(Some(30), marker(11), marker(12))),
        Output::Cascade(branches(Some(30), marker(21), marker(22))),
    );
    let mut state = State::default();
    state
        .apply_analog(
            analog(1, 0, 120),
            analog_parameters(100),
            &condition,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    state
        .apply_analog(
            analog(1, 120, 90),
            analog_parameters(100),
            &condition,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    let output = state
        .apply_analog(
            analog(2, 0, 120),
            analog_parameters(100),
            &cascade,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(analog_flags(output), [12]);
}

#[test]
fn rapid_mapping_continuous_override_controls_rearming_below_actuation() {
    for continuous in [None, Some(false), Some(true)] {
        let wrap = |flag| Output::RapidTrigger {
            actuation: 100,
            make_sensitivity: 10,
            break_sensitivity: 20,
            continuous,
            mapping: Box::new(marker(flag)),
        };
        let pair = pair(30, wrap(1), wrap(2));
        let mut state = State::default();
        let parameters = AnalogParameters {
            default_continuous: true,
            ..analog_parameters(200)
        };
        for (previous, value) in [(0, 100), (100, 80), (80, 60)] {
            state
                .apply_analog(
                    analog(1, previous, value),
                    parameters,
                    &pair,
                    |_| None,
                    Duration::ZERO,
                )
                .unwrap();
        }
        let result = state
            .apply_analog(
                analog(1, 60, 70),
                parameters,
                &pair,
                |_| None,
                Duration::ZERO,
            )
            .unwrap();
        assert_eq!(!result.is_empty(), continuous != Some(false));
    }
}

#[test]
fn double_mapping_preserves_raw_level_and_orders_analog_before_digital() {
    use crate::mapping_native_analog::NativeAnalogOutput;
    let analog = NativeAnalogOutput::from_native_json(&serde_json::json!({
        "type":"analogJoystickV2", "id":"axes", "actuations":{"65535":[127,0,0]}
    }))
    .unwrap();
    let wrap = |flag| Output::DoubleMapping {
        analog: analog.clone(),
        digital: Box::new(marker(flag)),
    };
    let pair = pair(30, wrap(1), wrap(2));
    let mut state = State::default();
    let output = state
        .apply_analog(
            AnalogSample {
                maximum: 65535,
                ..self::analog(1, 0, 4095)
            },
            analog_parameters(1000),
            &pair,
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert!(matches!(
        &output[0],
        AnalogDispatch::Analog {
            actuation: 4095,
            ..
        }
    ));
    assert!(matches!(&output[1], AnalogDispatch::Action(dispatch) if flag(dispatch) == 1));
    let cleanup = state
        .drain_analog_dispatches(|_| None, Duration::ZERO)
        .unwrap();
    assert_eq!(cleanup.len(), 2);
    assert!(matches!(&cleanup[0], AnalogDispatch::Action(dispatch) if flag(dispatch) == 2));
    assert!(matches!(
        &cleanup[1],
        AnalogDispatch::Analog { actuation: 0, .. }
    ));
}

#[test]
fn analog_modtap_uses_fixed_break_threshold_with_descriptor_rapid_enabled() {
    let pair = pair(30, modtap(100, 11, 12), modtap(100, 21, 22));
    let mut state = State::default();
    let parameters = AnalogParameters {
        rapid: Some(RapidParameters {
            actuation: 100,
            make_sensitivity: 1,
            break_sensitivity: 1,
            continuous: true,
        }),
        ..analog_parameters(100)
    };
    assert!(
        state
            .apply_analog(
                analog(1, 0, 150),
                parameters,
                &pair,
                |_| None,
                Duration::ZERO
            )
            .unwrap()
            .is_empty()
    );
    assert!(
        state
            .apply_analog(
                analog(1, 150, 149),
                parameters,
                &pair,
                |_| None,
                Duration::from_millis(1)
            )
            .unwrap()
            .is_empty()
    );
    let release = state
        .apply_analog(
            analog(1, 149, 99),
            parameters,
            &pair,
            |_| None,
            Duration::from_millis(2),
        )
        .unwrap();
    assert_eq!(analog_flags(release), [11, 21]);
}

#[test]
fn rapid_trigger_tracks_extremes_and_retriggers_with_source_sensitivities() {
    let mut state = State::default();
    let parameters = RapidParameters {
        actuation: 100,
        make_sensitivity: 10,
        break_sensitivity: 20,
        continuous: false,
    };
    for (previous, value, expected) in [
        (0, 99, false),
        (99, 100, true),
        (100, 150, false),
        (150, 131, false),
        (131, 130, true),
        (130, 135, false),
        (135, 140, true),
        (140, 120, true),
        (120, 90, false),
        (90, 99, false),
        (99, 100, true),
        (100, 100, false),
    ] {
        assert_eq!(
            state.rapid_transition(analog(1, previous, value), parameters),
            expected,
            "{previous}->{value}"
        );
    }
}

#[test]
fn rapid_continuous_rearms_below_actuation_and_zero_forces_release() {
    let mut state = State::default();
    let parameters = RapidParameters {
        actuation: 100,
        make_sensitivity: 10,
        break_sensitivity: 20,
        continuous: true,
    };
    for (previous, value, expected) in [
        (0, 100, true),
        (100, 80, true),
        (80, 20, false),
        (20, 29, false),
        (29, 30, true),
        (30, 15, false),
        (15, 0, true),
        (0, 1, false),
        (1, 10, true),
    ] {
        assert_eq!(
            state.rapid_transition(analog(1, previous, value), parameters),
            expected,
            "{previous}->{value}"
        );
    }
    let parameters = RapidParameters {
        break_sensitivity: 200,
        ..parameters
    };
    assert!(state.rapid_transition(analog(1, 10, 0), parameters));
    assert!(!state.rapid_transition(analog(1, 0, 10), parameters));
    assert!(state.rapid_transition(analog(1, 10, 100), parameters));
    state.clear();
    assert!(!state.rapid_transition(analog(1, 100, 0), parameters));
}

#[test]
fn trigger_crossings_require_retained_make_and_deduplicate_each_key_and_trigger() {
    let mut state = State::default();
    assert!(!state.trigger_transition(analog(1, 0, 100), 1, 100, false));
    assert!(state.trigger_transition(analog(1, 0, 100), 1, 100, true));
    assert!(!state.trigger_transition(analog(1, 100, 101), 1, 100, true));
    assert!(state.trigger_transition(analog(1, 0, 100), 2, 100, true));
    assert!(state.trigger_transition(analog(2, 0, 100), 1, 100, true));
    assert!(!state.trigger_transition(analog(1, 150, 140), 1, 100, false));
    assert!(state.trigger_transition(analog(1, 140, 100), 1, 100, false));
    assert!(!state.trigger_transition(analog(1, 100, 99), 1, 100, false));
    assert!(state.trigger_transition(analog(1, 99, 0), 2, 100, false));
    state.clear();
    assert!(!state.trigger_transition(analog(2, 100, 0), 1, 100, false));
}

#[test]
fn trigger_zero_and_maximum_thresholds_keep_inclusive_native_boundaries() {
    let mut state = State::default();
    assert!(state.trigger_transition(analog(1, 0, 1), 1, 0, true));
    assert!(!state.trigger_transition(analog(1, 2, 1), 1, 0, true));
    assert!(state.trigger_transition(analog(1, 1, 0), 1, 0, true));
    assert!(state.trigger_transition(analog(1, 254, 255), 1, 255, true));
    assert!(state.trigger_transition(analog(1, 255, 254), 1, 255, true));
    let sample = AnalogSample {
        maximum: 4095,
        ..analog(2, 4094, 4095)
    };
    assert!(state.trigger_transition(sample, 1, 4095, true));
    assert!(state.trigger_transition(
        AnalogSample {
            previous: 4095,
            value: 4094,
            phase: Phase::Release,
            ..sample
        },
        1,
        4095,
        true
    ));
}

#[test]
fn modtap_short_press_emits_stored_make_tap_then_retained_break_tap() {
    let mut state = State::default();
    let make = resolved(input(30), modtap(100, 1, 2));
    assert!(
        state
            .apply_at(make, |_| None, Duration::ZERO)
            .unwrap()
            .is_empty()
    );
    let selected = state
        .apply_at(
            resolved(input(30).released(), modtap(100, 3, 4)),
            |_| None,
            Duration::from_millis(99),
        )
        .unwrap();
    assert_eq!(selected.iter().map(flag).collect::<Vec<_>>(), [1, 3]);
    assert_eq!(selected[0].resolved.input.phase(), Phase::Press);
    assert_eq!(selected[1].resolved.input.phase(), Phase::Release);
    assert!(state.poll(Duration::from_millis(101)).is_empty());
}

#[test]
fn modtap_hold_deadline_and_release_preserve_independent_mapping_branches() {
    let mut state = State::default();
    state
        .apply_at(
            resolved(input(30), modtap(100, 1, 2)),
            |_| None,
            Duration::from_millis(5),
        )
        .unwrap();
    assert!(state.poll(Duration::from_millis(104)).is_empty());
    let selected = state.poll(Duration::from_millis(105));
    assert_eq!(selected.iter().map(flag).collect::<Vec<_>>(), [2]);
    assert_eq!(selected[0].resolved.input.phase(), Phase::Press);
    assert!(state.poll(Duration::from_millis(110)).is_empty());
    let selected = state
        .apply_at(
            resolved(input(30).released(), modtap(100, 3, 4)),
            |_| None,
            Duration::from_millis(120),
        )
        .unwrap();
    assert_eq!(selected.iter().map(flag).collect::<Vec<_>>(), [4]);
    assert_eq!(selected[0].resolved.input.phase(), Phase::Release);
}

#[test]
fn modtap_duplicate_cannot_replace_deadline_and_clear_rejects_old_work() {
    let mut state = State::default();
    state
        .apply_at(
            resolved(input(30), modtap(100, 1, 2)),
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert!(
        state
            .apply_at(
                resolved(input(30), modtap(200, 3, 4)),
                |_| None,
                Duration::ZERO
            )
            .is_err()
    );
    assert_eq!(
        state
            .poll(Duration::from_millis(100))
            .iter()
            .map(flag)
            .collect::<Vec<_>>(),
        [2]
    );
    state
        .apply_at(
            resolved(input(30), modtap(100, 5, 6)),
            |_| None,
            Duration::from_millis(100),
        )
        .unwrap();
    state.clear();
    state
        .apply_at(
            resolved(input(30), modtap(100, 7, 8)),
            |_| None,
            Duration::from_millis(150),
        )
        .unwrap();
    assert!(state.poll(Duration::from_millis(200)).is_empty());
    assert_eq!(
        state
            .poll(Duration::from_millis(250))
            .iter()
            .map(flag)
            .collect::<Vec<_>>(),
        [8]
    );
}

#[test]
fn modtap_orders_due_timers_and_preserves_device_queue_order_at_exact_deadline() {
    let mut state = State::default();
    for (key, delay) in [(30, 100), (31, 50), (32, 50)] {
        state
            .apply_at(
                resolved(input(key), modtap(delay, key.into(), key.into())),
                |_| None,
                Duration::ZERO,
            )
            .unwrap();
    }
    assert_eq!(
        state
            .poll(Duration::from_millis(100))
            .iter()
            .map(flag)
            .collect::<Vec<_>>(),
        [31, 32, 30]
    );
    state
        .apply_at(
            resolved(input(33), modtap(0, 7, 8)),
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    let selected = state
        .apply_at(
            resolved(input(33).released(), modtap(0, 9, 10)),
            |_| None,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(selected.iter().map(flag).collect::<Vec<_>>(), [7, 9]);
    assert!(state.poll(Duration::ZERO).is_empty());
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

#[test]
fn dpad_missing_previous_mapping_retains_error_and_dispatches_new_output() {
    let mut state = State::default();
    let first = Output::Dpad(branches(None, marker(1), marker(2)));
    state.apply(resolved(input(30), first), |_| None).unwrap();
    let next = Output::Dpad(branches(None, marker(3), marker(4)));
    let selected = state.apply(resolved(input(31), next), |_| None).unwrap();
    assert_eq!(selected.iter().map(flag).collect::<Vec<_>>(), [3]);
    assert_eq!(
        state.take_diagnostics(),
        ["D-pad previous input has no retained mapping"]
    );
    assert!(state.take_diagnostics().is_empty());
}

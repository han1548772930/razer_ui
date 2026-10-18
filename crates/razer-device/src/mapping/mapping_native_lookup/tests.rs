use super::*;
use serde_json::json;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Output {
    Macro {
        guid: &'static str,
        flag: u8,
        repeat: u32,
    },
    Key(u16),
    Disabled,
}

fn modes() -> Modes {
    Modes {
        mapping_enabled: true,
        hypershift: false,
        gaming: false,
        otfs: false,
        global_shortcuts_enabled: true,
    }
}
fn key(flag: u16) -> Input {
    Input::new(InputKind::Keyboard { scancode: 30, flag })
}
fn row(flag: u16, family: Family, output: Output) -> CompiledMapping<Output> {
    let selectors = match family {
        Family::Normal => Selectors::default(),
        Family::Gaming => Selectors {
            gaming: true,
            ..Default::default()
        },
        Family::Otfs => Selectors {
            otfs: true,
            ..Default::default()
        },
        Family::DynamicKey => Selectors {
            dynamic_key: Some(3),
            ..Default::default()
        },
        Family::SnapTap => Selectors {
            snap_tap: Some(7),
            ..Default::default()
        },
    };
    CompiledMapping {
        input: key(flag),
        selectors,
        output,
    }
}
fn tables(output: u16) -> Tables<Output> {
    Tables::new(
        vec![],
        vec![
            row(0, Family::Normal, Output::Key(output)),
            row(1, Family::Normal, Output::Key(output + 1)),
        ],
        vec![],
    )
    .unwrap()
}
fn mapped(value: Option<Resolved<Output>>) -> Output {
    match value.unwrap().output {
        ResolvedOutput::Mapped(output) => output,
        _ => panic!("Expected mapping"),
    }
}

#[test]
fn analog_pair_survives_threshold_edges_and_profile_replacement_until_zero() {
    let mut lookup = Lookup::new(tables(50));
    let first = lookup.analog_mapping(&key(0), modes(), true).unwrap();
    assert_eq!(mapped(Some(first.make)), Output::Key(50));
    lookup.replace_tables(tables(80));
    let falling = lookup.analog_mapping(&key(1), modes(), true).unwrap();
    assert_eq!(mapped(Some(falling.release)), Output::Key(51));
    lookup.release_analog_mapping(&key(0).identity());
    let next = lookup.analog_mapping(&key(0), modes(), true).unwrap();
    assert_eq!(mapped(Some(next.make)), Output::Key(80));
    assert_eq!(lookup.held_count(), 0);
}

#[test]
fn analog_key_passthrough_changes_output_kind_without_changing_lookup_identity() {
    let input = Input::new(InputKind::AnalogKey {
        scancode: 30,
        flag: 0,
    });
    let mut lookup = Lookup::new(Tables::<Output>::new(vec![], vec![], vec![]).unwrap());
    let pair = lookup.analog_mapping(&input, modes(), true).unwrap();
    assert_eq!(pair.make.input, input);
    assert!(matches!(
        pair.make.output,
        ResolvedOutput::Passthrough(Input {
            kind: InputKind::Keyboard {
                scancode: 30,
                flag: 0
            },
            ..
        })
    ));
    assert!(matches!(
        pair.release.output,
        ResolvedOutput::Passthrough(Input {
            kind: InputKind::Keyboard {
                scancode: 30,
                flag: 1
            },
            ..
        })
    ));
}

#[test]
fn interrupt_registration_distinguishes_absent_and_empty_lists() {
    let absent = parse_input(&json!({"type":"analogKey","scancode":30,"flag":0})).unwrap();
    let empty =
        parse_input(&json!({"type":"analogKey","scancode":31,"flag":0,"interrupts":[]})).unwrap();
    assert!(!absent.interruptible);
    assert!(empty.interruptible);
    let mut interrupts = AnalogInterrupts::default();
    interrupts.register(1, &absent);
    interrupts.register(2, &empty);
    let other = Input::new(InputKind::Keyboard {
        scancode: 40,
        flag: 0,
    });
    assert_eq!(interrupts.notify(&other), [2]);
    assert!(interrupts.blocked_keys().is_empty());
}

#[test]
fn implicit_interrupt_ignores_own_keyboard_and_break_but_accepts_instant_events() {
    let mut input = Input::new(InputKind::AnalogKey {
        scancode: 30,
        flag: 2,
    });
    input.interruptible = true;
    let mut interrupts = AnalogInterrupts::default();
    interrupts.register(1, &input);
    assert!(interrupts.notify(&input).is_empty());
    assert!(
        interrupts
            .notify(&Input::new(InputKind::Keyboard {
                scancode: 30,
                flag: 2
            }))
            .is_empty()
    );
    assert!(
        interrupts
            .notify(&Input::new(InputKind::Keyboard {
                scancode: 40,
                flag: 1
            }))
            .is_empty()
    );
    let instant = Input::new(InputKind::ControlPodDial { clockwise: true });
    assert_eq!(interrupts.notify(&instant), [1]);
    interrupts.block(1);
    // 8e394 has no duplicate-block check while the registration is present.
    assert_eq!(interrupts.notify(&instant), [1]);
    interrupts.remove_missing(&BTreeSet::from([1]));
    assert!(interrupts.blocked_keys().contains(&1));
    interrupts.remove_missing(&BTreeSet::new());
    assert!(interrupts.blocked_keys().is_empty());
    assert_eq!(interrupts.notify(&instant), [1]);
    assert!(interrupts.blocked_keys().is_empty());
    interrupts.block(1);
    interrupts.clear_registrations();
    assert!(interrupts.notify(&instant).is_empty());
    assert!(interrupts.blocked_keys().contains(&1));
}

#[test]
fn explicit_interrupt_list_matches_stateless_identity_and_overrides_same_key_exception() {
    let input = parse_input(&json!({"type":"analogKey","scancode":30,"flag":0,
        "interrupts":[{"type":"keyboard","scancode":30,"flag":0,"modifiers":9,
            "interrupts":[{"type":"keyboard","scancode":40,"flag":0}]}]
    }))
    .unwrap();
    assert!(!input.interrupts[0].interruptible);
    let mut interrupts = AnalogInterrupts::default();
    interrupts.register(1, &input);
    assert!(
        interrupts
            .notify(&Input::new(InputKind::Keyboard {
                scancode: 40,
                flag: 0
            }))
            .is_empty()
    );
    assert_eq!(
        interrupts.notify(&Input::new(InputKind::Keyboard {
            scancode: 30,
            flag: 0
        })),
        [1]
    );
}

#[test]
fn analog_global_overrides_device_and_dynamic_key_is_not_an_analog_family() {
    let mut lookup = Lookup::new(
        Tables::new(
            vec![],
            vec![
                row(0, Family::DynamicKey, Output::Key(1)),
                row(1, Family::DynamicKey, Output::Key(2)),
                row(0, Family::Gaming, Output::Key(3)),
                row(1, Family::Gaming, Output::Key(4)),
            ],
            vec![
                row(0, Family::Normal, Output::Key(5)),
                row(1, Family::Normal, Output::Key(6)),
            ],
        )
        .unwrap(),
    );
    let pair = lookup
        .analog_mapping(
            &key(0),
            Modes {
                gaming: true,
                ..modes()
            },
            true,
        )
        .unwrap();
    assert_eq!(mapped(Some(pair.make)), Output::Key(5));
    assert_eq!(mapped(Some(pair.release)), Output::Key(6));
    lookup.clear_analog_mappings();
    assert!(
        lookup
            .analog_mapping(
                &key(0),
                Modes {
                    global_shortcuts_enabled: false,
                    ..modes()
                },
                false
            )
            .is_none()
    );
}

#[test]
fn lookup_retains_release_across_profile_and_layer_change() {
    let mut lookup = Lookup::new(tables(50));
    assert_eq!(mapped(lookup.resolve(key(0), modes())), Output::Key(50));
    lookup.replace_tables(tables(80));
    assert!(lookup.resolve(key(0), modes()).is_none());
    let changed = Modes {
        hypershift: true,
        ..modes()
    };
    assert_eq!(mapped(lookup.resolve(key(1), changed)), Output::Key(51));
    assert!(lookup.resolve(key(1), changed).is_none());
    assert_eq!(mapped(lookup.resolve(key(0), modes())), Output::Key(80));
}

#[test]
fn lookup_merges_defaults_and_active_and_keeps_disabled_defaults() {
    let default = vec![
        row(0, Family::Normal, Output::Key(10)),
        row(1, Family::Normal, Output::Key(11)),
    ];
    let active = vec![row(0, Family::Normal, Output::Key(20))];
    let tables = Tables::new(default, active, vec![]).unwrap();
    let mut lookup = Lookup::new(tables);
    assert_eq!(mapped(lookup.resolve(key(0), modes())), Output::Key(20));
    assert_eq!(mapped(lookup.resolve(key(1), modes())), Output::Key(11));
    assert_eq!(
        mapped(lookup.resolve(
            key(0),
            Modes {
                mapping_enabled: false,
                ..modes()
            }
        )),
        Output::Key(10)
    );
}

#[test]
fn lookup_source_family_and_global_priority_is_preserved() {
    let active = vec![
        row(0, Family::Gaming, Output::Key(1)),
        row(0, Family::Otfs, Output::Key(2)),
        row(0, Family::DynamicKey, Output::Key(3)),
        row(1, Family::DynamicKey, Output::Key(4)),
        row(0, Family::SnapTap, Output::Key(5)),
        row(0, Family::Normal, Output::Key(6)),
    ];
    let tables = Tables::new(vec![], active, vec![row(0, Family::Normal, Output::Key(7))]).unwrap();
    for (gaming, otfs, expected) in [(true, true, 1), (false, true, 2), (false, false, 3)] {
        assert_eq!(
            mapped(Lookup::new(tables.clone()).resolve(
                key(0),
                Modes {
                    gaming,
                    otfs,
                    ..modes()
                }
            )),
            Output::Key(expected)
        );
    }
    let tables = Tables::new(
        vec![],
        vec![row(0, Family::Normal, Output::Key(6))],
        vec![row(0, Family::Normal, Output::Key(7))],
    )
    .unwrap();
    let resolved = Lookup::new(tables).resolve(key(0), modes()).unwrap();
    assert!(resolved.global_shortcut);
    assert_eq!(resolved.output, ResolvedOutput::Mapped(Output::Key(7)));
}

#[test]
fn lookup_global_release_never_falls_through_to_device_table() {
    let tables = Tables::new(
        vec![],
        vec![row(1, Family::Normal, Output::Key(70))],
        vec![row(0, Family::Normal, Output::Key(7))],
    )
    .unwrap();
    let mut lookup = Lookup::new(tables);
    lookup.resolve(key(0), modes());
    let release = lookup.resolve(key(1), modes()).unwrap();
    assert!(release.global_shortcut);
    assert!(release.is_passthrough());
}

#[test]
fn lookup_hypershift_exit_retains_held_pairs_until_suppressed_physical_release() {
    let mut lookup = Lookup::new(tables(50));
    lookup.resolve(key(0), modes());
    let released = lookup.hypershift_disabled();
    assert_eq!(released.len(), 1);
    assert_eq!(released[0].output, ResolvedOutput::Mapped(Output::Key(51)));
    assert_eq!(lookup.held_count(), 1);
    assert!(lookup.held_by_stateless(&key(0).stateless_id()).is_some());
    // 6ae30 retains the pair, including for another mode notification.
    assert_eq!(lookup.hypershift_disabled().len(), 1);
    assert!(lookup.resolve(key(0), modes()).is_none());
    assert!(lookup.resolve(key(1), modes()).is_none());
    assert_eq!(lookup.held_count(), 0);
    assert!(lookup.hypershift_disabled().is_empty());
    assert!(lookup.resolve(key(1), modes()).is_none());
    assert!(lookup.resolve(key(0), modes()).is_some());
}

#[test]
fn lookup_shutdown_drains_hypershift_suppressed_pairs() {
    let mut lookup = Lookup::new(tables(50));
    lookup.resolve(key(0), modes());
    lookup.hypershift_disabled();
    let releases = lookup.drain_releases();
    assert_eq!(releases.len(), 1);
    assert_eq!(releases[0].output, ResolvedOutput::Mapped(Output::Key(51)));
    assert_eq!(lookup.held_count(), 0);
    assert!(lookup.drain_releases().is_empty());
    assert!(lookup.resolve(key(0), modes()).is_some());
}

#[test]
fn lookup_retains_complete_macro_stop_identity_and_drains_once() {
    let start = Output::Macro {
        guid: "shared",
        flag: 2,
        repeat: 7,
    };
    let stop = Output::Macro {
        guid: "shared",
        flag: 1,
        repeat: 1,
    };
    let mut lookup = Lookup::new(
        Tables::new(
            vec![],
            vec![
                row(0, Family::Normal, start.clone()),
                row(1, Family::Normal, stop.clone()),
            ],
            vec![],
        )
        .unwrap(),
    );
    assert_eq!(mapped(lookup.resolve(key(0), modes())), start);
    let release = lookup.drain_releases();
    assert_eq!(release.len(), 1);
    assert_eq!(release[0].output, ResolvedOutput::Mapped(stop));
    assert!(lookup.drain_releases().is_empty());
}

#[test]
fn lookup_modifiers_are_global_only_and_held_identity_ignores_them() {
    let mut global = row(0, Family::Normal, Output::Key(10));
    global.input.modifiers = 2;
    let mut lookup = Lookup::new(
        Tables::new(
            vec![],
            vec![row(0, Family::Normal, Output::Key(20))],
            vec![global],
        )
        .unwrap(),
    );
    let mut input = key(0);
    input.modifiers = 2;
    assert_eq!(mapped(lookup.resolve(input, modes())), Output::Key(10));
    assert!(lookup.resolve(key(0), modes()).is_none());
    assert!(lookup.resolve(key(1), modes()).is_some());
    assert_eq!(mapped(lookup.resolve(key(0), modes())), Output::Key(20));
}

#[test]
fn lookup_mouse_identity_unifies_edges_but_preserves_wheel_direction_and_unit() {
    let down = parse_input(&json!({"type":"mouse","id":0,"data":1,"unitId":9})).unwrap();
    let up = parse_input(&json!({"type":"mouse","id":0,"data":2,"unitId":9})).unwrap();
    assert_eq!(down.identity(), up.identity());
    assert_ne!(down.lookup_identity(), up.lookup_identity());
    assert_eq!(down.released(), up);
    let wheel = parse_input(&json!({"type":"mouse","id":1024,"data":65416})).unwrap();
    assert_eq!(wheel.phase(), Phase::Instant);
    assert_eq!(wheel.released(), wheel);
    assert!(parse_input(&json!({"type":"mouse","id":3})).is_err());
}

#[test]
fn lookup_publication_validates_inputs_and_collects_interrupt_hooks() {
    let document = json!({"activeProfile":"a","profiles":[{"guid":"a","appEngine":{"mappings":[
        {"input":{"type":"keyboard","scancode":30,"flag":0,"interrupts":[{"type":"keyboard","scancode":42,"flag":0}]},"output":{"type":"disabled"}}
    ]}}],"defaultMappings":{"appEngine":{"mappings":[]}}});
    let table = from_device_document(&document, None, |_| Ok(Output::Disabled)).unwrap();
    assert_eq!(table.hook_inputs().len(), 2);
    let broken = vec![row(0, Family::DynamicKey, Output::Disabled)];
    assert!(Tables::new(vec![], broken, vec![]).is_err());
    assert!(parse_input(&json!({"type":"keyboard","flag":0})).is_err());
    assert!(
        parse_input(&json!({"type":"keyboard","scancode":30,"flag":0,"modifiers":"2"})).is_err()
    );
}

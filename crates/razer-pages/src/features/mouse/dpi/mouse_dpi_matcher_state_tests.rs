// Pure state projections: no middleware, device, application or vendor code.
fn library(values: Vec<Value>) -> Library {
    Library::from_source_file(&json!({"profiles": values})).unwrap()
}

fn stages(values: Vec<Value>) -> Vec<SourceStage> {
    values
        .iter()
        .map(|value| SourceStage::from_source_file(value).unwrap())
        .collect()
}

#[::core::prelude::v1::test]
fn matcher_initial_none_profile_is_a_local_uuid_placeholder_and_not_observed_dpi() {
    let state = MatcherState::default();
    let placeholder = &state.library.profiles()[0];
    let guid = uuid::Uuid::parse_str(placeholder.guid()).unwrap();
    assert_eq!(guid.get_version_num(), 4);
    assert_eq!(placeholder.name().text(), Some("None"));
    assert_eq!(placeholder.file_value().get("dpi"), Some(&Value::Null));
    // AS adds its disabled localized sentinel when selected GUID is empty.
    let (items, selected) = state.dataset("Localized None");
    assert_eq!(selected, 0);
    assert_eq!(items.len(), 2);
    assert!(items[0].disabled());
    assert_eq!(items[1].guid(), placeholder.guid());
}

#[::core::prelude::v1::test]
fn matcher_library_observations_preserve_extensions_while_mounted_and_unmounted() {
    let observed = json!({
        "libraryExtension":{"version":2,"optional":null},
        "profiles":[{
            "guid":"real",
            "name":"Observed",
            "profileExtension":["keep",3],
            "dpi":{"x":900,"y":800,"dpiExtension":{"value":"keep"}}
        }]
    });
    for mounted in [false, true] {
        let mut state = MatcherState::default();
        let library = Library::from_source_file(&observed).unwrap();
        let observation = MatcherObservation::Profiles {
            library: library.clone(),
            selected: "real".into(),
        };
        let actions = if mounted {
            state.mount();
            state.observe(observation)
        } else {
            state.observe_unmounted(observation)
        };
        assert!(actions.is_empty());
        assert_eq!(state.library(), &library);
        assert_eq!(state.profile_update().library().file_value(), observed);
        assert_eq!(
            state.profile_payload(),
            json!({
                "sensitivityMatcher":observed,
                "selectedSensitivityMatcher":"real"
            })
        );
        state.unmount();
        assert_eq!(state.library().file_value(), observed);
    }
}

#[::core::prelude::v1::test]
fn matcher_selection_removes_none_without_discarding_library_extensions() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        library: Library::from_source_file(&json!({
            "libraryExtension":{"value":"keep"},
            "profiles":[
                {"guid":"none","name":"None"},
                {
                    "guid":"real",
                    "name":"Observed",
                    "profileExtension":null,
                    "dpi":{"x":900,"y":800,"dpiExtension":[1,2]}
                }
            ]
        }))
        .unwrap(),
        selected: "none".into(),
    });
    state
        .apply_local_action(&MatcherAction::SelectProfile("real".into()))
        .unwrap();
    let expected = json!({
        "libraryExtension":{"value":"keep"},
        "profiles":[{
            "guid":"real",
            "name":"Observed",
            "profileExtension":null,
            "dpi":{"x":900,"y":800,"dpiExtension":[1,2]}
        }]
    });
    assert_eq!(state.library().file_value(), expected);
    assert_eq!(
        state.profile_payload(),
        json!({
            "sensitivityMatcher":expected,
            "selectedSensitivityMatcher":"real"
        })
    );
}

#[::core::prelude::v1::test]
fn matcher_rename_and_sort_preserve_library_profile_and_dpi_extensions() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        library: Library::from_source_file(&json!({
            "libraryExtension":{"value":"keep"},
            "profiles":[
                {"guid":"other","name":"Beta","profileExtension":[false,null]},
                {
                    "guid":"selected",
                    "name":"Zulu",
                    "profileExtension":{"value":"keep"},
                    "dpi":{"x":900,"y":800,"dpiExtension":null}
                }
            ]
        }))
        .unwrap(),
        selected: "selected".into(),
    });
    state
        .apply_local_action(&MatcherAction::RenameProfile("Alpha".into()))
        .unwrap();
    let expected = json!({
        "libraryExtension":{"value":"keep"},
        "profiles":[
            {
                "guid":"selected",
                "name":"Alpha",
                "profileExtension":{"value":"keep"},
                "dpi":{"x":900,"y":800,"dpiExtension":null}
            },
            {"guid":"other","name":"Beta","profileExtension":[false,null]}
        ]
    });
    assert_eq!(state.library().file_value(), expected);
    assert_eq!(
        state.profile_payload(),
        json!({
            "sensitivityMatcher":expected,
            "selectedSensitivityMatcher":"selected"
        })
    );
    assert_eq!(state.dataset("None").1, 0);
}

#[::core::prelude::v1::test]
fn matcher_rename_sorts_localized_names_by_the_source_name_key() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        library: Library::from_source_file(&json!({
            "libraryExtension":"keep",
            "profiles":[
                {"guid":"a","name":"Old","profileExtension":"keep"},
                {"guid":"b","name":{"en":"Device"}}
            ]
        }))
        .unwrap(),
        selected: "a".into(),
    });
    state
        .apply_local_action(&MatcherAction::RenameProfile("New".into()))
        .unwrap();
    assert_eq!(
        state.profile_update().file_payload(),
        json!({
            "sensitivityMatcher": {
                "libraryExtension":"keep",
                "profiles":[
                    {"guid":"b","name":{"en":"Device"}},
                    {"guid":"a","name":"New","profileExtension":"keep"}
                ]
            },
            "selectedSensitivityMatcher":"a"
        })
    );
}

#[::core::prelude::v1::test]
fn matcher_profiles_normalize_duplicate_keys_and_request_one_independent_save() {
    for mounted in [false, true] {
        let mut state = MatcherState::default();
        let input = Library::from_source_file(&json!({
            "libraryExtension":{"keep":true},
            "profiles":[
                {"guid":"first","name":"Mouse"},
                {"guid":"duplicate","name":{"en":"Mouse","zh-cn":"Localized"},"extension":null},
                {"guid":"numbered","name":"Mouse 1","dpi":{"x":900,"y":800}},
                {"guid":"later","name":{"en":"Mouse"}},
                {"guid":"none-a","name":"None"},
                {"guid":"none-b","name":"None"}
            ]
        }))
        .unwrap();
        let observation = MatcherObservation::Profiles {
            library: input,
            selected: "duplicate".into(),
        };
        let actions = if mounted {
            state.observe(observation)
        } else {
            state.observe_unmounted(observation)
        };
        assert_eq!(
            state.library().file_value(),
            json!({
                "libraryExtension":{"keep":true},
                "profiles":[
                    {"guid":"first","name":"Mouse"},
                    {"guid":"duplicate","name":"Mouse 1","extension":null},
                    {"guid":"numbered","name":"Mouse 1 1","dpi":{"x":900,"y":800}},
                    {"guid":"later","name":"Mouse 2"},
                    {"guid":"none-a","name":"None"},
                    {"guid":"none-b","name":"None"}
                ]
            })
        );
        assert_eq!(state.selected(), "duplicate");
        assert_eq!(actions, [MatcherAction::NormalizeProfiles]);
        let normalized = state.library().clone();
        let echo = MatcherObservation::Profiles {
            library: normalized.clone(),
            selected: "duplicate".into(),
        };
        assert!(state.observe_unmounted(echo).is_empty());
        assert_eq!(state.library(), &normalized);
    }
}

#[::core::prelude::v1::test]
fn matcher_selection_filters_english_none_without_using_the_first_translation() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        library: library(vec![
            json!({"guid":"english-none","name":{"en":"None","zh-cn":"Localized"}}),
            json!({"guid":"other-locale-none","name":{"zh-cn":"None"}}),
            json!({"guid":"plain-none","name":"None"}),
            json!({"guid":"real","name":"Mouse"}),
        ]),
        selected: "real".into(),
    });
    state
        .apply_local_action(&MatcherAction::SelectProfile("real".into()))
        .unwrap();
    assert_eq!(
        state
            .library()
            .profiles()
            .iter()
            .map(MatcherProfile::guid)
            .collect::<Vec<_>>(),
        ["other-locale-none", "real"]
    );
}

#[::core::prelude::v1::test]
fn matcher_rename_uses_stable_uppercase_utf16_order_before_projecting_submission() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        library: library(vec![
            json!({"guid":"private","name":"\u{e000}"}),
            json!({"guid":"selected","name":"Zulu"}),
            json!({"guid":"supplementary","name":"\u{10000}"}),
            json!({"guid":"first-alpha","name":"alpha"}),
            json!({"guid":"second-alpha","name":"ALPHA"}),
            json!({"guid":"sharp-s","name":"ß"}),
            json!({"guid":"double-s","name":"ss"}),
        ]),
        selected: "selected".into(),
    });
    let actions = state.commit_rename("beta");
    assert_eq!(state.library.profiles()[1].name().text(), Some("Zulu"));
    assert_eq!(actions, vec![MatcherAction::RenameProfile("beta".into())]);
    state.apply_local_action(&actions[0]).unwrap();
    let payload = state.profile_update().file_payload();
    let guids: Vec<_> = payload["sensitivityMatcher"]["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|profile| profile["guid"].as_str().unwrap())
        .collect();
    assert_eq!(
        guids,
        [
            "first-alpha",
            "second-alpha",
            "selected",
            "sharp-s",
            "double-s",
            "supplementary",
            "private"
        ]
    );
    assert_eq!(payload["selectedSensitivityMatcher"], "selected");
    assert_eq!(state.dataset("None").1, 2);
}

#[::core::prelude::v1::test]
fn matcher_rename_trims_ecmascript_whitespace_and_checks_original_names_strictly() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        library: library(vec![
            json!({"guid":"a","name":"Exact"}),
            json!({"guid":"b","name":{"en":"Localized"}}),
        ]),
        selected: "a".into(),
    });
    assert!(state.commit_rename("\u{feff}Exact\u{feff}").is_empty());
    assert!(state.commit_rename("\u{feff}\u{2028}\u{00a0}").is_empty());
    assert!(state.commit_rename("\u{3000}None\u{feff}").is_empty());
    assert_eq!(
        state.commit_rename("\u{feff}\u{1680}exact\u{2029}\u{205f}"),
        vec![MatcherAction::RenameProfile("exact".into())]
    );
    assert_eq!(
        state.commit_rename("Localized"),
        vec![MatcherAction::RenameProfile("Localized".into())]
    );
    // U+0085 is Rust whitespace but is not ECMAScript whitespace; U+180E
    // likewise remains an ordinary character in current ECMAScript.
    for name in ["\u{0085}", "\u{0085}Name\u{0085}", "\u{180e}"] {
        assert_eq!(
            state.commit_rename(name),
            vec![MatcherAction::RenameProfile(name.into())]
        );
    }
}

#[::core::prelude::v1::test]
fn matcher_device_product_name_only_fills_an_empty_profile_label() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Language("ZH-CN".into()));
    state.observe_unmounted(MatcherObservation::Profiles {
        library: library(vec![
            json!({"guid":"a","name":"Profile A"}),
            json!({"guid":"b","name":"Profile B"}),
            json!({"guid":"c","name":{"en":"Profile C","zh-cn":"Local C"}}),
            json!({"guid":"d","name":"Profile D"}),
            json!({"guid":"empty","name":""}),
            json!({"guid":"empty-locale","name":{"en":""}}),
        ]),
        selected: "a".into(),
    });
    state.observe_unmounted(MatcherObservation::Devices(vec![
        json!({"guid":"a","productName":"Plain product string"}),
        json!({"guid":"b","productName":{"zh-cn":"","en":"English device"}}),
        json!({"guid":"c","productName":{"zh-cn":"","en":""}}),
        json!({"guid":"d","productName":{"zh-cn":"Local device","en":"English D"}}),
        json!({"guid":"empty","productName":{"zh-cn":"","en":"English device"}}),
        json!({"guid":"empty-locale","productName":{"zh-cn":"Local device","en":"English"}}),
    ]));
    let (items, selected) = state.dataset("None");
    assert_eq!(selected, 0);
    assert_eq!(
        items.iter().map(ProfileItem::name).collect::<Vec<_>>(),
        [
            "Profile A",
            "Profile B",
            "Local C",
            "Profile D",
            "English device",
            "Local device"
        ]
    );
}

#[::core::prelude::v1::test]
fn matcher_language_changes_keep_profile_identity_and_local_drafts() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        library: library(vec![
            json!({"guid":"first","name":{"en":"First","zh-cn":"第一个"}}),
            json!({"guid":"selected","name":{"en":"Selected","zh-cn":"选中的"}}),
        ]),
        selected: "selected".into(),
    });
    let before = state.profile_update();
    state.open();
    for (language, expected) in [("ZH-CN", "选中的"), ("en", "Selected")] {
        assert!(
            state
                .observe(MatcherObservation::Language(language.into()))
                .is_empty()
        );
        let (items, selected) = state.dataset("None");
        assert_eq!(items[selected].guid(), "selected");
        assert_eq!(items[selected].name(), expected);
        assert_eq!(state.profile_update(), before);
        assert!(state.popup());
        assert!(state.start_listener);
    }
}

#[::core::prelude::v1::test]
fn matcher_unmounted_observations_do_not_run_update_lifecycle_or_consume_first_error() {
    let mut state = MatcherState::default();
    for phase in [Phase::Calibrating, Phase::Completed, Phase::Error] {
        assert!(state.observe_unmounted(observation(phase)).is_empty());
        assert_eq!(state.phase(), phase);
        assert!(!state.popup());
        assert!(!state.first_load);
    }
    assert_eq!(
        state.mount(),
        vec![MatcherAction::Command(MatcherCommand::GetState)]
    );
    // Mounting does not retroactively run componentDidUpdate for existing props.
    assert!(state.observe(observation(Phase::Error)).is_empty());
    assert!(!state.popup());
    state.observe(observation(Phase::Ready));
    assert_eq!(
        state.observe(observation(Phase::Error)),
        vec![MatcherAction::Command(MatcherCommand::Reset)]
    );
    assert!(!state.popup());
    assert!(state.first_load);
    state.open();
    state.unmount();
    state.observe_unmounted(observation(Phase::Ready));
    assert!(state.popup()); // Redux popup state survives component destruction.
    assert!(!state.first_load);
    assert!(state.window_click(false).is_empty());
}

#[::core::prelude::v1::test]
fn matcher_selection_updates_only_local_reducer_state_and_commands_never_acknowledge() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        library: library(vec![
            json!({"guid":"none","name":"None"}),
            json!({"guid":"real","name":"Observed","dpi":{"x":900,"y":800}}),
        ]),
        selected: "none".into(),
    });
    state.observe_unmounted(MatcherObservation::Stages {
        stages: stages(vec![json!({"x":400,"y":400}), json!({"x":1600,"y":1600})]),
        active: 2,
    });
    state.observe_unmounted(MatcherObservation::Calibration(Calibration::new(
        Phase::Completed,
        99.,
        400.,
        900.,
    )));
    let actions = state.choose("real");
    assert_eq!(state.selected(), "none");
    assert_eq!(state.stages[1].file_value(), json!({"x":1600,"y":1600}));
    for action in &actions {
        state.apply_local_action(action).unwrap();
    }
    assert_eq!(state.selected(), "real");
    assert_eq!(
        state.stages,
        stages(vec![json!({"x":400,"y":400}), json!({"x":900,"y":800})])
    );
    assert_eq!(state.library.profiles().len(), 1);
    let profiles = state.profile_update().file_payload();
    let calibration = state.calibration;
    let stages = state.stages.clone();
    for command in [
        MatcherCommand::GetState,
        MatcherCommand::Start,
        MatcherCommand::Confirm,
        MatcherCommand::Cancel,
        MatcherCommand::Reset,
        MatcherCommand::DeleteProfile("real".into()),
        MatcherCommand::DeleteAllProfiles,
    ] {
        state
            .apply_local_action(&MatcherAction::Command(command))
            .unwrap();
        assert_eq!(state.profile_update().file_payload(), profiles);
        assert_eq!(state.calibration, calibration);
        assert_eq!(state.stages, stages);
    }
    assert_eq!(state.phase(), Phase::Completed);
}

#[::core::prelude::v1::test]
fn matcher_selection_projects_every_stage_and_retains_the_live_profile_numbers() {
    use razer_device::mouse_current_dpi::SourceNumber;

    let profile = MatcherProfile::new(
        "raw".into(),
        ProfileName::Text("Observed".into()),
        Some(SourceStage::new(
            Some(SourceNumber::new(f64::NAN)),
            Some(SourceNumber::new(f64::NEG_INFINITY)),
            Some(true),
            Some(false),
        )),
    );
    let observed_stages = vec![
        SourceStage::new(
            Some(SourceNumber::new(f64::INFINITY)),
            Some(SourceNumber::new(f64::NAN)),
            Some(false),
            None,
        ),
        SourceStage::new(Some(400_u16.into()), Some(400_u16.into()), None, None),
        SourceStage::new(
            Some(SourceNumber::new(-0.)),
            Some(SourceNumber::new(70000.25)),
            Some(true),
            Some(true),
        ),
    ];
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        library: Library::new(vec![
            MatcherProfile::new("none".into(), ProfileName::Text("None".into()), None),
            profile.clone(),
        ]),
        selected: "none".into(),
    });
    state.observe_unmounted(MatcherObservation::Stages {
        stages: observed_stages.clone(),
        active: 2,
    });

    let actions = state.choose("raw");
    assert_eq!(state.selected(), "none");
    assert_eq!(state.stages, observed_stages);
    assert_eq!(state.library.profiles()[1], profile);
    let [
        MatcherAction::SetStageValueX {
            stages: projected,
            active,
        },
        MatcherAction::SelectProfile(guid),
    ] = actions.as_slice()
    else {
        panic!("SELECT must dispatch the stage draft before library selection");
    };
    assert_eq!(*active, 2);
    assert_eq!(guid, "raw");
    assert_eq!(
        projected,
        &stages(vec![
            json!({"x":null,"y":null,"visible":false}),
            json!({"x":null,"y":null,"visible":true,"independent":false}),
            json!({"x":0,"y":70000.25,"visible":true,"independent":true}),
        ])
    );
    assert!(projected[0].x().is_none());
    assert!(projected[0].y().is_none());
    assert!(projected[1].x().is_none());
    assert!(projected[1].y().is_none());
    assert_eq!(projected[2].x().unwrap().value().to_bits(), 0_f64.to_bits());

    for action in &actions {
        state.apply_local_action(action).unwrap();
    }
    assert_eq!(state.selected(), "raw");
    assert_eq!(state.stages, *projected);
    assert_eq!(state.library.profiles(), &[profile]);
    let update = state.profile_update();
    let dpi = update.profiles()[0].dpi().unwrap();
    assert!(dpi.x().unwrap().value().is_nan());
    assert_eq!(dpi.y().unwrap().value(), f64::NEG_INFINITY);
    assert_eq!(
        update.file_payload()["sensitivityMatcher"]["profiles"][0]["dpi"],
        json!({"x":null,"y":null,"visible":true,"independent":false})
    );
}

#[::core::prelude::v1::test]
fn matcher_calibration_snapshots_preserve_initial_absence_and_numeric_children() {
    let mut state = MatcherState::default();
    assert_eq!(state.phase(), Phase::Idle);
    assert_eq!(state.current_dpi(), None);
    assert_eq!(state.new_dpi(), 0.);
    assert_eq!(state.process(), 0.);
    for (new_dpi, text) in [
        (400., "400"),
        (f64::NAN, "NaN"),
        (f64::INFINITY, "Infinity"),
    ] {
        state.observe(MatcherObservation::Calibration(Calibration::new(
            Phase::Completed,
            99.,
            800.,
            new_dpi,
        )));
        assert_eq!(state.current_dpi(), Some(800.));
        assert_eq!(
            razer_device::source_string::number_text(state.new_dpi()),
            text
        );
        assert_eq!(state.phase(), Phase::Completed);
        assert_eq!(state.process(), 99.);
        assert_eq!(
            state.apply(),
            [MatcherAction::Command(MatcherCommand::Confirm)]
        );
    }
}

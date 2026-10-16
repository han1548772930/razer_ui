// Pure state projections: no middleware, device, application or vendor code.
#[::core::prelude::v1::test]
fn initial_none_profile_is_a_local_uuid_placeholder_and_not_observed_dpi() {
    let state = MatcherState::default();
    let placeholder = &state.profiles[0];
    let guid = uuid::Uuid::parse_str(placeholder["guid"].as_str().unwrap()).unwrap();
    assert_eq!(guid.get_version_num(), 4);
    assert_eq!(placeholder["name"], "None");
    assert_eq!(placeholder["dpi"], Value::Null);
    // AS adds its disabled localized sentinel when selected GUID is empty.
    let (items, selected) = state.dataset("Localized None");
    assert_eq!(selected, 0);
    assert_eq!(items.len(), 2);
    assert!(items[0].disabled());
    assert_eq!(items[1].guid(), placeholder["guid"].as_str().unwrap());
}

#[::core::prelude::v1::test]
fn failed_source_name_sort_cannot_mutate_or_submit_a_renamed_library() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        profiles: vec![
            json!({"guid":"a","name":"Old"}),
            json!({"guid":"b","name":{"en":"Device"}}),
        ],
        selected: "a".into(),
    });
    let before = state.profile_payload();
    assert_eq!(
        state.apply_local_action(&MatcherAction::RenameProfile("New".into())),
        Err(MatcherReducerError::NonStringProfileName)
    );
    assert_eq!(state.profile_payload(), before);
}

#[::core::prelude::v1::test]
fn rename_uses_stable_uppercase_utf16_order_before_projecting_submission() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        profiles: vec![
            json!({"guid":"private","name":"\u{e000}"}),
            json!({"guid":"selected","name":"Zulu"}),
            json!({"guid":"supplementary","name":"\u{10000}"}),
            json!({"guid":"first-alpha","name":"alpha"}),
            json!({"guid":"second-alpha","name":"ALPHA"}),
            json!({"guid":"sharp-s","name":"ß"}),
            json!({"guid":"double-s","name":"ss"}),
        ],
        selected: "selected".into(),
    });
    let actions = state.commit_rename("beta");
    assert_eq!(state.profiles[1]["name"], "Zulu");
    assert_eq!(actions, vec![MatcherAction::RenameProfile("beta".into())]);
    state.apply_local_action(&actions[0]).unwrap();
    let payload = state.profile_payload();
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
fn rename_trims_ecmascript_whitespace_and_checks_original_names_strictly() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        profiles: vec![
            json!({"guid":"a","name":"Exact"}),
            json!({"guid":"b","name":{"en":"Localized"}}),
        ],
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
fn device_product_name_requires_localized_properties_and_keeps_profile_fallback() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Language("ZH-CN".into()));
    state.observe_unmounted(MatcherObservation::Profiles {
        profiles: vec![
            json!({"guid":"a","name":"Profile A"}),
            json!({"guid":"b","name":"Profile B"}),
            json!({"guid":"c","name":{"en":"Profile C","zh-cn":"Local C"}}),
            json!({"guid":"d","name":"Profile D"}),
        ],
        selected: "a".into(),
    });
    state.observe_unmounted(MatcherObservation::Devices(vec![
        json!({"guid":"a","productName":"Plain product string"}),
        json!({"guid":"b","productName":{"zh-cn":"","en":"English device"}}),
        json!({"guid":"c","productName":{"zh-cn":"","en":""}}),
        json!({"guid":"d","productName":{"zh-cn":"Local device","en":"English D"}}),
    ]));
    let (items, selected) = state.dataset("None");
    assert_eq!(selected, 0);
    assert_eq!(
        items.iter().map(ProfileItem::name).collect::<Vec<_>>(),
        ["Profile A", "English device", "Local C", "Local device"]
    );
}

#[::core::prelude::v1::test]
fn unmounted_observations_do_not_run_update_lifecycle_or_consume_first_error() {
    let mut state = MatcherState::default();
    for phase in ["calibrating", "completed", "error"] {
        assert!(
            state
                .observe_unmounted(MatcherObservation::Calibration(json!({"state":phase})))
                .is_empty()
        );
        assert_eq!(state.phase(), Some(phase));
        assert!(!state.popup());
        assert!(!state.first_load);
    }
    assert_eq!(
        state.mount(),
        vec![MatcherAction::Command(MatcherCommand::GetState)]
    );
    // Mounting does not retroactively run componentDidUpdate for existing props.
    assert!(
        state
            .observe(MatcherObservation::Calibration(json!({"state":"error"})))
            .is_empty()
    );
    assert!(!state.popup());
    state.observe(MatcherObservation::Calibration(json!({"state":"ready"})));
    assert_eq!(
        state.observe(MatcherObservation::Calibration(json!({"state":"error"}))),
        vec![MatcherAction::Command(MatcherCommand::Reset)]
    );
    assert!(!state.popup());
    assert!(state.first_load);
    state.open();
    state.unmount();
    state.observe_unmounted(MatcherObservation::Calibration(json!({"state":"ready"})));
    assert!(state.popup()); // Redux popup state survives component destruction.
    assert!(!state.first_load);
    assert!(state.window_click(false).is_empty());
}

#[::core::prelude::v1::test]
fn selection_updates_only_local_reducer_state_and_commands_never_acknowledge() {
    let mut state = MatcherState::default();
    state.observe_unmounted(MatcherObservation::Profiles {
        profiles: vec![
            json!({"guid":"none","name":"None"}),
            json!({"guid":"real","name":"Observed","dpi":{"x":900,"y":800}}),
        ],
        selected: "none".into(),
    });
    state.observe_unmounted(MatcherObservation::Stages {
        stages: vec![json!({"x":400,"y":400}), json!({"x":1600,"y":1600})],
        active: 2,
    });
    state.observe_unmounted(MatcherObservation::Calibration(
        json!({"state":"completed","currentDpi":400,"newDpi":900}),
    ));
    let actions = state.choose("real");
    assert_eq!(state.selected(), "none");
    assert_eq!(state.stages[1], json!({"x":1600,"y":1600}));
    for action in &actions {
        state.apply_local_action(action).unwrap();
    }
    assert_eq!(state.selected(), "real");
    assert_eq!(
        state.stages,
        vec![json!({"x":400,"y":400}), json!({"x":900,"y":800})]
    );
    assert_eq!(state.profiles.len(), 1);
    let profiles = state.profile_payload();
    let calibration = state.calibration.clone();
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
        assert_eq!(state.profile_payload(), profiles);
        assert_eq!(state.calibration, calibration);
        assert_eq!(state.stages, stages);
    }
    assert_eq!(state.phase(), Some("completed"));
}

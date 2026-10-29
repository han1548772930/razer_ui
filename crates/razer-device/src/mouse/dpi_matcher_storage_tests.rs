use super::*;

fn document(names: &[Value]) -> Value {
    json!({
        "version":7,
        "activeProfile":"current",
        "profiles":[{"guid":"current","selectedSensitivityMatcher":"previous-0","dpiStages":{
            "active":2,"enable":true,"count":2,
            "stages":[
                {"x":800,"y":900,"visible":true,"independent":true},
                {"x":1600,"y":1700,"visible":true,"independent":true}
            ]
        }}],
        "sensitivityMatcher":{"profiles":names.iter().enumerate().map(|(ix,name)|
            json!({
                "guid":format!("previous-{ix}"),"name":name,
                "dpi":{"x":800,"y":900,"visible":true,"independent":true}
            })
        ).collect::<Vec<_>>()}
    })
}

fn record(names: &[Value]) -> SourceData {
    SourceData::from_document(&document(names)).unwrap()
}

fn name(value: Value) -> ProfileName {
    serde_json::from_value(value).unwrap()
}

fn row(x: f64, y: f64) -> SourceStage {
    SourceStage::new(
        Some(SourceNumber::new(x)),
        Some(SourceNumber::new(y)),
        Some(true),
        Some(false),
    )
}

fn assert_axes(stage: &SourceStage, x: f64, y: f64) {
    assert_eq!(stage.x().unwrap().value().to_bits(), x.to_bits());
    assert_eq!(stage.y().unwrap().value().to_bits(), y.to_bits());
}

#[test]
fn dpi_matcher_zero_result_reaches_the_source_confirmation_plan() {
    let result = confirm(&record(&[]), 0., "zero", name(json!("Mouse"))).unwrap();
    assert_axes(result.response().profiles()[0].dpi().unwrap(), 0., 0.);
    assert_axes(&result.stages().unwrap().stages().stages()[1], 0., 0.);
    assert_eq!(result.response().profiles()[0].file_value()["dpi"]["x"], 0);
}

#[test]
fn dpi_matcher_confirm_uses_first_free_string_candidate_and_strict_equality() {
    // Current 226 main module55680 S/AM and 190 module76665 d/W$ compare
    // names with === and test the base before suffixes 1..99.
    for (names, base, expected) in [
        (
            vec![json!("Mouse"), json!("Mouse 1"), json!("Mouse 3")],
            "Mouse",
            "Mouse 2",
        ),
        (
            vec![json!("mouse"), json!({"en":"Mouse"})],
            "Mouse",
            "Mouse 1",
        ),
        (vec![json!("Keymap")], "Keymap", "Keymap 1"),
        (
            vec![json!("Keymap 1"), json!("Keymap 3")],
            "Keymap",
            "Keymap 2",
        ),
        (vec![json!("")], "", " 1"),
    ] {
        let source = record(&names);
        let before_library = source.library().clone();
        let before_stages = source.stages().unwrap().clone();
        let result = confirm(&source, 1450., "measured", name(json!(base))).unwrap();
        assert_eq!(
            result.response().profiles()[names.len()].name().text(),
            Some(expected)
        );
        assert_eq!(result.response().selected(), Some("measured"));
        let stages = result.stages().unwrap().stages();
        assert_eq!(stages.active_stage(), 2);
        assert!(stages.enabled());
        assert_axes(&stages.stages()[1], 1450., 1450.);
        assert_eq!(stages.stages()[1].is_independent(), Some(false));
        assert_eq!(stages.stages()[1].is_visible(), Some(true));
        assert_eq!(stages.stages()[0], before_stages.stages()[0]);
        assert_eq!(source.library(), &before_library);
        assert_eq!(source.stages(), Some(&before_stages));
    }
}

#[test]
fn dpi_matcher_confirm_deduplicates_english_names_and_appends_the_same_chinese_suffix() {
    let localized = json!({"en":"Mouse","zh-cn":"Source mouse"});
    let source = record(&[localized.clone(), localized.clone()]);
    let result = confirm(&source, 1200., "measured", name(localized.clone())).unwrap();
    assert_eq!(
        result.response().profiles()[2].name(),
        &name(json!({"en":"Mouse 1","zh-cn":"Source mouse 1"}))
    );

    let source = record(&[
        json!("Mouse"),
        json!({"en":"Mouse 1","zh-cn":"Different"}),
        json!({"en":"mouse 2"}),
        json!({"en":"Mouse 3"}),
    ]);
    let result = confirm(
        &source,
        1200.,
        "measured",
        name(json!({"en":"Mouse","zh-cn":"Source mouse","extra":"keep"})),
    )
    .unwrap();
    assert_eq!(
        result.response().profiles()[4].name(),
        &name(json!({"en":"Mouse 2","zh-cn":"Source mouse 2","extra":"keep"}))
    );

    for base in ["Mouse", "Keymap"] {
        let mut names: Vec<_> = (1..=100)
            .map(|suffix| json!(format!("{base} {suffix}")))
            .collect();
        names.push(json!(base));
        let result = confirm(&record(&names), 1200., "measured", name(json!(base))).unwrap();
        // Both helpers return the last candidate even when it still collides.
        assert_eq!(
            result.response().profiles()[101].name().text(),
            Some(format!("{base} 100").as_str())
        );
        let result = confirm(
            &record(&names),
            1200.,
            "localized",
            name(json!({"en":base,"zh-cn":"Localized"})),
        )
        .unwrap();
        assert_eq!(
            result.response().profiles()[101].name(),
            &name(json!({"en":format!("{base} 100"),"zh-cn":"Localized 100"}))
        );
    }
}

#[test]
fn dpi_matcher_kx_reads_first_property_in_ecmascript_order_and_keeps_falsy_fallbacks_empty() {
    let first_localized: ProfileName =
        serde_json::from_str(r#"{"zh-cn":"区域","fr":"French"}"#).unwrap();
    assert_eq!(
        first_localized
            .english_or_first_property_or_empty()
            .unwrap(),
        "区域"
    );

    let en_empty: ProfileName = serde_json::from_str(r#"{"en":"","zh-cn":"后面的值"}"#).unwrap();
    assert_eq!(en_empty.english_or_first_property_or_empty().unwrap(), "");

    for value in [
        r#"{"en":null,"zh-cn":"后面的值"}"#,
        r#"{"en":false,"zh-cn":"后面的值"}"#,
        r#"{"en":0,"zh-cn":"后面的值"}"#,
        r#"{}"#,
    ] {
        let name: ProfileName = serde_json::from_str(value).unwrap();
        assert_eq!(name.english_or_first_property_or_empty().unwrap(), "");
    }
    for value in [r#"{"en":true}"#, r#"{"en":{}}"#] {
        let name: ProfileName = serde_json::from_str(value).unwrap();
        assert!(name.english_or_first_property_or_empty().is_err());
    }

    let numeric_keys: ProfileName = serde_json::from_str(
        r#"{"10":"ten","2":"two","01":"leading","0":"zero","4294967295":"max","tail":"tail"}"#,
    )
    .unwrap();
    assert_eq!(
        numeric_keys.english_or_first_property_or_empty().unwrap(),
        "zero"
    );
    let no_zero: ProfileName = serde_json::from_str(
        r#"{"10":"ten","2":"two","01":"leading","4294967295":"max","tail":"tail"}"#,
    )
    .unwrap();
    assert_eq!(no_zero.english_or_first_property_or_empty().unwrap(), "two");
    let no_index: ProfileName =
        serde_json::from_str(r#"{"4294967295":"max","01":"leading","tail":"tail"}"#).unwrap();
    assert_eq!(
        no_index.english_or_first_property_or_empty().unwrap(),
        "max"
    );
}

#[test]
fn dpi_matcher_d_name_and_none_filter_keep_source_truthiness_and_strict_equality() {
    assert_eq!(name(json!("Mouse")).english_or_empty().unwrap(), "Mouse");
    for value in [
        r#"{"en":""}"#,
        r#"{"en":null}"#,
        r#"{"en":false}"#,
        r#"{"en":0}"#,
    ] {
        let localized: ProfileName = serde_json::from_str(value).unwrap();
        assert_eq!(localized.english_or_empty().unwrap(), "");
    }
    let truthy_number: ProfileName = serde_json::from_str(r#"{"en":7}"#).unwrap();
    assert!(truthy_number.english_or_empty().is_err());

    assert!(name(json!("None")).is_english_none());
    assert!(name(json!({"en":"None","zh-cn":"无"})).is_english_none());
    assert!(!name(json!({"zh-cn":"None"})).is_english_none());
    assert!(!name(json!({"en":null,"zh-cn":"None"})).is_english_none());
}

#[test]
fn dpi_matcher_k2_deduplicates_typed_keys_without_reordering_or_dropping_extensions() {
    let source: Value = serde_json::from_str(
        r#"{
            "libraryExtension":{"version":3},
            "profiles":[
                {"guid":"plain-none","name":"None","profileExtension":1},
                {"guid":"localized-none","name":{"en":"None","zh-cn":"无"}},
                {"guid":"localized-none-duplicate","name":{"en":"None","zh-cn":"另一个"}},
                {"guid":"empty","name":""},
                {"guid":"mouse","name":{"en":"Mouse","zh-cn":"鼠标"},"profileExtension":{"keep":true}},
                {"guid":"mouse-duplicate","name":{"en":"Mouse","zh-cn":"鼠标二"}},
                {"guid":"mouse-one","name":{"en":"Mouse 1"}},
                {"guid":"lower","name":"mouse"},
                {"guid":"number","name":{"en":1}},
                {"guid":"string-number","name":{"en":"1"}},
                {"guid":"number-duplicate","name":{"en":1}},
                {"guid":"zero","name":{"en":0,"zh-cn":"零"}},
                {"guid":"truthy","name":{"en":true}},
                {"guid":"truthy-duplicate","name":{"en":true}},
                {"guid":"object","name":{"en":{"label":"a"}}},
                {"guid":"object-duplicate","name":{"en":{"label":"a"}}}
            ]
        }"#,
    )
    .unwrap();
    let library = Library::from_source_file(&source).unwrap();
    let deduplicated = library.with_deduplicated_names();
    let profiles = deduplicated.profiles();
    assert_eq!(
        profiles
            .iter()
            .map(MatcherProfile::guid)
            .collect::<Vec<_>>(),
        library
            .profiles()
            .iter()
            .map(MatcherProfile::guid)
            .collect::<Vec<_>>()
    );
    assert_eq!(profiles[0].name().text(), Some("None"));
    assert_eq!(profiles[1].name(), &name(json!({"en":"None","zh-cn":"无"})));
    assert_eq!(profiles[2].name().text(), Some("None 1"));
    assert_eq!(profiles[3].name().text(), Some(""));
    assert_eq!(
        profiles[4].name(),
        &name(json!({"en":"Mouse","zh-cn":"鼠标"}))
    );
    let ProfileName::Localized(values) = profiles[4].name() else {
        panic!("localized source name must retain its ordered map");
    };
    assert_eq!(
        values.keys().map(String::as_str).collect::<Vec<_>>(),
        ["en", "zh-cn"]
    );
    assert_eq!(profiles[5].name().text(), Some("Mouse 1"));
    assert_eq!(profiles[6].name().text(), Some("Mouse 1 1"));
    assert_eq!(profiles[7].name().text(), Some("mouse"));
    assert_eq!(profiles[8].name(), &name(json!({"en":1})));
    assert_eq!(profiles[9].name(), &name(json!({"en":"1"})));
    assert_eq!(profiles[10].name().text(), Some("1 1"));
    assert_eq!(profiles[11].name(), &name(json!({"en":0,"zh-cn":"零"})));
    assert_eq!(profiles[12].name(), &name(json!({"en":true})));
    assert_eq!(profiles[13].name().text(), Some("true 1"));
    assert!(matches!(profiles[14].name(), ProfileName::Localized(_)));
    assert!(matches!(profiles[15].name(), ProfileName::Localized(_)));
    assert_eq!(profiles[4].file_value()["profileExtension"]["keep"], true);
    assert_eq!(deduplicated.file_value()["libraryExtension"]["version"], 3);
}

#[test]
fn dpi_matcher_confirm_rejects_invalid_identity_without_mutation() {
    let source = record(&[json!("Mouse")]);
    let before_library = source.library().clone();
    let before_stages = source.stages().unwrap().clone();
    for guid in ["", "previous-0"] {
        assert!(confirm(&source, 1200., guid, name(json!("Mouse"))).is_err());
        assert_eq!(source.library(), &before_library);
        assert_eq!(source.stages(), Some(&before_stages));
        assert_eq!(source.selected(), Some("previous-0"));
    }
    assert!(serde_json::from_value::<ProfileName>(Value::Null).is_err());
    for invalid in [json!({"en":"Mouse"}), json!({"en":null,"zh-cn":"Mouse"})] {
        assert!(confirm(&source, 1200., "measured", name(invalid)).is_err());
        assert_eq!(source.library(), &before_library);
        assert_eq!(source.stages(), Some(&before_stages));
    }
}

#[test]
fn dpi_matcher_confirm_result_preserves_the_typed_source_stage_fields() {
    let result = confirm(&record(&[]), 400., "measured", name(json!("Mouse"))).unwrap();
    let stages = result.stages().unwrap().stages().stages();
    assert_axes(&stages[0], 800., 900.);
    assert_axes(&stages[1], 400., 400.);
    assert_eq!(stages[0].is_independent(), Some(true));
    assert_eq!(stages[1].is_independent(), Some(false));
    assert_eq!(stages[1].is_visible(), Some(true));
}

#[test]
fn dpi_matcher_confirm_keeps_raw_numbers_until_the_file_boundary() {
    // 32807 CONFIRM copies M.newDpi; ft structuredClone retains each Number.
    // Only the separately invoked JSON.stringify file boundary maps it to null.
    for (dpi, file) in [
        (0., json!(0)),
        (-0., json!(0)),
        (-800., json!(-800)),
        (f64::NAN, Value::Null),
        (f64::INFINITY, Value::Null),
        (f64::NEG_INFINITY, Value::Null),
        (70_000., json!(70_000)),
        (4_294_967_297.75, json!(4_294_967_297.75)),
    ] {
        let source = record(&[]);
        let result = confirm(&source, dpi, "measured", name(json!("Mouse"))).unwrap();
        let profile = &result.response().profiles()[0];
        assert_axes(profile.dpi().unwrap(), dpi, dpi);
        assert_axes(&result.stages().unwrap().stages().stages()[1], dpi, dpi);
        assert_eq!(profile.file_value()["dpi"]["x"], file);
        assert_eq!(profile.file_value()["dpi"]["y"], file);
        assert_axes(&source.stages().unwrap().stages()[1], 1600., 1700.);
    }
}

#[test]
fn dpi_matcher_select_projects_the_stage_request_but_retains_the_live_library() {
    for dpi in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut source = record(&[]);
        source.apply(&Change::Profiles(ProfileUpdate::new(
            Library::new(vec![MatcherProfile::new(
                "raw".into(),
                name(json!("Mouse")),
                Some(row(dpi, dpi)),
            )]),
            Some("previous".into()),
        )));
        let result = select(&source, "raw").unwrap();
        let selected = &result.stages().unwrap().stages().stages()[1];
        assert_eq!(selected.x(), None);
        assert_eq!(selected.y(), None);
        assert_eq!(selected.is_visible(), Some(true));
        assert_eq!(result.stages().unwrap().selected(), Some(""));
        assert_axes(result.response().profiles()[0].dpi().unwrap(), dpi, dpi);
        assert_eq!(result.response().selected(), Some("raw"));
        assert_eq!(source.selected(), Some("previous"));
    }
}

#[test]
fn dpi_matcher_select_json_clone_projects_every_existing_stage() {
    let mut source = record(&[]);
    source.apply(&Change::Stages(StageUpdate::new(
        SourceStages::new(
            true,
            2,
            vec![row(f64::NAN, f64::INFINITY), row(1600., 1700.)],
        ),
        None,
    )));
    source.apply(&Change::Profiles(ProfileUpdate::new(
        Library::new(vec![MatcherProfile::new(
            "finite".into(),
            name(json!("Mouse")),
            Some(row(1450., 1500.)),
        )]),
        None,
    )));
    let result = select(&source, "finite").unwrap();
    let stages = result.stages().unwrap().stages();
    assert_eq!(stages.stages()[0].x(), None);
    assert_eq!(stages.stages()[0].y(), None);
    assert_axes(&stages.stages()[1], 1450., 1500.);
    assert_axes(
        &source.stages().unwrap().stages()[0],
        f64::NAN,
        f64::INFINITY,
    );
}

#[test]
fn dpi_matcher_stage_and_library_changes_are_independent() {
    let mut source = record(&[json!("Previous")]);
    let old_library = source.library().clone();
    let old_stages = source.stages().unwrap().clone();
    let result = confirm(&source, 1450., "measured", name(json!("Mouse"))).unwrap();
    let stage_change = Change::Stages(result.stages().unwrap().clone());
    let library_change = Change::Profiles(result.response().clone());

    source.apply(&stage_change);
    assert_axes(&source.stages().unwrap().stages()[1], 1450., 1450.);
    assert_eq!(source.library(), &old_library);
    assert_eq!(source.selected(), Some(""));
    let saved_stages = source.stages().unwrap().clone();

    source.apply(&library_change);
    assert_eq!(source.stages(), Some(&saved_stages));
    assert_eq!(source.library().profiles().len(), 2);
    assert_eq!(source.library().profiles()[1].guid(), "measured");
    assert_eq!(source.selected(), Some("measured"));

    let mut stage_not_saved = record(&[json!("Previous")]);
    stage_not_saved.apply(&library_change);
    assert_eq!(stage_not_saved.stages(), Some(&old_stages));
    assert_eq!(stage_not_saved.selected(), Some("measured"));
    assert_eq!(stage_not_saved.library().profiles().len(), 2);
}

#[test]
fn dpi_matcher_stage_selection_uses_the_first_strict_x_match_only() {
    let mut data = document(&[json!("First"), json!("Second"), json!("Third")]);
    data["sensitivityMatcher"]["profiles"][0]["dpi"] =
        json!({"x":1450,"y":1,"visible":false,"independent":true});
    data["sensitivityMatcher"]["profiles"][1]["dpi"] =
        json!({"x":1450,"y":1450,"visible":true,"independent":false});
    data["sensitivityMatcher"]["profiles"][2]["dpi"] = json!({"x":1450.00001,"y":1450});
    let source = SourceData::from_document(&data).unwrap();
    let result = confirm(&source, 1450., "measured", name(json!("Mouse"))).unwrap();
    assert_eq!(result.stages().unwrap().selected(), Some("previous-0"));
    assert_eq!(result.response().selected(), Some("measured"));

    for dpi in [0., -800., f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let mut source = record(&[]);
        source.apply(&Change::Profiles(ProfileUpdate::new(
            Library::new(vec![MatcherProfile::new(
                "matching".into(),
                name(json!("Other")),
                Some(row(dpi, 123.)),
            )]),
            None,
        )));
        let result = confirm(&source, dpi, "measured", name(json!("Mouse"))).unwrap();
        assert_eq!(
            result.stages().unwrap().selected(),
            Some(if dpi.is_nan() { "" } else { "matching" })
        );
    }
}

#[test]
fn dpi_matcher_stage_task_leaves_selection_when_the_library_is_absent() {
    let mut data = document(&[]);
    data.as_object_mut().unwrap().remove("sensitivityMatcher");
    let mut source = SourceData::from_document(&data).unwrap();
    let result = confirm(&source, 1450., "measured", name(json!("Mouse"))).unwrap();
    assert_eq!(result.stages().unwrap().selected(), None);
    source.apply(&Change::Stages(result.stages().unwrap().clone()));
    assert_eq!(source.selected(), Some("previous-0"));
    assert!(source.library().profiles().is_empty());
}

#[test]
fn dpi_matcher_confirm_without_an_active_row_does_not_create_one() {
    for active in [0, 3] {
        let mut data = document(&[]);
        data["profiles"][0]["dpiStages"]["active"] = json!(active);
        let source = SourceData::from_document(&data).unwrap();
        let before = source.stages().unwrap().clone();
        let result = confirm(&source, 1450., "measured", name(json!("Mouse"))).unwrap();
        assert_eq!(result.stages().unwrap().stages(), &before);
        assert_eq!(result.stages().unwrap().selected(), None);
        assert_eq!(result.response().selected(), Some("measured"));
        assert_eq!(result.response().profiles().len(), 1);
    }
}

#[test]
fn dpi_matcher_confirm_keeps_a_null_active_row_while_select_replaces_it() {
    let mut data = document(&[json!("Previous")]);
    data["profiles"][0]["dpiStages"]["stages"][1] = Value::Null;
    let source = SourceData::from_document(&data).unwrap();
    let confirmed = confirm(&source, 1450., "measured", name(json!("Mouse"))).unwrap();
    assert!(confirmed.stages().unwrap().stages().stages()[1].is_null());
    assert_eq!(confirmed.stages().unwrap().selected(), None);
    assert_eq!(confirmed.response().selected(), Some("measured"));

    let selected = select(&source, "previous-0").unwrap();
    assert_axes(&selected.stages().unwrap().stages().stages()[1], 800., 900.);
    assert_eq!(selected.stages().unwrap().selected(), Some("previous-0"));
}

#[test]
fn dpi_matcher_select_preserves_null_and_missing_x_strict_equality() {
    for dpi in [
        json!({"x":null,"y":900,"visible":true}),
        json!({"y":900,"visible":true}),
    ] {
        let mut data = document(&[json!("Previous")]);
        data["sensitivityMatcher"]["profiles"][0]["dpi"] = dpi.clone();
        let source = SourceData::from_document(&data).unwrap();
        let selected = select(&source, "previous-0").unwrap();
        let stage = &selected.stages().unwrap().stages().stages()[1];
        assert_eq!(stage.file_value(), dpi);
        assert_eq!(selected.stages().unwrap().selected(), Some("previous-0"));
    }
}

#[test]
fn dpi_matcher_select_removes_only_the_literal_none_profile() {
    let source = record(&[json!("None"), json!({"en":"None"}), json!("none")]);
    let result = select(&source, "previous-1").unwrap();
    assert_eq!(result.response().profiles().len(), 2);
    assert_eq!(result.response().profiles()[0].guid(), "previous-1");
    assert_eq!(result.response().profiles()[1].guid(), "previous-2");
    assert_eq!(source.library().profiles().len(), 3);
    assert!(select(&source, "missing").is_err());
    assert!(select(&source, "previous-0").is_err());
}

#[test]
fn dpi_matcher_baseline_uses_the_local_x_and_source_falsy_values() {
    assert_eq!(record(&[]).baseline().unwrap(), 1600.);
    for x in [0., -0., f64::NAN] {
        let mut source = record(&[]);
        source.apply(&Change::Stages(StageUpdate::new(
            SourceStages::new(true, 1, vec![row(x, 99.)]),
            None,
        )));
        assert_eq!(source.baseline().unwrap(), 100.);
    }
    for x in [-800., f64::INFINITY, f64::NEG_INFINITY] {
        let mut source = record(&[]);
        source.apply(&Change::Stages(StageUpdate::new(
            SourceStages::new(true, 1, vec![row(x, 99.)]),
            None,
        )));
        assert_eq!(source.baseline().unwrap().to_bits(), x.to_bits());
    }
    for row in [json!({"y":900}), json!({"x":null,"y":900})] {
        let mut data = document(&[]);
        data["profiles"][0]["dpiStages"]["stages"][1] = row;
        assert_eq!(
            SourceData::from_document(&data)
                .unwrap()
                .baseline()
                .unwrap(),
            100.
        );
    }
}

#[test]
fn dpi_matcher_baseline_missing_stages_differs_from_a_missing_active_row() {
    for stages in [None, Some(Value::Null)] {
        let mut data = document(&[]);
        let profile = data["profiles"][0].as_object_mut().unwrap();
        match stages {
            Some(value) => {
                profile.insert("dpiStages".into(), value);
            }
            None => {
                profile.remove("dpiStages");
            }
        }
        let source = SourceData::from_document(&data).unwrap();
        assert_eq!(source.baseline().unwrap(), 100.);
        let result = confirm(&source, 1450., "measured", name(json!("Mouse"))).unwrap();
        assert!(result.stages().is_none());
    }
    for active in [0, 3] {
        let mut data = document(&[]);
        data["profiles"][0]["dpiStages"]["active"] = json!(active);
        assert!(
            SourceData::from_document(&data)
                .unwrap()
                .baseline()
                .is_err()
        );
    }
    let mut data = document(&[]);
    data["profiles"][0]["dpiStages"]["stages"][1] = Value::Null;
    assert!(
        SourceData::from_document(&data)
            .unwrap()
            .baseline()
            .is_err()
    );
}

#[test]
fn dpi_matcher_baseline_does_not_default_a_failed_document_read() {
    for data in [
        Value::Null,
        json!({}),
        json!({"activeProfile":"missing","profiles":[]}),
        json!({"activeProfile":"","profiles":[{"guid":""}]}),
        json!({"activeProfile":"current","profiles":[{"guid":"current","dpiStages":{}}]}),
    ] {
        assert!(SourceData::from_document(&data).is_err());
    }
    let mut data = document(&[]);
    data["profiles"][0]["dpiStages"]["stages"][1]["x"] = json!("broken");
    assert!(SourceData::from_document(&data).is_err());
}

#[test]
fn dpi_matcher_retain_live_requires_the_same_profile_and_unchanged_file_fields() {
    let mut previous = record(&[]);
    let result = confirm(&previous, f64::NAN, "measured", name(json!("Mouse"))).unwrap();
    previous.apply(&Change::Stages(result.stages().unwrap().clone()));
    previous.apply(&Change::Profiles(result.response().clone()));

    let mut data = document(&[]);
    data["profiles"][0]["dpiStages"] = json!({
        "enable":true,"active":2,"count":2,"stages":[
            {"x":800,"y":900,"visible":true,"independent":true},
            {"x":null,"y":null,"visible":true,"independent":false}
        ]
    });
    data["sensitivityMatcher"] = json!({"profiles":[{
        "guid":"measured","name":"Mouse",
        "dpi":{"x":null,"y":null,"visible":true,"independent":false}
    }]});
    let mut refreshed = SourceData::from_document(&data).unwrap();
    refreshed.retain_live(&previous);
    assert!(
        refreshed.library().profiles()[0]
            .dpi()
            .unwrap()
            .x()
            .unwrap()
            .value()
            .is_nan()
    );
    assert!(
        refreshed.stages().unwrap().stages()[1]
            .x()
            .unwrap()
            .value()
            .is_nan()
    );

    data["profiles"][0]["dpiStages"]["stages"][1]["x"] = json!(900);
    let mut changed = SourceData::from_document(&data).unwrap();
    changed.retain_live(&previous);
    assert_eq!(
        changed.stages().unwrap().stages()[1].x().unwrap().value(),
        900.
    );

    data["activeProfile"] = json!("successor");
    data["profiles"][0]["guid"] = json!("successor");
    let mut successor = SourceData::from_document(&data).unwrap();
    successor.retain_live(&previous);
    assert_eq!(successor.library().profiles()[0].dpi().unwrap().x(), None);
}

#[test]
fn dpi_matcher_retain_live_does_not_hide_a_localized_name_property_reorder() {
    let ordered_name = json!({"zh-cn":"鼠标","fr":"mouse"});
    let reordered_name = json!({"fr":"mouse","zh-cn":"鼠标"});
    let mut previous = SourceData::from_document(&document(&[ordered_name.clone()])).unwrap();
    let live_library = Library::new(vec![MatcherProfile::new(
        "previous-0".into(),
        name(ordered_name.clone()),
        Some(row(f64::NAN, f64::NAN)),
    )]);
    previous.apply(&Change::Profiles(ProfileUpdate::new(
        live_library,
        Some("previous-0".into()),
    )));

    let mut same_order = document(&[ordered_name]);
    same_order["sensitivityMatcher"]["profiles"][0]["dpi"] =
        json!({"x":null,"y":null,"visible":true,"independent":false});
    let mut same_order = SourceData::from_document(&same_order).unwrap();
    same_order.retain_live(&previous);
    assert!(
        same_order.library().profiles()[0]
            .dpi()
            .unwrap()
            .x()
            .unwrap()
            .value()
            .is_nan()
    );

    let mut reordered = document(&[reordered_name]);
    reordered["sensitivityMatcher"]["profiles"][0]["dpi"] =
        json!({"x":null,"y":null,"visible":true,"independent":false});
    let mut reordered = SourceData::from_document(&reordered).unwrap();
    reordered.retain_live(&previous);
    assert_eq!(reordered.library().profiles()[0].dpi().unwrap().x(), None);
}

#[test]
fn dpi_matcher_file_library_rejects_internal_number_tags() {
    let mut data = document(&[json!("Mouse")]);
    data["sensitivityMatcher"]["profiles"][0]["dpi"]["x"] =
        json!({"nonfinite_bits":"7ff8000000000000"});
    assert!(SourceData::from_document(&data).is_err());
    let payload = json!({"sensitivityMatcher": data["sensitivityMatcher"]});
    assert!(ProfileUpdate::from_payload(&payload).is_err());

    let live = Library::new(vec![MatcherProfile::new(
        "raw".into(),
        name(json!("Mouse")),
        Some(row(f64::NAN, f64::INFINITY)),
    )]);
    assert!(Library::from_source_file(&serde_json::to_value(&live).unwrap()).is_err());
    let projected = Library::from_source_file(&live.file_value()).unwrap();
    assert_eq!(projected.profiles()[0].dpi().unwrap().x(), None);
    assert_eq!(
        projected.file_value()["profiles"][0]["dpi"]["y"],
        Value::Null
    );
}

#[test]
fn dpi_matcher_file_library_keeps_missing_and_null_dpi_distinct() {
    let library = json!({"extension":{"version":3}, "profiles":[
        {"guid":"missing","name":"Missing","extension":7},
        {"guid":"null","name":"Null","dpi":null},
        {"guid":"partial","name":"Partial","dpi":{"x":null,"axisExtension":9}}
    ]});
    let parsed = Library::from_source_file(&library).unwrap();
    assert!(parsed.profiles()[0].dpi().is_none());
    assert!(parsed.profiles()[1].dpi().unwrap().is_null());
    assert_eq!(parsed.file_value(), library);
    let update = ProfileUpdate::from_payload(&json!({"sensitivityMatcher":library})).unwrap();
    assert_eq!(update.library().file_value(), library);
}

#[test]
fn dpi_matcher_saved_count_projection_does_not_erase_live_raw_axes() {
    let mut document = document(&[]);
    document["profiles"][0]["dpiStages"]["count"] = json!(99);
    let mut live = SourceData::from_document(&document).unwrap();
    let mutation = confirm(&live, f64::NAN, "measured", name(json!("Mouse"))).unwrap();
    let change = Change::Stages(mutation.stages().unwrap().clone());
    live.apply(&change);
    document["profiles"][0]["dpiStages"] = mutation.stages().unwrap().stages().source_file_value();
    assert_eq!(document["profiles"][0]["dpiStages"]["count"], 2);
    assert_eq!(
        document["profiles"][0]["dpiStages"]["stages"][1]["x"],
        Value::Null
    );
    let mut current = SourceData::from_document(&document).unwrap();
    current.retain_live(&live);
    assert!(
        current.stages().unwrap().stages()[1]
            .x()
            .unwrap()
            .value()
            .is_nan()
    );
}

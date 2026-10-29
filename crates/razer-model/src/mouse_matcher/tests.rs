use super::*;
use razer_device::{
    dpi_matcher_storage::{Library, MatcherProfile, ProfileName, ProfileUpdate, StageUpdate},
    mouse_current_dpi::{SourceNumber, SourceStage, SourceStages},
};

fn device_for(product_id: u32) -> Device {
    let mut device = crate::test_devices::with_profile(product_id);
    device.active_profile = device.profiles[0].id.clone();
    device.profiles[0].source_settings = Some(json!({
        "guid": device.profiles[0].guid,
        "dpiStages":{"active":1,"enable":true,"count":1,"stageExtension":"keep",
            "stages":[{"x":800,"y":800,"visible":true,"independent":false,"rowExtension":3}]},
        "pollingRate":1000,"extension":{"keep":true}
    }));
    device.source_device_settings = Some(json!({"version":7,"deviceExtension":"keep"}));
    device
}

fn device() -> Device {
    device_for(190)
}

fn stage_change(device: &Device, selected: Option<&str>) -> Change {
    let mut stages = device.profiles[0].source_settings.as_ref().unwrap()["dpiStages"].clone();
    stages["stages"][0]["x"] = json!(1200);
    Change::Stages(StageUpdate::new(
        SourceStages::from_source_file(&stages).unwrap(),
        selected.map(str::to_owned),
    ))
}

fn profile_change(selected: Option<&str>) -> Change {
    let library: Library = serde_json::from_value(json!({
        "profiles":[{"guid":"measured","name":{"en":"Observed mouse","zh-cn":"Observed mouse"},
            "dpi":{"x":1200,"y":1200,"visible":true,"independent":false,"dpiExtension":5},
            "profileExtension":"keep"}],
        "libraryExtension":{"keep":true}
    }))
    .unwrap();
    Change::Profiles(ProfileUpdate::new(library, selected.map(str::to_owned)))
}

fn mutation(device: &Device, change: &Change) -> Mutation {
    Mutation::from_change(&source_record(device).unwrap(), change).unwrap()
}

fn assert_atomic_failure(mutation: &Mutation, device: &mut Device) {
    let before = serde_json::to_value(&*device).unwrap();
    assert!(mutation.apply(device).is_err());
    assert_eq!(serde_json::to_value(device).unwrap(), before);
}

#[test]
fn mouse_matcher_stage_task_preserves_parallel_library_and_unrelated_settings() {
    let mut device = device();
    let mutation = mutation(&device, &stage_change(&device, Some("measured")));
    assert!(!mutation.is_profile_update());
    device.profiles[0].source_settings.as_mut().unwrap()["pollingRate"] = json!(500);
    let parallel_library = json!({"profiles":[],"parallelExtension":true});
    let root = device.source_device_settings.as_mut().unwrap();
    root["sensitivityMatcher"] = parallel_library.clone();
    root["version"] = json!(9);
    mutation.apply(&mut device).unwrap();
    let source = device.profiles[0].source_settings.as_ref().unwrap();
    assert_eq!(source["pollingRate"], 500);
    assert_eq!(source["extension"], json!({"keep":true}));
    assert_eq!(source["selectedSensitivityMatcher"], "measured");
    assert_eq!(
        source["dpiStages"],
        json!({"active":1,"enable":true,"count":1,
        "stageExtension":"keep","stages":[{"x":1200,"y":800,"visible":true,
        "independent":false,"rowExtension":3}]})
    );
    let root = device.source_device_settings.unwrap();
    assert_eq!(root["sensitivityMatcher"], parallel_library);
    assert_eq!(root["deviceExtension"], "keep");
    assert_eq!(root["version"], 10);
}

#[test]
fn mouse_matcher_profile_task_preserves_parallel_stages_and_all_extension_fields() {
    let mut device = device();
    let change = profile_change(Some("measured"));
    let mutation = mutation(&device, &change);
    assert!(mutation.is_profile_update());
    device.profiles[0].source_settings.as_mut().unwrap()["dpiStages"]["stages"][0]["x"] =
        json!(1600);
    let expected_stages = device.profiles[0].source_settings.as_ref().unwrap()["dpiStages"].clone();
    mutation.apply(&mut device).unwrap();
    let source = device.profiles[0].source_settings.as_ref().unwrap();
    assert_eq!(source["dpiStages"], expected_stages);
    assert_eq!(source["pollingRate"], 1000);
    assert_eq!(source["extension"], json!({"keep":true}));
    assert_eq!(source["selectedSensitivityMatcher"], "measured");
    let root = device.source_device_settings.unwrap();
    assert_eq!(root["version"], 8);
    assert_eq!(root["deviceExtension"], "keep");
    assert_eq!(
        root["sensitivityMatcher"]["libraryExtension"],
        json!({"keep":true})
    );
    let profile = &root["sensitivityMatcher"]["profiles"][0];
    assert_eq!(profile["profileExtension"], "keep");
    assert_eq!(profile["dpi"]["dpiExtension"], 5);
    assert_eq!(profile["dpi"]["x"], 1200);
}

#[test]
fn mouse_matcher_independent_stage_and_profile_tasks_each_advance_version_once() {
    let mut device = device();
    mutation(&device, &stage_change(&device, Some("measured")))
        .apply(&mut device)
        .unwrap();
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["version"],
        8
    );
    assert!(
        device
            .source_device_settings
            .as_ref()
            .unwrap()
            .get("sensitivityMatcher")
            .is_none()
    );
    mutation(&device, &profile_change(Some("measured")))
        .apply(&mut device)
        .unwrap();
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["version"],
        9
    );
    assert_eq!(
        device.profiles[0].source_settings.as_ref().unwrap()["dpiStages"]["stages"][0]["x"],
        1200
    );
}

#[test]
fn mouse_matcher_owned_field_conflicts_and_identity_changes_are_atomic() {
    for profiles in [false, true] {
        for conflict in 0..12 {
            let mut device = device();
            let change = if profiles {
                profile_change(Some("measured"))
            } else {
                stage_change(&device, Some("measured"))
            };
            let mutation = mutation(&device, &change);
            match conflict {
                0 if profiles => {
                    device.source_device_settings.as_mut().unwrap()["sensitivityMatcher"] =
                        json!({"profiles":[]})
                }
                0 => {
                    device.profiles[0].source_settings.as_mut().unwrap()["dpiStages"]["stages"][0]
                        ["x"] = json!(1600)
                }
                1 => {
                    device.profiles[0].source_settings.as_mut().unwrap()["selectedSensitivityMatcher"] =
                        json!("other")
                }
                2 => device.profiles.clear(),
                3 => device.profiles[0].id = "replacement-profile".into(),
                4 => device.profiles[0].guid = "replacement-guid".into(),
                5 => device.device_container_id = "different-device".into(),
                6 => device.product_id = 192,
                7 => device.source_device_settings.as_mut().unwrap()["version"] = Value::Null,
                8 => device.source_device_settings.as_mut().unwrap()["version"] = json!("8"),
                9 => device.active_profile = "different-active-profile".into(),
                10 => {
                    device.profiles[0].source_settings.as_mut().unwrap()["guid"] =
                        json!("different-source-guid")
                }
                _ => {
                    device.source_device_settings.as_mut().unwrap()["version"] =
                        json!(9_007_199_254_740_992_u64)
                }
            }
            assert_atomic_failure(&mutation, &mut device);
        }
    }
}

#[test]
fn mouse_matcher_stage_without_selection_preserves_a_parallel_selection() {
    let mut device = device();
    let mutation = mutation(&device, &stage_change(&device, None));
    device.profiles[0].source_settings.as_mut().unwrap()["selectedSensitivityMatcher"] =
        json!("parallel");
    mutation.apply(&mut device).unwrap();
    assert_eq!(
        device.profiles[0].source_settings.as_ref().unwrap()["selectedSensitivityMatcher"],
        "parallel"
    );
    assert_eq!(
        device.profiles[0].source_settings.as_ref().unwrap()["dpiStages"]["stages"][0]["x"],
        1200
    );
}

#[test]
fn mouse_matcher_selection_absence_and_empty_text_follow_each_task_contract() {
    for profiles in [false, true] {
        for selected in [None, Some("")] {
            let mut device = device();
            device.profiles[0].source_settings.as_mut().unwrap()["selectedSensitivityMatcher"] =
                json!("previous");
            let change = if profiles {
                profile_change(selected)
            } else {
                stage_change(&device, selected)
            };
            mutation(&device, &change).apply(&mut device).unwrap();
            let actual = device.profiles[0]
                .source_settings
                .as_ref()
                .unwrap()
                .get("selectedSensitivityMatcher");
            match (profiles, selected) {
                (true, None) => assert_eq!(actual, None),
                (false, None) => assert_eq!(actual, Some(&json!("previous"))),
                (_, Some(_)) => assert_eq!(actual, Some(&json!(""))),
            }
        }
    }
}

#[test]
fn mouse_matcher_owned_null_fields_are_distinct_from_absent_fields() {
    for field in [LIBRARY_FIELD, SELECTED_FIELD, STAGES_FIELD] {
        let mut device = device();
        if field == LIBRARY_FIELD {
            device.source_device_settings.as_mut().unwrap()[field] = Value::Null;
        } else {
            device.profiles[0].source_settings.as_mut().unwrap()[field] = Value::Null;
        }
        let change = if field == STAGES_FIELD {
            let stages = SourceStages::new(
                true,
                1,
                vec![SourceStage::new(
                    Some(1200.into()),
                    Some(1200.into()),
                    Some(true),
                    Some(false),
                )],
            );
            Change::Stages(StageUpdate::new(stages, Some("measured".into())))
        } else {
            profile_change(Some("measured"))
        };
        let mutation = mutation(&device, &change);
        let mut unchanged = device.clone();
        mutation.apply(&mut unchanged).unwrap();
        if field == LIBRARY_FIELD {
            device
                .source_device_settings
                .as_mut()
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(field);
        } else {
            device.profiles[0]
                .source_settings
                .as_mut()
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(field);
        }
        assert_atomic_failure(&mutation, &mut device);
    }
}

#[test]
fn mouse_matcher_nonfinite_stage_numbers_are_projected_only_at_the_file_boundary() {
    let mut device = device();
    let nan = SourceNumber::new(f64::NAN);
    let infinity = SourceNumber::new(f64::INFINITY);
    let change = Change::Stages(StageUpdate::new(
        SourceStages::new(
            true,
            1,
            vec![SourceStage::new(
                Some(nan),
                Some(infinity),
                Some(true),
                Some(false),
            )],
        ),
        None,
    ));
    mutation(&device, &change).apply(&mut device).unwrap();
    let stages = &device.profiles[0].source_settings.as_ref().unwrap()["dpiStages"];
    assert_eq!(
        stages,
        &json!({"enable":true,"active":1,"count":1,"stages":[{"x":null,"y":null,"visible":true,"independent":false}]})
    );
    let Change::Stages(update) = &change else {
        unreachable!()
    };
    assert_eq!(update.stages().stages()[0].x(), Some(nan));
    assert_eq!(update.stages().stages()[0].y(), Some(infinity));
}

#[test]
fn mouse_matcher_stage_task_recounts_only_explicitly_visible_rows() {
    let mut device = device();
    let stages = SourceStages::from_source_file(&json!({
        "active":1,"enable":true,"count":99,"stages":[
            {"x":800,"y":800,"visible":true},
            {"x":1600,"y":1600,"visible":false},
            {"x":3200,"y":3200},null
        ]
    }))
    .unwrap();
    let change = Change::Stages(StageUpdate::new(stages, None));
    mutation(&device, &change).apply(&mut device).unwrap();
    let stages = &device.profiles[0].source_settings.as_ref().unwrap()["dpiStages"];
    assert_eq!(stages["count"], 1);
    assert_eq!(stages["stages"].as_array().unwrap().len(), 4);
    assert_eq!(stages["stages"][3], Value::Null);
}

#[test]
fn mouse_matcher_profile_task_projects_live_nonfinite_values_without_ipc_tags() {
    let mut device = device();
    let number = SourceNumber::new(f64::NEG_INFINITY);
    let profile = MatcherProfile::new(
        "measured".into(),
        ProfileName::Text("Observed mouse".into()),
        Some(SourceStage::new(
            Some(number),
            Some(number),
            Some(true),
            Some(false),
        )),
    );
    let change = Change::Profiles(ProfileUpdate::new(
        Library::new(vec![profile]),
        Some("measured".into()),
    ));
    mutation(&device, &change).apply(&mut device).unwrap();
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["sensitivityMatcher"]["profiles"][0]["dpi"],
        json!({"x":null,"y":null,"visible":true,"independent":false})
    );
    let Change::Profiles(update) = &change else {
        unreachable!()
    };
    assert_eq!(update.profiles()[0].dpi().unwrap().x(), Some(number));
}

#[test]
fn mouse_matcher_rejects_failed_stage_preparation_without_blocking_the_library_task() {
    let mut device = device();
    device.source_device_settings.as_mut().unwrap()[LIBRARY_FIELD] = json!({"profiles":[
        {"guid":"incomplete","name":"Missing DPI"},
        {"guid":"match","name":"Observed mouse","dpi":{"x":1200,"y":1200,"visible":true,"independent":false}}
    ]});
    let record = source_record(&device).unwrap();
    let source =
        razer_device::dpi_matcher_storage::SourceData::from_document(record.document()).unwrap();
    let selected = razer_device::dpi_matcher_storage::select(&source, "match").unwrap();
    let stages = selected.stages().unwrap().clone();
    assert!(stages.validate().is_err());
    let before = serde_json::to_value(&device).unwrap();
    assert!(Mutation::from_change(&record, &Change::Stages(stages)).is_err());
    assert_eq!(serde_json::to_value(&device).unwrap(), before);

    let stages_before = device.profiles[0].source_settings.as_ref().unwrap()[STAGES_FIELD].clone();
    Mutation::from_change(&record, &Change::Profiles(selected.response().clone()))
        .unwrap()
        .apply(&mut device)
        .unwrap();
    assert_eq!(
        device.profiles[0].source_settings.as_ref().unwrap()[STAGES_FIELD],
        stages_before
    );
    assert_eq!(
        device.profiles[0].source_settings.as_ref().unwrap()[SELECTED_FIELD],
        "match"
    );
    let root = device.source_device_settings.as_ref().unwrap();
    assert_eq!(root[VERSION_FIELD], 8);
    assert_eq!(root[LIBRARY_FIELD]["profiles"].as_array().unwrap().len(), 2);
}

#[test]
fn mouse_matcher_version_absence_and_maximum_follow_the_source_counter() {
    for (previous, expected) in [(None, 1), (Some(9_007_199_254_740_991_u64), 0)] {
        let mut device = device();
        let root = device
            .source_device_settings
            .as_mut()
            .unwrap()
            .as_object_mut()
            .unwrap();
        if let Some(previous) = previous {
            root.insert(VERSION_FIELD.into(), json!(previous));
        } else {
            root.remove(VERSION_FIELD);
        }
        mutation(&device, &profile_change(None))
            .apply(&mut device)
            .unwrap();
        assert_eq!(
            device.source_device_settings.as_ref().unwrap()[VERSION_FIELD],
            expected
        );
    }
}

#[test]
fn mouse_matcher_source_rejects_wrong_product_and_inconsistent_profile_guid() {
    let mut device = device();
    device.product_id = 70;
    assert!(source_record(&device).is_err());
    device.product_id = 190;
    device.profiles[0].source_settings.as_mut().unwrap()["guid"] = json!("different");
    assert!(source_record(&device).is_err());
}

#[test]
fn mouse_matcher_source_and_both_tasks_retain_each_audited_product_identity() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut device = device_for(product_id);
        let identity = device.workspace_identity();
        mutation(&device, &stage_change(&device, Some("measured")))
            .apply(&mut device)
            .unwrap();
        assert_eq!(
            device.source_device_settings.as_ref().unwrap()["version"],
            8
        );
        mutation(&device, &profile_change(Some("measured")))
            .apply(&mut device)
            .unwrap();
        assert_eq!(
            device.source_device_settings.as_ref().unwrap()["version"],
            9
        );
        assert_eq!(device.product_id, product_id);
        assert_eq!(device.workspace_identity(), identity);
    }
}

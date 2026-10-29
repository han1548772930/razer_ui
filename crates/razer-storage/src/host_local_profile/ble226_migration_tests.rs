use super::*;
use serde_json::json;

// Literal expectations follow MW92148.Cn in the current226 738e48c31 snapshot.
fn run_step(step: Step, source: Value) -> Value {
    let mut candidate = Candidate::parse(source.as_object().unwrap().clone()).unwrap();
    step.apply(&mut candidate).unwrap();
    candidate.into_value()
}

#[test]
fn ble226_migration_01_moves_defined_stream_fields_only_when_group_is_missing() {
    assert_eq!(
        run_step(
            Step::StreamMixer,
            json!({"keep":null,"profiles":[
                {"guid":"a","isStreamMixerEnabled":false,"streamMixVolume":null,
                 "playbackMixVolume":0,"isStreamMixMonitor":false,"playbackMixDevice":"",
                 "otherCategories":[],"keep":9},
                {"guid":"b","streamMixerSettings":null,"streamMixVolume":8},
                {"guid":"c","streamMixerSettings":{},"streamMixVolume":4},
                {"guid":"d"}
            ]})
        ),
        json!({"keep":null,"profiles":[
            {"guid":"a","keep":9,"streamMixerSettings":{
                "isStreamMixerEnabled":false,"streamMixVolume":null,"playbackMixVolume":0,
                "isStreamMixMonitor":false,"playbackMixDevice":"","otherCategories":[]}},
            {"guid":"b","streamMixerSettings":null,"streamMixVolume":8},
            {"guid":"c","streamMixerSettings":{},"streamMixVolume":4},
            {"guid":"d"}
        ]})
    );
}

#[test]
fn ble226_migration_02_removes_profile_volume_including_null_and_zero() {
    assert_eq!(
        run_step(
            Step::RemoveVolume,
            json!({"volume":9,"micVolume":null,"profiles":[
                {"guid":"a","volume":0,"micVolume":null,"keep":false},
                {"guid":"b","volume":false,"micVolume":"","keep":3},
                {"guid":"c"}
            ]})
        ),
        json!({"volume":9,"micVolume":null,"profiles":[
            {"guid":"a","keep":false},{"guid":"b","keep":3},{"guid":"c"}
        ]})
    );
}

#[test]
fn ble226_migration_03_moves_only_truthy_numeric_boss_key_and_overwrites_profiles() {
    assert_eq!(
        run_step(
            Step::MoveBossKey,
            json!({"bossKey":-7.5,"profiles":[
                {"guid":"a","bossKey":null},{"guid":"b","bossKey":0}
            ]})
        ),
        json!({"profiles":[{"guid":"a","bossKey":-7.5},{"guid":"b","bossKey":-7.5}]})
    );
    for unchanged in [
        json!({"profiles":[{"bossKey":4}]}),
        json!({"bossKey":0,"profiles":[{"bossKey":4}]}),
        json!({"bossKey":0.0,"profiles":[{"bossKey":4}]}),
        json!({"bossKey":null,"profiles":[{"bossKey":4}]}),
        json!({"bossKey":false,"profiles":[{"bossKey":4}]}),
        json!({"bossKey":"7","profiles":[{"bossKey":4}]}),
        json!({"bossKey":[],"profiles":[{"bossKey":4}]}),
        json!({"bossKey":{},"profiles":[{"bossKey":4}]}),
    ] {
        assert_eq!(run_step(Step::MoveBossKey, unchanged.clone()), unchanged);
    }
}

#[test]
fn ble226_migration_04_removes_only_truthy_numeric_root_indicator() {
    assert_eq!(
        run_step(
            Step::RemoveIndicatorLed,
            json!({"indicatorLedStatus":-1,"profiles":[{"indicatorLedStatus":7}]})
        ),
        json!({"profiles":[{"indicatorLedStatus":7}]})
    );
    for value in [
        json!(0),
        json!(-0.0),
        Value::Null,
        json!(false),
        json!("1"),
        json!([]),
        json!({}),
    ] {
        let source = json!({"indicatorLedStatus":value,"profiles":[]});
        assert_eq!(run_step(Step::RemoveIndicatorLed, source.clone()), source);
    }
}

#[test]
fn ble226_migration_05_adopts_first_defined_microphone_value_without_overwriting_root_null() {
    assert_eq!(
        run_step(
            Step::MoveMicrophoneSettings,
            json!({"samplingRate":null,"profiles":[
                {"guid":"a","samplingRate":48000,"isHighPassFilterEnabled":false,
                 "isAnalogGainLimiterEnabled":null,"keep":1},
                {"guid":"b","samplingRate":96000,"isHighPassFilterEnabled":true,
                 "isAnalogGainLimiterEnabled":true}
            ]})
        ),
        json!({"samplingRate":null,"isHighPassFilterEnabled":false,
            "isAnalogGainLimiterEnabled":null,"profiles":[{"guid":"a","keep":1},{"guid":"b"}]})
    );
}

#[test]
fn ble226_migration_06_filters_only_exact_none_matcher_name_and_preserves_order() {
    assert_eq!(
        run_step(
            Step::RemoveDpiMatcherNoneProfile,
            json!({"sensitivityMatcher":{"keep":8,"profiles":[
                {"name":"None","guid":"discard"},{"name":"none","guid":"a"},
                {"guid":"unnamed"},{"name":null},{"name":"None "},{"name":"B","keep":false}
            ]},"profiles":[]})
        ),
        json!({"sensitivityMatcher":{"keep":8,"profiles":[
            {"name":"none","guid":"a"},{"guid":"unnamed"},{"name":null},
            {"name":"None "},{"name":"B","keep":false}
        ]},"profiles":[]})
    );
    assert_eq!(
        run_step(Step::RemoveDpiMatcherNoneProfile, json!({"profiles":[]})),
        json!({"profiles":[]})
    );
}

#[test]
fn ble226_migration_07_removes_root_preview_even_when_falsy_and_keeps_nested_field() {
    for value in [Value::Null, json!(false), json!(0), json!("")] {
        assert_eq!(
            run_step(
                Step::RemoveCameraPreview,
                json!({"isPreview":value,"profiles":[{"camera":{"isPreview":false}}]})
            ),
            json!({"profiles":[{"camera":{"isPreview":false}}]})
        );
    }
}

#[test]
fn ble226_migration_08_replaces_root_rapid_trigger_and_moves_truthiness_from_entries() {
    assert_eq!(
        run_step(
            Step::RemoveContinuousRapidTriggerPerKey,
            json!({"activeProfile":"b","analogData":{"keep":9,
                "rapidTrigger":{"continuous":false,"discard":true}},"profiles":[
                {"guid":"a","appEngine":{"keep":true,"rapidTriggers":{
                    "1":{"continuous":[],"keep":1},"2":{"continuous":0},"3":{}}}},
                {"guid":"b","appEngine":{"rapidTriggers":[
                    {"continuous":null,"keep":4},{"continuous":"yes"}]}},
                {"guid":"c","appEngine":null,"continuousRapidTrigger":"untouched"}
            ]})
        ),
        json!({"activeProfile":"b","analogData":{"keep":9,
            "rapidTrigger":{"continuous":true}},"profiles":[
            {"guid":"a","continuousRapidTrigger":true,"appEngine":{"keep":true,
                "rapidTriggers":{"1":{"keep":1},"2":{},"3":{}}}},
            {"guid":"b","continuousRapidTrigger":true,
                "appEngine":{"rapidTriggers":[{"keep":4},{}]}},
            {"guid":"c","appEngine":null,"continuousRapidTrigger":"untouched"}
        ]})
    );
    assert_eq!(
        run_step(
            Step::RemoveContinuousRapidTriggerPerKey,
            json!({"activeProfile":"missing","analogData":{},"profiles":[
                {"guid":"a","appEngine":{"rapidTriggers":{"1":{"continuous":true}}}}
            ]})
        ),
        json!({"activeProfile":"missing","analogData":{"rapidTrigger":{"continuous":false}},
            "profiles":[{"guid":"a","continuousRapidTrigger":true,
                "appEngine":{"rapidTriggers":{"1":{}}}}]})
    );
    for source in [
        json!({"profiles":[{"appEngine":{"rapidTriggers":null}}]}),
        json!({"analogData":null,"profiles":[{"appEngine":{"rapidTriggers":null}}]}),
        json!({"analogData":false,"profiles":[{"appEngine":{"rapidTriggers":null}}]}),
        json!({"analogData":0,"profiles":[{"appEngine":{"rapidTriggers":null}}]}),
        json!({"analogData":"","profiles":[{"appEngine":{"rapidTriggers":null}}]}),
    ] {
        assert_eq!(
            run_step(Step::RemoveContinuousRapidTriggerPerKey, source.clone()),
            source
        );
    }
}

#[test]
fn ble226_migration_09_wraps_object_and_null_key_list_but_preserves_arrays() {
    assert_eq!(
        run_step(
            Step::SnapTapKeyListArray,
            json!({"profiles":[
                {"snapTap":{"keyList":{"key":"A"},"keep":1}},
                {"snapTap":{"keyList":null}},
                {"snapTap":{"keyList":[{"id":0}]}},
                {"snapTap":{"keyList":7}},
                {"snapTap":null}
            ]})
        ),
        json!({"profiles":[
            {"snapTap":{"keyList":[{"key":"A"}],"keep":1}},
            {"snapTap":{"keyList":[null]}},
            {"snapTap":{"keyList":[{"id":0}]}},
            {"snapTap":{"keyList":7}},
            {"snapTap":null}
        ]})
    );
}

#[test]
fn ble226_migration_10_renumbers_all_entries_only_when_any_id_is_missing() {
    assert_eq!(
        run_step(
            Step::AddSnapTapKeyListId,
            json!({"profiles":[
                {"snapTap":{"keyList":[{"id":8,"key":"A"},{"key":"B"},{"id":null}]}},
                {"snapTap":{"keyList":[{"id":null},{"id":0},{"id":false}]}},
                {"snapTap":{"keyList":[]}}
            ]})
        ),
        json!({"profiles":[
            {"snapTap":{"keyList":[{"id":1,"key":"A"},{"id":2,"key":"B"},{"id":3}]}},
            {"snapTap":{"keyList":[{"id":null},{"id":0},{"id":false}]}},
            {"snapTap":{"keyList":[]}}
        ]})
    );
}

#[test]
fn ble226_migration_11_removes_group_and_falls_through_to_renumbering() {
    assert_eq!(
        run_step(
            Step::RemoveStreamMixerSettings,
            json!({"streamMixerSettings":{"root":true},"profiles":[
                {"streamMixerSettings":null,"snapTap":{"keyList":[{"id":8},{"key":"B"}]}},
                {"streamMixerSettings":{},"snapTap":false}
            ]})
        ),
        json!({"streamMixerSettings":{"root":true},"profiles":[
            {"snapTap":{"keyList":[{"id":1},{"id":2,"key":"B"}]}},{"snapTap":false}
        ]})
    );
}

#[test]
fn ble226_migration_12_renames_defined_fn_sleep_value_and_overwrites_new_value() {
    assert_eq!(
        run_step(
            Step::GamingModeFnSleep,
            json!({"profiles":[
                {"gamingMode":{"isFnZDisabled":false,"isFnSleepEnabled":true,"keep":9}},
                {"gamingMode":{"isFnZDisabled":null,"isFnSleepEnabled":false}},
                {"gamingMode":{"isFnSleepEnabled":7}},{"gamingMode":null},
                {"gamingMode":false},{"gamingMode":[]},{"gamingMode":"text"}
            ]})
        ),
        json!({"profiles":[
            {"gamingMode":{"isFnSleepEnabled":false,"keep":9}},
            {"gamingMode":{"isFnSleepEnabled":null}},
            {"gamingMode":{"isFnSleepEnabled":7}},{"gamingMode":null},
            {"gamingMode":false},{"gamingMode":[]},{"gamingMode":"text"}
        ]})
    );
}

#[test]
fn ble226_migration_13_keeps_only_strict_camera_numeric_values() {
    for value in [
        json!(0),
        json!(1),
        json!(2),
        json!(3),
        json!(1.0),
        json!(-0.0),
    ] {
        let source = json!({"profiles":[{"camera":{"antiFlicker":value,"keep":9}}]});
        assert_eq!(run_step(Step::CameraAntiFlicker, source.clone()), source);
    }
    for value in [
        Value::Null,
        json!(false),
        json!(true),
        json!("1"),
        json!(4),
        json!(-1),
        json!(0.5),
        json!([]),
        json!({}),
    ] {
        assert_eq!(
            run_step(
                Step::CameraAntiFlicker,
                json!({"profiles":[{"camera":{"antiFlicker":value,"keep":9}}]})
            ),
            json!({"profiles":[{"camera":{"antiFlicker":0,"keep":9}}]})
        );
    }
    assert_eq!(
        run_step(Step::CameraAntiFlicker, json!({"profiles":[{"camera":{}}]})),
        json!({"profiles":[{"camera":{"antiFlicker":0}}]})
    );
    for source in [
        json!({"profiles":[{}]}),
        json!({"profiles":[{"camera":null}]}),
        json!({"profiles":[{"camera":false}]}),
        json!({"profiles":[{"camera":0}]}),
        json!({"profiles":[{"camera":""}]}),
    ] {
        assert_eq!(run_step(Step::CameraAntiFlicker, source.clone()), source);
    }
}

#[test]
fn ble226_migration_missing_null_and_zero_schema_start_at_one() {
    let expected =
        json!({"schemaVersion":13,"profiles":[{"gamingMode":{"isFnSleepEnabled":null}}]});
    for source in [
        json!({"profiles":[{"volume":null,"gamingMode":{"isFnZDisabled":null}}]}),
        json!({"schemaVersion":null,"profiles":[{"volume":null,"gamingMode":{"isFnZDisabled":null}}]}),
        json!({"schemaVersion":0,"profiles":[{"volume":null,"gamingMode":{"isFnZDisabled":null}}]}),
    ] {
        assert_eq!(migrate(source).unwrap(), expected);
    }
}

#[test]
fn ble226_migration_resumes_after_stored_step_and_runs_eleven_fallthrough() {
    assert_eq!(
        migrate(json!({"schemaVersion":10,"profiles":[{
            "volume":null,"isStreamMixerEnabled":true,"streamMixerSettings":null,
            "snapTap":{"keyList":[{"id":7},{"key":"A"}]},
            "gamingMode":{"isFnZDisabled":false},"camera":{"antiFlicker":"2"}
        }]}))
        .unwrap(),
        json!({"schemaVersion":13,"profiles":[{
            "volume":null,"isStreamMixerEnabled":true,
            "snapTap":{"keyList":[{"id":1},{"id":2,"key":"A"}]},
            "gamingMode":{"isFnSleepEnabled":false},"camera":{"antiFlicker":0}
        }]})
    );
    assert_eq!(
        migrate(json!({"schemaVersion":12,"profiles":[{
            "volume":3,"streamMixerSettings":null,"snapTap":{"keyList":null},
            "gamingMode":{"isFnZDisabled":true},"camera":{"antiFlicker":false}
        }]}))
        .unwrap(),
        json!({"schemaVersion":13,"profiles":[{
            "volume":3,"streamMixerSettings":null,"snapTap":{"keyList":null},
            "gamingMode":{"isFnZDisabled":true},"camera":{"antiFlicker":0}
        }]})
    );
    for schema in 0..=12 {
        assert_eq!(
            migrate(json!({"schemaVersion":schema,"profiles":[]})).unwrap(),
            json!({"schemaVersion":13,"profiles":[]})
        );
    }
}

#[test]
fn ble226_migration_current_thirteen_is_zero_steps_even_with_legacy_extensions() {
    let source = json!({"schemaVersion":13,"bossKey":3,"isPreview":null,
    "sensitivityMatcher":null,"analogData":{},"profiles":[{
        "volume":null,"streamMixerSettings":null,"snapTap":{"keyList":null},
        "gamingMode":{"isFnZDisabled":true},"camera":{"antiFlicker":"bad"}
    }]});
    assert_eq!(migrate(source.clone()).unwrap(), source);
}

#[test]
fn ble226_migration_rejects_invalid_schema_and_document_shapes() {
    for version in [
        json!(14),
        json!(u64::MAX),
        json!(-1),
        json!(1.5),
        json!("12"),
        json!(false),
        json!([]),
        json!({}),
    ] {
        let source = json!({"schemaVersion":version,"profiles":[]});
        assert!(migrate(source.clone()).is_err(), "accepted {source}");
    }
    for source in [
        Value::Null,
        json!([]),
        json!("document"),
        json!({}),
        json!({"profiles":null}),
        json!({"profiles":{}}),
        json!({"profiles":[null]}),
        json!({"profiles":[7]}),
    ] {
        assert!(migrate(source.clone()).is_err(), "accepted {source}");
    }
}

#[test]
fn ble226_migration_rejects_accessed_bad_shapes_and_keeps_failed_candidate_private() {
    for source in [
        json!({"schemaVersion":5,"sensitivityMatcher":null,"profiles":[]}),
        json!({"schemaVersion":5,"sensitivityMatcher":{},"profiles":[]}),
        json!({"schemaVersion":5,"sensitivityMatcher":{"profiles":null},"profiles":[]}),
        json!({"schemaVersion":5,"sensitivityMatcher":{"profiles":[null]},"profiles":[]}),
        json!({"schemaVersion":7,"analogData":[],"profiles":[]}),
        json!({"schemaVersion":7,"analogData":{},"profiles":[{"appEngine":{"rapidTriggers":null}}]}),
        json!({"schemaVersion":7,"analogData":{},"profiles":[{"appEngine":{"rapidTriggers":{"A":null}}}]}),
        json!({"schemaVersion":8,"profiles":[{"snapTap":{"keyList":null}}]}),
        json!({"schemaVersion":8,"profiles":[{"snapTap":{"keyList":7}}]}),
        json!({"schemaVersion":9,"profiles":[{"snapTap":{}}]}),
        json!({"schemaVersion":9,"profiles":[{"snapTap":{"keyList":[null]}}]}),
        json!({"schemaVersion":10,"profiles":[{"streamMixerSettings":null,"snapTap":{"keyList":null}}]}),
        json!({"schemaVersion":12,"profiles":[{"camera":true}]}),
    ] {
        let before = source.clone();
        assert!(migrate(source.clone()).is_err(), "accepted {source}");
        assert_eq!(source, before);
    }
    let source = json!({"schemaVersion":0,"bossKey":7,"isPreview":false,
        "sensitivityMatcher":null,"profiles":[{"volume":null,"streamMixVolume":4}]});
    let before = source.clone();
    assert!(migrate(source.clone()).is_err());
    assert_eq!(source, before);
    assert_eq!(source["schemaVersion"], 0);
    assert_eq!(source["profiles"][0]["streamMixVolume"], 4);
    assert!(source["profiles"][0].get("streamMixerSettings").is_none());
}

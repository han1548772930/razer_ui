use super::*;
#[test]
fn source_product_name_uses_current_edition_locale_and_has_no_name_fallback() {
    assert_eq!(
        product_name(1422, 0, "zh-cn").unwrap(),
        "Razer 魔音海妖V3专业版"
    );
    for edition in [0, 128, 129] {
        assert_eq!(
            product_name(1446, edition, "zh-cn").unwrap(),
            "Razer 魔音海妖\u{a0}V3\u{a0}幻彩版\u{a0}32\u{a0}位\u{a0}DSP"
        );
        assert_eq!(
            product_name(1446, edition, "en").unwrap(),
            "Razer Seiren V3 Chroma 32-Bit DSP"
        );
    }
    assert!(product_name(1422, 128, "en").is_err());
    assert!(product_name(1446, 0, "zh-CN").is_err());
    assert!(product_name(1446, 0, "").is_err());
    assert!(product_name(1446, 0, "unknown").is_err());
    assert!(product_name(9999, 0, "en").is_err());
}

fn cache() -> Value {
    prepare_cache(None, 1422, "first").unwrap().0
}
fn action(event: &str, payload: Value) -> Action {
    Action::new(event, payload).unwrap()
}
#[test]
fn shared_local_cache_adds_current_device_without_replacing_presets_or_other_devices() {
    let mut original = cache();
    original["unknown"] = json!({"retained":true});
    original["presets"][0]["name"] = json!("本地预设");
    let (second, changed) = prepare_cache(Some(original.clone()), 1446, "unused").unwrap();
    assert!(changed);
    assert_eq!(second["presets"], original["presets"]);
    assert_eq!(second["activePresetGuid"], "first");
    assert_eq!(second["unknown"], original["unknown"]);
    assert_eq!(second["devices"].as_array().unwrap().len(), 2);
    let (same, changed) = prepare_cache(Some(second.clone()), 1446, "unused").unwrap();
    assert!(!changed);
    assert_eq!(same, second);
    assert!(second.get("samplingRate").is_none());
    assert!(second["presets"][0].get("samplingRate").is_none());
}
#[test]
fn source_control_persistence_strips_only_ephemeral_toggle_and_preserves_version_wrap() {
    let mut value = cache();
    value["version"] = json!(9_007_199_254_740_991u64);
    let edit = action(
        "ON_SET_STREAM_MIX_VOLUME",
        json!({"streamMixVolume":{"value":39,"isEnabled":false,"isToggle":true}}),
    );
    apply_cache(&mut value, 1422, &edit, "unused").unwrap();
    assert_eq!(value["version"], 0);
    assert_eq!(
        active_preset(&value).unwrap()["outputChannel"]["streamMixVolume"],
        json!({"value":39,"isEnabled":false})
    );
    assert_eq!(
        action_operations(1422, &edit).unwrap(),
        vec![Operation::Native(NativeOperation::SetDeviceMute {
            device_id: 131073,
            muted: true
        })]
    );
}
#[test]
fn source_add_rename_select_and_reset_have_distinct_version_and_identity_semantics() {
    let mut value = cache();
    apply_cache(
        &mut value,
        1422,
        &action(
            "ON_ADD_STREAM_MIXER_PRESET",
            json!({"addSteamMixerPreset":{"newPreset":{"guid":"added","name":"Added"}}}),
        ),
        "default-generated",
    )
    .unwrap();
    assert_eq!(value["version"], 1);
    assert_eq!(value["activePresetGuid"], "added");
    apply_cache(
        &mut value,
        1422,
        &action(
            "ON_RENAME_STREAM_MIXER_PRESET",
            json!({"renameSteamMixerPreset":{"selectedPresetGuid":"added","newName":"Renamed"}}),
        ),
        "unused",
    )
    .unwrap();
    assert_eq!(value["version"], 1);
    active_mut(&mut value).unwrap()["outputChannel"]["streamMixVolume"]["value"] = json!(91);
    apply_cache(
        &mut value,
        1422,
        &action(
            "ON_RESET_STREAM_MIXER_PRESET",
            json!({"resetSteamMixerPreset":{"selectedPresetGuid":"added"}}),
        ),
        "discarded",
    )
    .unwrap();
    assert_eq!(value["version"], 2);
    assert_eq!(active_preset(&value).unwrap()["guid"], "added");
    assert_eq!(active_preset(&value).unwrap()["name"], "Renamed");
    assert_eq!(
        active_preset(&value).unwrap()["outputChannel"]["streamMixVolume"]["value"],
        50
    );
    let mut only = cache();
    let before = only.clone();
    assert!(!apply_cache(&mut only,1422,&action("ON_DELETE_STREAM_MIXER_PRESET",json!({"deleteSteamMixerPreset":{"deletedPresetId":"first","selectedPresetGuid":"other"}})),"unused").unwrap());
    assert_eq!(only, before);
}
#[test]
fn preset_restore_keeps_source_crossed_input_levels_and_visibility_gates() {
    let mut preset = default_preset(1422, "preset").unwrap();
    preset["inputChannels"][0]["streamMixerInputVolume"]["value"] = json!(20);
    preset["inputChannels"][0]["playbackMixInputVolume"]["value"] = json!(70);
    let operations = preset_operations(1422, &preset).unwrap();
    assert_eq!(
        &operations[6..10],
        &[
            Operation::Native(NativeOperation::SetMixLevel {
                output_id: 131073,
                input_id: 65537,
                level: 70
            }),
            Operation::Native(NativeOperation::SetMixEnable {
                output_id: 131073,
                input_id: 65537,
                enabled: false
            }),
            Operation::Native(NativeOperation::SetMixLevel {
                output_id: 131074,
                input_id: 65537,
                level: 20
            }),
            Operation::Native(NativeOperation::SetMixEnable {
                output_id: 131074,
                input_id: 65537,
                enabled: false
            }),
        ]
    );
    assert!(
        operations.contains(&Operation::Native(NativeOperation::SetMixEnable {
            output_id: 131073,
            input_id: 131072,
            enabled: true
        }))
    );
}
#[test]
fn endpoint_lookup_uses_exact_case_first_source_match_and_never_a_usb_identifier() {
    let playback = vec![
        "Headphones (Other)".into(),
        "headphones (Razer Seiren V3 Pro)".into(),
        "Headphones (Razer Seiren V3 Pro)".into(),
        "Headphones 2 (Razer Seiren V3 Pro)".into(),
    ];
    let record = vec!["Playback Mix (Razer Seiren V3 Pro)".into()];
    assert_eq!(
        endpoint_name("Razer Seiren V3 Pro", "Headphones", &playback, &record),
        &playback[2]
    );
    assert_eq!(
        endpoint_name("Razer Seiren V3 Pro", "PlaybackMix", &playback, &record),
        &record[0]
    );
    assert_eq!(
        endpoint_name("Razer Seiren V3 Pro", "Aux1", &playback, &record),
        ""
    );
}
#[test]
fn malformed_cache_and_unknown_dispatch_do_not_become_default_success() {
    assert!(prepare_cache(Some(json!({"devices":[],"presets":[]})), 1422, "unused").is_err());
    assert!(Action::new("SET_STREAM_MIXER", json!({"isStreamMixerEnabled":true})).is_err());
    let direct = Action::new(
        "ON_ADD_STREAM_MIXER_PRESET",
        json!({"newPreset":{"name":"Raw","guid":"raw"},"version":10,"timertick":123}),
    )
    .unwrap();
    assert_eq!(
        direct.argument().unwrap(),
        &json!({"newPreset":{"name":"Raw","guid":"raw"}})
    );
}
#[test]
fn source_reorder_versions_cache_without_persisting_ui_order_or_writing_native() {
    let mut value = cache();
    let original = value["presets"].clone();
    let edit = action(
        "ON_MODIFY_INPUT_CHANNEL",
        json!({"inputChannel":{"action":"reorder","inputChannels":[{"name":"Microphone","order":2}]}}),
    );
    assert!(apply_cache(&mut value, 1422, &edit, "unused").unwrap());
    assert_eq!(value["version"], 2);
    assert_eq!(value["presets"], original);
    assert!(action_operations(1422, &edit).unwrap().is_empty());
}

#[test]
fn input_returns_keep_last_native_assignment_and_external_conjunction() {
    let disable = Operation::Native(NativeOperation::SetMixEnable {
        output_id: 131073,
        input_id: 65537,
        enabled: false,
    });
    let external = Operation::External {
        channel: "Aux1".into(),
        device: String::new(),
    };
    for verb in ["remove", "edit"] {
        let request = action(
            "ON_MODIFY_INPUT_CHANNEL",
            json!({"inputChannel":{"action":verb}}),
        );
        assert!(
            operation_result(
                &request,
                &[(disable.clone(), false), (disable.clone(), true)]
            )
            .unwrap()
        );
        assert!(
            !operation_result(
                &request,
                &[
                    (disable.clone(), true),
                    (disable.clone(), false),
                    (external.clone(), true)
                ]
            )
            .unwrap()
        );
        assert!(
            !operation_result(
                &request,
                &[(disable.clone(), true), (external.clone(), false)]
            )
            .unwrap()
        );
        assert!(!operation_result(&request, &[(external.clone(), true)]).unwrap());
    }
    let add = action(
        "ON_MODIFY_INPUT_CHANNEL",
        json!({"inputChannel":{"action":"add"}}),
    );
    assert!(
        !operation_result(
            &add,
            &[
                (disable.clone(), false),
                (disable.clone(), true),
                (external.clone(), true)
            ]
        )
        .unwrap()
    );
    assert!(operation_result(&add, &[(disable, true), (external, true)]).unwrap());
}

use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "razer-earbuds-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("local.json")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn profile(guid: &str, auto_pause: Option<bool>) -> EarbudsProfile {
    EarbudsProfile {
        guid: guid.into(),
        name: guid.into(),
        audio_auto_pause_control: auto_pause.map(|is_enabled| AutoPauseControl { is_enabled }),
        noise_cancellation: None,
        extra: Map::new(),
    }
}

#[test]
fn default_store_matches_source_default_profile() {
    let store = EarbudsProfileStore::source_default(2657);
    let profile = &store.profiles[0];
    assert_eq!(profile.name, "Default");
    assert_eq!(
        profile.audio_auto_pause_control,
        Some(AutoPauseControl { is_enabled: true })
    );
    assert_eq!(
        profile.noise_cancellation,
        Some(NoiseCancellation {
            is_enabled: true,
            status: 10,
            level: 10
        })
    );
    let other = EarbudsProfileStore::source_default(2660);
    assert!(other.profiles[0].noise_cancellation.is_none());
    assert!(other.profiles[0].audio_auto_pause_control.is_some());
}

#[test]
fn selection_active_guid_first_profile_and_unmatched() {
    let mut store = EarbudsProfileStore {
        schema_version: 1,
        active_guid: None,
        profiles: vec![profile("a", Some(true)), profile("b", Some(false))],
    };
    assert_eq!(store.select(), ProfileSelection::Profile(0));
    store.active_guid = Some("b".into());
    assert_eq!(store.select(), ProfileSelection::Profile(1));
    store.active_guid = Some("zzz".into());
    assert_eq!(store.select(), ProfileSelection::Unmatched("zzz".into()));
    assert!(store.selected().is_err());
    store.active_guid = None;
    store.profiles.clear();
    assert_eq!(store.select(), ProfileSelection::Empty);
}

#[test]
fn anc_update_storage_level_defaults_to_one_and_merges() {
    let mut p = profile("a", None);
    let stored = p
        .set_noise_cancellation(AncChange {
            enabled: Some(false),
            ..AncChange::default()
        })
        .unwrap();
    assert_eq!(
        stored,
        NoiseCancellation {
            is_enabled: false,
            status: 10,
            level: 1
        }
    );
    let stored = p
        .set_noise_cancellation(AncChange {
            enabled: Some(true),
            mode: Some(AncMode::Ambient),
            level: Some(AncLevel::Full),
        })
        .unwrap();
    assert_eq!(
        stored,
        NoiseCancellation {
            is_enabled: true,
            status: 255,
            level: 10
        }
    );
}

#[test]
fn unknown_persisted_anc_values_are_rejected() {
    assert!(
        NoiseCancellation {
            is_enabled: true,
            status: 80,
            level: 10
        }
        .to_setting()
        .is_err()
    );
    assert!(
        NoiseCancellation {
            is_enabled: true,
            status: 10,
            level: 5
        }
        .to_setting()
        .is_err()
    );
}

#[test]
fn load_or_create_only_creates_when_missing_and_preserves_extra() {
    let temp = Temp::new();
    let path = temp.file();
    assert!(load(&path, 2657, "SN1").unwrap().is_none());
    let mut store = load_or_create(&path, 2657, "SN1").unwrap();
    store.profiles[0].set_auto_pause(false);
    store.profiles[0]
        .extra
        .insert("notDisturb".into(), serde_json::json!({"isEnabled":true}));
    save(&path, 2657, "SN1", &store).unwrap();
    let again = load_or_create(&path, 2657, "SN1").unwrap();
    assert_eq!(again, store);
    assert_eq!(again.profiles[0].auto_pause_field().is_enabled, Some(false));
    assert!(load(&path, 2657, "").is_err());
}

fn workspace_device() -> razer_model::model::Device {
    let mut device = razer_model::product_preview::registered_preview(2657).unwrap();
    device.active_profile = "active".into();
    device.profiles = ["active", "edited"]
        .into_iter()
        .map(|id| razer_model::model::Profile {
            id: id.into(),
            guid: format!("guid-{id}"),
            name: id.into(),
            source_settings: Some(serde_json::json!({
                "profile": {"audioAutoPauseControl": {"isEnabled": true, "extension": 7},
                    "equalizer": {"selected": 3}},
                "device": {"autoPause": {"isEnabled": false}},
                "extension": [1, 2]
            })),
            settings: None,
            dpi_stages: None,
        })
        .collect();
    device
}

#[test]
fn workspace_edit_targets_captured_profile_and_preserves_adjacent_values() {
    let mut device = workspace_device();
    let other = device.profiles[0].clone();
    let mut expected = device.profiles[1].source_settings.clone().unwrap();
    expected["profile"]["audioAutoPauseControl"]["isEnabled"] = Value::Bool(false);
    merge_auto_pause(&mut device, "edited", false).unwrap();
    assert_eq!(device.profiles[0], other);
    assert_eq!(device.profiles[1].source_settings, Some(expected));
    assert_eq!(device.active_profile, "active");
    assert_eq!(
        read_auto_pause(&device, "edited").unwrap().is_enabled,
        Some(false)
    );
    assert_eq!(
        read_auto_pause(&device, "active").unwrap().is_enabled,
        Some(true)
    );
}

#[test]
fn workspace_missing_fields_remain_absent_until_explicit_edit() {
    for source in [
        None,
        Some(serde_json::json!({})),
        Some(serde_json::json!({"profile": {}})),
        Some(serde_json::json!({"profile": {"audioAutoPauseControl": {}}})),
    ] {
        let mut device = workspace_device();
        device.profiles[0].source_settings = source.clone();
        assert_eq!(read_auto_pause(&device, "active").unwrap().is_enabled, None);
        assert_eq!(device.profiles[0].source_settings, source);
        merge_auto_pause(&mut device, "active", false).unwrap();
        assert_eq!(
            read_auto_pause(&device, "active").unwrap().is_enabled,
            Some(false)
        );
    }
}

#[test]
fn workspace_rejects_malformed_or_ambiguous_values_without_mutation() {
    for source in [
        serde_json::json!(null),
        serde_json::json!([]),
        serde_json::json!({"profile": null}),
        serde_json::json!({"profile": {"audioAutoPauseControl": false}}),
        serde_json::json!({"profile": {"audioAutoPauseControl": {"isEnabled": 1}}}),
        serde_json::json!({"profile": {"audioAutoPauseControl": {"isEnabled": null}}}),
        serde_json::json!({"profile": {}, "audioAutoPauseControl": {"isEnabled": false}}),
    ] {
        let mut device = workspace_device();
        device.profiles[0].source_settings = Some(source.clone());
        assert!(read_auto_pause(&device, "active").is_err());
        assert!(merge_auto_pause(&mut device, "active", true).is_err());
        assert_eq!(device.profiles[0].source_settings, Some(source));
    }
}

#[test]
fn workspace_rejects_removed_duplicate_and_wrong_product_identities() {
    for mode in ["removed", "empty", "duplicate", "wrong-product"] {
        let mut device = workspace_device();
        let selected = match mode {
            "removed" => "removed",
            "empty" => "",
            "duplicate" => {
                device.profiles.push(device.profiles[0].clone());
                "active"
            }
            _ => {
                device.product_id = 1427;
                "active"
            }
        };
        let before = device.profiles.clone();
        assert!(read_auto_pause(&device, selected).is_err());
        assert!(merge_auto_pause(&mut device, selected, false).is_err());
        assert_eq!(device.profiles, before);
    }
}

#[test]
fn workspace_original_profile_document_preserves_its_shape() {
    let mut device = workspace_device();
    device.profiles[0].source_settings = Some(serde_json::json!({
        "guid": "guid-active", "audioAutoPauseControl": {"isEnabled": false, "extension": 9}
    }));
    assert_eq!(
        read_auto_pause(&device, "active").unwrap().is_enabled,
        Some(false)
    );
    merge_auto_pause(&mut device, "active", true).unwrap();
    assert_eq!(
        device.profiles[0].source_settings,
        Some(serde_json::json!({
            "guid": "guid-active", "audioAutoPauseControl": {"isEnabled": true, "extension": 9}
        }))
    );
    device.profiles[0].source_settings = Some(serde_json::json!({"guid": "guid-active"}));
    assert_eq!(read_auto_pause(&device, "active").unwrap().is_enabled, None);
    merge_auto_pause(&mut device, "active", false).unwrap();
    assert_eq!(
        device.profiles[0].source_settings,
        Some(serde_json::json!({
            "guid": "guid-active", "audioAutoPauseControl": {"isEnabled": false}
        }))
    );
}

#[test]
fn workspace_queued_edit_rebases_without_replacing_an_intervening_save() {
    let baseline = workspace_device();
    let mut proposed = baseline.clone();
    merge_auto_pause(&mut proposed, "edited", false).unwrap();
    let mut latest = baseline.clone();
    latest.profiles[1].source_settings.as_mut().unwrap()["profile"]["equalizer"]["selected"] =
        serde_json::json!(9);
    latest.active_profile = "edited".into();
    let queued = crate::workspace::DeviceSaveDraft::new(vec![baseline], vec![proposed]);
    let saved = queued.rebase(&[latest]).unwrap();
    assert_eq!(
        read_auto_pause(&saved[0], "edited").unwrap().is_enabled,
        Some(false)
    );
    assert_eq!(saved[0].active_profile, "edited");
    assert_eq!(
        saved[0].profiles[1].source_settings.as_ref().unwrap()["profile"]["equalizer"]["selected"],
        serde_json::json!(9)
    );
}

#[test]
fn workspace_initial_profile_has_complete_source_template_and_real_identity() {
    for pid in proto::AUTO_PAUSE_PRODUCTS {
        let mut device = workspace_device();
        device.product_id = pid;
        device.profiles.clear();
        device.active_profile.clear();
        initialize_auto_pause_profile(&mut device, "generated-guid", "Desktop-Default").unwrap();
        assert_eq!(device.active_profile, "generated-guid");
        assert_eq!(device.profiles[0].guid, "generated-guid");
        assert_eq!(device.profiles[0].name, "Desktop-Default");
        let profile = &device.profiles[0].source_settings.as_ref().unwrap()["profile"];
        assert_eq!(
            profile["audioEqualizer"]["presets"]["selectedPreset"]["standard"],
            "game"
        );
        let presets = profile["audioEqualizer"]["presets"]["standard"]
            .as_array()
            .unwrap();
        assert_eq!(presets.len(), 4);
        for (preset, expected) in presets.iter().zip([
            [2, 2, 3, 4, 1, -1, 2, 3, 3, 3],
            [3, 3, 3, -4, -4, -2, 2, 3, 3, 3],
            [2, 2, -1, -1, -1, -1, 1, 3, 3, 3],
            [0; 10],
        ]) {
            assert_eq!(preset["default"], serde_json::json!(expected));
            assert_eq!(preset["current"], serde_json::json!(expected));
        }
        assert_eq!(profile["notDisturb"]["isEnabled"], false);
        for key in [
            "audioGestureControl",
            "audioVoicePromptControl",
            "audioAutoPauseControl",
        ] {
            assert_eq!(profile[key]["isEnabled"], true);
        }
        match pid {
            2657 => assert_eq!(
                profile["noiseCancellation"],
                serde_json::json!({"isEnabled":true,"status":10,"level":10})
            ),
            2664 | 2668 => assert_eq!(
                profile["noiseCancellation"],
                serde_json::json!({"isEnabled":true,"status":10})
            ),
            _ => assert!(profile.get("noiseCancellation").is_none()),
        }
    }
}

#[test]
fn workspace_initial_profile_does_not_repair_or_replace_existing_selection() {
    for with_profile in [false, true] {
        let mut device = workspace_device();
        if !with_profile {
            device.profiles.clear();
        }
        let before = device.profiles.clone();
        assert!(initialize_auto_pause_profile(&mut device, "new", "Desktop-Default").is_err());
        assert_eq!(device.active_profile, "active");
        assert_eq!(device.profiles, before);
    }
}

#[test]
fn workspace_do_not_disturb_preserves_auto_pause_and_other_profile_fields() {
    let mut device = workspace_device();
    let mut expected = device.profiles[1].source_settings.clone().unwrap();
    assert_eq!(read_do_not_disturb(&device, "edited").unwrap(), None);
    merge_do_not_disturb(&mut device, "edited", true).unwrap();
    expected["profile"]["notDisturb"] = serde_json::json!({"isEnabled": true});
    assert_eq!(device.profiles[1].source_settings, Some(expected));
    assert_eq!(
        read_auto_pause(&device, "edited").unwrap().is_enabled,
        Some(true)
    );
    merge_auto_pause(&mut device, "edited", false).unwrap();
    assert_eq!(read_do_not_disturb(&device, "edited").unwrap(), Some(true));
    assert_eq!(read_do_not_disturb(&device, "active").unwrap(), None);
}

#[test]
fn workspace_do_not_disturb_keeps_missing_and_rejects_malformed_values() {
    let mut device = workspace_device();
    for source in [
        None,
        Some(serde_json::json!({"guid":"active"})),
        Some(serde_json::json!({"profile":{"notDisturb":{}}})),
    ] {
        device.profiles[0].source_settings = source.clone();
        assert_eq!(read_do_not_disturb(&device, "active").unwrap(), None);
        assert_eq!(device.profiles[0].source_settings, source);
    }
    for field in [
        serde_json::json!(null),
        serde_json::json!(false),
        serde_json::json!({"isEnabled":1}),
    ] {
        let source = serde_json::json!({"profile":{"notDisturb":field}});
        device.profiles[0].source_settings = Some(source.clone());
        assert!(read_do_not_disturb(&device, "active").is_err());
        assert!(merge_do_not_disturb(&mut device, "active", true).is_err());
        assert_eq!(device.profiles[0].source_settings, Some(source));
    }
}

#[test]
fn workspace_voice_prompt_is_absent_until_edited_and_keeps_other_controls() {
    let mut device = workspace_device();
    let original = device.profiles[1].source_settings.clone();
    assert_eq!(read_voice_prompt(&device, "edited").unwrap(), None);
    assert_eq!(device.profiles[1].source_settings, original);
    merge_do_not_disturb(&mut device, "edited", true).unwrap();
    merge_voice_prompt(&mut device, "edited", false).unwrap();
    assert_eq!(read_voice_prompt(&device, "edited").unwrap(), Some(false));
    assert_eq!(read_do_not_disturb(&device, "edited").unwrap(), Some(true));
    assert_eq!(
        read_auto_pause(&device, "edited").unwrap().is_enabled,
        Some(true)
    );
    assert_eq!(read_voice_prompt(&device, "active").unwrap(), None);
}

#[test]
fn workspace_voice_prompt_rejects_invalid_shape_without_repairing_it() {
    let mut device = workspace_device();
    let source = serde_json::json!({"profile":{"audioVoicePromptControl":{"isEnabled":"false"}}});
    device.profiles[0].source_settings = Some(source.clone());
    assert!(read_voice_prompt(&device, "active").is_err());
    assert!(merge_voice_prompt(&mut device, "active", false).is_err());
    assert_eq!(device.profiles[0].source_settings, Some(source));
}

#[test]
fn workspace_gesture_tap_edits_preserve_extensions_and_do_not_materialize_restore_defaults() {
    use razer_device::earbuds_gesture::TapSpeed;
    for pid in proto::AUTO_PAUSE_PRODUCTS {
        let mut device = workspace_device();
        device.product_id = pid;
        let source = device.profiles[1].source_settings.clone();
        assert_eq!(read_gesture(&device, "edited").unwrap(), None);
        assert_eq!(read_tap_speed(&device, "edited").unwrap(), None);
        assert_eq!(device.profiles[1].source_settings, source);
        merge_gesture(&mut device, "edited", false).unwrap();
        merge_tap_speed(&mut device, "edited", TapSpeed::Slowest).unwrap();
        assert_eq!(read_gesture(&device, "edited").unwrap(), Some(false));
        assert_eq!(
            read_tap_speed(&device, "edited").unwrap(),
            Some(TapSpeed::Slowest)
        );
        assert_eq!(
            read_auto_pause(&device, "edited").unwrap().is_enabled,
            Some(true)
        );
    }
    let mut device = workspace_device();
    for value in [
        serde_json::json!(null),
        serde_json::json!({}),
        serde_json::json!({"tapSpeed":"turbo"}),
        serde_json::json!({"tapSpeed":60}),
    ] {
        let source = serde_json::json!({"profile":{"audioTapSpeed":value}});
        device.profiles[0].source_settings = Some(source.clone());
        assert!(read_tap_speed(&device, "active").is_err());
        assert!(merge_tap_speed(&mut device, "active", TapSpeed::Normal).is_err());
        assert_eq!(device.profiles[0].source_settings, Some(source));
    }
}

#[test]
fn workspace_anc_validates_the_registered_triple_and_preserves_extensions() {
    let mut device=workspace_device();device.product_id=2657;
    assert_eq!(read_anc(&device,"edited").unwrap(),None);
    let setting=AncSetting{mode:AncMode::Ambient,level:AncLevel::Reduced,..proto::ANC_PROFILE_DEFAULT};
    merge_anc(&mut device,"edited",setting).unwrap();
    assert_eq!(read_anc(&device,"edited").unwrap(),Some(setting));
    assert_eq!(read_auto_pause(&device,"edited").unwrap().is_enabled,Some(true));
    for field in [serde_json::json!({"isEnabled":true,"status":10}),serde_json::json!({"isEnabled":true,"status":7,"level":10}),serde_json::json!({"isEnabled":true,"status":10,"level":2})] {
        let source=serde_json::json!({"profile":{"noiseCancellation":field}});device.profiles[0].source_settings=Some(source.clone());
        assert!(read_anc(&device,"active").is_err());assert!(merge_anc(&mut device,"active",setting).is_err());
        assert_eq!(device.profiles[0].source_settings,Some(source));
    }
    device.product_id=2664;assert!(read_anc(&device,"edited").is_err());
}

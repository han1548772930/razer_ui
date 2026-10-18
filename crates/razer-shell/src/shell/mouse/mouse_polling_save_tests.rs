use super::{Accepted, PollingField, merge_profile};
use serde_json::json;

#[test]
fn base_polling_save_keeps_later_game_settings_and_unrelated_dpi() {
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    let profile_id = device.profiles[0].id.clone();
    device.profiles[0].source_settings =
        Some(json!({"pollingRate":1000,"pollingRateWireless":2000,
        "inGamePollingRate":{"state":1,"wired":8000,"wireless":4000},"dpiStages":{"active":3}}));
    device.source_device_settings = Some(json!({"version":7,"extension":{"preserved":true}}));
    merge_profile(
        &mut device,
        &profile_id,
        Accepted::Rate {
            field: PollingField::Wired,
            hz: 500,
        },
    )
    .unwrap();
    let profile = device.profiles[0].source_settings.as_ref().unwrap();
    assert_eq!(profile["pollingRate"], 500);
    assert_eq!(profile["pollingRateWireless"], 2000);
    assert_eq!(profile["inGamePollingRate"]["wired"], 8000);
    assert_eq!(profile["dpiStages"]["active"], 3);
    assert_eq!(profile["_pollingLocalFieldsV1"], json!(["pollingRate"]));
    assert_eq!(
        device.source_device_settings.as_ref().unwrap(),
        &json!({"version":8,"extension":{"preserved":true}})
    );
}

#[test]
fn save_only_game_setting_preserves_base_rates_and_uses_source_root_version() {
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    let profile_id = device.profiles[0].id.clone();
    device.profiles[0].source_settings =
        Some(json!({"pollingRate":500,"pollingRateWireless":2000,"dpiStages":{"active":2}}));
    device.source_device_settings = None;
    let setting = json!({"state":1,"wired":8000,"wireless":4000,"extension":{"preserved":true}});
    merge_profile(&mut device, &profile_id, Accepted::InGame(setting.clone())).unwrap();
    let profile = device.profiles[0].source_settings.as_ref().unwrap();
    assert_eq!(profile["pollingRate"], 500);
    assert_eq!(profile["pollingRateWireless"], 2000);
    assert_eq!(profile["inGamePollingRate"], setting);
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["version"],
        1
    );
    device.source_device_settings.as_mut().unwrap()["version"] =
        json!(razer_device::mapping_submission::MAX_SOURCE_VERSION);
    merge_profile(&mut device, &profile_id, Accepted::InGame(setting)).unwrap();
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["version"],
        0
    );
}

#[test]
fn invalid_game_setting_root_markers_and_removed_profiles_leave_durable_state_untouched() {
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    let profile_id = device.profiles[0].id.clone();
    for (settings, root, removed, accepted) in [
        (
            json!({}),
            json!({"version":4}),
            false,
            Accepted::InGame(json!({"state":2,"wired":1000,"wireless":1000})),
        ),
        (
            json!({"_pollingLocalFieldsV1":false}),
            json!({"version":4}),
            false,
            Accepted::Rate {
                field: PollingField::Wired,
                hz: 8000,
            },
        ),
        (
            json!({}),
            json!({"version":"4"}),
            false,
            Accepted::Rate {
                field: PollingField::Wired,
                hz: 8000,
            },
        ),
        (
            json!({}),
            json!({"version":4}),
            true,
            Accepted::Rate {
                field: PollingField::Wired,
                hz: 8000,
            },
        ),
    ] {
        device.profiles[0].source_settings = Some(settings);
        device.source_device_settings = Some(root);
        let before = serde_json::to_value(&device).unwrap();
        assert!(
            merge_profile(
                &mut device,
                if removed { "removed" } else { &profile_id },
                accepted
            )
            .is_err()
        );
        assert_eq!(serde_json::to_value(&device).unwrap(), before);
    }
}

#[test]
fn legacy_uppercase_polling_field_and_current182_version_remain_supported() {
    let mut legacy = razer_model::product_preview::registered_preview(70).unwrap();
    let profile_id = legacy.profiles[0].id.clone();
    legacy.profiles[0].source_settings = Some(json!({"PollingRate":125}));
    let root = legacy.source_device_settings.clone();
    merge_profile(
        &mut legacy,
        &profile_id,
        Accepted::Rate {
            field: PollingField::Wired,
            hz: 1000,
        },
    )
    .unwrap();
    assert_eq!(
        legacy.profiles[0].source_settings.as_ref().unwrap()["PollingRate"],
        1000
    );
    assert!(
        legacy.profiles[0]
            .source_settings
            .as_ref()
            .unwrap()
            .get("pollingRate")
            .is_none()
    );
    assert_eq!(legacy.source_device_settings, root);
    let mut current = razer_model::product_preview::registered_preview(182).unwrap();
    let profile_id = current.profiles[0].id.clone();
    current.source_device_settings = Some(json!({"version":1}));
    merge_profile(
        &mut current,
        &profile_id,
        Accepted::Rate {
            field: PollingField::Wireless,
            hz: 8000,
        },
    )
    .unwrap();
    assert_eq!(
        current.source_device_settings.as_ref().unwrap()["version"],
        2
    );
}

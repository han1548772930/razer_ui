use super::*;
use serde_json::json;

fn local_device(product_id: u32) -> Device {
    let mut device = razer_model::product_preview::registered_preview(product_id).unwrap();
    device.active_profile = "test-profile".into();
    device.profiles = vec![razer_model::model::Profile {
        id: device.active_profile.clone(),
        guid: device.active_profile.clone(),
        name: "Test profile".into(),
        settings: None,
        source_settings: None,
        dpi_stages: None,
    }];
    device
}

#[::core::prelude::v1::test]
fn mouse_dynamic_queued_save_preserves_other_features_and_advances_current_source_version() {
    for product_id in razer_device::mouse_dynamic::PRODUCTS {
        let mut device = local_device(*product_id);
        let profile_id = device.profiles[0].id.clone();
        device.profiles[0].source_settings =
            Some(json!({"pollingRate":8000,"dpiStages":{"active":4},"extra":{"preserve":true}}));
        device.source_device_settings =
            Some(json!({"version":7,"deviceMetadatas":{"preserve":true}}));
        let sensitivity =
            json!({"state":1,"mode":2,"templateId":1,"points":[],"customSensorAccelerations":[]});
        merge(&mut device, &profile_id, &sensitivity).unwrap();
        let stored = device.profiles[0].source_settings.as_ref().unwrap();
        assert_eq!(stored["dynamicSensitivity"], sensitivity);
        assert_eq!(stored["pollingRate"], 8000);
        assert_eq!(stored["dpiStages"]["active"], 4);
        assert_eq!(stored["extra"]["preserve"], true);
        let root = device.source_device_settings.as_ref().unwrap();
        assert_eq!(root["version"], 8);
        assert_eq!(root["deviceMetadatas"]["preserve"], true);
    }
}
#[::core::prelude::v1::test]
fn mouse_dynamic_save_rejects_removed_profile_invalid_version_without_partial_mutation() {
    let mut device = local_device(190);
    let profile_id = device.profiles[0].id.clone();
    for (id, profile, root, value) in [
        ("removed", json!({}), json!({"version":3}), json!({})),
        (
            profile_id.as_str(),
            json!(false),
            json!({"version":3}),
            json!({}),
        ),
        (
            profile_id.as_str(),
            json!({}),
            json!({"version":"3"}),
            json!({}),
        ),
        (profile_id.as_str(), json!({}), json!([]), json!({})),
        (
            profile_id.as_str(),
            json!({}),
            json!({"version":3}),
            json!([]),
        ),
    ] {
        device.profiles[0].source_settings = Some(profile);
        device.source_device_settings = Some(root);
        let before = serde_json::to_value(&device).unwrap();
        assert!(merge(&mut device, id, &value).is_err());
        assert_eq!(serde_json::to_value(&device).unwrap(), before);
    }
}
#[::core::prelude::v1::test]
fn mouse_dynamic_save_uses_source_first_and_wrapped_53_bit_versions() {
    let mut device = local_device(226);
    let profile_id = device.profiles[0].id.clone();
    device.source_device_settings = None;
    merge(&mut device, &profile_id, &json!({})).unwrap();
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["version"],
        1
    );
    device.source_device_settings.as_mut().unwrap()["version"] =
        json!(razer_device::mapping_submission::MAX_SOURCE_VERSION);
    merge(&mut device, &profile_id, &json!({})).unwrap();
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["version"],
        0
    );
}

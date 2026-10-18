use super::merge_current_dpi_profile;
use serde_json::json;

#[test]
fn queued_current190_dpi_save_keeps_later_polling_and_advances_the_root_version() {
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    let profile_id = device.profiles[0].id.clone();
    device.profiles[0].source_settings = Some(json!({
        "pollingRate":8000,"inGamePollingRate":{"state":1,"wired":4000,"wireless":8000},
        "dpiStages":{"enable":true},"extra":{"retained":true}
    }));
    device.source_device_settings = Some(json!({"version":7,"deviceMetadatas":{"retained":true}}));
    let accepted = json!({
        "pollingRate":1000,"dpiStages":{"active":3,"enable":false,"count":5,
        "stages":[
            {"x":400,"y":400,"visible":true,"independent":false},
            {"x":800,"y":800,"visible":true,"independent":false},
            {"x":2000,"y":2100,"visible":true,"independent":true},
            {"x":3200,"y":3200,"visible":true,"independent":false},
            {"x":6400,"y":6400,"visible":true,"independent":false}
        ]}
    });
    merge_current_dpi_profile(&mut device, &profile_id, &accepted).unwrap();
    let stored = device.profiles[0].source_settings.as_ref().unwrap();
    assert_eq!(stored["dpiStages"], accepted["dpiStages"]);
    assert_eq!(stored["pollingRate"], 8000);
    assert_eq!(stored["inGamePollingRate"]["wired"], 4000);
    assert_eq!(stored["extra"]["retained"], true);
    let root = device.source_device_settings.as_ref().unwrap();
    assert_eq!(root["version"], 8);
    assert_eq!(root["deviceMetadatas"]["retained"], true);
}

#[test]
fn current182_and190_dpi_saves_use_the_source_first_and_wrapped_version() {
    for product_id in [182, 190] {
        let mut device = razer_model::product_preview::registered_preview(product_id).unwrap();
        let profile_id = device.profiles[0].id.clone();
        device.source_device_settings = None;
        merge_current_dpi_profile(&mut device, &profile_id, &json!({"dpiStages":{}})).unwrap();
        assert_eq!(
            device.source_device_settings.as_ref().unwrap()["version"],
            1
        );
        device.source_device_settings.as_mut().unwrap()["version"] =
            json!(razer_device::mapping_submission::MAX_SOURCE_VERSION);
        merge_current_dpi_profile(&mut device, &profile_id, &json!({"dpiStages":{}})).unwrap();
        assert_eq!(
            device.source_device_settings.as_ref().unwrap()["version"],
            0
        );
    }
}

#[test]
fn removed_profile_and_invalid_documents_leave_the_durable_device_untouched() {
    let mut device = razer_model::product_preview::registered_preview(190).unwrap();
    let profile_id = device.profiles[0].id.clone();
    for (profile, root, source, id) in [
        (json!({}), json!({"version":3}), json!({}), "removed"),
        (
            json!(null),
            json!({"version":3}),
            json!({}),
            profile_id.as_str(),
        ),
        (json!({}), json!(false), json!({}), profile_id.as_str()),
        (
            json!({}),
            json!({"version":"3"}),
            json!({}),
            profile_id.as_str(),
        ),
        (
            json!({}),
            json!({"version":-1}),
            json!({}),
            profile_id.as_str(),
        ),
        (
            json!({}),
            json!({"version":3}),
            json!([]),
            profile_id.as_str(),
        ),
    ] {
        device.profiles[0].source_settings = Some(profile);
        device.source_device_settings = Some(root);
        let before = serde_json::to_value(&device).unwrap();
        assert!(merge_current_dpi_profile(&mut device, id, &source).is_err());
        assert_eq!(serde_json::to_value(&device).unwrap(), before);
    }
}

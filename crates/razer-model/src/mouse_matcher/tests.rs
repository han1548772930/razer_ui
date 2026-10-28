use super::*;

fn device_for(product_id: u32) -> Device {
    let mut device = crate::test_devices::with_profile(product_id);
    device.active_profile = device.profiles[0].id.clone();
    device.profiles[0].source_settings = Some(json!({
        "guid": device.profiles[0].guid,
        "dpiStages":{"active":1,"enable":true,"stages":[{"x":800,"y":800,"visible":true,"independent":false}]},
        "pollingRate":1000,"extension":{"keep":true}
    }));
    device.source_device_settings = Some(json!({"version":7,"deviceExtension":"keep"}));
    device
}

fn device() -> Device {
    device_for(190)
}

fn mutation(device: &Device) -> Mutation {
    let before = source_record(device).unwrap();
    let mut after = before.document().clone();
    after["sensitivityMatcher"] = json!({"profiles":[{"guid":"measured","name":{"en":"Observed mouse","zh-cn":"Observed mouse"},"dpi":{"x":1200,"y":1200,"visible":true,"independent":false}}]});
    after["profiles"][0]["selectedSensitivityMatcher"] = json!("measured");
    after["profiles"][0]["dpiStages"]["stages"][0]["x"] = json!(1200);
    Mutation::from_source(&before, &after).unwrap()
}

#[test]
fn mouse_matcher_save_merges_only_owned_fields_and_advances_latest_version() {
    let mut device = device();
    let mutation = mutation(&device);
    device.profiles[0].source_settings.as_mut().unwrap()["pollingRate"] = json!(500);
    device.source_device_settings.as_mut().unwrap()["version"] = json!(9);
    mutation.apply(&mut device).unwrap();
    let source = device.profiles[0].source_settings.as_ref().unwrap();
    assert_eq!(source["pollingRate"], 500);
    assert_eq!(source["extension"], json!({"keep":true}));
    assert_eq!(source["selectedSensitivityMatcher"], "measured");
    assert_eq!(source["dpiStages"]["stages"][0]["x"], 1200);
    let root = device.source_device_settings.unwrap();
    assert_eq!(root["deviceExtension"], "keep");
    assert_eq!(root["version"], 10);
}

#[test]
fn mouse_matcher_conflicts_and_removed_profiles_never_partially_mutate_storage() {
    for conflict in 0..6 {
        let mut device = device();
        let mutation = mutation(&device);
        match conflict {
            0 => {
                device.source_device_settings.as_mut().unwrap()["sensitivityMatcher"] =
                    json!({"profiles":[]})
            }
            1 => {
                device.profiles[0].source_settings.as_mut().unwrap()["selectedSensitivityMatcher"] =
                    json!("other")
            }
            2 => {
                device.profiles[0].source_settings.as_mut().unwrap()["dpiStages"]["stages"][0]["x"] =
                    json!(1600)
            }
            3 => device.profiles.clear(),
            4 => device.profiles[0].id = "replacement-profile".into(),
            _ => device.device_container_id = "different-device".into(),
        }
        let before = serde_json::to_value(&device).unwrap();
        assert!(mutation.apply(&mut device).is_err());
        assert_eq!(serde_json::to_value(device).unwrap(), before);
    }
}

#[test]
fn mouse_matcher_source_rejects_wrong_product_and_inconsistent_profile_guid() {
    let mut device = device();
    device.profiles[0].source_settings.as_mut().unwrap()["guid"] = json!("different");
    assert!(source_record(&device).is_err());
    device.product_id = 70;
    assert!(source_record(&device).is_err());
}

#[test]
fn mouse_matcher_save_preserves_observed_null_fields_as_distinct_from_absent_fields() {
    let mut device = device();
    device.source_device_settings.as_mut().unwrap()["sensitivityMatcher"] = Value::Null;
    device.profiles[0].source_settings.as_mut().unwrap()["selectedSensitivityMatcher"] =
        Value::Null;
    let mutation = mutation(&device);
    mutation.apply(&mut device).unwrap();
    assert_eq!(
        device.profiles[0].source_settings.as_ref().unwrap()["selectedSensitivityMatcher"],
        "measured"
    );
}

#[test]
fn mouse_matcher_source_and_mutation_retain_each_audited_product_identity() {
    for product_id in [190, 192, 222, 226, 229, 239] {
        let mut device = device_for(product_id);
        let mutation = mutation(&device);
        mutation.apply(&mut device).unwrap();
        assert_eq!(device.product_id, product_id);
    }
}

#[test]
fn mouse_matcher_mutation_rejects_a_different_audited_product() {
    let source = device_for(190);
    let mutation = mutation(&source);
    let mut replacement = device_for(192);
    let before = serde_json::to_value(&replacement).unwrap();
    assert!(mutation.apply(&mut replacement).is_err());
    assert_eq!(serde_json::to_value(replacement).unwrap(), before);
}

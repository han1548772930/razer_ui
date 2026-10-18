use super::*;
use serde_json::json;

#[::core::prelude::v1::test]
fn rotation_persistence_keeps_other_settings_and_advances_only_source_version() {
    for product in rotation::PRODUCTS {
        let mut device = razer_model::product_preview::registered_preview(*product).unwrap();
        let profile = device.profiles[0].id.clone();
        device.profiles[0].source_settings =
            Some(json!({"pollingRate":8000,"extra":{"keep":true}}));
        device.source_device_settings = Some(json!({"version":7,"metadata":{"keep":true}}));
        merge(
            &mut device,
            &profile,
            Setting {
                enabled: false,
                angle_degrees: -19,
            },
        )
        .unwrap();
        assert_eq!(
            device.profiles[0].source_settings.as_ref().unwrap(),
            &json!({
                "pollingRate":8000,"extra":{"keep":true},"rotation":{"isEnabled":false,"value":-19}
            })
        );
        assert_eq!(
            device.source_device_settings.as_ref().unwrap(),
            &json!({"version":8,"metadata":{"keep":true}})
        );
    }
}

#[::core::prelude::v1::test]
fn rotation_persistence_rejects_invalid_scope_or_version_without_partial_mutation() {
    let mut device = razer_model::product_preview::registered_preview(196).unwrap();
    let profile_id = device.profiles[0].id.clone();
    for (profile, root, source, setting) in [
        (
            "removed",
            json!({"version":3}),
            json!({}),
            Setting {
                enabled: true,
                angle_degrees: 0,
            },
        ),
        (
            profile_id.as_str(),
            json!({"version":"3"}),
            json!({}),
            Setting {
                enabled: true,
                angle_degrees: 0,
            },
        ),
        (
            profile_id.as_str(),
            json!([]),
            json!({}),
            Setting {
                enabled: true,
                angle_degrees: 0,
            },
        ),
        (
            profile_id.as_str(),
            json!({"version":3}),
            json!(false),
            Setting {
                enabled: true,
                angle_degrees: 0,
            },
        ),
        (
            profile_id.as_str(),
            json!({"version":3}),
            json!({}),
            Setting {
                enabled: true,
                angle_degrees: 45,
            },
        ),
    ] {
        device.source_device_settings = Some(root);
        device.profiles[0].source_settings = Some(source);
        let before = serde_json::to_value(&device).unwrap();
        assert!(merge(&mut device, profile, setting).is_err());
        assert_eq!(serde_json::to_value(&device).unwrap(), before);
    }
}

#[::core::prelude::v1::test]
fn rotation_persistence_uses_source_first_and_wrapped_53_bit_versions() {
    let mut device = razer_model::product_preview::registered_preview(196).unwrap();
    let profile = device.profiles[0].id.clone();
    let setting = Setting {
        enabled: true,
        angle_degrees: 44,
    };
    device.source_device_settings = None;
    merge(&mut device, &profile, setting).unwrap();
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["version"],
        1
    );
    device.source_device_settings.as_mut().unwrap()["version"] =
        json!(razer_device::mapping_submission::MAX_SOURCE_VERSION);
    merge(&mut device, &profile, setting).unwrap();
    assert_eq!(
        device.source_device_settings.as_ref().unwrap()["version"],
        0
    );
}

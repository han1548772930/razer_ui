use super::publish_reset_profile_snapshot;
use razer_model::model::{Device, DeviceConnectionObservation, Profile};
use serde_json::json;

fn device() -> Device {
    let mut device = razer_model::product_preview::registered_preview(1398).unwrap();
    device.serial_number = "observed-serial".into();
    device.device_container_id = "observed-container".into();
    device.dashboard.connection_observation = Some(DeviceConnectionObservation::HidPresent);
    device
}

fn candidate(current: &Device) -> Device {
    let mut candidate = current.clone();
    candidate.profiles.push(Profile {
        id: "fresh-profile".into(),
        guid: "fresh-profile".into(),
        name: "Host-Default".into(),
        dpi_stages: None,
        settings: None,
        source_settings: Some(json!({"guid":"fresh-profile", "name":"Host-Default", "gain":4})),
    });
    candidate.active_profile = "fresh-profile".into();
    candidate.source_device_settings = Some(json!({"version":3}));
    candidate
}

#[test]
fn reset_publication_changes_only_the_persisted_profile_fields() {
    let mut current = device();
    let original = current.clone();
    let mut next = candidate(&current);
    next.name.values.clear();
    next.setup_status = razer_model::model::SetupStatus::Unknown;
    assert!(publish_reset_profile_snapshot(&mut current, next));
    assert_eq!(current.active_profile, "fresh-profile");
    assert_eq!(
        current
            .active_profile_obj()
            .unwrap()
            .source_settings
            .as_ref()
            .unwrap()["gain"],
        4
    );
    assert_eq!(current.source_device_settings, Some(json!({"version":3})));
    assert_eq!(
        serde_json::to_value(&current.name).unwrap(),
        serde_json::to_value(&original.name).unwrap()
    );
    assert_eq!(current.setup_status, original.setup_status);
    assert_eq!(
        current.dashboard.connection_observation,
        original.dashboard.connection_observation
    );
}

#[test]
fn replacement_owner_or_connection_cannot_publish_a_reset_snapshot() {
    let original = device();
    for change in 0..8 {
        let mut current = original.clone();
        let before = serde_json::to_value(&current).unwrap();
        let mut next = candidate(&current);
        match change {
            0 => next.product_id += 1,
            1 => next.real_product_id += 1,
            2 => next.serial_number.push_str("replacement"),
            3 => next.device_container_id.push_str("replacement"),
            4 => next.layout_id += 1,
            5 => next.use_ble = !next.use_ble,
            6 => next.dashboard.connection_observation = None,
            7 => current.product_id = 1399,
            _ => unreachable!(),
        }
        let current_before = serde_json::to_value(&current).unwrap();
        assert!(
            !publish_reset_profile_snapshot(&mut current, next),
            "case {change}"
        );
        assert_eq!(serde_json::to_value(&current).unwrap(), current_before);
        if change != 7 {
            assert_eq!(current_before, before);
        }
    }
}

#[test]
fn malformed_active_profile_is_rejected_without_partial_publication() {
    for change in 0..6 {
        let mut current = device();
        let before = serde_json::to_value(&current).unwrap();
        let mut next = candidate(&current);
        match change {
            0 => next.active_profile = "absent".into(),
            1 => next.profiles.last_mut().unwrap().id.clear(),
            2 => next.profiles.last_mut().unwrap().guid = "other".into(),
            3 => next.profiles.last_mut().unwrap().source_settings = None,
            4 => {
                next.profiles
                    .last_mut()
                    .unwrap()
                    .source_settings
                    .as_mut()
                    .unwrap()["guid"] = json!("other")
            }
            5 => {
                next.profiles
                    .last_mut()
                    .unwrap()
                    .source_settings
                    .as_mut()
                    .unwrap()["name"] = json!("other")
            }
            _ => unreachable!(),
        }
        assert!(
            !publish_reset_profile_snapshot(&mut current, next),
            "case {change}"
        );
        assert_eq!(serde_json::to_value(&current).unwrap(), before);
    }
}

#[test]
fn reset_publication_rejects_replacing_or_editing_the_existing_collection() {
    for change in 0..4 {
        let mut current = device();
        let original = current.clone();
        let mut next = candidate(&current);
        match change {
            0 => {
                next.profiles.remove(0);
            }
            1 => next.profiles[0].name.push_str("edited"),
            2 => next.profiles.swap(0, 1),
            3 => {
                let existing = next.profiles[0].id.clone();
                let appended = next.profiles.last_mut().unwrap();
                appended.id = existing.clone();
                appended.guid = existing.clone();
                appended.source_settings.as_mut().unwrap()["guid"] = json!(existing);
                next.active_profile = existing;
            }
            _ => unreachable!(),
        }
        assert!(
            !publish_reset_profile_snapshot(&mut current, next),
            "case {change}"
        );
        assert_eq!(current.profiles, original.profiles);
        assert_eq!(current.active_profile, original.active_profile);
        assert_eq!(
            current.source_device_settings,
            original.source_device_settings
        );
    }
}

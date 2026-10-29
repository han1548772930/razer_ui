//! Source defaults for explicit profile creation and application-owned profile
//! selection/version persistence. Merely discovering a serial or missing local
//! cache does not initialize profiles or write hardware.
use super::*;

/// The serialized writer assigns versions after rebasing accepted edits. Copy
/// that durable revision into live state only while its prior version still
/// matches; later profile/device edits and their unrelated fields remain live.
pub(super) fn accept_saved_version(device: &mut Device, previous: &Device, snapshot: &Device) {
    if razer_device::mouse_scroll::capability(device.product_id).is_none()
        && device.product_id != razer_device::mouse_philips::PRODUCT_ID
        && super::super::mouse_products::matcher_device_fields(device.product_id).is_empty()
        && !matches!(device.product_id, 182 | 190)
    {
        return;
    }
    let version = |device: &Device| {
        device
            .source_device_settings
            .as_ref()
            .and_then(|root| root.get("version"))
            .cloned()
    };
    if version(device) != version(previous) {
        return;
    }
    let Some(saved_version) = version(snapshot) else {
        return;
    };
    let root = device
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}));
    if let Some(root) = root.as_object_mut() {
        root.insert("version".into(), saved_version);
    }
}

fn defaults(product: u32) -> Option<(&'static Value, &'static Value, u32)> {
    super::super::mouse_products::source_product(product)?.profile_document_defaults()
}

pub(super) fn default_profile(device: &Device, name: String) -> Option<Profile> {
    let (profile, _, _) = defaults(device.product_id)?;
    let guid = uuid::Uuid::new_v4().to_string();
    let mut source = profile.clone();
    source["guid"] = guid.clone().into();
    source["name"] = name.clone().into();
    Some(Profile {
        id: guid.clone(),
        guid,
        name,
        source_settings: Some(source),
        settings: None,
        dpi_stages: None,
    })
}

/// This name is 3298.G, including its empty computer-name result "-Default".
pub(super) fn default_name() -> String {
    format!(
        "{}-Default",
        std::env::var("COMPUTERNAME").unwrap_or_default()
    )
}

pub(super) fn sync_selection(device: &mut Device, mutation: bool) {
    if defaults(device.product_id).is_none() {
        return;
    }
    for profile in &mut device.profiles {
        if let Some(source) = profile
            .source_settings
            .as_mut()
            .and_then(Value::as_object_mut)
        {
            source.insert("guid".into(), profile.guid.clone().into());
            source.insert("name".into(), profile.name.clone().into());
        }
    }
    let Some(active) = device
        .profiles
        .iter()
        .find(|p| p.id == device.active_profile)
    else {
        return;
    };
    let guid = active.guid.clone();
    let root = device
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}));
    let Some(root_object) = root.as_object_mut() else {
        return;
    };
    if mutation {
        root_object.remove("isDefault");
    }
    root_object.insert("activeProfile".into(), guid.clone().into());
    if device.serial_number.is_empty() || device.serial_number.starts_with("PREVIEW-") {
        return;
    }
    let metadata = root_object
        .entry("deviceMetadatas")
        .or_insert_with(|| serde_json::json!({}));
    let Some(metadata) = metadata.as_object_mut() else {
        return;
    };
    let entry = metadata
        .entry(device.serial_number.clone())
        .or_insert_with(|| serde_json::json!({}));
    if let Some(entry) = entry.as_object_mut() {
        entry.insert("activeProfileGuid".into(), guid.into());
        entry
            .entry("name")
            .or_insert_with(|| device.product_name.get("en").to_owned().into());
    }
}

pub(super) fn removed_profile(device: &mut Device, removed: &str) {
    if defaults(device.product_id).is_none() {
        return;
    }
    let Some(active) = device.active_profile_obj() else {
        return;
    };
    let replacement = active.guid.clone();
    if let Some(metadata) = device
        .source_device_settings
        .as_mut()
        .and_then(|root| root.get_mut("deviceMetadatas"))
        .and_then(Value::as_object_mut)
    {
        for record in metadata.values_mut() {
            if record.get("activeProfileGuid").and_then(Value::as_str) == Some(removed) {
                record["activeProfileGuid"] = replacement.clone().into();
            }
        }
    }
}

pub(super) fn device_fields(product: u32) -> impl Iterator<Item = &'static str> {
    defaults(product)
        .into_iter()
        .flat_map(|(_, root, _)| {
            root.as_object()
                .into_iter()
                .flat_map(|root| root.keys().map(String::as_str))
        })
        .chain(
            defaults(product)
                .is_some()
                .then_some([
                    "schemaVersion",
                    "isDefault",
                    "activeProfile",
                    "deviceMetadatas",
                ])
                .into_iter()
                .flatten(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn scroll_durable_version_acknowledgement_preserves_new_edits_and_later_versions() {
        let mut previous = razer_model::product_preview::registered_preview(226).unwrap();
        previous.source_device_settings =
            Some(serde_json::json!({"version":7,"unknown":{"old":true}}));
        let mut live = previous.clone();
        live.source_device_settings.as_mut().unwrap()["unknown"] = serde_json::json!({"new":true});
        let mut snapshot = previous.clone();
        snapshot.source_device_settings.as_mut().unwrap()["version"] = serde_json::json!(8);
        accept_saved_version(&mut live, &previous, &snapshot);
        assert_eq!(live.source_device_settings.as_ref().unwrap()["version"], 8);
        assert_eq!(
            live.source_device_settings.as_ref().unwrap()["unknown"],
            serde_json::json!({"new":true})
        );
        live.source_device_settings.as_mut().unwrap()["version"] = serde_json::json!(10);
        accept_saved_version(&mut live, &previous, &snapshot);
        assert_eq!(live.source_device_settings.as_ref().unwrap()["version"], 10);
    }
    #[::core::prelude::v1::test]
    fn philips_durable_version_keeps_new_calibration_and_rejects_a_later_revision() {
        let mut previous = razer_model::product_preview::registered_preview(70).unwrap();
        previous.source_device_settings =
            Some(serde_json::json!({"version":7,"calibration":{"liftOffRangeValue":3}}));
        let mut live = previous.clone();
        live.source_device_settings.as_mut().unwrap()["calibration"]["liftOffRangeValue"] =
            serde_json::json!(5);
        let mut snapshot = previous.clone();
        snapshot.source_device_settings.as_mut().unwrap()["version"] = serde_json::json!(8);
        accept_saved_version(&mut live, &previous, &snapshot);
        assert_eq!(live.source_device_settings.as_ref().unwrap()["version"], 8);
        assert_eq!(
            live.source_device_settings.as_ref().unwrap()["calibration"]["liftOffRangeValue"],
            5
        );
        live.source_device_settings.as_mut().unwrap()["version"] = serde_json::json!(10);
        accept_saved_version(&mut live, &previous, &snapshot);
        assert_eq!(live.source_device_settings.as_ref().unwrap()["version"], 10);
    }

    #[::core::prelude::v1::test]
    fn matcher_durable_version_keeps_later_profile_edits_and_rejects_a_newer_revision() {
        for product_id in [190, 192, 222, 226, 229, 239] {
            let mut previous =
                razer_model::product_preview::registered_preview(product_id).unwrap();
            previous.source_device_settings =
                Some(serde_json::json!({"version":7,"sensitivityMatcher":{"profiles":[]}}));
            let mut live = previous.clone();
            live.source_device_settings.as_mut().unwrap()["sensitivityMatcher"] =
                serde_json::json!({"profiles":[{"guid":"new-draft"}]});
            let mut snapshot = previous.clone();
            snapshot.source_device_settings.as_mut().unwrap()["version"] = serde_json::json!(8);
            accept_saved_version(&mut live, &previous, &snapshot);
            assert_eq!(live.source_device_settings.as_ref().unwrap()["version"], 8);
            assert_eq!(
                live.source_device_settings.as_ref().unwrap()["sensitivityMatcher"]["profiles"][0]
                    ["guid"],
                "new-draft"
            );
            live.source_device_settings.as_mut().unwrap()["version"] = serde_json::json!(10);
            accept_saved_version(&mut live, &previous, &snapshot);
            assert_eq!(live.source_device_settings.as_ref().unwrap()["version"], 10);
        }
    }
}

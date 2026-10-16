//! Current 182 MW3298 defaults, 77846 serial selection and 90012.fn cache
//! initialization. The workspace store is the local cache owner; hardware
//! tasks start only after its existing serialized persistence completes.
use super::*;

/// The serialized writer assigns versions after rebasing accepted edits. Copy
/// that durable revision into live state only while its prior version still
/// matches; later profile/device edits and their unrelated fields remain live.
pub(super) fn accept_saved_version(device: &mut Device, previous: &Device, snapshot: &Device) {
    if razer_device::mouse_scroll::capability(device.product_id).is_none() {
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

pub(super) fn initialize(device: &mut Device) -> bool {
    if device.serial_number.is_empty() || device.serial_number.starts_with("PREVIEW-") {
        return false;
    }
    let Some((profile_default, root_default, schema)) = defaults(device.product_id) else {
        return false;
    };
    let before = serde_json::to_value(&*device).expect("device serialization");
    let absent = device
        .source_device_settings
        .as_ref()
        .and_then(|root| root.get("schemaVersion"))
        .is_none();
    let root = device
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}));
    if !root.is_object() {
        return false;
    }
    // Migrate this application's earlier partial cache without discarding its
    // profile edits. Current source supplies missing top-level defaults only.
    for (key, value) in root_default.as_object().expect("audited source root") {
        root.as_object_mut()
            .unwrap()
            .entry(key.clone())
            .or_insert_with(|| value.clone());
    }
    root.as_object_mut()
        .unwrap()
        .entry("schemaVersion")
        .or_insert(schema.into());
    let missing_serial = !absent
        && device
            .source_device_settings
            .as_ref()
            .and_then(|root| root.get("deviceMetadatas"))
            .and_then(|metadata| metadata.get(&device.serial_number))
            .and_then(|metadata| metadata.get("activeProfileGuid"))
            .and_then(Value::as_str)
            .is_none_or(str::is_empty);
    if device.profiles.is_empty() || missing_serial {
        let base = default_name();
        let mut name = base.clone();
        for suffix in 1..=100 {
            if !device.profiles.iter().any(|p| p.name == name) {
                break;
            }
            name = format!("{base} {suffix}");
        }
        let profile = default_profile(device, name).expect("audited profile");
        device.active_profile = profile.id.clone();
        device.profiles.push(profile);
    }
    if absent {
        device.source_device_settings.as_mut().unwrap()["isDefault"] = true.into();
    }
    let mut migrated_guids = Vec::new();
    for profile in &mut device.profiles {
        if profile.guid.is_empty() || profile.guid.starts_with("local-") {
            let old = profile.guid.clone();
            profile.guid = uuid::Uuid::new_v4().to_string();
            migrated_guids.push((old, profile.guid.clone()));
        }
        let mut source = profile_default.clone();
        if let Some(previous) = profile.source_settings.as_ref().and_then(Value::as_object) {
            source.as_object_mut().unwrap().extend(previous.clone());
        }
        source["guid"] = profile.guid.clone().into();
        source["name"] = profile.name.clone().into();
        profile.source_settings = Some(source);
    }
    for (old, new) in migrated_guids {
        let root = device.source_device_settings.as_mut().unwrap();
        if root.get("activeProfile").and_then(Value::as_str) == Some(old.as_str()) {
            root["activeProfile"] = new.clone().into();
        }
        if let Some(records) = root
            .get_mut("deviceMetadatas")
            .and_then(Value::as_object_mut)
        {
            for record in records.values_mut() {
                if record.get("activeProfileGuid").and_then(Value::as_str) == Some(old.as_str()) {
                    record["activeProfileGuid"] = new.clone().into();
                }
            }
        }
        for profile in &mut device.profiles {
            if let Some(mappings) = profile
                .source_settings
                .as_mut()
                .and_then(|p| p.get_mut("mappings"))
                .and_then(Value::as_array_mut)
            {
                for mapping in mappings {
                    if let Some(target) = mapping
                        .get_mut("profileNavigationGroup")
                        .and_then(Value::as_object_mut)
                    {
                        if target.get("guid").and_then(Value::as_str) == Some(old.as_str()) {
                            target.insert("guid".into(), new.clone().into());
                        }
                    }
                }
            }
        }
    }
    let stored_guid = device
        .source_device_settings
        .as_ref()
        .and_then(|root| root.get("deviceMetadatas"))
        .and_then(|metadata| metadata.get(&device.serial_number))
        .and_then(|metadata| metadata.get("activeProfileGuid"))
        .and_then(Value::as_str)
        .filter(|guid| !guid.is_empty())
        .map(str::to_owned);
    if let Some(guid) = stored_guid {
        // Source77846.M preserves an existing record even if its GUID is
        // dangling; it must not silently load another profile's settings.
        device.active_profile = device
            .profiles
            .iter()
            .find(|p| p.guid == guid)
            .map(|p| p.id.clone())
            .unwrap_or(guid);
    } else if !device
        .profiles
        .iter()
        .any(|p| p.id == device.active_profile)
    {
        device.active_profile = device.profiles[0].id.clone();
    }
    if absent || missing_serial {
        sync_selection(device, false);
    }
    before != serde_json::to_value(&*device).expect("device serialization")
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
    fn empty_cache_creates_source_uuid_full_defaults_and_serial_selection() {
        let mut device = razer_model::product_preview::registered_preview(182).unwrap();
        device.profiles.clear();
        device.active_profile.clear();
        device.source_device_settings = None;
        device.serial_number = "actual-source-serial".into();
        assert!(initialize(&mut device));
        let profile = &device.profiles[0];
        assert_eq!(
            uuid::Uuid::parse_str(&profile.guid)
                .unwrap()
                .get_version_num(),
            4
        );
        assert_eq!(
            profile.source_settings.as_ref().unwrap()["dpiStages"]["active"],
            3
        );
        let root = device.source_device_settings.as_ref().unwrap();
        assert_eq!(root["schemaVersion"], 13);
        assert_eq!(root["isDefault"], true);
        assert_eq!(root["calibration"]["smartTracking"]["trackingDistance"], 2);
        assert_eq!(
            root["deviceMetadatas"]["actual-source-serial"]["activeProfileGuid"],
            profile.guid
        );
        assert!(!initialize(&mut device));
    }
    #[::core::prelude::v1::test]
    fn serial_metadata_selects_source_guid_without_replacing_unknown_fields() {
        let mut device = razer_model::product_preview::registered_preview(182).unwrap();
        device.profiles.clear();
        device.source_device_settings = None;
        device.serial_number = "SERIAL".into();
        initialize(&mut device);
        let mut second = default_profile(&device, "second".into()).unwrap();
        second.id = "retained-ui-domain-id".into();
        second.source_settings.as_mut().unwrap()["unknownSourceField"] =
            serde_json::json!({"retained":true});
        device.active_profile = second.id.clone();
        device.profiles.push(second);
        sync_selection(&mut device, true);
        device.active_profile = device.profiles[0].id.clone();
        initialize(&mut device);
        assert_eq!(device.active_profile, "retained-ui-domain-id");
        assert!(
            device
                .source_device_settings
                .as_ref()
                .unwrap()
                .get("isDefault")
                .is_none()
        );
        assert_eq!(
            device.profiles[1].source_settings.as_ref().unwrap()["unknownSourceField"]["retained"],
            true
        );
        device.source_device_settings.as_mut().unwrap()["deviceMetadatas"]["SERIAL"]["activeProfileGuid"] =
            "missing-source-guid".into();
        initialize(&mut device);
        assert_eq!(device.active_profile, "missing-source-guid");
    }
    #[::core::prelude::v1::test]
    fn each_new_serial_gets_its_own_default_and_returning_serial_restores_its_profile() {
        let mut device = razer_model::product_preview::registered_preview(182).unwrap();
        device.profiles.clear();
        device.source_device_settings = None;
        device.serial_number = "FIRST".into();
        initialize(&mut device);
        let first = device.profiles[0].guid.clone();
        device.profiles[0].source_settings.as_mut().unwrap()["dpiStages"]["stages"][0]["x"] =
            1200.into();
        device.serial_number = "SECOND".into();
        assert!(initialize(&mut device));
        assert_eq!(device.profiles.len(), 2);
        let second = device.active_profile_obj().unwrap().guid.clone();
        assert_ne!(first, second);
        assert_eq!(
            device
                .active_profile_obj()
                .unwrap()
                .source_settings
                .as_ref()
                .unwrap()["dpiStages"]["stages"][0]["x"],
            400
        );
        assert_eq!(
            device.source_device_settings.as_ref().unwrap()["deviceMetadatas"]["FIRST"]["activeProfileGuid"],
            first
        );
        device.serial_number = "FIRST".into();
        initialize(&mut device);
        assert_eq!(device.active_profile_obj().unwrap().guid, first);
        assert_eq!(
            device
                .active_profile_obj()
                .unwrap()
                .source_settings
                .as_ref()
                .unwrap()["dpiStages"]["stages"][0]["x"],
            1200
        );
        assert_eq!(device.profiles.len(), 2);
    }
    #[::core::prelude::v1::test]
    fn absent_serial_does_not_create_a_cache_then_real_serial_migrates_user_settings() {
        let mut device = razer_model::product_preview::registered_preview(182).unwrap();
        device.serial_number.clear();
        device.source_device_settings = None;
        device.profiles = vec![Profile {
            id: "local-default".into(),
            guid: "local-default".into(),
            name: "my settings".into(),
            dpi_stages: None,
            settings: None,
            source_settings: Some(serde_json::json!({"pollingRate":500,"opaque":{"keep":true}})),
        }];
        device.active_profile = "local-default".into();
        assert!(!initialize(&mut device));
        assert!(device.source_device_settings.is_none());
        device.serial_number = "QUERIED".into();
        assert!(initialize(&mut device));
        let profile = device.active_profile_obj().unwrap();
        assert_eq!(profile.id, "local-default");
        assert!(uuid::Uuid::parse_str(&profile.guid).is_ok());
        assert_eq!(
            profile.source_settings.as_ref().unwrap()["pollingRate"],
            500
        );
        assert_eq!(
            profile.source_settings.as_ref().unwrap()["opaque"]["keep"],
            true
        );
        assert_eq!(
            device.source_device_settings.as_ref().unwrap()["deviceMetadatas"]["QUERIED"]["activeProfileGuid"],
            profile.guid
        );
    }
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
}

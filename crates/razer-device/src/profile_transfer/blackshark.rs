//! Global Profiles USER_COMMAND to current 1398 MW56144 import processing.
use crate::{blackshark_reset, mapping_submission::next_source_version};
use anyhow::{Context as _, Result, ensure};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// A local collection candidate. Publication and its storage receipt belong to
/// the workspace; preparing this value performs no device or service writes.
#[derive(Debug)]
#[non_exhaustive]
pub struct PreparedGlobalImport1398 {
    pub profiles: Vec<Value>,
    pub device_settings: Value,
}

#[derive(Deserialize)]
struct SelectedRow {
    profile: ImportedProfile,
    #[serde(rename = "listItems")]
    list_items: Vec<Value>,
}

#[derive(Deserialize)]
struct ImportedProfile {
    name: String,
    #[serde(flatten)]
    fields: Map<String, Value>,
}

#[derive(Deserialize)]
struct ExistingProfile {
    guid: String,
    name: String,
}

#[derive(Deserialize)]
struct DeviceVersion {
    #[serde(default, deserialize_with = "present_version")]
    version: Option<u64>,
}

fn present_version<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<u64>, D::Error> {
    u64::deserialize(deserializer).map(Some)
}

/// The mounted 1398 branch ignores selected macros and cloud arguments, merges
/// each external profile over ProductInfo defaults, then appends it without
/// changing the active profile. External extension fields survive the merge.
pub fn prepare_global_import_1398(
    rows: &[Value],
    existing_profiles: &[Value],
    device_settings: Option<&Value>,
    mut fresh_guid: impl FnMut() -> String,
) -> Result<PreparedGlobalImport1398> {
    ensure!(!rows.is_empty(), "No profiles were selected for import");
    let mut settings = match device_settings {
        Some(value) => value
            .as_object()
            .context("Current BlackShark device settings are not an object")?
            .clone(),
        None => Map::new(),
    };
    let version = serde_json::from_value::<DeviceVersion>(Value::Object(settings.clone()))
        .context("Current BlackShark device version is invalid")?
        .version;
    let next_version = next_source_version(version)?;
    let mut identities = BTreeSet::new();
    for profile in existing_profiles {
        let profile: ExistingProfile = serde_json::from_value(profile.clone())
            .context("Current BlackShark profile identity or name is absent")?;
        ensure!(
            !profile.guid.is_empty() && identities.insert(profile.guid),
            "Current BlackShark profile identity is absent or repeated"
        );
        let _ = profile.name;
    }
    let selected = rows
        .iter()
        .map(|row| {
            let row: SelectedRow = serde_json::from_value(row.clone())
                .context("Imported BlackShark profile row is invalid")?;
            // MW56144.ur calls QUICK_EFFECTS.find for a truthy quickEffects.
            // Current ProductInfo13513 exports no QUICK_EFFECTS; that branch
            // fails before persistence instead of selecting a lighting default.
            ensure!(
                !row.profile
                    .fields
                    .get("quickEffects")
                    .is_some_and(super::truthy),
                "Current BlackShark import has no QUICK_EFFECTS catalog for this profile"
            );
            // The source filters this list but discards its result. A null row
            // would still fail its property access, before any local save.
            ensure!(
                !row.list_items.iter().any(Value::is_null),
                "Imported BlackShark macro selection row is null"
            );
            Ok(row.profile)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut profiles = existing_profiles.to_vec();
    for profile in selected {
        let name = super::imported_name(&profiles, &profile.name);
        let guid = fresh_guid();
        ensure!(
            identities.insert(guid.clone()),
            "Imported BlackShark profile identity is repeated"
        );
        let defaults = blackshark_reset::profile_with_identity(&guid, &name)?;
        let mut merged = serde_json::to_value(defaults)?
            .as_object()
            .expect("typed BlackShark profile serializes to an object")
            .clone();
        merged.extend(profile.fields);
        merged.insert("name".into(), name.into());
        merged.insert("guid".into(), guid.into());
        profiles.push(Value::Object(merged));
    }
    settings.insert("version".into(), next_version.into());
    Ok(PreparedGlobalImport1398 {
        profiles,
        device_settings: Value::Object(settings),
    })
}

#[cfg(test)]
#[path = "blackshark_tests.rs"]
mod tests;

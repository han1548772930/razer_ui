//! Source-confirmed local profile selection. The Chromium store is only read;
//! imported settings retain provenance and are never device observations.
mod catalog;
mod leveldb;
use anyhow::{Context, Result, ensure};
pub use catalog::{LocalProfiles, Profile, SerialInitialization, SourceSelection};
use razer_device::{mouse_current_profile::Stages, mouse_dpi_stages::DpiStagesDraft};
use razer_model::model::Device;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

const ORIGIN: &str = "https://apps.razer.com";
const OWNERSHIP_KEY: &str = "razerUiMouseDpiOwnership";

fn storage_key(key: &str) -> Vec<u8> {
    format!("_{ORIGIN}\0\u{1}{key}").into_bytes()
}

/// Read one source-owned key through the same coherent LevelDB snapshot and
/// Chromium string decoder used by the mouse profile importer.
pub(crate) fn read_local_storage(root: &Path, key: &str) -> Result<Option<String>> {
    leveldb::read(root, &storage_key(key))?
        .map(|value| text(&value.bytes))
        .transpose()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct SourceIdentity {
    product_id: u32,
    serial: String,
    guid: String,
    sequence: u64,
    sha256: String,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct ProfileOwnership {
    serial: String,
    imported: Option<SourceIdentity>,
    explicitly_edited: bool,
    stages: DpiStagesDraft,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}
#[derive(Default, Deserialize, Serialize)]
struct Ownership {
    version: u32,
    profiles: BTreeMap<String, ProfileOwnership>,
    #[serde(flatten)]
    extensions: BTreeMap<String, serde_json::Value>,
}
fn text(bytes: &[u8]) -> Result<String> {
    match bytes.split_first() {
        Some((1, bytes)) => Ok(bytes.iter().map(|b| char::from(*b)).collect()),
        Some((0, bytes)) => {
            ensure!(bytes.len() % 2 == 0, "odd Chromium UTF-16 value length");
            Ok(String::from_utf16(
                &bytes
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect::<Vec<_>>(),
            )?)
        }
        _ => anyhow::bail!("unsupported Chromium localStorage string encoding"),
    }
}

/// These current products have a verified origin, storage key and serial/GUID
/// selector. Other schemas must be audited before they enter this boundary.
pub fn supported(product_id: u32) -> bool {
    matches!(product_id, 182 | 190)
}

pub fn read(root: &Path, product_id: u32, serial: &str) -> Result<Option<LocalProfiles>> {
    ensure!(
        supported(product_id),
        "current local profile schema is not verified for this product"
    );
    ensure!(
        !serial.is_empty() && !serial.starts_with("PREVIEW-") && !serial.contains('\0'),
        "local profile requires the observed device serial"
    );
    let key = storage_key(&format!("synapse_{product_id}"));
    let Some(value) = leveldb::read(root, &key)? else {
        return Ok(None);
    };
    Ok(Some(decode(product_id, serial, value)?))
}
fn decode(product_id: u32, serial: &str, value: leveldb::Value) -> Result<LocalProfiles> {
    catalog::decode(product_id, serial, value)
}
fn ownership(device: &Device) -> Result<Ownership> {
    ensure!(
        device
            .source_device_settings
            .as_ref()
            .is_none_or(serde_json::Value::is_object),
        "device settings is not an object"
    );
    let Some(value) = device
        .source_device_settings
        .as_ref()
        .and_then(|v| v.get(OWNERSHIP_KEY))
    else {
        return Ok(Ownership {
            version: 1,
            ..Ownership::default()
        });
    };
    let state: Ownership =
        serde_json::from_value(value.clone()).context("invalid DPI profile ownership")?;
    ensure!(
        state.version == 1,
        "unsupported DPI profile ownership version"
    );
    for entry in state.profiles.values() {
        ensure!(
            entry.explicitly_edited || entry.imported.is_some(),
            "DPI profile has no recorded source or explicit edit"
        );
        razer_device::mouse_current_dpi::pack_for(device.product_id, &entry.stages)?;
        if let Some(imported) = &entry.imported {
            ensure!(
                imported.product_id == device.product_id
                    && imported.serial == entry.serial
                    && !imported.serial.is_empty()
                    && !imported.guid.is_empty()
                    && imported.sha256.len() == 64
                    && imported.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid imported DPI profile identity"
            );
        }
    }
    Ok(state)
}
fn store(device: &mut Device, mut state: Ownership) -> Result<()> {
    state
        .profiles
        .retain(|id, _| device.profiles.iter().any(|profile| profile.id == *id));
    let root = device
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}));
    let object = root
        .as_object_mut()
        .context("device settings is not an object")?;
    object.insert(OWNERSHIP_KEY.to_owned(), serde_json::to_value(state)?);
    Ok(())
}
pub fn known_stages(device: &Device) -> Result<Option<DpiStagesDraft>> {
    let state = ownership(device)?;
    Ok(state
        .profiles
        .get(&device.active_profile)
        .filter(|entry| entry.serial == device.serial_number)
        .map(|entry| entry.stages.clone()))
}
pub fn stages_without_host(device: &Device) -> Result<Option<DpiStagesDraft>> {
    let state = ownership(device)?;
    let entry = state
        .profiles
        .get(&device.active_profile)
        .filter(|entry| entry.serial == device.serial_number);
    ensure!(
        entry.is_none_or(|entry| entry.explicitly_edited || entry.imported.is_none()),
        "imported host local profile is no longer present; prior imported settings are not current"
    );
    Ok(entry
        .filter(|entry| entry.explicitly_edited)
        .map(|entry| entry.stages.clone()))
}
pub fn owns_field(field: &str) -> bool {
    field == OWNERSHIP_KEY
}
pub fn accept_saved(device: &mut Device, previous: &Device, snapshot: &Device) -> Result<()> {
    let value = |device: &Device| {
        device
            .source_device_settings
            .as_ref()
            .and_then(|v| v.get(OWNERSHIP_KEY))
            .cloned()
    };
    if value(device) == value(previous) && value(snapshot) != value(previous) {
        store(device, ownership(snapshot)?)?;
    }
    Ok(())
}

/// Refresh the complete checked catalog. Explicit local edits retain ownership
/// and cannot be overwritten by an external revision. Only local fields move;
/// connection observations, device writes and mapping publication stay separate.
pub fn import(device: &mut Device, baseline: &Device, source: &LocalProfiles) -> Result<bool> {
    crate::workspace::ensure_local_profiles_unchanged(device, baseline)?;
    ensure!(
        device.product_id == source.product_id() && device.serial_number == source.serial(),
        "host profile belongs to another device"
    );
    let mut state = ownership(device)?;
    let mut guids = std::collections::BTreeSet::new();
    let mut ids = std::collections::BTreeSet::new();
    for profile in &device.profiles {
        ensure!(
            !profile.guid.is_empty()
                && guids.insert(profile.guid.as_str())
                && !profile.id.is_empty()
                && ids.insert(profile.id.as_str()),
            "local mouse profile identities are empty or duplicated"
        );
    }
    let mut profiles = Vec::with_capacity(source.profiles().len());
    for profile in source.profiles() {
        let previous = device
            .profiles
            .iter()
            .find(|existing| existing.guid == profile.guid());
        let id = previous.map_or(profile.guid(), |existing| existing.id.as_str());
        let old = state.profiles.get(id);
        let mut identity = source.identity(profile);
        let mut extensions = BTreeMap::new();
        if let Some(old) = old {
            ensure!(
                old.serial == device.serial_number,
                "DPI profile belongs to another serial"
            );
            let same_source = old.imported.as_ref().is_some_and(|imported| {
                imported.guid == profile.guid() && imported.sha256 == identity.sha256
            });
            ensure!(
                !old.explicitly_edited || old.imported.is_none() || same_source,
                "host active profile or revision changed; local DPI profile retained"
            );
            if old.explicitly_edited {
                profiles.push(
                    previous
                        .context("edited local mouse profile is missing")?
                        .clone(),
                );
                continue;
            }
            extensions = old.extensions.clone();
            if let Some(imported) = &old.imported {
                identity.extensions = imported.extensions.clone();
            }
        }
        profiles.push(razer_model::model::Profile {
            source_settings: Some(profile.source_settings()?),
            settings: None,
            name: profile.name().to_owned(),
            guid: profile.guid().to_owned(),
            id: id.to_owned(),
            dpi_stages: None,
        });
        state.profiles.insert(
            id.to_owned(),
            ProfileOwnership {
                serial: device.serial_number.clone(),
                imported: Some(identity),
                explicitly_edited: false,
                stages: profile.stages()?,
                extensions,
            },
        );
    }
    for profile in &device.profiles {
        if profiles.iter().any(|imported| imported.id == profile.id) {
            continue;
        }
        if let Some(old) = state.profiles.get(&profile.id) {
            ensure!(
                !old.explicitly_edited || old.imported.is_none(),
                "host removed an explicitly edited profile; local DPI profile retained"
            );
            if old.explicitly_edited {
                profiles.push(profile.clone());
            }
        } else {
            profiles.push(profile.clone());
        }
    }
    let mut projected_ids = std::collections::BTreeSet::new();
    ensure!(
        profiles
            .iter()
            .all(|profile| projected_ids.insert(profile.id.as_str())),
        "imported and local mouse profile identities collide"
    );
    let active = match source.active_guid() {
        Some(guid) => profiles
            .iter()
            .find(|profile| profile.guid == guid)
            .map(|profile| profile.id.clone())
            .context("selected host mouse profile was not projected")?,
        None => String::new(),
    };
    let mut candidate = device.clone();
    candidate.profiles = profiles;
    candidate.active_profile = active;
    let mut settings = source.device_settings()?;
    if let Some(local) = &device.source_device_settings {
        let target = settings
            .as_object_mut()
            .expect("typed mouse settings object");
        for (key, value) in local
            .as_object()
            .context("device settings is not an object")?
        {
            if !target.contains_key(key) {
                target.insert(key.clone(), value.clone());
            }
        }
    }
    candidate.source_device_settings = Some(settings);
    store(&mut candidate, state)?;
    let changed = candidate.profiles != device.profiles
        || candidate.active_profile != device.active_profile
        || candidate.source_device_settings != device.source_device_settings;
    device.profiles = candidate.profiles;
    device.active_profile = candidate.active_profile;
    device.source_device_settings = candidate.source_device_settings;
    Ok(changed)
}
pub fn stage_value(stages: &DpiStagesDraft) -> serde_json::Value {
    serde_json::json!({"enable":stages.enabled,"active":stages.active_stage,"count":stages.stages.iter().filter(|s| s.visible).count(),"stages":stages.stages})
}
pub fn mark_edited(
    device: &mut Device,
    profile_id: &str,
    source: &serde_json::Value,
) -> Result<()> {
    if !supported(device.product_id) {
        return Ok(());
    }
    let stages: Stages = serde_json::from_value(
        source
            .get("dpiStages")
            .context("edited DPI profile has no stage settings")?
            .clone(),
    )?;
    let stages = stages.draft()?;
    razer_device::mouse_current_dpi::pack_for(device.product_id, &stages)?;
    let mut state = ownership(device)?;
    let imported = state
        .profiles
        .get(profile_id)
        .and_then(|p| p.imported.clone());
    let extensions = state
        .profiles
        .get(profile_id)
        .map(|p| p.extensions.clone())
        .unwrap_or_default();
    state.profiles.insert(
        profile_id.to_owned(),
        ProfileOwnership {
            serial: device.serial_number.clone(),
            imported,
            explicitly_edited: true,
            stages,
            extensions,
        },
    );
    store(device, state)
}

#[cfg(test)]
#[path = "host_local_profile/profile_tests.rs"]
mod tests;

//! Source-confirmed local profile selection. The Chromium store is only read;
//! imported settings retain provenance and are never device observations.
mod leveldb;
use anyhow::{Context, Result, ensure};
use razer_device::mouse_dpi_stages::{DpiStage, DpiStagesDraft};
use razer_model::model::Device;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

const ORIGIN: &str = "https://apps.razer.com";
const OWNERSHIP_KEY: &str = "razerUiMouseDpiOwnership";

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
#[derive(Clone, Debug)]
pub struct LocalProfile {
    identity: SourceIdentity,
    name: String,
    stages: DpiStagesDraft,
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
#[derive(Deserialize)]
struct Document {
    #[serde(rename = "productId")]
    product_id: u32,
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    #[serde(rename = "deviceMetadatas")]
    metadata: BTreeMap<String, Metadata>,
    profiles: Vec<Profile>,
}
#[derive(Deserialize)]
struct Metadata {
    #[serde(rename = "activeProfileGuid")]
    active: String,
}
#[derive(Deserialize)]
struct Profile {
    guid: String,
    name: String,
    #[serde(rename = "dpiStages")]
    stages: Stages,
}
#[derive(Deserialize)]
struct Stages {
    enable: bool,
    active: usize,
    count: usize,
    stages: Vec<DpiStage>,
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

pub fn read(root: &Path, product_id: u32, serial: &str) -> Result<Option<LocalProfile>> {
    ensure!(
        supported(product_id),
        "current local profile schema is not verified for this product"
    );
    ensure!(
        !serial.is_empty() && !serial.starts_with("PREVIEW-"),
        "local profile requires the observed device serial"
    );
    let key = format!("_{ORIGIN}\0\u{1}synapse_{product_id}");
    let Some(value) = leveldb::read(root, key.as_bytes())? else {
        return Ok(None);
    };
    decode(product_id, serial, value)
}
fn decode(product_id: u32, serial: &str, value: leveldb::Value) -> Result<Option<LocalProfile>> {
    let decoded = text(&value.bytes)?;
    let document: Document =
        serde_json::from_str(&decoded).context("invalid current host local profile")?;
    ensure!(
        document.product_id == product_id,
        "host profile product does not match the requested storage key"
    );
    ensure!(
        document.schema_version == 13,
        "host local profile requires the current schema version 13"
    );
    let Some(metadata) = document.metadata.get(serial) else {
        return Ok(None);
    };
    ensure!(
        !metadata.active.is_empty(),
        "host local profile has an empty active GUID"
    );
    let mut matching = document
        .profiles
        .into_iter()
        .filter(|p| p.guid == metadata.active);
    let profile = matching
        .next()
        .context("host serial selects a missing profile GUID")?;
    ensure!(matching.next().is_none(), "host profile GUID is duplicated");
    let stages = validate(Stages { ..profile.stages })?;
    razer_device::mouse_current_dpi::pack_for(product_id, &stages)?;
    let hash = ring::digest::digest(&ring::digest::SHA256, &value.bytes);
    let sha256 = hash.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    Ok(Some(LocalProfile {
        identity: SourceIdentity {
            product_id,
            serial: serial.to_owned(),
            guid: profile.guid,
            sequence: value.sequence,
            sha256,
            extensions: BTreeMap::new(),
        },
        name: profile.name,
        stages,
    }))
}
fn validate(value: Stages) -> Result<DpiStagesDraft> {
    ensure!(
        value.stages.len() == 5 && (1..=5).contains(&value.active),
        "invalid current host DPI row selection"
    );
    ensure!(
        (1..=5).contains(&value.count)
            && value.stages.iter().filter(|s| s.visible).count() == value.count,
        "invalid current host DPI visibility/count"
    );
    ensure!(
        value.stages.iter().all(|s| s.x > 0 && s.y > 0),
        "invalid current host DPI value"
    );
    Ok(DpiStagesDraft {
        enabled: value.enable,
        active_stage: value.active,
        stages: value.stages,
    })
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

/// Refresh source-owned metadata from the exact host revision. Explicit local
/// edits retain ownership and cannot be overwritten by an external revision.
pub fn import(device: &mut Device, source: &LocalProfile) -> Result<bool> {
    ensure!(
        device.product_id == source.identity.product_id
            && device.serial_number == source.identity.serial,
        "host profile belongs to another device"
    );
    let mut state = ownership(device)?;
    if let Some(previous) = state.profiles.get(&device.active_profile) {
        ensure!(
            previous.serial == device.serial_number,
            "DPI profile belongs to another serial"
        );
        let same_source = previous.imported.as_ref().is_some_and(|imported| {
            imported.guid == source.identity.guid && imported.sha256 == source.identity.sha256
        });
        if previous.explicitly_edited {
            ensure!(
                previous.imported.is_none() || same_source,
                "host active profile or revision changed; local DPI profile retained"
            );
            return Ok(false);
        }
        if same_source {
            return Ok(false);
        }
    }
    let profile = device
        .profiles
        .iter_mut()
        .find(|p| p.id == device.active_profile)
        .context("active local DPI profile is missing")?;
    let root = profile
        .source_settings
        .get_or_insert_with(|| serde_json::json!({}));
    let root = root
        .as_object_mut()
        .context("local DPI profile is not an object")?;
    root.insert("dpiStages".to_owned(), stage_value(&source.stages));
    root.insert("guid".to_owned(), source.identity.guid.clone().into());
    root.insert("name".to_owned(), source.name.clone().into());
    profile.guid = source.identity.guid.clone();
    profile.name = source.name.clone();
    let extensions = state
        .profiles
        .get(&device.active_profile)
        .map(|entry| entry.extensions.clone())
        .unwrap_or_default();
    let mut imported = source.identity.clone();
    if let Some(previous) = state
        .profiles
        .get(&device.active_profile)
        .and_then(|entry| entry.imported.as_ref())
    {
        imported.extensions = previous.extensions.clone();
    }
    state.profiles.insert(
        device.active_profile.clone(),
        ProfileOwnership {
            serial: device.serial_number.clone(),
            imported: Some(imported),
            explicitly_edited: false,
            stages: source.stages.clone(),
            extensions,
        },
    );
    store(device, state)?;
    Ok(true)
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
    let stages = validate(serde_json::from_value(
        source
            .get("dpiStages")
            .context("edited DPI profile has no stage settings")?
            .clone(),
    )?)?;
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

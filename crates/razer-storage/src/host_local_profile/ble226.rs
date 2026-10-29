//! Current226 MW77846 serial selection and the ordered tn/It profile input.
//! The Chromium snapshot remains read-only; absent data never creates a profile.
use super::{SerialInitialization, SourceSelection, leveldb, storage_key, text};
use anyhow::{Context as _, Result, ensure};
use razer_device::mouse_current_dpi::{SourceStages, ble::initialization};
use razer_model::model::Device;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

#[path = "ble226_migration.rs"]
mod migration;

const PRODUCT_ID: u32 = 226;
const STORAGE_KEY: &str = "synapse_226";
const REVISION_KEY: &str = "razerUiBle226Profiles";
type Extensions = BTreeMap<String, Value>;

pub(super) fn owns_field(field: &str) -> bool {
    field == REVISION_KEY
}

fn present<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn stages<'de, D>(deserializer: D) -> std::result::Result<Option<SourceStages>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    SourceStages::from_source_file(&value)
        .map(Some)
        .map_err(serde::de::Error::custom)
}

fn source_stages<S>(value: &Option<SourceStages>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    value
        .as_ref()
        .map(SourceStages::source_file_value)
        .serialize(serializer)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    active_profile_guid: String,
    #[serde(flatten)]
    extensions: Extensions,
}

/// Only fields consumed by tn/It/Xc enter business decisions. Other current226
/// settings remain intact as external-file extensions, including vendor spelling.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    guid: String,
    name: String,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    obm_slot_id: Option<Vec<u8>>,
    #[serde(
        default,
        deserialize_with = "stages",
        serialize_with = "source_stages",
        skip_serializing_if = "Option::is_none"
    )]
    dpi_stages: Option<SourceStages>,
    #[serde(flatten)]
    extensions: Extensions,
}

impl Profile {
    pub fn guid(&self) -> &str {
        &self.guid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn source_settings(&self) -> Result<Value> {
        Ok(serde_json::to_value(self)?)
    }

    fn initialization_profile(&self) -> initialization::LocalProfile {
        initialization::LocalProfile {
            guid: self.guid.clone(),
            name: self.name.clone(),
            // Xc filters profiles with obmSlotId before matching; absence is
            // an empty association, never an inferred onboard slot.
            obm_slot_ids: match &self.obm_slot_id {
                Some(ids) => ids.clone(),
                None => vec![],
            },
            dpi_stages: self.dpi_stages.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    version: u64,
    product_id: u32,
    edition_id: u32,
    category: String,
    schema_version: u32,
    #[serde(rename = "reportIDs")]
    report_ids: BTreeMap<u8, super::catalog::ReportKind>,
    profiles: Vec<Profile>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    device_metadatas: Option<BTreeMap<String, Metadata>>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    active_profile: Option<String>,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug)]
pub struct Catalog {
    serial: String,
    document: Document,
    sequence: u64,
    sha256: String,
    selection: SourceSelection,
}

#[derive(Deserialize, Serialize)]
struct ImportedRevision {
    version: u32,
    serial: String,
    sequence: u64,
    sha256: String,
    #[serde(flatten)]
    extensions: Extensions,
}

impl Catalog {
    pub fn serial(&self) -> &str {
        &self.serial
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Provenance of the exact Chromium value bytes, not a compatibility gate.
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    pub fn profiles(&self) -> &[Profile] {
        &self.document.profiles
    }

    pub fn selection(&self) -> &SourceSelection {
        &self.selection
    }

    pub fn source_document(&self) -> Result<Value> {
        Ok(serde_json::to_value(&self.document)?)
    }

    pub fn initialization_request(&self, owner: &str) -> Result<Option<initialization::Request>> {
        let SourceSelection::SerialMetadata { guid } = &self.selection else {
            return Ok(None);
        };
        request(&self.document.profiles, guid, owner).map(Some)
    }

    /// Import local configuration only after this connection supplied its real
    /// serial. Device observations and application-owned fields retain ownership.
    pub fn import_into(&self, device: &mut Device, baseline: &Device) -> Result<bool> {
        ensure!(
            device.product_id == PRODUCT_ID
                && baseline.product_id == PRODUCT_ID
                && !device.device_container_id.is_empty()
                && !device.device_container_id.starts_with("preview-")
                && device.device_container_id == baseline.device_container_id,
            "BLE226 catalog endpoint or product changed"
        );
        validate_serial(&device.serial_number)?;
        ensure!(
            device.serial_number == self.serial,
            "BLE226 catalog belongs to another serial"
        );
        ensure!(
            baseline.serial_number.is_empty()
                || baseline.serial_number.starts_with("PREVIEW-")
                || baseline.serial_number == self.serial,
            "BLE226 baseline belongs to another serial"
        );
        crate::workspace::ensure_local_profiles_unchanged(device, baseline)?;
        let SourceSelection::SerialMetadata { guid } = &self.selection else {
            return Ok(false);
        };
        let mut local_ids = std::collections::BTreeSet::new();
        let mut local_guids = std::collections::BTreeSet::new();
        ensure!(
            device.profiles.iter().all(|profile| {
                !profile.id.is_empty()
                    && !profile.guid.is_empty()
                    && local_ids.insert(&profile.id)
                    && local_guids.insert(&profile.guid)
            }),
            "BLE226 local profile identities are empty or duplicated"
        );
        let mut profiles = Vec::with_capacity(self.profiles().len());
        for source in self.profiles() {
            let previous = device
                .profiles
                .iter()
                .find(|local| local.guid == source.guid);
            let mut local = previous
                .cloned()
                .unwrap_or_else(|| razer_model::model::Profile {
                    id: source.guid.clone(),
                    guid: source.guid.clone(),
                    name: source.name.clone(),
                    source_settings: None,
                    settings: None,
                    dpi_stages: None,
                });
            local.guid.clone_from(&source.guid);
            local.name.clone_from(&source.name);
            let mut settings = source.source_settings()?;
            if let Some(previous) = previous.and_then(|profile| profile.source_settings.as_ref()) {
                merge_unowned(
                    &mut settings,
                    previous,
                    &["guid", "name", "dpiStages", "obmSlotId"],
                )?;
            }
            local.source_settings = Some(settings);
            profiles.push(local);
        }
        for local in &device.profiles {
            if !profiles.iter().any(|source| source.guid == local.guid) {
                profiles.push(local.clone());
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        ensure!(
            profiles
                .iter()
                .all(|profile| !profile.id.is_empty() && ids.insert(&profile.id)),
            "BLE226 imported and local profile identities collide"
        );
        let active = profiles
            .iter()
            .find(|profile| &profile.guid == guid)
            .context("BLE226 serial-selected profile was not imported")?
            .id
            .clone();
        let mut settings = self.source_document()?;
        let root = settings.as_object_mut().expect("typed BLE226 document");
        root.remove("profiles");
        root.insert("activeProfile".into(), guid.clone().into());
        if let Some(previous) = &device.source_device_settings {
            merge_unowned(
                &mut settings,
                previous,
                &["profiles", "deviceMetadatas", "activeProfile", REVISION_KEY],
            )?;
        }
        let revision = device
            .source_device_settings
            .as_ref()
            .and_then(|root| root.get(REVISION_KEY))
            .map(|value| serde_json::from_value::<ImportedRevision>(value.clone()))
            .transpose()?;
        if let Some(revision) = &revision {
            ensure!(
                revision.version == 1,
                "BLE226 local source revision version is unsupported"
            );
        }
        let extensions = revision
            .map(|revision| revision.extensions)
            .unwrap_or_default();
        settings
            .as_object_mut()
            .expect("typed BLE226 settings")
            .insert(
                REVISION_KEY.into(),
                serde_json::to_value(ImportedRevision {
                    version: 1,
                    serial: self.serial.clone(),
                    sequence: self.sequence,
                    sha256: self.sha256.clone(),
                    extensions,
                })?,
            );
        let changed = device.profiles != profiles
            || device.active_profile != active
            || device.source_device_settings.as_ref() != Some(&settings);
        device.profiles = profiles;
        device.active_profile = active;
        device.source_device_settings = Some(settings);
        Ok(changed)
    }
}

fn merge_unowned(target: &mut Value, previous: &Value, owned: &[&str]) -> Result<()> {
    let target = target
        .as_object_mut()
        .context("BLE226 imported settings is not an object")?;
    let previous = previous
        .as_object()
        .context("BLE226 local settings is not an object")?;
    for (field, value) in previous {
        if !owned.contains(&field.as_str()) && !target.contains_key(field) {
            target.insert(field.clone(), value.clone());
        }
    }
    Ok(())
}

fn validate_serial(serial: &str) -> Result<()> {
    ensure!(
        !serial.is_empty()
            && !serial.starts_with("PREVIEW-")
            && !serial.contains('\0')
            && !matches!(serial, "NOSERIALNUMBER" | "DEFERSERIALNUMBER"),
        "BLE226 profiles require the actual observed serial"
    );
    Ok(())
}

fn request(profiles: &[Profile], selected: &str, owner: &str) -> Result<initialization::Request> {
    ensure!(
        !owner.is_empty() && !owner.contains('\0'),
        "BLE226 local OBM owner is unobserved"
    );
    let request = initialization::Request {
        profiles: profiles
            .iter()
            .map(Profile::initialization_profile)
            .collect(),
        active_profile_guid: selected.into(),
        owner: owner.into(),
    };
    request.validate()?;
    Ok(request)
}

pub fn read(root: &Path, serial: &str) -> Result<Option<Catalog>> {
    read_with(serial, |key| leveldb::read(root, key))
}

fn read_with(
    serial: &str,
    read: impl FnOnce(&[u8]) -> Result<Option<leveldb::Value>>,
) -> Result<Option<Catalog>> {
    validate_serial(serial)?;
    read(&storage_key(STORAGE_KEY))?
        .map(|value| decode(serial, value))
        .transpose()
}

fn decode(serial: &str, value: leveldb::Value) -> Result<Catalog> {
    validate_serial(serial)?;
    let external: Value = serde_json::from_str(&text(&value.bytes)?)
        .context("invalid current226 local profile JSON")?;
    let document: Document = serde_json::from_value(migration::migrate(external)?)
        .context("invalid current226 local profile document")?;
    ensure!(
        document.product_id == PRODUCT_ID && document.category == "MOUSE",
        "BLE226 storage identity differs"
    );
    ensure!(
        matches!(document.edition_id, 0 | 128),
        "BLE226 storage edition is unknown"
    );
    ensure!(
        document.schema_version == 13 && document.version <= (1_u64 << 53) - 1,
        "BLE226 storage schema or revision is unsupported"
    );
    ensure!(
        document.report_ids.get(&4) == Some(&super::catalog::ReportKind::RazerKey)
            && document.report_ids.get(&5) == Some(&super::catalog::ReportKind::HardwareEvent),
        "BLE226 report metadata is incomplete"
    );
    let mut guids = std::collections::BTreeSet::new();
    for profile in &document.profiles {
        ensure!(
            !profile.guid.is_empty() && guids.insert(profile.guid.as_str()),
            "BLE226 profile GUID is empty or duplicated"
        );
        if let Some(stages) = &profile.dpi_stages {
            razer_device::mouse_current_dpi::pack_for(PRODUCT_ID, stages)?;
        }
        let ids = profile.obm_slot_id.as_deref().unwrap_or(&[]);
        ensure!(
            ids.iter()
                .enumerate()
                .all(|(index, id)| *id != 0 && !ids[..index].contains(id)),
            "BLE226 profile slot associations are invalid"
        );
    }
    let selection = selection(document.device_metadatas.as_ref(), serial, &guids)?;
    let hash = ring::digest::digest(&ring::digest::SHA256, &value.bytes);
    Ok(Catalog {
        serial: serial.into(),
        document,
        sequence: value.sequence,
        sha256: hash
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        selection,
    })
}

fn selection(
    metadata: Option<&BTreeMap<String, Metadata>>,
    serial: &str,
    guids: &std::collections::BTreeSet<&str>,
) -> Result<SourceSelection> {
    Ok(match metadata.and_then(|metadata| metadata.get(serial)) {
        Some(metadata) if !metadata.active_profile_guid.is_empty() => {
            ensure!(
                guids.contains(metadata.active_profile_guid.as_str()),
                "BLE226 serial selects a missing profile GUID"
            );
            SourceSelection::SerialMetadata {
                guid: metadata.active_profile_guid.clone(),
            }
        }
        Some(_) => SourceSelection::NeedsSerialInitialization {
            reason: SerialInitialization::EmptyActiveGuid,
        },
        None => SourceSelection::NeedsSerialInitialization {
            reason: SerialInitialization::MissingMetadata,
        },
    })
}

/// A retained application snapshot may contain explicitly saved local profiles.
/// Its source GUID/name and exact serial metadata must agree before reuse.
pub fn from_device(
    device: &Device,
    serial: &str,
    owner: &str,
) -> Result<Option<initialization::Request>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct DeviceSettings {
        #[serde(default, deserialize_with = "present")]
        product_id: Option<u32>,
        #[serde(default, deserialize_with = "present")]
        device_metadatas: Option<BTreeMap<String, Metadata>>,
    }
    ensure!(
        device.product_id == PRODUCT_ID,
        "BLE226 local product differs"
    );
    validate_serial(serial)?;
    let Some(source) = &device.source_device_settings else {
        return Ok(None);
    };
    let source: DeviceSettings = serde_json::from_value(source.clone())?;
    ensure!(
        source
            .product_id
            .is_none_or(|product| product == PRODUCT_ID),
        "BLE226 local source product differs"
    );
    let Some(metadata) = source
        .device_metadatas
        .as_ref()
        .and_then(|metadata| metadata.get(serial))
    else {
        return Ok(None);
    };
    if metadata.active_profile_guid.is_empty() {
        return Ok(None);
    }
    let profiles = device
        .profiles
        .iter()
        .map(|local| {
            let profile: Profile = serde_json::from_value(
                local
                    .source_settings
                    .clone()
                    .context("BLE226 local profile has no source settings")?,
            )?;
            ensure!(
                profile.guid == local.guid && profile.name == local.name,
                "BLE226 local profile metadata differs from its source document"
            );
            Ok(profile)
        })
        .collect::<Result<Vec<_>>>()?;
    request(&profiles, &metadata.active_profile_guid, owner).map(Some)
}

#[cfg(test)]
#[path = "ble226_tests.rs"]
mod tests;

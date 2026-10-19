//! Read-only current keyboard profile documents. These are local profile
//! settings and serial-specific selection, never confirmed device state.
pub use crate::workspace::same_local_profiles;
use anyhow::{Context as _, Result, ensure};
use razer_device::keyboard_current_profile::{ProfileSettings, validate_profile_settings};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

type Extensions = BTreeMap<String, serde_json::Value>;

fn present<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Current89423 getActiveProfile uses the first existing local profile only
/// when this serial has no metadata record. It does not repair a dangling GUID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionOrigin {
    SerialMetadata,
    FirstProfileWithoutSerialMetadata,
    EmptyCollection,
}

#[derive(Clone, Debug)]
pub struct LocalProfiles {
    serial: String,
    document: Document,
    active: Option<usize>,
    selection: SelectionOrigin,
}

/// A local workspace projection. Applying it does not confirm any hardware
/// state, compile mappings, or change the device's observed identity.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalProfileProjection {
    profiles: Vec<razer_model::model::Profile>,
    active_profile: Option<String>,
    source_device_settings: serde_json::Value,
}

impl LocalProfileProjection {
    pub fn profiles(&self) -> &[razer_model::model::Profile] {
        &self.profiles
    }

    pub fn active_profile(&self) -> Option<&str> {
        self.active_profile.as_deref()
    }

    pub fn source_device_settings(&self) -> &serde_json::Value {
        &self.source_device_settings
    }

    pub fn into_parts(
        self,
    ) -> (
        Vec<razer_model::model::Profile>,
        Option<String>,
        serde_json::Value,
    ) {
        (
            self.profiles,
            self.active_profile,
            self.source_device_settings,
        )
    }
}

impl LocalProfiles {
    pub fn product_id(&self) -> u32 {
        self.document.product_id
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    pub fn profiles(&self) -> &[Profile] {
        &self.document.profiles
    }

    pub fn active_profile(&self) -> Option<&Profile> {
        self.active.map(|index| &self.document.profiles[index])
    }

    pub fn active_guid(&self) -> Option<&str> {
        self.active_profile().map(Profile::guid)
    }

    pub fn selection_origin(&self) -> SelectionOrigin {
        self.selection
    }

    pub fn project(&self) -> Result<LocalProfileProjection> {
        let profiles = self
            .profiles()
            .iter()
            .map(|profile| {
                Ok(razer_model::model::Profile {
                    source_settings: Some(profile.source_settings()?),
                    settings: None,
                    name: profile.name().to_owned(),
                    guid: profile.guid().to_owned(),
                    id: profile.guid().to_owned(),
                    dpi_stages: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(LocalProfileProjection {
            profiles,
            active_profile: self.active_guid().map(str::to_owned),
            source_device_settings: self.device_settings()?,
        })
    }

    /// Project the source's getLocalStorageDeviceCache serial selector into
    /// activeProfile while retaining every original profile and extension.
    pub fn source_document(&self) -> Result<serde_json::Value> {
        let mut document = self.document.clone();
        document.active_profile = self.active_guid().map(str::to_owned);
        Ok(serde_json::to_value(document)?)
    }

    /// Device-level local settings remain separate from the profile records.
    pub fn device_settings(&self) -> Result<serde_json::Value> {
        let mut document = self.document.clone();
        document.profiles.clear();
        document.active_profile = self.active_guid().map(str::to_owned);
        let mut value = serde_json::to_value(document)?;
        value
            .as_object_mut()
            .expect("typed keyboard document is an object")
            .remove("profiles");
        Ok(value)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    version: u64,
    vendor_id: u32,
    product_id: u32,
    category: String,
    schema_version: u32,
    profiles: Vec<Profile>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    active_profile: Option<String>,
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
    is_default: Option<bool>,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Metadata {
    active_profile_guid: String,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    name: Option<String>,
    #[serde(flatten)]
    extensions: Extensions,
}

/// Current keyboard settings required by their profile consumers. Other
/// external fields are preserved at the serialization boundary.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    guid: String,
    name: String,
    #[serde(flatten)]
    settings: ProfileSettings,
}

impl Profile {
    pub fn guid(&self) -> &str {
        &self.guid
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// External source profile payload for the existing product compiler.
    /// This does not compile, initialize, publish, or write the device.
    pub fn source_settings(&self) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(self)?)
    }
}

pub fn supported(product_id: u32) -> bool {
    matches!(product_id, 678 | 679 | 688 | 719)
}

/// Import one checked local revision without touching observations or mapping
/// publication. The caller owns persistence and connection-generation checks.
pub fn import(
    device: &mut razer_model::model::Device,
    baseline: &razer_model::model::Device,
    source: &LocalProfiles,
) -> Result<bool> {
    crate::workspace::ensure_local_profiles_unchanged(device, baseline)?;
    ensure!(
        device.product_id == source.product_id() && device.serial_number == source.serial(),
        "keyboard local profile identity changed during reading"
    );
    let (profiles, selected, settings) = source.project()?.into_parts();
    let active = match selected {
        Some(guid) => guid,
        None => String::new(),
    };
    let changed = device.profiles != profiles
        || device.active_profile != active
        || device.source_device_settings.as_ref() != Some(&settings);
    device.profiles = profiles;
    device.active_profile = active;
    device.source_device_settings = Some(settings);
    Ok(changed)
}

/// Missing is returned only for an absent exact host key. Snapshot corruption,
/// malformed fields and an invalid serial selection are errors.
pub fn read(root: &Path, product_id: u32, serial: &str) -> Result<Option<LocalProfiles>> {
    validate_identity(product_id, serial)?;
    let key = format!("synapse_{product_id}");
    crate::host_local_profile::read_local_storage(root, &key)?
        .map(|text| decode(product_id, serial, &text))
        .transpose()
}

pub fn decode(product_id: u32, serial: &str, text: &str) -> Result<LocalProfiles> {
    validate_identity(product_id, serial)?;
    let document: Document =
        serde_json::from_str(text).context("invalid current host keyboard profile document")?;
    ensure!(
        document.product_id == product_id
            && document.vendor_id == 5426
            && document.category == "KEYBOARD",
        "host keyboard profile identity does not match its storage key"
    );
    ensure!(
        document.schema_version == 13,
        "host keyboard profiles require current schema version 13"
    );
    ensure!(
        document.version <= (1u64 << 53) - 1,
        "host keyboard profile version exceeds the source integer range"
    );
    let mut guids = std::collections::BTreeSet::new();
    for profile in &document.profiles {
        ensure!(
            !profile.guid.is_empty() && guids.insert(profile.guid.as_str()),
            "host keyboard profile GUID is empty or duplicated"
        );
        validate_profile_settings(product_id, &profile.settings)?;
    }
    let metadata = document
        .device_metadatas
        .as_ref()
        .and_then(|records| records.get(serial));
    let (active, selection) = if let Some(metadata) = metadata {
        ensure!(
            !metadata.active_profile_guid.is_empty(),
            "host serial selects an empty keyboard profile GUID"
        );
        let index = document
            .profiles
            .iter()
            .position(|profile| profile.guid == metadata.active_profile_guid)
            .context("host serial selects a missing keyboard profile GUID")?;
        (Some(index), SelectionOrigin::SerialMetadata)
    } else if document.profiles.is_empty() {
        (None, SelectionOrigin::EmptyCollection)
    } else {
        (Some(0), SelectionOrigin::FirstProfileWithoutSerialMetadata)
    };
    Ok(LocalProfiles {
        serial: serial.to_owned(),
        document,
        active,
        selection,
    })
}

fn validate_identity(product_id: u32, serial: &str) -> Result<()> {
    ensure!(
        supported(product_id),
        "current host keyboard profile schema is not verified for this product"
    );
    ensure!(
        !serial.is_empty() && !serial.starts_with("PREVIEW-"),
        "host keyboard profiles require the actual observed serial"
    );
    Ok(())
}

#[cfg(test)]
#[path = "host_keyboard_profiles_tests.rs"]
mod tests;

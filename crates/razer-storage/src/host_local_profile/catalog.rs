//! Current182 MW77846 and current190 MW60763 complete local mouse catalogs.
//! Serial initialization is a separate source operation, never a failed-read
//! fallback or a statement about the hardware's configuration.
use super::{InitializationOrigin, SourceIdentity, leveldb, supported, text};
use anyhow::{Context as _, Result, ensure};
pub use razer_device::mouse_current_profile::Profile;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

type Extensions = BTreeMap<String, serde_json::Value>;

fn present<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SerialInitialization {
    MissingMetadata,
    EmptyActiveGuid,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceSelection {
    SerialMetadata { guid: String },
    NeedsSerialInitialization { reason: SerialInitialization },
}

#[derive(Clone, Debug)]
pub struct LocalProfiles {
    serial: String,
    document: Document,
    sequence: u64,
    sha256: String,
    selection: SourceSelection,
    initialized: Option<InitializationOrigin>,
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

    pub fn selection(&self) -> &SourceSelection {
        &self.selection
    }

    pub fn active_guid(&self) -> Option<&str> {
        match &self.selection {
            SourceSelection::SerialMetadata { guid } => Some(guid),
            SourceSelection::NeedsSerialInitialization { .. } => None,
        }
    }

    pub fn active_profile(&self) -> Option<&Profile> {
        let guid = self.active_guid()?;
        self.profiles()
            .iter()
            .find(|profile| profile.guid() == guid)
    }

    pub fn source_document(&self) -> Result<serde_json::Value> {
        let mut document = self.document.clone();
        document.active_profile = self.active_guid().map(str::to_owned);
        Ok(serde_json::to_value(document)?)
    }

    pub fn device_settings(&self) -> Result<serde_json::Value> {
        let mut value = self.source_document()?;
        value
            .as_object_mut()
            .expect("typed mouse document is an object")
            .remove("profiles");
        Ok(value)
    }

    pub(super) fn identity(&self, profile: &Profile) -> Option<SourceIdentity> {
        if self.initialized(profile).is_some() {
            return None;
        }
        Some(SourceIdentity {
            product_id: self.product_id(),
            serial: self.serial.clone(),
            guid: profile.guid().to_owned(),
            sequence: self.sequence,
            sha256: self.sha256.clone(),
            extensions: Extensions::new(),
        })
    }

    pub(super) fn initialized(&self, profile: &Profile) -> Option<&InitializationOrigin> {
        self.initialized
            .as_ref()
            .filter(|origin| origin.guid == profile.guid())
    }

    pub(super) fn revision(&self) -> (u64, &str) {
        (self.sequence, &self.sha256)
    }

    pub(super) fn with_initialized(
        &self,
        profile: Profile,
        origin: InitializationOrigin,
        device_name: &str,
    ) -> Result<Self> {
        ensure!(
            self.initialized.is_none(),
            "serial profile was already initialized"
        );
        ensure!(
            matches!(
                self.selection,
                SourceSelection::NeedsSerialInitialization { .. }
            ),
            "serial already has a selected source profile"
        );
        ensure!(
            !self
                .profiles()
                .iter()
                .any(|existing| existing.guid() == profile.guid()),
            "new serial profile GUID already exists"
        );
        profile.validate_for(self.product_id())?;
        let mut result = self.clone();
        let records = result
            .document
            .device_metadatas
            .get_or_insert_with(BTreeMap::new);
        let metadata = records
            .entry(self.serial.clone())
            .or_insert_with(|| Metadata {
                active_profile_guid: String::new(),
                name: None,
                extensions: Extensions::new(),
            });
        metadata.active_profile_guid = profile.guid().to_owned();
        metadata.name.get_or_insert_with(|| device_name.to_owned());
        result.document.active_profile = Some(profile.guid().to_owned());
        result.selection = SourceSelection::SerialMetadata {
            guid: profile.guid().to_owned(),
        };
        result.document.profiles.push(profile);
        result.initialized = Some(origin);
        Ok(result)
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
    #[serde(rename = "reportIDs")]
    report_ids: BTreeMap<u8, ReportKind>,
    calibration: Calibration,
    low_power_mode: u8,
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
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    is_default: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    sensitivity_matcher: Option<MatcherLibrary>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    selected_sensitivity_matcher: Option<String>,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub(super) enum ReportKind {
    #[serde(rename = "razerKeyReportID")]
    RazerKey,
    #[serde(rename = "hardwareEventReportID")]
    HardwareEvent,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Calibration {
    smart_tracking: SmartTracking,
    profiles: Vec<CalibrationProfile>,
    selected_profile: Extensions,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    calibration_tracking_mode: Option<bool>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    lift_off_range_value: Option<u8>,
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    preset_tracking: Option<PresetTracking>,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SmartTracking {
    is_asymmetric: bool,
    tracking_distance: u8,
    lift_off_distance: u8,
    landing_distance: u8,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PresetTracking {
    is_asymmetric: bool,
    surface_distance: u8,
    lift_off_distance: u8,
    landing_distance: u8,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CalibrationProfile {
    r#type: String,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MatcherLibrary {
    profiles: Vec<MatcherProfile>,
    #[serde(flatten)]
    extensions: Extensions,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MatcherProfile {
    guid: String,
    name: String,
    #[serde(flatten)]
    extensions: Extensions,
}

pub(super) fn decode(
    product_id: u32,
    serial: &str,
    value: leveldb::Value,
) -> Result<LocalProfiles> {
    ensure!(
        supported(product_id),
        "current host mouse profile schema is not verified for this product"
    );
    ensure!(
        !serial.is_empty() && !serial.starts_with("PREVIEW-") && !serial.contains('\0'),
        "host mouse profiles require the actual observed serial"
    );
    let document: Document = serde_json::from_str(&text(&value.bytes)?)
        .context("invalid current host mouse profile document")?;
    ensure!(
        document.product_id == product_id
            && document.vendor_id == 5426
            && document.category == "MOUSE",
        "host mouse profile identity does not match its storage key"
    );
    ensure!(
        document.schema_version == 13,
        "host mouse profiles require current schema version 13"
    );
    ensure!(
        document.version <= (1u64 << 53) - 1,
        "host mouse profile version exceeds the source integer range"
    );
    ensure!(
        document.report_ids.get(&4) == Some(&ReportKind::RazerKey)
            && document.report_ids.get(&5) == Some(&ReportKind::HardwareEvent),
        "host mouse report metadata is incomplete"
    );
    ensure!(
        document.low_power_mode <= 100,
        "host mouse low-power threshold exceeds 100"
    );
    if product_id == 182 {
        ensure!(
            document.calibration.calibration_tracking_mode.is_some()
                && document.calibration.lift_off_range_value.is_some()
                && document.calibration.preset_tracking.is_some(),
            "host current182 calibration settings are incomplete"
        );
    } else {
        ensure!(
            document.sensitivity_matcher.is_some()
                && document.selected_sensitivity_matcher.is_some(),
            "host current190 sensitivity matcher settings are absent"
        );
    }
    let mut guids = BTreeSet::new();
    for profile in &document.profiles {
        profile.validate_for(product_id)?;
        ensure!(
            guids.insert(profile.guid()),
            "host mouse profile GUID is duplicated"
        );
    }
    if let Some(library) = &document.sensitivity_matcher {
        let mut guids = BTreeSet::new();
        ensure!(
            library
                .profiles
                .iter()
                .all(|profile| !profile.guid.is_empty() && guids.insert(&profile.guid)),
            "host sensitivity matcher GUID is empty or duplicated"
        );
    }
    let selection = match document
        .device_metadatas
        .as_ref()
        .and_then(|records| records.get(serial))
    {
        Some(metadata) if !metadata.active_profile_guid.is_empty() => {
            ensure!(
                guids.contains(metadata.active_profile_guid.as_str()),
                "host serial selects a missing mouse profile GUID"
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
    };
    let hash = ring::digest::digest(&ring::digest::SHA256, &value.bytes);
    Ok(LocalProfiles {
        serial: serial.into(),
        document,
        sequence: value.sequence,
        sha256: hash
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        selection,
        initialized: None,
    })
}

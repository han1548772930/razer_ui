//! Local profile owner for the PROTOCOL40 earbuds auto-pause and PID2657 ANC
//! fields. This app owns the persisted format; it is a local profile, never a
//! device observation. Profile selection follows the source serial-metadata
//! selector: the active GUID's profile, else the first profile when there is
//! no active GUID. An active GUID that matches no profile is reported as
//! `Unmatched` and not resolved to any profile.
use anyhow::{Context as _, Result, bail};
use razer_device::earbuds_protocol40::{
    self as proto, AncChange, AncLevel, AncMode, AncSetting, supports_anc, supports_auto_pause,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoPauseControl {
    pub is_enabled: bool,
}

/// Persisted `{isEnabled, status, level}`; `status` 10/255, `level` 1/10.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoiseCancellation {
    pub is_enabled: bool,
    pub status: u8,
    pub level: u8,
}

impl NoiseCancellation {
    pub fn from_setting(setting: AncSetting) -> Self {
        Self {
            is_enabled: setting.enabled,
            status: setting.mode.ui_status(),
            level: setting.level as u8,
        }
    }

    /// Boundary validation: only the source page values are accepted.
    pub fn to_setting(self) -> Result<AncSetting> {
        let mode = match self.status {
            10 => AncMode::Anc,
            255 => AncMode::Ambient,
            other => bail!("unknown persisted ANC status {other}"),
        };
        let level = match self.level {
            1 => AncLevel::Reduced,
            10 => AncLevel::Full,
            other => bail!("unknown persisted ANC level {other}"),
        };
        Ok(AncSetting {
            enabled: self.is_enabled,
            mode,
            level,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EarbudsProfile {
    pub guid: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_auto_pause_control: Option<AutoPauseControl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noise_cancellation: Option<NoiseCancellation>,
    /// Other profile fields owned elsewhere are preserved unchanged.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EarbudsProfileStore {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_guid: Option<String>,
    pub profiles: Vec<EarbudsProfile>,
}

const SCHEMA_VERSION: u32 = 1;

impl EarbudsProfileStore {
    /// MW default profile: `guid:""`, `name:"Default"`, auto-pause on and,
    /// for PID2657 only, `noiseCancellation{true,10,10}`.
    pub fn source_default(product_id: u32) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            active_guid: None,
            profiles: vec![EarbudsProfile {
                guid: String::new(),
                name: "Default".into(),
                audio_auto_pause_control: supports_auto_pause(product_id).then_some(
                    AutoPauseControl {
                        is_enabled: proto::AUTO_PAUSE_PROFILE_DEFAULT,
                    },
                ),
                noise_cancellation: supports_anc(product_id)
                    .then(|| NoiseCancellation::from_setting(proto::ANC_PROFILE_DEFAULT)),
                extra: Map::new(),
            }],
        }
    }

    pub fn select(&self) -> ProfileSelection {
        match self.active_guid.as_deref().filter(|guid| !guid.is_empty()) {
            None if self.profiles.is_empty() => ProfileSelection::Empty,
            None => ProfileSelection::Profile(0),
            Some(guid) => self
                .profiles
                .iter()
                .position(|profile| profile.guid == guid)
                .map_or_else(
                    || ProfileSelection::Unmatched(guid.to_owned()),
                    ProfileSelection::Profile,
                ),
        }
    }

    pub fn selected_mut(&mut self) -> Result<&mut EarbudsProfile> {
        match self.select() {
            ProfileSelection::Profile(index) => Ok(&mut self.profiles[index]),
            other => bail!("no selectable earbuds profile: {other:?}"),
        }
    }

    pub fn selected(&self) -> Result<&EarbudsProfile> {
        match self.select() {
            ProfileSelection::Profile(index) => Ok(&self.profiles[index]),
            other => bail!("no selectable earbuds profile: {other:?}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileSelection {
    Profile(usize),
    Empty,
    /// Open ambiguity: the active GUID names no stored profile.
    Unmatched(String),
}

impl EarbudsProfile {
    /// Restore input: a missing field stays `None` (the feature then uses
    /// its own source fallback).
    pub fn auto_pause_field(&self) -> proto::AutoPauseProfileField {
        proto::AutoPauseProfileField {
            is_enabled: self.audio_auto_pause_control.map(|field| field.is_enabled),
        }
    }

    /// `updateStorage`: a missing object starts as `{isEnabled:true}`.
    pub fn set_auto_pause(&mut self, enabled: bool) -> AutoPauseControl {
        let field = AutoPauseControl {
            is_enabled: enabled,
        };
        self.audio_auto_pause_control = Some(field);
        field
    }

    /// `updateStorage` for ANC: `level ?? 1`, other nullish fields merge
    /// with the stored value (or local default). Returns the stored value.
    pub fn set_noise_cancellation(&mut self, change: AncChange) -> Result<NoiseCancellation> {
        let base = match self.noise_cancellation {
            Some(stored) => stored.to_setting()?,
            None => proto::ANC_PROFILE_DEFAULT,
        };
        let merged = AncSetting {
            enabled: change.enabled.unwrap_or(base.enabled),
            mode: change.mode.unwrap_or(base.mode),
            level: change.level.unwrap_or(AncLevel::Reduced),
        };
        let stored = NoiseCancellation::from_setting(merged);
        self.noise_cancellation = Some(stored);
        Ok(stored)
    }
}

fn key(product_id: u32, serial: &str) -> Result<String> {
    if serial.is_empty() {
        bail!("earbuds profile storage requires a serial number");
    }
    Ok(format!("razer-ui.earbuds-profiles.{product_id}.{serial}"))
}

pub fn load(path: &Path, product_id: u32, serial: &str) -> Result<Option<EarbudsProfileStore>> {
    let Some(value) = crate::source_local::get(path, &key(product_id, serial)?)? else {
        return Ok(None);
    };
    let store: EarbudsProfileStore =
        serde_json::from_value(value).context("invalid earbuds profile store")?;
    if store.schema_version != SCHEMA_VERSION {
        bail!(
            "unsupported earbuds profile schema {}",
            store.schema_version
        );
    }
    Ok(Some(store))
}

pub fn save(path: &Path, product_id: u32, serial: &str, store: &EarbudsProfileStore) -> Result<()> {
    crate::source_local::set(
        path,
        &key(product_id, serial)?,
        &serde_json::to_value(store)?,
    )
}

/// `setupOnDeviceReady`: create the default only when storage is missing.
pub fn load_or_create(path: &Path, product_id: u32, serial: &str) -> Result<EarbudsProfileStore> {
    if let Some(store) = load(path, product_id, serial)? {
        return Ok(store);
    }
    let store = EarbudsProfileStore::source_default(product_id);
    save(path, product_id, serial, &store)?;
    Ok(store)
}

#[cfg(test)]
#[path = "earbuds_profiles_tests.rs"]
mod tests;

//! PID190 matcher document projection at the source/storage boundary. Applying
//! a matcher mutation cannot replace another profile or unrelated settings.
use crate::model::Device;
use anyhow::{Context as _, ensure};
use serde::Deserialize;
use serde_json::{Value, json};

fn present_field<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug)]
pub struct Mutation {
    device_identity: String,
    profile_id: String,
    profile_guid: String,
    previous_library: Option<Value>,
    library: Value,
    previous_selected: Option<Value>,
    selected: Option<Value>,
    previous_stages: Option<Value>,
    stages: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct SourceRecord {
    device_identity: String,
    profile_id: String,
    document: Value,
}

impl SourceRecord {
    pub fn document(&self) -> &Value {
        &self.document
    }
}

pub fn source_record(device: &Device) -> anyhow::Result<SourceRecord> {
    ensure!(
        device.product_id == razer_device::dpi_matcher::PRODUCT_ID,
        "Matcher source is not PID190"
    );
    let mut root = device
        .source_device_settings
        .clone()
        .unwrap_or_else(|| json!({}));
    let fields = root
        .as_object_mut()
        .context("Matcher device settings is not an object")?;
    let active = device
        .profiles
        .iter()
        .find(|profile| profile.id == device.active_profile)
        .context("Matcher active profile is missing")?;
    ensure!(
        !active.guid.is_empty(),
        "Matcher active profile has no source GUID"
    );
    let profiles = device
        .profiles
        .iter()
        .map(|profile| {
            let mut value = profile
                .source_settings
                .clone()
                .context("Matcher profile has no source settings")?;
            let fields = value
                .as_object_mut()
                .context("Matcher profile settings is not an object")?;
            ensure!(
                !profile.guid.is_empty(),
                "Matcher profile has no source GUID"
            );
            if let Some(guid) = fields.get("guid") {
                ensure!(
                    guid.as_str() == Some(&profile.guid),
                    "Matcher profile source GUID differs"
                );
            }
            fields.insert("guid".into(), json!(profile.guid));
            Ok(value)
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut guids = std::collections::BTreeSet::new();
    ensure!(
        device
            .profiles
            .iter()
            .all(|profile| guids.insert(&profile.guid)),
        "Matcher profile GUIDs are ambiguous"
    );
    fields.insert("activeProfile".into(), json!(active.guid));
    fields.insert("profiles".into(), json!(profiles));
    Ok(SourceRecord {
        device_identity: device.workspace_identity(),
        profile_id: active.id.clone(),
        document: root,
    })
}

impl Mutation {
    pub fn from_source(observed: &SourceRecord, after: &Value) -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Record {
            active_profile: String,
            profiles: Vec<Profile>,
            #[serde(default, deserialize_with = "present_field")]
            sensitivity_matcher: Option<Value>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Profile {
            guid: String,
            #[serde(default, deserialize_with = "present_field")]
            selected_sensitivity_matcher: Option<Value>,
            #[serde(default, deserialize_with = "present_field")]
            dpi_stages: Option<Value>,
        }
        let before: Record = serde_json::from_value(observed.document.clone())?;
        let after: Record = serde_json::from_value(after.clone())?;
        ensure!(
            before.active_profile == after.active_profile && !before.active_profile.is_empty(),
            "Matcher mutation changed its active profile"
        );
        let previous = before
            .profiles
            .iter()
            .find(|profile| profile.guid == before.active_profile)
            .context("Matcher previous active profile is missing")?;
        let next = after
            .profiles
            .iter()
            .find(|profile| profile.guid == after.active_profile)
            .context("Matcher next active profile is missing")?;
        let library = after
            .sensitivity_matcher
            .context("Matcher mutation has no library")?;
        ensure!(
            library.get("profiles").is_some_and(Value::is_array),
            "Matcher library has no profiles"
        );
        Ok(Self {
            device_identity: observed.device_identity.clone(),
            profile_id: observed.profile_id.clone(),
            profile_guid: before.active_profile,
            previous_library: before.sensitivity_matcher,
            library,
            previous_selected: previous.selected_sensitivity_matcher.clone(),
            selected: next.selected_sensitivity_matcher.clone(),
            previous_stages: previous.dpi_stages.clone(),
            stages: next.dpi_stages.clone(),
        })
    }

    pub fn apply(&self, device: &mut Device) -> anyhow::Result<()> {
        ensure!(
            device.product_id == razer_device::dpi_matcher::PRODUCT_ID,
            "Matcher save belongs to another product"
        );
        ensure!(
            device.workspace_identity() == self.device_identity,
            "Matcher save belongs to another device"
        );
        let mut candidate = device.clone();
        let profile = candidate
            .profiles
            .iter_mut()
            .find(|profile| profile.id == self.profile_id && profile.guid == self.profile_guid)
            .context("Matcher target profile was removed before save")?;
        let source = profile
            .source_settings
            .as_mut()
            .and_then(Value::as_object_mut)
            .context("Matcher saved profile settings is not an object")?;
        let root = candidate
            .source_device_settings
            .get_or_insert_with(|| json!({}))
            .as_object_mut()
            .context("Matcher saved device settings is not an object")?;
        ensure!(
            root.get("sensitivityMatcher") == self.previous_library.as_ref(),
            "Matcher library changed before save"
        );
        if self.previous_selected != self.selected {
            ensure!(
                source.get("selectedSensitivityMatcher") == self.previous_selected.as_ref(),
                "Matcher selection changed before save"
            );
            if let Some(selected) = &self.selected {
                ensure!(selected.is_string(), "Matcher selection is not a GUID");
                source.insert("selectedSensitivityMatcher".into(), selected.clone());
            } else {
                source.remove("selectedSensitivityMatcher");
            }
        }
        if self.previous_stages != self.stages {
            ensure!(
                source.get("dpiStages") == self.previous_stages.as_ref(),
                "Matcher DPI stages changed before save"
            );
            let stages = self.stages.as_ref().context("Matcher removed DPI stages")?;
            ensure!(stages.is_object(), "Matcher DPI stages is not an object");
            source.insert("dpiStages".into(), stages.clone());
        }
        let previous = root
            .get("version")
            .map(|value| {
                value
                    .as_u64()
                    .context("Matcher source version is not an unsigned integer")
            })
            .transpose()?;
        let version = razer_device::mapping_submission::next_source_version(previous)?;
        root.insert("sensitivityMatcher".into(), self.library.clone());
        root.insert("version".into(), json!(version));
        *device = candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

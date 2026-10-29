//! Audited mouse Matcher document projection at the source/storage boundary.
//! Applying a matcher mutation cannot replace another product, profile or
//! unrelated settings.
use crate::model::Device;
use anyhow::{Context as _, ensure};
use razer_device::dpi_matcher_storage::Change;
use serde::Deserialize;
use serde_json::{Value, json};

const LIBRARY_FIELD: &str = "sensitivityMatcher";
const SELECTED_FIELD: &str = "selectedSensitivityMatcher";
const STAGES_FIELD: &str = "dpiStages";
const VERSION_FIELD: &str = "version";

fn present_field<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

#[derive(Clone, Debug)]
pub struct Mutation {
    product_id: u32,
    device_identity: String,
    profile_id: String,
    profile_guid: String,
    fields: ChangedFields,
}

#[derive(Clone, Debug)]
enum ChangedFields {
    Stages {
        stages: FieldChange,
        selected: Option<FieldChange>,
    },
    Profiles {
        library: FieldChange,
        selected: FieldChange,
    },
}

#[derive(Clone, Debug)]
struct FieldChange {
    previous: Option<Value>,
    next: Option<Value>,
}

impl FieldChange {
    fn apply(&self, fields: &mut serde_json::Map<String, Value>, name: &str) -> anyhow::Result<()> {
        ensure!(
            fields.get(name) == self.previous.as_ref(),
            "Matcher {name} changed before save"
        );
        if let Some(next) = &self.next {
            fields.insert(name.into(), next.clone());
        } else {
            fields.remove(name);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SourceRecord {
    product_id: u32,
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
    let product = razer_device::dpi_matcher::Product::new(device.product_id)?;
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
        product_id: product.product_id(),
        device_identity: device.workspace_identity(),
        profile_id: active.id.clone(),
        document: root,
    })
}

impl Mutation {
    pub fn from_change(observed: &SourceRecord, change: &Change) -> anyhow::Result<Self> {
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
        ensure!(
            !before.active_profile.is_empty(),
            "Matcher active profile is empty"
        );
        let profile = before
            .profiles
            .iter()
            .find(|profile| profile.guid == before.active_profile)
            .context("Matcher active profile is missing")?;
        let fields = match change {
            Change::Stages(update) => {
                update.validate()?;
                let stages = update.stages().source_file_value();
                ChangedFields::Stages {
                    stages: FieldChange {
                        previous: profile.dpi_stages.clone(),
                        next: Some(stages),
                    },
                    selected: update.selected().map(|selected| FieldChange {
                        previous: profile.selected_sensitivity_matcher.clone(),
                        next: Some(json!(selected)),
                    }),
                }
            }
            Change::Profiles(update) => ChangedFields::Profiles {
                library: FieldChange {
                    previous: before.sensitivity_matcher,
                    next: Some(update.library().file_value()),
                },
                selected: FieldChange {
                    previous: profile.selected_sensitivity_matcher.clone(),
                    next: update.selected().map(|selected| json!(selected)),
                },
            },
        };
        Ok(Self {
            product_id: observed.product_id,
            device_identity: observed.device_identity.clone(),
            profile_id: observed.profile_id.clone(),
            profile_guid: before.active_profile,
            fields,
        })
    }

    pub fn is_profile_update(&self) -> bool {
        matches!(self.fields, ChangedFields::Profiles { .. })
    }

    pub fn apply(&self, device: &mut Device) -> anyhow::Result<()> {
        let product = razer_device::dpi_matcher::Product::new(device.product_id)?;
        ensure!(
            product.product_id() == self.product_id,
            "Matcher save belongs to another product"
        );
        ensure!(
            device.workspace_identity() == self.device_identity,
            "Matcher save belongs to another device"
        );
        ensure!(
            device.active_profile == self.profile_id,
            "Matcher active profile changed before save"
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
        if let Some(guid) = source.get("guid") {
            ensure!(
                guid.as_str() == Some(self.profile_guid.as_str()),
                "Matcher profile source GUID changed before save"
            );
        }
        let root = candidate
            .source_device_settings
            .get_or_insert_with(|| json!({}))
            .as_object_mut()
            .context("Matcher saved device settings is not an object")?;
        match &self.fields {
            ChangedFields::Stages { stages, selected } => {
                stages.apply(source, STAGES_FIELD)?;
                if let Some(selected) = selected {
                    selected.apply(source, SELECTED_FIELD)?;
                }
            }
            ChangedFields::Profiles { library, selected } => {
                library.apply(root, LIBRARY_FIELD)?;
                selected.apply(source, SELECTED_FIELD)?;
            }
        }
        let previous = root
            .get(VERSION_FIELD)
            .map(|value| {
                value
                    .as_u64()
                    .context("Matcher source version is not an unsigned integer")
            })
            .transpose()?;
        let version = razer_device::mapping_submission::next_source_version(previous)?;
        root.insert(VERSION_FIELD.into(), json!(version));
        *device = candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

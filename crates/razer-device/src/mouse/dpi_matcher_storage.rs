//! Typed live Matcher state and the two independent source storage tasks.
//! File projection follows JSON.stringify; live numbers survive until that boundary.
use crate::mouse_current_dpi::{SourceNumber, SourceStage, SourceStages};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet};

#[cfg(test)]
#[path = "dpi_matcher_storage_tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ProfileName {
    Text(String),
    Localized(Map<String, Value>),
}
impl ProfileName {
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Localized(_) => None,
        }
    }
    fn english(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Localized(values) => values.get("en").and_then(Value::as_str),
        }
    }
    /// Implements the current D_ helper used by the profile-name reducer.
    ///
    /// Falsy localized `en` values become an empty string. A truthy value that
    /// is not text is left as an explicit error because the source consumer
    /// would throw when it invokes its string operation.
    pub fn english_or_empty(&self) -> anyhow::Result<&str> {
        match self {
            Self::Text(text) => Ok(text),
            Self::Localized(values) => match values.get("en") {
                Some(value) if js_truthy(value) => value
                    .as_str()
                    .context("Matcher profile English name is not a string"),
                _ => Ok(""),
            },
        }
    }
    /// Implements the current KX helper's `en || Object.values(name)[0] || ""`.
    /// The first object value follows ECMAScript property ordering: canonical
    /// array-index keys are numeric ascending, then other keys retain source
    /// insertion order.
    pub fn english_or_first_property_or_empty(&self) -> anyhow::Result<&str> {
        match self {
            Self::Text(text) => Ok(text),
            Self::Localized(values) => match kx_value(values) {
                Some(value) => value
                    .as_str()
                    .context("Matcher profile KX name is not a string"),
                None => Ok(""),
            },
        }
    }
    /// `D_(name) !== "None"` is the source filter used by SELECT.
    pub fn is_english_none(&self) -> bool {
        self.english() == Some("None")
    }
    pub fn localized(&self, language: &str) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Localized(values) => values
                .get(language)
                .filter(|value| value.as_str().is_some_and(|value| !value.is_empty()))
                .or_else(|| values.get("en"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MatcherProfile {
    guid: String,
    name: ProfileName,
    #[serde(
        default,
        deserialize_with = "present_dpi",
        skip_serializing_if = "Option::is_none"
    )]
    dpi: Option<SourceStage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    disabled: Option<bool>,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}
fn present_dpi<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<SourceStage>, D::Error> {
    SourceStage::deserialize(deserializer).map(Some)
}
impl MatcherProfile {
    pub fn new(guid: String, name: ProfileName, dpi: Option<SourceStage>) -> Self {
        Self {
            guid,
            name,
            dpi,
            disabled: None,
            extensions: BTreeMap::new(),
        }
    }
    pub fn guid(&self) -> &str {
        &self.guid
    }
    pub fn name(&self) -> &ProfileName {
        &self.name
    }
    pub fn dpi(&self) -> Option<&SourceStage> {
        self.dpi.as_ref()
    }
    pub fn is_disabled(&self) -> bool {
        self.disabled == Some(true)
    }
    pub fn set_name(&mut self, name: String) {
        self.name = ProfileName::Text(name);
    }
    pub fn file_value(&self) -> Value {
        let mut value = serde_json::to_value(self).expect("typed Matcher profile serializes");
        if let Some(dpi) = &self.dpi {
            value["dpi"] = dpi.file_value();
        }
        value
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Library {
    profiles: Vec<MatcherProfile>,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}
impl Library {
    pub fn new(profiles: Vec<MatcherProfile>) -> Self {
        Self {
            profiles,
            extensions: BTreeMap::new(),
        }
    }
    pub fn profiles(&self) -> &[MatcherProfile] {
        &self.profiles
    }
    pub fn with_profiles(&self, profiles: Vec<MatcherProfile>) -> Self {
        Self {
            profiles,
            extensions: self.extensions.clone(),
        }
    }
    /// Apply the source K2 name projection while retaining profile order and
    /// every extension field. Only repeated truthy base keys receive a plain
    /// string suffix; the first occurrence keeps its original name object.
    pub fn with_deduplicated_names(&self) -> Self {
        let mut seen = HashSet::new();
        let mut profiles = Vec::with_capacity(self.profiles.len());
        for (profile_index, profile) in self.profiles.iter().enumerate() {
            let Some(key) = profile.name.k2_key(profile_index) else {
                profiles.push(profile.clone());
                continue;
            };
            if seen.insert(key.clone()) {
                profiles.push(profile.clone());
                continue;
            }

            let base_text = key.suffix_base();
            let mut suffix = 1usize;
            loop {
                let candidate = format!("{base_text} {suffix}");
                if seen.insert(NameKey::String(candidate.clone())) {
                    let mut duplicate = profile.clone();
                    duplicate.set_name(candidate);
                    profiles.push(duplicate);
                    break;
                }
                suffix += 1;
            }
        }
        self.with_profiles(profiles)
    }
    pub fn from_source_file(value: &Value) -> anyhow::Result<Self> {
        let library: Self = serde_json::from_value(value.clone())?;
        for profile in &library.profiles {
            if let Some(dpi) = &profile.dpi {
                SourceStage::from_source_file(&serde_json::to_value(dpi)?)?;
            }
        }
        Ok(library)
    }
    pub fn file_value(&self) -> Value {
        let mut value = serde_json::to_value(self).expect("typed Matcher library serializes");
        value["profiles"] = Value::Array(
            self.profiles
                .iter()
                .map(MatcherProfile::file_value)
                .collect(),
        );
        value
    }
}

impl ProfileName {
    fn k2_key(&self, profile_index: usize) -> Option<NameKey> {
        match self {
            Self::Text(text) if !text.is_empty() && text != "None" => {
                Some(NameKey::String(text.clone()))
            }
            Self::Text(_) => None,
            Self::Localized(values) => {
                kx_value(values).map(|value| NameKey::from_value(value, profile_index))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum NameKey {
    String(String),
    Number(u64),
    Bool(bool),
    /// JavaScript Set compares object keys by identity. A parsed profile name
    /// owns its object, so the profile index is the stable identity here.
    Object(usize),
}

impl NameKey {
    fn from_value(value: &Value, profile_index: usize) -> Self {
        match value {
            Value::String(value) => Self::String(value.clone()),
            Value::Number(value) => {
                let number = value
                    .as_f64()
                    .expect("serde_json number has an ECMAScript Number projection");
                let number = if number == 0. { 0. } else { number };
                Self::Number(number.to_bits())
            }
            Value::Bool(value) => Self::Bool(*value),
            Value::Null => Self::String(String::new()),
            Value::Array(_) | Value::Object(_) => Self::Object(profile_index),
        }
    }

    fn suffix_base(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Number(bits) => crate::source_string::number_text(f64::from_bits(*bits)),
            Self::Bool(value) => value.to_string(),
            Self::Object(_) => {
                unreachable!("separately owned profile objects have distinct Set keys")
            }
        }
    }
}

fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => {
            value
                .as_f64()
                .expect("serde_json number has an ECMAScript Number projection")
                != 0.
        }
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn kx_value(values: &Map<String, Value>) -> Option<&Value> {
    values
        .get("en")
        .filter(|value| js_truthy(value))
        .or_else(|| js_first_property_value(values))
        .filter(|value| js_truthy(value))
}

fn js_array_index(key: &str) -> Option<u32> {
    let value = key.parse::<u32>().ok()?;
    (value != u32::MAX && value.to_string() == key).then_some(value)
}

fn js_first_property_value(values: &Map<String, Value>) -> Option<&Value> {
    let indexed = values
        .iter()
        .filter_map(|(key, value)| Some((js_array_index(key)?, value)))
        .min_by_key(|(index, _)| *index)
        .map(|(_, value)| value);
    indexed.or_else(|| {
        values
            .iter()
            .find(|(key, _)| js_array_index(key).is_none())
            .map(|(_, value)| value)
    })
}

// `serde_json::Map` equality is map-like and may ignore insertion order. KX's
// first-property fallback observes that order, so retain_live compares names
// explicitly before retaining any live values.
fn ordered_profile_name_equal(left: &ProfileName, right: &ProfileName) -> bool {
    match (left, right) {
        (ProfileName::Text(left), ProfileName::Text(right)) => left == right,
        (ProfileName::Localized(left), ProfileName::Localized(right)) => {
            left.len() == right.len()
                && left.iter().zip(right).all(
                    |((left_key, left_value), (right_key, right_value))| {
                        left_key == right_key && left_value == right_value
                    },
                )
        }
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileUpdate {
    library: Library,
    selected: Option<String>,
}
impl ProfileUpdate {
    pub fn new(library: Library, selected: Option<String>) -> Self {
        Self { library, selected }
    }
    pub fn library(&self) -> &Library {
        &self.library
    }
    pub fn profiles(&self) -> &[MatcherProfile] {
        self.library.profiles()
    }
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }
    pub fn file_payload(&self) -> Value {
        let mut value = json!({"sensitivityMatcher":self.library.file_value()});
        if let Some(selected) = &self.selected {
            value["selectedSensitivityMatcher"] = json!(selected);
        }
        value
    }
    pub fn from_payload(payload: &Value) -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Payload {
            sensitivity_matcher: Value,
            selected_sensitivity_matcher: Option<String>,
        }
        let parsed: Payload = serde_json::from_value(payload.clone())?;
        Ok(Self::new(
            Library::from_source_file(&parsed.sensitivity_matcher)?,
            parsed.selected_sensitivity_matcher,
        ))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StageUpdate {
    stages: SourceStages,
    selected: Option<String>,
    preparation_error: Option<String>,
}
impl StageUpdate {
    pub fn new(stages: SourceStages, selected: Option<String>) -> Self {
        Self {
            stages: stages.with_visible_count(),
            selected,
            preparation_error: None,
        }
    }
    pub fn stages(&self) -> &SourceStages {
        &self.stages
    }
    /// None means that the stage task does not change the Matcher selection.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }
    pub fn validate(&self) -> anyhow::Result<()> {
        if let Some(error) = &self.preparation_error {
            anyhow::bail!("{error}");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Stages(StageUpdate),
    Profiles(ProfileUpdate),
}

#[derive(Clone, Debug)]
pub struct Mutation {
    stages: Option<StageUpdate>,
    response: ProfileUpdate,
}
impl Mutation {
    pub fn stages(&self) -> Option<&StageUpdate> {
        self.stages.as_ref()
    }
    pub fn response(&self) -> &ProfileUpdate {
        &self.response
    }
}

#[derive(Clone, Debug)]
pub struct SourceData {
    active_profile: String,
    library: Library,
    library_present: bool,
    selected: Option<String>,
    stages: Option<SourceStages>,
}
impl SourceData {
    pub fn from_document(data: &Value) -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Record {
            active_profile: String,
            profiles: Vec<Profile>,
            sensitivity_matcher: Option<Value>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Profile {
            guid: String,
            dpi_stages: Option<Value>,
            selected_sensitivity_matcher: Option<String>,
        }
        let mut record: Record = serde_json::from_value(data.clone())?;
        let index = record
            .profiles
            .iter()
            .position(|profile| profile.guid == record.active_profile)
            .context("active Matcher profile is missing")?;
        let active = record.profiles.remove(index);
        ensure!(
            !active.guid.is_empty(),
            "active Matcher profile GUID is empty"
        );
        let stages = active
            .dpi_stages
            .as_ref()
            .map(SourceStages::from_source_file)
            .transpose()?;
        Ok(Self {
            active_profile: active.guid,
            library_present: record.sensitivity_matcher.is_some(),
            library: record
                .sensitivity_matcher
                .as_ref()
                .map(Library::from_source_file)
                .transpose()?
                .unwrap_or_else(|| Library::new(Vec::new())),
            selected: active.selected_sensitivity_matcher,
            stages,
        })
    }
    pub fn library(&self) -> &Library {
        &self.library
    }
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }
    pub fn stages(&self) -> Option<&SourceStages> {
        self.stages.as_ref()
    }
    /// The source calibration baseline is a local profile value, not a hardware read.
    pub fn baseline(&self) -> anyhow::Result<f64> {
        let Some(stages) = &self.stages else {
            return Ok(100.);
        };
        let row = stages
            .active_stage()
            .checked_sub(1)
            .and_then(|ix| stages.stages().get(ix))
            .context("Matcher active DPI stage is missing")?;
        ensure!(!row.is_null(), "Matcher active DPI stage is null");
        Ok(row
            .x()
            .map(SourceNumber::value)
            .filter(|value| *value != 0. && !value.is_nan())
            .unwrap_or(100.))
    }
    /// A successful current read may retain live Number values whose file view
    /// is unchanged. Profile changes and independently changed fields replace them.
    pub fn retain_live(&mut self, previous: &Self) {
        if self.active_profile != previous.active_profile {
            return;
        }
        if self.library_present == previous.library_present
            && self.library.file_value() == previous.library.file_value()
            && self
                .library
                .profiles
                .iter()
                .zip(&previous.library.profiles)
                .all(|(current, previous)| {
                    ordered_profile_name_equal(current.name(), previous.name())
                })
        {
            self.library = previous.library.clone();
        }
        if self.stages.as_ref().map(SourceStages::source_file_value)
            == previous
                .stages
                .as_ref()
                .map(SourceStages::source_file_value)
        {
            self.stages = previous.stages.clone();
        }
    }
    pub fn apply(&mut self, change: &Change) {
        match change {
            Change::Stages(update) => {
                self.stages = Some(update.stages.clone());
                if let Some(selected) = &update.selected {
                    self.selected = Some(selected.clone());
                }
            }
            Change::Profiles(update) => {
                self.library = update.library.clone();
                self.library_present = true;
                self.selected = update.selected.clone();
            }
        }
    }
}

fn stage_update(
    source: &SourceData,
    dpi: &SourceStage,
    replace_null: bool,
) -> anyhow::Result<Option<StageUpdate>> {
    let Some(stages) = &source.stages else {
        return Ok(None);
    };
    let index = stages.active_stage().checked_sub(1).filter(|ix| {
        *ix < stages.stages().len() && (replace_null || !stages.stages()[*ix].is_null())
    });
    let stages = match index {
        Some(index) => stages.with_replaced_stage(index, dpi.clone())?,
        None => stages.clone(),
    };
    let selection = (|| -> anyhow::Result<Option<String>> {
        if !source.library_present || index.is_none() {
            return Ok(None);
        }
        for profile in &source.library.profiles {
            let row = profile
                .dpi
                .as_ref()
                .context("Matcher profile DPI is missing in the stage task")?;
            if row.strict_x_equals(dpi)? {
                return Ok(Some(profile.guid.clone()));
            }
        }
        Ok(Some(String::new()))
    })();
    let mut update = StageUpdate::new(stages, None);
    match selection {
        Ok(selected) => update.selected = selected,
        Err(error) => update.preparation_error = Some(format!("{error:#}")),
    }
    Ok(Some(update))
}

pub fn select(source: &SourceData, guid: &str) -> anyhow::Result<Mutation> {
    let mut library = source.library.clone();
    library
        .profiles
        .retain(|profile| profile.name.text() != Some("None"));
    let profile = library
        .profiles
        .iter()
        .find(|profile| profile.guid == guid)
        .context("selected Matcher profile is absent")?;
    let dpi = profile
        .dpi
        .as_ref()
        .context("selected Matcher has no DPI")?;
    // The UI X-stage reducer clones the SELECT payload through JSON.stringify.
    let dpi = SourceStage::from_source_file(&dpi.file_value())?;
    let stages = stage_update(source, &dpi, true)?
        .map(|update| -> anyhow::Result<StageUpdate> {
            let mut update = update;
            update.stages = SourceStages::from_source_file(&update.stages.source_file_value())?;
            Ok(update)
        })
        .transpose()?;
    Ok(Mutation {
        stages,
        response: ProfileUpdate::new(library, Some(guid.into())),
    })
}

pub fn confirm(
    source: &SourceData,
    dpi: f64,
    guid: &str,
    name: ProfileName,
) -> anyhow::Result<Mutation> {
    ensure!(!guid.is_empty(), "missing generated UUID");
    let mut library = source.library.clone();
    ensure!(
        !library.profiles.iter().any(|profile| profile.guid == guid),
        "UUID already exists"
    );
    let name = match name {
        ProfileName::Text(base) => ProfileName::Text(crate::source_string::unique_profile_name(
            &base,
            |candidate| {
                library
                    .profiles
                    .iter()
                    .any(|profile| profile.name.english() == Some(candidate))
            },
        )),
        ProfileName::Localized(mut values) => {
            let base = values
                .get("en")
                .and_then(Value::as_str)
                .context("Matcher confirmation name omits its English text")?;
            let chinese = values
                .get("zh-cn")
                .and_then(Value::as_str)
                .context("Matcher confirmation name omits its Chinese text")?;
            let unique = crate::source_string::unique_profile_name(base, |candidate| {
                library
                    .profiles
                    .iter()
                    .any(|profile| profile.name.english() == Some(candidate))
            });
            let suffix = unique
                .strip_prefix(base)
                .expect("source unique name retains the original prefix");
            let chinese = format!("{chinese}{suffix}");
            values.insert("en".into(), json!(unique));
            values.insert("zh-cn".into(), json!(chinese));
            ProfileName::Localized(values)
        }
    };
    let dpi = SourceStage::new(
        Some(SourceNumber::new(dpi)),
        Some(SourceNumber::new(dpi)),
        Some(true),
        Some(false),
    );
    let stages = stage_update(source, &dpi, false)?;
    library
        .profiles
        .push(MatcherProfile::new(guid.into(), name, Some(dpi)));
    Ok(Mutation {
        stages,
        response: ProfileUpdate::new(library, Some(guid.into())),
    })
}

pub fn delete(source: &SourceData, guid: &str) -> anyhow::Result<Mutation> {
    let mut library = source.library.clone();
    library.profiles.retain(|profile| profile.guid != guid);
    let first = library.profiles.first();
    let selected = first.map(|profile| profile.guid.clone());
    let stages = if source.selected() == Some(guid) {
        first
            .and_then(|profile| profile.dpi.as_ref())
            .map(|dpi| stage_update(source, dpi, true))
            .transpose()?
            .flatten()
    } else {
        None
    };
    Ok(Mutation {
        stages,
        response: ProfileUpdate::new(library, selected),
    })
}
pub fn delete_all(_source: &SourceData) -> Mutation {
    Mutation {
        stages: None,
        response: ProfileUpdate::new(Library::new(Vec::new()), Some(String::new())),
    }
}
pub fn update(_source: &SourceData, update: &ProfileUpdate) -> Mutation {
    Mutation {
        stages: None,
        response: update.clone(),
    }
}

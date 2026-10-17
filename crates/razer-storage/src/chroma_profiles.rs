//! Local Chroma profile selection and the current Studio's shared projection.
//! Profile names/IDs are independent of the Studio effect/layer library. Reading
//! its source projection never confirms a native installation or a device write.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const VERSION: u32 = 1;
const SOURCE_CURRENT_USER: &str = "currentUserId";
const SOURCE_USERS: &str = "synapseChromaStudioUsers";
const SOURCE_DYNAMIC_LIGHTING: &str = "isDynamicLighting";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    id: String,
    name: String,
    // Studio module 1415/Tf preserves fields other than devices/layers/title.
    #[serde(flatten)]
    source_fields: Map<String, Value>,
}
impl Profile {
    pub fn new(id: String, name: String) -> anyhow::Result<Self> {
        let profile = Self { id, name, source_fields: Map::new() };
        profile.validate()?;
        Ok(profile)
    }
    pub fn id(&self) -> &str { &self.id }
    pub fn name(&self) -> &str { &self.name }
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(!self.id.is_empty(), "Chroma profile has an empty ID");
        ensure!(!self.name.trim().is_empty(), "Chroma profile has an empty name");
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    profiles: Vec<Profile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    current_profile: Option<String>,
    enabled: bool,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}
impl Configuration {
    fn empty_local() -> Self {
        Self { profiles: Vec::new(), current_profile: None, enabled: false, extensions:Map::new() }
    }
    pub fn profiles(&self) -> &[Profile] { &self.profiles }
    pub fn selected_profile(&self) -> Option<&str> { self.current_profile.as_deref() }
    pub fn enabled(&self) -> bool { self.enabled }
    fn validate(&self) -> anyhow::Result<()> {
        let mut ids = BTreeSet::new();
        for profile in &self.profiles {
            profile.validate()?;
            ensure!(ids.insert(profile.id()), "Duplicate Chroma profile ID");
        }
        ensure!(
            self.current_profile.as_ref().is_none_or(|selected| ids.contains(selected.as_str())),
            "Selected Chroma profile is absent from the profile list"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin { LocalConfiguration, ImportedProjection }

#[derive(Clone, Debug)]
pub struct Snapshot {
    configuration: Option<Configuration>,
    origin: Option<Origin>,
    dynamic_lighting: Option<bool>,
    module_installed: Option<bool>,
}
impl Snapshot {
    pub fn configuration(&self) -> Option<&Configuration> { self.configuration.as_ref() }
    pub fn origin(&self) -> Option<Origin> { self.origin }
    pub fn dynamic_lighting(&self) -> Option<bool> { self.dynamic_lighting }
    pub fn module_installed(&self) -> Option<bool> { self.module_installed }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Document {
    version: u32,
    revision: u64,
    configuration: Configuration,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceUser {
    user_id: String,
    profiles: Vec<Profile>,
    #[serde(default)]
    current_profile: Option<String>,
    enabled: bool,
}

pub fn path() -> PathBuf { crate::store_path().with_file_name("chroma-profiles.json") }

fn decode(text: &str) -> anyhow::Result<Document> {
    let document: Document = serde_json::from_str(text)?;
    ensure!(document.version == VERSION, "Unsupported local Chroma configuration version {}", document.version);
    document.configuration.validate()?;
    Ok(document)
}

pub fn load(path: &Path, source_path: &Path) -> anyhow::Result<Snapshot> {
    // Imported localStorage strings stay at the explicit external file boundary.
    let entries: std::collections::BTreeMap<String, String> =
        crate::read_document(source_path, |text| Ok(serde_json::from_str(text)?))?.unwrap_or_default();
    let dynamic_lighting = entries.get(SOURCE_DYNAMIC_LIGHTING)
        .map(|raw| serde_json::from_str::<bool>(raw)).transpose()?;
    let module_installed = if let Some(raw) = entries.get("installedModules") {
        let installed: std::collections::BTreeMap<String, Value> = serde_json::from_str(raw)?;
        let removing: Vec<String> = entries.get("uninstallingModules")
            .map(|raw| serde_json::from_str(raw)).transpose()?.unwrap_or_default();
        Some(installed.contains_key("chroma-app") && !removing.iter().any(|name| name == "chroma-app"))
    } else { None };
    if let Some(document) = crate::read_document(path, decode)? {
        return Ok(Snapshot { configuration: Some(document.configuration), origin: Some(Origin::LocalConfiguration), dynamic_lighting, module_installed });
    }
    let configuration = match (entries.get(SOURCE_CURRENT_USER), entries.get(SOURCE_USERS)) {
        (Some(current), Some(users)) => {
            let users: Vec<SourceUser> = serde_json::from_str(users)?;
            users.into_iter().find(|user| user.user_id == *current).map(|user| Configuration {
                profiles: user.profiles,
                current_profile: user.current_profile,
                enabled: user.enabled,
                extensions:Map::new(),
            })
        }
        _ => None,
    };
    if let Some(configuration) = &configuration { configuration.validate()?; }
    let origin = configuration.as_ref().map(|_| Origin::ImportedProjection);
    Ok(Snapshot { configuration, origin, dynamic_lighting, module_installed })
}

fn lock(path: &Path, guard: &AtomicBool) -> anyhow::Result<File> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let lock = OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .open(path.with_extension("json.lock"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        ensure!(guard.load(Ordering::Acquire), "Chroma configuration request was canceled");
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(TryLockError::Error(error)) => return Err(error.into()),
            Err(TryLockError::WouldBlock) => {
                ensure!(Instant::now() < deadline, "Chroma configuration lock timed out");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

pub enum Change {
    Toggle(bool),
    SelectProfile(String),
    /// Studio publishes its actual complete shared profile projection, rather
    /// than converting the separate editor draft into an installed profile.
    PublishProfiles { profiles: Vec<Profile>, selected: Option<String> },
}

pub fn update(
    path: &Path,
    source_path: &Path,
    change: Change,
    guard: &AtomicBool,
) -> anyhow::Result<Snapshot> {
    let _lock = lock(path, guard)?;
    let previous = crate::read_document(path, decode)?;
    let mut configuration = load(path, source_path)?.configuration.unwrap_or_else(Configuration::empty_local);
    match change {
        Change::Toggle(enabled) => configuration.enabled = enabled,
        Change::SelectProfile(id) => {
            ensure!(configuration.profiles.iter().any(|profile| profile.id() == id), "Requested Chroma profile does not exist");
            configuration.current_profile = Some(id);
            configuration.enabled = true;
        }
        Change::PublishProfiles { profiles, selected } => {
            configuration.profiles = profiles;
            configuration.current_profile = selected;
        }
    }
    configuration.validate()?;
    let revision = previous.as_ref().map_or(0, |document| document.revision)
        .checked_add(1).context("Chroma configuration revision exhausted")?;
    let document = Document { version: VERSION, revision, configuration, extensions: previous.as_ref().map(|document| document.extensions.clone()).unwrap_or_default() };
    ensure!(guard.load(Ordering::Acquire), "Chroma configuration request was canceled");
    crate::write_document(path, &document, |text| {
        ensure!(guard.load(Ordering::Acquire), "Chroma configuration request was canceled");
        ensure!(previous.as_ref() == Some(&decode(text)?), "Chroma configuration changed after it was read");
        Ok(())
    })?;
    load(path, source_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("razer-chroma-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos())))
        }
        fn local(&self) -> PathBuf { self.0.join("chroma.json") }
        fn source(&self) -> PathBuf { self.0.join("source.json") }
    }
    impl Drop for Temp { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    #[test]
    fn missing_external_record_stays_unobserved_until_a_local_mutation() {
        let temp = Temp::new();
        assert!(load(&temp.local(), &temp.source()).unwrap().configuration().is_none());
        let snapshot = update(&temp.local(), &temp.source(), Change::Toggle(true), &AtomicBool::new(true)).unwrap();
        assert_eq!(snapshot.origin(), Some(Origin::LocalConfiguration));
        assert!(snapshot.configuration().unwrap().enabled());
        assert!(!temp.source().exists());
    }
    #[test]
    fn imported_projection_preserves_profile_fields_but_does_not_retain_account_identity() {
        let temp = Temp::new();
        crate::source_local::set_raw(&temp.source(), SOURCE_CURRENT_USER, "source-user").unwrap();
        crate::source_local::set(&temp.source(), SOURCE_USERS, &serde_json::json!([
            {"userId":"source-user","enabled":false,"currentProfile":"a","profiles":[{"id":"a","name":"First","linkedGames":["local.exe"]},{"id":"b","name":"Second"}]}
        ])).unwrap();
        let imported = load(&temp.local(), &temp.source()).unwrap();
        assert_eq!(imported.origin(), Some(Origin::ImportedProjection));
        let before = std::fs::read(temp.source()).unwrap();
        let changed = update(&temp.local(), &temp.source(), Change::SelectProfile("b".into()), &AtomicBool::new(true)).unwrap();
        let config = changed.configuration().unwrap();
        assert_eq!(config.selected_profile(), Some("b"));
        assert!(config.enabled());
        assert_eq!(config.profiles[0].source_fields.get("linkedGames"), Some(&serde_json::json!(["local.exe"])));
        assert!(!std::fs::read_to_string(temp.local()).unwrap().contains("source-user"));
        assert_eq!(std::fs::read(temp.source()).unwrap(), before);
    }
    #[test]
    fn canceled_and_invalid_selection_do_not_replace_local_configuration() {
        let temp = Temp::new();
        update(&temp.local(), &temp.source(), Change::Toggle(false), &AtomicBool::new(true)).unwrap();
        let before = std::fs::read(temp.local()).unwrap();
        assert!(update(&temp.local(), &temp.source(), Change::Toggle(true), &AtomicBool::new(false)).is_err());
        assert!(update(&temp.local(), &temp.source(), Change::SelectProfile("missing".into()), &AtomicBool::new(true)).is_err());
        assert_eq!(std::fs::read(temp.local()).unwrap(), before);
    }
}

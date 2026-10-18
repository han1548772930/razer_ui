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
const STUDIO_LIBRARY: &str = "studio";

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
        let profile = Self {
            id,
            name,
            source_fields: Map::new(),
        };
        profile.validate()?;
        Ok(profile)
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    fn validate(&self) -> anyhow::Result<()> {
        ensure!(!self.id.is_empty(), "Chroma profile has an empty ID");
        ensure!(
            !self.name.trim().is_empty(),
            "Chroma profile has an empty name"
        );
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
        Self {
            profiles: Vec::new(),
            current_profile: None,
            enabled: false,
            extensions: Map::new(),
        }
    }
    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }
    pub fn selected_profile(&self) -> Option<&str> {
        self.current_profile.as_deref()
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    fn validate(&self) -> anyhow::Result<()> {
        let mut ids = BTreeSet::new();
        for profile in &self.profiles {
            profile.validate()?;
            ensure!(ids.insert(profile.id()), "Duplicate Chroma profile ID");
        }
        ensure!(
            self.current_profile
                .as_ref()
                .is_none_or(|selected| ids.contains(selected.as_str())),
            "Selected Chroma profile is absent from the profile list"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    LocalConfiguration,
    ImportedProjection,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    configuration: Option<Configuration>,
    origin: Option<Origin>,
    dynamic_lighting: Option<bool>,
    module_installed: Option<bool>,
}
impl Snapshot {
    pub fn configuration(&self) -> Option<&Configuration> {
        self.configuration.as_ref()
    }
    pub fn origin(&self) -> Option<Origin> {
        self.origin
    }
    pub fn dynamic_lighting(&self) -> Option<bool> {
        self.dynamic_lighting
    }
    pub fn module_installed(&self) -> Option<bool> {
        self.module_installed
    }
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

pub fn path() -> PathBuf {
    crate::store_path().with_file_name("chroma-profiles.json")
}

fn decode(text: &str) -> anyhow::Result<Document> {
    let document: Document = serde_json::from_str(text)?;
    ensure!(
        document.version == VERSION,
        "Unsupported local Chroma configuration version {}",
        document.version
    );
    document.configuration.validate()?;
    Ok(document)
}

fn decode_dynamic_lighting(raw: Option<&str>) -> anyhow::Result<Option<bool>> {
    raw.map(serde_json::from_str::<bool>)
        .transpose()
        .map_err(Into::into)
}

/// Reads the mounted device warning's local preference without loading Studio
/// configuration or inspecting its native installation.
pub fn load_dynamic_lighting(source_path: &Path) -> anyhow::Result<Option<bool>> {
    let raw = crate::source_local::get_raw(source_path, SOURCE_DYNAMIC_LIGHTING)?;
    decode_dynamic_lighting(raw.as_deref())
}

pub fn load(path: &Path, source_path: &Path) -> anyhow::Result<Snapshot> {
    // Imported localStorage strings stay at the explicit external file boundary.
    let entries: std::collections::BTreeMap<String, String> =
        crate::read_document(source_path, |text| Ok(serde_json::from_str(text)?))?
            .unwrap_or_default();
    let dynamic_lighting =
        decode_dynamic_lighting(entries.get(SOURCE_DYNAMIC_LIGHTING).map(String::as_str))?;
    let module_installed = if let Some(raw) = entries.get("installedModules") {
        let installed: std::collections::BTreeMap<String, Value> = serde_json::from_str(raw)?;
        let removing: Vec<String> = entries
            .get("uninstallingModules")
            .map(|raw| serde_json::from_str(raw))
            .transpose()?
            .unwrap_or_default();
        Some(
            installed.contains_key("chroma-app")
                && !removing.iter().any(|name| name == "chroma-app"),
        )
    } else {
        None
    };
    if let Some(document) = crate::read_document(path, decode)? {
        return Ok(Snapshot {
            configuration: Some(document.configuration),
            origin: Some(Origin::LocalConfiguration),
            dynamic_lighting,
            module_installed,
        });
    }
    let configuration = match (entries.get(SOURCE_CURRENT_USER), entries.get(SOURCE_USERS)) {
        (Some(current), Some(users)) => {
            let users: Vec<SourceUser> = serde_json::from_str(users)?;
            users
                .into_iter()
                .find(|user| user.user_id == *current)
                .map(|user| Configuration {
                    profiles: user.profiles,
                    current_profile: user.current_profile,
                    enabled: user.enabled,
                    extensions: Map::new(),
                })
        }
        _ => None,
    };
    if let Some(configuration) = &configuration {
        configuration.validate()?;
    }
    let origin = configuration.as_ref().map(|_| Origin::ImportedProjection);
    Ok(Snapshot {
        configuration,
        origin,
        dynamic_lighting,
        module_installed,
    })
}

fn lock(path: &Path, guard: &AtomicBool) -> anyhow::Result<File> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("json.lock"))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        ensure!(
            guard.load(Ordering::Acquire),
            "Chroma configuration request was canceled"
        );
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(TryLockError::Error(error)) => return Err(error.into()),
            Err(TryLockError::WouldBlock) => {
                ensure!(
                    Instant::now() < deadline,
                    "Chroma configuration lock timed out"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

pub enum Change {
    Toggle(bool),
    SelectProfile(String),
}

/// Actual local Studio profile contents. T is the editor's typed document,
/// validated by its owner; shared ID/name projection never substitutes for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StudioProfile<T> {
    id: String,
    name: String,
    editor: T,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    is_default: bool,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}
impl<T> StudioProfile<T> {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn editor(&self) -> &T {
        &self.editor
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StudioLibrary<T> {
    profiles: Vec<StudioProfile<T>>,
    selected: Option<String>,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}
impl<T> Default for StudioLibrary<T> {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            selected: None,
            extensions: Map::new(),
        }
    }
}
impl<T> StudioLibrary<T> {
    pub fn profiles(&self) -> &[StudioProfile<T>] {
        &self.profiles
    }
    pub fn selected_id(&self) -> Option<&str> {
        self.selected.as_deref()
    }
    pub fn selected(&self) -> Option<&StudioProfile<T>> {
        self.selected
            .as_ref()
            .and_then(|id| self.profiles.iter().find(|profile| profile.id == *id))
    }
    fn validate(&self, validate: &impl Fn(&T) -> anyhow::Result<()>) -> anyhow::Result<()> {
        let mut ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        for profile in &self.profiles {
            ensure!(
                !profile.id.is_empty() && ids.insert(profile.id.as_str()),
                "Studio profile identity is invalid or repeated"
            );
            ensure!(
                !profile.name.trim().is_empty()
                    && profile.name.encode_utf16().count() <= 32
                    && names.insert(profile.name.as_str()),
                "Studio profile name is invalid or repeated"
            );
            validate(&profile.editor)?;
        }
        ensure!(
            self.selected
                .as_ref()
                .is_none_or(|id| ids.contains(id.as_str())),
            "Studio selected profile is missing"
        );
        Ok(())
    }
}
impl<T: Clone> StudioLibrary<T> {
    /// Computes the source's optimistic edit without publishing it to disk.
    /// The persistent writer uses this same transition after reading its lock.
    pub fn changed(
        &self,
        change: StudioChange<T>,
        validate: impl Fn(&T) -> anyhow::Result<()>,
    ) -> anyhow::Result<Self> {
        let mut library = self.clone();
        match change {
            StudioChange::Add { id, name, editor } => {
                ensure!(
                    !library.profiles.iter().any(|profile| profile.id == id),
                    "New Studio identity already exists"
                );
                let first = library.profiles.is_empty();
                library.profiles.push(StudioProfile {
                    id: id.clone(),
                    name,
                    editor,
                    is_default: first,
                    extensions: Map::new(),
                });
                library.selected = Some(id);
            }
            StudioChange::Rename { id, name } => {
                let profile = library
                    .profiles
                    .iter_mut()
                    .find(|profile| profile.id == id)
                    .context("Renamed Studio profile is missing")?;
                if profile.name != name {
                    profile.name = name;
                    profile.is_default = false;
                }
            }
            StudioChange::Duplicate { id, new_id, name } => {
                ensure!(
                    !library.profiles.iter().any(|profile| profile.id == new_id),
                    "Duplicated Studio identity already exists"
                );
                let mut profile = library
                    .profiles
                    .iter()
                    .find(|profile| profile.id == id)
                    .context("Duplicated Studio profile is missing")?
                    .clone();
                profile.id = new_id.clone();
                profile.name = name;
                profile.is_default = false;
                library.profiles.push(profile);
                library.selected = Some(new_id);
            }
            StudioChange::Delete { id } => {
                ensure!(
                    library.profiles.len() > 1,
                    "Studio retains its last profile"
                );
                let position = library
                    .profiles
                    .iter()
                    .position(|profile| profile.id == id)
                    .context("Deleted Studio profile is missing")?;
                library.profiles.remove(position);
                if library.selected.as_deref() == Some(&id) {
                    library.selected = library
                        .profiles
                        .get(position)
                        .or_else(|| library.profiles.get(position.saturating_sub(1)))
                        .map(|profile| profile.id.clone());
                }
            }
            StudioChange::Select { id } => {
                ensure!(
                    library.profiles.iter().any(|profile| profile.id == id),
                    "Selected Studio profile has no editor document"
                );
                library.selected = Some(id);
            }
            StudioChange::Save { id, editor } => {
                let profile = library
                    .profiles
                    .iter_mut()
                    .find(|profile| profile.id == id)
                    .context("Saved Studio profile is missing")?;
                profile.editor = editor;
                profile.is_default = false;
            }
        }
        library.validate(&validate)?;
        Ok(library)
    }
}
#[derive(Clone)]
pub enum StudioChange<T> {
    Add {
        id: String,
        name: String,
        editor: T,
    },
    Rename {
        id: String,
        name: String,
    },
    Duplicate {
        id: String,
        new_id: String,
        name: String,
    },
    Delete {
        id: String,
    },
    Select {
        id: String,
    },
    Save {
        id: String,
        editor: T,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyStudioDraft<T> {
    version: u32,
    studio: T,
}

fn load_legacy_studio_draft_at<T: serde::de::DeserializeOwned>(
    path: &Path,
    validate: impl Fn(&T) -> anyhow::Result<()>,
) -> anyhow::Result<Option<T>> {
    crate::read_document(path, |text| {
        let draft: LegacyStudioDraft<T> = serde_json::from_str(text)?;
        ensure!(
            draft.version == 1,
            "Unsupported Studio draft version {}",
            draft.version
        );
        validate(&draft.studio)?;
        Ok(draft.studio)
    })
}

/// Background-only migration read. The original draft remains a recovery copy;
/// a selected local library profile always takes precedence at the caller.
pub fn load_legacy_studio_draft<T: serde::de::DeserializeOwned>(
    validate: impl Fn(&T) -> anyhow::Result<()>,
) -> anyhow::Result<Option<T>> {
    load_legacy_studio_draft_at(
        &crate::store_path().with_file_name("chroma-studio-draft.json"),
        validate,
    )
}

pub fn load_studio<T: serde::de::DeserializeOwned>(
    path: &Path,
    validate: impl Fn(&T) -> anyhow::Result<()>,
) -> anyhow::Result<StudioLibrary<T>> {
    let Some(document) = crate::read_document(path, decode)? else {
        return Ok(StudioLibrary::default());
    };
    let Some(value) = document.extensions.get(STUDIO_LIBRARY) else {
        return Ok(StudioLibrary::default());
    };
    let mut library: StudioLibrary<T> = serde_json::from_value(value.clone())?;
    // Device pages and Studio share selection. A projection without a complete
    // editor document remains explicit and cannot hydrate an empty profile.
    library.selected = document.configuration.current_profile;
    library.validate(&validate)?;
    Ok(library)
}
pub fn update_studio<T: Clone + Serialize + serde::de::DeserializeOwned>(
    path: &Path,
    source_path: &Path,
    change: StudioChange<T>,
    guard: &AtomicBool,
    validate: impl Fn(&T) -> anyhow::Result<()>,
) -> anyhow::Result<StudioLibrary<T>> {
    let _lock = lock(path, guard)?;
    let previous = crate::read_document(path, decode)?;
    let mut configuration = load(path, source_path)?
        .configuration
        .unwrap_or_else(Configuration::empty_local);
    let mut library: StudioLibrary<T> = previous
        .as_ref()
        .and_then(|document| document.extensions.get(STUDIO_LIBRARY))
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()?
        .unwrap_or_default();
    // A selection made by a product page is authoritative even when Studio's
    // stored library predates it. A rename must not restore an older selection.
    library.selected = configuration.current_profile.clone();
    match &change {
        StudioChange::Add { id, .. } | StudioChange::Duplicate { new_id: id, .. } => ensure!(
            !configuration
                .profiles
                .iter()
                .any(|profile| profile.id == *id),
            "New Studio identity already exists"
        ),
        StudioChange::Delete { id } => configuration.profiles.retain(|profile| profile.id != *id),
        _ => {}
    }
    if !matches!(&change, StudioChange::Rename { .. }) {
        configuration.enabled = true;
    }
    library = library.changed(change, &validate)?;
    for profile in &library.profiles {
        let mut fields = configuration
            .profiles
            .iter()
            .find(|row| row.id == profile.id)
            .map(|row| row.source_fields.clone())
            .unwrap_or_default();
        if profile.is_default {
            fields.insert("default".into(), true.into());
        } else {
            fields.remove("default");
        }
        let projection = Profile {
            id: profile.id.clone(),
            name: profile.name.clone(),
            source_fields: fields,
        };
        if let Some(existing) = configuration
            .profiles
            .iter_mut()
            .find(|row| row.id == profile.id)
        {
            *existing = projection;
        } else {
            configuration.profiles.push(projection);
        }
    }
    configuration.current_profile = library.selected.clone();
    configuration.validate()?;
    let mut extensions = previous
        .as_ref()
        .map(|document| document.extensions.clone())
        .unwrap_or_default();
    extensions.insert(STUDIO_LIBRARY.into(), serde_json::to_value(&library)?);
    let document = Document {
        version: VERSION,
        revision: previous
            .as_ref()
            .map_or(0, |document| document.revision)
            .checked_add(1)
            .context("Chroma revision exhausted")?,
        configuration,
        extensions,
    };
    ensure!(guard.load(Ordering::Acquire), "Studio request was canceled");
    crate::write_document(path, &document, |text| {
        ensure!(guard.load(Ordering::Acquire), "Studio request was canceled");
        ensure!(
            previous.as_ref() == Some(&decode(text)?),
            "Studio configuration changed after it was read"
        );
        Ok(())
    })?;
    Ok(library)
}

pub fn update(
    path: &Path,
    source_path: &Path,
    change: Change,
    guard: &AtomicBool,
) -> anyhow::Result<Snapshot> {
    let _lock = lock(path, guard)?;
    let previous = crate::read_document(path, decode)?;
    let mut configuration = load(path, source_path)?
        .configuration
        .unwrap_or_else(Configuration::empty_local);
    match change {
        Change::Toggle(enabled) => configuration.enabled = enabled,
        Change::SelectProfile(id) => {
            ensure!(
                configuration
                    .profiles
                    .iter()
                    .any(|profile| profile.id() == id),
                "Requested Chroma profile does not exist"
            );
            configuration.current_profile = Some(id);
            configuration.enabled = true;
        }
    }
    configuration.validate()?;
    let revision = previous
        .as_ref()
        .map_or(0, |document| document.revision)
        .checked_add(1)
        .context("Chroma configuration revision exhausted")?;
    let document = Document {
        version: VERSION,
        revision,
        configuration,
        extensions: previous
            .as_ref()
            .map(|document| document.extensions.clone())
            .unwrap_or_default(),
    };
    ensure!(
        guard.load(Ordering::Acquire),
        "Chroma configuration request was canceled"
    );
    crate::write_document(path, &document, |text| {
        ensure!(
            guard.load(Ordering::Acquire),
            "Chroma configuration request was canceled"
        );
        ensure!(
            previous.as_ref() == Some(&decode(text)?),
            "Chroma configuration changed after it was read"
        );
        Ok(())
    })?;
    load(path, source_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Editor {
        layers: Vec<u32>,
        #[serde(flatten)]
        extensions: Map<String, Value>,
    }
    fn editor(layer: u32) -> Editor {
        Editor {
            layers: vec![layer],
            extensions: Map::new(),
        }
    }
    fn validate_editor(editor: &Editor) -> anyhow::Result<()> {
        ensure!(
            editor.layers.iter().all(|layer| *layer > 0),
            "Invalid test editor layer"
        );
        Ok(())
    }
    fn studio(temp: &Temp, change: StudioChange<Editor>) -> StudioLibrary<Editor> {
        update_studio(
            &temp.local(),
            &temp.source(),
            change,
            &AtomicBool::new(true),
            validate_editor,
        )
        .unwrap()
    }
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                    "razer-chroma-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                )))
        }
        fn local(&self) -> PathBuf {
            self.0.join("chroma.json")
        }
        fn source(&self) -> PathBuf {
            self.0.join("source.json")
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn missing_external_record_stays_unobserved_until_a_local_mutation() {
        let temp = Temp::new();
        assert!(
            load(&temp.local(), &temp.source())
                .unwrap()
                .configuration()
                .is_none()
        );
        let snapshot = update(
            &temp.local(),
            &temp.source(),
            Change::Toggle(true),
            &AtomicBool::new(true),
        )
        .unwrap();
        assert_eq!(snapshot.origin(), Some(Origin::LocalConfiguration));
        assert!(snapshot.configuration().unwrap().enabled());
        assert!(!temp.source().exists());
    }
    #[test]
    fn dynamic_lighting_read_is_independent_and_preserves_missing_and_invalid_values() {
        let temp = Temp::new();
        assert_eq!(load_dynamic_lighting(&temp.source()).unwrap(), None);
        assert!(!temp.source().exists());
        crate::source_local::set_raw(&temp.source(), "installedModules", "invalid").unwrap();
        crate::source_local::set_raw(&temp.source(), SOURCE_USERS, "invalid").unwrap();
        crate::source_local::set_raw(&temp.source(), SOURCE_DYNAMIC_LIGHTING, "true").unwrap();
        let before = std::fs::read(temp.source()).unwrap();
        assert_eq!(load_dynamic_lighting(&temp.source()).unwrap(), Some(true));
        assert_eq!(std::fs::read(temp.source()).unwrap(), before);
        assert!(!temp.local().exists());
        assert!(load(&temp.local(), &temp.source()).is_err());

        crate::source_local::set_raw(&temp.source(), SOURCE_DYNAMIC_LIGHTING, "false").unwrap();
        assert_eq!(load_dynamic_lighting(&temp.source()).unwrap(), Some(false));
        for invalid in ["null", "0", "\"false\"", "invalid"] {
            crate::source_local::set_raw(&temp.source(), SOURCE_DYNAMIC_LIGHTING, invalid).unwrap();
            assert!(load_dynamic_lighting(&temp.source()).is_err(), "{invalid}");
        }
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
        let changed = update(
            &temp.local(),
            &temp.source(),
            Change::SelectProfile("b".into()),
            &AtomicBool::new(true),
        )
        .unwrap();
        let config = changed.configuration().unwrap();
        assert_eq!(config.selected_profile(), Some("b"));
        assert!(config.enabled());
        assert_eq!(
            config.profiles[0].source_fields.get("linkedGames"),
            Some(&serde_json::json!(["local.exe"]))
        );
        assert!(
            !std::fs::read_to_string(temp.local())
                .unwrap()
                .contains("source-user")
        );
        assert_eq!(std::fs::read(temp.source()).unwrap(), before);
    }
    #[test]
    fn canceled_and_invalid_selection_do_not_replace_local_configuration() {
        let temp = Temp::new();
        update(
            &temp.local(),
            &temp.source(),
            Change::Toggle(false),
            &AtomicBool::new(true),
        )
        .unwrap();
        let before = std::fs::read(temp.local()).unwrap();
        assert!(
            update(
                &temp.local(),
                &temp.source(),
                Change::Toggle(true),
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(
            update(
                &temp.local(),
                &temp.source(),
                Change::SelectProfile("missing".into()),
                &AtomicBool::new(true)
            )
            .is_err()
        );
        assert_eq!(std::fs::read(temp.local()).unwrap(), before);
    }
    #[test]
    fn studio_crud_publishes_complete_editors_and_the_shared_projection_together() {
        let temp = Temp::new();
        let first = studio(
            &temp,
            StudioChange::Add {
                id: "a".into(),
                name: "Effect 1".into(),
                editor: editor(1),
            },
        );
        assert!(first.selected().unwrap().is_default);
        studio(
            &temp,
            StudioChange::Duplicate {
                id: "a".into(),
                new_id: "b".into(),
                name: "Effect 1 (1)".into(),
            },
        );
        studio(
            &temp,
            StudioChange::Add {
                id: "c".into(),
                name: "Effect 2".into(),
                editor: editor(2),
            },
        );
        studio(&temp, StudioChange::Select { id: "b".into() });
        let renamed = studio(
            &temp,
            StudioChange::Rename {
                id: "b".into(),
                name: "Second".into(),
            },
        );
        assert_eq!(renamed.selected().unwrap().name(), "Second");
        let saved = studio(
            &temp,
            StudioChange::Save {
                id: "b".into(),
                editor: editor(3),
            },
        );
        assert_eq!(saved.selected().unwrap().editor().layers, [3]);
        assert!(!saved.selected().unwrap().is_default);
        let deleted = studio(&temp, StudioChange::Delete { id: "b".into() });
        assert_eq!(deleted.selected_id(), Some("c"));
        let config = load(&temp.local(), &temp.source())
            .unwrap()
            .configuration
            .unwrap();
        assert_eq!(config.selected_profile(), Some("c"));
        assert_eq!(
            config.profiles.iter().map(Profile::id).collect::<Vec<_>>(),
            ["a", "c"]
        );
        assert!(config.enabled());
        assert_eq!(
            load_studio::<Editor>(&temp.local(), validate_editor).unwrap(),
            deleted
        );
        assert!(!temp.source().exists());
        let deleted = studio(&temp, StudioChange::Delete { id: "c".into() });
        assert_eq!(deleted.selected_id(), Some("a"));
        let before = std::fs::read(temp.local()).unwrap();
        assert!(
            update_studio(
                &temp.local(),
                &temp.source(),
                StudioChange::<Editor>::Delete { id: "a".into() },
                &AtomicBool::new(true),
                validate_editor
            )
            .is_err()
        );
        assert_eq!(std::fs::read(temp.local()).unwrap(), before);
    }
    #[test]
    fn studio_rename_keeps_selection_made_by_another_local_consumer() {
        let temp = Temp::new();
        studio(
            &temp,
            StudioChange::Add {
                id: "a".into(),
                name: "First".into(),
                editor: editor(1),
            },
        );
        studio(
            &temp,
            StudioChange::Duplicate {
                id: "a".into(),
                new_id: "b".into(),
                name: "Second".into(),
            },
        );
        update(
            &temp.local(),
            &temp.source(),
            Change::SelectProfile("a".into()),
            &AtomicBool::new(true),
        )
        .unwrap();
        update(
            &temp.local(),
            &temp.source(),
            Change::Toggle(false),
            &AtomicBool::new(true),
        )
        .unwrap();
        let changed = studio(
            &temp,
            StudioChange::Rename {
                id: "b".into(),
                name: "Other".into(),
            },
        );
        assert_eq!(changed.selected_id(), Some("a"));
        let config = load(&temp.local(), &temp.source())
            .unwrap()
            .configuration
            .unwrap();
        assert_eq!(config.selected_profile(), Some("a"));
        assert!(!config.enabled());
    }
    #[test]
    fn studio_roundtrip_retains_unknown_fields_at_each_persistence_boundary() {
        let temp = Temp::new();
        studio(
            &temp,
            StudioChange::Add {
                id: "a".into(),
                name: "First".into(),
                editor: editor(1),
            },
        );
        let mut document = decode(&std::fs::read_to_string(temp.local()).unwrap()).unwrap();
        document
            .extensions
            .insert("futureDocument".into(), 42.into());
        document
            .configuration
            .extensions
            .insert("futureConfiguration".into(), true.into());
        document.configuration.profiles[0]
            .source_fields
            .insert("linkedGames".into(), serde_json::json!(["local.exe"]));
        let mut library: StudioLibrary<Editor> =
            serde_json::from_value(document.extensions[STUDIO_LIBRARY].clone()).unwrap();
        library.extensions.insert("futureLibrary".into(), 43.into());
        library.profiles[0]
            .extensions
            .insert("futureProfile".into(), 44.into());
        library.profiles[0]
            .editor
            .extensions
            .insert("futureEditor".into(), 45.into());
        document.extensions.insert(
            STUDIO_LIBRARY.into(),
            serde_json::to_value(library).unwrap(),
        );
        std::fs::write(temp.local(), serde_json::to_vec(&document).unwrap()).unwrap();
        let library = studio(
            &temp,
            StudioChange::Rename {
                id: "a".into(),
                name: "Renamed".into(),
            },
        );
        assert_eq!(library.extensions["futureLibrary"], 43);
        assert_eq!(library.profiles[0].extensions["futureProfile"], 44);
        assert_eq!(library.profiles[0].editor.extensions["futureEditor"], 45);
        let after = decode(&std::fs::read_to_string(temp.local()).unwrap()).unwrap();
        assert_eq!(after.extensions["futureDocument"], 42);
        assert_eq!(after.configuration.extensions["futureConfiguration"], true);
        assert_eq!(
            after.configuration.profiles[0].source_fields["linkedGames"],
            serde_json::json!(["local.exe"])
        );
    }
    #[test]
    fn studio_invalid_or_canceled_edits_leave_the_last_valid_document_intact() {
        let temp = Temp::new();
        let library = studio(
            &temp,
            StudioChange::Add {
                id: "a".into(),
                name: "First".into(),
                editor: editor(1),
            },
        );
        let before = std::fs::read(temp.local()).unwrap();
        for change in [
            StudioChange::Save {
                id: "a".into(),
                editor: editor(0),
            },
            StudioChange::Select {
                id: "missing".into(),
            },
            StudioChange::Rename {
                id: "a".into(),
                name: "😀".repeat(17),
            },
            StudioChange::Duplicate {
                id: "a".into(),
                new_id: "b".into(),
                name: "First".into(),
            },
        ] {
            assert!(library.changed(change.clone(), validate_editor).is_err());
            assert!(
                update_studio(
                    &temp.local(),
                    &temp.source(),
                    change,
                    &AtomicBool::new(true),
                    validate_editor
                )
                .is_err()
            );
            assert_eq!(std::fs::read(temp.local()).unwrap(), before);
        }
        assert!(
            update_studio(
                &temp.local(),
                &temp.source(),
                StudioChange::Save {
                    id: "a".into(),
                    editor: editor(2)
                },
                &AtomicBool::new(false),
                validate_editor
            )
            .is_err()
        );
        assert_eq!(std::fs::read(temp.local()).unwrap(), before);
        let preview = library
            .changed(
                StudioChange::Rename {
                    id: "a".into(),
                    name: "Preview".into(),
                },
                validate_editor,
            )
            .unwrap();
        assert_eq!(preview.selected().unwrap().name(), "Preview");
        assert_eq!(library.selected().unwrap().name(), "First");
        assert_eq!(std::fs::read(temp.local()).unwrap(), before);
    }

    #[test]
    fn studio_legacy_draft_missing_valid_and_invalid_reads_preserve_the_original() {
        let temp = Temp::new();
        let path = temp.0.join("chroma-studio-draft.json");
        assert!(
            load_legacy_studio_draft_at::<Editor>(&path, validate_editor)
                .unwrap()
                .is_none()
        );
        std::fs::create_dir_all(&temp.0).unwrap();
        let valid = br#"{"version":1,"studio":{"layers":[2],"futureEditor":42}}"#;
        std::fs::write(&path, valid).unwrap();
        let draft = load_legacy_studio_draft_at::<Editor>(&path, validate_editor)
            .unwrap()
            .unwrap();
        assert_eq!(draft.layers, [2]);
        assert_eq!(draft.extensions["futureEditor"], 42);
        assert_eq!(std::fs::read(&path).unwrap(), valid);
        for invalid in [
            r#"{"version":2,"studio":{"layers":[2]}}"#,
            r#"{"version":1,"studio":{"layers":[0]}}"#,
            r#"{"version":1,"studio":{"layers":"bad"}}"#,
            r#"{"version":1}"#,
        ] {
            std::fs::write(&path, invalid).unwrap();
            assert!(load_legacy_studio_draft_at::<Editor>(&path, validate_editor).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
        }
    }
}

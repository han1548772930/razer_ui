//! Versioned local Chroma settings. Only this boundary knows file layout/JSON.
use anyhow::{Context as _, ensure};
use razer_model::chroma_settings::{Change, DashboardPreferences, Preferences, SettingsDraft};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fs::{File, OpenOptions, TryLockError},
    io::Read as _,
    path::{Path, PathBuf},
    time::Duration,
};

const VERSION: u32 = 1;
const MAX_BYTES: usize = 1024 * 1024;
const FILE_NAME: &str = "chroma-settings.json";
const LEGACY_SETTINGS: &str = "chroma-settings-local.json";
const LEGACY_DASHBOARD: &str = "chroma-ui.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Current,
    Legacy,
    Missing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    preferences: Preferences,
    origin: Origin,
}
impl Observation {
    pub fn preferences(&self) -> &Preferences {
        &self.preferences
    }
    pub fn origin(&self) -> Origin {
        self.origin
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Fields<T> {
    #[serde(flatten)]
    value: T,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}

#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    settings: Fields<SettingsDraft>,
    dashboard: Fields<DashboardPreferences>,
    #[serde(flatten)]
    extensions: Map<String, Value>,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            version: VERSION,
            settings: Fields::default(),
            dashboard: Fields::default(),
            extensions: Map::new(),
        }
    }
}
impl Document {
    fn preferences(&self) -> Preferences {
        Preferences::from_parts(self.settings.value.clone(), self.dashboard.value.clone())
    }
    fn apply(&mut self, changes: &[Change]) {
        let mut preferences = self.preferences();
        for change in changes {
            preferences.apply(change);
        }
        self.settings.value = preferences.settings().clone();
        self.dashboard.value = preferences.dashboard().clone();
    }
}

pub fn path() -> anyhow::Result<PathBuf> {
    Ok(crate::store_path()?.with_file_name(FILE_NAME))
}

fn bytes(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("open Chroma settings"),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_BYTES,
        "Chroma settings exceed the size limit"
    );
    Ok(Some(bytes))
}

fn load_at(path: &Path) -> anyhow::Result<(Document, Origin, Option<Vec<u8>>)> {
    if let Some(bytes) = bytes(path)? {
        let document: Document =
            serde_json::from_slice(&bytes).context("decode Chroma settings")?;
        ensure!(
            document.version == VERSION,
            "Unsupported Chroma settings version {}",
            document.version
        );
        return Ok((document, Origin::Current, Some(bytes)));
    }
    let settings = bytes(&path.with_file_name(LEGACY_SETTINGS))?;
    let dashboard = bytes(&path.with_file_name(LEGACY_DASHBOARD))?;
    let origin = if settings.is_some() || dashboard.is_some() {
        Origin::Legacy
    } else {
        Origin::Missing
    };
    let document = Document {
        settings: settings
            .as_deref()
            .map(serde_json::from_slice)
            .transpose()
            .context("decode legacy Chroma settings")?
            .unwrap_or_default(),
        dashboard: dashboard
            .as_deref()
            .map(serde_json::from_slice)
            .transpose()
            .context("decode legacy Chroma dashboard preferences")?
            .unwrap_or_default(),
        ..Document::default()
    };
    Ok((document, origin, None))
}

fn writer_lock(path: &Path) -> anyhow::Result<File> {
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
    for _ in 0..100 {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(TryLockError::WouldBlock) => std::thread::sleep(Duration::from_millis(10)),
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
    }
    anyhow::bail!("Chroma settings writer lock timed out")
}

fn publish(
    path: &Path,
    previous: Option<Vec<u8>>,
    document: &Document,
    before_publish: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let encoded = serde_json::to_vec_pretty(document)?;
    ensure!(
        encoded.len() <= MAX_BYTES,
        "Chroma settings exceed the size limit"
    );
    let mut temporary = crate::TemporaryDocument::create(path)?;
    temporary.write(&encoded)?;
    before_publish()?;
    ensure!(
        bytes(path)? == previous,
        "Chroma settings changed during publication; reload before saving"
    );
    temporary.install(path)?;
    Ok(())
}

fn update_at(path: &Path, changes: &[Change]) -> anyhow::Result<Preferences> {
    ensure!(
        !changes.is_empty(),
        "No Chroma settings changes were supplied"
    );
    let _lock = writer_lock(path)?;
    let (mut document, _, previous) = load_at(path)?;
    document.apply(changes);
    publish(path, previous, &document, || Ok(()))?;
    Ok(document.preferences())
}

/// Run on the retained settings worker. Missing data remains an observation,
/// while malformed/unsupported files fail without replacing the previous file.
pub fn load() -> anyhow::Result<Observation> {
    let (document, origin, _) = load_at(&path()?)?;
    Ok(Observation {
        preferences: document.preferences(),
        origin,
    })
}

/// Apply idempotent changes to the newest file under the writer lock. Unknown
/// fields and unrelated settings survive; success confirms only this local file.
pub fn update(changes: &[Change]) -> anyhow::Result<Preferences> {
    update_at(&path()?, changes)
}

#[cfg(test)]
#[path = "chroma_settings/tests.rs"]
mod tests;

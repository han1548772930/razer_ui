//! Manifest lookup and a verified per-file installation cache. No product prefetch.
use anyhow::{Context as _, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

mod credentials;
mod image_cache;
mod runtime;
mod transport;
pub use image_cache::resource_image_cache;
pub use runtime::{
    ResourceChanged, observe_resources, resource_errors, retry_resources, start_resources,
};

#[derive(Debug)]
pub struct ResourcePending;
impl std::fmt::Display for ResourcePending {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("资源正在加载")
    }
}
impl std::error::Error for ResourcePending {}

pub fn is_resource_pending(error: &anyhow::Error) -> bool {
    error.is::<ResourcePending>()
}

mod metadata {
    include!("../../../assets/synapse/tables/remote-assets.rs");
}

#[derive(Clone, Deserialize)]
pub(super) struct ResourceFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Deserialize)]
pub(super) struct Entry {
    #[serde(flatten)]
    pub file: ResourceFile,
    pub key: String,
    pub view_box: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Deserialize)]
struct ListEntry {
    key: String,
    provider: String,
}

#[derive(Deserialize)]
pub(super) struct Storage {
    pub provider: String,
    pub repository: String,
    pub revision: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct Manifest {
    schema_version: u32,
    pub storage: Storage,
    entries: Vec<Entry>,
    list_entries: Vec<ListEntry>,
    pub fonts: Vec<ResourceFile>,
    files: Vec<ResourceFile>,
    #[serde(skip)]
    keys: BTreeMap<String, usize>,
    #[serde(skip)]
    paths: BTreeMap<String, usize>,
}

fn safe_relative(path: &str) -> anyhow::Result<&Path> {
    ensure!(
        !path.contains('\\')
            && !path.contains(':')
            && !path.contains('%')
            && !path.contains('?')
            && !path.contains('#'),
        "Invalid resource path"
    );
    let relative = Path::new(path);
    ensure!(
        !relative.is_absolute() && !path.is_empty(),
        "Resource path must be relative"
    );
    ensure!(
        relative
            .components()
            .all(|part| matches!(part, Component::Normal(_))),
        "Invalid resource path component"
    );
    ensure!(
        path.split('/').all(|part| !part.is_empty()
            && part != "."
            && part != ".."
            && !part.ends_with('.')
            && !part.ends_with(' ')),
        "Invalid resource path component"
    );
    Ok(relative)
}

impl Manifest {
    fn validate(mut self) -> anyhow::Result<Self> {
        let manifest = &mut self;
        ensure!(
            manifest.schema_version == 1 && manifest.storage.provider == "huggingface",
            "Unsupported resource metadata"
        );
        let repository = manifest.storage.repository.split('/').collect::<Vec<_>>();
        ensure!(
            repository.len() == 2
                && repository.iter().all(|part| !part.is_empty()
                    && *part != "."
                    && *part != ".."
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))),
            "Invalid resource repository"
        );
        if let Some(revision) = &manifest.storage.revision {
            ensure!(
                revision.len() == 40 && revision.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid resource revision"
            );
        }
        for file in manifest
            .files
            .iter()
            .chain(&manifest.fonts)
            .chain(manifest.entries.iter().map(|entry| &entry.file))
        {
            safe_relative(&file.path)?;
            ensure!(
                file.path.starts_with("assets/"),
                "Resource path is outside assets"
            );
            ensure!(
                file.sha256.len() == 64 && file.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid resource checksum"
            );
        }
        for (ix, entry) in manifest.entries.iter().enumerate() {
            manifest.keys.entry(entry.key.clone()).or_insert(ix);
        }
        for (ix, file) in manifest.files.iter().enumerate() {
            if let Some(previous) = manifest.paths.insert(file.path.clone(), ix) {
                let old = &manifest.files[previous];
                ensure!(
                    old.sha256 == file.sha256 && old.bytes == file.bytes,
                    "Conflicting resource path"
                );
            }
        }
        for file in manifest
            .fonts
            .iter()
            .chain(manifest.entries.iter().map(|entry| &entry.file))
        {
            let registered = manifest
                .paths
                .get(&file.path)
                .map(|&ix| &manifest.files[ix])
                .context("Resource alias has no registered file")?;
            ensure!(
                registered.bytes == file.bytes && registered.sha256 == file.sha256,
                "Resource alias identity differs from its file"
            );
        }
        ensure!(!manifest.keys.is_empty(), "Resource metadata is empty");
        Ok(self)
    }
}

pub(super) fn manifest() -> anyhow::Result<&'static Manifest> {
    static MANIFEST: OnceLock<Result<Manifest, String>> = OnceLock::new();
    MANIFEST
        .get_or_init(|| {
            metadata::manifest()
                .validate()
                .map_err(|error| format!("{error:#}"))
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

pub(super) fn list(prefix: &str) -> anyhow::Result<Vec<&'static str>> {
    Ok(manifest()?
        .list_entries
        .iter()
        .filter(|entry| {
            if entry.provider == "audio_demo_controls" {
                matches!(prefix, "" | "synapse" | "synapse/")
            } else {
                entry.key.starts_with(prefix)
            }
        })
        .map(|entry| entry.key.as_str())
        .collect())
}

#[cfg(test)]
pub(crate) fn identity(key: &str) -> Option<(&'static str, u64, &'static str)> {
    let manifest = manifest().ok()?;
    let file = &manifest.entries[*manifest.keys.get(key)?].file;
    Some((&file.path, file.bytes, &file.sha256))
}

pub(super) fn load(key: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let manifest = manifest()?;
    let Some(&ix) = manifest.keys.get(key) else {
        return Ok(None);
    };
    let entry = &manifest.entries[ix];
    let bytes = cached_or_request(&entry.file)?.ok_or(ResourcePending)?;
    if let Some(view_box) = &entry.view_box {
        let svg = std::str::from_utf8(&bytes).context("Resource SVG is not UTF-8")?;
        let root_end = svg.find('>').context("Resource SVG has no root")?;
        let dimensions = format!(
            " width=\"{}\" height=\"{}\" viewBox=\"{view_box}\"",
            entry.width.unwrap_or(20),
            entry.height.unwrap_or(20)
        );
        let mut selected = String::with_capacity(svg.len() + dimensions.len());
        selected.push_str(&svg[..root_end]);
        selected.push_str(&dimensions);
        selected.push_str(&svg[root_end..]);
        return Ok(Some(selected.into_bytes()));
    }
    Ok(Some(bytes))
}

pub(super) fn load_path(path: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let manifest = manifest()?;
    let Some(&ix) = manifest.paths.get(path) else {
        return Ok(None);
    };
    let bytes = cached_or_request(&manifest.files[ix])?.ok_or(ResourcePending)?;
    Ok(Some(bytes))
}

pub(super) fn cached_or_request(file: &ResourceFile) -> anyhow::Result<Option<Vec<u8>>> {
    runtime::service()?.read_or_request(file)
}

static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Changes only after a requested file finishes loading or fails.
pub fn resource_generation() -> u64 {
    GENERATION.load(Ordering::Acquire)
}

pub(super) struct Cache {
    root: PathBuf,
}

impl Cache {
    fn alongside(executable: &Path) -> anyhow::Result<Self> {
        let directory = executable
            .parent()
            .context("Executable has no installation directory")?;
        Ok(Self {
            root: directory.join("assets/cache"),
        })
    }

    fn path(&self, file: &ResourceFile) -> anyhow::Result<PathBuf> {
        let source = safe_relative(&file.path)?;
        let relative = source
            .strip_prefix("assets")
            .context("Invalid cached resource path")?;
        // Each content version has its own name, so installations cannot reuse stale bytes.
        let mut path = self.root.join(relative);
        let name = path
            .file_name()
            .context("Resource has no filename")?
            .to_string_lossy();
        path.set_file_name(format!("{}-{name}", file.sha256));
        Ok(path)
    }

    fn read(&self, file: &ResourceFile) -> anyhow::Result<Option<Vec<u8>>> {
        let path = self.path(file)?;
        reject_links(&self.root, &path)?;
        let input = match File::open(&path) {
            Ok(input) => input,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error).context("Cannot read the installation resource cache"),
        };
        if input.metadata()?.len() != file.bytes {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        input
            .take(file.bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if !verified(file, &bytes) {
            return Ok(None);
        }
        Ok(Some(bytes))
    }

    fn publish(&self, file: &ResourceFile, bytes: &[u8]) -> anyhow::Result<()> {
        ensure!(
            verified(file, bytes),
            "Downloaded resource length or SHA-256 differs"
        );
        let path = self.path(file)?;
        reject_links(&self.root, &path)?;
        fs::create_dir_all(path.parent().context("Cache file has no directory")?)
            .context("Cannot create the installation resource cache")?;
        reject_links(&self.root, &path)?;
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let temporary = path.with_extension(format!(
            "partial-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| -> anyhow::Result<()> {
            let mut output = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            output.write_all(bytes)?;
            output.sync_all()?;
            drop(output);
            // A previous valid cache entry wins; a corrupt entry is replaced only now.
            if path.exists() {
                if self.read(file)?.is_some() {
                    return Ok(());
                }
                fs::remove_file(&path)?;
            }
            fs::rename(&temporary, &path)?;
            Ok(())
        })();
        if temporary.exists() {
            let _ = fs::remove_file(&temporary);
        }
        result.context("Cannot publish the verified installation resource")
    }
}

fn verified(file: &ResourceFile, bytes: &[u8]) -> bool {
    bytes.len() as u64 == file.bytes && format!("{:x}", Sha256::digest(bytes)) == file.sha256
}

fn embedded(path: &str) -> Option<&'static [u8]> {
    macro_rules! native {
        ($name:literal) => {
            include_bytes!(concat!(
                "../../../assets/synapse/shared/pending-source-audit/",
                $name
            )) as &[u8]
        };
    }
    let name = path.strip_prefix("assets/synapse/shared/pending-source-audit/")?;
    Some(match name {
        "a64b8a64b721fb74-tray-native.rgba" => native!("a64b8a64b721fb74-tray-native.rgba"),
        "f979021796dfd342-tray-app.rgba" => native!("f979021796dfd342-tray-app.rgba"),
        "81851a9004a803d3-tray-icon-24.rgba" => native!("81851a9004a803d3-tray-icon-24.rgba"),
        "71b354f660adb5db-tray-icon.rgba" => native!("71b354f660adb5db-tray-icon.rgba"),
        "f8fd980e3bec82fe-tray-icon-40.rgba" => native!("f8fd980e3bec82fe-tray-icon-40.rgba"),
        "5e5df98271550aca-tray-icon-48.rgba" => native!("5e5df98271550aca-tray-icon-48.rgba"),
        "afb1350c23ce2416-tray-icon-64.rgba" => native!("afb1350c23ce2416-tray-icon-64.rgba"),
        "7747f51f284ded65-tray-gear-dark.rgba" => native!("7747f51f284ded65-tray-gear-dark.rgba"),
        "33892a0b56b91958-tray-gear-light.rgba" => native!("33892a0b56b91958-tray-gear-light.rgba"),
        _ => return None,
    })
}

fn available(path: &str) -> anyhow::Result<bool> {
    let manifest = manifest()?;
    let file = manifest
        .keys
        .get(path)
        .map(|&ix| &manifest.entries[ix].file)
        .or_else(|| manifest.paths.get(path).map(|&ix| &manifest.files[ix]));
    file.map_or(Ok(true), |file| {
        cached_or_request(file).map(|bytes| bytes.is_some())
    })
}

fn reject_links(root: &Path, path: &Path) -> anyhow::Result<()> {
    ensure!(
        path.starts_with(root),
        "Cache path escapes the installation resource directory"
    );
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                ensure!(
                    !metadata.file_type().is_symlink(),
                    "Resource cache contains a symbolic link"
                );
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt as _;
                    ensure!(
                        metadata.file_attributes() & 0x400 == 0,
                        "Resource cache contains a reparse point"
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

pub(super) fn remote_url(storage: &Storage, file: &ResourceFile) -> anyhow::Result<String> {
    safe_relative(&file.path)?;
    Ok(format!(
        "https://huggingface.co/datasets/{}/resolve/{}/{}",
        storage.repository,
        storage.revision.as_deref().unwrap_or("main"),
        file.path
    ))
}

#[cfg(test)]
mod tests;

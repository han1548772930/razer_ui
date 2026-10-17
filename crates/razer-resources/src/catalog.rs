//! Installed, immutable resource identities and ordered aliases.
use anyhow::{Context as _, ensure};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Component, Path},
    sync::OnceLock,
};

#[derive(Clone, Deserialize)]
pub(crate) struct ResourceFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Deserialize)]
pub(crate) struct Entry {
    #[serde(flatten)]
    pub file: ResourceFile,
    pub key: String,
    pub view_box: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Deserialize)]
pub(crate) struct ListEntry {
    key: String,
    provider: String,
}

#[derive(Deserialize)]
pub(crate) struct Storage {
    pub provider: String,
    pub repository: String,
    pub revision: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct Manifest {
    schema_version: u32,
    pub storage: Storage,
    pub(crate) entries: Vec<Entry>,
    list_entries: Vec<ListEntry>,
    pub fonts: Vec<ResourceFile>,
    pub(crate) files: Vec<ResourceFile>,
    #[serde(skip)]
    pub(crate) keys: BTreeMap<String, usize>,
    #[serde(skip)]
    pub(crate) paths: BTreeMap<String, usize>,
}

pub(crate) fn safe_relative(path: &str) -> anyhow::Result<&Path> {
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
        {
            let revision = manifest
                .storage
                .revision
                .as_ref()
                .context("Resource repository requires a fixed revision")?;
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

pub(crate) fn manifest() -> anyhow::Result<&'static Manifest> {
    static MANIFEST: OnceLock<Result<Manifest, String>> = OnceLock::new();
    MANIFEST
        .get_or_init(|| {
            read_manifest()
                .and_then(Manifest::validate)
                .map_err(|error| format!("{error:#}"))
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

fn read_manifest() -> anyhow::Result<Manifest> {
    razer_resource_db::read_catalog(|connection| {
        let (schema_version, storage) = connection.query_row(
            "SELECT schema_version, provider, repository, revision FROM resource_storage WHERE id=1", [],
            |row| Ok((row.get(0)?, Storage { provider: row.get(1)?, repository: row.get(2)?, revision: row.get(3)? })),
        )?;
        let mut statement = connection.prepare(
            "SELECT a.key, f.path, f.bytes, f.sha256, a.view_box, a.width, a.height \
             FROM resource_alias a JOIN resource_file f ON f.id=a.file_id ORDER BY a.position",
        )?;
        let entries = statement
            .query_map([], |row| {
                Ok(Entry {
                    key: row.get(0)?,
                    file: ResourceFile {
                        path: row.get(1)?,
                        bytes: row.get(2)?,
                        sha256: row.get(3)?,
                    },
                    view_box: row.get(4)?,
                    width: row.get(5)?,
                    height: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut statement =
            connection.prepare("SELECT key, provider FROM resource_list ORDER BY position")?;
        let list_entries = statement
            .query_map([], |row| {
                Ok(ListEntry {
                    key: row.get(0)?,
                    provider: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut statement = connection.prepare(
            "SELECT f.path, f.bytes, f.sha256 FROM resource_font o JOIN resource_file f ON f.id=o.file_id ORDER BY o.position",
        )?;
        let fonts = statement
            .query_map([], |row| {
                Ok(ResourceFile {
                    path: row.get(0)?,
                    bytes: row.get(1)?,
                    sha256: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut statement = connection.prepare(
            "SELECT f.path, f.bytes, f.sha256 FROM resource_file_order o JOIN resource_file f ON f.id=o.file_id ORDER BY o.position",
        )?;
        let files = statement
            .query_map([], |row| {
                Ok(ResourceFile {
                    path: row.get(0)?,
                    bytes: row.get(1)?,
                    sha256: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Manifest {
            schema_version,
            storage,
            entries,
            list_entries,
            fonts,
            files,
            keys: BTreeMap::new(),
            paths: BTreeMap::new(),
        })
    })
}

#[cfg(test)]
mod database_tests {
    use super::*;

    #[test]
    fn database_index_retains_duplicate_priority_list_order_and_svg_viewports() {
        let manifest = manifest().unwrap();
        assert_eq!(manifest.entries.len(), 12012);
        assert_eq!(manifest.files.len(), 7784);
        assert_eq!(manifest.fonts.len(), 14);
        let duplicate = "synapse/mouse-customize/f4f5f13b5ea47e5816fbf38f.png";
        let positions = manifest
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (entry.key == duplicate).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(positions, [1255, 11900]);
        assert_eq!(manifest.keys[duplicate], positions[0]);
        assert_eq!(
            list("synapse/mouse-customize/")
                .unwrap()
                .iter()
                .filter(|&&key| key == duplicate)
                .count(),
            2
        );
        let hover =
            &manifest.entries[manifest.keys["synapse/audio-volume-external-sprite.svg#hover"]];
        assert_eq!(hover.view_box.as_deref(), Some("0 20 20 20"));
        assert_eq!((hover.width, hover.height), (Some(20), Some(20)));
        for (key, path, bytes, sha256) in [
            (
                "synapse/audio-demo-drag.svg",
                "assets/synapse/products/1392/7ca7feaf57221001-audio-demo-drag.svg",
                853,
                "7ca7feaf572210018a2c840515c4717862f019d4e9972ad3502813b8237a637e",
            ),
            (
                "synapse/audio-demo-close.svg",
                "assets/synapse/products/1392/282f3c4f97f6180e-audio-demo-close.svg",
                2309,
                "282f3c4f97f6180ef3acae3115d58e366f30cb741d4c77065490d944f4818b13",
            ),
            (
                "synapse/audio-1352-fullscreen.svg",
                "assets/synapse/products/1352/bc9ac130ca9cb459-fullscreen.svg",
                622,
                "bc9ac130ca9cb459528cfa6111b290bbd0680e5041d8ce3bb2a446fb7a4eb2b3",
            ),
            (
                "synapse/screen-color-picker.cur",
                "assets/synapse/shared/cursors/d99b9c33f3257a5e-screen-color-picker.cur",
                435430,
                "d99b9c33f3257a5eecdba55204ec93e30839c1702917b2e51889fdef018312f3",
            ),
            (
                "synapse/gamepad-lighting/settings.svg",
                "assets/synapse/shared/products/7bf93d2d7e3c9c40-settings.svg",
                830,
                "7bf93d2d7e3c9c409f10383fbfdd21dc7dfbb06fc0eeeea11b3b62ddf9da75e5",
            ),
            (
                "synapse/gamepad-lighting/studio.svg",
                "assets/synapse/shared/products/2c32261f123494c4a00331f5cbbad1eaf2d3cc632ef90c9b279486d366e2d57c.svg",
                1275155,
                "2c32261f123494c4a00331f5cbbad1eaf2d3cc632ef90c9b279486d366e2d57c",
            ),
            (
                "synapse/gamepad-lighting/visualizer.svg",
                "assets/synapse/shared/products/9a03a852d178f619a0fb87c80ebd650eaaa408448550c4be6b02f449fb2c18a3.svg",
                322580,
                "9a03a852d178f619a0fb87c80ebd650eaaa408448550c4be6b02f449fb2c18a3",
            ),
            (
                "synapse/gamepad-lighting/warning.svg",
                "assets/synapse/products/1422/863d6b3c0cb80e81929ab6e647c5aa12a460372dc0602e3a062770d6f46a4c0f.svg",
                643,
                "863d6b3c0cb80e81929ab6e647c5aa12a460372dc0602e3a062770d6f46a4c0f",
            ),
            (
                "synapse/mouse-customize/scrollmode-switch-bg.svg",
                "assets/synapse/shared/products/20708f1406af7dcf-scrollmode-switch-bg.svg",
                696,
                "20708f1406af7dcfac00890c8cd5be4ff4c246358e7153e193afbfdef5b87c78",
            ),
            (
                "synapse/mouse-customize/scrollmode-switch-top.svg",
                "assets/synapse/shared/products/4c9e234418bb0197-scrollmode-switch-top.svg",
                585,
                "4c9e234418bb019740060a4662835a0738426faeae6c032e4985d0ac931810a7",
            ),
            (
                "synapse/wireless-argb-3884-refresh-hover.svg",
                "assets/synapse/shared/products/141769f9885e1f51-wireless-argb-3884-refresh-hover.svg",
                222,
                "141769f9885e1f51aef92722fe55af46982d16ca5d397615350b95ee77325072",
            ),
            (
                "synapse/wireless-argb-3886-power-base.svg",
                "assets/synapse/products/3886/6844a1a2812121f0-wireless-argb-3886-power-base.svg",
                1169,
                "6844a1a2812121f0801780528331a228c879c9c1b60ec11e23e50a267935602d",
            ),
        ] {
            assert_eq!(identity(key), Some((path, bytes, sha256)));
        }
        assert!(list("synapse/audio-demo-control-").unwrap().is_empty());
        assert_eq!(
            &list("synapse/").unwrap()[..4],
            &[
                "synapse/audio-demo-control-play.svg",
                "synapse/audio-demo-control-pause.svg",
                "synapse/audio-demo-control-fullscreen.svg",
                "synapse/audio-demo-control-fullscreen-exit.svg",
            ]
        );
    }
}

pub fn list(prefix: &str) -> anyhow::Result<Vec<&'static str>> {
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

pub fn identity(key: &str) -> Option<(&'static str, u64, &'static str)> {
    let manifest = manifest().ok()?;
    let file = if let Some(&ix) = manifest.keys.get(key) {
        &manifest.entries[ix].file
    } else {
        &manifest.files[*manifest.paths.get(key)?]
    };
    Some((&file.path, file.bytes, &file.sha256))
}

pub(crate) fn select_view(entry: &Entry, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    if let Some(view_box) = &entry.view_box {
        let svg = std::str::from_utf8(bytes).context("Resource SVG is not UTF-8")?;
        let root_end = svg.find('>').context("Resource SVG has no root")?;
        let dimensions = format!(
            " width=\"{}\" height=\"{}\" viewBox=\"{view_box}\"",
            entry.width.unwrap_or(20),
            entry.height.unwrap_or(20)
        );
        return Ok(format!("{}{dimensions}{}", &svg[..root_end], &svg[root_end..]).into_bytes());
    }
    Ok(bytes.to_vec())
}

pub(crate) fn remote_url(storage: &Storage, file: &ResourceFile) -> anyhow::Result<String> {
    safe_relative(&file.path)?;
    Ok(format!(
        "https://huggingface.co/datasets/{}/resolve/{}/{}",
        storage.repository,
        storage
            .revision
            .as_deref()
            .context("Resource repository requires a fixed revision")?,
        file.path
    ))
}

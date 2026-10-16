use anyhow::{Context, bail};
use razer_asset_pack::PackReader;
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Seek},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

struct Manifest {
    ordered: Vec<&'static str>,
    keys: BTreeSet<&'static str>,
}

fn manifest() -> anyhow::Result<&'static Manifest> {
    static MANIFEST: OnceLock<Result<Manifest, String>> = OnceLock::new();
    MANIFEST
        .get_or_init(|| {
            let ordered: Vec<&'static str> = serde_json::from_str(include_str!(
                "../../../assets/synapse/indexes/asset-list.json"
            ))
            .map_err(|error| error.to_string())?;
            if ordered.is_empty() {
                return Err("The product asset list is empty".into());
            }
            let mut keys: BTreeSet<&'static str> = ordered.iter().copied().collect();
            keys.extend([
                "synapse/audio-demo-control-play.svg",
                "synapse/audio-demo-control-pause.svg",
                "synapse/audio-demo-control-fullscreen.svg",
                "synapse/audio-demo-control-fullscreen-exit.svg",
                "synapse/audio-volume-external-sprite.svg",
            ]);
            Ok(Manifest { ordered, keys })
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!("Invalid product asset list: {error}"))
}

pub(super) fn aliases() -> anyhow::Result<&'static [&'static str]> {
    Ok(&manifest()?.ordered)
}

fn pack_path(executable: &Path) -> anyhow::Result<PathBuf> {
    let directory = executable
        .parent()
        .context("Executable has no parent directory")?;
    let installed = directory.join("assets/synapse.pack");
    match installed.try_exists() {
        Ok(true) => return Ok(installed),
        Err(error) => return Err(error).context("Cannot inspect the installed asset pack"),
        Ok(false) => {}
    }
    if directory.file_name().is_some_and(|name| name == "deps") {
        return Ok(directory
            .parent()
            .context("Cargo dependency directory has no profile directory")?
            .join("assets/synapse.pack"));
    }
    Ok(installed)
}

fn check_keys<Source: Read + Seek>(
    reader: &PackReader<Source>,
    expected: &BTreeSet<&str>,
) -> anyhow::Result<()> {
    for key in expected {
        if reader.entry(key).is_none() {
            bail!("Product asset pack is missing declared asset: {key}");
        }
    }
    for key in reader.keys() {
        if !expected.contains(key) {
            bail!("Product asset pack contains an undeclared asset: {key}");
        }
    }
    Ok(())
}

fn reader() -> anyhow::Result<&'static Mutex<PackReader<File>>> {
    static READER: OnceLock<Result<Mutex<PackReader<File>>, String>> = OnceLock::new();
    READER
        .get_or_init(|| {
            let initialize = || -> anyhow::Result<Mutex<PackReader<File>>> {
                let path = pack_path(&std::env::current_exe()?)?;
                let file = File::open(&path).with_context(|| {
                    format!("Cannot open local asset pack {}; restore assets/synapse.pack alongside the executable", path.display())
                })?;
                let reader = PackReader::open(file)
                    .with_context(|| format!("Invalid asset pack {}", path.display()))?;
                check_keys(&reader, &manifest()?.keys)?;
                Ok(Mutex::new(reader))
            };
            initialize().map_err(|error| format!("{error:#}"))
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

pub(super) fn validate() -> anyhow::Result<()> {
    reader()?;
    Ok(())
}

pub(super) fn load(key: &str) -> anyhow::Result<Option<Vec<u8>>> {
    if !manifest()?.keys.contains(key) {
        return Ok(None);
    }
    let mut reader = reader()?
        .lock()
        .map_err(|_| anyhow::anyhow!("Product asset pack reader lock is poisoned"))?;
    let bytes = reader
        .read(key)
        .with_context(|| format!("Cannot read or verify product asset: {key}"))?
        .with_context(|| format!("Product asset pack is missing declared asset: {key}"))?;
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_asset_pack::{PackInput, write_pack};
    use std::io::Cursor;

    #[test]
    fn package_path_is_relative_to_the_executable_not_the_working_directory() {
        let executable = Path::new("unused-test-package").join("razer-ui.exe");
        assert_eq!(
            pack_path(&executable).unwrap(),
            Path::new("unused-test-package/assets/synapse.pack")
        );
        let test_executable = Path::new("unused-test-profile/deps/assets-tests.exe");
        assert_eq!(
            pack_path(test_executable).unwrap(),
            Path::new("unused-test-profile/assets/synapse.pack")
        );
    }

    #[test]
    fn stale_and_incomplete_packs_are_not_accepted_as_missing_images() {
        let mut bytes = Vec::new();
        write_pack(
            &mut bytes,
            &[PackInput {
                key: "synapse/example.svg",
                bytes: b"svg",
            }],
        )
        .unwrap();
        let reader = PackReader::open(Cursor::new(bytes)).unwrap();
        check_keys(&reader, &BTreeSet::from(["synapse/example.svg"])).unwrap();
        assert!(check_keys(&reader, &BTreeSet::from(["synapse/missing.svg"])).is_err());
        assert!(check_keys(&reader, &BTreeSet::new()).is_err());
    }

    #[test]
    fn unknown_assets_do_not_require_a_pack_or_fabricate_bytes() {
        assert!(
            load("synapse/this-asset-is-not-registered.svg")
                .unwrap()
                .is_none()
        );
    }
}

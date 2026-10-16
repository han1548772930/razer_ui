use anyhow::{Context as _, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// All installed catalog and downloaded media paths share this executable's cache.
pub struct InstallationPaths {
    cache: PathBuf,
    seed: PathBuf,
}

impl InstallationPaths {
    pub fn current() -> anyhow::Result<Self> {
        Self::from_executable(
            &std::env::current_exe().context("Cannot locate the installed executable")?,
        )
    }

    pub fn from_executable(executable: &Path) -> anyhow::Result<Self> {
        ensure!(
            executable.is_absolute(),
            "Installation executable path must be absolute"
        );
        let directory = executable
            .parent()
            .context("Executable has no installation directory")?;
        Ok(Self {
            cache: directory.join(crate::packaged::CACHE_RELATIVE),
            seed: directory.join(crate::packaged::CATALOG_FILENAME),
        })
    }

    pub fn cache_directory(&self) -> &Path {
        &self.cache
    }

    pub(super) fn catalog(&self) -> anyhow::Result<PathBuf> {
        self.catalog_with_identity(
            crate::packaged::CATALOG_LENGTH,
            &crate::packaged::CATALOG_SHA256,
        )
    }

    fn catalog_with_identity(&self, length: u64, digest: &[u8; 32]) -> anyhow::Result<PathBuf> {
        let path = self
            .cache
            .join("catalogs")
            .join(format!("{}.sqlite3", hex(&digest)));
        reject_links(&path)?;
        if matches_identity(&path, length, &digest)? {
            return Ok(path);
        }
        reject_links(&self.seed)?;
        let mut bytes = Vec::with_capacity(usize::try_from(length)?);
        File::open(&self.seed)
            .context("Packaged resource catalog is missing beside the executable; repair this installation")?
            .take(length + 1)
            .read_to_end(&mut bytes)?;
        let actual: [u8; 32] = Sha256::digest(&bytes).into();
        ensure!(
            bytes.len() as u64 == length && &actual == digest,
            "Packaged resource catalog differs from this application version"
        );
        self.publish_catalog(&bytes)
    }

    pub(super) fn publish_catalog(&self, seed: &[u8]) -> anyhow::Result<PathBuf> {
        let directory = self.cache.join("catalogs");
        let path = directory.join(format!("{:x}.sqlite3", Sha256::digest(seed)));
        reject_links(&path)?;
        if matches(&path, seed)? {
            return Ok(path);
        }
        fs::create_dir_all(&directory)
            .context("Cannot create the installed resource database directory")?;
        reject_links(&path)?;
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let temporary = directory.join(format!(
            "catalog-{}-{}.partial",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| -> anyhow::Result<()> {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(seed)?;
            file.sync_all()?;
            drop(file);
            reject_links(&path)?;
            if !matches(&path, seed)? {
                // Same-volume rename atomically replaces corrupt bytes. Other versions have separate names.
                fs::rename(&temporary, &path)?;
            }
            ensure!(
                matches(&path, seed)?,
                "Installed resource database differs from packaged bytes"
            );
            Ok(())
        })();
        if temporary.exists() {
            fs::remove_file(&temporary)
                .context("Cannot clean up resource database staging file")?;
        }
        result.context("Cannot publish the packaged resource database")?;
        Ok(path)
    }
}

fn matches(path: &Path, seed: &[u8]) -> anyhow::Result<bool> {
    matches_identity(path, seed.len() as u64, &Sha256::digest(seed).into())
}

fn hex(bytes: &[u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("Writing to a String cannot fail");
    }
    output
}

fn matches_identity(path: &Path, length: u64, expected: &[u8; 32]) -> anyhow::Result<bool> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("Cannot read the installed resource database"),
    };
    if file.metadata()?.len() != length {
        return Ok(false);
    }
    let mut hash = Sha256::new();
    std::io::copy(&mut file, &mut hash)?;
    let actual: [u8; 32] = hash.finalize().into();
    Ok(&actual == expected)
}

fn reject_links(path: &Path) -> anyhow::Result<()> {
    for component in path.ancestors() {
        let metadata = match fs::symlink_metadata(component) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error).context("Cannot inspect resource database path"),
        };
        ensure!(
            !metadata.file_type().is_symlink(),
            "Resource database path traverses a link"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "Resource database path traverses a reparse point"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remove_test_directory(root: &Path) {
        let actual = root.canonicalize().unwrap();
        assert_eq!(
            actual.parent(),
            Some(std::env::temp_dir().canonicalize().unwrap().as_path())
        );
        assert!(
            actual
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("razer-catalog-")
        );
        fs::remove_dir_all(actual).unwrap();
    }

    #[test]
    fn offline_seed_repairs_corruption_and_upgrade_keeps_previous_catalog_and_media() {
        let root = std::env::temp_dir().join(format!("razer-catalog-test-{}", std::process::id()));
        let paths = InstallationPaths::from_executable(&root.join("razer_ui.exe")).unwrap();
        let first = paths.publish_catalog(b"first seed").unwrap();
        let media = paths.cache_directory().join("downloaded.png");
        fs::write(&media, b"cached original").unwrap();
        fs::write(&first, b"corrupt").unwrap();
        assert_eq!(paths.publish_catalog(b"first seed").unwrap(), first);
        assert_eq!(fs::read(&first).unwrap(), b"first seed");
        let second = paths.publish_catalog(b"second seed").unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(&first).unwrap(), b"first seed");
        assert_eq!(fs::read(&media).unwrap(), b"cached original");
        // Only this test's explicitly owned directory is removed.
        remove_test_directory(&root);
    }

    #[test]
    fn a_file_in_the_cache_directory_reports_failure() {
        let root =
            std::env::temp_dir().join(format!("razer-catalog-blocked-{}", std::process::id()));
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::write(root.join("assets/cache"), b"not a directory").unwrap();
        let paths = InstallationPaths::from_executable(&root.join("razer_ui.exe")).unwrap();
        assert!(paths.publish_catalog(b"seed").is_err());
        remove_test_directory(&root);
    }

    #[test]
    fn external_catalog_is_required_on_cache_miss_and_valid_cache_works_without_it() {
        let root =
            std::env::temp_dir().join(format!("razer-catalog-external-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let paths = InstallationPaths::from_executable(&root.join("razer_ui.exe")).unwrap();
        let seed = b"packaged offline catalog";
        let digest: [u8; 32] = Sha256::digest(seed).into();
        assert!(
            paths
                .catalog_with_identity(seed.len() as u64, &digest)
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
        fs::write(&paths.seed, b"a different application version").unwrap();
        assert!(
            paths
                .catalog_with_identity(seed.len() as u64, &digest)
                .is_err()
        );
        fs::write(&paths.seed, seed).unwrap();
        let cache = paths
            .catalog_with_identity(seed.len() as u64, &digest)
            .unwrap();
        assert_eq!(fs::read(&cache).unwrap(), seed);
        fs::write(&cache, b"corrupt cached catalog").unwrap();
        assert_eq!(
            paths
                .catalog_with_identity(seed.len() as u64, &digest)
                .unwrap(),
            cache
        );
        assert_eq!(fs::read(&cache).unwrap(), seed);
        fs::remove_file(&paths.seed).unwrap();
        assert_eq!(
            paths
                .catalog_with_identity(seed.len() as u64, &digest)
                .unwrap(),
            cache
        );
        let next: [u8; 32] = Sha256::digest(b"new version").into();
        assert!(paths.catalog_with_identity(11, &next).is_err());
        assert_eq!(fs::read(&cache).unwrap(), seed);
        assert!(!cache.parent().unwrap().read_dir().unwrap().any(|entry| {
            entry
                .unwrap()
                .path()
                .extension()
                .is_some_and(|extension| extension == "partial")
        }));
        remove_test_directory(&root);
    }
}

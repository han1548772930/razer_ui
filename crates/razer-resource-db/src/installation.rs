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
            cache: directory.join("assets/cache"),
        })
    }

    pub fn cache_directory(&self) -> &Path {
        &self.cache
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
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("Cannot read the installed resource database"),
    };
    if file.metadata()?.len() != seed.len() as u64 {
        return Ok(false);
    }
    let mut bytes = Vec::with_capacity(seed.len());
    file.take(seed.len() as u64 + 1).read_to_end(&mut bytes)?;
    Ok(bytes == seed)
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
}

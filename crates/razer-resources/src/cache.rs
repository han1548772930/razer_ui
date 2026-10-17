//! Content-verified installation cache; all callers run on resource workers.
use crate::catalog::{ResourceFile, safe_relative};
use anyhow::{Context as _, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) struct Cache {
    pub(crate) root: PathBuf,
}

impl Cache {
    pub(crate) fn alongside(executable: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            root: razer_resource_db::InstallationPaths::from_executable(executable)?
                .cache_directory()
                .to_owned(),
        })
    }

    pub(crate) fn path(&self, file: &ResourceFile) -> anyhow::Result<PathBuf> {
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

    pub(crate) fn read(&self, file: &ResourceFile) -> anyhow::Result<Option<Vec<u8>>> {
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

    pub(crate) fn publish(&self, file: &ResourceFile, bytes: &[u8]) -> anyhow::Result<()> {
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
        let (temporary, mut output) = TemporaryFile::create(temporary)
            .context("Cannot create the resource cache staging file")?;
        let result = (|| -> anyhow::Result<()> {
            output.write_all(bytes)?;
            output.sync_all()?;
            drop(output);
            // A previous valid cache entry wins; a corrupt entry is replaced only now.
            if path.exists() {
                if self.read(file)?.is_some() {
                    return Ok(());
                }
            }
            atomic_replace(&temporary.path, &path)?;
            Ok(())
        })();
        result.context("Cannot publish the verified installation resource")
    }
}

/// Cleans only a staging path this process successfully created exclusively.
struct TemporaryFile {
    path: PathBuf,
}

impl TemporaryFile {
    fn create(path: PathBuf) -> std::io::Result<(Self, File)> {
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        Ok((Self { path }, output))
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(windows)]
fn atomic_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: Both buffers are live, terminated paths in the same cache directory.
    // Replacement preserves any existing entry until the verified file is published.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

pub(crate) fn verified(file: &ResourceFile, bytes: &[u8]) -> bool {
    bytes.len() as u64 == file.bytes && format!("{:x}", Sha256::digest(bytes)) == file.sha256
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

#[cfg(test)]
pub(crate) mod tests;

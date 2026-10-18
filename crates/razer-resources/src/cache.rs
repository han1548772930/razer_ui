//! Content-verified installation cache; all callers run on resource workers.
use crate::catalog::{ResourceFile, safe_relative};
use anyhow::{Context as _, ensure};
use razer_resource_db::StagedResource;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
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

    pub(crate) fn publish<Permit>(
        &self,
        file: &ResourceFile,
        bytes: &[u8],
        authorize_commit: impl FnOnce() -> anyhow::Result<Permit>,
    ) -> anyhow::Result<()> {
        ensure!(
            verified(file, bytes),
            "Downloaded resource length or SHA-256 differs"
        );
        let path = self.path(file)?;
        reject_links(&self.root, &path)?;
        fs::create_dir_all(path.parent().context("Cache file has no directory")?)
            .context("Cannot create the installation resource cache")?;
        reject_links(&self.root, &path)?;
        let staged = StagedResource::write(&path, bytes)
            .context("Cannot create the resource cache staging file")?;
        let result = (|| -> anyhow::Result<()> {
            // A previous valid cache entry wins; a corrupt entry is replaced only now.
            if path.exists() {
                if self.read(file)?.is_some() {
                    return Ok(());
                }
            }
            // The service holds its commit permit through the final rename.
            // Staging, synchronization and old-cache validation happen before it.
            let permit = authorize_commit()?;
            let published = staged.publish();
            drop(permit);
            self.resolve_publication(file, published)
        })();
        result.context("Cannot publish the verified installation resource")
    }

    fn resolve_publication(
        &self,
        file: &ResourceFile,
        published: std::io::Result<()>,
    ) -> anyhow::Result<()> {
        if let Err(error) = published {
            // Another owner may have published these same immutable bytes.
            // A failed replacement is acceptable only after verifying that result.
            if !matches!(self.read(file), Ok(Some(_))) {
                return Err(error.into());
            }
        }
        Ok(())
    }
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

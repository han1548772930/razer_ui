//! Atomic publication of installed catalog and media files.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Owns a synchronized staging file until its owner authorizes publication.
/// The caller validates the destination and content before staging. Dropping
/// the value discards only the exclusively created file; it preserves the
/// destination and any staging file owned by another publisher.
pub struct StagedResource {
    temporary: Option<PathBuf>,
    destination: PathBuf,
    output: Option<File>,
}

impl StagedResource {
    pub fn write(destination: &Path, bytes: &[u8]) -> io::Result<Self> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let temporary = destination.with_extension(format!(
            "partial-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        Self::write_at(destination, temporary, bytes)
    }

    fn write_at(destination: &Path, temporary: PathBuf, bytes: &[u8]) -> io::Result<Self> {
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        let mut staged = Self {
            temporary: Some(temporary),
            destination: destination.to_owned(),
            output: Some(output),
        };
        let output = staged.output.as_mut().expect("Staging file is open");
        output.write_all(bytes)?;
        output.sync_all()?;
        // Windows requires the staging handle to close before replacement or cleanup.
        staged.output.take();
        Ok(staged)
    }

    /// Atomically replaces the destination. Cancellation and commit ordering
    /// remain the responsibility of the owner holding this value.
    pub fn publish(mut self) -> io::Result<()> {
        atomic_replace(
            self.temporary.as_deref().expect("Staging file is owned"),
            &self.destination,
        )?;
        self.temporary.take();
        Ok(())
    }
}

impl Drop for StagedResource {
    fn drop(&mut self) {
        self.output.take();
        if let Some(path) = self.temporary.take() {
            if let Err(error) = fs::remove_file(path) {
                if error.kind() != io::ErrorKind::NotFound {
                    eprintln!("Cannot clean up installed resource staging file: {error}");
                }
            }
        }
    }
}

#[cfg(windows)]
fn atomic_replace(source: &Path, target: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: Both buffers are live, terminated paths in the same cache directory.
    // Replacement preserves the previous entry until synchronized bytes are published.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, target: &Path) -> io::Result<()> {
    fs::rename(source, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "razer-resource-publication-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let root = self.0.canonicalize().unwrap();
            assert_eq!(
                root.parent(),
                Some(std::env::temp_dir().canonicalize().unwrap().as_path())
            );
            assert!(
                root.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("razer-resource-publication-")
            );
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn creation_failure_preserves_another_publishers_staging_file() {
        let directory = TestDirectory::new();
        let target = directory.0.join("catalog.sqlite3");
        let occupied = directory.0.join("occupied.partial");
        fs::write(&target, b"usable cache").unwrap();
        fs::write(&occupied, b"owned by another publisher").unwrap();
        assert!(StagedResource::write_at(&target, occupied.clone(), b"new").is_err());
        assert_eq!(fs::read(&target).unwrap(), b"usable cache");
        assert_eq!(fs::read(&occupied).unwrap(), b"owned by another publisher");
    }

    #[test]
    fn dropping_staged_bytes_preserves_the_destination_and_cleans_only_its_file() {
        let directory = TestDirectory::new();
        let target = directory.0.join("media.svg");
        fs::write(&target, b"usable cache").unwrap();
        let staged = StagedResource::write(&target, b"cancelled replacement").unwrap();
        let temporary = staged.temporary.as_ref().unwrap().clone();
        assert_eq!(fs::read(&temporary).unwrap(), b"cancelled replacement");
        assert_eq!(fs::read(&target).unwrap(), b"usable cache");
        drop(staged);
        assert!(!temporary.exists());
        assert_eq!(fs::read(&target).unwrap(), b"usable cache");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn publication_replaces_the_destination_after_staging_is_complete() {
        let directory = TestDirectory::new();
        let target = directory.0.join("catalog.sqlite3");
        fs::write(&target, b"old cache").unwrap();
        let staged = StagedResource::write(&target, b"complete replacement").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"old cache");
        staged.publish().unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"complete replacement");
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn failed_publication_discards_owned_bytes_without_removing_the_destination() {
        let directory = TestDirectory::new();
        let target = directory.0.join("occupied");
        fs::create_dir(&target).unwrap();
        let staged = StagedResource::write(&target, b"cannot replace a directory").unwrap();
        let temporary = staged.temporary.as_ref().unwrap().clone();
        assert!(staged.publish().is_err());
        assert!(target.is_dir());
        assert!(!temporary.exists());
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
}

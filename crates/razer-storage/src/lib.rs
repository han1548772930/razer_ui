//! Local draft persistence, source-confirmed receiver binding JSON and original
//! host process-local Map semantics. Storage does not write devices or claim
//! Chromium database or vendor profile file formats.
pub mod app_features;
pub mod game_catalog;
pub mod linked_profiles;
pub mod keyboard_calibration;
pub mod keyboard_analog_restore;
pub mod host;
pub mod macro_recorder;
pub mod profile_notification;
pub mod receiver_pairing;
pub mod receiver_reset;
pub mod receiver_indicator;
pub mod receiver_legacy;
pub mod source_identity;
pub mod source_local;
use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
#[cfg(test)]
mod document_tests;

/// Own only a newly created staging file. Concurrent saves and files left by
/// an earlier process must never share a truncatable temporary file.
struct TemporaryDocument {
    path: PathBuf,
    output: Option<File>,
    installed: bool,
}
impl TemporaryDocument {
    fn create(destination: &Path) -> std::io::Result<Self> {
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let filename = destination.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "missing document filename",
            )
        })?;
        loop {
            let mut name = filename.to_os_string();
            name.push(format!(
                ".{}.{}.tmp",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            let path = destination.with_file_name(name);
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(output) => {
                    return Ok(Self {
                        path,
                        output: Some(output),
                        installed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        let output = self.output.as_mut().expect("staging file is open");
        output.write_all(bytes)?;
        output.sync_all()?;
        // Windows requires closing the staging handle before rename/removal.
        self.output.take();
        Ok(())
    }
    fn install(mut self, destination: &Path) -> std::io::Result<()> {
        std::fs::rename(&self.path, destination)?;
        self.installed = true;
        Ok(())
    }
}
impl Drop for TemporaryDocument {
    fn drop(&mut self) {
        self.output.take();
        if !self.installed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
pub fn read_document<T>(
    path: &Path,
    decode: impl FnOnce(&str) -> anyhow::Result<T>,
) -> anyhow::Result<Option<T>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    decode(&text).map(Some)
}
pub fn write_document<T: serde::Serialize>(
    path: &Path,
    file: &T,
    validate_previous: impl FnOnce(&str) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    // Recheck the current disk file, not just the version loaded at startup.
    // A newer app or external edit must not be silently replaced by this one.
    let previous = match std::fs::read(path) {
        Ok(bytes) => {
            validate_previous(std::str::from_utf8(&bytes)?)?;
            Some(bytes)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_vec_pretty(file)?;
    let mut temporary = TemporaryDocument::create(path)?;
    temporary.write(&text)?;
    let current = match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    anyhow::ensure!(
        current == previous,
        "配置文件在保存期间被其他程序修改；本次更改未写入，请重新读取后再保存。"
    );
    if let Some(previous) = previous {
        std::fs::write(path.with_extension("json.bak"), previous)?;
    }
    temporary.install(path)?;
    Ok(())
}
// Kept for the existing explicit CLI self-test.

pub fn store_path() -> std::path::PathBuf {
    std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("razer_ui")
        .join("profiles.json")
}

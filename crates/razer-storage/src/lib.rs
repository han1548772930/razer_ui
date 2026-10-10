//! Local draft persistence, source-confirmed receiver binding JSON and original
//! host process-local Map semantics. Storage does not write devices or claim
//! Chromium database or vendor profile file formats.
pub mod host;
pub mod receiver_pairing;
pub mod receiver_reset;
use std::path::Path;
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
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    use std::io::Write;
    let mut output = std::fs::File::create(&tmp)?;
    output.write_all(&text)?;
    output.sync_all()?;
    drop(output);
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
    std::fs::rename(&tmp, path)?;
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

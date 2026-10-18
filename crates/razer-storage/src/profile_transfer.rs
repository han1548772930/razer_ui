//! File boundaries for local drafts and the current profile envelope.
use crate::file_transfer::{Operation, Publication};
use razer_device::profile_transfer::{DecodedFile, TransferTarget};
use razer_model::{model::Profile, profile_transfer as local};
use std::path::Path;

pub fn read_local(
    operation: &Operation,
    path: &Path,
    pid: u32,
    layout: u32,
) -> Result<Profile, String> {
    let bytes = operation.read(path, local::MAX_BYTES)?;
    let profile = local::decode_profile(&bytes, pid, layout)?;
    operation.check()?;
    Ok(profile)
}

pub fn write_local(operation: &Operation, path: &Path, bytes: &[u8]) -> Result<(), String> {
    operation.write(path, bytes, local::MAX_BYTES, Publication::CreateNew)
}

pub fn read_source(
    operation: &Operation,
    path: &Path,
    target: &serde_json::Value,
) -> Result<DecodedFile, String> {
    let bytes = operation.read(path, razer_device::profile_transfer::MAX_BYTES)?;
    let file = razer_device::profile_transfer::decode(&bytes, target)
        .map_err(|error| error.to_string())?;
    operation.check()?;
    Ok(file)
}

pub fn read_global(
    operation: &Operation,
    path: &Path,
    target: &TransferTarget,
    default_profile: Option<&serde_json::Value>,
) -> Result<DecodedFile, String> {
    let bytes = operation.read(path, razer_device::profile_transfer::MAX_BYTES)?;
    let file = razer_device::profile_transfer::decode_global(&bytes, target, default_profile)
        .map_err(|error| error.to_string())?;
    operation.check()?;
    Ok(file)
}

pub fn write_source(operation: &Operation, path: &Path, bytes: &[u8]) -> Result<(), String> {
    operation.write(
        path,
        bytes,
        razer_device::profile_transfer::MAX_BYTES,
        Publication::Replace,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_transfer::Transfer;

    #[test]
    fn local_export_is_create_new_and_cancelled_source_export_preserves_it() {
        let path = std::env::temp_dir().join(format!(
            "razer-profile-transfer-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let first = Transfer::new();
        write_local(&first.accept().unwrap(), &path, b"first export").unwrap();
        let second = Transfer::new();
        assert!(write_local(&second.accept().unwrap(), &path, b"replacement").is_err());
        let cancelled = Transfer::new();
        let operation = cancelled.accept().unwrap();
        cancelled.cancel();
        assert!(write_source(&operation, &path, b"late export").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"first export");
        assert!(read_local(&operation, &path, 653, 0).is_err());
        std::fs::remove_file(path).unwrap();
    }
}

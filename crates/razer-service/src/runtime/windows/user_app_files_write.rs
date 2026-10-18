//! Signature-gated Apps publication from 0x274f4/0x4bdb0/0x4c10c/0x4c725.
use super::signature;
use anyhow::{Context, Result, ensure};
use razer_device::user_app_files::{AppFileResponse, AppServiceEvent, validate_write};
use razer_storage::file_transfer::{Publication, Transfer};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;

pub(super) struct Written {
    pub response: AppFileResponse,
    pub event: Option<AppServiceEvent>,
}
fn rejected(reason: impl Into<String>) -> Written {
    Written {
        response: AppFileResponse {
            result: false,
            reason: reason.into(),
        },
        event: None,
    }
}

pub(super) fn write(root: &Path, folder: &str, relative: &str, bytes: &[u8]) -> Result<Written> {
    if let Some(response) = validate_write(folder, relative, bytes.len() as u64)? {
        return Ok(Written {
            response,
            event: None,
        });
    }
    if !root.exists() {
        return Ok(rejected("Error : Apps directory does not exist."));
    }
    let target = super::removal::scoped_path(root, folder, relative)?;
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let root = root.canonicalize().context("Resolve Apps write root")?;
    let parent = target.parent().context("Apps file has no parent")?;
    // Validate the nearest existing ancestor before creating any directories.
    // Recheck the actual parent after creation to reject junction escapes.
    let ancestor = parent
        .ancestors()
        .find(|path| path.exists())
        .context("Apps parent has no existing ancestor")?;
    ensure!(
        ancestor.canonicalize()?.starts_with(&root),
        "Apps write target is outside its installed root"
    );
    let verification = if extension == "zip" {
        Ok(())
    } else {
        let staged = VerificationFile::create(bytes, &extension)?;
        let result = signature::verify(
            staged.file.as_ref().expect("verification handle is owned"),
            &staged.path,
            bytes,
            &extension,
        );
        complete_verification(result, staged.close())
    };
    if let Err(error) = verification {
        return Ok(rejected(error.to_string()));
    }
    if let Err(error) = std::fs::create_dir_all(parent) {
        return Err(error).context(format!(
            "Error : Failed to create directory. \"{}\"",
            target.display()
        ));
    }
    ensure!(
        parent.canonicalize()?.starts_with(&root),
        "Apps write parent is outside its installed root"
    );
    if target.exists() {
        ensure!(
            target.canonicalize()?.starts_with(&root),
            "Apps write target is outside its installed root"
        );
    }
    let transfer = Transfer::new();
    let operation = transfer.accept().map_err(anyhow::Error::msg)?;
    operation
        .write(&target, bytes, u32::MAX as usize, Publication::Replace)
        .map_err(anyhow::Error::msg)
        .context("Publish verified Apps file")?;
    Ok(Written {
        response: AppFileResponse {
            result: true,
            reason: String::new(),
        },
        event: Some(AppServiceEvent::from_native(
            0,
            serde_json::to_string(&serde_json::json!({"app":folder,"filePath":relative}))?,
        )),
    })
}

/// Trust providers require a pathname for MSI/MSIX and a handle for PE files.
/// This exclusive temporary owner never publishes or overwrites another file.
struct VerificationFile {
    file: Option<File>,
    path: PathBuf,
}

fn complete_verification(verification: Result<()>, cleanup: Result<()>) -> Result<()> {
    match (verification, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(verification), Err(cleanup)) => {
            anyhow::bail!("{verification:#}; verification input cleanup failed: {cleanup:#}")
        }
    }
}

impl VerificationFile {
    fn create(bytes: &[u8], extension: &str) -> Result<Self> {
        ensure!(
            !extension.contains(['/', '\\', '\0', ':']),
            "Invalid Apps extension"
        );
        let path = std::env::temp_dir().join(format!(
            "razer-app-verify-{}.{}",
            uuid::Uuid::new_v4(),
            extension
        ));
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)
            .context("Error : Failed to create temp file.")?;
        let mut staged = Self {
            file: Some(file),
            path,
        };
        let file = staged.file.as_mut().expect("verification file was opened");
        let written = file
            .write_all(bytes)
            .context("Error : Failed to write file.")
            .and_then(|()| file.sync_all().context("Flush Apps verification input"));
        if let Err(error) = written {
            complete_verification(Err(error), staged.close())?;
            unreachable!("failed input write cannot verify successfully");
        }
        Ok(staged)
    }
    fn close(mut self) -> Result<()> {
        self.file.take();
        std::fs::remove_file(&self.path).context("Remove Apps verification input")
    }
}
impl Drop for VerificationFile {
    fn drop(&mut self) {
        self.file.take();
        if let Err(error) = std::fs::remove_file(&self.path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                eprintln!("Apps verification temporary cleanup failed: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("razer-app-write-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn signed() -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/apps-signature/rsa-sha256.ico"),
        )
        .unwrap()
    }
    #[test]
    fn verification_cleanup_retains_both_failures_and_removes_owned_temporary() {
        let result = complete_verification(
            Err(anyhow::anyhow!("signature rejected")),
            Err(anyhow::anyhow!("sharing violation")),
        )
        .unwrap_err()
        .to_string();
        assert!(result.contains("signature rejected") && result.contains("sharing violation"));
        for extension in ["dll", "msi", "msix"] {
            let staged = VerificationFile::create(b"unsigned input", extension).unwrap();
            let path = staged.path.clone();
            let result = signature::verify(
                staged.file.as_ref().unwrap(),
                &path,
                b"unsigned input",
                extension,
            );
            assert!(complete_verification(result, staged.close()).is_err());
            assert!(!path.exists());
        }
    }
    #[test]
    fn signed_write_publishes_complete_bytes_event_and_actual_inventory() {
        let fixture = Fixture::new();
        let bytes = signed();
        let result = write(&fixture.0, "Common", "nested/image.ICO", &bytes).unwrap();
        assert!(result.response.result);
        assert_eq!(
            std::fs::read(fixture.0.join("Common/nested/image.ICO")).unwrap(),
            bytes
        );
        let event = result.event.unwrap();
        assert_eq!(event.event_id, "onsimpleadduserappfile");
        let inventory = razer_storage::user_app_files::read(&fixture.0, "Common").unwrap();
        assert!(inventory.result);
        assert!(inventory.user_apps.contains("nested\\\\image.ICO"));
    }
    #[test]
    fn failed_signature_preserves_existing_file_and_creates_no_destination() {
        let fixture = Fixture::new();
        std::fs::create_dir(fixture.0.join("Common")).unwrap();
        let target = fixture.0.join("Common/image.ico");
        std::fs::write(&target, b"prior valid bytes").unwrap();
        let mut bytes = signed();
        let last = bytes.len() - 8;
        bytes[last] ^= 1;
        let result = write(&fixture.0, "Common", "image.ico", &bytes).unwrap();
        assert!(!result.response.result);
        assert!(result.event.is_none());
        assert_eq!(std::fs::read(target).unwrap(), b"prior valid bytes");
        let result = write(
            &fixture.0,
            "NewApp",
            "missing/file.dll",
            b"unsigned executable",
        )
        .unwrap();
        assert!(!result.response.result);
        assert_eq!(result.response.reason, signature::NOT_TRUSTED);
        assert!(!fixture.0.join("NewApp").exists());
        assert_eq!(
            std::fs::read_dir(fixture.0.join("Common")).unwrap().count(),
            1
        );
    }
    #[test]
    fn zip_branch_is_unsigned_and_replaces_only_after_complete_staging() {
        let fixture = Fixture::new();
        for bytes in [b"first".as_slice(), b"second complete"] {
            assert!(
                write(&fixture.0, "Common", "bundle.ZIP", bytes)
                    .unwrap()
                    .response
                    .result
            );
            assert_eq!(
                std::fs::read(fixture.0.join("Common/bundle.ZIP")).unwrap(),
                bytes
            );
        }
        assert!(write(&fixture.0, "Common", "../escape.zip", b"payload").is_err());
        assert!(write(&fixture.0, "Common", "bad.zip:stream", b"payload").is_err());
        assert_eq!(
            std::fs::read_dir(fixture.0.join("Common")).unwrap().count(),
            1
        );
    }
}

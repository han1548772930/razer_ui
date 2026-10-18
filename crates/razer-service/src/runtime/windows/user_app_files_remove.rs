//! Direct Apps removals from native producers 0x2793e/0x27b22/0x27d72.
use anyhow::{Context, Result, ensure};
use razer_device::user_app_files::{AppFileResponse, AppServiceEvent, UserAppFilesOperation};
use std::{
    fs,
    os::windows::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::GetLastError,
    Storage::FileSystem::{
        DeleteFileW, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_READONLY, GetFileAttributesW,
        INVALID_FILE_ATTRIBUTES, RemoveDirectoryW, SetFileAttributesW,
    },
};

pub(super) struct Removal {
    pub response: AppFileResponse,
    pub event: Option<AppServiceEvent>,
}

enum Kind<'a> {
    App,
    Directory(&'a str),
    File { path: &'a str, hash: &'a str },
}

pub(super) fn remove(root: &Path, operation: &UserAppFilesOperation) -> Result<Option<Removal>> {
    let (folder, kind) = match operation {
        UserAppFilesOperation::RemoveApp { folder_name } => (folder_name, Kind::App),
        UserAppFilesOperation::RemoveDirectory {
            folder_name,
            dir_path,
        } => (folder_name, Kind::Directory(dir_path)),
        UserAppFilesOperation::RemoveFile {
            folder_name,
            file_path,
            file_hash,
        } => (
            folder_name,
            Kind::File {
                path: file_path,
                hash: file_hash,
            },
        ),
        _ => return Ok(None),
    };
    let outcome = execute(root, folder, kind)?;
    Ok(Some(outcome))
}

fn rejected(reason: impl Into<String>) -> Removal {
    Removal {
        response: AppFileResponse {
            result: false,
            reason: reason.into(),
        },
        event: None,
    }
}

fn execute(root: &Path, folder: &str, kind: Kind<'_>) -> Result<Removal> {
    ensure!(!folder.contains('\0'), "Apps Service arguments contain NUL");
    if folder.is_empty() {
        return Ok(rejected("Error : App is empty."));
    }
    let (relative, noun, empty_reason, event_kind, data) = match kind {
        Kind::App => (
            "",
            "App",
            "Error : App is not empty.",
            1,
            serde_json::json!({"app":folder}),
        ),
        Kind::Directory(path) => {
            ensure!(!path.contains('\0'), "Apps Service arguments contain NUL");
            if path.is_empty() {
                return Ok(rejected("Error : Directory path is empty."));
            }
            (
                path,
                "User App directory",
                "Error : User App directory is not empty.",
                2,
                serde_json::json!({"app":folder,"directoryPath":path}),
            )
        }
        Kind::File { path, hash } => {
            ensure!(
                !path.contains('\0') && !hash.contains('\0'),
                "Apps Service arguments contain NUL"
            );
            if path.is_empty() {
                return Ok(rejected("Error : File path is empty."));
            }
            if hash.is_empty() {
                return Ok(rejected("Error : File hash is empty."));
            }
            (
                path,
                "User App file",
                "",
                3,
                serde_json::json!({"app":folder,"filePath":path,"fileHash":hash}),
            )
        }
    };
    if !root.exists() {
        return Ok(rejected("Error : Apps directory does not exist."));
    }
    let path = scoped_path(root, folder, relative)?;
    if !path.exists() {
        return Ok(rejected(format!("Error : {noun} does not exist.")));
    }
    // Keep a source app operation within its actual Apps root even when an
    // installed directory is a junction. Never recursively delete a tree.
    let actual_root = root
        .canonicalize()
        .context("Resolve Apps root before removal")?;
    let actual = path
        .canonicalize()
        .context("Resolve Apps target before removal")?;
    ensure!(
        actual != actual_root && actual.starts_with(&actual_root),
        "Apps removal target is outside its installed root"
    );
    match kind {
        Kind::File { hash, .. } => {
            let digest = match razer_storage::user_app_files::file_sha256(&path) {
                Ok(value) => value,
                Err(error) => {
                    return Ok(rejected(format!(
                        "Error : Cannot read User App file hash: {error:#}"
                    )));
                }
            };
            if digest != hash {
                return Ok(rejected("Error : Invalid User App file hash."));
            }
        }
        _ => {
            if path.is_dir() {
                let mut entries = match fs::read_dir(&path) {
                    Ok(value) => value,
                    Err(error) => {
                        return Ok(rejected(format!("Error : Cannot inspect {noun}: {error}")));
                    }
                };
                match entries.next() {
                    Some(Ok(_)) => return Ok(rejected(empty_reason)),
                    Some(Err(error)) => {
                        return Ok(rejected(format!("Error : Cannot inspect {noun}: {error}")));
                    }
                    None => {}
                }
            }
        }
    }
    if let Err(error) = delete(&path) {
        let label = if matches!(kind, Kind::App) {
            "User App"
        } else {
            noun
        };
        return Ok(rejected(format!(
            "Error : Failed to delete {label}. {error:#}"
        )));
    }
    Ok(Removal {
        response: AppFileResponse {
            result: true,
            reason: String::new(),
        },
        event: Some(AppServiceEvent::from_native(
            event_kind,
            serde_json::to_string(&data)?,
        )),
    })
}

pub(super) fn scoped_path(root: &Path, folder: &str, relative: &str) -> Result<PathBuf> {
    for value in [folder, relative] {
        ensure!(
            Path::new(value)
                .components()
                .all(|part| matches!(part, Component::Normal(_) | Component::CurDir)),
            "Apps removal path must be relative to the installed root"
        );
        ensure!(
            !value.contains(['*', '?', ':']),
            "Apps removal cannot address a wildcard or alternate stream"
        );
    }
    let path = root.join(folder).join(relative);
    ensure!(
        path != root && path.starts_with(root),
        "Apps removal cannot target the Apps root"
    );
    Ok(path)
}

fn delete(path: &Path) -> Result<()> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    // Native DoDeleteFile uses the legacy Windows path length boundary.
    ensure!(
        wide.len() <= 259,
        "Apps removal path exceeds the source Windows path limit"
    );
    wide.push(0);
    // SAFETY: wide is a terminated owned buffer retained through all calls.
    let attributes = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attributes == INVALID_FILE_ATTRIBUTES {
        let error = unsafe { GetLastError() };
        if matches!(error, 2 | 3) {
            return Ok(());
        }
        return Err(std::io::Error::from_raw_os_error(error as i32).into());
    }
    if attributes & FILE_ATTRIBUTE_READONLY != 0 {
        ensure!(
            unsafe { SetFileAttributesW(wide.as_ptr(), attributes & !FILE_ATTRIBUTE_READONLY) }
                != 0,
            "Clear source read-only attribute failed: {}",
            std::io::Error::last_os_error()
        );
    }
    let removed = unsafe {
        if attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
            RemoveDirectoryW(wide.as_ptr())
        } else {
            DeleteFileW(wide.as_ptr())
        }
    };
    if removed == 0 {
        let error = unsafe { GetLastError() };
        if !matches!(error, 2 | 3) {
            return Err(std::io::Error::from_raw_os_error(error as i32).into());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "user_app_files_remove_tests.rs"]
mod tests;

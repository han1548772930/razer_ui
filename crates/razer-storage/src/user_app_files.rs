//! Direct local Apps inventory, from Simple Service GetUserApps (RVA 0x26caa).
//! The caller owns the installed root; observations never use cached hashes.
use anyhow::{Context, Result, ensure};
use razer_device::user_app_files::UserAppsResponse;
use ring::digest::{Context as Digest, SHA256};
use serde::Serialize;
use std::{collections::BTreeMap, fs, io::Read, path::Path};

#[derive(Serialize)]
struct FileObservation {
    name: String,
    sha256: String,
}

/// The native callback contains an array of single-entry app dictionaries as
/// a JSON string. The failure callback at RVA 0x21fb0 supplies an empty string
/// when no app matched, even though the producer serialized an empty array.
pub fn read(root: &Path, name: &str) -> Result<UserAppsResponse> {
    ensure!(!name.contains('\0'), "Apps Service arguments contain NUL");
    if !root.exists() {
        return Ok(failed("Error : Apps directory does not exist."));
    }
    if !name.is_empty() && !root.join(name).exists() {
        return Ok(failed("Error : App does not exist."));
    }
    let mut applications = Vec::new();
    let mut failures = Vec::new();
    for item in entries(root, &mut failures) {
        if !item.path().is_dir() {
            continue;
        }
        let Ok(app_name) = item.file_name().into_string() else {
            failures.push(format!(
                "Installed app name is not valid Unicode: {}",
                item.path().display()
            ));
            continue;
        };
        if !name.is_empty() && name != app_name {
            continue;
        }
        applications.push(BTreeMap::from([(
            app_name,
            files(&item.path(), &mut failures),
        )]));
        if !name.is_empty() {
            break;
        }
    }
    Ok(UserAppsResponse {
        result: !applications.is_empty() && failures.is_empty(),
        reason: failures.join("; "),
        user_apps: if applications.is_empty() {
            String::new()
        } else {
            serde_json::to_string(&applications)?
        },
    })
}

fn failed(reason: &str) -> UserAppsResponse {
    UserAppsResponse {
        result: false,
        reason: reason.into(),
        user_apps: String::new(),
    }
}

fn entries(directory: &Path, failures: &mut Vec<String>) -> Vec<fs::DirEntry> {
    match fs::read_dir(directory) {
        Ok(entries) => entries
            .filter_map(|entry| match entry {
                Ok(entry) => Some(entry),
                Err(error) => {
                    failures.push(format!("Enumerate {}: {error}", directory.display()));
                    None
                }
            })
            .collect(),
        Err(error) => {
            failures.push(format!("Enumerate {}: {error}", directory.display()));
            Vec::new()
        }
    }
}

fn files(root: &Path, failures: &mut Vec<String>) -> Vec<FileObservation> {
    let mut pending = vec![root.to_owned()];
    let mut files = Vec::new();
    // The native enumerator completes each directory before popping the last
    // queued child. Reparse directories are not recursively followed.
    while let Some(directory) = pending.pop() {
        for entry in entries(&directory, failures) {
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    failures.push(format!("Inspect {}: {error}", path.display()));
                    continue;
                }
            };
            if path.is_dir() {
                if !is_reparse_directory(&metadata) {
                    pending.push(path);
                }
                continue;
            }
            let observation = (|| -> Result<FileObservation> {
                let name = path
                    .strip_prefix(root)?
                    .to_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Installed file name is not valid Unicode: {}",
                            path.display()
                        )
                    })?
                    .to_owned();
                Ok(FileObservation {
                    name,
                    sha256: file_sha256(&path)?,
                })
            })();
            // The source emits an empty digest on failed reads. Preserve the
            // actual failure instead of publishing it as a successful read.
            match observation {
                Ok(file) => files.push(file),
                Err(error) => failures.push(format!("{error:#}")),
            }
        }
    }
    files
}

fn is_reparse_directory(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    metadata.file_type().is_symlink()
}

/// Source uppercase file digest used by inventory and guarded file removal.
pub fn file_sha256(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("Open installed app file {}", path.display()))?;
    let mut digest = Digest::new(&SHA256);
    let mut bytes = [0_u8; 1024];
    loop {
        let count = file
            .read(&mut bytes)
            .context("Read installed app file for SHA-256")?;
        if count == 0 {
            break;
        }
        digest.update(&bytes[..count]);
    }
    Ok(digest
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect())
}

#[cfg(test)]
#[path = "user_app_files_tests.rs"]
mod tests;

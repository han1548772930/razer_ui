//! Current local Apps Service file contract. The native service owns its Apps
//! root, signature verification, writes and notifications; draft storage is not
//! a substitute for this response. See testdata/re/evidence/user-app-files-current.json.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppFileResponse {
    pub result: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAppsResponse {
    pub result: bool,
    pub reason: String,
    #[serde(rename = "userApps")]
    pub user_apps: String,
}

/// Source bytes are passed unchanged with an unsigned 32-bit byte count. A
/// local file is read by the worker, avoiding the JSON frame size limit; its
/// pathname is never substituted for the original destination filePath.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum AppFileInput {
    Bytes { data: Vec<u8> },
    LocalFile { path: std::path::PathBuf },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum UserAppFilesOperation {
    Subscribe {
        owner: String,
    },
    Unsubscribe {
        owner: String,
    },
    DrainEvents,
    Read {
        name: String,
    },
    Write {
        folder_name: String,
        file_path: String,
        input: AppFileInput,
    },
    RemoveFile {
        folder_name: String,
        file_path: String,
        file_hash: String,
    },
    RemoveDirectory {
        folder_name: String,
        dir_path: String,
    },
    RemoveApp {
        folder_name: String,
    },
}

impl UserAppFilesOperation {
    pub fn mutates_files(&self) -> bool {
        matches!(
            self,
            Self::Write { .. }
                | Self::RemoveFile { .. }
                | Self::RemoveDirectory { .. }
                | Self::RemoveApp { .. }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppServiceEvent {
    #[serde(rename = "eventID")]
    pub event_id: String,
    #[serde(rename = "eventData")]
    pub event_data: String,
}

impl AppServiceEvent {
    /// Current host createCbAppsServiceEvent forwards the untouched native
    /// payload to every live view; it never changes a write callback to success.
    pub fn from_native(kind: i32, event_data: String) -> Self {
        let event_id = match kind {
            0 => "onsimpleadduserappfile",
            1 => "onsimpleremoveuserapp",
            2 => "onsimpleremoveuserappdirectory",
            3 => "onsimpleremoveuserappfile",
            4 => "onsimplelaunchprocesserror",
            5 => "onsimpleprocesslaunched",
            6 => "onsimpleprocessexited",
            _ => "unknown",
        };
        Self {
            event_id: event_id.into(),
            event_data,
        }
    }
}

/// Native RVA 0x10660 validates these fields in order. Whitespace is not
/// trimmed; there is no invented empty-file success or automatic retry.
pub fn validate_write(
    folder: &str,
    path: &str,
    size: u64,
) -> anyhow::Result<Option<AppFileResponse>> {
    anyhow::ensure!(
        !folder.contains('\0') && !path.contains('\0'),
        "Apps Service arguments contain NUL"
    );
    anyhow::ensure!(
        size <= u32::MAX as u64,
        "Apps Service file exceeds its uint32 byte-count ABI"
    );
    let reason = if folder.is_empty() {
        Some("Error : App is empty.")
    } else if path.is_empty() {
        Some("Error : File path is empty.")
    } else if size == 0 {
        Some("Error : File data is empty.")
    } else {
        None
    };
    Ok(reason.map(|reason| AppFileResponse {
        result: false,
        reason: reason.into(),
    }))
}

/// Source-specific native dispatch, exposed for analysis/tests only. Runtime
/// verification stays in the installed service and must never be bypassed by
/// treating an unrecognized extension as an unsigned ordinary file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationRoute {
    ZipDirect,
    PackagePath,
    IconHandle,
    AuthenticodeHandle,
}

pub fn verification_route(extension: &str) -> VerificationRoute {
    match extension.to_ascii_lowercase().as_str() {
        ".zip" => VerificationRoute::ZipDirect,
        ".msi" | ".msix" => VerificationRoute::PackagePath,
        ".ico" => VerificationRoute::IconHandle,
        _ => VerificationRoute::AuthenticodeHandle,
    }
}

#[cfg(test)]
#[path = "user_app_files_tests.rs"]
mod tests;

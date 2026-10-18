//! Installed local host storage paths shared by file and service adapters.
use std::path::PathBuf;

/// Current host constants.js userDataDir. Installation observations and native
/// resource discovery share this source-confirmed platform path.
pub fn host_user_data_root() -> anyhow::Result<PathBuf> {
    #[cfg(windows)]
    {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};
        use windows::Win32::UI::Shell::{SHGFP_TYPE_CURRENT, SHGetFolderPathW};
        let mut directory = [0_u16; 260];
        // Source Apps path provider uses CSIDL_LOCAL_APPDATA with current
        // flags and no token, then appends these four fixed components.
        unsafe { SHGetFolderPathW(None, 28, None, SHGFP_TYPE_CURRENT.0 as u32, &mut directory) }?;
        let end = directory
            .iter()
            .position(|unit| *unit == 0)
            .ok_or_else(|| anyhow::anyhow!("Local AppData path was not terminated"))?;
        anyhow::ensure!(end != 0, "Local AppData path is empty");
        return Ok(PathBuf::from(OsString::from_wide(&directory[..end]))
            .join("Razer/RazerAppEngine/User Data"));
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|root| root.join("Library/Application Support/Razer/RazerAppEngine/User Data"))
            .ok_or_else(|| anyhow::anyhow!("The user home directory is unavailable"));
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    anyhow::bail!("The current Razer host has no installation path for this platform")
}

/// The Apps Service root. Existence is checked by the operation so a missing
/// installation remains distinct from an empty inventory.
pub fn user_apps_directory() -> anyhow::Result<PathBuf> {
    Ok(host_user_data_root()?.join("Apps"))
}

/// The current Electron host uses the default persistent Chromium session.
pub fn host_local_storage_directory() -> anyhow::Result<PathBuf> {
    Ok(host_user_data_root()?.join("Default/Local Storage/leveldb"))
}

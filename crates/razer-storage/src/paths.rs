//! User-owned configuration paths. Resolving a path never reads a document or
//! creates a directory; an unavailable location is an error for its consumer.
use anyhow::{Context as _, Result, ensure};
use std::{ffi::OsString, path::PathBuf};

const APPLICATION_DIRECTORY: &str = "razer_ui";
const PROFILES_FILE: &str = "profiles.json";
const RUNTIME_LOG_FILE: &str = "runtime.log";
const PANIC_LOG_FILE: &str = "panic.log";

fn absolute_directory(value: Option<OsString>, name: &str) -> Result<PathBuf> {
    let value =
        value.with_context(|| format!("User configuration environment {name} is unavailable"))?;
    ensure!(
        !value.is_empty(),
        "User configuration environment {name} is empty"
    );
    let path = PathBuf::from(value);
    ensure!(
        path.is_absolute(),
        "User configuration environment {name} is not an absolute path"
    );
    Ok(path)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_configuration_directory() -> Result<PathBuf> {
    unix_configuration_directory(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

#[cfg(any(all(unix, not(target_os = "macos")), test))]
fn unix_configuration_directory(
    xdg: Option<OsString>,
    user_home: Option<OsString>,
) -> Result<PathBuf> {
    match xdg {
        Some(xdg) => absolute_directory(Some(xdg), "XDG_CONFIG_HOME"),
        None => Ok(absolute_directory(user_home, "HOME")?.join(".config")),
    }
}

#[cfg(target_os = "macos")]
fn platform_configuration_directory() -> Result<PathBuf> {
    Ok(absolute_directory(std::env::var_os("HOME"), "HOME")?.join("Library/Application Support"))
}

#[cfg(windows)]
fn platform_configuration_directory() -> Result<PathBuf> {
    absolute_directory(std::env::var_os("APPDATA"), "APPDATA")
}

#[cfg(not(any(windows, unix)))]
fn platform_configuration_directory() -> Result<PathBuf> {
    anyhow::bail!("User configuration directories are not available on this platform")
}

pub fn configuration_directory() -> Result<PathBuf> {
    Ok(platform_configuration_directory()?.join(APPLICATION_DIRECTORY))
}

pub fn store_path() -> Result<PathBuf> {
    Ok(configuration_directory()?.join(PROFILES_FILE))
}

/// Local diagnostics share the user configuration directory.
pub fn runtime_log_path() -> Result<PathBuf> {
    Ok(configuration_directory()?.join(RUNTIME_LOG_FILE))
}

pub fn panic_log_path() -> Result<PathBuf> {
    Ok(configuration_directory()?.join(PANIC_LOG_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_empty_and_relative_directories_are_errors_without_working_directory_substitution() {
        for value in [
            None,
            Some(OsString::new()),
            Some(OsString::from(".")),
            Some(OsString::from("relative/config")),
        ] {
            assert!(absolute_directory(value, "APPDATA").is_err());
        }
        let root = std::env::temp_dir();
        assert!(root.is_absolute());
        assert_eq!(
            absolute_directory(Some(root.clone().into_os_string()), "APPDATA").unwrap(),
            root
        );
    }

    #[test]
    fn an_invalid_configured_xdg_directory_does_not_select_the_home_directory() {
        let user_home = std::env::temp_dir();
        let user_home = Some(user_home.into_os_string());
        for xdg in [
            OsString::new(),
            OsString::from("relative"),
            OsString::from("."),
        ] {
            assert!(unix_configuration_directory(Some(xdg), user_home.clone()).is_err());
        }
        assert!(unix_configuration_directory(None, None).is_err());
    }

    #[test]
    fn absent_xdg_uses_the_standard_home_location_and_explicit_xdg_keeps_its_location() {
        let user_home = std::env::temp_dir().join("profile-owner");
        let xdg = std::env::temp_dir().join("configuration-owner");
        assert_eq!(
            unix_configuration_directory(None, Some(user_home.clone().into_os_string())).unwrap(),
            user_home.join(".config")
        );
        assert_eq!(
            unix_configuration_directory(Some(xdg.clone().into_os_string()), None).unwrap(),
            xdg
        );
    }
}

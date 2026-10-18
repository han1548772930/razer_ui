//! Read-only Windows properties and explicit user-triggered property dialogs.

/// Read the local computer name through the current host's system API contract.
/// This performs a synchronous OS query and belongs on a background worker.
pub fn computer_name() -> anyhow::Result<String> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::computer_name()
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("computer name observation is only available on Windows")
    }
}

/// Settings `Fs` uses `new Date().getFullYear()`, i.e. the local calendar year.
pub fn copyright_year() -> String {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::copyright_year()
    }
    #[cfg(not(target_os = "windows"))]
    {
        // The reconstruction targets Windows. Omit an unavailable local year
        // instead of treating a UTC date or a frozen audit year as local time.
        String::new()
    }
}
/// The monitor pages branch their HDR tip on
/// `osName === windows && osVersion === "11"`; Windows 11 starts at build 22000.
/// This reads the same registry value the settings check uses and changes
/// nothing else.
pub fn is_windows_11() -> bool {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::is_windows_11()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Preserve an unavailable OS read instead of mistaking it for Windows 10.
/// MapText intentionally treats an unavailable version like Windows 11.
pub fn windows_major_version() -> Option<&'static str> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::windows_major_version()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Settings `ia` uses build 22631+, or 22621 with update revision 2506+.
/// Reading the OS version does not initialize a Razer service or change WDL.
pub fn supports_dynamic_lighting() -> bool {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::supports_dynamic_lighting()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[derive(Clone, Copy)]
pub enum Properties {
    Mouse,
    Keyboard,
    Sound,
    Volume,
    GameController,
}

/// The original color-management action launches `colorcpl.exe` through its
/// user-app launcher; this adapter directly launches the same utility.
pub fn open_color_management() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::open_color_management()
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("color management is only available on Windows")
    }
}
/// Current MapText's character-map action opens the Windows character utility.
pub fn open_character_map() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::open_character_map()
    }
    #[cfg(not(target_os = "windows"))]
    anyhow::bail!("Character Map is only available on Windows")
}
/// Current monitor refresh-rate links dispatch `msSettings("display")`.
/// The Windows adapter preserves the native ANSI command line and SW_SHOW.
pub fn open_display_settings() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::open_display_settings()
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("display settings are only available on Windows")
    }
}
/// Current THX instructions QT.gv invokes source msSettings("sound").
pub fn open_sound_settings() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::open_sound_settings()
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("sound settings are only available on Windows")
    }
}
/// Current Mixer Help's original helper invokes msSettings("apps-volume").
pub fn open_apps_volume_settings() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::open_apps_volume_settings()
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("app volume settings are only available on Windows")
    }
}

/// Request the source system restart after the notification's explicit action.
/// A successful result means the Windows utility was launched, not rebooted.
pub fn restart_workstation() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::restart_workstation()
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("system restart is only available on Windows")
    }
}

pub fn open(properties: Properties) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::system::open(properties)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = properties;
        anyhow::bail!("Windows only")
    }
}

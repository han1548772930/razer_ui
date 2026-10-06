//! Read-only Windows properties and explicit user-triggered property dialogs.

/// Settings `Fs` uses `new Date().getFullYear()`, i.e. the local calendar year.
pub(crate) fn copyright_year() -> String {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::{Foundation::SYSTEMTIME, System::SystemInformation::GetLocalTime};
        let mut local_time = SYSTEMTIME::default();
        // GetLocalTime initializes this writable SYSTEMTIME; no service is opened.
        unsafe { GetLocalTime(&mut local_time) };
        local_time.wYear.to_string()
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
pub(crate) fn is_windows_11() -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::Registry::{
            HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW,
        };
        let wide = |text: &str| text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let key = wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
        let build_name = wide("CurrentBuildNumber");
        let mut build = [0u16; 32];
        let mut size = std::mem::size_of_val(&build) as u32;
        // All buffers are live, writable and sized in bytes as required by Win32.
        if unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                build_name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                build.as_mut_ptr().cast(),
                &mut size,
            )
        } != 0
        {
            return false;
        }
        let end = build.iter().position(|c| *c == 0).unwrap_or(build.len());
        String::from_utf16(&build[..end])
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .is_some_and(|build| build >= 22000)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Settings `ia` uses build 22631+, or 22621 with update revision 2506+.
/// Reading the OS version does not initialize a Razer service or change WDL.
pub(crate) fn supports_dynamic_lighting() -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::Registry::{
            HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
        };
        let wide = |text: &str| text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let key = wide(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion");
        let build_name = wide("CurrentBuildNumber");
        let revision_name = wide("UBR");
        let mut build = [0u16; 32];
        let mut size = std::mem::size_of_val(&build) as u32;
        // All buffers are live, writable and sized in bytes as required by Win32.
        if unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                build_name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                build.as_mut_ptr().cast(),
                &mut size,
            )
        } != 0
        {
            return false;
        }
        let end = build.iter().position(|c| *c == 0).unwrap_or(build.len());
        let Some(build) = String::from_utf16(&build[..end])
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
        else {
            return false;
        };
        if build >= 22631 {
            return true;
        }
        if build != 22621 {
            return false;
        }
        let mut revision = 0u32;
        let mut size = std::mem::size_of_val(&revision) as u32;
        let read_revision = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                revision_name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut revision as *mut u32).cast(),
                &mut size,
            ) == 0
        };
        read_revision && revision >= 2506
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Properties {
    Mouse,
    Keyboard,
    Sound,
    Volume,
}

/// `RSA.openColorManagement`：原版通过 `simpleLaunchUserAppProcess("Common",
/// "wLauncher_v<version>.exe", "colorcpl.exe")` 拉起 Windows 的颜色管理面板，
/// 这里直接启动同一个程序。
pub(crate) fn open_color_management() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("colorcpl.exe").spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("color management is only available on Windows")
    }
}
/// Current MapText's character-map action opens the Windows character utility.
pub(crate) fn open_character_map() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("charmap.exe").spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    anyhow::bail!("Character Map is only available on Windows")
}
/// 3858/3880 刷新率组件里 `displaySettings` 链接的动作。
///
/// 源调用 `RiA.A.msSettings("display")`：Electron 下走宿主动作 `msSettings`
/// （`payload:{actionArgs:"display"}`），宿主把它转给 `sysutil/win` 原生模块
/// （`msSettings:["void",["string"]]`）。该模块的实现是 DLL，静态不可读；应用内可读
/// 的同族写法是 Dashboard 动作表里的
/// `case"PowerUserMenu":return"cmd /c start ms-settings:"`，因此这里用同样的
/// `cmd /c start ms-settings:<页面>` 打开源参数指定的 `display` 页。
pub(crate) fn open_display_settings() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "ms-settings:display"])
            .spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("display settings are only available on Windows")
    }
}
pub(crate) fn open(properties: Properties) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let mut command = match properties {
            Properties::Volume => std::process::Command::new("sndvol.exe"),
            _ => {
                let mut command = std::process::Command::new("control.exe");
                match properties {
                    Properties::Mouse => {
                        command.arg("main.cpl");
                    }
                    Properties::Keyboard => {
                        command.args(["main.cpl", ",@1"]);
                    }
                    Properties::Sound => {
                        command.arg("mmsys.cpl");
                    }
                    Properties::Volume => unreachable!(),
                }
                command
            }
        };
        command.spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = properties;
        anyhow::bail!("Windows only")
    }
}

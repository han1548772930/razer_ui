//! Windows registry, calendar and user-triggered system utilities.
use crate::system::Properties;

pub(crate) fn copyright_year() -> String {
    use windows_sys::Win32::{Foundation::SYSTEMTIME, System::SystemInformation::GetLocalTime};
    let mut local_time = SYSTEMTIME::default();
    // GetLocalTime initializes this writable SYSTEMTIME; no service is opened.
    unsafe { GetLocalTime(&mut local_time) };
    local_time.wYear.to_string()
}

pub(crate) fn is_windows_11() -> bool {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
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

pub(crate) fn supports_dynamic_lighting() -> bool {
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

pub(crate) fn open_color_management() -> anyhow::Result<()> {
    std::process::Command::new("colorcpl.exe").spawn()?;
    Ok(())
}

pub(crate) fn open_character_map() -> anyhow::Result<()> {
    std::process::Command::new("charmap.exe").spawn()?;
    Ok(())
}

pub(crate) fn open_display_settings() -> anyhow::Result<()> {
    std::process::Command::new("cmd")
        .args(["/c", "start", "ms-settings:display"])
        .spawn()?;
    Ok(())
}

pub(crate) fn open(properties: Properties) -> anyhow::Result<()> {
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

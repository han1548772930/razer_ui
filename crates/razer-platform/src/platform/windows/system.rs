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
    // msSettings RVA 0x70a30 formats this ANSI command and uses SW_SHOW.
    source_win_exec(c"explorer ms-settings:display")
}

pub(crate) fn open(properties: Properties) -> anyhow::Result<()> {
    // Current SysUtilsNative 0x70b40/60/80/a0 pass these exact command lines.
    // In particular "sounds" selects the source Sound tab and "keyboard"
    // is the original Keyboard control-panel dispatch.
    let command = match properties {
        Properties::Mouse => c"control main.cpl",
        Properties::Keyboard => c"control keyboard",
        Properties::Sound => c"control mmsys.cpl sounds",
        Properties::Volume => c"sndvol.exe",
        // Current OpenGameController RVA 0x3ffd0 uses this exact utility.
        Properties::GameController => c"control joy.cpl",
    };
    source_win_exec(command)
}

fn source_win_exec(command: &std::ffi::CStr) -> anyhow::Result<()> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn WinExec(command: *const std::ffi::c_char, show: u32) -> u32;
    }
    // Source uses 5 (SW_SHOW), not a shell's cmd/start process. Host declares
    // these native commands void; the Rust caller separately surfaces failure.
    let result = unsafe { WinExec(command.as_ptr(), 5) };
    anyhow::ensure!(
        result > 31,
        "Windows utility launch failed (WinExec {result})"
    );
    Ok(())
}

//! Windows registry, calendar and user-triggered system utilities.
use crate::system::Properties;

pub(crate) fn computer_name() -> anyhow::Result<String> {
    use anyhow::ensure;
    use windows_sys::Win32::{
        Globalization::{CP_THREAD_ACP, MultiByteToWideChar},
        System::WindowsProgramming::GetComputerNameA,
    };

    // SysUtilsNative::computerName (RVA 0x703b0) reads 256 ANSI bytes and
    // converts them using the calling thread's ANSI code page before UTF-8.
    let mut bytes = [0u8; 256];
    let mut length = bytes.len() as u32;
    // Both buffers remain writable for the synchronous Win32 call. The length
    // excludes the trailing NUL on success.
    let observed = unsafe { GetComputerNameA(bytes.as_mut_ptr(), &mut length) };
    ensure!(
        observed != 0,
        "Read computer name: {}",
        std::io::Error::last_os_error()
    );
    ensure!(
        length > 0 && (length as usize) < bytes.len(),
        "Invalid computer name length"
    );
    let mut wide = [0u16; 256];
    // The source conversion includes the NUL; our explicit length excludes
    // it so the resulting Rust string cannot contain that terminator.
    let converted = unsafe {
        MultiByteToWideChar(
            CP_THREAD_ACP,
            0,
            bytes.as_ptr(),
            length as i32,
            wide.as_mut_ptr(),
            wide.len() as i32,
        )
    };
    ensure!(
        converted > 0,
        "Decode computer name: {}",
        std::io::Error::last_os_error()
    );
    decode_computer_name(&wide, converted)
}

fn decode_computer_name(wide: &[u16], length: i32) -> anyhow::Result<String> {
    use anyhow::{Context as _, ensure};
    ensure!(
        length > 0 && (length as usize) <= wide.len(),
        "Invalid decoded computer name length"
    );
    let name = &wide[..length as usize];
    ensure!(!name.contains(&0), "Computer name contains an embedded NUL");
    String::from_utf16(name).context("Computer name is not valid UTF-16")
}

#[cfg(test)]
mod computer_name_tests {
    use super::decode_computer_name;

    #[test]
    fn observed_name_preserves_unicode_and_excludes_buffer_tail() {
        let mut buffer = "工作站-PC".encode_utf16().collect::<Vec<_>>();
        let length = buffer.len() as i32;
        buffer.extend([0, 65]);
        assert_eq!(decode_computer_name(&buffer, length).unwrap(), "工作站-PC");
    }

    #[test]
    fn invalid_counts_and_malformed_utf16_remain_errors() {
        for (buffer, length) in [
            (&[65u16][..], 0),
            (&[65][..], 2),
            (&[0][..], 1),
            (&[0xd800][..], 1),
        ] {
            assert!(decode_computer_name(buffer, length).is_err());
        }
    }
}

pub(crate) fn copyright_year() -> String {
    use windows_sys::Win32::{Foundation::SYSTEMTIME, System::SystemInformation::GetLocalTime};
    let mut local_time = SYSTEMTIME::default();
    // GetLocalTime initializes this writable SYSTEMTIME; no service is opened.
    unsafe { GetLocalTime(&mut local_time) };
    local_time.wYear.to_string()
}

pub(crate) fn is_windows_11() -> bool {
    windows_major_version() == Some("11")
}

pub(crate) fn windows_major_version() -> Option<&'static str> {
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
        return None;
    }
    let end = build.iter().position(|c| *c == 0).unwrap_or(build.len());
    String::from_utf16(&build[..end])
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .map(|build| if build >= 22000 { "11" } else { "10" })
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
pub(crate) fn open_sound_settings() -> anyhow::Result<()> {
    // Same IDA-verified SysUtils msSettings RVA 0x70a30 format as display;
    // the actual current THX instructions caller supplies "sound".
    source_win_exec(c"explorer ms-settings:sound")
}
pub(crate) fn open_apps_volume_settings() -> anyhow::Result<()> {
    // Same IDA-verified msSettings 0x70a30 format; source Mixer Help passes
    // apps-volume. Verification never invokes WinExec or the vendor DLL.
    source_win_exec(c"explorer ms-settings:apps-volume")
}

pub(crate) fn restart_workstation() -> anyhow::Result<()> {
    // Current SysUtilsNative.dll restartWorkstation, RVA 0x70370, passes
    // exactly this command to WinExec with SW_HIDE. The host declares void;
    // launch diagnostics do not assert that Windows actually restarted.
    source_win_exec_with_visibility(c"shutdown -r -t 0", 0)
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
    source_win_exec_with_visibility(command, 5)
}

fn source_win_exec_with_visibility(command: &std::ffi::CStr, show: u32) -> anyhow::Result<()> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn WinExec(command: *const std::ffi::c_char, show: u32) -> u32;
    }
    // The original commands specify SW_SHOW or SW_HIDE individually. Host
    // declares these commands void; the Rust caller separately retains errors.
    let result = unsafe { WinExec(command.as_ptr(), show) };
    anyhow::ensure!(
        result > 31,
        "Windows utility launch failed (WinExec {result})"
    );
    Ok(())
}

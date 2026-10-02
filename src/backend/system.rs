//! Explicit user-triggered Windows property dialogs; no device-write simulation.
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

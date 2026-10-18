//! System mappings at RVA 0xae48e, including the AoAc monitor-power path.
use anyhow::{Result, ensure};
use razer_device::mapping_native_action::SystemAction;
use std::{ptr, sync::OnceLock, time::Duration};
use windows_sys::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Power::{
                CallNtPowerInformation, SYSTEM_POWER_CAPABILITIES, SetSuspendState,
                SystemPowerCapabilities,
            },
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, HWND_MESSAGE, RegisterClassExW,
            SC_MONITORPOWER, SendMessageW, WM_SYSCOMMAND, WNDCLASSEXW,
        },
    },
    core::w,
};

pub(super) fn execute(action: SystemAction) -> Result<()> {
    if action == SystemAction::Sleep && modern_standby()? {
        let window = MessageWindow::open()?;
        // RVA 0x13447e delays the monitor-power command for 500 ms.
        std::thread::sleep(Duration::from_millis(500));
        unsafe {
            SendMessageW(window.0, WM_SYSCOMMAND, SC_MONITORPOWER as usize, 2);
        }
        window.close()
    } else {
        ensure!(
            unsafe { SetSuspendState(action == SystemAction::Hibernate, false, false) },
            "SetSuspendState: {}",
            std::io::Error::last_os_error()
        );
        Ok(())
    }
}

fn modern_standby() -> Result<bool> {
    let mut capabilities = SYSTEM_POWER_CAPABILITIES::default();
    // The complete typed structure is writable for the duration of this call.
    let status = unsafe {
        CallNtPowerInformation(
            SystemPowerCapabilities,
            ptr::null(),
            0,
            (&mut capabilities as *mut SYSTEM_POWER_CAPABILITIES).cast(),
            std::mem::size_of_val(&capabilities) as u32,
        )
    };
    ensure!(status == 0, "SystemPowerCapabilities: NTSTATUS {status:#x}");
    Ok(capabilities.AoAc)
}

struct MessageWindow(HWND);
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // No borrowed application state is retained by this window procedure.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}
impl MessageWindow {
    fn open() -> Result<Self> {
        static CLASS: OnceLock<Result<u16, String>> = OnceLock::new();
        let class = CLASS
            .get_or_init(|| {
                let registration = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(window_proc),
                    hInstance: unsafe { GetModuleHandleW(ptr::null()) },
                    lpszClassName: w!("RazerLocalMappingPower"),
                    ..Default::default()
                };
                let atom = unsafe { RegisterClassExW(&registration) };
                if atom == 0 {
                    Err(std::io::Error::last_os_error().to_string())
                } else {
                    Ok(atom)
                }
            })
            .as_ref()
            .map_err(|error| anyhow::anyhow!("Register mapping power window: {error}"))?;
        let window = unsafe {
            CreateWindowExW(
                0,
                *class as usize as *const u16,
                ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                ptr::null_mut(),
                GetModuleHandleW(ptr::null()),
                ptr::null(),
            )
        };
        ensure!(
            !window.is_null(),
            "Create mapping power window: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self(window))
    }
    fn close(mut self) -> Result<()> {
        ensure!(
            unsafe { DestroyWindow(self.0) } != 0,
            "Destroy mapping power window: {}",
            std::io::Error::last_os_error()
        );
        self.0 = ptr::null_mut();
        Ok(())
    }
}
impl Drop for MessageWindow {
    fn drop(&mut self) {
        if !self.0.is_null() && unsafe { DestroyWindow(self.0) } == 0 {
            eprintln!(
                "Mapping power window cleanup: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}

//! Electron main.js MAXIMIZE toggles isMaximized ? unmaximize : maximize.
//! GPUI 0.3.7's Windows zoom() only calls SW_MAXIMIZE, so it cannot restore.
use gpui_kit::Window;

/// Electron BrowserWindow.isVisible/isMinimized. GPUI is_visible() describes
/// frame presentation/occlusion and is not equivalent to BrowserWindow state.
pub(in crate::shell) fn status(
    window: &Window,
) -> anyhow::Result<Option<razer_model::host_window_focus::HostWindowStatus>> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindow, IsWindowVisible};
        let handle =
            HasWindowHandle::window_handle(window).map_err(|error| anyhow::anyhow!("{error}"))?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            anyhow::bail!("expected the owning Win32 window");
        };
        let hwnd = handle.hwnd.get() as _;
        unsafe {
            if IsWindow(hwnd) == 0 {
                return Ok(None);
            }
            Ok(Some(razer_model::host_window_focus::HostWindowStatus {
                is_visible: Some(IsWindowVisible(hwnd) != 0),
                is_minimized: Some(IsIconic(hwnd) != 0),
            }))
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        // No equivalent platform adapter has been established. Do not invent
        // BrowserWindow visibility from GPUI presentation state.
        let _ = window;
        anyhow::bail!("BrowserWindow status adapter is not implemented on this platform")
    }
}

pub(in crate::shell) fn toggle_maximize(window: &Window) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::RawWindowHandle;
        let handle = raw_window_handle::HasWindowHandle::window_handle(window)
            .map_err(|error| anyhow::anyhow!("{error}"))?;
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            anyhow::bail!("expected a Win32 window");
        };
        // Borrowed from this Window; never use the foreground window (which may
        // belong to a different application by the time this callback runs).
        unsafe { toggle_native(handle.hwnd.get() as _) }
    }
    #[cfg(not(target_os = "windows"))]
    {
        window.zoom_window();
        Ok(())
    }
}

#[cfg(target_os = "windows")]
unsafe fn toggle_native(hwnd: windows_sys::Win32::Foundation::HWND) -> anyhow::Result<()> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsZoomed, PostMessageW, SC_MAXIMIZE, SC_RESTORE, WM_SYSCOMMAND,
    };
    // Query the OS at activation time, including maximization through a double
    // click, snapping or the system menu. Do not maintain a second local state.
    let command = if unsafe { IsZoomed(hwnd) } != 0 {
        SC_RESTORE
    } else {
        SC_MAXIMIZE
    };
    // Posting avoids reentering GPUI with WM_SIZE while its UI callback is active.
    if unsafe { PostMessageW(hwnd, WM_SYSCOMMAND, command as usize, 0) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::toggle_native;
    use std::{
        ptr,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::*,
    };

    struct TestWindow(HWND);
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }

    fn wait_for(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !condition() {
            let mut message = MSG::default();
            unsafe {
                while PeekMessageW(&mut message, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            assert!(Instant::now() < deadline, "native window did not settle");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    #[ignore = "Requires a Windows desktop and opens a temporary native test window"]
    fn native_maximize_restore_preserves_bounds_and_reads_external_state() {
        let class = "STATIC\0".encode_utf16().collect::<Vec<_>>();
        let title = "Synapse window control regression\0"
            .encode_utf16()
            .collect::<Vec<_>>();
        let window = TestWindow(unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                100,
                100,
                640,
                480,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null(),
            )
        });
        assert!(!window.0.is_null());
        unsafe {
            ShowWindow(window.0, SW_SHOWNOACTIVATE);
        }
        let mut before = RECT::default();
        assert_ne!(unsafe { GetWindowRect(window.0, &mut before) }, 0);
        for _ in 0..2 {
            unsafe {
                toggle_native(window.0).unwrap();
            }
            wait_for(|| unsafe { IsZoomed(window.0) } != 0);
            unsafe {
                toggle_native(window.0).unwrap();
            }
            wait_for(|| unsafe { IsZoomed(window.0) } == 0);
            let mut after = RECT::default();
            assert_ne!(unsafe { GetWindowRect(window.0, &mut after) }, 0);
            assert_eq!(
                (after.left, after.top, after.right, after.bottom),
                (before.left, before.top, before.right, before.bottom)
            );
        }
        // A system-initiated maximize must also be reversible by our button.
        unsafe {
            ShowWindow(window.0, SW_MAXIMIZE);
        }
        wait_for(|| unsafe { IsZoomed(window.0) } != 0);
        unsafe {
            toggle_native(window.0).unwrap();
        }
        wait_for(|| unsafe { IsZoomed(window.0) } == 0);
    }
}

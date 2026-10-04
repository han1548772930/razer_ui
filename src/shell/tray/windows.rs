//! Windows HWND placement, visibility, foreground and notification diagnostics.
use super::{DesktopTray, Dismiss, TrayPopup};
use gpui_kit::*;

impl DesktopTray {
    pub(in crate::shell) fn show_popup(
        &mut self,
        toggle: bool,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        if self.popup.is_none() {
            cx.bind_keys([KeyBinding::new("escape", Dismiss, Some("TrayPopup"))]);
            let sender = self.sender.clone();
            let options = WindowOptions {
                // 宿主 `LeftSystray.js` 建窗用 width:300/height:200（最小同值），弹窗
                // 自己再 `resizeTo(360, header2.clientHeight + 列表高度)`；宽度取
                // `.systray{width:360px}`，高度被 minimum_height:200 夹住，因此最终
                // 就是 360x200。
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(0.), px(0.)),
                    size(px(360.), px(200.)),
                ))),
                window_min_size: Some(size(px(300.), px(200.))),
                window_background: WindowBackgroundAppearance::Transparent,
                titlebar: None,
                show: false,
                focus: false,
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                ..Default::default()
            };
            self.popup = Some(gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| TrayPopup::new(sender, window, cx))
            })?);
        }
        let (handle, popup) = self.popup.as_ref().expect("created tray popup");
        let rect = self.icon.rect();
        let popup = popup.clone();
        handle.update(cx, |_, window, cx| {
            popup.update(cx, |view, cx| {
                view.blur_task = None;
                if toggle && (native::visible(window) || view.show_task.is_some()) {
                    view.show_task = None;
                    native::hide(window);
                } else {
                    let placement = rect.and_then(|rect| native::popup_placement(window, rect));
                    view.show_task = Some(cx.spawn_in(window, async move |this, cx| {
                        if let Some(placement) = placement {
                            cx.background_spawn(async move {
                                placement.apply();
                            })
                            .await;
                        }
                        let address = this
                            .update_in(cx, |view, window, cx| {
                                native::show_popup(window);
                                view.focus.focus(window, cx);
                                native::address(window)
                            })
                            .ok()
                            .flatten();
                        if let Some(address) = address {
                            cx.background_spawn(async move {
                                native::foreground(address);
                            })
                            .await;
                        }
                        let _ = this.update_in(cx, |view, _, _| view.show_task = None);
                    }));
                }
            });
        })?;
        Ok(())
    }

    pub(in crate::shell) fn hide_popup(&mut self, cx: &mut App) {
        if let Some((handle, popup)) = &self.popup {
            popup.update(cx, |view, _| view.show_task = None);
            let _ = handle.update(cx, |_, window, _| native::hide(window));
        }
    }
}

pub(in crate::shell) mod native {
    pub(in crate::shell::tray) fn verify_registration(
        icon: &tray_icon::TrayIcon,
    ) -> anyhow::Result<()> {
        // 0.26 deliberately returns Ok even if NIM_ADD fails (Explorer might
        // not be ready). NIM_MODIFY in set_tooltip checks shell registration;
        // set_visible alone does not propagate that Win32 failure.
        icon.set_visible(true)?;
        // Clear failures left by ChangeWindowMessageFilterEx / NIM_ADD; a
        // Shell_NotifyIcon failure is not guaranteed to replace LastError.
        unsafe {
            windows_sys::Win32::Foundation::SetLastError(0);
        }
        icon.set_tooltip(Some("Razer")).map_err(|error| {
            log_registration_context(icon.window_handle());
            anyhow::anyhow!("Windows notification-area registration failed: {error}")
        })?;
        Ok(())
    }
    use gpui_kit::Window;
    use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::*};
    pub(in crate::shell::tray) fn notification_icon()
    -> Result<(tray_icon::Icon, u32), tray_icon::BadIcon> {
        // Match the Windows small-icon metric with the ICO's original small
        // raster. Never reduce its 512px preview to 32px and then again in Explorer.
        let requested = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16) as u32;
        let variants: &[(u32, &[u8])] = &[
            (
                16,
                include_bytes!("../../../assets/synapse/tray-icon-16.rgba"),
            ),
            (
                20,
                include_bytes!("../../../assets/synapse/tray-icon-20.rgba"),
            ),
            (
                24,
                include_bytes!("../../../assets/synapse/tray-icon-24.rgba"),
            ),
            (32, include_bytes!("../../../assets/synapse/tray-icon.rgba")),
            (
                40,
                include_bytes!("../../../assets/synapse/tray-icon-40.rgba"),
            ),
            (
                48,
                include_bytes!("../../../assets/synapse/tray-icon-48.rgba"),
            ),
            (
                64,
                include_bytes!("../../../assets/synapse/tray-icon-64.rgba"),
            ),
        ];
        let &(size, bytes) = variants
            .iter()
            .find(|(size, _)| *size >= requested)
            .unwrap_or_else(|| variants.last().expect("tray icon variants"));
        tray_icon::Icon::from_rgba(bytes.to_vec(), size, size).map(|icon| (icon, size))
    }
    pub(in crate::shell) fn log_registration_context(tray: HWND) {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            Security::{
                GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation,
                IsTokenRestricted, TOKEN_ELEVATION, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
                TokenElevation, TokenIntegrityLevel, TokenIsAppContainer,
            },
            System::Threading::{
                GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken,
                PROCESS_QUERY_LIMITED_INFORMATION,
            },
        };
        unsafe fn elevation(process: windows_sys::Win32::Foundation::HANDLE) -> Option<bool> {
            let mut token = std::ptr::null_mut();
            if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
                return None;
            }
            let mut value = TOKEN_ELEVATION::default();
            let mut bytes = 0;
            let ok = unsafe {
                GetTokenInformation(
                    token,
                    TokenElevation,
                    (&mut value as *mut TOKEN_ELEVATION).cast(),
                    std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                    &mut bytes,
                )
            };
            unsafe {
                CloseHandle(token);
            }
            (ok != 0).then_some(value.TokenIsElevated != 0)
        }
        unsafe fn restrictions(
            process: windows_sys::Win32::Foundation::HANDLE,
        ) -> Option<(u32, bool, bool)> {
            let mut token = std::ptr::null_mut();
            if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
                return None;
            }
            // Word-aligned storage for TOKEN_MANDATORY_LABEL and its SID.
            let mut label = [0usize; 64];
            let mut bytes = 0;
            let ok = unsafe {
                GetTokenInformation(
                    token,
                    TokenIntegrityLevel,
                    label.as_mut_ptr().cast(),
                    std::mem::size_of_val(&label) as u32,
                    &mut bytes,
                )
            };
            let mut rid = 0;
            if ok != 0 {
                let sid = unsafe {
                    (*(label.as_ptr().cast::<TOKEN_MANDATORY_LABEL>()))
                        .Label
                        .Sid
                };
                let count = unsafe { *GetSidSubAuthorityCount(sid) };
                if count > 0 {
                    rid = unsafe { *GetSidSubAuthority(sid, u32::from(count - 1)) };
                }
            }
            let mut app_container = 0u32;
            unsafe {
                GetTokenInformation(
                    token,
                    TokenIsAppContainer,
                    (&mut app_container as *mut u32).cast(),
                    4,
                    &mut bytes,
                );
            }
            let restricted = unsafe { IsTokenRestricted(token) != 0 };
            unsafe {
                CloseHandle(token);
            }
            Some((rid, app_container != 0, restricted))
        }
        let class: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
        unsafe {
            let shell = FindWindowW(class.as_ptr(), std::ptr::null());
            let mut shell_pid = 0;
            GetWindowThreadProcessId(shell, &mut shell_pid);
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, shell_pid);
            let shell_elevated = if process.is_null() {
                None
            } else {
                let value = elevation(process);
                CloseHandle(process);
                value
            };
            let shell_process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, shell_pid);
            let shell_restrictions = if shell_process.is_null() {
                None
            } else {
                let value = restrictions(shell_process);
                CloseHandle(shell_process);
                value
            };
            let mut tray_pid = 0;
            GetWindowThreadProcessId(tray, &mut tray_pid);
            eprintln!(
                "[tray] context: valid_window={}, owned_window={}, shell_present={}, app_elevated={:?}, shell_elevated={:?}",
                IsWindow(tray) != 0,
                tray_pid == GetCurrentProcessId(),
                !shell.is_null(),
                elevation(GetCurrentProcess()),
                shell_elevated
            );
            eprintln!(
                "[tray] token (integrity RID, app-container, restricted): app={:?}, shell={:?}",
                restrictions(GetCurrentProcess()),
                shell_restrictions
            );
            use windows_sys::Win32::System::JobObjects::{
                JOBOBJECT_BASIC_UI_RESTRICTIONS, JobObjectBasicUIRestrictions,
                QueryInformationJobObject,
            };
            let mut limits = JOBOBJECT_BASIC_UI_RESTRICTIONS::default();
            let known = QueryInformationJobObject(
                std::ptr::null_mut(),
                JobObjectBasicUIRestrictions,
                (&mut limits as *mut JOBOBJECT_BASIC_UI_RESTRICTIONS).cast(),
                std::mem::size_of_val(&limits) as u32,
                std::ptr::null_mut(),
            ) != 0;
            eprintln!(
                "[tray] enclosing job UI restrictions: {:?}",
                known.then_some(limits.UIRestrictionsClass)
            );
        }
    }
    fn hwnd(window: &Window) -> Option<HWND> {
        let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
        let raw_window_handle::RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return None;
        };
        Some(handle.hwnd.get() as HWND)
    }
    pub(in crate::shell) fn hide(window: &Window) {
        if let Some(hwnd) = hwnd(window) {
            unsafe {
                ShowWindowAsync(hwnd, SW_HIDE);
            }
        }
    }
    pub(in crate::shell) fn show(window: &Window) {
        if let Some(hwnd) = hwnd(window) {
            unsafe {
                ShowWindowAsync(
                    hwnd,
                    if IsIconic(hwnd) != 0 {
                        SW_RESTORE
                    } else {
                        SW_SHOW
                    },
                );
            }
            window.activate_window();
        }
    }
    pub(in crate::shell) fn visible(window: &Window) -> bool {
        hwnd(window).is_some_and(|hwnd| unsafe { IsWindowVisible(hwnd) != 0 })
    }
    pub(in crate::shell) fn show_popup(window: &Window) {
        // Avoid activate_window's initial placement restoring the original 0,0
        // bounds. Queue native visibility; focus activation is handled below.
        if let Some(hwnd) = hwnd(window) {
            unsafe {
                ShowWindowAsync(hwnd, SW_SHOW);
            }
        }
    }
    pub(in crate::shell) fn address(window: &Window) -> Option<usize> {
        hwnd(window).map(|h| h as usize)
    }
    pub(in crate::shell) fn foreground(address: usize) {
        unsafe {
            let hwnd = address as HWND;
            if IsWindowVisible(hwnd) != 0 {
                SetForegroundWindow(hwnd);
            }
        }
    }
    pub(in crate::shell) struct PopupPlacement {
        address: usize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }
    impl PopupPlacement {
        pub(in crate::shell) fn apply(self) {
            // Awaited worker: synchronous WM_SIZE processing cannot reenter a
            // borrowed GPUI entity; visibility is queued only after completion.
            unsafe {
                SetWindowPos(
                    self.address as HWND,
                    HWND_TOPMOST,
                    self.x,
                    self.y,
                    self.width,
                    self.height,
                    SWP_NOACTIVATE,
                );
            }
        }
    }
    /// 位置端口自弹窗应用自己的定位函数
    /// （`.ref/applications/systray/systrayv2/static/js/main.9579c403.js` 的 `c()`）：
    /// `x = trayX - width / 2`、`y = trayY - height`，再按显示器工作区夹取
    /// （原版还会按 dpi 与 `S / dpiScaleY * 0.7` 收缩，这里做同一件事的等价实现）。
    /// 宿主 `LeftSystray.calculateWindowPosition` 只把 `{x,y,trayX,trayY}` 通过
    /// `align` 事件发给页面，真正移动窗口的是这段逻辑。
    pub(in crate::shell) fn popup_placement(
        window: &Window,
        rect: tray_icon::Rect,
    ) -> Option<PopupPlacement> {
        use windows_sys::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromPoint,
        };
        let address = hwnd(window)? as usize;
        let monitor = unsafe {
            MonitorFromPoint(
                windows_sys::Win32::Foundation::POINT { x: 0, y: 0 },
                MONITOR_DEFAULTTOPRIMARY,
            )
        };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
            return None;
        }
        let scale = window.scale_factor();
        let width = (360. * scale).round() as i32;
        let height = (200. * scale).round() as i32;
        let work = info.rcWork;
        let work_width = work.right - work.left;
        let work_height = work.bottom - work.top;
        let tray_x = rect.position.x as i32;
        let tray_y = rect.position.y as i32;
        // `.systray>.header-2` + `.apps` 各 60px；窗口高度由 minimum_height 夹到 200。
        let mut y = tray_y - height;
        if y < work.top {
            y = work.top;
        }
        if y + height > work.bottom {
            y = work.bottom - height;
        }
        let mut x = tray_x - width / 2;
        if x < work.left {
            x = work.left;
        }
        if x + width > work.right {
            x = work.right - width;
        }
        let _ = (work_height, work_width);
        Some(PopupPlacement {
            address,
            x,
            y,
            width,
            height,
        })
    }
    pub(in crate::shell) fn dark_theme() -> bool {
        use windows_sys::Win32::System::Registry::*;
        let key: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize\0"
            .encode_utf16()
            .collect();
        let name: Vec<u16> = "AppsUseLightTheme\0".encode_utf16().collect();
        let mut light = 1u32;
        let mut length = 4;
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                std::ptr::null_mut(),
                (&mut light as *mut u32).cast(),
                &mut length,
            );
        }
        light == 0
    }
}

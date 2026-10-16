//! Dashboard WindowStorage producer, notification consumer and native window.
use super::AppShell;
use gpui_kit::*;
use razer_dashboard::restart_notification::{
    RestartCallers, RestartNotification, RestartNotificationEvent, STORAGE_KEY,
};
use razer_storage::host::{HostStorage, HostStorageCall, HostStorageView, HostStoreKind};
use serde_json::json;

const DASHBOARD_VIEW: u64 = 0xffff_0100;
const NOTIFICATION_VIEW: u64 = 0xffff_0101;

#[derive(Default)]
pub(super) struct State {
    callers: RestartCallers,
    window: Option<(AnyWindowHandle, Subscription)>,
}

fn signal(storage: &mut HostStorage, requested: bool) -> anyhow::Result<Vec<serde_json::Value>> {
    for (id, url) in [
        (DASHBOARD_VIEW, "razer-local:synapse-dashboard"),
        (NOTIFICATION_VIEW, "razer-local:notification-manager"),
    ] {
        storage.register_view(HostStorageView {
            id,
            url: url.into(),
            remote: false,
            destroyed: false,
            crashed: false,
            disposed: false,
            tracked_url: true,
        });
    }
    storage.call(HostStorageCall {
        store: HostStoreKind::Window,
        sender_id: NOTIFICATION_VIEW,
        action: "registerWindowStorageEvent".into(),
        payload: json!({}),
        target_url_array: vec![],
    })?;
    storage.call(HostStorageCall {
        store: HostStoreKind::Window,
        sender_id: DASHBOARD_VIEW,
        action: "setWindowStorageItem".into(),
        payload: json!({"key":STORAGE_KEY,"value":requested.to_string()}),
        target_url_array: vec![],
    })?;
    Ok(storage
        .drain_events(NOTIFICATION_VIEW)?
        .into_iter()
        .filter(|event| event.channel == "windowStorageEvent")
        .map(|event| event.payload)
        .collect())
}

impl AppShell {
    pub(in crate::shell) fn request_restart_notification(
        &mut self,
        parent: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.update_restart_notification(true, parent, cx) {
            // Additional Rust diagnostics stay outside the original window.
            self.status = format!("Restart notification: {error}");
            cx.notify();
        }
    }

    fn update_restart_notification(
        &mut self,
        requested: bool,
        parent: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        for event in signal(&mut self.host_storage, requested)? {
            let Some(show) = self
                .restart_notifications
                .callers
                .consume(&event)
                .map_err(anyhow::Error::msg)?
            else {
                continue;
            };
            let live = self
                .restart_notifications
                .window
                .as_ref()
                .is_some_and(|(handle, _)| cx.windows().contains(handle));
            if !show {
                if let Some((handle, _)) = self.restart_notifications.window.take().filter(|_| live)
                {
                    handle.update(cx, |_, window, _| window.remove_window())?;
                }
                continue;
            }
            // Root.useV only checks existence. It does not focus an existing
            // notification or reopen the underlying device tab.
            if live {
                continue;
            }
            self.restart_notifications.window = None;
            let display = parent.display(cx);
            let display_id = display.as_ref().map(|display| display.id());
            let parent_handle = parent.window_handle();
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(
                        parent.rem_size() * (482. / 16.),
                        parent.rem_size() * (162. / 16.),
                    ),
                    cx,
                ))),
                display_id,
                titlebar: None,
                kind: WindowKind::PopUp,
                is_resizable: false,
                is_minimizable: false,
                is_movable: false,
                // The source shows the browser only after measurement.
                show: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            };
            let (handle, view) = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| RestartNotification::new(window, cx))
            })?;
            let subscription = cx.subscribe(&view, move |this, _, event, cx| {
                let result = match event {
                    RestartNotificationEvent::RestartRequested => {
                        razer_platform::system::restart_workstation()
                    }
                    RestartNotificationEvent::SizeChanged(size) => {
                        let size = *size;
                        handle
                            .update(cx, |_, window, cx| {
                                resize_and_show(window, parent_handle, size, cx)
                            })
                            .and_then(|result| result)
                    }
                };
                if let Err(error) = result {
                    this.status = format!("Restart notification: {error}");
                    cx.notify();
                }
            });
            // The OS window's Root retains the view. This owner only retains
            // the weak event subscription; closing drops the timer and focus.
            self.restart_notifications.window = Some((handle, subscription));
        }
        Ok(())
    }
}

fn resize_and_show(
    window: &mut Window,
    parent: AnyWindowHandle,
    size: Size<Pixels>,
    cx: &mut App,
) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows_sys::Win32::{
            Foundation::RECT,
            Graphics::Gdi::{
                GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
            },
            UI::WindowsAndMessaging::{
                HWND_TOPMOST, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SetWindowPos, ShowWindow,
            },
        };
        let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window)
            .map_err(|error| anyhow::anyhow!("{error}"))?
            .as_raw()
        else {
            anyhow::bail!("restart notification requires its Win32 window");
        };
        let hwnd = handle.hwnd.get() as _;
        let parent_hwnd = parent
            .update(cx, |_, parent, _| {
                HasWindowHandle::window_handle(parent)
                    .ok()
                    .and_then(|handle| match handle.as_raw() {
                        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                        _ => None,
                    })
            })
            .ok()
            .flatten()
            .map(|handle| handle as _)
            .unwrap_or(hwnd);
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT::default(),
            rcWork: RECT::default(),
            dwFlags: 0,
        };
        // The current source centers against full monitor.bounds, not workArea.
        let monitor_handle = unsafe { MonitorFromWindow(parent_hwnd, MONITOR_DEFAULTTONEAREST) };
        anyhow::ensure!(
            unsafe { GetMonitorInfoW(monitor_handle, &mut monitor) } != 0,
            "restart notification monitor lookup failed: {}",
            std::io::Error::last_os_error()
        );
        let scale = window.scale_factor();
        let width = (f32::from(size.width) * scale).round() as i32;
        let height = (f32::from(size.height) * scale).round() as i32;
        let x =
            monitor.rcMonitor.left + (monitor.rcMonitor.right - monitor.rcMonitor.left - width) / 2;
        let y =
            monitor.rcMonitor.top + (monitor.rcMonitor.bottom - monitor.rcMonitor.top - height) / 2;
        anyhow::ensure!(
            unsafe { SetWindowPos(hwnd, HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE) } != 0,
            "restart notification bounds update failed: {}",
            std::io::Error::last_os_error()
        );
        unsafe {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (parent, cx);
        window.resize(size);
        window.activate_window();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{NOTIFICATION_VIEW, signal};
    use razer_dashboard::restart_notification::RestartCallers;
    use razer_storage::host::HostStorage;

    #[test]
    fn dashboard_requests_cross_actual_local_storage_and_repeat_after_dismissal() {
        let mut storage = HostStorage::default();
        let mut callers = RestartCallers::default();
        let events = signal(&mut storage, true).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["keyState"], "created");
        assert_eq!(callers.consume(&events[0]).unwrap(), Some(true));
        let events = signal(&mut storage, true).unwrap();
        assert_eq!(events[0]["keyState"], "changed");
        assert_eq!(callers.consume(&events[0]).unwrap(), Some(true));
        assert!(storage.drain_events(NOTIFICATION_VIEW).unwrap().is_empty());
        let events = signal(&mut storage, false).unwrap();
        assert_eq!(callers.consume(&events[0]).unwrap(), Some(false));
    }
}

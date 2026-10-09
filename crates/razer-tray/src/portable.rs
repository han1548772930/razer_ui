//! GPUI popup lifetime on macOS/Linux. Unlike HWND hiding, dismissal removes
//! the auxiliary window; the common tray registration stays alive.
use super::{DesktopTray, Dismiss, TrayPopup};
use gpui_kit::*;

impl DesktopTray {
    pub fn show_popup(&mut self, toggle: bool, cx: &mut App) -> anyhow::Result<()> {
        if self.popup.is_some() {
            if toggle {
                self.hide_popup(cx);
            } else if let Some((handle, _)) = &self.popup {
                handle.update(cx, |_, window, _| window.activate_window())?;
            }
            return Ok(());
        }
        cx.bind_keys([KeyBinding::new("escape", Dismiss, Some("TrayPopup"))]);
        let sender = self.sender.clone();
        // KSNI does not expose panel-icon bounds. Center the auxiliary window
        // using GPUI display geometry instead of assuming a Windows taskbar.
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(360.), px(400.)), cx)),
            window_min_size: Some(size(px(360.), px(60.))),
            window_background: WindowBackgroundAppearance::Transparent,
            titlebar: None,
            show: true,
            focus: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            ..Default::default()
        };
        self.popup = Some(gpui_kit::open_window(options, cx, |window, cx| {
            let popup =
                cx.new(|cx| TrayPopup::new(sender, window, cx, self.widget_devices.clone()));
            popup.read(cx).focus.focus(window, cx);
            popup
        })?);
        Ok(())
    }

    pub fn hide_popup(&mut self, cx: &mut App) {
        if let Some((handle, _)) = self.popup.take() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }
}

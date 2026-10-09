//! Host policy for the independent settings view.
use super::{AppShell, display_window};
use gpui_kit::{component::*, *};
use razer_settings::settings_window::SettingsWindow;
impl AppShell {
    pub(super) fn open_settings_window(&mut self, quick_panel: bool, cx: &mut Context<Self>) {
        let settings = self.settings.clone();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1280.), px(720.)), cx)),
            // Current tray 2554/he passes these overrides only from its gear.
            // open_or_focus ignores options when a named window already exists.
            // main.js resolves M.minimumWidth/Height from WINDOW_SIZE_DEFAULTS.
            // Only the tray gear overrides those values through the SDK options.
            window_min_size: Some(if quick_panel {
                size(px(1000.), px(768.))
            } else {
                size(px(600.), px(500.))
            }),
            ..TitleBar::window_options()
        };
        if let Err(error) = display_window::open_or_focus(
            cx,
            "razer-settings".into(),
            display_window::WindowPolicy::Different,
            options,
            move |window, cx| cx.new(|cx| SettingsWindow::new(settings, window, cx)),
        ) {
            self.status = error.to_string();
            cx.notify();
        }
    }
}

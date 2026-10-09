//! Host subscriptions and routing for the independent tray package.
use gpui_kit::*;
use razer_tray::{DesktopTray, Event, show_main};
use std::time::Duration;
use tray_icon::TrayIconEvent;
impl crate::shell::AppShell {
    pub(in crate::shell) fn install_tray(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.push(cx.on_app_quit(|this, _| {
            this.tray_startup = None;
            this.tray_click = None;
            this.tray_events = None;
            this.tray = None;
            async {}
        }));
        // Start after the GPUI window/entity and native message loop are ready.
        // Retry only failed registrations; never recreate an existing icon.
        self.tray_startup = Some(cx.spawn_in(window, async move |this, cx| {
            for (attempt, delay) in [0, 250, 1000, 3000].into_iter().enumerate() {
                cx.background_executor()
                    .timer(Duration::from_millis(delay))
                    .await;
                let result = this.update_in(cx, |this, window, cx| {
                    this.try_install_tray(attempt + 1, window, cx)
                });
                match result {
                    Ok(true) | Err(_) => break,
                    Ok(false) => {}
                }
            }
            let _ = this.update_in(cx, |this, _, _| this.tray_startup = None);
        }));
    }

    fn try_install_tray(
        &mut self,
        attempt: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.tray.is_some() {
            return true;
        }
        let (tray, receiver) = match DesktopTray::new(cx, self.tray_widgets(cx)) {
            Ok(value) => value,
            Err(error) => {
                self.status = format!("托盘初始化失败：{error}");
                eprintln!("[tray] initialization attempt {attempt}/4 failed: {error:#}");
                cx.notify();
                return false;
            }
        };
        eprintln!(
            "[tray] registered; notification-area rect: {:?}",
            tray.icon.rect()
        );
        if self.status.starts_with("托盘初始化失败：") {
            self.status = "本地配置预览 · 尚未写入硬件".into();
        }
        self.tray = Some(tray);
        self.tray_events = Some(cx.spawn_in(window, async move |this, cx| {
            while let Ok(event) = receiver.recv().await {
                if this
                    .update_in(cx, |this, window, cx| {
                        this.handle_tray_event(event, window, cx)
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        cx.notify();
        true
    }

    fn handle_tray_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        use tray_icon::{MouseButton, MouseButtonState};
        match event {
            Event::Icon(TrayIconEvent::Click { .. } | TrayIconEvent::DoubleClick { .. })
                if cfg!(target_os = "macos") => {}
            Event::Icon(TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            }) => {
                self.tray_click = None;
                self.tray_ignore_release = true;
                match self.settings.read(cx).tray_double_click() {
                    razer_state::TrayDoubleClickAction::ShowMenu => {
                        if let Some(tray) = &mut self.tray {
                            tray.cancel_blur(cx);
                            if let Err(error) = tray.show_popup(false, cx) {
                                self.status = error.to_string();
                            }
                        }
                    }
                }
            }
            Event::Icon(TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }) => {
                if std::mem::take(&mut self.tray_ignore_release) {
                    return;
                }
                if let Some(tray) = &self.tray {
                    tray.cancel_blur(cx);
                }
                self.tray_click = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(200))
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(tray) = &mut this.tray {
                            // Host forwards `click`; renderer Je reads
                            // getWindowStatus and toggles browser visibility.
                            if let Err(error) = tray.show_popup(true, cx) {
                                this.status = error.to_string();
                            }
                        }
                    });
                }));
            }
            Event::Icon(TrayIconEvent::Enter { .. }) => {
                if let Some(tray) = &mut self.tray {
                    tray.refresh(cx);
                }
            }
            Event::Dismiss => {
                if let Some(tray) = &mut self.tray {
                    tray.hide_popup(cx);
                }
            }
            Event::Device(key) => {
                if let Some(tray) = &mut self.tray {
                    tray.hide_popup(cx);
                }
                if self
                    .devices
                    .iter()
                    .any(|workspace| workspace.read(cx).identity(cx) == key)
                {
                    show_main(window);
                    self.navigate(super::Location::Device(key), window, cx);
                }
            }
            Event::Menu(command) => {
                self.tray_click = None;
                if let Some(tray) = &mut self.tray {
                    tray.hide_popup(cx);
                }
                match command.as_str() {
                    "synapse" => show_main(window),
                    "settings"
                    | "settings-quick-panel"
                    | "settings-widgets"
                    | "settings-notifications" => {
                        // Current /settings/ has no legacy section listener.
                        self.open_settings_window(command == "settings-quick-panel", cx);
                    }
                    "account-online" => {
                        show_main(window);
                        self.status = "账户服务尚未连接，无法打开在线账户。".into();
                        cx.notify();
                    }
                    // Guest's source command is logOut(), which starts the
                    // account flow. No provider is available to perform it.
                    "login" | "account-guest-logout" => {
                        show_main(window);
                        self.status = "账户登录服务尚未连接。".into();
                        cx.notify();
                    }
                    "exit" => self.request_exit(window, cx),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

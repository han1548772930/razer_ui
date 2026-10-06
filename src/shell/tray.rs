//! Platform-independent systray presentation, translations, and UI commands.
//! OS registration and native window operations belong to platform adapters.
use crate::{
    i18n,
    ui::{surface, theme::TrayColors},
};
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;
use std::{sync::OnceLock, time::Duration};
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub(super) use windows::native;
#[cfg(not(target_os = "windows"))]
mod portable;
use tray_icon::{
    TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{IconMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

mod account;

actions!(tray_popup, [Dismiss]);

pub(super) enum Event {
    Icon(tray_icon::TrayIconEvent),
    Dismiss,
    Menu(String),
}

fn text(group: &str, key: &str) -> String {
    static STRINGS: OnceLock<Value> = OnceLock::new();
    let data = STRINGS.get_or_init(|| {
        serde_json::from_str(include_str!("tray_strings.json"))
            .expect("validated tray translations")
    });
    let locale = i18n::locale().to_ascii_lowercase();
    data[group]
        .get(&locale)
        .and_then(|v| v.get(key))
        .or_else(|| data[group]["en"].get(key))
        .and_then(Value::as_str)
        .unwrap_or(key)
        .to_owned()
}

// Platform adapters materialize these semantic items as native menus.
// This process owns one application; Exit All requires a real multi-app registry.
enum MenuIcon {
    Application,
    Settings,
    Account,
}
enum MenuEntry {
    Action {
        id: &'static str,
        label: String,
        icon: Option<MenuIcon>,
    },
    Separator,
}
fn menu_entries() -> [MenuEntry; 6] {
    [
        MenuEntry::Action {
            id: "synapse",
            label: text("host", "RAZER_SYNAPSE"),
            icon: Some(MenuIcon::Application),
        },
        MenuEntry::Separator,
        MenuEntry::Action {
            id: "settings",
            label: text("host", "settings"),
            icon: Some(MenuIcon::Settings),
        },
        MenuEntry::Action {
            id: "login",
            label: text("host", "log_in"),
            icon: Some(MenuIcon::Account),
        },
        MenuEntry::Separator,
        MenuEntry::Action {
            id: "exit",
            label: text("host", "exit"),
            icon: None,
        },
    ]
}

struct TrayPopup {
    sender: async_channel::Sender<Event>,
    focus: FocusHandle,
    blur_task: Option<Task<()>>,
    show_task: Option<Task<()>>,
    _activation: Subscription,
    login_hovered: bool,
    launcher_hovered: bool,
    launcher_pressed: bool,
    // The host renderer has three account branches: no user id, a guest
    // session, and an authenticated user.  Keep the session branch explicit
    // so a future host-session event can mount the guest/notification surface
    // without treating local workspace data as an account response.
    session: TraySession,
    section: TraySection,
    account_hovered: bool,
    account_pressed: bool,
    account_name: String,
    account_avatar: Option<SharedString>,
    hovered_tab: Option<TraySection>,
    settings_hovered: bool,
    notifications_loaded_empty: bool,
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum TraySession {
    SignedOut,
    Guest,
    Authenticated,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TraySection {
    Widgets,
    Notifications,
}
impl TrayPopup {
    fn new(
        sender: async_channel::Sender<Event>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            this.blur_task = None;
            if !window.is_window_active() {
                this.blur_task = Some(cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
                    let _ = this.update_in(cx, |view, window, _| {
                        if !window.is_window_active() {
                            let _ = view.sender.try_send(Event::Dismiss);
                        }
                    });
                }));
            }
        });
        Self {
            sender,
            focus: cx.focus_handle(),
            blur_task: None,
            show_task: None,
            _activation: activation,
            login_hovered: false,
            launcher_hovered: false,
            launcher_pressed: false,
            // There is no host user session in the local shell.  This keeps
            // the audited `!user.item.id` branch as the visible default.
            session: TraySession::SignedOut,
            section: TraySection::Widgets,
            account_hovered: false,
            account_pressed: false,
            account_name: String::new(),
            account_avatar: None,
            hovered_tab: None,
            settings_hovered: false,
            notifications_loaded_empty: false,
        }
    }

    /// Mount a host-provided session branch once the real account channel is
    /// available.  Until then the signed-out branch remains visible and no
    /// local fixture is presented as an account.
    #[allow(dead_code)]
    fn set_session(&mut self, session: TraySession) {
        self.session = session;
        self.notifications_loaded_empty = false;
        self.section = TraySection::Widgets;
    }

    #[allow(dead_code)]
    fn set_section(&mut self, section: TraySection) {
        self.section = section;
    }

    fn command(&self, id: &str) {
        let _ = self.sender.try_send(Event::Menu(id.to_owned()));
    }
}
impl Render for TrayPopup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // systrayv2 554 CSS: header bg/text, launcher bg/title are .1s
        // ease-in-out; the pressed launch icon fades with .1s linear.
        let transition = || Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut);
        let login_bg = motion::transition(
            "tray-login-background",
            if self.login_hovered {
                TrayColors::border()
            } else {
                TrayColors::surface()
            },
            transition(),
            window,
            cx,
        );
        let login_text = motion::transition(
            "tray-login-foreground",
            if self.login_hovered {
                TrayColors::hover_text()
            } else {
                TrayColors::text()
            },
            transition(),
            window,
            cx,
        );
        let launcher_bg = motion::transition(
            "tray-launcher-background",
            if self.launcher_hovered {
                TrayColors::border()
            } else {
                TrayColors::launcher()
            },
            transition(),
            window,
            cx,
        );
        let launcher_text = motion::transition(
            "tray-launcher-foreground",
            if self.launcher_hovered {
                TrayColors::hover_text()
            } else {
                TrayColors::muted()
            },
            transition(),
            window,
            cx,
        );
        let icon_opacity = motion::transition(
            "tray-launcher-icon-opacity",
            if self.launcher_pressed { 0.3_f32 } else { 1. },
            Transition::new(Duration::from_millis(100)).easing(Easing::Linear),
            window,
            cx,
        );
        // Re: no user.item.id => header-2 + launchers, without navbar/widgets.
        // Guest/notification content is mounted only after a real host
        // session event calls `set_session`; local workspace data is never
        // used as an account response.
        let signed_out = self.session == TraySession::SignedOut;
        let account_surface = if signed_out {
            gpui_kit::base::Button::new("tray-login")
                .accessibility_label(text("popup", "TEXT_LOG_IN_TO_GET_STARTED"))
                .w_full()
                .h(surface::css(60.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .px(surface::css(20.))
                .pb(surface::css(1.))
                .bg(login_bg)
                .text_color(login_text)
                .on_hover(cx.listener(|this, hovered, _, cx| {
                    this.login_hovered = *hovered;
                    cx.notify();
                }))
                .focus_visible(|s| s.border_1().border_color(cx.theme().primary))
                .child(text("popup", "TEXT_LOG_IN_TO_GET_STARTED"))
                .on_click(cx.listener(|this, _, _, _| this.command("login")))
                .into_any_element()
        } else {
            self.account_header(window, cx)
        };

        v_flex()
            .id("source-tray-popup")
            .key_context("TrayPopup")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Dismiss, _, _| {
                let _ = this.sender.try_send(Event::Dismiss);
            }))
            .w_full()
            .h_full()
            .bg(TrayColors::surface())
            .border_1()
            .border_color(TrayColors::border())
            .text_color(TrayColors::text())
            .font_family("Roboto")
            .text_size(surface::css(16.))
            .line_height(relative(1.22))
            .child(account_surface)
            .when(!signed_out, |root| {
                root.child(self.account_navigation(window, cx))
                    .child(self.account_body(window, cx))
            })
            .child(
                gpui_kit::base::Button::new("tray-launch-synapse")
                    .group("tray-launcher")
                    .accessibility_label(text("host", "RAZER_SYNAPSE"))
                    .w_full()
                    .h(surface::css(60.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(surface::css(10.))
                    .border_t_1()
                    .border_color(TrayColors::surface())
                    .bg(launcher_bg)
                    .text_size(surface::css(12.))
                    .text_color(launcher_text)
                    .gap(surface::css(10.))
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        this.launcher_hovered = *hovered;
                        if !hovered {
                            this.launcher_pressed = false;
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.launcher_pressed = true;
                            cx.notify();
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.launcher_pressed = false;
                            cx.notify();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.launcher_pressed = false;
                            cx.notify();
                        }),
                    )
                    .focus_visible(|s| s.border_1().border_color(cx.theme().primary))
                    .child(
                        img("synapse/tray-synapse.svg")
                            .size(surface::css(32.))
                            .opacity(icon_opacity),
                    )
                    .child(text("host", "RAZER_SYNAPSE"))
                    .on_click(cx.listener(|this, _, _, _| this.command("synapse"))),
            )
    }
}

pub(in crate::shell) struct DesktopTray {
    icon: TrayIcon,
    icon_size: u32,
    popup: Option<(AnyWindowHandle, Entity<TrayPopup>)>,
    sender: async_channel::Sender<Event>,
}

impl DesktopTray {
    pub(in crate::shell) fn new(
        cx: &App,
    ) -> anyhow::Result<(Self, async_channel::Receiver<Event>)> {
        let (sender, receiver) = async_channel::unbounded();
        let (icon, icon_size) = notification_icon()?;
        let icon = TrayIconBuilder::new()
            .with_id("razer-ui-tray")
            .with_tooltip("Razer")
            .with_icon(icon)
            // Current host skips LeftSystray on macOS; use its native menu.
            .with_menu_on_left_click(cfg!(target_os = "macos"))
            .with_menu(Box::new(native_menu(cx)?))
            .build()?;
        #[cfg(target_os = "windows")]
        native::verify_registration(&icon)?;
        let events = sender.clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            let _ = events.try_send(Event::Icon(event));
        }));
        let events = sender.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = events.try_send(Event::Menu(event.id.0));
        }));
        Ok((
            Self {
                icon,
                icon_size,
                popup: None,
                sender,
            },
            receiver,
        ))
    }

    pub(in crate::shell) fn refresh(&mut self, cx: &mut App) {
        if let Ok((icon, size)) = notification_icon() {
            if size != self.icon_size && self.icon.set_icon(Some(icon)).is_ok() {
                self.icon_size = size;
            }
        }
        if let Ok(menu) = native_menu(cx) {
            self.icon.set_menu(Some(Box::new(menu)));
        }
        if let Some((_, popup)) = &self.popup {
            popup.update(cx, |_, cx| cx.notify());
        }
    }

    pub(in crate::shell) fn cancel_blur(&self, cx: &mut App) {
        if let Some((_, popup)) = &self.popup {
            popup.update(cx, |view, _| view.blur_task = None);
        }
    }
}

impl Drop for DesktopTray {
    fn drop(&mut self) {
        // These process-wide handlers belong to the single app tray instance.
        TrayIconEvent::set_event_handler(None::<fn(TrayIconEvent)>);
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
        self.sender.close();
    }
}

fn native_menu(cx: &App) -> anyhow::Result<Menu> {
    let menu = Menu::new();
    let dark = menu_is_dark(cx);
    for entry in menu_entries() {
        match entry {
            MenuEntry::Separator => menu.append(&PredefinedMenuItem::separator())?,
            MenuEntry::Action { id, label, icon } => {
                if let Some(icon) = icon {
                    let bytes: &[u8] = match icon {
                        MenuIcon::Application => {
                            include_bytes!("../../assets/synapse/tray-app.rgba")
                        }
                        MenuIcon::Settings if dark => {
                            include_bytes!("../../assets/synapse/tray-gear-dark.rgba")
                        }
                        MenuIcon::Settings => {
                            include_bytes!("../../assets/synapse/tray-gear-light.rgba")
                        }
                        MenuIcon::Account if dark => {
                            include_bytes!("../../assets/synapse/tray-user-dark.rgba")
                        }
                        MenuIcon::Account => {
                            include_bytes!("../../assets/synapse/tray-user-light.rgba")
                        }
                    };
                    let image = tray_icon::menu::Icon::from_rgba(bytes.to_vec(), 20, 20)?;
                    menu.append(&IconMenuItem::with_id(id, label, true, Some(image), None))?;
                } else {
                    menu.append(&MenuItem::with_id(id, label, true, None))?;
                }
            }
        }
    }
    Ok(menu)
}

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
        let (tray, receiver) = match DesktopTray::new(cx) {
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
                if let Some(tray) = &mut self.tray {
                    tray.cancel_blur(cx);
                    if let Err(error) = tray.show_popup(false, cx) {
                        self.status = error.to_string();
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
                            // 原版左键：不可见时先对齐，再转发 `{action:"click"}` 并聚焦，
                            // 不会因为再次点击而隐藏（隐藏由失焦 300ms 负责）。
                            if let Err(error) = tray.show_popup(false, cx) {
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
            Event::Menu(command) => {
                self.tray_click = None;
                if let Some(tray) = &mut self.tray {
                    tray.hide_popup(cx);
                }
                match command.as_str() {
                    "synapse" => show_main(window),
                    "settings" => {
                        show_main(window);
                        self.navigate(
                            crate::shell::Location::Main(crate::nav::Tab::Setting),
                            window,
                            cx,
                        );
                    }
                    "settings-widgets" | "settings-notifications" => {
                        // The local Settings page is available directly. Its
                        // notification toggle remains local-only until the
                        // host service is connected.
                        show_main(window);
                        self.navigate(
                            crate::shell::Location::Main(crate::nav::Tab::Setting),
                            window,
                            cx,
                        );
                    }
                    "account-online" => {
                        show_main(window);
                        self.status = "账户服务尚未连接，无法打开在线账户。".into();
                        cx.notify();
                    }
                    "login" => {
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

fn menu_is_dark(_cx: &App) -> bool {
    #[cfg(target_os = "windows")]
    {
        native::dark_theme()
    }
    #[cfg(not(target_os = "windows"))]
    {
        matches!(
            _cx.window_appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        )
    }
}

fn show_main(window: &mut Window) {
    #[cfg(target_os = "windows")]
    native::show(window);
    #[cfg(not(target_os = "windows"))]
    window.activate_window();
}

fn notification_icon() -> Result<(tray_icon::Icon, u32), tray_icon::BadIcon> {
    #[cfg(target_os = "windows")]
    {
        native::notification_icon()
    }
    #[cfg(not(target_os = "windows"))]
    {
        tray_icon::Icon::from_rgba(
            include_bytes!("../../assets/synapse/tray-native.rgba").to_vec(),
            16,
            16,
        )
        .map(|icon| (icon, 16))
    }
}

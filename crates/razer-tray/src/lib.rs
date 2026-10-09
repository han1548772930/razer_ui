//! Platform-independent systray presentation, translations, and UI commands.
//! OS registration and native window operations belong to platform adapters.
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use razer_i18n as i18n;
use razer_widgets::surface;
use razer_widgets::theme::TrayColors;
use serde_json::Value;
use std::{sync::OnceLock, time::Duration};
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::native;
#[cfg(not(target_os = "windows"))]
mod portable;
use tray_icon::{
    TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{IconMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

mod account;
mod launcher;
mod widgets;
use launcher::{TrayLauncher, launcher_list, version_badge};
pub use widgets::category_icon as widget_category_icon;
pub use widgets::{TrayWidgetBattery, TrayWidgetDevice, TrayWidgetProfile};

actions!(tray_popup, [Dismiss]);

pub enum Event {
    Icon(tray_icon::TrayIconEvent),
    Dismiss,
    Menu(String),
    Device(String),
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
    resize_task: Option<Task<()>>,
    widget_height: Option<f32>,
    widget_revision: u64,
    #[cfg(target_os = "windows")]
    anchor: Option<(i32, i32)>,
    #[cfg(target_os = "windows")]
    placed_height: Option<f32>,
    _activation: Subscription,
    login_hovered: bool,
    // All three source account surfaces exist. A real host-session publisher
    // must select the branch; local workspace data is not an account response.
    session: TraySession,
    section: TraySection,
    account_hovered: bool,
    account_pressed: bool,
    account_name: String,
    account_avatar: Option<SharedString>,
    hovered_tab: Option<TraySection>,
    settings_hovered: bool,
    settings_tip_mounted: bool,
    settings_tip_visible: bool,
    settings_tip_task: Option<Task<()>>,
    notifications_loaded_empty: bool,
    widget_devices: Vec<TrayWidgetDevice>,
    launchers: Vec<TrayLauncher>,
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
        widget_devices: Vec<TrayWidgetDevice>,
    ) -> Self {
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            this.blur_task = None;
            if !window.is_window_active() {
                this.hover_settings(false, cx);
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
            resize_task: None,
            widget_height: None,
            widget_revision: 0,
            #[cfg(target_os = "windows")]
            anchor: None,
            #[cfg(target_os = "windows")]
            placed_height: None,
            _activation: activation,
            login_hovered: false,
            // Present the source's Guest surface for the local account-less
            // shell. This is a presentation choice, not an observed Razer
            // session: source Guest itself requires a real user.item.id.
            session: TraySession::Guest,
            section: TraySection::Widgets,
            account_hovered: false,
            account_pressed: false,
            account_name: String::new(),
            account_avatar: None,
            hovered_tab: None,
            settings_hovered: false,
            settings_tip_mounted: false,
            settings_tip_visible: false,
            settings_tip_task: None,
            notifications_loaded_empty: false,
            widget_devices,
            // This running process supplies one real local activation target.
            // It does not imply the official installedModules/apps registry
            // or the user's launcher preferences have been read.
            launchers: vec![TrayLauncher {
                name: "synapse".into(),
                title: text("host", "RAZER_SYNAPSE"),
                logo: "synapse/tray-synapse.svg".into(),
            }],
        }
    }

    /// Mount a host-provided session branch once the real account channel is
    /// available. The local Guest presentation is not an authenticated account.
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

    pub fn set_widget_devices(&mut self, devices: Vec<TrayWidgetDevice>) {
        self.widget_devices = devices;
        self.widget_revision = self.widget_revision.wrapping_add(1);
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
        // Re: no user.item.id => header-2 + launchers, without navbar/widgets.
        // The local Guest presentation uses the same account DOM branch;
        // local workspace data is never used as a Razer account response.
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
                .line_height(relative(1.22))
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

        // `.systray` is a fixed-width block, not a flex column. Its content
        // may exceed the window; `body { overflow:hidden }` clips the viewport.
        let panel = div()
            .id("source-tray-popup")
            .key_context("TrayPopup")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Dismiss, _, _| {
                let _ = this.sender.try_send(Event::Dismiss);
            }))
            .w(surface::css(360.))
            // Source sets min-height:100%, not height:100%. The root must
            // grow around its header/list; the browser viewport clips it.
            .min_h_full()
            .max_h(surface::css(700.))
            .cursor(CursorStyle::default())
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
            .when(!self.launchers.is_empty(), |panel| {
                let sender = self.sender.clone();
                panel.child(launcher_list(
                    &self.launchers,
                    move |launcher, _, _| {
                        let _ = sender.try_send(Event::Menu(launcher.name.clone()));
                    },
                    window,
                    cx,
                ))
            });
        div()
            .id("tray-viewport")
            .relative()
            .size_full()
            .overflow_hidden()
            .child(panel)
            .child(version_badge("1.1.97"))
    }
}

pub struct DesktopTray {
    pub icon: TrayIcon,
    icon_size: u32,
    popup: Option<(AnyWindowHandle, Entity<TrayPopup>)>,
    sender: async_channel::Sender<Event>,
    widget_devices: Vec<TrayWidgetDevice>,
}

impl DesktopTray {
    pub fn new(
        cx: &App,
        widget_devices: Vec<TrayWidgetDevice>,
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
                widget_devices,
            },
            receiver,
        ))
    }

    pub fn refresh(&mut self, cx: &mut App) {
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

    pub fn set_widget_devices(&mut self, devices: Vec<TrayWidgetDevice>, cx: &mut App) {
        self.widget_devices = devices.clone();
        if let Some((_, popup)) = &self.popup {
            popup.update(cx, |view, cx| {
                view.set_widget_devices(devices);
                cx.notify();
            });
        }
    }

    pub fn cancel_blur(&self, cx: &mut App) {
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
                    // getCachedThemeIcon preserves the source PNG dimensions;
                    // only getCachedProductIcon requests a 20x20 application icon.
                    let (bytes, width, height): (&[u8], u32, u32) = match icon {
                        MenuIcon::Application => (
                            include_bytes!("../../../assets/synapse/tray-app.rgba"),
                            20,
                            20,
                        ),
                        MenuIcon::Settings if dark => (
                            include_bytes!("../../../assets/synapse/tray-gear-dark.rgba"),
                            14,
                            14,
                        ),
                        MenuIcon::Settings => (
                            include_bytes!("../../../assets/synapse/tray-gear-light.rgba"),
                            14,
                            14,
                        ),
                        MenuIcon::Account if dark => (
                            include_bytes!("../../../assets/synapse/tray-user-dark.rgba"),
                            14,
                            14,
                        ),
                        MenuIcon::Account => (
                            include_bytes!("../../../assets/synapse/tray-user-light.rgba"),
                            12,
                            16,
                        ),
                    };
                    let image = tray_icon::menu::Icon::from_rgba(bytes.to_vec(), width, height)?;
                    menu.append(&IconMenuItem::with_id(id, label, true, Some(image), None))?;
                } else {
                    menu.append(&MenuItem::with_id(id, label, true, None))?;
                }
            }
        }
    }
    Ok(menu)
}

fn menu_is_dark(_cx: &App) -> bool {
    // The host forces Chromium Views dark. The retained HMENU is a permitted
    // platform presentation difference: choose readable source icons for the
    // actual native menu surface, not Windows' unrelated AppsUseLightTheme.
    #[cfg(target_os = "windows")]
    {
        native::menu_is_dark()
    }
    #[cfg(not(target_os = "windows"))]
    {
        matches!(
            _cx.window_appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        )
    }
}

pub fn show_main(window: &mut Window) {
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
            include_bytes!("../../../assets/synapse/tray-native.rgba").to_vec(),
            16,
            16,
        )
        .map(|icon| (icon, 16))
    }
}

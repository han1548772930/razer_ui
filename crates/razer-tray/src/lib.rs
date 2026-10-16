//! Platform-independent systray presentation, translations, and UI commands.
//! OS registration and native window operations belong to platform adapters.
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use razer_i18n as i18n;
use razer_widgets::surface;
use razer_widgets::theme::TrayColors;
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};
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

mod launcher;
mod local_panel;
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

type Translations = BTreeMap<String, BTreeMap<String, String>>;
struct TrayStrings {
    host: Translations,
    popup: Translations,
}

fn text(group: &str, key: &str) -> String {
    static STRINGS: OnceLock<TrayStrings> = OnceLock::new();
    let data = STRINGS.get_or_init(|| read_tray_strings_catalog());
    let translations = match group {
        "host" => &data.host,
        "popup" => &data.popup,
        _ => return key.to_owned(),
    };
    let locale = i18n::locale().to_ascii_lowercase();
    translations
        .get(&locale)
        .and_then(|v| v.get(key))
        .or_else(|| translations.get("en").and_then(|strings| strings.get(key)))
        .map(String::as_str)
        .unwrap_or(key)
        .to_owned()
}

// Platform adapters materialize these semantic items as native menus.
// This process owns one application; Exit All requires a real multi-app registry.
enum MenuIcon {
    Application,
    Settings,
}
enum MenuEntry {
    Action {
        id: &'static str,
        label: String,
        icon: Option<MenuIcon>,
    },
    Separator,
}
fn menu_entries() -> [MenuEntry; 5] {
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
    settings_hovered: bool,
    settings_tip_mounted: bool,
    settings_tip_visible: bool,
    settings_tip_task: Option<Task<()>>,
    widget_devices: Vec<TrayWidgetDevice>,
    launchers: Vec<TrayLauncher>,
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
            settings_hovered: false,
            settings_tip_mounted: false,
            settings_tip_visible: false,
            settings_tip_task: None,
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
            .max_h(surface::css(640.))
            .cursor(CursorStyle::default())
            .bg(TrayColors::surface())
            .border_1()
            .border_color(TrayColors::border())
            .text_color(TrayColors::text())
            .font_family("Roboto")
            .text_size(surface::css(16.))
            .line_height(relative(1.22))
            .child(self.local_navigation(window, cx))
            .child(self.local_body(window, cx))
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
        let content = div()
            .id("tray-viewport")
            .relative()
            .size_full()
            .overflow_hidden()
            .child(panel)
            .child(version_badge("1.1.97"));
        image_cache(razer_assets::resource_image_cache())
            .size_full()
            .child(content)
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
                    let icon = match icon {
                        MenuIcon::Application => razer_assets::NativeIcon::Application,
                        MenuIcon::Settings if dark => razer_assets::NativeIcon::SettingsDark,
                        MenuIcon::Settings => razer_assets::NativeIcon::SettingsLight,
                    };
                    let (bytes, width, height) = (icon.rgba(), icon.pixels(), icon.pixels());
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
            razer_assets::NativeIcon::Notification16.rgba().to_vec(),
            16,
            16,
        )
        .map(|icon| (icon, 16))
    }
}

// Generated catalog reader: razer-tray/tray_strings.
fn read_tray_strings_catalog() -> TrayStrings {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<TrayStrings> {
        let value = match r.tag()? {
            0 => TrayStrings {
                host: r.child(decode_1)?,
                popup: r.child(decode_1)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_2, decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_2, decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_TRAY_TRAY_STRINGS,
        0,
        "09c63385b79313ce732472c02dc021ca5007d56754da21d24dbfba62ee4e0161",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_tray_strings_decodes() {
    let _ = read_tray_strings_catalog();
}
// End generated catalog reader: razer-tray/tray_strings.

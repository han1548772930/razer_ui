//! Independent `policy=5` Chroma application window.
//!
//! The Dashboard contract names this window `chroma-app` and keeps it outside
//! the Synapse host tab strip. Dashboard and Studio are separate roots in
//! the current host's Chroma subtab manager.
use super::{
    AppShell, Location,
    chroma_page::{ChromaPage, ChromaPageEvent},
    chroma_studio_window::StudioSession,
    display_window::{CHROMA_APP_WINDOW_ICON_PATH, DisplayMode},
    host_tabs,
};
use crate::features::ProductWorkspace;
use crate::ui::{surface::css, theme::HostColors};
use gpui_kit::{component::*, *};
#[path = "chroma_settings.rs"]
mod settings;
use settings::{ChromaSettings, SettingsAction};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChromaTab {
    Dashboard,
    Studio,
    Settings,
    Migration,
}
impl ChromaTab {
    fn id(self) -> &'static str {
        match self {
            Self::Dashboard => "chroma-app",
            Self::Studio => "chroma-studio",
            Self::Settings => "settings-chroma",
            Self::Migration => "chroma-app-syn3-profile-migration",
        }
    }
    fn title(self) -> String {
        match self {
            Self::Dashboard => "CHROMA".into(),
            Self::Studio => crate::i18n::t("CHROMA_STUDIO"),
            Self::Settings => settings::text("SETTINGS_HEADER"),
            Self::Migration => settings::text("PROFILE_MIGRATION"),
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Dashboard => "synapse/host-app-chroma.svg",
            Self::Studio => "synapse/host-chroma-studio-favicon.svg",
            Self::Settings => "synapse/settings.svg",
            Self::Migration => "synapse/migration-favicon.svg",
        }
    }
}

pub(super) struct ChromaWindow {
    page: Entity<ChromaPage>,
    studio: Option<Entity<StudioSession>>,
    settings: Option<Entity<ChromaSettings>>,
    migration: Option<Entity<super::profile_migration::MigrationPage>>,
    migration_focus: FocusHandle,
    active: ChromaTab,
    settings_return: ChromaTab,
    window: AnyWindowHandle,
    owner: WeakEntity<AppShell>,
    page_subscription: Option<Subscription>,
    settings_subscription: Option<Subscription>,
    settings_observer: Option<Subscription>,
}

impl ChromaWindow {
    pub(super) fn new(
        owner: WeakEntity<AppShell>,
        devices: Vec<Entity<ProductWorkspace>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.set_window_title("Chroma");
        // gpui's portable WindowOptions accepts decoded image data rather
        // than the host's path flag; retain the audited path as the source
        // contract while the native title bar supplies the window chrome.
        let _icon_path = CHROMA_APP_WINDOW_ICON_PATH;
        let page = cx.new(|cx| ChromaPage::new(window, cx));
        page.update(cx, |page, cx| page.set_devices(devices, cx));
        let mut this = Self {
            page,
            studio: None,
            settings: None,
            migration: None,
            migration_focus: cx.focus_handle(),
            active: ChromaTab::Dashboard,
            settings_return: ChromaTab::Dashboard,
            window: window.window_handle(),
            owner,
            page_subscription: None,
            settings_subscription: None,
            settings_observer: None,
        };
        this.bind_window(window, cx);
        this
    }
    pub(super) fn bind_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.window = window.window_handle();
        if let Some(settings) = &self.settings {
            settings.update(cx, |settings, cx| settings.bind_window(window, cx));
        }
        let owner = self.owner.clone();
        // Host navigation uses the shell's main-window handle, never
        // subscribe_in's retained association (ensure_window uses or_insert).
        self.page_subscription = Some(cx.subscribe(
            &self.page,
            move |this, _, event: &ChromaPageEvent, cx| match event {
                ChromaPageEvent::OpenStudio => {
                    let owner = owner.clone();
                    cx.defer(move |cx| {
                        let _ = owner.update(cx, |shell, cx| shell.open_chroma_studio(cx));
                    });
                }
                ChromaPageEvent::OpenSettings => {
                    let handle = this.window;
                    let target = cx.entity().downgrade();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            let _ = target.update(cx, |this, cx| this.open_settings(window, cx));
                        });
                    });
                }
                ChromaPageEvent::OpenTour(kind) => {
                    let Ok(handle) = owner.update(cx, |shell, _| shell.main_window) else {
                        return;
                    };
                    let owner = owner.clone();
                    let kind = *kind;
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            #[cfg(target_os = "windows")]
                            super::tray::native::show(window);
                            window.activate_window();
                            let _ = owner.update(cx, |shell, cx| {
                                shell.navigate(Location::Tour(kind), window, cx)
                            });
                        });
                    });
                }
            },
        ));
    }
    pub(super) fn open_studio(&mut self, studio: Entity<StudioSession>, cx: &mut Context<Self>) {
        self.studio = Some(studio.clone());
        self.active = ChromaTab::Studio;
        let handle = self.window;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                studio.update(cx, |studio, cx| studio.focus(window, cx))
            });
        });
        cx.notify();
    }
    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active != ChromaTab::Settings {
            self.settings_return = self.active;
        }
        if self.settings.is_none() {
            let Ok(locale) = self.owner.update(cx, |shell, _| shell.settings.clone()) else {
                return;
            };
            let dashboard = self.page.clone();
            let page = cx.new(|cx| ChromaSettings::new(locale, dashboard, window, cx));
            self.settings_subscription = Some(cx.subscribe(&page, |this, _, action, cx| {
                match action {
                    SettingsAction::ResetTutorials => {
                        this.page.update(cx, |page, cx| page.reset_tutorials(cx))
                    }
                    SettingsAction::OpenMigration => {
                        if this.migration.is_none() {
                            // Current migration OD receives app only for its toolbar;
                            // scanner state is isolated in this Chroma-owned instance.
                            this.migration = Some(cx.new(|cx| {
                                super::profile_migration::MigrationPage::new_for(
                                    super::profile_migration::MigrationApp::Chroma,
                                    cx,
                                )
                            }));
                        }
                        this.active = ChromaTab::Migration;
                        let focus = this.migration_focus.clone();
                        let handle = this.window;
                        cx.defer(move |cx| {
                            let _ = handle.update(cx, |_, window, cx| focus.focus(window, cx));
                        });
                        cx.notify();
                    }
                }
            }));
            self.settings_observer = Some(cx.observe(&page, |_, _, cx| cx.notify()));
            self.settings = Some(page);
        }
        self.select_tab(ChromaTab::Settings, window, cx);
    }
    fn select_tab(&mut self, tab: ChromaTab, window: &mut Window, cx: &mut Context<Self>) {
        self.active = tab;
        match tab {
            ChromaTab::Dashboard => self.page.update(cx, |p, cx| p.focus(window, cx)),
            ChromaTab::Studio => {
                if let Some(p) = &self.studio {
                    p.update(cx, |p, cx| p.focus(window, cx));
                }
            }
            ChromaTab::Settings => {
                if let Some(p) = &self.settings {
                    p.update(cx, |p, cx| p.focus(window, cx));
                }
            }
            ChromaTab::Migration => self.migration_focus.focus(window, cx),
        }
        cx.notify();
    }
    fn close_tab(&mut self, tab: ChromaTab, window: &mut Window, cx: &mut Context<Self>) {
        match tab {
            ChromaTab::Settings => {
                if self
                    .settings
                    .as_ref()
                    .is_some_and(|p| !p.update(cx, |p, cx| p.can_close(cx)))
                {
                    self.select_tab(ChromaTab::Settings, window, cx);
                    return;
                }
                self.settings = None;
                self.settings_subscription = None;
                self.settings_observer = None;
            }
            ChromaTab::Migration => self.migration = None,
            ChromaTab::Studio => self.studio = None,
            ChromaTab::Dashboard => return,
        }
        if self.active == tab {
            let next = match (tab, self.settings_return) {
                (ChromaTab::Settings, ChromaTab::Studio) if self.studio.is_some() => {
                    ChromaTab::Studio
                }
                (ChromaTab::Settings, ChromaTab::Migration) if self.migration.is_some() => {
                    ChromaTab::Migration
                }
                (ChromaTab::Migration, _) if self.settings.is_some() => ChromaTab::Settings,
                _ => ChromaTab::Dashboard,
            };
            self.select_tab(next, window, cx);
        }
        cx.notify();
    }
    fn tab(&self, tab: ChromaTab, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let label = tab.title().to_uppercase();
        let active = self.active == tab;
        let id = tab.id();
        let mut frame = host_tabs::tab_frame(id.into(), active)
            .w(css(host_tabs::tab_width(&label, window)))
            .child(
                gpui_kit::base::Button::new(id)
                    .accessibility_label(label.clone())
                    .flex()
                    .items_center()
                    .size_full()
                    .rounded_t(css(5.))
                    .px(css(12.))
                    .py(css(6.))
                    .justify_start()
                    .text_size(css(12.))
                    .font_weight(FontWeight::LIGHT)
                    .bg(if active {
                        HostColors::surface()
                    } else {
                        HostColors::background()
                    })
                    .text_color(if active {
                        HostColors::active_text()
                    } else {
                        HostColors::inactive_text()
                    })
                    .focus_visible(|s| s.border_1().border_color(cx.theme().primary))
                    .child(img(tab.icon()).size(css(20.)).flex_shrink_0())
                    .child(div().ml(css(10.)).text_ellipsis().child(label))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_tab(tab, window, cx);
                    })),
            );
        if tab != ChromaTab::Dashboard {
            frame = frame.child(
                gpui_kit::base::Button::new(SharedString::from(format!("close-{}-tab", tab.id())))
                    .accessibility_label(crate::i18n::t("CLOSE"))
                    .absolute()
                    .right(css(5.))
                    .top(css(5.))
                    .size(css(24.))
                    .opacity(0.)
                    .group_hover(SharedString::from(id), |s| s.opacity(1.))
                    .focus_visible(|s| s.opacity(1.).border_1().border_color(cx.theme().primary))
                    .child(
                        img(if active {
                            "synapse/host-close_active_tab.svg"
                        } else {
                            "synapse/host-close-original.svg"
                        })
                        .size_full(),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.close_tab(tab, window, cx);
                    })),
            );
        }
        frame.into_any_element()
    }
}

impl Render for ChromaWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = host_tabs::titlebar_frame(false)
            .child(div().w(css(8.)))
            .child(self.tab(ChromaTab::Dashboard, window, cx));
        if self.studio.is_some() {
            bar = bar
                .child(div().w(css(4.)))
                .child(self.tab(ChromaTab::Studio, window, cx));
        }
        if self.settings.is_some() {
            bar = bar
                .child(div().w(css(4.)))
                .child(self.tab(ChromaTab::Settings, window, cx));
        }
        if self.migration.is_some() {
            bar = bar
                .child(div().w(css(4.)))
                .child(self.tab(ChromaTab::Migration, window, cx));
        }
        bar = bar
            .child(div().flex_1())
            .child(
                host_tabs::window_button(
                    "chroma-minimize",
                    "synapse/host-minimize.svg",
                    "Minimize",
                )
                .on_click(|_, window, _| window.minimize_window()),
            )
            .child(
                host_tabs::maximize_button(window.is_maximized()).on_click(|_, window, _| {
                    let _ = host_tabs::host_window::toggle_maximize(window);
                }),
            )
            .child(
                host_tabs::window_button("chroma-close", "synapse/host-close.svg", "Close")
                    .on_click(|_, window, _| window.remove_window()),
            );
        let root = match self.active {
            ChromaTab::Dashboard => None,
            ChromaTab::Studio => self.studio.as_ref().map(|p| p.clone().into_any_element()),
            ChromaTab::Settings => self.settings.as_ref().map(|p| p.clone().into_any_element()),
            ChromaTab::Migration => self.migration.as_ref().map(|p| {
                div()
                    .id("chroma-migration")
                    .track_focus(&self.migration_focus)
                    .size_full()
                    .child(p.clone())
                    .into_any_element()
            }),
        }
        .unwrap_or_else(|| self.page.clone().into_any_element());
        v_flex()
            .id(SharedString::from(format!(
                "display-mode-{}",
                DisplayMode::ChromaApp.key()
            )))
            .size_full()
            .child(bar)
            .child(div().flex_1().min_h_0().child(root))
    }
}

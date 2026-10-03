//! App navigation and persistence. Device feature state lives in DeviceWorkspace.
use crate::ui::scroll::SourceScrollable as _;
use crate::{
    features::{ProductWorkspace, WorkspaceEvent},
    model::Device,
    nav::Tab,
    preferences::CustomColors,
    store,
    ui::source_alert::{AlertAction, AlertPlacement, SourceAlert},
    ui::surface,
};
use gpui_kit::component::{
    button::{ButtonCustomVariant, ButtonRounded, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::collections::{BTreeMap, VecDeque};

#[cfg(test)]
#[path = "shell/save_queue_tests.rs"]
mod save_queue_tests;

mod account_menu;
mod alexa_page;
mod app_picker;
mod app_picker_host;
mod display_window;
mod firmware_update;
mod header_status;
mod host_tabs;
mod introduction_tour;
mod iot_popup;
mod macro_page;
mod main_pages;
mod pairing_page;
mod pairing_window;
mod profile_migration;
mod release_notes;
mod runtime_page;
mod service_pages;
mod settings_page;
mod tray;
use introduction_tour::TourKind;

#[derive(Clone, PartialEq)]
enum Location {
    Main(Tab),
    Device(String),
    Pairing,
    Tour(TourKind),
    Alexa,
    FirmwareUpdate,
    ProfileMigration,
    Macro,
}
struct PreparedSave {
    window: AnyWindowHandle,
    file: store::WorkspaceFile,
}
#[derive(Debug, PartialEq, Eq)]
enum SaveContinuation {
    Queued,
    Auxiliary,
    Close,
    Idle,
}
fn save_continuation(
    close_requested: &mut bool,
    succeeded: bool,
    queued: bool,
    auxiliary_pending: bool,
) -> SaveContinuation {
    if !succeeded {
        // A later successful write in another scope must not hide this failure
        // by closing the app. Keep queued requests, but do not spin on autosave.
        *close_requested = false;
        return if queued {
            SaveContinuation::Queued
        } else {
            SaveContinuation::Idle
        };
    }
    if queued {
        SaveContinuation::Queued
    } else if auxiliary_pending {
        SaveContinuation::Auxiliary
    } else if std::mem::take(close_requested) {
        SaveContinuation::Close
    } else {
        SaveContinuation::Idle
    }
}
pub struct AppShell {
    #[cfg(target_os = "windows")]
    main_window: AnyWindowHandle,
    tray: Option<tray::DesktopTray>,
    tray_events: Option<Task<()>>,
    tray_startup: Option<Task<()>>,
    tray_click: Option<Task<()>>,
    tray_ignore_release: bool,
    devices: Vec<Entity<ProductWorkspace>>,
    host_tabs: host_tabs::HostTabs,
    location: Location,
    history: Vec<Location>,
    history_index: usize,
    tracking_intro_seen: bool,
    saved_intro_seen: bool,
    subscriptions: Vec<Subscription>,
    save_task: Option<Task<()>>,
    pending_saves: VecDeque<PreparedSave>,
    close_requested: bool,
    storage_error: Option<String>,
    status: String,
    dashboard_state: Entity<main_pages::DashboardState>,
    dashboard_tutorial: Entity<main_pages::DashboardTutorial>,
    introduction_tours:
        BTreeMap<TourKind, (Entity<introduction_tour::IntroductionTour>, Subscription)>,
    alexa: Option<Entity<alexa_page::AlexaPage>>,
    alexa_subscription: Option<Subscription>,
    firmware_update: Option<(Entity<firmware_update::FirmwareUpdate>, Subscription)>,
    profile_migration: Option<Entity<profile_migration::MigrationPage>>,
    macro_page: Option<Entity<macro_page::MacroPage>>,
    tour_trigger: FocusHandle,
    shortcuts: Entity<crate::features::shortcuts::Shortcuts>,
    settings: Entity<settings_page::SettingsPage>,
    gamer_room: Entity<service_pages::GamerRoomPage>,
    module_catalog: Entity<service_pages::ModuleCatalog>,
    pairing: Entity<pairing_page::PairingPage>,
    source_alert: Option<Entity<SourceAlert>>,
    release_notes: Option<Entity<release_notes::ReleaseNotes>>,
    iot_popup: Option<Entity<iot_popup::IotPopup>>,
    account_menu: Entity<account_menu::AccountMenu>,
    app_picker: Entity<app_picker::AppPicker>,
}
impl AppShell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (devices, intro, shortcuts, preferences, custom_colors, host_order, dashboard, error) =
            match store::read_workspace(&store::store_path()) {
                Ok(Some(file)) => (
                    file.devices,
                    file.tracking_intro_seen,
                    file.shortcuts,
                    file.preferences,
                    file.custom_colors,
                    file.host_tab_order,
                    file.dashboard,
                    None,
                ),
                Ok(None) => (
                    crate::model::measured_devices(),
                    false,
                    vec![],
                    Default::default(),
                    [None; 16],
                    vec![],
                    Default::default(),
                    None,
                ),
                Err(error) => (
                    crate::model::measured_devices(),
                    false,
                    vec![],
                    Default::default(),
                    [None; 16],
                    vec![],
                    Default::default(),
                    Some(error.to_string()),
                ),
            };
        cx.set_global(CustomColors::new(custom_colors));
        let shortcuts =
            cx.new(|cx| crate::features::shortcuts::Shortcuts::new(shortcuts, window, cx));
        let runtime = cx.new(|_| runtime_page::RuntimePanel::new());
        let gamer_room_seen = preferences.gamer_room_tutorial_seen;
        let dashboard_seen = preferences.dashboard_tutorial_seen;
        let settings =
            cx.new(|cx| settings_page::SettingsPage::new(preferences, runtime, window, cx));
        let mut this = Self {
            devices: vec![],
            host_tabs: host_tabs::HostTabs::new(cx),
            location: Location::Main(Tab::Home),
            history: vec![],
            history_index: 0,
            tracking_intro_seen: intro,
            saved_intro_seen: intro,
            subscriptions: vec![],
            save_task: None,
            pending_saves: VecDeque::new(),
            close_requested: false,
            tray: None,
            #[cfg(target_os = "windows")]
            main_window: window.window_handle(),
            tray_events: None,
            tray_startup: None,
            tray_click: None,
            tray_ignore_release: false,
            storage_error: error,
            status: "本地配置预览 · 尚未写入硬件".into(),
            dashboard_state: cx.new(|_| main_pages::DashboardState::new(dashboard)),
            dashboard_tutorial: cx.new(|_| main_pages::DashboardTutorial::new(dashboard_seen)),
            introduction_tours: BTreeMap::new(),
            alexa: None,
            alexa_subscription: None,
            firmware_update: None,
            profile_migration: None,
            macro_page: None,
            tour_trigger: cx.focus_handle().tab_stop(true),
            shortcuts,
            settings,
            gamer_room: cx.new(|_| service_pages::GamerRoomPage::new()),
            module_catalog: cx.new(|_| service_pages::ModuleCatalog::new()),
            pairing: cx.new(|cx| pairing_page::PairingPage::new(window, cx)),
            source_alert: None,
            release_notes: None,
            iot_popup: None,
            account_menu: cx.new(|cx| account_menu::AccountMenu::new(window, cx)),
            app_picker: cx.new(|cx| app_picker::AppPicker::new(window, cx)),
        };
        this.gamer_room
            .update(cx, |page, cx| page.set_tutorial_seen(gamer_room_seen, cx));
        this.sync_persistence_state(cx);
        this.subscriptions
            .push(cx.observe(&this.dashboard_state, |_, _, cx| cx.notify()));
        this.subscriptions.push(cx.subscribe_in(
            &this.dashboard_state,
            window,
            |this, _, _: &main_pages::DashboardChanged, window, cx| {
                this.sync_app_picker(window, cx);
                this.save_auxiliary_preferences(cx);
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.app_picker,
            window,
            |this, _, event, window, cx| this.handle_app_picker(event, window, cx),
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.account_menu,
            window,
            |this, _, _: &account_menu::ExitRequested, window, cx| this.request_exit(window, cx),
        ));
        this.subscriptions
            .push(cx.observe_global::<CustomColors>(|this, cx| {
                if cx.global::<CustomColors>().dirty() {
                    this.save_auxiliary_preferences(cx);
                }
                cx.notify();
            }));
        this.subscriptions.push(cx.subscribe_in(
            &this.settings,
            window,
            |this, _, event, window, cx| match event {
                settings_page::SettingsEvent::Changed => {
                    let preferences = this.settings.read(cx).snapshot();
                    let seen = preferences.gamer_room_tutorial_seen;
                    this.gamer_room
                        .update(cx, |page, cx| page.set_tutorial_seen(seen, cx));
                    this.dashboard_tutorial.update(cx, |tutorial, cx| {
                        tutorial.set_seen(preferences.dashboard_tutorial_seen, cx)
                    });
                    this.save_auxiliary_preferences(cx);
                    cx.notify();
                }
                settings_page::SettingsEvent::Language => {
                    if let Some(tray) = &mut this.tray {
                        tray.refresh(cx);
                    }
                    for device in &this.devices {
                        device.update(cx, |device, cx| device.refresh_locale(window, cx));
                    }
                    cx.refresh_windows();
                }
                settings_page::SettingsEvent::Preview(pid) => this.add_preview(*pid, window, cx),
                settings_page::SettingsEvent::PreviewChromaTour => {
                    this.navigate(Location::Tour(TourKind::Chroma), window, cx);
                }
                settings_page::SettingsEvent::PreviewAlexa => {
                    alexa_page::open_preview(window, cx);
                }
                settings_page::SettingsEvent::PreviewAppPicker => {
                    app_picker::open_preview(window, cx);
                }
                settings_page::SettingsEvent::ProfileMigration => {
                    this.navigate(Location::ProfileMigration, window, cx);
                }
                settings_page::SettingsEvent::PreviewModules => {
                    this.module_catalog
                        .update(cx, |catalog, cx| catalog.open_preview(window, cx));
                }
                settings_page::SettingsEvent::PreviewHeader => {
                    header_status::open_preview(window, cx);
                }
                settings_page::SettingsEvent::ReleaseNotes => {
                    this.release_notes = Some(release_notes::open(window, cx));
                    cx.notify();
                }
                settings_page::SettingsEvent::Pairing => {
                    this.navigate(Location::Pairing, window, cx)
                }
                settings_page::SettingsEvent::ResetTutorials => {
                    this.tracking_intro_seen = false;
                    for device in &this.devices {
                        device.update(cx, |device, cx| device.set_intro_seen(false, cx));
                    }
                    this.gamer_room
                        .update(cx, |page, cx| page.reset_tutorial(cx));
                    this.dashboard_tutorial
                        .update(cx, |tutorial, cx| tutorial.reset(cx));
                    this.settings.update(cx, |settings, cx| {
                        settings.tutorial_seen(false, cx);
                        settings.dashboard_tutorial_seen(false, cx);
                    });
                    this.save_auxiliary_preferences(cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe(
            &this.dashboard_tutorial,
            |this, _, _: &main_pages::DashboardTutorialEvent, cx| {
                this.settings.update(cx, |settings, cx| {
                    settings.dashboard_tutorial_seen(true, cx)
                });
                this.save_auxiliary_preferences(cx);
                cx.notify();
            },
        ));
        this.subscriptions.push(cx.subscribe(
            &this.gamer_room,
            |this, _, _: &service_pages::GamerRoomEvent, cx| {
                this.settings
                    .update(cx, |settings, cx| settings.tutorial_seen(true, cx));
                this.save_auxiliary_preferences(cx);
                cx.notify();
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.pairing,
            window,
            |this, _, event: &pairing_page::PairingPageEvent, window, cx| match event {
                pairing_page::PairingPageEvent::Back => {
                    this.navigate(Location::Main(Tab::Home), window, cx)
                }
                // Dashboard 7861 `hi`：设备盒按下时用 `productId` 与
                // `deviceContainerId` 打开该产品的配对窗口；两者缺一就不动作。
                pairing_page::PairingPageEvent::OpenProductWindow(device) => {
                    this.open_product_pairing_window(device, cx)
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.module_catalog,
            window,
            |this, _, event: &service_pages::ModuleCatalogEvent, window, cx| match event {
                service_pages::ModuleCatalogEvent::OpenModule(module) => match *module {
                    service_pages::ModulePage::Picker(module) => {
                        this.handle_app_picker(
                            &app_picker::AppPickerEvent::Open(app_picker::PickerTarget::Module(
                                module,
                            )),
                            window,
                            cx,
                        );
                    }
                    // 原版此盒聚焦 `synapse-introduction` 窗口。
                    service_pages::ModulePage::IntroductionTour => {
                        this.navigate(Location::Tour(TourKind::Synapse), window, cx);
                    }
                    // 原版此盒聚焦名为 `macro` 的窗口（`/synapse/macro/`）。
                    service_pages::ModulePage::Macro => {
                        this.navigate(Location::Macro, window, cx);
                    }
                },
                service_pages::ModuleCatalogEvent::FirmwareUpdate { device, preview } => {
                    this.open_firmware_update(device.clone(), *preview, window, cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe(
            &this.shortcuts,
            |this, _, _: &crate::features::shortcuts::ShortcutsChanged, cx| {
                if this.shortcuts.read(cx).committed_pending() {
                    this.save_auxiliary_preferences(cx);
                }
                cx.notify();
            },
        ));
        for device in devices {
            this.add_device(device, window, cx);
        }
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "--demo-keyboard") {
            this.add_device(crate::demo::demo_keyboard(), window, cx);
        }
        if let Some(pid) = args
            .iter()
            .position(|a| a == "--preview-product")
            .and_then(|p| args.get(p + 1))
            .and_then(|v| v.parse::<u32>().ok())
        {
            if crate::features::has_product_workspace(pid) {
                this.add_preview(pid, window, cx);
            }
        }
        if let Some(key) = args
            .iter()
            .position(|a| a == "--tab")
            .and_then(|p| args.get(p + 1))
        {
            let tab = Tab::from_arg(key);
            if tab.is_some_and(Tab::is_main) {
                this.location = Location::Main(tab.unwrap());
            } else if tab == Some(Tab::Pairing) {
                this.location = Location::Pairing;
            } else {
                let normalized = key.to_ascii_uppercase();
                let device = this
                    .devices
                    .iter()
                    .rev()
                    .find(|d| {
                        let pid = d.read(cx).device(cx).product_id;
                        let original = Tab::for_product(pid);
                        tab.is_some_and(|t| {
                            original.contains(&t) || (t == Tab::Help && !original.is_empty())
                        }) || crate::product::registered(pid)
                            .and_then(|p| p.primary_navigation())
                            .is_some_and(|n| {
                                n.pages().iter().any(|p| {
                                    p.id().key() == key
                                        || p.kind().key() == normalized
                                        || p.kind().key().strip_prefix("TAB_")
                                            == Some(normalized.as_str())
                                })
                            })
                    })
                    .cloned();
                if let Some(device) = device {
                    let identity = device.read(cx).identity(cx);
                    device.update(cx, |d, cx| {
                        if let Some(tab) = tab {
                            d.set_page(tab, window, cx);
                        }
                        d.set_source_page(key, window, cx);
                    });
                    this.location = Location::Device(identity);
                } else {
                    this.status = "?????????????".into();
                }
            }
        }
        this.host_tabs.visit(&this.location, cx);
        this.host_tabs.restore_order(&host_order);
        this.host_tabs.reveal_active(&this.location);
        this.subscriptions
            .push(cx.observe_window_bounds(window, |this, _, cx| {
                this.host_tabs.reveal_active(&this.location);
                cx.notify();
            }));
        if window.focused(cx).is_none() {
            this.host_tabs.focus_location(&this.location, window, cx);
        }
        this.history = vec![this.location.clone()];
        let weak = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let Some(entity) = weak.upgrade() else {
                return true;
            };
            entity.update(cx, |this, cx| this.request_window_close(window, cx));
            false
        });
        this.install_tray(window, cx);
        this
    }
    fn add_device(&mut self, mut device: Device, window: &mut Window, cx: &mut Context<Self>) {
        // Migrate only the explicit previews created before layout identity was stored.
        if device.layout_id == 0
            && (device.serial_number == "PREVIEW-653"
                || device.serial_number == "DEMO-KEYBOARD-0001")
        {
            device.layout_id = 1;
        }
        let entity =
            cx.new(|cx| ProductWorkspace::new(device, self.tracking_intro_seen, window, cx));
        self.subscriptions.push(
            cx.subscribe_in(&entity, window, |this, _, event, _window, cx| match event {
                WorkspaceEvent::Changed => cx.notify(),
                WorkspaceEvent::IntroDismissed => {
                    this.tracking_intro_seen = true;
                    for device in &this.devices {
                        device.update(cx, |d, cx| d.set_intro_seen(true, cx));
                    }
                    this.settings
                        .update(cx, |settings, cx| settings.tutorial_viewed(cx));
                    this.save_auxiliary_preferences(cx);
                }
            }),
        );
        if crate::features::has_product_workspace(entity.read(cx).device(cx).product_id) {
            self.host_tabs
                .open(host_tabs::HostTab::Device(entity.read(cx).identity(cx)), cx);
        }
        self.devices.push(entity);
        self.sync_app_picker(window, cx);
    }
    fn navigate(&mut self, next: Location, window: &mut Window, cx: &mut Context<Self>) {
        self.request_navigation(next, None, window, cx);
    }
    /// 打开产品的 `displayMode=multiDevicePairing` 窗口（Dashboard 7861 `hi`）。
    ///
    /// 原版把 `/synapse/products/<pid>/ui/index.html` 连同 `containerId`、
    /// `displayMode` 与（非空时）`allMasters` 交给宿主的具名窗口，窗口名与策略见
    /// [窗口契约](../docs/re/display-window-contract.md)。窗口几何由宿主决定、契约
    /// 里没有给出，这里沿用主窗口的尺寸。
    fn open_product_pairing_window(&mut self, device: &serde_json::Value, cx: &mut App) {
        let product_id = device
            .get("productId")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok());
        let container_id = device
            .get("deviceContainerId")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty());
        let (Some(product_id), Some(container_id)) = (product_id, container_id) else {
            return;
        };
        let identity = display_window::WindowIdentity {
            container_id: Some(container_id),
            product_id: Some(product_id),
            serial_number: device
                .get("serialNumber")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        };
        let name = display_window::multi_device_pairing_name(&identity);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1280.), px(820.)), cx)),
            window_min_size: Some(size(px(1080.), px(720.))),
            ..TitleBar::window_options()
        };
        // 原版对这个窗口传 `ZP.sameWindow`（`policy=3`）与 `ZP.autoFocus`
        // （`shouldFocus=1`）：命中同名窗口就复用并聚焦。
        let policy = display_window::WindowPolicy::Same;
        // `allMasters` 来自宿主写入的 connectedDeviceInfo 投影（Dashboard `jt`/`Et`），
        // 该投影尚未审计；没有真实记录时传 None，窗口显示 4130 页面原有的空态。
        let payload = pairing_window::PairingWindowPayload { all_masters: None };
        if let Err(error) =
            display_window::open_or_focus(cx, name, policy, options, move |window, cx| {
                cx.new(|cx| pairing_window::PairingWindow::new(window, cx, payload))
            })
        {
            eprintln!("无法打开配对窗口：{error}");
        }
    }
    fn request_navigation(
        &mut self,
        next: Location,
        history_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if next == self.location {
            self.host_tabs.focus_location(&next, window, cx);
            if next == Location::Alexa {
                if let Some(page) = &self.alexa {
                    page.update(cx, |page, cx| page.focus(window, cx));
                }
            }
            if next == Location::FirmwareUpdate {
                if let Some((page, _)) = &self.firmware_update {
                    page.update(cx, |page, cx| page.focus(window, cx));
                }
            }
            return;
        }
        if self.location == Location::Main(Tab::Shortcuts) && self.shortcuts.read(cx).draft_dirty()
        {
            let save = cx.entity().downgrade();
            let discard = save.clone();
            let save_next = next.clone();
            let discard_next = next.clone();
            self.source_alert = Some(SourceAlert::open(
                crate::i18n::t("SAVE_REMAPPED_BUTTON_HEADER"),
                format!(
                    "{}\n\n{}",
                    crate::i18n::t("SAVE_REMAPPED_BUTTON_MSG1"),
                    crate::i18n::t("SAVE_REMAPPED_BUTTON_MSG2")
                ),
                "shortcuts-nav-keep",
                vec![
                    AlertAction::new(
                        "shortcuts-nav-discard",
                        crate::i18n::t("DONT_SAVE"),
                        move |window, cx| {
                            let _ = discard.update(cx, |this, cx| {
                                this.shortcuts.update(cx, |shortcuts, cx| {
                                    shortcuts.dismiss_for_navigation(cx)
                                });
                                this.navigate_now(discard_next.clone(), history_index, window, cx);
                            });
                        },
                    ),
                    AlertAction::new(
                        "shortcuts-nav-save",
                        crate::i18n::t("SAVE"),
                        move |window, cx| {
                            let _ = save.update(cx, |this, cx| {
                                if this
                                    .shortcuts
                                    .update(cx, |shortcuts, cx| shortcuts.prepare_save(window, cx))
                                {
                                    this.navigate_now(save_next.clone(), history_index, window, cx);
                                }
                            });
                        },
                    )
                    .primary()
                    .disabled(!self.shortcuts.read(cx).valid()),
                ],
                AlertPlacement::AboveCenter,
                window,
                cx,
            ));
            cx.notify();
        } else {
            if self.location == Location::Main(Tab::Shortcuts) {
                self.shortcuts
                    .update(cx, |s, cx| s.dismiss_for_navigation(cx));
            }
            self.navigate_now(next, history_index, window, cx);
        }
    }
    fn navigate_now(
        &mut self,
        next: Location,
        history_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.app_picker
            .update(cx, |picker, cx| picker.dismiss(window, cx));
        self.host_tabs.visit(&next, cx);
        if next != self.location {
            self.host_tabs.focus_location(&next, window, cx);
            if let Location::Device(key) = &self.location {
                if let Some(device) = self
                    .devices
                    .iter()
                    .find(|device| device.read(cx).identity(cx) == *key)
                {
                    device.update(cx, |device, cx| device.dismiss_profile_dialog(window, cx));
                }
            }
            if self.location == Location::Pairing {
                self.pairing.update(cx, |page, cx| page.deactivate(cx));
            }
            if next == Location::Pairing {
                self.pairing
                    .update(cx, |page, cx| page.activate(window, cx));
            }
            if let Location::Tour(kind) = next {
                self.introduction_tours.entry(kind).or_insert_with(|| {
                    let tour = cx.new(|cx| introduction_tour::IntroductionTour::new(kind, cx));
                    let subscription = cx.subscribe_in(
                        &tour,
                        window,
                        move |this, _, _: &introduction_tour::CloseRequested, window, cx| {
                            this.close_host_tab(host_tabs::HostTab::Tour(kind), window, cx);
                        },
                    );
                    (tour, subscription)
                });
                self.introduction_tours
                    .get(&kind)
                    .unwrap()
                    .0
                    .update(cx, |tour, cx| tour.focus(window, cx));
            }
            if next == Location::Alexa {
                if self.alexa.is_none() {
                    let page = cx.new(|cx| alexa_page::AlexaPage::new(window, cx));
                    self.alexa_subscription = Some(cx.observe(&page, |_, _, cx| cx.notify()));
                    self.alexa = Some(page);
                }
                self.alexa
                    .as_ref()
                    .unwrap()
                    .update(cx, |page, cx| page.focus(window, cx));
            }
            if next == Location::FirmwareUpdate {
                if self.firmware_update.is_none() {
                    self.initialize_firmware_update(None, false, window, cx);
                }
                if let Some((page, _)) = &self.firmware_update {
                    page.update(cx, |page, cx| page.focus(window, cx));
                }
            }
            if next == Location::ProfileMigration && self.profile_migration.is_none() {
                self.profile_migration = Some(cx.new(profile_migration::MigrationPage::new));
            }
            if next == Location::Macro {
                if self.macro_page.is_none() {
                    self.macro_page = Some(cx.new(macro_page::MacroPage::new));
                }
                self.macro_page
                    .as_ref()
                    .unwrap()
                    .update(cx, |page, cx| page.focus(window, cx));
            }
            if let Some(index) = history_index {
                self.history_index = index;
            } else {
                self.history.truncate(self.history_index + 1);
                self.history.push(next.clone());
                self.history_index = self.history.len() - 1;
            }
            self.location = next;
            cx.notify();
        }
    }
    fn move_history(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.location == Location::Alexa {
            if let Some(page) = &self.alexa {
                page.update(cx, |page, cx| {
                    if delta < 0 {
                        page.go_back(window, cx);
                    } else {
                        page.go_forward(window, cx);
                    }
                });
            }
            return;
        }
        let target = self.history_index as isize + delta;
        if target >= 0 && target < self.history.len() as isize {
            self.request_navigation(
                self.history[target as usize].clone(),
                Some(target as usize),
                window,
                cx,
            );
        }
    }
    fn discard_profiles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.save_task.is_some() {
            return;
        }
        let mut identities = Vec::new();
        let mut blocked = Vec::new();
        for device in &self.devices {
            let device = device.read(cx);
            if !device.committed_pending(cx) {
                continue;
            }
            identities.push(device.identity(cx));
            if device.discard_would_remove_mapping(cx) {
                blocked.push(device.device(cx).display_name());
            }
        }
        if blocked.is_empty() {
            self.apply_profile_discard(&identities, false, window, cx);
            return;
        }
        let owner = cx.entity().downgrade();
        self.source_alert = Some(SourceAlert::open(
            "放弃正在编辑的按键映射？",
            format!(
                "恢复已保存配置会移除以下设备正在编辑的配置文件或旋钮模式：\n{}\n\n继续编辑可保留这些映射草稿。",
                blocked.join("\n")
            ),
            "profiles-keep-editing",
            vec![
                AlertAction::new(
                    "profiles-discard-mapping",
                    "丢弃映射并回退配置",
                    move |window, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            this.apply_profile_discard(&identities, true, window, cx);
                        });
                    },
                ),
                AlertAction::new("profiles-cancel-discard", "继续编辑", |_, _| {}).primary(),
            ],
            AlertPlacement::AboveCenter,
            window,
            cx,
        ));
        cx.notify();
    }
    fn apply_profile_discard(
        &mut self,
        identities: &[String],
        allow_mapping_discard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.save_task.is_some() {
            self.status = "正在保存，请等待完成后再丢弃配置。".into();
            cx.notify();
            return;
        }
        for device in &self.devices {
            if identities.contains(&device.read(cx).identity(cx)) {
                device.update(cx, |device, cx| {
                    if !device.discard_committed(window, cx) && allow_mapping_discard {
                        device.discard(window, cx);
                    }
                });
            }
        }
        cx.notify();
    }
    fn sync_persistence_state(&self, cx: &mut Context<Self>) {
        let error = self.storage_error.clone();
        self.settings
            .update(cx, |settings, cx| settings.set_persistence_state(error, cx));
    }
    fn auxiliary_preferences_pending(&self, cx: &App) -> bool {
        self.tracking_intro_seen != self.saved_intro_seen
            || self.dashboard_state.read(cx).pending()
            || self.host_tabs.order_pending()
            || self.settings.read(cx).dirty()
            || cx.global::<CustomColors>().dirty()
            || self.shortcuts.read(cx).committed_pending()
    }
    fn save_profiles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(error) = &self.storage_error {
            self.status = format!("未保存：原配置文件无法读取（{error}）。请先修复配置文件。");
            cx.notify();
            return;
        }
        let devices: Vec<Device> = self
            .devices
            .iter()
            .map(|d| d.read(cx).snapshot(cx))
            .collect();
        let shortcuts = self.shortcuts.read(cx).saved_snapshot();
        let preferences = self.settings.read(cx).snapshot();
        let custom_colors = cx.global::<CustomColors>().colors();
        let intro = self.tracking_intro_seen;
        let host_order = self.host_tabs.order();
        let dashboard = self.dashboard_state.read(cx).snapshot();
        let file = store::WorkspaceFile::new(devices, intro)
            .with_shortcuts(shortcuts)
            .with_preferences(preferences)
            .with_custom_colors(custom_colors)
            .with_host_tab_order(host_order)
            .with_dashboard(dashboard);
        self.pending_saves.push_back(PreparedSave {
            window: window.window_handle(),
            file,
        });
        self.start_pending_save(cx);
    }
    fn start_pending_save(&mut self, cx: &mut Context<Self>) -> bool {
        if self.save_task.is_some() {
            return true;
        }
        let Some(mut request) = self.pending_saves.pop_front() else {
            return false;
        };
        // Keep validated device drafts captured at the click. Preferences have
        // immediate semantics, so a queued save always takes their latest value.
        request.file.preferences = self.settings.read(cx).snapshot();
        request.file.shortcuts = self.shortcuts.read(cx).saved_snapshot();
        let PreparedSave { window, file } = request;
        let snapshot = file.clone();
        let path = store::store_path();
        self.status = "正在保存到本机…".into();
        cx.notify();
        self.save_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { store::write_workspace(&path, &file) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.save_task = None;
                this.sync_persistence_state(cx);
                match result {
                    Ok(()) => {
                        let store::WorkspaceFile {
                            devices,
                            preferences,
                            custom_colors,
                            tracking_intro_seen: intro,
                            host_tab_order: host_order,
                            dashboard,
                            ..
                        } = snapshot;
                        // Commit exactly the captured revision. New edits during I/O stay dirty.
                        for (entity, snapshot) in this.devices.iter().zip(devices) {
                            entity.update(cx, |d, cx| d.mark_saved(snapshot, cx));
                        }
                        this.saved_intro_seen = intro;
                        this.host_tabs.mark_order_saved(host_order);
                        this.dashboard_state
                            .update(cx, |state, _| state.mark_saved(dashboard));
                        CustomColors::mark_saved(custom_colors, cx);
                        this.settings
                            .update(cx, |settings, cx| settings.mark_saved(preferences, cx));
                        this.status = "已保存到本机 · 尚未发送到设备".into();
                        if this.settings.read(cx).snapshot().notifications && !this.close_requested
                        {
                            // Persistence completes without needing the original
                            // window; a closed window only suppresses its toast.
                            cx.defer(move |cx| {
                                let _ = window.update(cx, |_, window, cx| {
                                    window.push_notification("配置已保存到本机。", cx);
                                });
                            });
                        }
                        this.continue_save_queue(true, cx);
                    }
                    Err(error) => {
                        this.status = format!("保存失败：{error}");
                        this.continue_save_queue(false, cx);
                    }
                }
                cx.notify();
            });
        }));
        self.sync_persistence_state(cx);
        true
    }
    fn continue_save_queue(&mut self, succeeded: bool, cx: &mut Context<Self>) {
        #[cfg(target_os = "windows")]
        if self.close_requested && !succeeded {
            let handle = self.main_window;
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| tray::native::show(window));
            });
        }
        let queued = !self.pending_saves.is_empty();
        let auxiliary_pending = self.auxiliary_preferences_pending(cx);
        match save_continuation(
            &mut self.close_requested,
            succeeded,
            queued,
            auxiliary_pending,
        ) {
            SaveContinuation::Queued => {
                self.start_pending_save(cx);
            }
            SaveContinuation::Auxiliary => self.save_auxiliary_preferences(cx),
            SaveContinuation::Close => cx.quit(),
            SaveContinuation::Idle => {}
        }
    }
    fn save_auxiliary_preferences(&mut self, cx: &mut Context<Self>) {
        // Settings, tutorial flags, palette edits and committed shortcuts share
        // the serialized writer. Uncommitted device/editor drafts stay out.
        if self.save_task.is_some() || self.storage_error.is_some() {
            cx.notify();
            return;
        }
        let intro = self.tracking_intro_seen;
        let preferences = self.settings.read(cx).snapshot();
        let custom_colors = cx.global::<CustomColors>().colors();
        let shortcuts = self.shortcuts.read(cx).snapshot();
        let shortcuts_pending = self.shortcuts.read(cx).committed_pending();
        let host_order = self.host_tabs.order();
        let dashboard = self.dashboard_state.read(cx).snapshot();
        let file = store::WorkspaceFile::new(
            self.devices
                .iter()
                .map(|d| d.read(cx).saved_snapshot(cx))
                .collect(),
            intro,
        )
        .with_shortcuts(shortcuts.clone())
        .with_preferences(preferences.clone())
        .with_custom_colors(custom_colors)
        .with_host_tab_order(host_order.clone())
        .with_dashboard(dashboard.clone());
        let path = store::store_path();
        self.save_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { store::write_workspace(&path, &file) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.save_task = None;
                this.sync_persistence_state(cx);
                match result {
                    Ok(()) => {
                        this.saved_intro_seen = intro;
                        this.host_tabs.mark_order_saved(host_order);
                        this.dashboard_state
                            .update(cx, |state, _| state.mark_saved(dashboard));
                        CustomColors::mark_saved(custom_colors, cx);
                        this.shortcuts.update(cx, |shortcuts_state, cx| {
                            shortcuts_state.mark_saved(shortcuts, cx)
                        });
                        this.settings
                            .update(cx, |settings, cx| settings.mark_saved(preferences, cx));
                        if shortcuts_pending {
                            this.status = "快捷键已保存到本机 · 尚未应用到引擎".into();
                        }
                        this.continue_save_queue(true, cx);
                    }
                    Err(error) => {
                        this.status = format!("本地偏好保存失败：{error}");
                        this.settings.update(cx, |settings, cx| {
                            settings.set_persistence_state(Some(error.to_string()), cx)
                        });
                        this.continue_save_queue(false, cx);
                    }
                }
                cx.notify();
            });
        }));
        self.sync_persistence_state(cx);
    }
    fn request_window_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(target_os = "windows")]
        if let Some(tray) = &mut self.tray {
            tray.hide_popup(cx);
            tray::native::hide(window);
            return;
        }
        // If the tray could not be created, keep a reachable exit route.
        self.request_exit(window, cx);
    }

    fn request_exit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((page, _)) = &self.firmware_update {
            if !page.update(cx, |page, cx| page.allow_close(window, cx)) {
                #[cfg(target_os = "windows")]
                tray::native::show(window);
                self.navigate(Location::FirmwareUpdate, window, cx);
                return;
            }
        }
        // Closing is not a Save command. Drain only writes already requested;
        // unsubmitted profile/mapping drafts do not create an exit prompt.
        if self.save_task.is_some() || !self.pending_saves.is_empty() {
            self.close_requested = true;
            self.start_pending_save(cx);
        } else {
            cx.quit();
        }
    }
    fn add_preview(&mut self, pid: u32, window: &mut Window, cx: &mut Context<Self>) {
        if crate::product::registered(pid).is_none() {
            return;
        }
        let serial = format!("PREVIEW-{pid}");
        if !self
            .devices
            .iter()
            .any(|d| d.read(cx).device(cx).serial_number == serial)
        {
            let mut device = crate::demo::mouse_mat_preview(pid)
                .or_else(|| crate::demo::registered_preview(pid))
                .expect("registered preview");
            // 653's own root selects layoutId || 1 for its default preview.
            if pid == 653 {
                device.layout_id = 1;
            }
            self.add_device(device, window, cx);
        }
        let key = self
            .devices
            .iter()
            .find(|d| d.read(cx).device(cx).serial_number == format!("PREVIEW-{pid}"))
            .unwrap()
            .read(cx)
            .identity(cx);
        self.navigate(Location::Device(key), window, cx);
    }
    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let save_profiles = cx.entity().downgrade();
        let discard_profiles = save_profiles.clone();
        let title = match &self.location {
            Location::Device(key) => self
                .devices
                .iter()
                .find(|d| d.read(cx).identity(cx) == *key)
                .map(|d| d.read(cx).device(cx).display_name())
                .unwrap_or_default(),
            Location::Main(Tab::Setting) => "设置".into(),
            Location::Main(_) => "RAZER SYNAPSE".into(),
            Location::Pairing => "多设备配对".into(),
            Location::Tour(kind) => kind.title().into(),
            Location::Alexa => "Alexa".into(),
            Location::FirmwareUpdate => "固件更新".into(),
            Location::ProfileMigration => crate::i18n::t("PROFILE_MIGRATION").into(),
            // 原版窗口名就是 `macro`（Dashboard 模块 69937 的 `O="macro"`）。
            Location::Macro => crate::i18n::t_or("TEXT_PROFILE_BAR_MACRO", "宏"),
        };
        let (has_previous, has_next) = if self.location == Location::Alexa {
            self.alexa.as_ref().map_or((false, false), |page| {
                let page = page.read(cx);
                (page.has_previous_page(), page.has_next_page())
            })
        } else if self.location == Location::ProfileMigration {
            // Migration OD passes an empty tabNavigations list to its toolbar.
            (false, false)
        } else {
            (
                self.history_index > 0,
                self.history_index + 1 < self.history.len(),
            )
        };
        h_flex()
            .id("app-toolbar")
            .h(rems(2.375))
            .flex_shrink_0()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        surface::asset_button(
                            "history-back",
                            "synapse/history-back.svg",
                            "后退",
                            cx,
                        )
                        .w(surface::css(40.))
                        .h_full()
                        .rounded(ButtonRounded::None)
                        .custom(
                            ButtonCustomVariant::new(cx)
                                .color(cx.theme().transparent)
                                .hover(cx.theme().secondary_hover)
                                .active(cx.theme().secondary_hover),
                        )
                        .disabled(!has_previous)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.move_history(-1, window, cx)),
                        ),
                    )
                    .child(
                        surface::asset_button(
                            "history-forward",
                            "synapse/history-forward.svg",
                            "前进",
                            cx,
                        )
                        .w(surface::css(40.))
                        .h_full()
                        .rounded(ButtonRounded::None)
                        .custom(
                            ButtonCustomVariant::new(cx)
                                .color(cx.theme().transparent)
                                .hover(cx.theme().secondary_hover)
                                .active(cx.theme().secondary_hover),
                        )
                        .disabled(!has_next)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.move_history(1, window, cx)),
                        ),
                    )
                    .when(
                        matches!(self.location, Location::Alexa | Location::ProfileMigration),
                        |navigation| {
                            navigation.child(
                                surface::asset_button(
                                    if self.location == Location::Alexa {
                                        "alexa-refresh"
                                    } else {
                                        "migration-refresh"
                                    },
                                    "synapse/alexa-refresh.svg",
                                    crate::i18n::t("REFRESH"),
                                    cx,
                                )
                                .w(surface::css(40.))
                                .h_full()
                                .rounded(ButtonRounded::None)
                                .custom(
                                    ButtonCustomVariant::new(cx)
                                        .color(cx.theme().transparent)
                                        .hover(cx.theme().secondary_hover)
                                        .active(cx.theme().secondary_hover),
                                )
                                .on_click(cx.listener(
                                    |this, _, window, cx| {
                                        if this.location == Location::Alexa {
                                            if let Some(page) = &this.alexa {
                                                page.update(cx, |page, cx| {
                                                    page.refresh(window, cx)
                                                });
                                            }
                                        } else if this.location == Location::ProfileMigration {
                                            this.profile_migration =
                                                Some(cx.new(profile_migration::MigrationPage::new));
                                            cx.notify();
                                        }
                                    },
                                )),
                            )
                        },
                    ),
            )
            .child(
                div()
                    .id("toolbar-title")
                    .flex_grow(3.)
                    .flex_basis(surface::css(340.))
                    .min_w_0()
                    .text_size(surface::css(14.))
                    .text_color(cx.theme().muted_foreground)
                    .text_center()
                    .text_ellipsis()
                    .when(self.location == Location::Alexa, |title| {
                        title.flex().items_center().justify_center().child(
                            img("synapse/alexa-header.svg")
                                .h(surface::css(16.))
                                .w(surface::css(205.382 * 16. / 30.))
                                .object_fit(ObjectFit::Contain),
                        )
                    })
                    .when(self.location != Location::Alexa, |view| view.child(title)),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .justify_end()
                    .child(header_status::unsaved_profiles(
                        self.devices
                            .iter()
                            .filter_map(|device| {
                                let device = device.read(cx);
                                device.committed_pending(cx).then(|| {
                                    (device.identity(cx), device.device(cx).display_name())
                                })
                            })
                            .collect(),
                        self.save_task.is_some(),
                        move |window, cx| {
                            let _ =
                                save_profiles.update(cx, |this, cx| this.save_profiles(window, cx));
                        },
                        move |window, cx| {
                            let _ = discard_profiles
                                .update(cx, |this, cx| this.discard_profiles(window, cx));
                        },
                        cx,
                    ))
                    .when(
                        self.settings
                            .read(cx)
                            .snapshot()
                            .profile_migration_icon_visible,
                        |view| {
                            view.child(profile_migration::header_button(cx).on_click(cx.listener(
                                |this, _, window, cx| {
                                    this.navigate(Location::ProfileMigration, window, cx);
                                },
                            )))
                        },
                    )
                    .child(self.app_picker.clone())
                    .child(
                        surface::asset_button("app-settings", "synapse/settings.svg", "设置", cx)
                            .w(surface::css(46.))
                            .h_full()
                            // `.toolbar .right>div:hover`: square, immediate
                            // #2d2d2d fill; pressing keeps that same hover fill.
                            .rounded(ButtonRounded::None)
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(cx.theme().transparent)
                                    .hover(cx.theme().secondary_hover)
                                    .active(cx.theme().secondary_hover),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.navigate(Location::Main(Tab::Setting), window, cx)
                            })),
                    )
                    .child(self.account_menu.clone()),
            )
            .into_any_element()
    }
    fn main_page(&self, page: Tab, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        if page == Tab::Setting {
            return self.settings.clone().into_any_element();
        }
        let mut layout = main_pages::MainLayout::new(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
            if page == Tab::Home {
                self.devices.len()
            } else {
                0
            },
        );
        if page == Tab::GamerRoom
            && f32::from(window.viewport_size().width) * 16. / f32::from(window.rem_size()) >= 2560.
        {
            layout.body_max_width = 2540.;
        }
        let body = match page {
            Tab::Home => self.dashboard(&layout, cx),
            Tab::Modules => self.modules_page(cx),
            Tab::GamerRoom => self.gamer_room_page(cx),
            Tab::Shortcuts => self.shortcuts_page(cx),
            _ => div().into_any_element(),
        };
        v_flex()
            .size_full()
            .when(page != Tab::Setting, |this| {
                this.child(
                    gpui_kit::base::Tabs::new("main-navigation")
                        .h(surface::css(48.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .tab_group()
                        .gap(surface::css(20.))
                        .border_b_2()
                        .border_color(cx.theme().title_bar)
                        .children(Tab::MAIN.map(|tab| {
                            let button = surface::navigation_button(
                                SharedString::from(format!("main-tab-{}", tab.id())),
                                tab.label(),
                                tab == page,
                                cx,
                            )
                            .role(Role::Tab)
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.navigate(Location::Main(tab), window, cx)
                                },
                            ));
                            if tab == Tab::GamerRoom {
                                div()
                                    .relative()
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .child(button)
                                    .when(page == Tab::Home, |nav| {
                                        nav.child(self.dashboard_tutorial.clone())
                                    })
                                    .into_any_element()
                            } else {
                                button.into_any_element()
                            }
                        })),
                )
            })
            .child(
                div()
                    .id("main-page-scroll")
                    .flex_1()
                    .min_h_0()
                    .scrollable_both()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(surface::css(layout.body_max_width))
                            .mx_auto()
                            .pt(surface::css(10.))
                            .px(surface::css(layout.gutter))
                            .pb(surface::css(20.))
                            .gap(surface::css(20.))
                            .child(body),
                    ),
            )
            .into_any_element()
    }
}
impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match &self.location {
            Location::Device(key) => self
                .devices
                .iter()
                .find(|d| d.read(cx).identity(cx) == *key)
                .map(|d| d.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::Main(page) => self.main_page(*page, window, cx),
            Location::Tour(kind) => self
                .introduction_tours
                .get(kind)
                .map(|(tour, _)| tour.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::Alexa => self
                .alexa
                .as_ref()
                .map(|page| page.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::FirmwareUpdate => self
                .firmware_update
                .as_ref()
                .map(|(page, _)| page.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::ProfileMigration => self
                .profile_migration
                .as_ref()
                .map(|page| page.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::Macro => self
                .macro_page
                .as_ref()
                .map(|page| page.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::Pairing => div()
                .id("pairing-page-scroll")
                .size_full()
                .scrollable_both()
                .child(self.pairing.clone())
                .into_any_element(),
        };
        v_flex()
            .key_context("AppShell")
            .track_focus(&self.host_tabs.focus)
            .on_action(cx.listener(Self::close_current_host_tab))
            .on_action(cx.listener(Self::reopen_host_tab))
            .on_action(cx.listener(Self::next_host_tab))
            .on_action(cx.listener(Self::previous_host_tab))
            .on_action(cx.listener(Self::move_host_tab_left))
            .on_action(cx.listener(Self::move_host_tab_right))
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.title_bar(window, cx))
            .child(self.toolbar(cx))
            .child(
                div()
                    .id("shell-content-viewport")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(div().absolute().inset_0().child(content)),
            )
            .when_some(self.source_alert.clone(), |view, alert| view.child(alert))
            .when_some(self.release_notes.clone(), |view, notes| view.child(notes))
            .when_some(self.iot_popup.clone(), |view, popup| view.child(popup))
            .child(
                div()
                    .h(surface::css(24.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .text_size(surface::css(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(self.status.clone()),
            )
    }
}

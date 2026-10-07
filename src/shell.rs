//! App navigation and persistence. Device feature state lives in DeviceWorkspace.
use crate::ui::scroll::SourceScrollable as _;
use crate::{
    features::{
        ProductWorkspace, WorkspaceEvent,
        display_mode_roots::{self, DisplayModeRoot},
    },
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
mod armory_page;
mod chroma_page;
mod chroma_studio_window;
mod chroma_window;
mod device_discovery;
mod display_window;
mod feedback_page;
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
mod profiles_page;
mod release_notes;
mod runtime_page;
mod service_pages;
mod settings_page;
mod settings_systray_action;
mod settings_window;
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
    Armory,
    Chroma,
    Profiles,
    Feedback,
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
enum HistoryTarget {
    Device(Entity<ProductWorkspace>),
    Profiles(Entity<profiles_page::ProfilesPage>),
    Alexa(Entity<alexa_page::AlexaPage>),
    Macro(Entity<macro_page::MacroPage>),
    Armory(Entity<armory_page::ArmoryPage>),
    Chroma(Entity<chroma_page::ChromaPage>),
    Shell(usize),
}

pub struct AppShell {
    main_window: AnyWindowHandle,
    tray: Option<tray::DesktopTray>,
    tray_events: Option<Task<()>>,
    tray_startup: Option<Task<()>>,
    tray_click: Option<Task<()>>,
    tray_ignore_release: bool,
    devices: Vec<Entity<ProductWorkspace>>,
    receiver_queries: BTreeMap<String, (u64, u64)>,
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
    macro_exit_wait: Option<Subscription>,
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
    macro_library: Entity<crate::features::macro_library::MacroLibrary>,
    armory_page: Option<Entity<armory_page::ArmoryPage>>,
    chroma_page: Entity<chroma_page::ChromaPage>,
    chroma_studio: Option<Entity<chroma_studio_window::StudioSession>>,
    chroma_host: Option<Entity<chroma_window::ChromaWindow>>,
    profiles_page: Option<Entity<profiles_page::ProfilesPage>>,
    feedback_page: Option<Entity<feedback_page::FeedbackPage>>,
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
        let (
            mut devices,
            intro,
            shortcuts,
            macros,
            preferences,
            custom_colors,
            host_order,
            dashboard,
            module_services,
            error,
        ) = match store::read_workspace(&store::store_path()) {
            Ok(Some(file)) => (
                file.devices,
                file.tracking_intro_seen,
                file.shortcuts,
                file.macros,
                file.preferences,
                file.custom_colors,
                file.host_tab_order,
                file.dashboard,
                file.module_services,
                None,
            ),
            Ok(None) => (
                vec![],
                false,
                vec![],
                Default::default(),
                Default::default(),
                [None; 16],
                vec![],
                Default::default(),
                None,
                None,
            ),
            Err(error) => (
                vec![],
                false,
                vec![],
                Default::default(),
                Default::default(),
                [None; 16],
                vec![],
                Default::default(),
                None,
                Some(error.to_string()),
            ),
        };
        for device in &mut devices {
            device.begin_local_session();
        }
        cx.set_global(CustomColors::new(custom_colors));
        let macro_library = cx.new(|_| crate::features::macro_library::MacroLibrary::new(macros));
        let shortcuts =
            cx.new(|cx| crate::features::shortcuts::Shortcuts::new(shortcuts, window, cx));
        shortcuts.update(cx, |shortcuts, cx| {
            shortcuts.set_macro_library(&macro_library.read(cx).snapshot(), window, cx)
        });
        let runtime = cx.new(|_| runtime_page::RuntimePanel::new());
        let gamer_room_seen = preferences.gamer_room_tutorial_seen;
        let dashboard_seen = preferences.dashboard_tutorial_seen;
        let settings =
            cx.new(|cx| settings_page::SettingsPage::new(preferences, runtime.clone(), window, cx));
        let mut this = Self {
            devices: vec![],
            receiver_queries: BTreeMap::new(),
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
            macro_exit_wait: None,
            tray: None,
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
            macro_library,
            armory_page: None,
            chroma_page: cx.new(|cx| chroma_page::ChromaPage::new(window, cx)),
            chroma_studio: None,
            chroma_host: None,
            profiles_page: None,
            feedback_page: None,
            tour_trigger: cx.focus_handle().tab_stop(true),
            shortcuts,
            settings,
            gamer_room: cx.new(|_| service_pages::GamerRoomPage::new()),
            module_catalog: cx.new(|cx| service_pages::ModuleCatalog::new(module_services, cx)),
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
            &this.chroma_page,
            window,
            |this, _, event: &chroma_page::ChromaPageEvent, window, cx| match event {
                chroma_page::ChromaPageEvent::OpenStudio => this.open_chroma_studio(cx),
                chroma_page::ChromaPageEvent::OpenSettings => {
                    this.navigate(Location::Main(Tab::Setting), window, cx)
                }
                chroma_page::ChromaPageEvent::OpenTour(kind) => {
                    this.navigate(Location::Tour(*kind), window, cx)
                }
            },
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
                    if let Some(page) = &this.profiles_page {
                        page.update(cx, |page, cx| page.refresh_locale(window, cx));
                    }
                    cx.refresh_windows();
                }
                settings_page::SettingsEvent::Preview(pid) => this.add_preview(*pid, window, cx),
                settings_page::SettingsEvent::PreviewVariant(pid, edition, layout) => {
                    this.add_preview_variant(*pid, *edition, *layout, window, cx)
                }
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
        this.subscriptions.push(cx.subscribe_in(
            &this.gamer_room,
            window,
            |this, _, event: &service_pages::GamerRoomEvent, window, cx| {
                match event {
                    service_pages::GamerRoomEvent::TutorialCompleted => {
                        this.settings
                            .update(cx, |settings, cx| settings.tutorial_seen(true, cx));
                        this.save_auxiliary_preferences(cx);
                    }
                    service_pages::GamerRoomEvent::OpenDevice {
                        product_id,
                        serial_number,
                        device_container_id,
                    } => {
                        let key = this.devices.iter().find_map(|workspace| {
                            let workspace = workspace.read(cx);
                            let device = workspace.device(cx);
                            let matches = |pid: u32, container: &str, serial: &str| {
                                pid == *product_id
                                    && if !device_container_id.is_empty() {
                                        container == device_container_id
                                    } else {
                                        !serial_number.is_empty() && serial == serial_number
                                    }
                            };
                            let direct = matches(
                                device.product_id,
                                &device.device_container_id,
                                &device.serial_number,
                            );
                            let child = device.sub_devices.as_ref().is_some_and(|items| {
                                items.iter().any(|item| {
                                    let pid = item
                                        .get("productId")
                                        .and_then(|v| v.as_u64())
                                        .and_then(|n| u32::try_from(n).ok())
                                        .unwrap_or(device.product_id);
                                    let container = item
                                        .get("deviceContainerId")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(&device.device_container_id);
                                    let serial = item
                                        .get("serialNumber")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(&device.serial_number);
                                    matches(pid, container, serial)
                                })
                            });
                            ((direct || child)
                                && crate::features::has_product_workspace(device.product_id))
                            .then(|| (workspace.identity(cx), !direct))
                        });
                        if let Some((key, child)) = key {
                            this.navigate(Location::Device(key), window, cx);
                            if child {
                                this.status = "子设备切换服务尚未连接。".into();
                            }
                        }
                    }
                    service_pages::GamerRoomEvent::DeviceCommand {
                        product_id,
                        device_container_id,
                        action,
                        payload,
                    } => {
                        // Preserve the source command boundary. There is no IoT
                        // transport response to justify changing observed power
                        // or override state after the trailing click handler.
                        if *product_id == 0
                            || device_container_id.is_empty()
                            || action.is_empty()
                            || !payload.is_object()
                        {
                            return;
                        }
                        this.status = "IoT 设备服务尚未连接，请求未发送。".into();
                    }
                }
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
                service_pages::ModuleCatalogEvent::OpenDevice(container) => {
                    let key = this.devices.iter().find_map(|workspace| {
                        let workspace = workspace.read(cx);
                        (workspace.device(cx).device_container_id == *container)
                            .then(|| workspace.identity(cx))
                    });
                    if let Some(key) = key {
                        this.navigate(Location::Device(key), window, cx);
                    }
                }
                service_pages::ModuleCatalogEvent::ServiceCommand {
                    action,
                    record,
                    clear_settings,
                } => {
                    let name = crate::features::module_service::localized(
                        record.get("title").or_else(|| record.get("productName")),
                        &crate::i18n::locale().to_ascii_lowercase(),
                    );
                    // A request is not an observed phase change. No native installer /
                    // uninstaller/SDK transport is connected to these source records.
                    this.status = format!(
                        "未执行 {action}：{name} 的设备/模块服务尚未连接{}。",
                        if *clear_settings {
                            "（包含清除设置请求）"
                        } else {
                            ""
                        }
                    );
                    window.push_notification(this.status.clone(), cx);
                    cx.notify();
                }
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
                    service_pages::ModulePage::Alexa => {
                        this.open_module_tab(service_pages::ModulePage::Alexa, window, cx);
                    }
                    // 原版此盒聚焦 `synapse-introduction` 窗口。
                    service_pages::ModulePage::IntroductionTour => {
                        this.navigate(Location::Tour(TourKind::Synapse), window, cx);
                    }
                    // 原版此盒聚焦名为 `macro` 的窗口（`/synapse/macro/`）。
                    service_pages::ModulePage::Macro => {
                        this.open_module_tab(service_pages::ModulePage::Macro, window, cx);
                    }
                    // 原版此盒打开 `armory` 窗口（`/synapse/armory/`）。
                    service_pages::ModulePage::Armory => {
                        this.open_module_tab(service_pages::ModulePage::Armory, window, cx);
                    }
                    // 原版此盒打开 `profiles` 窗口（`/synapse/profiles/`）。
                    service_pages::ModulePage::Profiles => {
                        this.open_module_tab(service_pages::ModulePage::Profiles, window, cx);
                    }
                    service_pages::ModulePage::Feedback => {
                        this.open_module_tab(service_pages::ModulePage::Feedback, window, cx);
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
        this.subscriptions.push(cx.subscribe_in(
            &this.macro_library,
            window,
            |this, _, _: &crate::features::macro_library::MacroLibraryChanged, window, cx| {
                let snapshot = this.macro_library.read(cx).snapshot();
                this.shortcuts.update(cx, |shortcuts, cx| {
                    shortcuts.set_macro_library(&snapshot, window, cx)
                });
                if this.macro_library.read(cx).pending() {
                    this.save_auxiliary_preferences(cx);
                }
                cx.notify();
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.shortcuts,
            window,
            |this, _, event: &crate::features::shortcuts::ShortcutsOpenModule, window, cx| {
                use crate::features::shortcuts::ShortcutsOpenModule;
                match event {
                    ShortcutsOpenModule::CharacterMap => {
                        if let Err(error) = crate::backend::system::open_character_map() {
                            this.status = format!("无法打开字符映射表：{error}");
                            window.push_notification(this.status.clone(), cx);
                            cx.notify();
                        }
                    }
                    ShortcutsOpenModule::Macro => {
                        this.open_module_tab(service_pages::ModulePage::Macro, window, cx)
                    }
                    ShortcutsOpenModule::ChromaStudio => this.handle_app_picker(
                        &app_picker::AppPickerEvent::Open(app_picker::PickerTarget::Module(
                            app_picker::PickerModule::ChromaStudio,
                        )),
                        window,
                        cx,
                    ),
                }
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
                    this.status = format!("没有设备提供页面 {key}；未切换页面。");
                }
            }
        }
        // --tab assigns the initial location directly, and can override the
        // earlier --preview-product navigation. Activate only the final device.
        for device in &this.devices {
            let active = matches!(&this.location, Location::Device(key)
                if device.read(cx).identity(cx) == *key);
            device.update(cx, |device, cx| device.set_active(active, window, cx));
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
        this.subscriptions.push(cx.subscribe_in(
            &runtime,
            window,
            |this, _, event: &runtime_page::DiscoveryObserved, window, cx| {
                this.observe_discovery(&event.0, window, cx);
            },
        ));
        // Enumeration is scheduled once after ownership/subscriptions are installed,
        // never from render. Startup does not initialize unrelated audio/mapping APIs.
        runtime.update(cx, |runtime, cx| runtime.discover_devices(cx));
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
        self.subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            |this, entity, event, window, cx| match event {
                WorkspaceEvent::Changed => {
                    this.sync_gamer_room(cx);
                    this.sync_known_devices(cx);
                    cx.notify();
                }
                WorkspaceEvent::ShareProfile => {
                    let device = entity.read(cx).snapshot(cx);
                    this.open_module_tab(service_pages::ModulePage::Armory, window, cx);
                    if this.location == Location::Armory {
                        // Current Armory mounts the product's `displayMode=armory`
                        // mapping root for the device being shared, next to the
                        // share form. Open both from the same entry.
                        let opened = this.armory_page.as_ref().is_some_and(|page| {
                            page.update(cx, |page, cx| {
                                page.open_device_root(entity.clone(), window, cx)
                            })
                        });
                        if !opened {
                            this.status = "此设备没有本地 Armory 映射页（源包无 armory 分支，或该产品没有本地映射页）。"
                                .into();
                        }
                        if let Some(page) = &this.armory_page {
                            page.update(cx, |page, cx| page.open_share_profile(device, window, cx));
                        }
                    }
                }
                WorkspaceEvent::OpenChroma => {
                    // Hue's advanced-effects entry targets the same local
                    // Chroma app contract as the app picker. Opening it is a
                    // navigation action and does not claim native install
                    // state.
                    this.open_chroma_window(cx);
                }
                WorkspaceEvent::OpenStudio => this.open_chroma_studio(cx),
                WorkspaceEvent::OpenDevice {
                    product_id,
                    edition_id,
                } => {
                    // 源 `z(e)`：切到该 `productId`+`editionId` 的设备工作区。
                    let key = this.devices.iter().find_map(|workspace| {
                        let device = workspace.read(cx).device(cx);
                        (device.product_id == *product_id && device.edition_id == *edition_id)
                            .then(|| workspace.read(cx).identity(cx))
                    });
                    if let Some(key) = key {
                        this.navigate(Location::Device(key), window, cx);
                    }
                }
                WorkspaceEvent::IntroDismissed => {
                    this.tracking_intro_seen = true;
                    for device in &this.devices {
                        device.update(cx, |d, cx| d.set_intro_seen(true, cx));
                    }
                    this.settings
                        .update(cx, |settings, cx| settings.tutorial_viewed(cx));
                    this.save_auxiliary_preferences(cx);
                }
                WorkspaceEvent::PairingRequested(device) => {
                    let payload = serde_json::json!({
                        "productId": device.product_id,
                        "deviceContainerId": device.device_container_id,
                        "serialNumber": device.serial_number,
                        "category": serde_json::to_value(device.category)
                            .ok()
                            .and_then(|value| value.as_str().map(str::to_owned)),
                        "deviceName": device.display_name(),
                    });
                    this.open_product_pairing_window(&payload, cx);
                }
            },
        ));
        if crate::features::has_product_workspace(entity.read(cx).device(cx).product_id) {
            self.host_tabs
                .open(host_tabs::HostTab::Device(entity.read(cx).identity(cx)), cx);
        }
        self.subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            |this, entity, event: &crate::features::ReceiverPairingEvent, _, cx| {
                this.query_receiver_pairing(entity.clone(), event, cx);
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            |this, entity, event: &crate::features::DockPairingEvent, _, cx| {
                this.query_dock_pairing(entity.clone(), event, cx);
            },
        ));
        self.devices.push(entity);
        if let Some(page) = &self.profiles_page {
            page.update(cx, |page, cx| page.set_devices(self.devices.clone(), cx));
        }
        if let Some(page) = &self.macro_page {
            page.update(cx, |page, cx| page.set_devices(self.devices.clone(), cx));
        }
        self.chroma_page
            .update(cx, |page, cx| page.set_devices(self.devices.clone(), cx));
        self.sync_gamer_room(cx);
        self.sync_known_devices(cx);
        self.sync_app_picker(window, cx);
    }
    /// 164/241 配对页的设备名链接需要应用当前设备列表（`productId`+`editionId`）。
    fn sync_known_devices(&self, cx: &mut Context<Self>) {
        let list: Vec<(u32, u32)> = self
            .devices
            .iter()
            .map(|workspace| {
                let device = workspace.read(cx).device(cx);
                (device.product_id, device.edition_id)
            })
            .collect();
        for workspace in &self.devices {
            workspace.update(cx, |workspace, cx| {
                workspace.set_known_devices(list.clone(), cx)
            });
        }
    }

    fn sync_gamer_room(&self, cx: &mut Context<Self>) {
        let devices = self
            .devices
            .iter()
            .map(|workspace| workspace.read(cx).snapshot(cx))
            .collect::<Vec<_>>();
        self.gamer_room
            .update(cx, |page, cx| page.sync_devices(&devices, cx));
        self.module_catalog
            .update(cx, |page, cx| page.sync_local_devices(&devices, cx));
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
            .or_else(|| device.get("containerId"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty());
        let (Some(product_id), Some(container_id)) = (product_id, container_id) else {
            return;
        };
        // The product source only mounts this root for the audited
        // `multiDevicePairing` product set. A pairing record without that
        // branch must stay on the dashboard page instead of opening a local
        // approximation of a non-existent product root.
        if !display_mode_roots::has_root_branch(DisplayModeRoot::MultiDevicePairing, product_id) {
            return;
        }
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
        // The source opener uses policy=3 (a host tab). Keep the separately
        // requested second pairing window as an explicit local exception;
        // its OS-window policy must not be presented as source equivalence.
        let policy = display_window::WindowPolicy::Different;
        // `allMasters` 来自宿主写入的 connectedDeviceInfo 投影（Dashboard `jt`/`Et`），
        // 该投影尚未审计；没有真实记录时传 None，窗口显示 4130 页面原有的空态。
        // The source URL serializes `allMasters` and parses it again in the
        // product root. Accept both representations at this native boundary.
        let all_masters = device.get("allMasters").and_then(|value| {
            value.as_str().map_or_else(
                || Some(value.clone()),
                |encoded| serde_json::from_str(encoded).ok(),
            )
        });
        let payload = pairing_window::PairingWindowPayload { all_masters };
        if let Err(error) =
            display_window::open_or_focus(cx, name, policy, options, move |window, cx| {
                cx.new(|cx| pairing_window::PairingWindow::new(window, cx, payload))
            })
        {
            eprintln!("无法打开配对窗口：{error}");
        }
    }
    /// Dashboard `chromaApp` uses policy=5: a named independent Chroma window.
    pub(super) fn open_chroma_window(&mut self, cx: &mut Context<Self>) {
        let owner = cx.entity().downgrade();
        let devices = self.devices.clone();
        let existing = self.chroma_host.clone();
        let created = std::rc::Rc::new(std::cell::RefCell::new(None));
        let result = created.clone();
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1280.), px(720.)), cx)),
            window_min_size: Some(size(px(600.), px(500.))),
            ..TitleBar::window_options()
        };
        if let Err(error) = display_window::open_or_focus(
            cx,
            "chroma-app".into(),
            display_window::WindowPolicy::Different,
            options,
            move |window, cx| {
                let host = existing.unwrap_or_else(|| {
                    cx.new(|cx| chroma_window::ChromaWindow::new(owner, devices, window, cx))
                });
                host.update(cx, |host, cx| host.bind_window(window, cx));
                result.replace(Some(host.clone()));
                host
            },
        ) {
            eprintln!("无法打开 Chroma 窗口：{error}");
        }
        if let Some(host) = created.take() {
            self.chroma_host = Some(host);
        }
    }
    /// Dashboard modules use policy=3: a named tab in the current host window.
    /// Tab.js routes this policy through sendCreateNewTabAction; only the
    /// different-window policies create an OS window. See the host policy audit.
    fn open_module_tab(
        &mut self,
        module: service_pages::ModulePage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next = match module {
            service_pages::ModulePage::Alexa => Location::Alexa,
            service_pages::ModulePage::Macro => Location::Macro,
            service_pages::ModulePage::Armory => Location::Armory,
            service_pages::ModulePage::Profiles => Location::Profiles,
            service_pages::ModulePage::Feedback => Location::Feedback,
            _ => return,
        };
        self.navigate(next, window, cx);
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
            if next == Location::Feedback {
                if let Some(page) = &self.feedback_page {
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
            if self.location == Location::Macro {
                if let Some(page) = &self.macro_page {
                    page.update(cx, |page, cx| page.cancel_recording(cx));
                }
            }
            self.module_catalog
                .update(cx, |page, cx| page.dismiss_service_removal(window, cx));
            self.host_tabs.focus_location(&next, window, cx);
            if let Location::Device(key) = &self.location {
                if let Some(device) = self
                    .devices
                    .iter()
                    .find(|device| device.read(cx).identity(cx) == *key)
                {
                    device.update(cx, |device, cx| {
                        device.set_active(false, window, cx);
                        device.dismiss_profile_dialog(window, cx);
                    });
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
            if next == Location::Feedback {
                if self.feedback_page.is_none() {
                    self.feedback_page =
                        Some(cx.new(|cx| feedback_page::FeedbackPage::new(window, cx)));
                }
                self.feedback_page
                    .as_ref()
                    .unwrap()
                    .update(cx, |page, cx| page.focus(window, cx));
            }
            if next == Location::ProfileMigration && self.profile_migration.is_none() {
                self.profile_migration = Some(cx.new(profile_migration::MigrationPage::new));
            }
            if next == Location::Profiles {
                if self.profiles_page.is_none() {
                    let page = cx.new(|cx| profiles_page::ProfilesPage::new(window, cx));
                    page.update(cx, |page, cx| page.set_devices(self.devices.clone(), cx));
                    self.subscriptions
                        .push(cx.observe(&page, |_, _, cx| cx.notify()));
                    self.profiles_page = Some(page);
                }
                self.profiles_page
                    .as_ref()
                    .unwrap()
                    .update(cx, |page, cx| page.focus(window, cx));
            }
            if next == Location::Armory {
                if self.armory_page.is_none() {
                    let page = cx.new(|cx| armory_page::ArmoryPage::new(window, cx));
                    self.subscriptions
                        .push(cx.observe(&page, |_, _, cx| cx.notify()));
                    self.armory_page = Some(page);
                }
                self.armory_page
                    .as_ref()
                    .unwrap()
                    .update(cx, |page, cx| page.focus(window, cx));
            }
            if next == Location::Macro {
                if self.macro_page.is_none() {
                    let page = cx.new(|cx| {
                        macro_page::MacroPage::new(self.macro_library.clone(), window, cx)
                    });
                    page.update(cx, |page, cx| page.set_devices(self.devices.clone(), cx));
                    self.subscriptions
                        .push(cx.observe(&page, |_, _, cx| cx.notify()));
                    self.macro_page = Some(page);
                }
                self.macro_page
                    .as_ref()
                    .unwrap()
                    .update(cx, |page, cx| page.focus(window, cx));
            }
            if next == Location::Chroma {
                self.chroma_page
                    .update(cx, |page, cx| page.focus(window, cx));
            }
            if let Some(index) = history_index {
                self.history_index = index;
            } else {
                self.history.truncate(self.history_index + 1);
                self.history.push(next.clone());
                self.history_index = self.history.len() - 1;
            }
            if let Location::Device(key) = &next {
                if let Some(device) = self
                    .devices
                    .iter()
                    .find(|device| device.read(cx).identity(cx) == *key)
                {
                    device.update(cx, |device, cx| device.set_active(true, window, cx));
                }
            }
            self.location = next;
            cx.notify();
        }
    }
    // Resolve once for both disabled presentation and activation. Internal page
    // history takes precedence; at its boundary, use recorded shell navigation.
    fn history_target(&self, forward: bool, cx: &App) -> Option<HistoryTarget> {
        match &self.location {
            Location::Device(key) => {
                if let Some(device) = self
                    .devices
                    .iter()
                    .find(|d| d.read(cx).identity(cx) == *key)
                    .filter(|d| d.read(cx).can_step_history(forward, cx))
                {
                    return Some(HistoryTarget::Device(device.clone()));
                }
            }
            Location::Profiles => {
                if self
                    .profiles_page
                    .as_ref()
                    .is_some_and(|page| page.read(cx).history_blocked(cx))
                {
                    return None;
                }
                if let Some(page) = self.profiles_page.as_ref().filter(|page| {
                    let page = page.read(cx);
                    if forward {
                        page.has_next_page()
                    } else {
                        page.has_previous_page()
                    }
                }) {
                    return Some(HistoryTarget::Profiles(page.clone()));
                }
            }
            Location::Alexa => {
                // A modal lock is not the boundary of the application's history.
                // Preserve it instead of falling through to shell navigation.
                if self
                    .alexa
                    .as_ref()
                    .is_some_and(|page| page.read(cx).history_blocked())
                {
                    return None;
                }
                if let Some(page) = self.alexa.as_ref().filter(|page| {
                    let page = page.read(cx);
                    if forward {
                        page.has_next_page()
                    } else {
                        page.has_previous_page()
                    }
                }) {
                    return Some(HistoryTarget::Alexa(page.clone()));
                }
            }
            Location::Macro => {
                if let Some(page) = self.macro_page.as_ref().filter(|page| {
                    let page = page.read(cx);
                    if forward {
                        page.has_next_page()
                    } else {
                        page.has_previous_page()
                    }
                }) {
                    return Some(HistoryTarget::Macro(page.clone()));
                }
            }
            Location::Armory => {
                if let Some(page) = self.armory_page.as_ref().filter(|page| {
                    let page = page.read(cx);
                    if forward {
                        page.has_next_page()
                    } else {
                        page.has_previous_page()
                    }
                }) {
                    return Some(HistoryTarget::Armory(page.clone()));
                }
            }
            Location::Chroma => {
                if self.chroma_page.read(cx).can_step_history(forward) {
                    return Some(HistoryTarget::Chroma(self.chroma_page.clone()));
                }
            }
            _ => {}
        }
        // Closing a host tab can leave equal entries adjacent. Skip those
        // retained entries instead of disabling an otherwise usable history.
        let index = if forward {
            (self.history_index.saturating_add(1)..self.history.len())
                .find(|index| self.history[*index] != self.location)
        } else {
            (0..self.history_index)
                .rev()
                .find(|index| self.history[*index] != self.location)
        };
        index.map(HistoryTarget::Shell)
    }
    fn move_history(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        let forward = delta > 0;
        let Some(target) = self.history_target(forward, cx) else {
            return;
        };
        match target {
            HistoryTarget::Device(device) => device.update(cx, |device, cx| {
                device.step_page_history(forward, window, cx)
            }),
            HistoryTarget::Profiles(page) => {
                page.update(cx, |page, cx| page.step_history(forward, window, cx))
            }
            HistoryTarget::Alexa(page) => page.update(cx, |page, cx| {
                if forward {
                    page.go_forward(window, cx);
                } else {
                    page.go_back(window, cx);
                }
            }),
            HistoryTarget::Macro(page) => {
                page.update(cx, |page, cx| page.step_history(forward, window, cx))
            }
            HistoryTarget::Armory(page) => {
                page.update(cx, |page, cx| page.step_history(forward, window, cx))
            }
            HistoryTarget::Chroma(page) => {
                page.update(cx, |page, cx| page.step_history(forward, cx))
            }
            HistoryTarget::Shell(index) => {
                self.request_navigation(self.history[index].clone(), Some(index), window, cx)
            }
        }
        cx.notify();
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
            || self.macro_library.read(cx).pending()
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
            .with_dashboard(dashboard)
            .with_module_services(self.module_catalog.read(cx).service_snapshot());
        let file = file.with_macros(self.macro_library.read(cx).snapshot());
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
        request.file.macros = self.macro_library.read(cx).snapshot();
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
                            macros,
                            ..
                        } = snapshot;
                        // Commit exactly the captured revision. New edits during I/O stay dirty.
                        for (entity, snapshot) in this.devices.iter().zip(devices) {
                            entity.update(cx, |d, cx| d.mark_saved(snapshot, cx));
                        }
                        this.saved_intro_seen = intro;
                        this.macro_library
                            .update(cx, |library, cx| library.mark_saved(macros, cx));
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
        // A separately stored Studio draft is another requested write. Keep the
        // exit intent until its session reports completion; do not autosave edits.
        if succeeded && !queued && !auxiliary_pending && self.studio_save_pending(cx) {
            return;
        }
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
        let macros = self.macro_library.read(cx).snapshot();
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
        .with_dashboard(dashboard.clone())
        .with_module_services(self.module_catalog.read(cx).service_snapshot());
        let file = file.with_macros(macros.clone());
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
                        this.macro_library
                            .update(cx, |library, cx| library.mark_saved(macros, cx));
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
        if let Some(page) = &self.macro_page {
            page.update(cx, |page, cx| page.cancel_recording(cx));
        }
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
        if let Some(page) = self.macro_page.clone() {
            if page.read(cx).recording_busy() {
                if self.macro_exit_wait.is_none() {
                    self.macro_exit_wait =
                        Some(cx.observe_in(&page, window, |this, page, window, cx| {
                            if !page.read(cx).recording_busy() {
                                this.macro_exit_wait = None;
                                this.request_exit(window, cx);
                            }
                        }));
                }
                page.update(cx, |page, cx| page.cancel_recording(cx));
                self.status = "正在结束宏录制并恢复临时映射状态…".into();
                cx.notify();
                return;
            }
        }
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
        if self.save_task.is_some()
            || !self.pending_saves.is_empty()
            || self.studio_save_pending(cx)
        {
            self.close_requested = true;
            self.start_pending_save(cx);
        } else {
            cx.quit();
        }
    }
    fn add_preview(&mut self, pid: u32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(edition) = settings_page::preview_editions(pid).first().copied() else {
            return;
        };
        let Some(layout) = settings_page::preview_layouts(pid).first().copied() else {
            return;
        };
        self.add_preview_variant(pid, edition, layout, window, cx);
    }
    fn add_preview_variant(
        &mut self,
        pid: u32,
        edition: u32,
        layout: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editions = settings_page::preview_editions(pid);
        let layouts = settings_page::preview_layouts(pid);
        if !editions.contains(&edition) || !layouts.contains(&layout) {
            return;
        }
        let default_variant =
            editions.first() == Some(&edition) && layouts.first() == Some(&layout);
        let serial = if default_variant {
            format!("PREVIEW-{pid}")
        } else {
            format!("PREVIEW-{pid}-{edition}-{layout}")
        };
        if !self
            .devices
            .iter()
            .any(|d| d.read(cx).device(cx).serial_number == serial)
        {
            let mut device = crate::demo::mouse_mat_preview(pid)
                .or_else(|| crate::demo::registered_preview(pid))
                .expect("registered preview");
            device.edition_id = edition;
            device.layout_id = layout;
            device.serial_number = serial.clone();
            if !default_variant {
                device.device_container_id = format!("preview-{pid}-{edition}-{layout}");
                for profile in &mut device.profiles {
                    let id = format!("{}-{edition}-{layout}", profile.id);
                    if device.active_profile == profile.id {
                        device.active_profile = id.clone();
                    }
                    profile.id = id.clone();
                    profile.guid = id;
                }
                for name in device.name.values.values_mut() {
                    *name = format!("{name} · edition {edition} / layout {layout}");
                }
                device.product_name = device.name.clone();
            }
            crate::demo::apply_preview_names(&mut device);
            self.add_device(device, window, cx);
        }
        let key = self
            .devices
            .iter()
            .find(|d| d.read(cx).device(cx).serial_number == serial)
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
            Location::Macro => crate::i18n::t("MACRO_SOURCE.TEXT_PROFILE_BAR_MACRO").into(),
            // Match the current hook's isExchangeEnabled=false until observed.
            Location::Armory => crate::i18n::t("ARMORY_SOURCE.DASHBOARD_WORKSHOP"),
            Location::Chroma => crate::i18n::t_or("CHROMA_STUDIO", "Chroma Studio").into(),
            // 模块表把 `linkedGames` 指向 profiles 窗口，标题用它的文案 key。
            Location::Profiles => crate::i18n::t_or("LINKED_GAMES", "已关联的游戏"),
            Location::Feedback => crate::i18n::t("FEEDBACK"),
        };
        let has_previous = self.history_target(false, cx).is_some();
        let has_next = self.history_target(true, cx).is_some();
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
                        surface::history_button("history-back", false, has_previous, cx).on_click(
                            cx.listener(|this, _, window, cx| this.move_history(-1, window, cx)),
                        ),
                    )
                    .child(
                        surface::history_button("history-forward", true, has_next, cx).on_click(
                            cx.listener(|this, _, window, cx| this.move_history(1, window, cx)),
                        ),
                    )
                    .when(
                        matches!(
                            self.location,
                            Location::Alexa
                                | Location::ProfileMigration
                                | Location::Profiles
                                | Location::Macro
                        ),
                        |navigation| {
                            navigation.child(
                                surface::asset_button(
                                    if self.location == Location::Alexa {
                                        "alexa-refresh"
                                    } else if self.location == Location::Profiles {
                                        "profiles-refresh"
                                    } else if self.location == Location::Macro {
                                        "macro-refresh"
                                    } else {
                                        "migration-refresh"
                                    },
                                    if self.location == Location::Profiles {
                                        "synapse/profiles-refresh.svg"
                                    } else {
                                        "synapse/alexa-refresh.svg"
                                    },
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
                                        } else if this.location == Location::Profiles {
                                            if let Some(page) = &this.profiles_page {
                                                page.update(cx, |page, cx| {
                                                    page.refresh(window, cx)
                                                });
                                            }
                                        } else if this.location == Location::Macro {
                                            if let Some(page) = &this.macro_page {
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
    fn main_page(&self, page: Tab, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
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
        let available = f32::from(window.viewport_size().width) * 16.
            / f32::from(window.rem_size())
            - surface::NAV_MORE_WIDTH
            - surface::NAV_MORE_MARGIN
            - 10.;
        let (visible, hidden) =
            surface::split_navs(&Tab::MAIN, |tab| tab.label(), available, window);
        let overflow = if hidden.is_empty() {
            None
        } else {
            let owner = cx.entity().downgrade();
            let items = hidden
                .iter()
                .map(|tab| (tab.label(), *tab == page))
                .collect();
            Some(surface::nav_overflow(
                "main-nav-overflow",
                items,
                move |index, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.navigate(Location::Main(hidden[index]), window, cx)
                    });
                },
                window,
                cx,
            ))
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
                        .children(visible.into_iter().map(|tab| {
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
                        }))
                        .children(overflow),
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
                            .max_w(surface::css(if page == Tab::Home {
                                2540.
                            } else {
                                layout.body_max_width
                            }))
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
            Location::Armory => self
                .armory_page
                .as_ref()
                .map(|page| page.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::Chroma => self.chroma_page.clone().into_any_element(),
            Location::Profiles => self
                .profiles_page
                .as_ref()
                .map(|page| page.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::Feedback => self
                .feedback_page
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
            // The source captures key releases at window scope, even after an
            // invalid outside click moves focus. Route only to the visible device.
            .capture_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                if let Location::Device(key) = &this.location {
                    if let Some(device) = this
                        .devices
                        .iter()
                        .find(|d| d.read(cx).identity(cx) == *key)
                        .cloned()
                    {
                        device.update(cx, |device, cx| device.capture_snap_key_up(event, cx));
                    }
                }
            }))
            .capture_key_down(cx.listener(|this, _: &KeyDownEvent, _, cx| {
                if let Location::Device(key) = &this.location {
                    if this.devices.iter().any(|d| {
                        d.read(cx).identity(cx) == *key && d.read(cx).captures_snap_keys(cx)
                    }) {
                        cx.stop_propagation();
                    }
                }
            }))
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
            // Feedback mounts only its own form and footer beneath the host tabs.
            .when(self.location != Location::Feedback, |view| {
                view.child(self.toolbar(cx))
            })
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
            .when(self.location != Location::Feedback, |view| {
                view.child(
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
            })
    }
}

//! App navigation and persistence. Device feature state lives in DeviceWorkspace.
use crate::store;
#[path = "shell/persistence.rs"]
mod persistence;
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonRounded, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use persistence::{PreparedSave, ProductSave};
use razer_model::model::Device;
use razer_pages::features::ProductWorkspace;
use razer_pages::features::WorkspaceEvent;
use razer_pages::features::display_mode_roots;
use razer_pages::features::display_mode_roots::DisplayModeRoot;
use razer_pages::nav::Tab;
use razer_state::CustomColors;
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::source_alert::AlertAction;
use razer_widgets::source_alert::AlertPlacement;
use razer_widgets::source_alert::SourceAlert;
use razer_widgets::surface;
use std::collections::{BTreeMap, VecDeque};

#[cfg(test)]
#[path = "shell/save_queue_tests.rs"]
mod save_queue_tests;

use razer_app_pages::app_picker;
#[path = "shell/app_picker_host.rs"]
mod app_picker_host;
#[path = "shell/receiver/local_pairing_context.rs"]
mod local_pairing_context;
use razer_app_pages::chroma_page;
#[path = "shell/audio/audio_mixer.rs"]
mod audio_mixer;
#[path = "shell/audio/audio_notifications.rs"]
mod audio_notifications;
#[path = "shell/audio/audio_volume.rs"]
mod audio_volume;
#[path = "shell/windows/chroma_studio_window.rs"]
mod chroma_studio_window;
#[path = "shell/windows/chroma_window.rs"]
mod chroma_window;
#[path = "shell/gamepad/controller_calibration.rs"]
mod controller_calibration;
#[path = "shell/gamepad/controller_calibration_focus.rs"]
mod controller_calibration_focus;
#[path = "shell/gamepad/controller_chroma.rs"]
mod controller_chroma;
#[path = "shell/gamepad/controller_input.rs"]
mod controller_input;
#[path = "shell/gamepad/controller_lighting.rs"]
mod controller_lighting;
#[path = "shell/gamepad/controller_power.rs"]
mod controller_power;
#[path = "shell/gamepad/controller_triggers.rs"]
mod controller_triggers;
#[path = "shell/devices/device_discovery.rs"]
mod device_discovery;
#[path = "shell/windows/display_window.rs"]
mod display_window;
#[path = "shell/windows/firmware_update.rs"]
mod firmware_update;
#[path = "shell/host/game_monitor.rs"]
mod game_monitor;
#[path = "shell/accessories/hanbo_cooler.rs"]
mod hanbo_cooler;
#[path = "shell/header_status.rs"]
mod header_status;
#[path = "shell/help_owner.rs"]
mod help_owner;
#[path = "shell/host/host_tabs.rs"]
mod host_tabs;
#[path = "shell/keyboard/keyboard_actuation_write.rs"]
mod keyboard_actuation_write;
#[path = "shell/keyboard/keyboard_brightness_read.rs"]
mod keyboard_brightness_read;
#[path = "shell/keyboard/keyboard_brightness_write.rs"]
mod keyboard_brightness_write;
#[path = "shell/keyboard/keyboard_indicator.rs"]
mod keyboard_indicator;
#[path = "shell/keyboard/keyboard_latest_restore.rs"]
mod keyboard_latest_restore;
#[path = "shell/keyboard/keyboard_legacy_calibration.rs"]
mod keyboard_legacy_calibration;
#[path = "shell/keyboard/keyboard_lifecycle.rs"]
mod keyboard_lifecycle;
#[path = "shell/keyboard/keyboard_sensitivity_preference.rs"]
mod keyboard_sensitivity_preference;
#[path = "shell/keyboard/keyboard_wristrest.rs"]
mod keyboard_wristrest;
#[path = "shell/audio/leviathan.rs"]
mod leviathan;
#[path = "shell/host/macro_recorder.rs"]
mod macro_recorder;
#[path = "shell/mouse/mouse_current_dpi.rs"]
mod mouse_current_dpi;
#[path = "shell/mouse/mouse_current_power.rs"]
mod mouse_current_power;
#[path = "shell/mouse/mouse_dpi_70.rs"]
mod mouse_dpi_70;
#[path = "shell/mouse/mouse_dpi_stages.rs"]
mod mouse_dpi_stages;
#[path = "shell/mouse/mouse_receiver_indicator.rs"]
mod mouse_receiver_indicator;
#[path = "shell/mouse/mouse_runtime_artwork.rs"]
mod mouse_runtime_artwork;
#[path = "shell/mouse/mouse_scroll.rs"]
mod mouse_scroll;
use controller_chroma::ControllerChromaSave;
use controller_lighting::ControllerLightingSave;
use controller_triggers::ControllerTriggerSave;
use mouse_current_dpi::MouseCurrentDpiSave;
use mouse_current_power::MouseCurrentPowerSave;
use mouse_dpi_stages::MouseDpiStagesSave;
#[path = "shell/mouse/mouse_polling_write.rs"]
mod mouse_polling_write;
#[path = "shell/devices/product_lighting.rs"]
mod product_lighting;
#[path = "shell/devices/product_mapping.rs"]
mod product_mapping;
#[path = "shell/devices/product_settings_navigation.rs"]
mod product_settings_navigation;
#[path = "shell/receiver/receiver_brightness_page.rs"]
mod receiver_brightness_page;
#[path = "shell/receiver/receiver_pairing.rs"]
mod receiver_pairing;
#[path = "shell/receiver/receiver_reset.rs"]
mod receiver_reset;
#[path = "shell/audio/seiren_audio_capture.rs"]
mod seiren_audio_capture;
#[path = "shell/audio/seiren_audio_playback.rs"]
mod seiren_audio_playback;
#[path = "shell/audio/seiren_eq.rs"]
mod seiren_eq;
#[path = "shell/audio/seiren_microphone_setup.rs"]
mod seiren_microphone_setup;
#[path = "shell/audio/thx_sound.rs"]
mod thx_sound;
use razer_app_pages::introduction_tour;
use razer_app_pages::iot_popup;
use razer_app_pages::macro_page;
#[path = "shell/navigation/main_pages.rs"]
mod main_pages;
use razer_app_pages::pairing_page;
use razer_app_pages::profile_migration;
use razer_app_pages::profiles_page;
use razer_app_pages::release_notes;
use razer_app_pages::service_pages;
use razer_settings::runtime_page;
use razer_settings::settings_page;
#[path = "shell/windows/profile_switch_notification.rs"]
mod profile_switch_notification;
#[path = "shell/windows/restart_notification.rs"]
mod restart_notification;
#[path = "shell/windows/settings_window.rs"]
mod settings_window;
#[path = "shell/host/startup.rs"]
mod startup;
#[path = "shell/tray.rs"]
mod tray;
use introduction_tour::TourKind;

#[derive(Clone, PartialEq)]
enum Location {
    Main(Tab),
    Device(String),
    Pairing(String),
    Tour(TourKind),
    FirmwareUpdate,
    ProfileMigration,
    ProductSettings(bool),
    ProductSettingsMigration,
    Macro,
    Chroma,
    Profiles,
}
struct MouseLegacyProfileSave {
    workspace: Entity<ProductWorkspace>,
    scope: Option<razer_pages::features::mouse_polling::MousePollingScope>,
    identity: String,
    device: Device,
    apply: bool,
    scroll: Option<razer_pages::features::mouse_products::MouseScrollRequested>,
}
struct MouseLegacyDpiSave {
    workspace: Entity<ProductWorkspace>,
    request: razer_pages::features::mouse_products::MouseLegacyDpiRequested,
    identity: String,
    profile: razer_model::model::Profile,
}
struct MousePollingSave {
    workspace: Entity<ProductWorkspace>,
    scope: razer_pages::features::mouse_polling::MousePollingScope,
    field: razer_pages::features::mouse_polling::PollingField,
    hz: u32,
    identity: String,
    profile: razer_model::model::Profile,
}
#[derive(Debug, PartialEq, Eq)]
enum SaveContinuation {
    Queued,
    Auxiliary,
    Close,
    Idle,
}
fn finish_identity_cache_batch(pending: &mut usize) -> bool {
    *pending = pending
        .checked_sub(1)
        .expect("accepted identity cache batch");
    *pending == 0
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
    Macro(Entity<macro_page::MacroPage>),
    Chroma(Entity<chroma_page::ChromaPage>),
    Shell(usize),
}

pub struct AppShell {
    main_window: AnyWindowHandle,
    tray: Option<razer_tray::DesktopTray>,
    tray_events: Option<Task<()>>,
    tray_startup: Option<Task<()>>,
    tray_click: Option<Task<()>>,
    tray_ignore_release: bool,
    devices: Vec<Entity<ProductWorkspace>>,
    mapping_interdevice_holds: BTreeMap<EntityId, product_mapping::InterdeviceHold>,
    receiver_queries: BTreeMap<String, (u64, u64)>,
    receiver_pairing_operations: BTreeMap<String, receiver_pairing::Session>,
    pairing_transport: std::sync::Arc<receiver_pairing::standalone::Transport>,
    pairing_transport_delivery: Option<Task<()>>,
    receiver_reset: BTreeMap<String, receiver_reset::Session>,
    receiver_reset_cleanup: Vec<std::thread::JoinHandle<()>>,
    receiver_brightness_page: BTreeMap<String, receiver_brightness_page::Session>,
    receiver_brightness_cleanup: Vec<std::thread::JoinHandle<()>>,
    receiver_pairing_cleanup: Vec<std::thread::JoinHandle<()>>,
    receiver_reconnect_diagnostics: BTreeMap<String, serde_json::Value>,
    receiver_devices: BTreeMap<String, (u32, u32)>,
    receiver_devices_complete: bool,
    local_app_features: Option<serde_json::Value>,
    local_pairing_records: Option<serde_json::Value>,
    discovery_revision: u64,
    device_observations: Vec<razer_discovery::discovery::ObservedDevice>,
    hanbo_cooler_guards: BTreeMap<String, std::sync::Arc<std::sync::atomic::AtomicBool>>,
    keyboard_actuation_operations: BTreeMap<EntityId, keyboard_actuation_write::Session>,
    keyboard_actuation_workers: Vec<std::thread::JoinHandle<()>>,
    keyboard_wristrest: BTreeMap<String, keyboard_wristrest::Session>,
    keyboard_wristrest_cleanup: Vec<keyboard_wristrest::Cleanup>,
    mouse_runtime_artwork: BTreeMap<String, mouse_runtime_artwork::Session>,
    mouse_runtime_artwork_cleanup: Vec<mouse_runtime_artwork::Cleanup>,
    mouse_receiver_indicator: BTreeMap<EntityId, mouse_receiver_indicator::Session>,
    mouse_receiver_indicator_cleanup: Vec<mouse_receiver_indicator::Cleanup>,
    controller_inputs: BTreeMap<String, controller_input::Session>,
    controller_input_cleanup: Vec<controller_input::Cleanup>,
    device_value_owners: std::collections::BTreeSet<String>,
    keyboard_identity_cache_task: Option<Task<()>>,
    keyboard_identity_cache_pending: usize,
    runtime: Entity<runtime_page::RuntimePanel>,
    runtime_exit_wait: Option<Subscription>,
    device_read_scopes: BTreeMap<String, razer_pages::features::mouse_polling::MousePollingScope>,
    audio_notifications: BTreeMap<String, audio_notifications::Session>,
    audio_global_shortcuts_enabled: bool,
    thx_jobs: BTreeMap<String, thx_sound::Session>,
    thx_owners: BTreeMap<String, std::sync::Arc<thx_sound::RetainedOwner>>,
    // Actual transport receipts are independent of the source UI/profile draft.
    thx_progress: BTreeMap<String, razer_device::thx_submission::ThxSubmissionProgress>,
    thx_render_discovery: razer_device::thx_lifecycle_discovery::ThxRenderDiscoveryTracker,
    thx_render_progress: BTreeMap<String, razer_device::thx_submission::ThxSubmissionProgress>,
    thx_job_sequence: u64,
    // Scoped diagnostic history also retains results of canceled/replaced jobs.
    thx_receipt_history: std::sync::Arc<
        std::sync::Mutex<VecDeque<razer_device::thx_submission::ThxSubmissionProgress>>,
    >,
    thx_cleanup: Vec<std::thread::JoinHandle<()>>,
    controller_calibrations: BTreeMap<String, controller_calibration::Session>,
    controller_calibration_cleanup: Vec<std::thread::JoinHandle<()>>,
    controller_calibration_stop_cleanup:
        Vec<(Task<()>, std::sync::Arc<std::sync::atomic::AtomicBool>)>,
    audio_preset_shortcuts: BTreeMap<String, audio_mixer::presets::Session>,
    audio_preset_shortcut_cleanup: Vec<std::thread::JoinHandle<()>>,
    audio_notification_cleanup: Vec<audio_notifications::Retired>,
    host_tabs: host_tabs::HostTabs,
    host_storage: razer_storage::host::HostStorage,
    macro_recorder: macro_recorder::State,
    game_observer: game_monitor::State,
    keyboard_indicator: keyboard_indicator::State,
    keyboard_latest_restore: keyboard_latest_restore::State,
    keyboard_legacy_calibration: keyboard_legacy_calibration::State,
    keyboard_lifecycle: Option<keyboard_lifecycle::Owner>,
    product_settings: product_settings_navigation::ProductSettingsState,
    location: Location,
    history: Vec<Location>,
    history_index: usize,
    tracking_intro_seen: bool,
    saved_intro_seen: bool,
    subscriptions: Vec<Subscription>,
    save_task: Option<Task<()>>,
    pending_saves: VecDeque<PreparedSave>,
    close_requested: bool,
    legacy_dpi_workers: usize,
    current_dpi_workers: usize,
    mouse_scroll_workers: usize,
    current_power_workers: usize,
    controller_trigger_workers: usize,
    controller_lighting_workers: usize,
    controller_lighting_owners: BTreeMap<String, controller_lighting::Session>,
    controller_lighting_cleanup: Vec<std::sync::Arc<controller_lighting::RetainedOwner>>,
    controller_software_status: BTreeMap<String, razer_device::controller_lighting::SoftwareStatus>,
    controller_chroma: controller_chroma::State,
    controller_power: controller_power::State,
    seiren_eq_workers: usize,
    seiren_microphone_setup: seiren_microphone_setup::State,
    seiren_audio_captures: BTreeMap<u64, seiren_audio_capture::Worker>,
    seiren_audio_playbacks: BTreeMap<u64, seiren_audio_playback::Worker>,
    mapping_exit_task: Option<Task<()>>,
    macro_exit_wait: Option<Subscription>,
    storage_error: Option<String>,
    status: String,
    dashboard_state: Entity<razer_dashboard::DashboardState>,
    restart_notifications: restart_notification::State,
    profile_notifications: profile_switch_notification::State,
    startup: startup::State,
    dashboard_tutorial: Entity<razer_dashboard::DashboardTutorial>,
    introduction_tours:
        BTreeMap<TourKind, (Entity<introduction_tour::IntroductionTour>, Subscription)>,
    firmware_update: Option<(
        Entity<razer_app_pages::firmware_update::FirmwareUpdate>,
        Subscription,
    )>,
    profile_migration: Option<Entity<profile_migration::MigrationPage>>,
    macro_page: Option<Entity<macro_page::MacroPage>>,
    macro_library: Entity<razer_pages::features::macro_library::MacroLibrary>,
    chroma_page: Entity<chroma_page::ChromaPage>,
    chroma_studio: Option<Entity<chroma_studio_window::StudioSession>>,
    chroma_host: Option<Entity<chroma_window::ChromaWindow>>,
    profiles_page: Option<Entity<profiles_page::ProfilesPage>>,
    tour_trigger: FocusHandle,
    shortcuts: Entity<razer_pages::features::shortcuts::Shortcuts>,
    settings: Entity<settings_page::SettingsPage>,
    gamer_room: Entity<service_pages::GamerRoomPage>,
    module_catalog: Entity<service_pages::ModuleCatalog>,
    pairing: Entity<pairing_page::PairingPage>,
    source_alert: Option<Entity<SourceAlert>>,
    release_notes: Option<Entity<release_notes::ReleaseNotes>>,
    iot_popup: Option<Entity<iot_popup::IotPopup>>,
    app_picker: Entity<app_picker::AppPicker>,
}
impl AppShell {
    fn tray_widgets(&self, cx: &App) -> Vec<razer_tray::TrayWidgetDevice> {
        self.devices
            .iter()
            .filter_map(|workspace| {
                let device = workspace.read(cx).snapshot(cx);
                if device.dashboard.connection_observation.is_none() {
                    return None;
                }
                let icon = razer_tray::widget_category_icon(device.category);
                let profile = device
                    .profiles
                    .iter()
                    .find(|profile| profile.id == device.active_profile)
                    .map(|profile| razer_tray::TrayWidgetProfile {
                        name: profile.name.clone(),
                        disabled: device.profiles.len() < 2,
                        has_dropdown: true,
                        local_draft: device.dashboard.local_snapshot,
                    });
                let battery = device
                    .current_power_status()
                    .filter(|power| (0..=100).contains(&power.level))
                    .map(|power| razer_tray::TrayWidgetBattery::Percent {
                        level: power.level as u8,
                        charging: power.is_charging(),
                    });
                Some(razer_tray::TrayWidgetDevice {
                    id: workspace.read(cx).identity(cx),
                    icon: icon.into(),
                    // 5492/y selects productName[zh-cn] for Chinese and en
                    // otherwise. Source catalog strings are presentation,
                    // independent of the observed hardware values.
                    title: razer_model::product_preview::source_product_name(
                        device.product_id,
                        device.edition_id,
                        if razer_i18n::locale().eq_ignore_ascii_case("zh-cn") {
                            "zh-cn"
                        } else {
                            "en"
                        },
                    ),
                    profile,
                    battery,
                })
            })
            .collect()
    }

    pub fn new(
        workspace: anyhow::Result<Option<store::WorkspaceFile>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
        ) = match workspace {
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
        let macro_library =
            cx.new(|_| razer_pages::features::macro_library::MacroLibrary::new(macros));
        let shortcuts =
            cx.new(|cx| razer_pages::features::shortcuts::Shortcuts::new(shortcuts, window, cx));
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
            mapping_interdevice_holds: BTreeMap::new(),
            receiver_queries: BTreeMap::new(),
            receiver_pairing_operations: BTreeMap::new(),
            pairing_transport: std::sync::Arc::new(receiver_pairing::standalone::Transport::new(
                cx.background_executor().clone(),
            )),
            pairing_transport_delivery: None,
            receiver_reset: BTreeMap::new(),
            receiver_reset_cleanup: Vec::new(),
            receiver_brightness_page: BTreeMap::new(),
            receiver_brightness_cleanup: Vec::new(),
            receiver_pairing_cleanup: Vec::new(),
            receiver_reconnect_diagnostics: BTreeMap::new(),
            discovery_revision: 0,
            device_observations: Vec::new(),
            hanbo_cooler_guards: BTreeMap::new(),
            keyboard_actuation_operations: BTreeMap::new(),
            keyboard_actuation_workers: Vec::new(),
            keyboard_wristrest: BTreeMap::new(),
            keyboard_wristrest_cleanup: Vec::new(),
            mouse_runtime_artwork: BTreeMap::new(),
            mouse_runtime_artwork_cleanup: Vec::new(),
            mouse_receiver_indicator: BTreeMap::new(),
            mouse_receiver_indicator_cleanup: Vec::new(),
            controller_inputs: BTreeMap::new(),
            controller_input_cleanup: Vec::new(),
            device_value_owners: std::collections::BTreeSet::new(),
            keyboard_identity_cache_task: None,
            keyboard_identity_cache_pending: 0,
            runtime: runtime.clone(),
            runtime_exit_wait: None,
            receiver_devices: BTreeMap::new(),
            receiver_devices_complete: false,
            local_app_features: None,
            local_pairing_records: None,
            device_read_scopes: BTreeMap::new(),
            audio_notifications: BTreeMap::new(),
            audio_global_shortcuts_enabled: true,
            thx_jobs: BTreeMap::new(),
            thx_owners: BTreeMap::new(),
            thx_progress: BTreeMap::new(),
            thx_render_discovery: Default::default(),
            thx_render_progress: BTreeMap::new(),
            thx_job_sequence: 0,
            thx_receipt_history: Default::default(),
            thx_cleanup: Vec::new(),
            controller_calibrations: BTreeMap::new(),
            controller_calibration_cleanup: Vec::new(),
            controller_calibration_stop_cleanup: Vec::new(),
            audio_preset_shortcuts: BTreeMap::new(),
            audio_preset_shortcut_cleanup: Vec::new(),
            audio_notification_cleanup: Vec::new(),
            host_tabs: host_tabs::HostTabs::new(cx),
            host_storage: razer_storage::host::HostStorage::default(),
            macro_recorder: macro_recorder::State::default(),
            game_observer: game_monitor::State::default(),
            keyboard_indicator: keyboard_indicator::State::default(),
            keyboard_latest_restore: keyboard_latest_restore::State::default(),
            keyboard_legacy_calibration: keyboard_legacy_calibration::State::default(),
            keyboard_lifecycle: None,
            product_settings: product_settings_navigation::ProductSettingsState::default(),
            location: Location::Main(Tab::Home),
            history: vec![],
            history_index: 0,
            tracking_intro_seen: intro,
            saved_intro_seen: intro,
            subscriptions: vec![],
            save_task: None,
            pending_saves: VecDeque::new(),
            close_requested: false,
            legacy_dpi_workers: 0,
            current_dpi_workers: 0,
            mouse_scroll_workers: 0,
            current_power_workers: 0,
            controller_trigger_workers: 0,
            controller_lighting_workers: 0,
            controller_lighting_owners: BTreeMap::new(),
            controller_chroma: controller_chroma::State::default(),
            controller_power: controller_power::State::default(),
            controller_lighting_cleanup: Vec::new(),
            controller_software_status: BTreeMap::new(),
            seiren_eq_workers: 0,
            seiren_microphone_setup: seiren_microphone_setup::State::default(),
            seiren_audio_captures: BTreeMap::new(),
            seiren_audio_playbacks: BTreeMap::new(),
            mapping_exit_task: None,
            macro_exit_wait: None,
            tray: None,
            main_window: window.window_handle(),
            tray_events: None,
            tray_startup: None,
            tray_click: None,
            tray_ignore_release: false,
            storage_error: error,
            // Startup itself has no read/write outcome. Source runtime and
            // actual task responses publish their own independent state.
            status: String::new(),
            dashboard_state: cx.new(|_| razer_dashboard::DashboardState::new(dashboard)),
            restart_notifications: restart_notification::State::default(),
            profile_notifications: profile_switch_notification::State::default(),
            startup: startup::State::default(),
            dashboard_tutorial: cx.new(|_| razer_dashboard::DashboardTutorial::new(dashboard_seen)),
            introduction_tours: BTreeMap::new(),
            firmware_update: None,
            profile_migration: None,
            macro_page: None,
            macro_library,
            chroma_page: cx.new(|cx| chroma_page::ChromaPage::new(window, cx)),
            chroma_studio: None,
            chroma_host: None,
            profiles_page: None,
            tour_trigger: cx.focus_handle().tab_stop(true),
            shortcuts,
            settings,
            gamer_room: cx.new(|_| service_pages::GamerRoomPage::new()),
            module_catalog: cx.new(|cx| service_pages::ModuleCatalog::new(module_services, cx)),
            pairing: cx.new(|cx| pairing_page::PairingPage::new(window, cx)),
            source_alert: None,
            release_notes: None,
            iot_popup: None,
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
            |this, _, _: &razer_dashboard::DashboardChanged, window, cx| {
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
            |this, _, event, window, cx| {
                match event {
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
                    settings_page::SettingsEvent::StartupRequested(setting) => {
                        this.request_startup(*setting, cx);
                    }
                    settings_page::SettingsEvent::DynamicLightingRequested(_) => {
                        // ia writes isDynamicLighting; native lighting ownership is
                        // a separate storage/service consumer and remains unimplemented.
                        this.status =
                            "灯光模式已保留为本地选择；设备控制权的服务写入适配尚未完成".into();
                        cx.notify();
                    }
                    settings_page::SettingsEvent::Preview(pid) => {
                        this.add_preview(*pid, window, cx)
                    }
                    settings_page::SettingsEvent::PreviewVariant(pid, edition, layout) => {
                        this.add_preview_variant(*pid, *edition, *layout, window, cx)
                    }
                    settings_page::SettingsEvent::ChromaTour => {
                        this.navigate(Location::Tour(TourKind::Chroma), window, cx)
                    }
                    settings_page::SettingsEvent::AppPicker => this
                        .app_picker
                        .update(cx, |picker, cx| picker.show(window, cx)),
                    settings_page::SettingsEvent::Modules => {
                        this.navigate(Location::Main(Tab::Modules), window, cx)
                    }
                    settings_page::SettingsEvent::Dashboard => {
                        this.navigate(Location::Main(Tab::Home), window, cx)
                    }
                    settings_page::SettingsEvent::Pairing => {
                        this.navigate(Location::Pairing("multi-device-pairing".into()), window, cx)
                    }
                    settings_page::SettingsEvent::ProfileMigration => {
                        this.navigate(Location::ProfileMigration, window, cx);
                    }
                    settings_page::SettingsEvent::ReleaseNotes => {
                        this.release_notes = Some(release_notes::open_from_storage(
                            release_notes::NotesApp::Synapse,
                            &mut this.host_storage,
                            window,
                            cx,
                        ));
                        cx.notify();
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
                }
            },
        ));
        this.subscriptions.push(cx.subscribe(
            &this.dashboard_tutorial,
            |this, _, _: &razer_dashboard::DashboardTutorialEvent, cx| {
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
                                && razer_pages::features::has_product_workspace(device.product_id))
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
                    this.open_product_pairing_tab(device, window, cx)
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
                        (workspace.device(cx).device_container_id == *container
                            && workspace.has_local_page(cx))
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
                    let name = razer_pages::features::module_service::localized(
                        record.get("title").or_else(|| record.get("productName")),
                        &razer_i18n::locale().to_ascii_lowercase(),
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
                    // 原版此盒聚焦 `synapse-introduction` 窗口。
                    service_pages::ModulePage::IntroductionTour => {
                        this.navigate(Location::Tour(TourKind::Synapse), window, cx);
                    }
                    // 原版此盒聚焦名为 `macro` 的窗口（`/synapse/macro/`）。
                    service_pages::ModulePage::Macro => {
                        this.open_module_tab(service_pages::ModulePage::Macro, window, cx);
                    }
                    // 原版此盒打开 `armory` 窗口（`/synapse/armory/`）。
                    // 原版此盒打开 `profiles` 窗口（`/synapse/profiles/`）。
                    service_pages::ModulePage::Profiles => {
                        this.open_module_tab(service_pages::ModulePage::Profiles, window, cx);
                    }
                },
                service_pages::ModuleCatalogEvent::FirmwareUpdate { device, preview } => {
                    this.open_firmware_update(device.clone(), *preview, window, cx);
                }
            },
        ));
        this.subscriptions.push(cx.subscribe(
            &this.shortcuts,
            |this, _, _: &razer_pages::features::shortcuts::ShortcutsChanged, cx| {
                if this.shortcuts.read(cx).committed_pending() {
                    this.save_auxiliary_preferences(cx);
                }
                cx.notify();
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.macro_library,
            window,
            |this, _, _: &razer_pages::features::macro_library::MacroLibraryChanged, window, cx| {
                let snapshot = this.macro_library.read(cx).snapshot();
                this.shortcuts.update(cx, |shortcuts, cx| {
                    shortcuts.set_macro_library(&snapshot, window, cx)
                });
                for workspace in &this.devices {
                    workspace.update(cx, |workspace, cx| {
                        workspace.set_mapping_macro_library(&snapshot, window, cx)
                    });
                }
                if this.macro_library.read(cx).pending() {
                    this.save_auxiliary_preferences(cx);
                }
                cx.notify();
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &this.shortcuts,
            window,
            |this, _, event: &razer_pages::features::shortcuts::ShortcutsOpenModule, window, cx| {
                use razer_pages::features::shortcuts::ShortcutsOpenModule;
                match event {
                    ShortcutsOpenModule::CharacterMap => {
                        if let Err(error) = razer_platform::system::open_character_map() {
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
            this.attach_device(device, window, cx);
        }
        // Restore every owner/subscription before publishing the shared list.
        // Repeating this after each restored product cloned and refreshed every
        // earlier profile, mapping target, Dashboard and picker again.
        this.sync_device_contexts(window, cx);
        this.load_local_pairing_context(window, cx);
        let args: Vec<String> = std::env::args().collect();
        if let Some(pid) = args
            .iter()
            .position(|a| a == "--preview-product")
            .and_then(|p| args.get(p + 1))
            .and_then(|v| v.parse::<u32>().ok())
        {
            if razer_pages::features::has_product_workspace(pid) {
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
                this.location = Location::Pairing("multi-device-pairing".into());
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
                        }) || razer_catalog::registered(pid)
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
        this.observe_camera_host_window(window, cx);
        this.host_tabs.visit(&this.location, cx);
        this.host_tabs.restore_order(&host_order);
        this.host_tabs.reveal_active(&this.location);
        this.subscriptions
            .push(cx.observe_window_bounds(window, |this, window, cx| {
                this.observe_camera_host_window(window, cx);
                this.host_tabs.reveal_active(&this.location);
                cx.notify();
            }));
        this.subscriptions
            .push(cx.observe_window_activation(window, |this, window, cx| {
                this.observe_controller_calibration_window_focus(window, cx);
                this.observe_help_owner_window_focus(window, cx);
                let _ = this.observe_seiren_audio_host_visibility(window, cx);
                this.observe_camera_host_window(window, cx);
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
        this.install_startup(cx);
        this.install_tray(window, cx);
        this.subscriptions.push(cx.on_app_quit(|this, cx| {
            let discovery = this
                .runtime
                .update(cx, |runtime, cx| runtime.shutdown_for_app_quit(cx));
            let identity_cache = this.keyboard_identity_cache_task.take();
            async move {
                if let Some(discovery) = discovery {
                    discovery.await;
                }
                if let Some(identity_cache) = identity_cache {
                    identity_cache.await;
                }
            }
        }));
        this.install_macro_recorder_storage(cx);
        this.install_profile_switch_notification_cleanup(cx);
        if let Err(error) = this.initialize_profile_switch_notifications(window, cx) {
            this.status = format!("Profile notification initialization: {error:#}");
        }
        this.install_audio_notification_cleanup(cx);
        this.install_thx_cleanup(cx);
        this.install_controller_calibration_cleanup(cx);
        this.install_keyboard_lifecycle(cx);
        this.install_game_monitor(cx);
        this.install_keyboard_actuation_cleanup(cx);
        this.install_keyboard_wristrest_cleanup(cx);
        this.install_keyboard_indicator_cleanup(cx);
        this.install_keyboard_latest_restore_cleanup(cx);
        this.install_keyboard_legacy_calibration_cleanup(cx);
        this.install_mouse_runtime_artwork_cleanup(cx);
        this.install_mouse_receiver_indicator_cleanup(cx);
        this.install_controller_input_cleanup(cx);
        this.install_controller_lighting_cleanup(cx);
        this.install_seiren_audio_capture_cleanup(window, cx);
        this.install_seiren_audio_playback_cleanup(cx);
        this.install_seiren_microphone_setup_cleanup(cx);
        this.install_audio_preset_shortcut_cleanup(cx);
        this.install_receiver_pairing_cleanup(cx);
        this.install_standalone_pairing_transport(window, cx);
        this.install_receiver_reset_cleanup(cx);
        this.install_receiver_brightness_cleanup(cx);
        this.subscriptions.push(cx.subscribe_in(
            &runtime,
            window,
            |this, _, event: &runtime_page::DiscoveryObserved, window, cx| {
                this.observe_discovery(&event.0, window, cx);
                // Device queries start only after the interface observation and
                // its profile/connection scopes have actually been installed.
                if let Some(ready) = &event.1 {
                    let _ = ready.send(());
                }
            },
        ));
        this.subscriptions.push(cx.subscribe_in(
            &runtime,
            window,
            |this, _, event: &runtime_page::DeviceValuesObserved, window, cx| {
                this.observe_device_values(&event.0, window, cx);
            },
        ));
        // Enumeration is scheduled once after ownership/subscriptions are installed,
        // never from render. Startup does not initialize unrelated audio/mapping APIs.
        runtime.update(cx, |runtime, cx| runtime.discover_devices(cx));
        if this.macro_library.read(cx).pending() {
            this.save_auxiliary_preferences(cx);
        }
        this
    }
    fn add_device(&mut self, device: Device, window: &mut Window, cx: &mut Context<Self>) {
        self.attach_device(device, window, cx);
        self.sync_device_contexts(window, cx);
    }

    fn attach_device(&mut self, mut device: Device, window: &mut Window, cx: &mut Context<Self>) {
        // Migrate only the explicit previews created before layout identity was stored.
        if device.layout_id == 0 && device.serial_number == "PREVIEW-653" {
            device.layout_id = 1;
        }
        let entity =
            cx.new(|cx| ProductWorkspace::new(device, self.tracking_intro_seen, window, cx));
        self.restore_seiren_microphone_setup_seen(&entity, window, cx);
        let tutorial_visible = self
            .settings
            .read(cx)
            .snapshot()
            .mouse_dynamic_tutorial_visible;
        entity.update(cx, |workspace, cx| {
            workspace.restore_mouse_dynamic_tutorial_preference(tutorial_visible, cx);
        });
        let calibration_welcome = self
            .settings
            .read(cx)
            .snapshot()
            .mouse_calibration_welcome_visible;
        entity.update(cx, |workspace, cx| {
            workspace.restore_mouse_calibration_welcome(calibration_welcome, cx);
        });
        let login_warning_hidden = self
            .settings
            .read(cx)
            .snapshot()
            .windows_login_warning_hide
            .unwrap_or(false);
        entity.update(cx, |workspace, cx| {
            workspace.observe_keyboard_mapping_login_warning_hidden(login_warning_hidden, cx);
        });
        let macros = self.macro_library.read(cx).snapshot();
        entity.update(cx, |workspace, cx| {
            workspace.set_mapping_macro_library(&macros, window, cx)
        });
        self.subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            |this, entity, event, window, cx| match event {
                WorkspaceEvent::MouseDynamicTutorialChanged(visible) => {
                    this.settings.update(cx, |settings, cx| {
                        settings.mouse_dynamic_tutorial_visible(*visible, cx);
                    });
                    this.save_auxiliary_preferences(cx);
                }
                WorkspaceEvent::MouseCalibrationWelcomeChanged(visible) => {
                    this.settings.update(cx, |settings, cx| {
                        settings.mouse_calibration_welcome_visible(*visible, cx);
                    });
                    this.save_auxiliary_preferences(cx);
                }
                WorkspaceEvent::KeyboardLightingRequested(request) => {
                    this.request_keyboard_lighting(entity.clone(), request, window, cx);
                }
                WorkspaceEvent::HanboCoolerRequested(request) => {
                    this.request_hanbo_cooler(entity.clone(), request, window, cx);
                }
                WorkspaceEvent::MouseLightingRequested(request) => {
                    this.request_mouse_lighting(entity.clone(), request, window, cx);
                }
                WorkspaceEvent::ControllerChromaRequested(request) => {
                    this.request_controller_chroma(entity.clone(), request, window, cx);
                }
                WorkspaceEvent::HelpCamoRequested(request) => {
                    if entity.read(cx).accepts_help_camo(request, cx) {
                        eprintln!("[help-camo] {:?}: current Camo host/storage/serial submission adapter is not implemented", request.command());
                    }
                }
                WorkspaceEvent::HelpSystemInfoRequested(request) => {
                    if entity.read(cx).accepts_help_system_info(request, cx) {
                        // Actual systemInfoReducer reads and the original local
                        // system-log writer are still required. Host PC values
                        // cannot replace this product's native observation.
                        eprintln!("[help-system-info] {:?}: current native system-info/log-save adapter is not implemented", request.command());
                    }
                }
                WorkspaceEvent::HelpOwnerRequested(request) => {
                    this.request_help_owner(&entity, request, window, cx);
                }
                WorkspaceEvent::RipsawMonitoringRequested(request) => {
                    if entity.read(cx).accepts_ripsaw_monitoring(request, cx) {
                        // Keep the actual source command at the boundary. No
                        // vendor DLL is called and no response is fabricated.
                        eprintln!("[ripsaw-monitoring] {}: current native monitoring submission/readback is not implemented", request.source_message()["type"]);
                    }
                }
                WorkspaceEvent::HyperfluxRequested(request) => {
                    if entity.read(cx).accepts_hyperflux(request, cx) {
                        // Keep original intent and state. No successful read,
                        // write, unbind or window lookup is fabricated.
                        eprintln!("[hyperflux] {:?}: current service/state/window adapter is not implemented", request.operation());
                    }
                }
                WorkspaceEvent::KeyboardMappingLoginWarningReadRequested => {
                    let hidden = this.settings.read(cx).snapshot().windows_login_warning_hide.unwrap_or(false);
                    entity.update(cx, |workspace, cx| {
                        workspace.observe_keyboard_mapping_login_warning_hidden(hidden, cx);
                    });
                }
                WorkspaceEvent::KeyboardMappingSensitivityWarningReadRequested { product_id } => {
                    this.read_keyboard_sensitivity_preference(&entity, *product_id, cx);
                }
                WorkspaceEvent::KeyboardMappingDialReadRequested(request) => {
                    this.read_keyboard_mapping_dial(&entity, request, cx);
                }
                WorkspaceEvent::KeyboardMappingBrightnessReadRequested(request) => {
                    this.read_keyboard_mapping_brightness(&entity, request, cx);
                }
                WorkspaceEvent::KeyboardMappingSensitivityWarningHidden { product_id } => {
                    this.hide_keyboard_sensitivity_preference(&entity, *product_id, cx);
                }
                WorkspaceEvent::KeyboardMappingLoginWarningHidden => {
                    this.settings.update(cx, |settings, cx| {
                        settings.windows_login_warning_hidden(cx);
                    });
                    // The original component closes before localStorage.set.
                    // Persistence failure must not reopen its warning.
                    this.save_auxiliary_preferences(cx);
                }
                WorkspaceEvent::KeyboardCharacterMapRequested => {
                    if let Err(error) = razer_platform::system::open_character_map() {
                        this.status = error.to_string();
                        window.push_notification(this.status.clone(), cx);
                        cx.notify();
                    }
                }
                WorkspaceEvent::MouseMappingTargetRequested(request) => {
                    this.configure_mouse_mapping_target(&entity,request,window,cx);
                }
                WorkspaceEvent::MappingCallbackRequested(request) => {
                    this.route_mapping_callback(&entity,request,window,cx);
                }
                WorkspaceEvent::KeyboardMappingInterDeviceConfigureRequested(request) => {
                    if let Err(error) = this.configure_mapping_target(&entity, request, window, cx) {
                        eprintln!("[inter-device-mapping] {error}");
                    }
                }
                WorkspaceEvent::MouseCalibrationRequested(request) => {
                    entity.update(cx, |workspace, cx| {
                        workspace.reject_mouse_calibration(
                            request,
                            "Mouse surface-calibration transport is not implemented".into(),
                            cx,
                        );
                    });
                }
                WorkspaceEvent::MouseMappingSaveRejected(reason) => {
                    // The current alert has no mounted error paragraph. Retain
                    // the draft/confirmation and report the genuine rejected
                    // local operation without adding source-absent UI text.
                    eprintln!("[mapping] local Save rejected for {}: {reason:?}", entity.read(cx).identity(cx));
                }
                WorkspaceEvent::MouseIdleRequested { scope, minutes } => {
                    this.write_mouse_idle(entity.clone(), *scope, *minutes, window, cx);
                }
                WorkspaceEvent::MouseDynamicRequested(request) => {
                    entity.update(cx, |workspace, cx| {
                        workspace.complete_mouse_dynamic(
                            request,
                            razer_pages::features::mouse_products::MouseDynamicCompletion::Unsupported(
                                "Mouse dynamic-sensitivity device adapter is not implemented".into(),
                            ),
                            window,
                            cx,
                        );
                    });
                }
                WorkspaceEvent::MouseMatcherRequested(request) => {
                    // The portable matcher controller is implemented separately.
                    // Native input monitoring/locks and PID190 stage submission
                    // are still missing; a UI click must never fabricate a reply.
                    let reason = "DPI Matcher native monitoring/storage adapter is not implemented";
                    eprintln!("[dpi-matcher] {:?}: {reason}", request.action());
                    entity.update(cx, |workspace, cx| {
                        workspace.reject_mouse_matcher(request.scope(), reason.into(), cx);
                    });
                }
                WorkspaceEvent::KeyboardBrightnessRequested { generation, percent } => {
                    this.write_keyboard_brightness(entity.clone(), *generation, *percent, window, cx);
                }
                WorkspaceEvent::KeyboardActuationRequested(request) => {
                    if !this.submit_keyboard_latest_intent(entity.clone(), request, cx)
                        && !this.submit_keyboard_legacy_calibration_intent(entity.clone(), request, cx) {
                        this.submit_keyboard_actuation(entity.clone(), request, window, cx);
                    }
                }
                WorkspaceEvent::KeyboardLegacyCalibrationHostRequested(request) => {
                    this.request_keyboard_legacy_calibration(entity.clone(), request, window, cx);
                }
                WorkspaceEvent::TriggerCalibrationRequested(request) => {
                    this.request_controller_calibration(entity.clone(), request.clone(), window, cx);
                }
                WorkspaceEvent::KeyboardIndicatorLedRequested(request) => {
                    this.submit_keyboard_indicator(entity.clone(), request, cx);
                }
                WorkspaceEvent::KeyboardBrightnessReadRequested { generation } => {
                    this.read_keyboard_brightness(entity.clone(), *generation, window, cx);
                }
                WorkspaceEvent::MousePollingRequested { scope, field, hz } => {
                    this.write_mouse_polling(entity.clone(), *scope, *field, *hz, window, cx);
                }
                WorkspaceEvent::MousePollingRestoreRequested { scope, field, hz } => {
                    this.write_mouse_polling_device(entity.clone(), *scope, *field, *hz, window, cx);
                }
                WorkspaceEvent::MouseProfileSerialObserved => {
                    entity.update(cx, |workspace, cx| workspace.restore_mouse_profile_after_serial(window, cx));
                    this.initialize_mouse182_profiles(cx);
                }
                WorkspaceEvent::MouseReceiverMonitorRequested(request) => {
                    this.start_mouse_receiver_indicator(entity.clone(), request.clone(), window, cx);
                }
                WorkspaceEvent::MouseReceiverIndicatorRequested(request) => {
                    this.submit_mouse_receiver_indicator(entity.clone(), request.clone(), cx);
                }
                WorkspaceEvent::KeyboardLinkedBaseSelected(guid) => {
                    this.select_keyboard_linked_base(entity.clone(), guid, cx);
                }
                WorkspaceEvent::MouseLegacyDpiRequested(request) => {
                    if request.persist {
                        this.persist_legacy_dpi(entity.clone(), request.clone(), window, cx);
                    } else {
                        this.submit_legacy_dpi_device(entity.clone(), request.clone(), window, cx);
                    }
                }
                WorkspaceEvent::MouseCurrentDpiRequested(request) => {
                    this.persist_current_dpi(entity.clone(), request.clone(), window, cx);
                }
                WorkspaceEvent::MouseCurrentPowerRequested(request) => {
                    this.persist_current_power(entity.clone(), request.clone(), window, cx);
                }
                WorkspaceEvent::SeirenStreamMixerRequested(request) => {
                    this.request_seiren_stream_mixer(entity.clone(),request.clone(),window,cx);
                }
                WorkspaceEvent::SeirenStreamMixerCaptureChanged(request) => {
                    this.set_seiren_stream_mixer_capture(entity.clone(),request.clone(),window,cx);
                }
                WorkspaceEvent::SeirenEqRequested(request) => {
                    this.persist_seiren_eq(entity.clone(), request.clone(), window, cx);
                }
                WorkspaceEvent::SeirenAudioCaptureRequested(request) => {
                    this.start_seiren_audio_capture(entity.clone(), request.clone(), window, cx);
                }
                WorkspaceEvent::SeirenAudioPlaybackRequested(request) => {
                    this.start_seiren_audio_playback(entity.clone(),request.clone(),window,cx);
                }
                WorkspaceEvent::SeirenMicrophoneSetupSeen(seen) => {
                    this.save_seiren_microphone_setup_seen(*seen,cx);
                }
                WorkspaceEvent::MouseLegacyProfileRequested { scope, apply } => {
                    this.persist_legacy_mouse_profile(entity.clone(), *scope, *apply, window, cx);
                }
                WorkspaceEvent::MouseScrollRequested(request) => {
                    this.persist_mouse_scroll(entity.clone(), request.clone(), window, cx);
                }
                WorkspaceEvent::KeyboardLegacyProfileRequested => {
                    this.persist_legacy_keyboard_profile(entity.clone(), window, cx);
                }
                WorkspaceEvent::KeyboardLatestProfileSwitchRequested {guid,defer_persistence} => {
                    this.switch_keyboard_latest_profile(entity.clone(),guid.clone(),*defer_persistence,cx);
                }
                WorkspaceEvent::KeyboardLatestProfileMutationRequested(mutation) => {
                    // The 756 device layer has no ProfileMutation request, so the
                    // previous mapping owner cannot reach the device queue yet.
                    // The applied local candidate stays a draft; no device write
                    // is claimed and no success is fabricated.
                    let reason = format!("配置文件变更尚未写入设备：{mutation:?}");
                    eprintln!("[keyboard756 mutation] {reason}");
                    this.status = reason;
                    cx.notify();
                }
                WorkspaceEvent::KeyboardLatestOnboardProfileRequested {slot_id,guid} => {
                    this.switch_keyboard_latest_onboard_profile(entity.clone(),*slot_id,guid.clone(),cx);
                }
                WorkspaceEvent::ProfileMacrosImported(documents) => {
                    let result=this.macro_library.update(cx,|library,cx|library.import_source_documents(documents,cx));
                    if let Err(error)=result {
                        this.status=format!("配置宏导入未完成：{error}");
                        cx.notify();
                    }
                }
                WorkspaceEvent::MouseLegacyProfileNavigate(event) => {
                    this.navigate_legacy_mouse_profile(&entity, event, window, cx);
                }
                WorkspaceEvent::MouseDpiStagesRequested { scope, revision, draft, persist_local } => {
                    this.write_mouse_dpi_stages(entity.clone(), *scope, *revision, draft.clone(), *persist_local, window, cx);
                }
                WorkspaceEvent::MouseDpiStagesReadRequested { scope, revision } => {
                    this.read_mouse_dpi_stages(entity.clone(), *scope, *revision, window, cx);
                }
                WorkspaceEvent::AudioVolumeRequested { request } => {
                    this.request_audio_volume(entity.clone(), *request, window, cx);
                }
                WorkspaceEvent::LeviathanRequested(request) => {
                    this.request_leviathan(entity.clone(),request.clone(),window,cx);
                }
                WorkspaceEvent::AudioMixerRequested { request } => {
                    this.request_audio_mixer(entity.clone(), request.clone(), window, cx);
                }
                WorkspaceEvent::AudioPresetShortcutRequested { request } => {
                    this.request_audio_preset_shortcuts(entity.clone(),request.clone(),window,cx);
                }
                WorkspaceEvent::HelpResetRequested(request) => this.request_help_reset(entity.clone(),*request,window,cx),
                WorkspaceEvent::HelpResetCanceled(request) => this.cancel_help_reset(entity,*request,cx),
                WorkspaceEvent::ReceiverBrightnessRequested(request) => this.receiver_brightness_write(entity.clone(),*request,window,cx),
                WorkspaceEvent::ReceiverBrightnessReadRequested(request) => this.receiver_brightness_read(entity.clone(),*request,window,cx),
                WorkspaceEvent::ControllerTriggerRequested(request) => {
                    this.persist_controller_trigger(entity.clone(),request.clone(),window,cx);
                }
                WorkspaceEvent::ControllerLightingRequested(request) => {
                    this.persist_controller_lighting(entity.clone(),request.clone(),window,cx);
                }
                WorkspaceEvent::Changed => {
                    this.sync_local_mapping_targets(window,cx);
                    this.sync_gamer_room(cx);
                    this.sync_known_devices(cx);
                    cx.notify();
                }
                WorkspaceEvent::KeyboardLegacyRuntimeStatus(message) => {
                    this.status = message.clone();
                    cx.notify();
                }
                WorkspaceEvent::OpenChroma => {
                    // Hue's advanced-effects entry targets the same local
                    // Chroma app contract as the app picker. Opening it is a
                    // navigation action and does not claim native install
                    // state.
                    this.open_chroma_window(cx);
                }
                WorkspaceEvent::OpenStudio => this.open_chroma_studio(cx),
                WorkspaceEvent::OpenMacro => {
                    this.open_module_tab(service_pages::ModulePage::Macro, window, cx)
                }
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
                    this.open_product_pairing_tab(&payload, window, cx);
                }
            },
        ));
        if razer_pages::features::has_product_workspace(entity.read(cx).device(cx).product_id) {
            self.host_tabs
                .open(host_tabs::HostTab::Device(entity.read(cx).identity(cx)), cx);
        }
        self.subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            |this, entity, event: &razer_pages::features::ReceiverPairingEvent, window, cx| {
                this.query_receiver_pairing(entity.clone(), event, window, cx);
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            |this, _, event: &razer_pages::features::ReceiverDeviceRequested, window, cx| {
                this.open_receiver_device(event, window, cx);
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            |this, entity, event: &razer_pages::features::DockPairingEvent, window, cx| {
                this.query_dock_pairing(entity.clone(), event, window, cx);
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            |this, _, event: &razer_pages::features::DockPairingUtilityRequested, window, cx| {
                this.open_product_pairing_tab(event.device_info(), window, cx);
            },
        ));
        entity.update(cx, |workspace, cx| {
            workspace.observe_running_games(self.game_observer.running_games(), cx);
        });
        self.devices.push(entity);
    }

    fn sync_device_contexts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_local_mapping_targets(window, cx);
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
        let snapshots: Vec<Device> = self
            .devices
            .iter()
            .map(|workspace| workspace.read(cx).snapshot(cx))
            .collect();
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
                workspace.set_known_devices(list.clone(), cx);
                workspace.set_device_context(&snapshots, cx);
                workspace.set_multi_pairing_feature(
                    local_pairing_context::multipairing_enabled(self.local_app_features.as_ref()),
                    cx,
                );
                if let Some(records) = &self.local_pairing_records {
                    workspace.set_pairing_cache(records.clone(), cx);
                }
            });
            workspace.update(cx, |workspace, cx| {
                let devices = self.receiver_devices.values().copied().collect();
                workspace.observe_receiver_devices(
                    if self.receiver_devices_complete {
                        razer_pages::features::ReceiverDevicesObservation::complete(devices)
                    } else {
                        razer_pages::features::ReceiverDevicesObservation::partial(devices)
                    },
                    cx,
                )
            });
        }
    }

    fn sync_gamer_room(&self, cx: &mut Context<Self>) {
        let mut devices = Vec::with_capacity(self.devices.len());
        let mut local_page_devices = std::collections::BTreeSet::new();
        for entity in &self.devices {
            let workspace = entity.read(cx);
            let device = workspace.snapshot(cx);
            if workspace.has_local_page(cx) {
                local_page_devices.insert((
                    device.product_id,
                    device.serial_number.clone(),
                    device.device_container_id.clone(),
                ));
            }
            devices.push(device);
        }
        self.gamer_room
            .update(cx, |page, cx| page.sync_devices(&devices, cx));
        self.module_catalog.update(cx, |page, cx| {
            page.sync_local_devices(&devices, local_page_devices, cx)
        });
    }
    fn navigate(&mut self, next: Location, window: &mut Window, cx: &mut Context<Self>) {
        self.request_navigation(next, None, window, cx);
    }
    /// 打开产品的 `displayMode=multiDevicePairing` 具名宿主 Tab（Dashboard 7861 `hi`）。
    ///
    /// 原版把 `/synapse/products/<pid>/ui/index.html` 连同 `containerId`、
    /// `displayMode` 与（非空时）`allMasters` 交给宿主的具名 Tab。Tab 名称见
    /// [窗口契约](../docs/re/display-window-contract.md)。
    fn open_product_pairing_tab(
        &mut self,
        device: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = razer_app_pages::pairing_page::product_pairing_target(device) else {
            return;
        };
        let product_id = target.product_id();
        let container_id = target.container_id().to_owned();
        let device = target.device_info();
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
        self.pairing.update(cx, |page, cx| {
            if let Err(error) = page.apply_device_info(device.clone(), window, cx) {
                eprintln!("[pairing initialization] {error}");
            }
        });
        if let Some(all_masters) = all_masters {
            self.pairing.update(cx, |page, cx| {
                page.apply_all_masters(all_masters, window, cx)
            });
        }
        self.navigate(Location::Pairing(name.to_string()), window, cx);
    }
    /// Dashboard `chromaApp` uses policy=5: a named independent Chroma window.
    pub(super) fn open_chroma_window(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.try_open_chroma_window(cx) {
            eprintln!("[chroma window] {error}");
        }
    }
    pub(super) fn try_open_chroma_window(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
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
        display_window::open_or_focus(
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
        )
        .map_err(|error| error.to_string())?;
        if let Some(host) = created.take() {
            self.chroma_host = Some(host);
        }
        Ok(())
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
            service_pages::ModulePage::Macro => Location::Macro,
            service_pages::ModulePage::Profiles => Location::Profiles,
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
                razer_i18n::t("SAVE_REMAPPED_BUTTON_HEADER"),
                format!(
                    "{}\n\n{}",
                    razer_i18n::t("SAVE_REMAPPED_BUTTON_MSG1"),
                    razer_i18n::t("SAVE_REMAPPED_BUTTON_MSG2")
                ),
                "shortcuts-nav-keep",
                vec![
                    AlertAction::new(
                        "shortcuts-nav-discard",
                        razer_i18n::t("DONT_SAVE"),
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
                        razer_i18n::t("SAVE"),
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
            // TabManager broadcasts activeTabChanged with the selected frame;
            // this predicate differs from BrowserWindow focus/blur.
            self.observe_controller_calibration_tab(&next, window, cx);
            self.observe_help_owner_tab(&next, cx);
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
            if matches!(self.location, Location::Pairing(_)) {
                self.pairing.update(cx, |page, cx| page.deactivate(cx));
            }
            if matches!(next, Location::Pairing(_)) {
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
            if next == Location::FirmwareUpdate {
                if self.firmware_update.is_none() {
                    self.initialize_firmware_update(None, false, window, cx);
                }
                if let Some((page, _)) = &self.firmware_update {
                    page.update(cx, |page, cx| page.focus(window, cx));
                }
            }
            if let Location::ProductSettings(chroma) = &next {
                self.prepare_product_settings(*chroma, window, cx);
                self.focus_product_settings(*chroma, window, cx);
            }
            if next == Location::ProductSettingsMigration {
                self.prepare_product_settings_migration(cx);
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
            if next == Location::Macro {
                if self.macro_page.is_none() {
                    self.initialize_macro_recorder_page();
                    let page = cx.new(|cx| {
                        macro_page::MacroPage::new(self.macro_library.clone(), window, cx)
                    });
                    page.update(cx, |page, cx| page.set_devices(self.devices.clone(), cx));
                    self.subscriptions.push(cx.subscribe_in(
                        &page,
                        window,
                        |this, _, change: &macro_page::MacroRecorderStorageChanged, window, cx| {
                            this.publish_macro_recorder_storage(*change, window, cx);
                        },
                    ));
                    self.subscriptions.push(cx.subscribe(
                        &page,
                        |this, _, _: &macro_page::MacroGlobalShortcutsRestored, _| {
                            this.set_audio_global_shortcuts_enabled(true);
                        },
                    ));
                    self.subscriptions.push(cx.observe(&page, |this, page, cx| {
                        let enabled = !page.read(cx).keyboard_shortcut_capture_active()
                            && !page.read(cx).record_shortcut_capture_active();
                        this.set_audio_global_shortcuts_enabled(enabled);
                        this.initialize_mouse182_profiles(cx);
                        this.sync_receiver_macro_access(cx);
                        cx.notify();
                    }));
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
        // Current profile-migration OD passes tabNavigations:[] to pD for
        // both app roots. Its toolbar must not fall through to host history.
        if self.location == Location::ProductSettingsMigration {
            return None;
        }
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
                device.step_page_history(forward, window, cx);
                device.mapping_history_clicked(forward, window, cx);
            }),
            HistoryTarget::Profiles(page) => {
                page.update(cx, |page, cx| page.step_history(forward, window, cx))
            }
            HistoryTarget::Macro(page) => {
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
        self.sync_local_mapping_targets(window, cx);
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
        let file = self.workspace_file(devices, cx);
        self.pending_saves.push_back(PreparedSave {
            window: window.window_handle(),
            file,
            product: None,
        });
        self.start_pending_save(cx);
    }
    fn persist_mouse_polling(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        scope: razer_pages::features::mouse_polling::MousePollingScope,
        field: razer_pages::features::mouse_polling::PollingField,
        hz: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let valid = self.storage_error.is_none()
            && workspace
                .read(cx)
                .mouse_polling_request_matches(scope, field, hz, cx);
        let device = workspace.read(cx).snapshot(cx);
        let profile = device
            .profiles
            .iter()
            .find(|p| p.id == device.active_profile)
            .cloned();
        if !valid || profile.is_none() {
            if let Some(error) = &self.storage_error {
                eprintln!("Mouse polling profile cannot be persisted: {error}");
            }
            workspace.update(cx, |workspace, cx| {
                workspace.finish_mouse_polling(scope, field, hz, None, cx);
            });
            return;
        }
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|entity| entity.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        let identity = workspace.read(cx).identity(cx);
        self.pending_saves.push_back(PreparedSave {
            window: window.window_handle(),
            file,
            product: Some(ProductSave::MousePolling(MousePollingSave {
                workspace,
                scope,
                field,
                hz,
                identity,
                profile: profile.expect("checked active profile"),
            })),
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
        if let Err(error) = self.rebase_current_dpi_save(&mut request, cx) {
            self.status = format!("DPI 本地保存失败：{error:#}");
            if let Some(ProductSave::MouseCurrentDpi(dpi)) = request
                .product
                .take_if(|action| matches!(action, ProductSave::MouseCurrentDpi(_)))
            {
                self.finish_current_dpi_save(dpi, false, request.window, cx);
            }
            self.continue_save_queue(false, cx);
            return false;
        }
        if let Err(error) = self.rebase_leviathan_save(&mut request, cx) {
            self.status = format!("???????????{error:#}");
            if let Some(ProductSave::Leviathan(sound)) = request
                .product
                .take_if(|action| matches!(action, ProductSave::Leviathan(_)))
            {
                self.finish_leviathan_save(sound, false, cx);
            }
            self.continue_save_queue(false, cx);
            return false;
        }
        self.rebase_current_power_save(&mut request, cx);
        if let Err(error) = self.rebase_mouse_scroll_save(&mut request, cx) {
            self.status = format!("滚轮配置保存失败：{error:#}");
            if let Some(ProductSave::MouseProfile(profile)) = request
                .product
                .take_if(|action| matches!(action, ProductSave::MouseProfile(_)))
            {
                self.finish_mouse_scroll_save(profile, false, request.window, cx);
            }
            self.continue_save_queue(false, cx);
            return false;
        }
        self.rebase_controller_trigger_save(&mut request, cx);
        if let Err(error) = self.rebase_controller_lighting_save(&mut request, cx) {
            self.status = format!("手柄灯效本地保存失败：{error:#}");
            if let Some(ProductSave::ControllerLighting(lighting)) = request
                .product
                .take_if(|action| matches!(action, ProductSave::ControllerLighting(_)))
            {
                self.finish_controller_lighting_save(lighting, false, request.window, cx);
            }
            self.continue_save_queue(false, cx);
            return false;
        }
        if let Err(error) = self.rebase_controller_chroma_save(&mut request, cx) {
            self.status = format!("Chroma 本地保存失败：{error:#}");
            if let Some(ProductSave::ControllerChroma(chroma)) = request
                .product
                .take_if(|action| matches!(action, ProductSave::ControllerChroma(_)))
            {
                self.finish_controller_chroma_save(chroma, false, request.window, cx);
            }
            self.continue_save_queue(false, cx);
            return false;
        }
        if let Err(error) = self.rebase_seiren_eq_save(&mut request, cx) {
            self.status = format!("麦克风设置本地保存失败：{error:#}");
            if let Some(ProductSave::SeirenEq(eq)) = request
                .product
                .take_if(|action| matches!(action, ProductSave::SeirenEq(_)))
            {
                self.finish_seiren_eq_save(eq, false, request.window, cx);
            }
            self.continue_save_queue(false, cx);
            return false;
        }
        self.rebase_keyboard_actuation_save(&mut request, cx);
        if !self.rebase_mouse_dpi_stages_save(&mut request, cx) {
            if let Some(ProductSave::MouseDpiStages(dpi)) = request
                .product
                .take_if(|action| matches!(action, ProductSave::MouseDpiStages(_)))
            {
                self.finish_mouse_dpi_stages_save(dpi, false, request.window, cx);
            }
            self.continue_save_queue(false, cx);
            return false;
        }
        if let Err(error) = self.rebase_mouse_polling_save(&mut request, cx) {
            self.status = format!("??????????{error:#}");
            if let Some(ProductSave::MousePolling(polling)) = request
                .product
                .take_if(|action| matches!(action, ProductSave::MousePolling(_)))
            {
                polling.workspace.update(cx, |workspace, cx| {
                    workspace.finish_mouse_polling(
                        polling.scope,
                        polling.field,
                        polling.hz,
                        None,
                        cx,
                    )
                });
            }
            self.continue_save_queue(false, cx);
            return false;
        }
        if let Some(ProductSave::MouseLegacyDpi(dpi)) = &request.product {
            request.file.devices = self
                .devices
                .iter()
                .map(|entity| {
                    let mut saved = entity.read(cx).saved_snapshot(cx);
                    if entity.read(cx).identity(cx) == dpi.identity {
                        if let Some(profile) =
                            saved.profiles.iter_mut().find(|p| p.id == dpi.profile.id)
                        {
                            let settings = profile.source_settings.get_or_insert_with(|| {
                                dpi.profile
                                    .source_settings
                                    .clone()
                                    .unwrap_or_else(|| serde_json::json!({}))
                            });
                            settings["DPIStages"] = dpi.request.sensitivity.clone();
                        } else {
                            saved.profiles.push(dpi.profile.clone());
                        }
                    }
                    saved
                })
                .collect();
        }
        if let Some(ProductSave::MouseProfile(profile)) = &request.product
            && profile.scroll.is_none()
        {
            request.file.devices = self
                .devices
                .iter()
                .map(|entity| {
                    if entity.read(cx).identity(cx) == profile.identity {
                        let mut captured = profile.device.clone();
                        let previous = entity.read(cx).saved_snapshot(cx);
                        for key in [
                            "_mouse70_mapping_document",
                            "_keyboard515_mapping_document",
                            razer_device::keyboard_current_mapping::STORAGE_FIELD,
                        ] {
                            if let Some(document) = previous
                                .source_device_settings
                                .as_ref()
                                .and_then(|settings| settings.get(key))
                            {
                                captured
                                    .source_device_settings
                                    .get_or_insert_with(|| serde_json::json!({}))[key] =
                                    document.clone();
                            }
                        }
                        captured
                    } else {
                        entity.read(cx).saved_snapshot(cx)
                    }
                })
                .collect();
        }
        for device in &mut request.file.devices {
            if let Err(error) = mouse_dpi_70::prepare_mapping_document(device, &request.file.macros)
            {
                self.status = format!("Mouse profile preparation failed: {error:#}");
                if let Some(ProductSave::Leviathan(sound)) = request
                    .product
                    .take_if(|action| matches!(action, ProductSave::Leviathan(_)))
                {
                    self.finish_leviathan_save(sound, false, cx);
                }
                if let Some(ProductSave::KeyboardActuation(control)) = request
                    .product
                    .take_if(|action| matches!(action, ProductSave::KeyboardActuation(_)))
                {
                    self.finish_keyboard_actuation_save(control, false, request.window, cx);
                }
                if let Some(ProductSave::MouseCurrentDpi(dpi)) = request
                    .product
                    .take_if(|action| matches!(action, ProductSave::MouseCurrentDpi(_)))
                {
                    self.finish_current_dpi_save(dpi, false, request.window, cx);
                }
                if let Some(ProductSave::MouseCurrentPower(power)) = request
                    .product
                    .take_if(|action| matches!(action, ProductSave::MouseCurrentPower(_)))
                {
                    self.finish_current_power_save(power, false, request.window, cx);
                }
                if let Some(ProductSave::ControllerTrigger(trigger)) = request
                    .product
                    .take_if(|action| matches!(action, ProductSave::ControllerTrigger(_)))
                {
                    self.finish_controller_trigger_save(trigger, false, request.window, cx);
                }
                if let Some(ProductSave::ControllerLighting(lighting)) = request
                    .product
                    .take_if(|action| matches!(action, ProductSave::ControllerLighting(_)))
                {
                    self.finish_controller_lighting_save(lighting, false, request.window, cx);
                }
                if let Some(ProductSave::ControllerChroma(chroma)) = request
                    .product
                    .take_if(|action| matches!(action, ProductSave::ControllerChroma(_)))
                {
                    self.finish_controller_chroma_save(chroma, false, request.window, cx);
                }
                if let Some(ProductSave::SeirenEq(eq)) = request
                    .product
                    .take_if(|action| matches!(action, ProductSave::SeirenEq(_)))
                {
                    self.finish_seiren_eq_save(eq, false, request.window, cx);
                }
                if let Some(ProductSave::MouseDpiStages(dpi)) = request
                    .product
                    .take_if(|action| matches!(action, ProductSave::MouseDpiStages(_)))
                {
                    self.finish_mouse_dpi_stages_save(dpi, false, request.window, cx);
                }
                if let Some(ProductSave::MouseLegacyDpi(dpi)) = &request.product {
                    dpi.workspace.update(cx, |workspace, cx| {
                        workspace.finish_legacy_dpi(&dpi.request, cx)
                    });
                }
                if let Some(ProductSave::MouseProfile(profile)) = request
                    .product
                    .take_if(|action| matches!(action, ProductSave::MouseProfile(_)))
                {
                    self.finish_mouse_scroll_save(profile, false, request.window, cx);
                }
                if let Some(ProductSave::MousePolling(polling)) = &request.product {
                    polling.workspace.update(cx, |workspace, cx| {
                        workspace.finish_mouse_polling(
                            polling.scope,
                            polling.field,
                            polling.hz,
                            None,
                            cx,
                        );
                    });
                }
                cx.notify();
                self.continue_save_queue(false, cx);
                return false;
            }
        }
        let PreparedSave {
            window,
            file,
            mut product,
        } = request;
        let snapshot = file.clone();
        let save_owners = self.capture_save_owners(cx);
        let path = razer_storage::store_path();
        if product.is_none() {
            self.status = "正在保存到本机…".into();
        }
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
                        for snapshot in devices {
                            let Some(entity) =
                                this.saved_revision_owner(&save_owners, &snapshot, cx)
                            else {
                                continue;
                            };
                            let product_id = snapshot.product_id;
                            let mapping = snapshot
                                .source_device_settings
                                .as_ref()
                                .and_then(|settings| {
                                    settings.get(if snapshot.product_id == 756 {
                                        razer_device::keyboard_latest_restore::STORAGE_FIELD
                                    } else if snapshot.product_id == 679 {
                                        razer_device::keyboard_current_mapping::STORAGE_FIELD
                                    } else if snapshot.product_id == 515 {
                                        "_keyboard515_mapping_document"
                                    } else {
                                        "_mouse70_mapping_document"
                                    })
                                })
                                .cloned();
                            entity.update(cx, |d, cx| d.mark_saved(snapshot, cx));
                            if !this.close_requested {
                                if let Some(document) = mapping {
                                    if product_id == 756 {
                                        this.publish_keyboard_latest_restore(
                                            entity.clone(),
                                            document,
                                            cx,
                                        );
                                    } else {
                                        entity.update(cx, |d, cx| {
                                            d.publish_legacy_mapping(document, cx)
                                        });
                                    }
                                }
                            }
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
                        if product.is_none() {
                            this.status = "已保存到本机 · 尚未发送到设备".into();
                        }
                        if product.is_none()
                            && this.settings.read(cx).snapshot().notifications
                            && !this.close_requested
                        {
                            // Persistence completes without needing the original
                            // window; a closed window only suppresses its toast.
                            cx.defer(move |cx| {
                                let _ = window.update(cx, |_, window, cx| {
                                    window.push_notification("配置已保存到本机。", cx);
                                });
                            });
                        }
                        if let Some(ProductSave::MousePolling(polling)) =
                            product.take_if(|action| matches!(action, ProductSave::MousePolling(_)))
                        {
                            let owner = cx.entity().downgrade();
                            cx.defer(move |cx| {
                                let workspace = polling.workspace.clone();
                                let launched = window.update(cx, |_, window, cx| {
                                    let _ = owner.update(cx, |this, cx| {
                                        this.write_mouse_polling_device(
                                            polling.workspace,
                                            polling.scope,
                                            polling.field,
                                            polling.hz,
                                            window,
                                            cx,
                                        );
                                    });
                                });
                                if launched.is_err() {
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.finish_mouse_polling(
                                            polling.scope,
                                            polling.field,
                                            polling.hz,
                                            None,
                                            cx,
                                        );
                                    });
                                }
                            });
                        }
                        if let Some(ProductSave::MouseLegacyDpi(dpi)) = product
                            .take_if(|action| matches!(action, ProductSave::MouseLegacyDpi(_)))
                        {
                            let owner = cx.entity().downgrade();
                            cx.defer(move |cx| {
                                let workspace = dpi.workspace.clone();
                                let request = dpi.request.clone();
                                let launched = window.update(cx, |_, window, cx| {
                                    let _ = owner.update(cx, |this, cx| {
                                        this.submit_legacy_dpi_device(
                                            dpi.workspace,
                                            dpi.request,
                                            window,
                                            cx,
                                        );
                                    });
                                });
                                if launched.is_err() {
                                    workspace.update(cx, |workspace, cx| {
                                        workspace.finish_legacy_dpi(&request, cx)
                                    });
                                }
                            });
                        }
                        if let Some(ProductSave::MouseProfile(profile)) =
                            product.take_if(|action| matches!(action, ProductSave::MouseProfile(_)))
                        {
                            if profile.scroll.is_some() {
                                this.finish_mouse_scroll_save(profile, true, window, cx);
                            } else if !this.close_requested
                                && profile.apply
                                && this.devices.contains(&profile.workspace)
                                && profile.scope.is_some()
                                && profile.workspace.read(cx).mouse_polling_scope(cx)
                                    == profile.scope
                            {
                                if profile.workspace.read(cx).device(cx).product_id == 182 {
                                    this.initialize_mouse182_profiles(cx);
                                } else {
                                    profile.workspace.update(cx, |workspace, cx| {
                                        workspace.apply_legacy_performance_profile(
                                            profile.scope.expect("checked mouse scope"),
                                            cx,
                                        );
                                    });
                                }
                            }
                        }
                        if let Some(ProductSave::MouseCurrentDpi(dpi)) = product
                            .take_if(|action| matches!(action, ProductSave::MouseCurrentDpi(_)))
                        {
                            this.finish_current_dpi_save(dpi, true, window, cx);
                        }
                        if let Some(ProductSave::MouseDpiStages(dpi)) = product
                            .take_if(|action| matches!(action, ProductSave::MouseDpiStages(_)))
                        {
                            this.finish_mouse_dpi_stages_save(dpi, true, window, cx);
                        }
                        if let Some(ProductSave::MouseCurrentPower(power)) = product
                            .take_if(|action| matches!(action, ProductSave::MouseCurrentPower(_)))
                        {
                            this.finish_current_power_save(power, true, window, cx);
                        }
                        if let Some(ProductSave::ControllerTrigger(trigger)) = product
                            .take_if(|action| matches!(action, ProductSave::ControllerTrigger(_)))
                        {
                            this.finish_controller_trigger_save(trigger, true, window, cx);
                        }
                        if let Some(ProductSave::ControllerLighting(lighting)) = product
                            .take_if(|action| matches!(action, ProductSave::ControllerLighting(_)))
                        {
                            this.finish_controller_lighting_save(lighting, true, window, cx);
                        }
                        if let Some(ProductSave::ControllerChroma(chroma)) = product
                            .take_if(|action| matches!(action, ProductSave::ControllerChroma(_)))
                        {
                            this.finish_controller_chroma_save(chroma, true, window, cx);
                        }
                        if let Some(ProductSave::Leviathan(sound)) =
                            product.take_if(|action| matches!(action, ProductSave::Leviathan(_)))
                        {
                            this.finish_leviathan_save(sound, true, cx);
                        }
                        if let Some(ProductSave::SeirenEq(eq)) =
                            product.take_if(|action| matches!(action, ProductSave::SeirenEq(_)))
                        {
                            this.finish_seiren_eq_save(eq, true, window, cx);
                        }
                        if let Some(ProductSave::KeyboardActuation(control)) = product
                            .take_if(|action| matches!(action, ProductSave::KeyboardActuation(_)))
                        {
                            this.finish_keyboard_actuation_save(control, true, window, cx);
                        }
                        this.continue_save_queue(true, cx);
                    }
                    Err(error) => {
                        if let Some(ProductSave::MouseProfile(profile)) =
                            product.take_if(|action| matches!(action, ProductSave::MouseProfile(_)))
                        {
                            this.finish_mouse_scroll_save(profile, false, window, cx);
                        }
                        if let Some(ProductSave::MousePolling(polling)) =
                            product.take_if(|action| matches!(action, ProductSave::MousePolling(_)))
                        {
                            eprintln!("Mouse polling profile persistence failed: {error}");
                            polling.workspace.update(cx, |workspace, cx| {
                                workspace.finish_mouse_polling(
                                    polling.scope,
                                    polling.field,
                                    polling.hz,
                                    None,
                                    cx,
                                );
                            });
                        } else if let Some(ProductSave::MouseLegacyDpi(dpi)) = product
                            .take_if(|action| matches!(action, ProductSave::MouseLegacyDpi(_)))
                        {
                            eprintln!("Mouse DPI profile persistence failed: {error}");
                            dpi.workspace.update(cx, |workspace, cx| {
                                workspace.finish_legacy_dpi(&dpi.request, cx)
                            });
                            this.status = format!("DPI 本地保存失败：{error}");
                        } else {
                            this.status = format!("保存失败：{error}");
                        }
                        if let Some(ProductSave::MouseCurrentDpi(dpi)) = product
                            .take_if(|action| matches!(action, ProductSave::MouseCurrentDpi(_)))
                        {
                            this.finish_current_dpi_save(dpi, false, window, cx);
                        }
                        if let Some(ProductSave::MouseDpiStages(dpi)) = product
                            .take_if(|action| matches!(action, ProductSave::MouseDpiStages(_)))
                        {
                            this.status = format!("DPI 本地保存失败：{error}");
                            this.finish_mouse_dpi_stages_save(dpi, false, window, cx);
                        }
                        if let Some(ProductSave::MouseCurrentPower(power)) = product
                            .take_if(|action| matches!(action, ProductSave::MouseCurrentPower(_)))
                        {
                            this.finish_current_power_save(power, false, window, cx);
                        }
                        if let Some(ProductSave::ControllerTrigger(trigger)) = product
                            .take_if(|action| matches!(action, ProductSave::ControllerTrigger(_)))
                        {
                            this.finish_controller_trigger_save(trigger, false, window, cx);
                        }
                        if let Some(ProductSave::ControllerLighting(lighting)) = product
                            .take_if(|action| matches!(action, ProductSave::ControllerLighting(_)))
                        {
                            this.finish_controller_lighting_save(lighting, false, window, cx);
                        }
                        if let Some(ProductSave::ControllerChroma(chroma)) = product
                            .take_if(|action| matches!(action, ProductSave::ControllerChroma(_)))
                        {
                            this.finish_controller_chroma_save(chroma, false, window, cx);
                        }
                        if let Some(ProductSave::Leviathan(sound)) =
                            product.take_if(|action| matches!(action, ProductSave::Leviathan(_)))
                        {
                            this.finish_leviathan_save(sound, false, cx);
                        }
                        if let Some(ProductSave::SeirenEq(eq)) =
                            product.take_if(|action| matches!(action, ProductSave::SeirenEq(_)))
                        {
                            this.finish_seiren_eq_save(eq, false, window, cx);
                        }
                        if let Some(ProductSave::KeyboardActuation(control)) = product
                            .take_if(|action| matches!(action, ProductSave::KeyboardActuation(_)))
                        {
                            this.finish_keyboard_actuation_save(control, false, window, cx);
                        }
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
                let _ = handle.update(cx, |_, window, _| razer_tray::native::show(window));
            });
        }
        let queued = !self.pending_saves.is_empty();
        let auxiliary_pending = self.auxiliary_preferences_pending(cx);
        // A separately stored Studio draft is another requested write. Keep the
        // exit intent until its session reports completion; do not autosave edits.
        if succeeded
            && !queued
            && !auxiliary_pending
            && (self.studio_save_pending(cx)
                || self.keyboard_identity_cache_pending > 0
                || self.runtime.read(cx).shutdown_pending()
                || self.legacy_dpi_workers > 0
                || self.current_dpi_workers > 0
                || self.mouse_scroll_workers > 0
                || self.current_power_workers > 0
                || self.controller_trigger_workers > 0
                || self.controller_lighting_workers > 0
                || self.seiren_eq_workers > 0
                || (!self.seiren_audio_captures.is_empty()
                    || !self.seiren_audio_playbacks.is_empty())
                    && self.close_requested
                || (razer_pages::features::active_mapping_workers() > 0
                    || audio_mixer::active_local_workers() > 0)
                    && self.close_requested)
        {
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
        let file = self
            .workspace_file(
                self.devices
                    .iter()
                    .map(|d| d.read(cx).saved_snapshot(cx))
                    .collect(),
                cx,
            )
            .with_shortcuts(shortcuts.clone());
        let mut file = file.with_macros(macros.clone());
        let macros_pending = self.macro_library.read(cx).pending();
        let mut macro_devices = Vec::new();
        if macros_pending {
            for device in &mut file.devices {
                if matches!(device.product_id, 70 | 515 | 679 | 756) {
                    match mouse_dpi_70::prepare_mapping_document(device, &macros) {
                        Ok(()) => macro_devices.push(device.clone()),
                        Err(error) => {
                            eprintln!("Saved macro mapping preparation failed: {error:#}")
                        }
                    }
                }
            }
        }
        let save_owners = self.capture_save_owners(cx);
        let path = razer_storage::store_path();
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
                        for snapshot in macro_devices {
                            let Some(entity) =
                                this.saved_revision_owner(&save_owners, &snapshot, cx)
                            else {
                                continue;
                            };
                            let product_id = snapshot.product_id;
                            let key = if snapshot.product_id == 756 {
                                razer_device::keyboard_latest_restore::STORAGE_FIELD
                            } else if snapshot.product_id == 679 {
                                razer_device::keyboard_current_mapping::STORAGE_FIELD
                            } else if snapshot.product_id == 515 {
                                "_keyboard515_mapping_document"
                            } else {
                                "_mouse70_mapping_document"
                            };
                            let document = snapshot
                                .source_device_settings
                                .as_ref()
                                .and_then(|settings| settings.get(key))
                                .cloned();
                            entity.update(cx, |workspace, cx| workspace.mark_saved(snapshot, cx));
                            if !this.close_requested {
                                if let Some(document) = document {
                                    if product_id == 756 {
                                        this.publish_keyboard_latest_restore(
                                            entity.clone(),
                                            document,
                                            cx,
                                        );
                                    } else {
                                        entity.update(cx, |workspace, cx| {
                                            workspace.publish_legacy_mapping(document, cx)
                                        });
                                    }
                                }
                            }
                        }
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
            razer_tray::native::hide(window);
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
                razer_tray::native::show(window);
                self.navigate(Location::FirmwareUpdate, window, cx);
                return;
            }
        }
        // Closing is not a Save command. Drain only writes already requested;
        // unsubmitted profile/mapping drafts do not create an exit prompt.
        let runtime = self.runtime.clone();
        if self.runtime_exit_wait.is_none() {
            self.runtime_exit_wait = Some(cx.observe(&runtime, |this, runtime, cx| {
                if !runtime.read(cx).shutdown_pending() {
                    this.runtime_exit_wait = None;
                    if this.close_requested && this.save_task.is_none() {
                        this.continue_save_queue(true, cx);
                    }
                }
            }));
        }
        runtime.update(cx, |runtime, cx| runtime.prepare_exit(cx));
        self.cancel_hanbo_cooler();
        self.cancel_seiren_audio_captures(cx);
        self.cancel_seiren_audio_playbacks();
        self.retire_controller_lighting_owners(cx);
        for workspace in self.devices.clone() {
            workspace.update(cx, |workspace, cx| workspace.prepare_mouse_scroll_exit(cx));
            let eq = workspace.update(cx, |workspace, cx| workspace.prepare_seiren_eq_exit(cx));
            if let Some(request) = eq {
                self.persist_seiren_eq(workspace.clone(), request, window, cx);
            }
            let power =
                workspace.update(cx, |workspace, cx| workspace.prepare_current_power_exit(cx));
            if let Some(request) = power {
                self.persist_current_power(workspace.clone(), request, window, cx);
            }
            let current =
                workspace.update(cx, |workspace, cx| workspace.prepare_current_dpi_exit(cx));
            if let Some(request) = current {
                self.persist_current_dpi(workspace.clone(), request, window, cx);
            }
            let request =
                workspace.update(cx, |workspace, cx| workspace.prepare_legacy_dpi_exit(cx));
            if let Some(request) = request {
                self.persist_legacy_dpi(workspace, request, window, cx);
            }
        }
        if (razer_pages::features::active_mapping_workers() > 0
            || audio_mixer::active_local_workers() > 0)
            && self.mapping_exit_task.is_none()
        {
            self.mapping_exit_task = Some(cx.spawn(async move |this, cx| {
                while razer_pages::features::active_mapping_workers() > 0
                    || audio_mixer::active_local_workers() > 0
                {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(50))
                        .await;
                }
                let _ = this.update(cx, |this, cx| {
                    this.mapping_exit_task = None;
                    if this.close_requested && this.save_task.is_none() {
                        this.continue_save_queue(true, cx);
                    }
                });
            }));
        }
        if self.save_task.is_some()
            || self.keyboard_identity_cache_pending > 0
            || self.runtime.read(cx).shutdown_pending()
            || !self.pending_saves.is_empty()
            || self.studio_save_pending(cx)
            || self.legacy_dpi_workers > 0
            || self.current_dpi_workers > 0
            || self.mouse_scroll_workers > 0
            || self.current_power_workers > 0
            || self.controller_trigger_workers > 0
            || self.controller_lighting_workers > 0
            || self.seiren_eq_workers > 0
            || !self.seiren_audio_captures.is_empty()
            || !self.seiren_audio_playbacks.is_empty()
            || (razer_pages::features::active_mapping_workers() > 0
                || audio_mixer::active_local_workers() > 0)
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
        let Some(first_page) = razer_settings::products::first_local_page(pid) else {
            return;
        };
        if let Some(workspace) = self
            .devices
            .iter()
            .find(|workspace| {
                razer_settings::products::matching_connected_device(
                    workspace.read(cx).device(cx),
                    pid,
                    edition,
                    layout,
                ) && workspace.read(cx).has_local_page(cx)
            })
            .cloned()
        {
            let key = workspace.read(cx).identity(cx);
            self.navigate(Location::Device(key), window, cx);
            window.activate_window();
            return;
        }
        let Some(device) = razer_settings::products::preview_device(pid, edition, layout) else {
            return;
        };
        let serial = device.serial_number.clone();
        let is_new = !self
            .devices
            .iter()
            .any(|d| d.read(cx).device(cx).serial_number == serial);
        if is_new {
            self.add_device(device, window, cx);
        }
        let Some(workspace) = self
            .devices
            .iter()
            .find(|d| d.read(cx).device(cx).serial_number == serial)
            .cloned()
        else {
            return;
        };
        if !workspace.read(cx).has_local_page(cx) {
            return;
        }
        if is_new {
            workspace.update(cx, |workspace, cx| {
                workspace.set_source_page(first_page, window, cx)
            });
        }
        let key = workspace.read(cx).identity(cx);
        self.navigate(Location::Device(key), window, cx);
        window.activate_window();
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
            Location::Pairing(_) => "多设备配对".into(),
            Location::Tour(kind) => kind.title().into(),
            Location::FirmwareUpdate => "固件更新".into(),
            Location::ProfileMigration | Location::ProductSettingsMigration => {
                razer_i18n::t("PROFILE_MIGRATION").into()
            }
            Location::ProductSettings(_) => {
                razer_settings::settings_window::text("SETTINGS_HEADER")
            }
            // 原版窗口名就是 `macro`（Dashboard 模块 69937 的 `O="macro"`）。
            Location::Macro => razer_i18n::t("MACRO_SOURCE.TEXT_PROFILE_BAR_MACRO").into(),
            // Match the current hook's isExchangeEnabled=false until observed.
            Location::Chroma => razer_i18n::t_or("CHROMA_STUDIO", "Chroma Studio").into(),
            // 模块表把 `linkedGames` 指向 profiles 窗口，标题用它的文案 key。
            Location::Profiles => razer_i18n::t_or("LINKED_GAMES", "已关联的游戏"),
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
                            Location::ProfileMigration
                                | Location::ProductSettingsMigration
                                | Location::Profiles
                                | Location::Macro
                        ),
                        |navigation| {
                            navigation.child(
                                surface::asset_button(
                                    if self.location == Location::Profiles {
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
                                    razer_i18n::t("REFRESH"),
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
                                        if this.location == Location::Profiles {
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
                                        } else if this.location
                                            == Location::ProductSettingsMigration
                                        {
                                            this.refresh_product_settings_migration(cx);
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
                    .child(title),
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
                    ),
            )
            .into_any_element()
    }
    fn main_page(&self, page: Tab, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if page == Tab::Setting {
            return self.settings.clone().into_any_element();
        }
        let mut layout = razer_dashboard::MainLayout::new(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
            if page == Tab::Home { self.devices.len() } else { 0 },
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
        let resource_errors = razer_assets::resource_errors(cx);
        let keyboard_app_frame = match &self.location {
            Location::Device(key) => self
                .devices
                .iter()
                .find(|device| device.read(cx).identity(cx) == *key)
                .and_then(|device| device.read(cx).keyboard_app_frame_bounds_handle(cx)),
            _ => None,
        };
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
            Location::FirmwareUpdate => self
                .firmware_update
                .as_ref()
                .map(|(page, _)| page.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::ProductSettings(chroma) => self.product_settings_page(*chroma),
            Location::ProductSettingsMigration => self.product_settings_migration_page(),
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
            Location::Chroma => self.chroma_page.clone().into_any_element(),
            Location::Profiles => self
                .profiles_page
                .as_ref()
                .map(|page| page.clone().into_any_element())
                .unwrap_or_else(|| div().into_any_element()),
            Location::Pairing(name) => div()
                .id(format!("pairing-page-scroll-{name}"))
                .test_support()
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
            .child(
                v_flex()
                    .id("source-application-frame")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .on_prepaint(move |bounds, _, _| {
                        if let Some(handle) = &keyboard_app_frame {
                            handle.set(bounds);
                        }
                    })
                    // The product toolbar is inside the original iframe.
                    .child(self.toolbar(cx))
                    .child(
                        div()
                            .id("shell-content-viewport")
                            .relative()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .child(div().absolute().inset_0().child(content)),
                    ),
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
                    .gap_3()
                    .child(div().flex_1().min_w_0().child(self.status.clone()))
                    .when(!resource_errors.is_empty(), |bar| {
                        bar.child(format!("界面资源加载失败（{}）", resource_errors.len()))
                            .child(
                                Button::new("retry-interface-resources")
                                    .ghost()
                                    .xsmall()
                                    .label("重试资源")
                                    .tooltip(resource_errors.first().cloned().unwrap_or_default())
                                    .on_click(|_, _, cx| {
                                        if let Err(error) = razer_assets::retry_resources(cx) {
                                            eprintln!("资源重试失败：{error}");
                                        }
                                        cx.refresh_windows();
                                    }),
                            )
                    }),
            )
    }
}

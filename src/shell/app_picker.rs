//! `/rz-app-menu/`: source three-column picker, with explicit installation facts.
//! The shell owns opening pages and services; this view only emits requests.
use crate::{
    features::Choice,
    i18n,
    ui::{scroll::SourceScrollable as _, surface::css, theme::AppPickerColors},
};
use gpui_kit::base::{Button, Link, PopoverState, Positioner};
use gpui_kit::component::{
    select::{Select, SelectEvent, SelectState},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PickerApp {
    Synapse,
    Chroma,
    StreamerCompanion,
    VirtualRingLight,
}
impl PickerApp {
    const ALL: [Self; 4] = [
        Self::Chroma,
        Self::Synapse,
        Self::StreamerCompanion,
        Self::VirtualRingLight,
    ];
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::Synapse => "synapse",
            Self::Chroma => "chroma-app",
            Self::StreamerCompanion => "streamer-companion-app",
            Self::VirtualRingLight => "VirtualRingLight",
        }
    }
    fn label(self) -> String {
        i18n::t(match self {
            Self::Synapse => "SYNAPSE",
            Self::Chroma => "CHROMA_APP",
            Self::StreamerCompanion => "STREAMER_COMPANION_APP",
            Self::VirtualRingLight => "VIRTUAL_RING_LIGHT",
        })
    }
    fn icon(self) -> &'static str {
        match self {
            Self::Synapse => "synapse/module-synapse.svg",
            Self::Chroma => "synapse/module-chroma-studio.svg",
            Self::StreamerCompanion => "synapse/module-streamer-companion.svg",
            Self::VirtualRingLight => "synapse/module-virtual-ring-light.svg",
        }
    }
    fn recommendation(self) -> Option<Self> {
        match self {
            Self::Synapse => Some(Self::Chroma),
            Self::Chroma => Some(Self::Synapse),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PickerModule {
    LinkedGames,
    AddWifi,
    Macro,
    Alexa,
    Feedback,
    ProfileMigration,
    Armory,
    ChromaStudio,
    AudioVisualizer,
    ChromaConnect,
    SensaHd,
    PhilipsHue,
}
impl PickerModule {
    const SYNAPSE: [Self; 7] = [
        Self::LinkedGames,
        Self::AddWifi,
        Self::Macro,
        Self::Alexa,
        Self::Feedback,
        Self::ProfileMigration,
        Self::Armory,
    ];
    const CHROMA: [Self; 5] = [
        Self::ChromaStudio,
        Self::AudioVisualizer,
        Self::ChromaConnect,
        Self::SensaHd,
        Self::PhilipsHue,
    ];
    pub(super) fn key(self) -> &'static str {
        match self {
            Self::LinkedGames => "linkedGames",
            Self::AddWifi => "add-wifi-device",
            Self::Macro => "macro",
            Self::Alexa => "alexa",
            Self::Feedback => "feedback",
            Self::ProfileMigration => "syn3-profile-migration",
            Self::Armory => "armory",
            Self::ChromaStudio => "chroma-studio",
            Self::AudioVisualizer => "audio-visualizer",
            Self::ChromaConnect => "chroma-connect",
            Self::SensaHd => "sensa-hd",
            Self::PhilipsHue => "philips-hue",
        }
    }
    fn label(self, exchange: bool) -> String {
        i18n::t(match self {
            Self::LinkedGames => "LINKED_GAMES",
            Self::AddWifi => "ADD_WIFI_DEVICE",
            Self::Macro => "MACRO",
            Self::Alexa => "DASHBOARD_ALEXA",
            Self::Feedback => "FEEDBACK",
            Self::ProfileMigration => "PROFILE_MIGRATION",
            Self::Armory if exchange => "DASHBOARD_EXCHANGE",
            Self::Armory => "DASHBOARD_WORKSHOP",
            Self::ChromaStudio => "CHROMA_STUDIO",
            Self::AudioVisualizer => "DASHBOARD_AUDIO_VISUALIZER",
            Self::ChromaConnect => "DASHBOARD_CHROMA_CONNECT",
            Self::SensaHd => "DASHBOARD_SENSA",
            Self::PhilipsHue => "DASHBOARD_PHILIPS_HUE",
        })
    }
    fn icon(self, exchange: bool) -> &'static str {
        match self {
            Self::LinkedGames => "synapse/module-linked-games.svg",
            Self::AddWifi => "synapse/module-add-wifi.svg",
            Self::Macro => "synapse/module-macro.svg",
            Self::Alexa => "synapse/module-alexa.svg",
            Self::Feedback => "synapse/module-feedback.svg",
            Self::ProfileMigration => "synapse/module-profile-migration.svg",
            Self::Armory if exchange => "synapse/module-armory-exchange.svg",
            Self::Armory => "synapse/module-armory.svg",
            Self::ChromaStudio => "synapse/module-chroma-studio.svg",
            Self::AudioVisualizer => "synapse/module-audio-visualizer.svg",
            Self::ChromaConnect => "synapse/module-chroma-connect.svg",
            Self::SensaHd => "synapse/module-sensa-hd.svg",
            Self::PhilipsHue => "synapse/module-philips-hue.svg",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PickerDevice {
    product_id: u32,
    container_id: String,
    names: BTreeMap<String, String>,
    icon: SharedString,
    ready: bool,
    mixer_failed: bool,
    powered_off: bool,
    section_position: usize,
    launchable: bool,
    open_without_installation: bool,
}
impl PickerDevice {
    pub(super) fn new(
        product_id: u32,
        container_id: impl Into<String>,
        title: impl Into<String>,
        icon: impl Into<SharedString>,
    ) -> Self {
        Self {
            product_id,
            container_id: container_id.into(),
            names: BTreeMap::from([("en".into(), title.into())]),
            icon: icon.into(),
            ready: false,
            mixer_failed: false,
            powered_off: false,
            section_position: usize::MAX,
            launchable: false,
            open_without_installation: false,
        }
    }
    pub(super) fn localized_name(
        mut self,
        locale: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        self.names.insert(locale.into(), name.into());
        self
    }
    pub(super) fn ready(mut self, ready: bool) -> Self {
        self.ready = ready;
        self
    }
    pub(super) fn mixer_failed(mut self, failed: bool) -> Self {
        self.mixer_failed = failed;
        self
    }
    pub(super) fn powered_off(mut self, off: bool) -> Self {
        self.powered_off = off;
        self
    }
    pub(super) fn section_position(mut self, position: usize) -> Self {
        self.section_position = position;
        self
    }
    pub(super) fn launchable(mut self, enabled: bool) -> Self {
        self.launchable = enabled;
        self
    }
    pub(super) fn open_without_installation(mut self, enabled: bool) -> Self {
        self.open_without_installation = enabled;
        self
    }
    fn visible(&self) -> bool {
        (self.ready || self.open_without_installation)
            && !self.mixer_failed
            && !self.powered_off
            && self.container_id != "philips-hue"
    }
    fn label(&self) -> String {
        let locale = i18n::locale();
        self.names
            .get(&locale)
            .or_else(|| self.names.get(&locale.to_lowercase()))
            .or_else(|| self.names.get("en"))
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum RecommendationPhase {
    #[default]
    Idle,
    Downloading,
    Installing,
}
impl RecommendationPhase {
    fn busy(self) -> bool {
        matches!(self, Self::Downloading | Self::Installing)
    }
}

/// `None` means unread service state. `Some(empty)` is a confirmed empty result.
/// Local page availability is an independent capability and never implies installation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AppPickerCatalog {
    current_app: PickerApp,
    devices: Vec<PickerDevice>,
    installed_modules: Option<BTreeSet<String>>,
    native_apps: Option<BTreeSet<PickerApp>>,
    chroma_modules: Option<BTreeSet<String>>,
    uninstalling_modules: BTreeSet<String>,
    module_order: Vec<String>,
    device_order: Vec<String>,
    armory_version: Option<String>,
    armory_maintenance: bool,
    armory_exchange: bool,
    read_features: BTreeSet<String>,
    unread_features: bool,
    launchable_modules: BTreeSet<PickerModule>,
    bundled_modules: BTreeSet<PickerModule>,
    launchable_apps: BTreeSet<PickerApp>,
    installer_available: bool,
    recommendation_phase: RecommendationPhase,
}
impl AppPickerCatalog {
    pub(super) fn new(current_app: PickerApp) -> Self {
        Self {
            current_app,
            devices: vec![],
            installed_modules: None,
            native_apps: None,
            chroma_modules: None,
            uninstalling_modules: BTreeSet::new(),
            module_order: vec![],
            device_order: vec![],
            armory_version: None,
            armory_maintenance: false,
            armory_exchange: false,
            read_features: BTreeSet::new(),
            unread_features: false,
            launchable_modules: BTreeSet::new(),
            bundled_modules: BTreeSet::new(),
            launchable_apps: BTreeSet::new(),
            installer_available: false,
            recommendation_phase: RecommendationPhase::Idle,
        }
    }
    pub(super) fn devices(mut self, devices: Vec<PickerDevice>) -> Self {
        self.devices = devices;
        self
    }
    pub(super) fn installed_modules(
        mut self,
        keys: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.installed_modules = Some(keys.into_iter().map(Into::into).collect());
        self
    }
    pub(super) fn native_apps(mut self, apps: impl IntoIterator<Item = PickerApp>) -> Self {
        self.native_apps = Some(apps.into_iter().collect());
        self
    }
    pub(super) fn chroma_modules(
        mut self,
        keys: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.chroma_modules = Some(keys.into_iter().map(Into::into).collect());
        self
    }
    pub(super) fn uninstalling_modules(
        mut self,
        keys: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.uninstalling_modules = keys.into_iter().map(Into::into).collect();
        self
    }
    pub(super) fn module_order(
        mut self,
        keys: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.module_order = keys.into_iter().map(Into::into).collect();
        self
    }
    pub(super) fn device_order(
        mut self,
        keys: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.device_order = keys.into_iter().map(Into::into).collect();
        self
    }
    pub(super) fn armory_version(mut self, version: impl Into<String>) -> Self {
        self.armory_version = Some(version.into());
        self
    }
    pub(super) fn armory_maintenance(mut self, maintenance: bool) -> Self {
        self.armory_maintenance = maintenance;
        self
    }
    pub(super) fn armory_exchange(mut self, exchange: bool) -> Self {
        self.armory_exchange = exchange;
        self
    }
    pub(super) fn read_features(
        mut self,
        keys: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.read_features = keys.into_iter().map(Into::into).collect();
        self
    }
    pub(super) fn unread_features(mut self, unread: bool) -> Self {
        self.unread_features = unread;
        self
    }
    pub(super) fn launchable_modules(
        mut self,
        modules: impl IntoIterator<Item = PickerModule>,
    ) -> Self {
        self.launchable_modules = modules.into_iter().collect();
        self
    }
    /// Compiled native pages open locally regardless of external installation.
    pub(super) fn bundled_modules(
        mut self,
        modules: impl IntoIterator<Item = PickerModule>,
    ) -> Self {
        self.bundled_modules = modules.into_iter().collect();
        self
    }
    pub(super) fn launchable_apps(mut self, apps: impl IntoIterator<Item = PickerApp>) -> Self {
        self.launchable_apps = apps.into_iter().collect();
        self
    }
    pub(super) fn installer_available(mut self, available: bool) -> Self {
        self.installer_available = available;
        self
    }
    pub(super) fn recommendation_phase(mut self, phase: RecommendationPhase) -> Self {
        self.recommendation_phase = phase;
        self
    }
    fn status_unknown(&self) -> bool {
        self.installed_modules.is_none()
            || self.native_apps.is_none()
            || (self.current_app == PickerApp::Chroma && self.chroma_modules.is_none())
    }
    fn modules(&self) -> Vec<PickerModule> {
        let (source, installed): (&[PickerModule], _) = match self.current_app {
            PickerApp::Synapse => (&PickerModule::SYNAPSE, self.installed_modules.as_ref()),
            PickerApp::Chroma => (&PickerModule::CHROMA, self.chroma_modules.as_ref()),
            _ => return vec![],
        };
        let mut visible: Vec<_> = source
            .iter()
            .copied()
            .filter(|module| {
                self.bundled_modules.contains(module)
                    || self.launchable_modules.contains(module)
                    || (installed.is_some_and(|keys| keys.contains(module.key()))
                        && (self.current_app != PickerApp::Synapse
                            || !self.uninstalling_modules.contains(module.key()))
                        && (*module != PickerModule::Armory
                            || self
                                .armory_version
                                .as_ref()
                                .is_some_and(|version| !version.is_empty())))
            })
            .collect();
        let ordered: Vec<_> = self
            .module_order
            .iter()
            .filter_map(|key| {
                visible
                    .iter()
                    .find(|module| normalize_order(module.key()) == normalize_order(key))
                    .copied()
            })
            .collect();
        if !ordered.is_empty() {
            visible = unique(ordered);
            for module in source {
                if (self.bundled_modules.contains(module)
                    || self.launchable_modules.contains(module))
                    && !visible.contains(module)
                {
                    visible.push(*module);
                }
            }
        }
        visible
    }
    fn other_apps(&self) -> Vec<PickerApp> {
        PickerApp::ALL
            .into_iter()
            .filter(|app| {
                *app != self.current_app
                    && (self.launchable_apps.contains(app)
                        || (self
                            .installed_modules
                            .as_ref()
                            .is_some_and(|keys| keys.contains(app.key()))
                            && self
                                .native_apps
                                .as_ref()
                                .is_some_and(|apps| apps.contains(app))))
            })
            .collect()
    }
    fn ordered_devices(&self) -> Vec<&PickerDevice> {
        if self.current_app != PickerApp::Synapse {
            return vec![];
        }
        let mut visible: Vec<_> = self
            .devices
            .iter()
            .filter(|device| device.visible())
            .collect();
        visible.sort_by_key(|device| device.section_position);
        if self.device_order.len() == visible.len() && !visible.is_empty() {
            let mut ordered = Vec::new();
            for key in &self.device_order {
                let pid = key
                    .split_once('-')
                    .and_then(|(pid, _)| pid.parse::<u32>().ok());
                if let Some(device) =
                    pid.and_then(|pid| visible.iter().find(|device| device.product_id == pid))
                {
                    if !ordered.iter().any(|existing: &&PickerDevice| {
                        existing.container_id == device.container_id
                            && existing.product_id == device.product_id
                    }) {
                        ordered.push(*device);
                    }
                }
            }
            // Incomplete/stale host order must not erase a connected device.
            if ordered.len() == visible.len() {
                visible = ordered;
            }
        }
        visible
    }
    fn sections(&self) -> Vec<PickerSection> {
        let devices = self
            .ordered_devices()
            .into_iter()
            .map(|device| PickerItem {
                id: format!(
                    "app-picker-device-{}-{}",
                    device.product_id, device.container_id
                ),
                title: device.label(),
                icon: device.icon.clone(),
                title_case: false,
                target: PickerRequest::Open(PickerTarget::Device {
                    product_id: device.product_id,
                    container_id: device.container_id.clone(),
                }),
                reason: (!device.launchable).then(|| "此设备的页面尚未接入。".into()),
                is_new: false,
                busy: false,
            })
            .collect();
        let modules = self
            .modules()
            .into_iter()
            .map(|module| PickerItem {
                id: format!("app-picker-module-{}", module.key()),
                title: module.label(self.armory_exchange),
                icon: module.icon(self.armory_exchange).into(),
                title_case: true,
                target: if module == PickerModule::AddWifi {
                    PickerRequest::AddWifi
                } else {
                    PickerRequest::Open(PickerTarget::Module(module))
                },
                reason: if module == PickerModule::Armory && self.armory_maintenance {
                    Some(i18n::t("ARMORY_MAINTENANCE_DESC"))
                } else {
                    (!self.bundled_modules.contains(&module)
                        && !self.launchable_modules.contains(&module))
                    .then(|| "此模块的窗口服务尚未连接。".into())
                },
                is_new: module == PickerModule::Armory && !self.read_features.contains("armory"),
                busy: false,
            })
            .collect();
        let other_apps = self.other_apps();
        let apps = other_apps
            .iter()
            .copied()
            .map(|app| PickerItem {
                id: format!("app-picker-app-{}", app.key()),
                title: app.label(),
                icon: app.icon().into(),
                title_case: true,
                target: PickerRequest::Open(PickerTarget::App(app)),
                reason: (!self.launchable_apps.contains(&app))
                    .then(|| "此应用的窗口服务尚未连接。".into()),
                is_new: false,
                busy: false,
            })
            .collect();
        let recommended = if self.status_unknown() {
            None
        } else {
            self.current_app
                .recommendation()
                .filter(|app| !other_apps.contains(app))
        };
        let recommendations = recommended
            .into_iter()
            .map(|app| {
                let phase = self.recommendation_phase;
                PickerItem {
                    id: format!("app-picker-recommend-{}", app.key()),
                    title: app.label(),
                    icon: app.icon().into(),
                    title_case: true,
                    target: PickerRequest::Install(app),
                    is_new: false,
                    busy: phase.busy(),
                    reason: if phase.busy() {
                        Some("应用正在安装。".into())
                    } else if !self.installer_available {
                        Some("应用安装服务尚未连接。".into())
                    } else {
                        None
                    },
                }
            })
            .collect();
        [
            PickerSection::new("devices", "DEVICE", devices),
            PickerSection::new("modules", "MODULES_HEADER", modules),
            PickerSection::new("apps", "OTHER_INSTALLED_APPS", apps),
            PickerSection::new("recommended", "RECOMMENDED_APPS", recommendations),
        ]
        .into_iter()
        .filter(|section| !section.items.is_empty())
        .collect()
    }
}

fn normalize_order(key: &str) -> String {
    key.replace('-', "").to_lowercase()
}
fn unique<T: Clone + Ord>(items: Vec<T>) -> Vec<T> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum PickerTarget {
    Device {
        product_id: u32,
        container_id: String,
    },
    Module(PickerModule),
    App(PickerApp),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum AppPickerEvent {
    Open(PickerTarget),
    AddWifiDevice,
    Install(PickerApp),
    DiscoverMore,
    ReadFeature(String),
    UnreadCleared,
}
#[derive(Clone)]
enum PickerRequest {
    Open(PickerTarget),
    AddWifi,
    Install(PickerApp),
}
#[derive(Clone)]
struct PickerItem {
    id: String,
    title: String,
    icon: SharedString,
    title_case: bool,
    target: PickerRequest,
    reason: Option<String>,
    is_new: bool,
    busy: bool,
}
struct PickerSection {
    id: &'static str,
    title_key: &'static str,
    items: Vec<PickerItem>,
}
impl PickerSection {
    fn new(id: &'static str, title_key: &'static str, items: Vec<PickerItem>) -> Self {
        Self {
            id,
            title_key,
            items,
        }
    }
}

pub(super) struct AppPicker {
    catalog: AppPickerCatalog,
    popup: Entity<PopoverState>,
    trigger_focus: FocusHandle,
    trigger_bounds: Bounds<Pixels>,
    tile_focus: BTreeMap<String, FocusHandle>,
    scroll: ScrollHandle,
    has_unread: bool,
    _popup_observer: Subscription,
    _activation_observer: Subscription,
}
impl EventEmitter<AppPickerEvent> for AppPicker {}
impl AppPicker {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let popup = cx.new(|cx| PopoverState::new(false, cx));
        let observer = cx.observe(&popup, |_, _, cx| cx.notify());
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active()
                && !this.trigger_focus.contains_focused(window, cx)
                && !this.popup.focus_handle(cx).contains_focused(window, cx)
            {
                this.dismiss(window, cx);
            }
        });
        Self {
            catalog: AppPickerCatalog::new(PickerApp::Synapse),
            popup,
            trigger_focus: cx.focus_handle(),
            trigger_bounds: Bounds::default(),
            tile_focus: BTreeMap::new(),
            scroll: ScrollHandle::new(),
            has_unread: false,
            _popup_observer: observer,
            _activation_observer: activation,
        }
    }
    pub(super) fn set_catalog(
        &mut self,
        catalog: AppPickerCatalog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.catalog == catalog {
            return;
        }
        self.catalog = catalog;
        let items: Vec<_> = self
            .catalog
            .sections()
            .into_iter()
            .flat_map(|section| section.items)
            .collect();
        let removed_focus = self.tile_focus.iter().any(|(id, focus)| {
            focus.is_focused(window)
                && !items
                    .iter()
                    .any(|item| item.id == *id && item.reason.is_none())
        });
        self.tile_focus
            .retain(|id, _| items.iter().any(|item| item.id == *id));
        for item in items {
            self.tile_focus
                .entry(item.id)
                .or_insert_with(|| cx.focus_handle());
        }
        self.has_unread = self.catalog.unread_features;
        if removed_focus && self.popup.read(cx).is_open() {
            self.popup.focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }
    pub(super) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.popup.update(cx, |popup, cx| popup.dismiss(window, cx));
    }
    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.popup.read(cx).is_open() {
            self.dismiss(window, cx);
            return;
        }
        self.trigger_focus.focus(window, cx);
        if self.has_unread {
            self.has_unread = false;
            self.catalog.unread_features = false;
            cx.emit(AppPickerEvent::UnreadCleared);
        }
        self.popup.update(cx, |popup, cx| popup.show(window, cx));
        if let Some(focus) = self
            .catalog
            .sections()
            .iter()
            .flat_map(|section| &section.items)
            .find(|item| item.reason.is_none())
            .and_then(|item| self.tile_focus.get(&item.id))
        {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn activate(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.popup.read(cx).is_open() {
            return;
        }
        // Recheck current facts, including capability/maintenance changes since paint.
        let Some(item) = self
            .catalog
            .sections()
            .into_iter()
            .flat_map(|section| section.items)
            .find(|item| item.id == id && item.reason.is_none())
        else {
            return;
        };
        self.dismiss(window, cx);
        if item.is_new {
            self.catalog.read_features.insert("armory".into());
            cx.emit(AppPickerEvent::ReadFeature("armory".into()));
        }
        cx.emit(match item.target {
            PickerRequest::Open(target) => AppPickerEvent::Open(target),
            PickerRequest::AddWifi => AppPickerEvent::AddWifiDevice,
            PickerRequest::Install(app) => AppPickerEvent::Install(app),
        });
    }
    fn content(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let sections = self.catalog.sections();
        let empty = !sections.iter().any(|section| section.id != "recommended");
        let unknown = self.catalog.status_unknown();
        let trigger_bounds = self.trigger_bounds;
        let popup = self.popup.clone();
        let popup_focus = self.popup.focus_handle(cx);
        // The source's innerHeight belongs to the frontend, below the host
        // tab strip. Keep its 40px bottom clearance in the native full window.
        let max_height =
            (window.viewport_size().height - self.trigger_bounds.top() - window.rem_size() * 5.)
                .max(window.rem_size() * 3.);
        let groups = sections
            .into_iter()
            .map(|section| {
                let items = section
                    .items
                    .into_iter()
                    .map(|item| {
                        let id = item.id.clone();
                        let focus = self.tile_focus.get(&id).cloned();
                        let button = Button::new(SharedString::from(id.clone()))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.activate(&id, window, cx)
                            }))
                            .when_some(focus, |button, focus| button.track_focus(&focus));
                        PickerTile { item, button }
                    })
                    .collect::<Vec<_>>();
                div()
                    .id(section.id)
                    .test_support()
                    .flex_shrink_0()
                    .py(css(10.))
                    .border_b_1()
                    .border_color(AppPickerColors::border())
                    .child(
                        div()
                            .mb(css(10.))
                            .text_size(css(12.))
                            .line_height(relative(1.2))
                            .text_color(AppPickerColors::title())
                            .child(i18n::t(section.title_key).to_uppercase()),
                    )
                    .child(div().flex().flex_wrap().items_start().children(items))
            })
            .collect::<Vec<_>>();
        div()
            .id("app-picker-popup")
            .test_support()
            .role(Role::Dialog)
            .aria_label(i18n::t("MORE"))
            .track_focus(&popup_focus)
            .key_context("Popover")
            .on_action(
                cx.listener(|this, _: &gpui_kit::base::actions::Cancel, window, cx| {
                    this.dismiss(window, cx)
                }),
            )
            .on_mouse_down_out(move |event, window, cx| {
                if !trigger_bounds.contains(&event.position) {
                    popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                }
            })
            .w(css(410.))
            .max_w(window.viewport_size().width - window.rem_size())
            .max_h(max_height)
            .border_1()
            .border_color(AppPickerColors::border())
            .bg(AppPickerColors::surface())
            .text_color(AppPickerColors::text())
            .font_family("Roboto")
            .font_weight(FontWeight::NORMAL)
            .text_size(css(16.))
            .child(div().px(css(15.)).children(groups))
            .when(unknown && empty, |view| {
                view.child(
                    div()
                        .id("app-picker-unknown")
                        .test_support()
                        .px(css(15.))
                        .pt(css(10.))
                        .text_size(css(12.))
                        .text_color(AppPickerColors::title())
                        .child("应用安装状态尚未读取。"),
                )
            })
            .child(
                div()
                    .pt(css(10.))
                    .px(css(15.))
                    .pb(css(20.))
                    .when(empty && !unknown, |footer| {
                        footer.child(
                            img("synapse/app-picker-empty.png")
                                .w_full()
                                .h(css(150.))
                                .mb(css(10.))
                                .object_fit(ObjectFit::Cover),
                        )
                    })
                    .child(DiscoverLink {
                        link: Link::new("app-picker-discover")
                            .href("https://razer.com/pc/software")
                            .accessibility_label(i18n::t("DISCOVER_MORE_APPS"))
                            .on_activate(cx.listener(|this, _, window, cx| {
                                this.dismiss(window, cx);
                                cx.emit(AppPickerEvent::DiscoverMore);
                            })),
                    }),
            )
            .scrollable_y()
            .track_scroll(&self.scroll)
            .into_any_element()
    }
}
impl Render for AppPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.popup.read(cx).is_open();
        let tooltip = i18n::t("MORE");
        let position = self.trigger_bounds.origin
            + point(
                self.trigger_bounds.size.width,
                window.rem_size() * (40. / 16.),
            );
        let content = open.then(|| self.content(window, cx));
        div()
            .id("app-picker")
            .flex_shrink_0()
            .child(
                Button::new("app-picker-trigger")
                    .accessibility_label(i18n::t("MORE"))
                    .track_focus(&self.trigger_focus)
                    .w(css(46.))
                    .h(css(38.))
                    .p_0()
                    .rounded_none()
                    .bg(if open {
                        AppPickerColors::hover()
                    } else {
                        cx.theme().transparent
                    })
                    .hover(|style| style.bg(AppPickerColors::hover()))
                    .active(|style| style.bg(AppPickerColors::hover()))
                    .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
                    .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                    .child(
                        div()
                            .relative()
                            .size(css(20.))
                            .child(img("synapse/app-picker.svg").size_full())
                            .when(self.has_unread, |icon| {
                                icon.child(
                                    div()
                                        .absolute()
                                        .right(css(-2.))
                                        .top(css(-2.))
                                        .size(css(6.))
                                        .rounded_full()
                                        .bg(cx.theme().primary),
                                )
                            }),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.toggle(window, cx)))
                    .on_prepaint({
                        let owner = cx.entity().downgrade();
                        move |bounds, _, cx| {
                            _ = owner.update(cx, |this, cx| {
                                if this.trigger_bounds != bounds {
                                    this.trigger_bounds = bounds;
                                    cx.notify();
                                }
                            });
                        }
                    }),
            )
            .when_some(content, |view, content| {
                view.child(
                    deferred(
                        Positioner::corner(Anchor::TopRight, position)
                            .margin(window.rem_size() * 0.5)
                            .occlude()
                            .child(content),
                    )
                    .with_priority(gpui_kit::base::POPUP_PRIORITY),
                )
            })
    }
}

#[derive(Default)]
struct TileInteraction {
    hovered: bool,
}
#[derive(IntoElement)]
struct PickerTile {
    item: PickerItem,
    button: Button,
}
impl RenderOnce for PickerTile {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let item = self.item;
        let id: SharedString = item.id.clone().into();
        let state = window.use_keyed_state((ElementId::from(id.clone()), "hover"), cx, |_, _| {
            TileInteraction::default()
        });
        let long = title_is_long(&item.title, window);
        let disabled = item.reason.is_some();
        let hovered = state.read(cx).hovered && !disabled;
        let label = if item.title_case {
            source_title_case(&item.title)
        } else {
            item.title.clone()
        };
        let content = div()
            .id("item-content")
            .w_full()
            .h_auto()
            .flex()
            .flex_col()
            .items_center()
            .justify_start()
            .flex_shrink_0()
            .py(css(10.))
            .px(css(3.))
            .rounded(css(5.))
            .text_color(AppPickerColors::text())
            .font_family("Roboto")
            .text_size(css(14.))
            .line_height(relative(1.2))
            .when(!disabled, |content| {
                content.hover(|style| style.bg(AppPickerColors::hover()))
            })
            .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                state.hovered = *hovered;
                cx.notify();
            }))
            .child(
                div()
                    .relative()
                    .w_full()
                    .mb(css(5.))
                    .flex()
                    .justify_center()
                    .child(div().size(css(40.)).when(!item.icon.is_empty(), |slot| {
                        slot.child(img(item.icon).size_full().object_fit(ObjectFit::Contain))
                    }))
                    .when(item.is_new, |icon| {
                        icon.child(
                            div()
                                .absolute()
                                .right(css(22.))
                                .top(css(-12.))
                                .border_2()
                                .border_color(AppPickerColors::badge_text())
                                .rounded(css(10.))
                                .py(css(2.))
                                .px(css(6.))
                                .bg(AppPickerColors::badge_background())
                                .text_color(AppPickerColors::badge_text())
                                .text_size(css(10.))
                                .child("New"),
                        )
                    }),
            )
            .child(
                div()
                    .w(css(110.))
                    .text_center()
                    .when(item.busy, |text| text.flex().flex_wrap().justify_center())
                    .child(
                        div()
                            .max_w(css(110.))
                            .when(long && !hovered, |text| text.h(css(33.)).line_clamp(2))
                            .child(label.clone()),
                    )
                    .when(item.busy, |text| {
                        text.child(
                            div().ml(css(5.)).child(
                                svg()
                                    .path("synapse/app-picker-spinner.svg")
                                    .size(css(16.))
                                    .text_color(cx.theme().primary)
                                    .with_animation(
                                        "app-picker-install-spinner",
                                        Animation::new(Duration::from_secs(1))
                                            .repeat()
                                            .with_easing(linear),
                                        |icon, value| {
                                            icon.with_transformation(Transformation::rotate(
                                                percentage(value),
                                            ))
                                        },
                                    ),
                            ),
                        )
                    }),
            );
        // `.item` owns activation across its full 96px slot; `.item-content`
        // alone owns the rounded, naturally sized hover background.
        let mut button = self
            .button
            .accessibility_label(label)
            .disabled(disabled)
            .w_full()
            .h(css(96.))
            .p_0()
            .flex_col()
            .items_stretch()
            .justify_start()
            .opacity(if disabled { 0.3 } else { 1. })
            .when(!disabled, |button| {
                button
                    .active(|style| style.opacity(0.5))
                    .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
            })
            .child(content);
        if let Some(reason) = item.reason {
            button = button
                .aria_description(reason.clone())
                .tooltip(move |window, cx| Tooltip::new(reason.clone()).build(window, cx));
        }
        // Deferred paint preserves the 96px grid row while an expanded title
        // appears above the next row, matching source hover z-index:100.
        div()
            .id(SharedString::from(format!("{id}-slot")))
            .test_support()
            .flex_shrink_0()
            .flex_basis(relative(0.33))
            .h(css(96.))
            .child(if hovered {
                deferred(button)
                    .with_priority(gpui_kit::base::POPUP_PRIORITY + 1)
                    .into_any_element()
            } else {
                button.into_any_element()
            })
    }
}

fn title_is_long(title: &str, window: &Window) -> bool {
    let mut font = window.text_style().font();
    font.family = "Roboto".into();
    font.weight = FontWeight::NORMAL;
    window
        .text_system()
        .shape_line(
            title.to_owned().into(),
            window.rem_size() * (14. / 16.),
            &[TextRun {
                len: title.len(),
                font,
                color: AppPickerColors::text(),
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        )
        .width
        > window.rem_size() * (110. / 16.)
}
// Source lowercases the entire label, then capitalizes ASCII /\b\w+\b/ words.
// Non-ASCII text must not be split or uppercased by a locale-agnostic title helper.
fn source_title_case(title: &str) -> String {
    let text = title.trim().to_lowercase();
    let mut result = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, output: &mut String| {
        if word == "hd" {
            output.push_str("HD");
        } else if !word.is_empty() {
            let mut chars = word.chars();
            output.extend(chars.next().unwrap().to_uppercase());
            output.extend(chars);
        }
        word.clear();
    };
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            word.push(ch);
        } else {
            flush(&mut word, &mut result);
            result.push(ch);
        }
    }
    flush(&mut word, &mut result);
    result
}

#[derive(Default)]
struct LinkInteraction {
    hovered: bool,
    pressed: bool,
}
#[derive(IntoElement)]
struct DiscoverLink {
    link: Link,
}
impl RenderOnce for DiscoverLink {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_kit::base::motion::{self, Transition};
        let state = window.use_keyed_state("app-picker-discover-state", cx, |_, _| {
            LinkInteraction::default()
        });
        let interaction = state.read(cx);
        let color = if interaction.hovered {
            cx.theme().primary
        } else {
            AppPickerColors::text()
        };
        let opacity = if interaction.pressed && interaction.hovered {
            0.7
        } else {
            1.
        };
        let transition = Transition::new(Duration::from_millis(300));
        let color = motion::transition(
            "app-picker-link-color",
            color,
            transition.clone(),
            window,
            cx,
        );
        let opacity =
            motion::transition("app-picker-link-opacity", opacity, transition, window, cx);
        let link = self
            .link
            .text_size(css(14.))
            .line_height(css(20.))
            .text_color(color)
            .opacity(opacity)
            .underline()
            .flex()
            .items_center()
            .focus_visible(|style| style.text_color(cx.theme().primary))
            .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                state.hovered = *hovered;
                if !hovered {
                    state.pressed = false;
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = true;
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                window.listener_for(&state, |state, _, _, cx| {
                    state.pressed = false;
                    cx.notify();
                }),
            )
            .child(i18n::t("DISCOVER_MORE_APPS"))
            .child(
                svg()
                    .path("synapse/external-link.svg")
                    .size(css(20.))
                    .ml(css(5.))
                    .flex_shrink_0()
                    .text_color(color),
            );
        div()
            .id("app-picker-discover-target")
            .test_support()
            .flex()
            .items_start()
            .child(link)
    }
}

/// Isolated source-state preview. It never changes production installation facts.
pub(super) fn open_preview(window: &mut Window, cx: &mut App) {
    let preview = cx.new(|cx| AppPickerPreview::new(window, cx));
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("更多应用 · 界面预览")
            .width(window.rem_size() * (560. / 16.))
            .child(preview.clone())
    });
}
const PREVIEW_SCENES: [(&str, &str); 10] = [
    ("synapse", "Synapse · 已安装模块"),
    ("chroma", "Chroma · 已安装模块"),
    ("empty", "无设备和模块"),
    ("unknown", "安装状态未读取"),
    ("maintenance", "Armory 维护中"),
    ("recommended", "推荐应用可安装"),
    ("downloading", "推荐应用正在下载"),
    ("installing", "推荐应用正在安装"),
    ("filtered", "设备条件与自定义排序"),
    ("unavailable", "已安装但窗口服务未连接"),
];
fn preview_catalog(scene: &str) -> AppPickerCatalog {
    let current = if scene == "chroma" {
        PickerApp::Chroma
    } else {
        PickerApp::Synapse
    };
    if scene == "unknown" {
        return AppPickerCatalog::new(current);
    }
    let mut catalog = AppPickerCatalog::new(current)
        .installed_modules(std::iter::empty::<String>())
        .native_apps([])
        .chroma_modules(std::iter::empty::<String>());
    if scene == "empty" {
        return catalog;
    }
    let devices = vec![
        PickerDevice::new(
            182,
            "preview-mouse",
            "Razer DeathAdder V3 Pro",
            preview_device_icon(182),
        )
        .ready(true)
        .section_position(0)
        .launchable(true),
        PickerDevice::new(
            653,
            "preview-keyboard",
            "Razer BlackWidow V4 Pro",
            preview_device_icon(653),
        )
        .ready(true)
        .section_position(1)
        .launchable(true),
        PickerDevice::new(
            777,
            "preview-headset",
            "Razer Kraken BT",
            preview_device_icon(777),
        )
        .ready(true)
        .section_position(2)
        .launchable(true),
    ];
    let keys = PickerModule::SYNAPSE
        .into_iter()
        .map(|module| module.key().to_string())
        .chain([
            PickerApp::Chroma.key().into(),
            PickerApp::Synapse.key().into(),
            PickerApp::StreamerCompanion.key().into(),
            PickerApp::VirtualRingLight.key().into(),
        ]);
    catalog = catalog
        .devices(devices)
        .installed_modules(keys)
        .native_apps(PickerApp::ALL)
        .chroma_modules(PickerModule::CHROMA.map(PickerModule::key))
        .armory_version("preview")
        .unread_features(true)
        .launchable_modules(
            PickerModule::SYNAPSE
                .into_iter()
                .chain(PickerModule::CHROMA),
        )
        .launchable_apps(PickerApp::ALL);
    match scene {
        "maintenance" => catalog.armory_maintenance(true).armory_exchange(true),
        "recommended" => catalog
            .native_apps([])
            .launchable_apps([])
            .installer_available(true),
        "downloading" => catalog
            .native_apps([])
            .launchable_apps([])
            .recommendation_phase(RecommendationPhase::Downloading),
        "installing" => catalog
            .native_apps([])
            .launchable_apps([])
            .recommendation_phase(RecommendationPhase::Installing),
        "unavailable" => catalog.launchable_modules([]).launchable_apps([]),
        "filtered" => {
            let mut devices = catalog.devices.clone();
            devices.push(
                PickerDevice::new(
                    769,
                    "philips-hue",
                    "Philips Hue",
                    "synapse/module-philips-hue.svg",
                )
                .ready(true),
            );
            devices.push(
                PickerDevice::new(182, "off", "Powered off", preview_device_icon(182))
                    .ready(true)
                    .powered_off(true),
            );
            devices.push(PickerDevice::new(
                653,
                "not-ready",
                "Not ready",
                preview_device_icon(653),
            ));
            devices.push(
                PickerDevice::new(
                    777,
                    "mixer-failed",
                    "Mixer check failed",
                    preview_device_icon(777),
                )
                .ready(true)
                .mixer_failed(true),
            );
            catalog
                .devices(devices)
                .device_order(["777-preview", "182-preview", "653-preview"])
                .uninstalling_modules(["macro"])
                .module_order([
                    "alexa",
                    "macro",
                    "linked-games",
                    "syn3-profile-migration",
                    "armory",
                ])
                .read_features(["armory"])
        }
        _ => catalog,
    }
}
fn preview_device_icon(pid: u32) -> &'static str {
    crate::resources::dashboard_image(pid, 0, 1).unwrap_or("synapse/module-synapse.svg")
}
struct AppPickerPreview {
    picker: Entity<AppPicker>,
    selected: Entity<SelectState<Vec<Choice>>>,
    last_request: Option<String>,
    _selection: Subscription,
    _requests: Subscription,
}
impl AppPickerPreview {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let picker = cx.new(|cx| AppPicker::new(window, cx));
        picker.update(cx, |picker, cx| {
            picker.set_catalog(preview_catalog("synapse"), window, cx)
        });
        let choices = PREVIEW_SCENES
            .iter()
            .map(|(key, label)| Choice::new(*key, *label))
            .collect::<Vec<_>>();
        let selected = cx.new(|cx| SelectState::new(choices, Some(IndexPath::new(0)), window, cx));
        let selection = cx.subscribe_in(
            &selected,
            window,
            |this, _, event: &SelectEvent<Vec<Choice>>, window, cx| {
                if let SelectEvent::Confirm(Some(choice)) = event {
                    this.picker.update(cx, |picker, cx| {
                        picker.dismiss(window, cx);
                        picker.set_catalog(preview_catalog(choice), window, cx);
                    });
                    this.last_request = None;
                    cx.notify();
                }
            },
        );
        let requests = cx.subscribe(&picker, |this, _, event: &AppPickerEvent, cx| {
            this.last_request = Some(match event {
                AppPickerEvent::Open(PickerTarget::Module(module)) => {
                    format!("预览选择：{}", source_title_case(&module.label(false)))
                }
                AppPickerEvent::Open(PickerTarget::App(app)) | AppPickerEvent::Install(app) => {
                    format!("预览选择：{}", app.label())
                }
                AppPickerEvent::Open(PickerTarget::Device { product_id, .. }) => {
                    format!("预览选择：设备 {product_id}")
                }
                AppPickerEvent::AddWifiDevice => "预览选择：添加 Wi-Fi 设备".into(),
                AppPickerEvent::DiscoverMore => "预览选择：发现更多应用".into(),
                AppPickerEvent::ReadFeature(_) => "预览中的新功能已读。".into(),
                AppPickerEvent::UnreadCleared => "预览中的工具栏未读标记已清除。".into(),
            });
            cx.notify();
        });
        Self {
            picker,
            selected,
            last_request: None,
            _selection: selection,
            _requests: requests,
        }
    }
}
impl Render for AppPickerPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().gap(css(16.)).text_size(css(14.))
            .child("选择一个原版状态，然后点击右侧“更多”图标查看。这里的设备和安装状态仅用于界面预览。")
            .child(Select::new(&self.selected).w_full())
            .child(h_flex().justify_end().w_full().h(css(38.)).bg(AppPickerColors::surface()).child(self.picker.clone()))
            .child(div().min_h(css(36.)).text_color(cx.theme().muted_foreground).child(self.last_request.clone().unwrap_or_else(|| "预览操作不会启动录音、安装程序或外部应用。".into())))
    }
}

#[cfg(test)]
#[path = "app_picker_tests.rs"]
mod tests;

//! Shell-facing owner for the original adapters and source-specific products.
//! Selecting a registered product never creates another product's controls.
use super::{
    DeviceWorkspace, WorkspaceEvent, chroma_product,
    display_mode_roots::{self, DisplayModeRoot},
    source_workspace::SourceProductWorkspace,
    workspace::Continue,
};
use crate::{model::Device, nav::Tab};
use gpui_kit::*;

#[path = "profile_collection.rs"]
mod profile_collection;
pub(super) use profile_collection::{
    Action as ProfileCollectionAction, edit as edit_profile_collection,
};

#[cfg(test)]
#[path = "product_workspace_tests.rs"]
mod tests;

enum Body {
    Existing(Entity<DeviceWorkspace>),
    Source(Entity<SourceProductWorkspace>),
}

/// Local metadata changes made by the Profiles application. They do not select
/// an active hardware profile or replace a product's source settings.
pub(super) enum ProfileMetadata {
    Rename(String),
    LinkGame {
        name: String,
        executable: String,
        linked: bool,
    },
}

pub(super) fn edit_profile_metadata(
    device: &mut Device,
    id: &str,
    change: ProfileMetadata,
) -> bool {
    let Some(index) = device.profiles.iter().position(|profile| profile.id == id) else {
        return false;
    };
    match change {
        ProfileMetadata::Rename(name) => {
            let name = name.trim();
            if name.is_empty()
                || name.encode_utf16().count() > 32
                || device.profiles.iter().any(|p| p.name == name && p.id != id)
                || device.profiles[index].name == name
            {
                return false;
            }
            device.profiles[index].name = name.into();
        }
        ProfileMetadata::LinkGame {
            name,
            executable,
            linked,
        } => {
            let key = |value: &str| value.replace('/', "\\").to_lowercase();
            let requested = key(&executable);
            // Source linkedDeviceToGame assigns at most one profile per device.
            for profile in &mut device.profiles {
                if let Some(settings) = &mut profile.settings {
                    settings
                        .linked_games
                        .retain(|game| key(&game.executable) != requested);
                }
            }
            if linked {
                device.profiles[index]
                    .settings
                    .get_or_insert_with(|| {
                        super::settings::ProfileSettings::for_product(device.product_id)
                    })
                    .linked_games
                    .push(super::settings::LinkedGame { name, executable });
            }
        }
    }
    true
}
pub(crate) struct ProductWorkspace {
    body: Body,
    _subscription: Subscription,
    _oled_subscription: Option<Subscription>,
    _receiver_subscription: Option<Subscription>,
    _receiver_device_subscription: Option<Subscription>,
    _dock_subscription: Option<Subscription>,
}
impl EventEmitter<WorkspaceEvent> for ProductWorkspace {}
impl EventEmitter<super::OledRuntimeRequested> for ProductWorkspace {}
impl EventEmitter<super::ReceiverPairingEvent> for ProductWorkspace {}
impl EventEmitter<super::ReceiverDeviceRequested> for ProductWorkspace {}
impl EventEmitter<super::DockPairingEvent> for ProductWorkspace {}

impl ProductWorkspace {
    pub(crate) fn set_mapping_macro_library(
        &mut self,
        file: &super::macro_library::MacroLibraryFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Existing(body) = &self.body {
            body.update(cx, |body, cx| {
                body.set_mapping_macro_library(file, window, cx)
            });
        }
    }
    pub(crate) fn observe_read_values(
        &mut self,
        values: Option<crate::backend::device_reads::DeviceReadValues>,
        cx: &mut Context<Self>,
    ) {
        match &self.body {
            Body::Existing(body) => {
                body.update(cx, |body, cx| body.observe_read_values(values, cx))
            }
            Body::Source(body) => body.update(cx, |body, cx| body.observe_read_values(values, cx)),
        }
    }
    pub(crate) fn observe_connection(
        &mut self,
        observation: Option<crate::model::DeviceConnectionObservation>,
        cx: &mut Context<Self>,
    ) {
        match &self.body {
            Body::Existing(body) => {
                body.update(cx, |body, cx| body.observe_connection(observation, cx))
            }
            Body::Source(body) => {
                body.update(cx, |body, cx| body.observe_connection(observation, cx))
            }
        }
    }

    pub(crate) fn observe_receiver_pairing(
        &mut self,
        observation: super::ReceiverPairingObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_receiver_pairing(observation, cx)
            })
        } else {
            false
        }
    }
    pub(crate) fn observe_receiver_devices(
        &mut self,
        devices: super::ReceiverDevicesObservation,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.observe_receiver_devices(devices, cx));
        }
    }
    pub(crate) fn observe_dock_pairing(
        &mut self,
        observation: super::DockPairingObservation,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.observe_dock_pairing(observation, cx));
        }
    }
    pub(crate) fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.set_active(active, window, cx));
        }
    }

    #[allow(dead_code)]
    pub(crate) fn mouse_polling_scope(
        &self,
        cx: &App,
    ) -> Option<super::mouse_polling::MousePollingScope> {
        match &self.body {
            Body::Source(body) => body.read(cx).mouse_polling_scope(cx),
            Body::Existing(body) => body.read(cx).mouse_polling_scope(cx),
        }
    }
    #[allow(dead_code)]
    pub(crate) fn observe_mouse_polling(
        &mut self,
        scope: super::mouse_polling::MousePollingScope,
        observation: super::mouse_polling::MousePollingObservation,
        cx: &mut Context<Self>,
    ) {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_mouse_polling(scope, observation, cx)
            }),
            Body::Existing(body) => body.update(cx, |body, cx| {
                body.observe_mouse_polling(scope, observation, cx)
            }),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn observe_dpi_editing_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_dpi_editing_enabled(enabled, window, cx)
            });
        }
    }
    #[allow(dead_code)]
    pub(crate) fn observe_scroll_wheel(
        &mut self,
        observation: super::ScrollWheelObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_scroll_wheel(observation, window, cx)
            });
        }
    }
    pub(crate) fn captures_snap_keys(&self, cx: &App) -> bool {
        matches!(&self.body, Body::Source(body) if body.read(cx).captures_snap_keys(cx))
    }
    pub(crate) fn capture_snap_key_up(&mut self, event: &KeyUpEvent, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.capture_snap_key_up(event, cx));
        }
    }
    /// Dedicated current Snap Tap observations; never a synthetic input/read reply.
    #[allow(dead_code)]
    pub(crate) fn observe_snap_tap(
        &mut self,
        observation: super::SnapTapObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_snap_tap(observation, window, cx)
            });
        }
    }

    /// Current calibration query scope; no query is started by reading this ID.
    #[allow(dead_code)]
    pub(crate) fn calibration_generation(&self, cx: &App) -> Option<u64> {
        match &self.body {
            Body::Source(body) => body.read(cx).calibration_generation(cx),
            _ => None,
        }
    }

    /// Local UI request only; never a device acknowledgement or a write call.
    #[allow(dead_code)]
    pub(crate) fn calibration_intent(
        &self,
        cx: &App,
    ) -> Option<super::gamepad_products::CalibrationIntent> {
        match &self.body {
            Body::Source(body) => body.read(cx).calibration_intent(cx),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn observe_calibration(
        &mut self,
        observation: super::gamepad_products::CalibrationObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_calibration(observation, window, cx)
            }),
            _ => false,
        }
    }
    /// Dedicated Stream Mixer MW observations; never substitute generic audio enumeration.
    #[allow(dead_code)]
    pub(crate) fn observe_stream_mixer(
        &mut self,
        observation: super::StreamMixerObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_stream_mixer(observation, window, cx)
            });
        }
    }
    /// Adapter boundary for real 1383 runtime observations. Profile data never
    /// supplies BLE/dongle status or a synthetic download reply.
    #[allow(dead_code)]
    pub(crate) fn observe_oled_runtime(
        &mut self,
        observation: super::OledRuntimeObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_oled_runtime(observation, window, cx)
            });
        }
    }

    /// Read-only adapter boundary; no monitor-service publisher is synthesized.
    #[allow(dead_code)]
    pub(crate) fn observe_monitor_runtime(
        &mut self,
        runtime: Option<&serde_json::Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_monitor_runtime(runtime, window, cx)
            });
        }
    }
    pub(crate) fn new(
        device: Device,
        intro: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        if !Tab::for_product(device.product_id).is_empty() {
            let entity = cx.new(|cx| DeviceWorkspace::new(device, intro, window, cx));
            let subscription = cx.subscribe(&entity, |_, _, event, cx| {
                cx.emit(match event {
                    WorkspaceEvent::Changed => WorkspaceEvent::Changed,
                    WorkspaceEvent::IntroDismissed => WorkspaceEvent::IntroDismissed,
                    WorkspaceEvent::ShareProfile => WorkspaceEvent::ShareProfile,
                    WorkspaceEvent::OpenChroma => WorkspaceEvent::OpenChroma,
                    WorkspaceEvent::OpenStudio => WorkspaceEvent::OpenStudio,
                    WorkspaceEvent::OpenMacro => WorkspaceEvent::OpenMacro,
                    WorkspaceEvent::PairingRequested(device) => {
                        WorkspaceEvent::PairingRequested(device.clone())
                    }
                    WorkspaceEvent::OpenDevice {
                        product_id,
                        edition_id,
                    } => WorkspaceEvent::OpenDevice {
                        product_id: *product_id,
                        edition_id: *edition_id,
                    },
                });
                cx.notify();
            });
            Self {
                body: Body::Existing(entity),
                _subscription: subscription,
                _oled_subscription: None,
                _receiver_subscription: None,
                _receiver_device_subscription: None,
                _dock_subscription: None,
            }
        } else {
            let entity = cx.new(|cx| SourceProductWorkspace::new(device, window, cx));
            let subscription = cx.subscribe(&entity, |_, _, event, cx| {
                cx.emit(match event {
                    WorkspaceEvent::Changed => WorkspaceEvent::Changed,
                    WorkspaceEvent::IntroDismissed => WorkspaceEvent::IntroDismissed,
                    WorkspaceEvent::ShareProfile => WorkspaceEvent::ShareProfile,
                    WorkspaceEvent::OpenChroma => WorkspaceEvent::OpenChroma,
                    WorkspaceEvent::OpenStudio => WorkspaceEvent::OpenStudio,
                    WorkspaceEvent::OpenMacro => WorkspaceEvent::OpenMacro,
                    WorkspaceEvent::PairingRequested(device) => {
                        WorkspaceEvent::PairingRequested(device.clone())
                    }
                    WorkspaceEvent::OpenDevice {
                        product_id,
                        edition_id,
                    } => WorkspaceEvent::OpenDevice {
                        product_id: *product_id,
                        edition_id: *edition_id,
                    },
                });
                cx.notify();
            });
            let oled_subscription = cx
                .subscribe(&entity, |_, _, event: &super::OledRuntimeRequested, cx| {
                    cx.emit(event.clone())
                });
            let receiver_subscription =
                cx.subscribe(&entity, |_, _, event: &super::ReceiverPairingEvent, cx| {
                    cx.emit(event.clone());
                });
            let receiver_device_subscription = cx.subscribe(
                &entity,
                |_, _, event: &super::ReceiverDeviceRequested, cx| cx.emit(event.clone()),
            );
            let dock_subscription =
                cx.subscribe(&entity, |_, _, event: &super::DockPairingEvent, cx| {
                    cx.emit(event.clone());
                });
            Self {
                body: Body::Source(entity),
                _subscription: subscription,
                _oled_subscription: Some(oled_subscription),
                _receiver_subscription: Some(receiver_subscription),
                _receiver_device_subscription: Some(receiver_device_subscription),
                _dock_subscription: Some(dock_subscription),
            }
        }
    }
    /// 把应用设备列表下发给子工作区（164/241 配对页的设备名链接比对用）。
    pub(crate) fn set_known_devices(&mut self, devices: Vec<(u32, u32)>, cx: &mut Context<Self>) {
        match &self.body {
            Body::Source(workspace) => {
                workspace.update(cx, |workspace, cx| workspace.set_known_devices(devices, cx))
            }
            // 只有 164/241 走 source 工作区并挂配对页。
            _ => {}
        }
    }

    pub(crate) fn device<'a>(&'a self, cx: &'a App) -> &'a Device {
        match &self.body {
            Body::Existing(e) => e.read(cx).device(),
            Body::Source(e) => e.read(cx).device(),
        }
    }
    pub(crate) fn identity(&self, cx: &App) -> String {
        let d = self.device(cx);
        format!(
            "{}:{}:{}",
            d.product_id, d.serial_number, d.device_container_id
        )
    }
    pub(crate) fn snapshot(&self, cx: &App) -> Device {
        self.device(cx).clone()
    }
    /// A retained product renderer with at least one implemented non-help page.
    /// Registry membership alone can still lead to a Pending source body.
    pub(crate) fn has_local_page(&self, cx: &App) -> bool {
        match &self.body {
            Body::Existing(_) => true,
            Body::Source(workspace) => workspace.read(cx).has_local_page(),
        }
    }
    /// The current 653 chromaApp root mounts the same lighting content as its
    /// normal lighting tab, without the product navigation/profile chrome.
    /// Only products whose bundle has a root-level `chromaApp` branch and whose
    /// local page set contains a lighting page qualify; the quick-effect
    /// buttons need this editing path, so a product without one stays disabled.
    pub(crate) fn chroma_lighting_workspace(&self, cx: &App) -> Option<Entity<DeviceWorkspace>> {
        let device = self.device(cx);
        if !chroma_product::has_chroma_app_root(device.product_id)
            || !Tab::for_product(device.product_id).contains(&Tab::Lighting)
        {
            return None;
        }
        match &self.body {
            Body::Existing(workspace) => Some(workspace.clone()),
            Body::Source(_) => None,
        }
    }
    /// Whether this device has a local `displayMode=chromaApp` popup root.
    pub(crate) fn has_chroma_device_page(&self, cx: &App) -> bool {
        let device = self.device(cx);
        if !chroma_product::has_chroma_app_root(device.product_id) {
            return false;
        }
        match &self.body {
            Body::Existing(_) => Tab::for_product(device.product_id).contains(&Tab::Lighting),
            Body::Source(workspace) => workspace.read(cx).supports_lighting_page(),
        }
    }
    /// The popup content the Chroma application mounts for one device: the
    /// product's lighting page without the product chrome.
    pub(crate) fn chroma_device_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.has_chroma_device_page(cx) {
            return None;
        }
        match &self.body {
            Body::Existing(workspace) => {
                Some(workspace.update(cx, |workspace, cx| workspace.lighting_page(cx)))
            }
            Body::Source(workspace) => workspace.update(cx, |workspace, cx| {
                workspace.lighting_page_element(window, cx)
            }),
        }
    }
    /// Whether this device has a local `displayMode=armory` root. The Armory
    /// application embeds the product's mapping page for the device it shares.
    pub(crate) fn has_armory_device_page(&self, cx: &App) -> bool {
        let device = self.device(cx);
        if !display_mode_roots::has_root_branch(DisplayModeRoot::Armory, device.product_id) {
            return false;
        }
        match &self.body {
            Body::Existing(_) => Tab::for_product(device.product_id).contains(&Tab::Customize),
            Body::Source(workspace) => workspace.read(cx).supports_mapping_page(),
        }
    }
    /// The mapping content the Armory application mounts for one device, without
    /// the product navigation and profile chrome.
    pub(crate) fn armory_device_page(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.has_armory_device_page(cx) {
            return None;
        }
        match &self.body {
            Body::Existing(workspace) => Some(workspace.update(cx, |workspace, cx| {
                workspace.armory_mapping_page(window, cx)
            })),
            Body::Source(workspace) => {
                workspace.update(cx, |workspace, cx| workspace.mapping_page_element(cx))
            }
        }
    }
    /// Existing local profile associations; no executable is launched or scanned.
    pub(crate) fn profile_linked_games(&self, profile: &str, cx: &App) -> Vec<(String, String)> {
        self.device(cx)
            .profiles
            .iter()
            .find(|entry| entry.id == profile)
            .and_then(|entry| entry.settings.as_ref())
            .map(|settings| {
                settings
                    .linked_games
                    .iter()
                    .map(|game| (game.name.clone(), game.executable.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Select a profile from the Profiles module and reuse the product page's
    /// existing continuation path. Existing workspaces preserve their dirty
    /// mapping decision; source-backed products restore the selected profile
    /// snapshot locally without contacting hardware.
    pub(crate) fn select_profile(
        &mut self,
        profile: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &self.body {
            Body::Existing(entity) => entity.update(cx, |workspace, cx| {
                workspace.continue_with(Continue::Profile(profile.to_owned()), window, cx)
            }),
            Body::Source(entity) => entity.update(cx, |workspace, cx| {
                workspace.select_profile(profile, window, cx)
            }),
        }
        cx.notify();
    }
    pub(crate) fn rename_profile(
        &mut self,
        profile: &str,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_profile_metadata(profile, ProfileMetadata::Rename(name), window, cx);
    }
    pub(crate) fn add_local_profile(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        self.change_profile_collection(ProfileCollectionAction::Add, window, cx)
    }
    pub(crate) fn duplicate_local_profile(
        &mut self,
        profile: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        self.change_profile_collection(
            ProfileCollectionAction::Duplicate(profile.into()),
            window,
            cx,
        )
    }
    pub(crate) fn delete_local_profile(
        &mut self,
        profile: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        self.change_profile_collection(ProfileCollectionAction::Delete(profile.into()), window, cx)
    }
    fn change_profile_collection(
        &mut self,
        action: ProfileCollectionAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        match &self.body {
            Body::Existing(entity) => entity.update(cx, |workspace, cx| {
                workspace.change_profile_collection(action, window, cx)
            }),
            Body::Source(entity) => entity.update(cx, |workspace, cx| {
                workspace.change_profile_collection(action, window, cx)
            }),
        }
    }
    pub(crate) fn link_profile_game(
        &mut self,
        profile: &str,
        name: String,
        executable: String,
        linked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.change_profile_metadata(
            profile,
            ProfileMetadata::LinkGame {
                name,
                executable,
                linked,
            },
            window,
            cx,
        );
    }
    fn change_profile_metadata(
        &mut self,
        profile: &str,
        change: ProfileMetadata,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &self.body {
            Body::Existing(entity) => entity.update(cx, |value, cx| {
                value.change_profile_metadata(profile, change, window, cx)
            }),
            Body::Source(entity) => entity.update(cx, |value, cx| {
                value.change_profile_metadata(profile, change, window, cx)
            }),
        }
    }
    pub(crate) fn saved_snapshot(&self, cx: &App) -> Device {
        match &self.body {
            Body::Existing(e) => e.read(cx).saved_snapshot(),
            Body::Source(e) => e.read(cx).saved_snapshot(),
        }
    }
    pub(crate) fn committed_pending(&self, cx: &App) -> bool {
        match &self.body {
            Body::Existing(e) => e.read(cx).committed_pending(),
            Body::Source(e) => e.read(cx).dirty(),
        }
    }
    pub(crate) fn discard_would_remove_mapping(&self, cx: &App) -> bool {
        matches!(&self.body, Body::Existing(e) if e.read(cx).discard_would_remove_mapping())
    }
    pub(crate) fn mark_saved(&mut self, snapshot: Device, cx: &mut Context<Self>) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.mark_saved(snapshot, cx)),
            Body::Source(e) => e.update(cx, |v, cx| v.mark_saved(snapshot, cx)),
        }
        cx.notify();
    }
    pub(crate) fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.discard(window, cx)),
            Body::Source(e) => e.update(cx, |v, cx| v.discard(window, cx)),
        }
        cx.notify();
    }
    pub(crate) fn discard_committed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.discard_committed(window, cx)),
            Body::Source(e) => {
                e.update(cx, |v, cx| v.discard(window, cx));
                true
            }
        }
    }
    pub(crate) fn set_intro_seen(&mut self, seen: bool, cx: &mut Context<Self>) {
        if let Body::Existing(e) = &self.body {
            e.update(cx, |v, cx| v.set_intro_seen(seen, cx));
        }
    }
    pub(crate) fn set_page(&mut self, page: Tab, window: &mut Window, cx: &mut Context<Self>) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.set_page(page, window, cx)),
            Body::Source(e) => e.update(cx, |v, cx| v.set_page_key(page.id(), window, cx)),
        }
        cx.notify();
    }
    pub(crate) fn can_step_history(&self, forward: bool, cx: &App) -> bool {
        match &self.body {
            Body::Existing(e) => e.read(cx).can_step_history(forward),
            Body::Source(e) => e.read(cx).can_step_history(forward),
        }
    }
    pub(crate) fn step_page_history(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.step_page_history(forward, window, cx)),
            Body::Source(e) => e.update(cx, |v, cx| v.step_page_history(forward, window, cx)),
        }
        cx.notify();
    }
    pub(crate) fn set_source_page(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(e) = &self.body {
            e.update(cx, |v, cx| v.set_page_key(key, window, cx));
        }
    }
    pub(crate) fn dismiss_profile_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.dismiss_profile_dialog(window, cx)),
            Body::Source(e) => e.update(cx, |v, cx| v.dismiss_profile_dialog(window, cx)),
        }
    }
    pub(crate) fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match &self.body {
            Body::Existing(e) => e.update(cx, |v, cx| v.refresh_locale(window, cx)),
            Body::Source(e) => e.update(cx, |_, cx| cx.notify()),
        }
    }
}
impl Render for ProductWorkspace {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.body {
            Body::Existing(e) => e.clone().into_any_element(),
            Body::Source(e) => e.clone().into_any_element(),
        }
    }
}

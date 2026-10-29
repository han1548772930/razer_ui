//! Native product ownership. Source navigation identity and local profile data
//! stay independent from the original ten hand-written adapters.
use super::{Choice, WorkspaceEvent};
use gpui_kit::base::ElementExt as _;
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::{
    input::{InputEvent, InputState},
    list::ListState,
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_catalog as product;
use razer_catalog::ProductPage;
use razer_catalog::ProductPageId;
use razer_catalog::ProductPageRole;
use razer_model::model::Device;
use razer_model::model::Profile;
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::surface;
use serde_json::Value;

#[path = "source_workspace/accessory.rs"]
mod accessory;
#[path = "source_workspace/audio_mixer_local_settings.rs"]
mod audio_mixer_local_settings;
#[path = "source_workspace/controller_initialization.rs"]
mod controller_initialization;
#[path = "source_workspace/help_camo.rs"]
mod help_camo;
#[path = "source_workspace/help_owner.rs"]
mod help_owner;
#[path = "source_workspace/help_scroll.rs"]
mod help_scroll;
#[path = "source_workspace/help_system.rs"]
mod help_system;
#[path = "source_workspace/help_versions.rs"]
mod help_versions;
#[path = "source_workspace/hyperflux.rs"]
mod hyperflux;
#[path = "source_workspace/hyperpolling_runtime.rs"]
mod hyperpolling_runtime;
#[path = "source_workspace/keyboard_linked_profiles.rs"]
mod keyboard_linked_profiles;
#[path = "source_workspace/keyboard_local_profiles.rs"]
mod keyboard_local_profiles;
#[path = "source_workspace/keyboard_mapping.rs"]
mod keyboard_mapping;
#[path = "source_workspace/keyboard_profile_inventory.rs"]
mod keyboard_profile_inventory;
pub use controller_initialization::ControllerInitializationRequested;
#[path = "source_workspace/controller_socd.rs"]
mod controller_socd;
pub use controller_socd::ControllerSocdRequested;
#[path = "source_workspace/keyboard_profile_mutations.rs"]
mod keyboard_profile_mutations;
#[path = "source_workspace/lifecycle.rs"]
mod lifecycle;
#[path = "source_workspace/lighting.rs"]
mod lighting;
#[path = "source_workspace/mapping_reconnect.rs"]
mod mapping_reconnect;
#[path = "source_workspace/mouse_profile_source.rs"]
mod mouse_profile_source;
#[path = "source_workspace/mouse_receiver_indicator.rs"]
mod mouse_receiver_indicator;
#[path = "source_workspace/mouse_runtime_artwork.rs"]
mod mouse_runtime_artwork;
#[path = "source_workspace/profile_actions.rs"]
mod profile_actions;
#[path = "source_workspace/profile_bar.rs"]
mod profile_bar;
#[path = "source_workspace/profile_dropdown.rs"]
mod profile_dropdown;
#[path = "source_workspace/profile_linked_games.rs"]
mod profile_linked_games;
#[path = "source_workspace/profile_menu.rs"]
mod profile_menu;
#[path = "source_workspace/profile_transfer.rs"]
mod profile_transfer;
#[path = "source_workspace/receiver_profile_selection.rs"]
mod receiver_profile_selection;
pub use receiver_profile_selection::ReceiverProfileSelectionRequested;
#[path = "source_workspace/ripsaw.rs"]
mod ripsaw;
#[path = "source_workspace/seiren_local_settings.rs"]
mod seiren_local_settings;
#[cfg(test)]
#[path = "source_workspace/tests.rs"]
mod tests;
use accessory::AccessoryPage;

// Current mounted mouse roots remove a tab only on their isBle branch:
// 204/45066 remove power; 170/214/235 remove calibration. In particular,
// 199/208 still mount power while removing only the polling controls.
// Full root bodies and reachable chunk identities: mouse-main-audit.json.
fn mouse_connection_navigation(product_id: u32, page: &str, is_ble: bool) -> bool {
    !is_ble
        || !matches!(
            (product_id, page),
            (204 | 45066, "TAB_POWER") | (170 | 214 | 235, "TAB_CALIBRATION")
        )
}

/// Automatic receiver reads need a live physical owner, never a preview or a
/// persisted identity alone. This validates data without entering a DLL loader.
fn receiver_read_owner(device: &Device) -> bool {
    use razer_model::model::DeviceConnectionObservation;
    let serial = device.serial_number.to_ascii_uppercase();
    let container = &device.device_container_id;
    device.product_id == 179
        && !serial.starts_with("PREVIEW-")
        && !serial.starts_with("DEMO-")
        && matches!(
            device.dashboard.connection_observation,
            Some(DeviceConnectionObservation::UsbPresent | DeviceConnectionObservation::HidPresent)
        )
        && ((container.len() == 38
            && container.starts_with('{')
            && container.ends_with('}')
            && uuid::Uuid::parse_str(container).is_ok_and(|id| !id.is_nil()))
            // Scheduling hint only; discovery owns exact node validation.
            || container.starts_with(&format!("hid-collection:1532:{:04x}:", device.real_product_id)))
}

enum FamilyBody {
    Mouse(Entity<super::mouse_products::MouseProductWorkspace>),
    Keyboard(Entity<super::keyboard_products::KeyboardProductWorkspace>),
    Gamepad(Entity<super::gamepad_products::GamepadProductWorkspace>),
    Controls(Entity<super::source_controls::SourceControls>),
    Audio(Entity<super::audio_products::AudioProductWorkspace>),
    System(Entity<super::system_products::SystemProductWorkspace>),
    AccessorySystem(Entity<super::accessory_system_products::AccessorySystemProductWorkspace>),
    Hue(Entity<super::hue::HueWorkspace>),
    Pending,
}
pub struct SourceProductWorkspace {
    device: Device,
    saved: Device,
    mouse_profile_observed_serial: Option<String>,
    mouse_profile_connection_key: Option<String>,
    mouse_scroll_connection_key: Option<String>,
    mapping_connection_generation: u64,
    mapping_reconnect: mapping_reconnect::State,
    keyboard_latest_ble: bool,
    keyboard_profile_inventory: keyboard_profile_inventory::Owner,
    controller_initialization: controller_initialization::Owner,
    controller_socd: controller_socd::Owner,
    page: Option<ProductPageId>,
    page_history: Vec<ProductPageId>,
    page_history_index: usize,
    active: bool,
    hanbo_telemetry_requested: bool,
    hanbo_profile_revision: u64,
    receiver_profile_selection: std::cell::RefCell<receiver_profile_selection::State>,
    exiting: bool,
    trigger_navigation: super::gamepad_products::TriggerNavigation,
    body: FamilyBody,
    help: Entity<super::source_help::SourceHelp>,
    help_owner: Entity<super::source_help_owner::SourceHelpOwner>,
    body_scroll: ScrollHandle,
    help_scroll: Option<help_scroll::Target>,
    keymap_bar_width: f32,
    supplement: Option<Entity<super::source_controls::SourceControls>>,
    dock_pairing: Option<Entity<super::dock_pairing::DockPairing>>,
    dual_link_widget: Option<Entity<super::dock_pairing::DualLinkWidget>>,
    accessory: Option<AccessoryPage>,
    profile: Entity<SelectState<Vec<Choice>>>,
    profile_choices: Vec<Choice>,
    profile_decorations:
        std::collections::BTreeMap<String, razer_widgets::synapse_select::SelectOptionDecoration>,
    profile_status: Option<String>,
    profile_name: Entity<InputState>,
    profile_rename: Option<String>,
    local_mapping_macros: Vec<Value>,
    profile_menu: Entity<ListState<profile_menu::ProfileCommands>>,
    profile_confirmation: Option<profile_actions::ProfileConfirmation>,
    profile_confirm_focus: FocusHandle,
    profile_confirm_task: Option<Task<()>>,
    profile_transfer: Option<Entity<profile_transfer::SourceProfileTransfer>>,
    profile_transfer_subscription: Option<Subscription>,
    profile_transfer_request: Option<razer_storage::file_transfer::Transfer>,
    profile_export_task: Option<profile_actions::PendingProfileExport>,
    profile_import: Option<razer_model::profile_transfer::SourceProfileImport>,
    profile_linked_games: Option<Entity<profile_linked_games::ProfileLinkedGames>>,
    profile_linked_games_subscription: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<WorkspaceEvent> for SourceProductWorkspace {}
impl EventEmitter<super::OledRuntimeRequested> for SourceProductWorkspace {}
impl EventEmitter<super::ReceiverPairingEvent> for SourceProductWorkspace {}
impl EventEmitter<super::ReceiverDeviceRequested> for SourceProductWorkspace {}
impl EventEmitter<super::DockPairingEvent> for SourceProductWorkspace {}
impl EventEmitter<super::DockPairingUtilityRequested> for SourceProductWorkspace {}

impl SourceProductWorkspace {
    pub fn interface_resource_errors(&self, cx: &App) -> Vec<String> {
        match &self.body {
            FamilyBody::AccessorySystem(body) => body.read(cx).hanbo_label_errors(cx),
            _ => Vec::new(),
        }
    }

    pub fn retry_interface_resources(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::AccessorySystem(body) = &self.body {
            body.update(cx, |body, cx| body.retry_hanbo_labels(cx));
        }
    }

    pub fn initialize_mouse_scroll_profile(&mut self, key: Option<String>, cx: &mut Context<Self>) {
        self.mouse_scroll_connection_key = key;
        self.sync_mouse_scroll_profile(cx);
    }

    fn sync_mouse_scroll_profile(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(body) = &self.body {
            let guid = self
                .device
                .active_profile_obj()
                .map(|profile| profile.guid.as_str());
            let key = guid.and(self.mouse_scroll_connection_key.as_deref());
            body.update(cx, |body, cx| {
                body.initialize_mouse_scroll_profile(key, guid.unwrap_or(""), cx)
            });
        }
    }

    pub fn refresh_locale(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(mouse) = &self.body {
            mouse.update(cx, |mouse, cx| mouse.refresh_locale(window, cx));
        }
        if let FamilyBody::Keyboard(keyboard) = &self.body {
            keyboard.update(cx, |keyboard, cx| keyboard.refresh_locale(window, cx));
        }
        if let FamilyBody::Controls(controls) = &self.body {
            controls.update(cx, |controls, cx| controls.refresh_locale(window, cx));
        }
        if let FamilyBody::AccessorySystem(body) = &self.body {
            body.update(cx, |body, cx| body.refresh_hanbo_locale(cx));
        }
        if let Some(controls) = &self.supplement {
            controls.update(cx, |controls, cx| controls.refresh_locale(window, cx));
        }
        cx.notify();
    }
}

impl SourceProductWorkspace {
    pub(super) fn keyboard_app_frame_bounds_handle(
        &self,
        cx: &App,
    ) -> Option<std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>> {
        match &self.body {
            FamilyBody::Keyboard(body) => body.read(cx).mapping_app_frame_bounds(),
            _ => None,
        }
    }
    pub fn reject_mouse_calibration(
        &mut self,
        request: &super::mouse_products::MouseCalibrationRequested,
        error: String,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.reject_calibration_request(request, error, cx);
            });
        }
    }
    pub fn mouse_calibration_request_matches(
        &self,
        request: &super::mouse_products::MouseCalibrationRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).calibration_request_matches(request, cx))
    }
    pub fn finish_mouse_calibration(
        &mut self,
        request: &super::mouse_products::MouseCalibrationRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.complete_calibration_operation(request, error, cx)
            });
        }
    }
    pub fn cancel_mouse_calibration(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, _| body.cancel_calibration_requests());
        }
    }
    pub fn prepare_mouse_calibration_exit(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Vec<super::mouse_products::MouseCalibrationRequested> {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.prepare_calibration_exit(cx))
        } else {
            Vec::new()
        }
    }
    pub fn begin_mouse_philips_local_save(
        &mut self,
        request: &super::mouse_products::MouseCalibrationRequested,
        cx: &mut Context<Self>,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body)
            if body.update(cx, |body, cx| body.begin_philips_local_save(request, cx)))
    }
    pub fn mouse_philips_local_save_matches(
        &self,
        request: &super::mouse_products::MouseCalibrationRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body)
            if body.read(cx).philips_local_save_matches(request, cx))
    }
    pub fn finish_mouse_philips_local_save(
        &mut self,
        request: &super::mouse_products::MouseCalibrationRequested,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, _| body.finish_philips_local_save(request));
        }
    }
    pub fn record_mouse_tracking_report(
        &mut self,
        request: &super::mouse_products::MouseCalibrationRequested,
        report: razer_device::mouse_tracking::Progress,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.record_tracking_report(request, report, cx)
            });
        }
    }
    pub fn record_mouse_philips_report(
        &mut self,
        request: &super::mouse_products::MouseCalibrationRequested,
        report: razer_device::mouse_philips::Progress,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.record_philips_report(request, report, cx)
            });
        }
    }
    pub fn complete_mouse_philips_context(
        &mut self,
        request: &super::mouse_products::MouseCalibrationRequested,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.complete_philips_context(request, cx));
        }
    }
    pub fn record_mouse_philips_measurement(
        &mut self,
        request: &super::mouse_products::MouseCalibrationRequested,
        progress: razer_device::mouse_philips::measurement::Progress,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.record_philips_measurement(request, progress, window, cx)
            });
        }
    }
    pub fn finish_mouse_philips_measurement(
        &mut self,
        request: &super::mouse_products::MouseCalibrationRequested,
        progress: Option<razer_device::mouse_philips::measurement::Progress>,
        error: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_philips_measurement(request, progress, error, window, cx)
            });
        }
    }
    pub fn publish_mouse_philips_measurement_save(
        &mut self,
        request: &super::mouse_products::MouseCalibrationRequested,
        projection: Value,
        cx: &mut Context<Self>,
    ) {
        if !self.mouse_calibration_request_matches(request, cx) {
            return;
        }
        // mark_saved has accepted this writer's snapshot. The source keeps a
        // separate emitted recalibration profile; retain that difference in
        // the page without making its projection a second durable writer.
        if let Some(calibration) = self
            .saved
            .source_device_settings
            .as_ref()
            .and_then(|root| root.get("calibration"))
        {
            self.device
                .source_device_settings
                .get_or_insert_with(|| serde_json::json!({}))["calibration"] = calibration.clone();
        }
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.publish_philips_measurement_save(request, projection, cx)
            });
        }
        cx.notify();
    }
    pub fn restore_mouse_calibration_welcome(
        &mut self,
        visible: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.restore_calibration_welcome(visible.unwrap_or(true), cx)
            });
        }
    }
    pub fn reject_mouse_matcher(
        &mut self,
        scope: &super::mouse_products::MouseMatcherScope,
        reason: String,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.reject_mouse_matcher(scope, reason, cx));
        }
    }
    pub fn mouse_matcher_scope(
        &self,
        cx: &App,
    ) -> Option<super::mouse_products::MouseMatcherScope> {
        match &self.body {
            FamilyBody::Mouse(body) => body.read(cx).mouse_matcher_scope(cx),
            _ => None,
        }
    }
    pub fn retains_mouse_matcher_scope(
        &self,
        scope: &super::mouse_products::MouseMatcherScope,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).retains_mouse_matcher_scope(scope, cx))
    }
    pub fn accepts_mouse_matcher_scope(
        &self,
        scope: &super::mouse_products::MouseMatcherScope,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).accepts_mouse_matcher_scope(scope, cx))
    }
    pub fn bind_mouse_matcher_requests(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.bind_mouse_matcher_requests(cx));
        }
    }
    pub fn accept_mouse_matcher_saved_profiles(
        &mut self,
        scope: &super::mouse_products::MouseMatcherScope,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let saved = super::mouse_products::SavedMatcherProfiles::from_device(&self.saved)?;
        let mut device = self.device.clone();
        saved.apply_to_device(&mut device)?;
        let FamilyBody::Mouse(body) = &self.body else {
            anyhow::bail!("Matcher saved workspace has no mouse page");
        };
        body.update(cx, |body, cx| {
            body.accept_mouse_matcher_saved_profiles(scope, &saved, cx)
        })?;
        self.device = device;
        cx.notify();
        Ok(())
    }
    pub fn observe_mouse_matcher(
        &mut self,
        scope: &super::mouse_products::MouseMatcherScope,
        observation: super::mouse_dpi_matcher::MatcherObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_mouse_matcher(scope, observation, window, cx)
            });
        }
    }
    pub fn submit_mouse_matcher_stages(
        &mut self,
        scope: &super::mouse_products::MouseMatcherScope,
        stages: razer_device::mouse_current_dpi::SourceStages,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let FamilyBody::Mouse(body) = &self.body else {
            anyhow::bail!("Matcher stage workspace has no mouse page");
        };
        body.update(cx, |body, cx| {
            body.submit_mouse_matcher_stages(scope, stages, cx)
        })
    }
    pub fn restore_mouse_dynamic_tutorial_preference(
        &mut self,
        visible: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.restore_dynamic_tutorial_preference(visible, cx)
            });
        }
    }
    pub fn finish_keyboard_indicator_led(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_indicator_led(generation, result, cx)
            });
        }
    }

    pub fn observe_keyboard_indicator_led(
        &mut self,
        observation: super::keyboard_products::KeyboardIndicatorLedObservation,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.observe_indicator_led(observation, cx));
        }
    }
    /// Actual connection observations, independent of locally saved profiles.
    pub fn observe_keyboard_polling_connection(
        &mut self,
        observation: super::keyboard_products::KeyboardPollingConnection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(widget) = &self.dual_link_widget {
            widget.update(cx, |widget, cx| {
                widget.set_bluetooth(observation.is_ble, cx)
            });
        }
        let previous_key = self.current_page().map(|page| page.kind().key());
        self.help.update(cx, |help, cx| {
            help.observe_connection(observation.is_ble, cx)
        });
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_polling_connection(observation, cx)
            });
        }
        if self.device.product_id == 756 {
            let target = self.active_navigation(cx).and_then(|navigation| {
                navigation
                    .pages()
                    .iter()
                    .find(|page| Some(page.kind().key()) == previous_key)
                    .or_else(|| navigation.pages().first())
            });
            if let Some(page) = target {
                self.set_page(page.id(), window, cx);
            }
        }
        cx.notify();
    }

    pub fn observe_keyboard_legacy_calibration_capture(
        &mut self,
        initialized: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_legacy_calibration_capture(initialized, window, cx)
            });
        }
    }
    pub fn observe_keyboard_legacy_calibration_input(
        &mut self,
        input: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_legacy_calibration_input(input, window, cx)
            });
        }
    }
    pub fn observe_keyboard_legacy_calibration_foreground(
        &mut self,
        name: &str,
        host_active: bool,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_legacy_calibration_foreground(name, host_active, cx)
            });
        }
    }
    pub fn observe_keyboard_legacy_calibration(
        &mut self,
        action: &str,
        payload: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_legacy_calibration(action, payload, cx)
            })
        } else {
            false
        }
    }
    pub fn observe_keyboard_calibration_v2(
        &mut self,
        action: &str,
        payload: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_calibration_v2(action, payload, window, cx)
            });
        }
    }

    pub fn observe_keyboard_calibration_v2_connection(
        &mut self,
        is_ble: bool,
        normal_mode: bool,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_calibration_v2_connection(is_ble, normal_mode, cx)
            });
        }
    }

    pub fn cancel_keyboard_calibration_v2(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.cancel_calibration_v2(cx));
        }
    }

    pub fn observe_help_obm_reset_change(&mut self, cx: &mut Context<Self>) {
        self.help
            .update(cx, |help, cx| help.observe_obm_reset_change(cx));
    }
    pub fn observe_help_oled_resetting(
        &mut self,
        owner: &Device,
        resetting: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.device.product_id, 691 | 1383)
            || self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
        {
            return false;
        }
        self.help
            .update(cx, |help, cx| help.observe_oled_resetting(resetting, cx));
        true
    }
    pub fn mouse_dynamic_scope_is_current(
        &self,
        request: &super::mouse_products::MouseDynamicRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).mouse_dynamic_scope_is_current(request))
    }
    pub fn mouse_dynamic_connection_is_current(
        &self,
        connection: super::mouse_products::MouseDynamicConnection,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).mouse_dynamic_connection_is_current(connection))
    }
    pub fn mouse_rotation_scope_is_current(
        &self,
        request: &super::mouse_products::MouseRotationRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).mouse_rotation_scope_is_current(request, cx))
    }
    pub fn mouse_rotation_request_is_current(
        &self,
        request: &super::mouse_products::MouseRotationRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).mouse_rotation_request_is_current(request, cx))
    }
    pub fn mark_mouse_rotation_persisted(
        &mut self,
        request: &super::mouse_products::MouseRotationRequested,
        cx: &mut Context<Self>,
    ) -> bool {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.mark_mouse_rotation_persisted(request, cx)
            })
        } else {
            false
        }
    }
    pub fn complete_mouse_rotation(
        &mut self,
        request: &super::mouse_products::MouseRotationRequested,
        completion: super::mouse_products::MouseRotationCompletion,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.complete_mouse_rotation(request, completion, cx)
            })
        } else {
            None
        }
    }
    pub fn mouse_dynamic_request_is_current(
        &self,
        request: &super::mouse_products::MouseDynamicRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).mouse_dynamic_request_is_current(request))
    }
    pub fn observe_mouse_dynamic_speed(
        &mut self,
        connection: super::mouse_products::MouseDynamicConnection,
        speed: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_mouse_dynamic_speed(connection, speed, window, cx);
            });
        }
    }
    pub fn complete_mouse_dynamic(
        &mut self,
        request: &super::mouse_products::MouseDynamicRequested,
        completion: super::mouse_products::MouseDynamicCompletion,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.complete_mouse_dynamic(request, completion, cx)
            })
        } else {
            None
        }
    }
    pub fn complete_leviathan(
        &mut self,
        generation: u64,
        result: Result<razer_device::leviathan::Report, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.complete_leviathan(generation, result, window, cx);
            });
        }
    }
    pub fn audio_volume_request_matches(
        &self,
        request: super::audio_products::AudioVolumeRequest,
        cx: &App,
    ) -> bool {
        matches!(&self.body,FamilyBody::Audio(body) if body.read(cx).volume_request_matches(request))
    }
    pub fn audio_volume_request_current(
        &self,
        request: super::audio_products::AudioVolumeRequest,
        cx: &App,
    ) -> bool {
        matches!(&self.body,FamilyBody::Audio(body) if body.read(cx).volume_request_current(request))
    }
    pub fn audio_volume_cancellation(
        &self,
        request: super::audio_products::AudioVolumeRequest,
        cx: &App,
    ) -> Option<std::sync::Arc<std::sync::atomic::AtomicBool>> {
        if let FamilyBody::Audio(body) = &self.body {
            body.read(cx).volume_cancellation(request)
        } else {
            None
        }
    }
    pub fn finish_audio_volume(
        &mut self,
        request: super::audio_products::AudioVolumeRequest,
        result: Result<super::audio_products::AudioVolumeCompletion, String>,
        scope_current: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_volume(request, result, scope_current, window, cx)
            });
        }
    }
    pub fn mixer_request_matches(
        &self,
        request: &super::audio_products::AudioMixerRequest,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Audio(body) if body.read(cx).mixer_request_matches(request))
    }
    pub fn mixer_cancellation(
        &self,
        request: &super::audio_products::AudioMixerRequest,
        cx: &App,
    ) -> Option<std::sync::Arc<std::sync::atomic::AtomicBool>> {
        if let FamilyBody::Audio(body) = &self.body {
            body.read(cx).mixer_request_cancellation(request)
        } else {
            None
        }
    }
    pub fn mixer_request_current(
        &self,
        request: &super::audio_products::AudioMixerRequest,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Audio(body) if body.read(cx).mixer_request_current(request))
    }
    pub fn finish_mixer(
        &mut self,
        request: super::audio_products::AudioMixerRequest,
        result: Result<super::audio_products::AudioMixerCompletion, String>,
        scope_current: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_mixer(request, result, scope_current, window, cx)
            });
        }
    }
    /// Diagnostics remain separate from the original control's observed value.
    pub fn keyboard_brightness_diagnostic(&self, cx: &App) -> Option<String> {
        match &self.body {
            FamilyBody::Keyboard(body) => Some(body.read(cx).brightness_runtime_text()),
            _ => None,
        }
    }
    pub fn keyboard_brightness_read_matches(&self, generation: u64, cx: &App) -> bool {
        matches!(&self.body,FamilyBody::Keyboard(body) if body.read(cx).brightness_read_matches(generation))
    }
    pub fn keyboard_brightness_cancellation_guard(
        &self,
        generation: u64,
        percent: Option<u8>,
        cx: &App,
    ) -> Option<std::sync::Arc<std::sync::atomic::AtomicBool>> {
        match &self.body {
            FamilyBody::Keyboard(body) => body
                .read(cx)
                .brightness_cancellation_guard(generation, percent),
            _ => None,
        }
    }
    pub fn finish_keyboard_actuation(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.finish_actuation(generation, result, cx));
        }
    }
    pub fn keyboard_actuation_request_current(&self, generation: u64, cx: &App) -> bool {
        matches!(&self.body, FamilyBody::Keyboard(body) if body.read(cx).actuation_request_current(generation))
    }
    pub fn keyboard_brightness_read_current(&self, generation: u64, cx: &App) -> bool {
        matches!(&self.body,FamilyBody::Keyboard(body) if body.read(cx).brightness_read_current(generation))
    }
    pub fn sync_keyboard_brightness_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.sync_brightness_controls(window, cx));
        }
    }
    pub fn finish_keyboard_brightness_read(
        &mut self,
        generation: u64,
        observed: Option<u8>,
        error: Option<String>,
        scope_current: bool,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_brightness_read(generation, observed, error, scope_current, cx)
            });
        }
    }
    pub fn keyboard_brightness_request_matches(
        &self,
        generation: u64,
        percent: u8,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Keyboard(body) if body.read(cx).brightness_request_matches(generation, percent))
    }
    pub fn finish_keyboard_brightness(
        &mut self,
        generation: u64,
        percent: u8,
        observed: Option<u8>,
        error: Option<String>,
        scope_current: bool,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_brightness(generation, percent, observed, error, scope_current, cx)
            });
        }
    }
    pub fn keyboard_brightness_request_current(
        &self,
        generation: u64,
        percent: u8,
        cx: &App,
    ) -> bool {
        matches!(&self.body,FamilyBody::Keyboard(body) if body.read(cx).brightness_request_current(generation,percent))
    }
    pub fn cancel_keyboard_brightness_connection(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.cancel_brightness_connection();
                body.cancel_actuation_connection();
                body.cancel_indicator_led_connection(cx);
            });
        }
    }
    pub fn observe_keyboard_metadata(
        &mut self,
        reading: Option<razer_device::keyboard_metadata::KeyboardMetadataReading>,
        cx: &mut Context<Self>,
    ) {
        self.device.observe_keyboard_metadata(reading.clone());
        self.saved.observe_keyboard_metadata(reading.clone());
        if reading.is_some()
            && let FamilyBody::Keyboard(body) = &self.body
        {
            body.update(cx, |body, cx| {
                body.observe_source_artwork(
                    super::keyboard_products::SourceArtworkObservation::for_device(&self.device),
                    cx,
                )
            });
        }
        let device = self.device.clone();
        self.help
            .update(cx, |help, cx| help.set_device(&device, cx));
        cx.notify();
    }

    pub fn observe_read_values(
        &mut self,
        values: Option<razer_device::device_reads::DeviceReadValues>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_mouse_polling_values(values.as_ref(), cx)
            });
        }
        if let FamilyBody::Controls(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_camera_firmware(
                    values
                        .as_ref()
                        .and_then(|values| values.firmware.as_deref()),
                    cx,
                );
            });
        }
        if let Some(firmware) = values
            .as_ref()
            .and_then(|values| values.firmware.as_deref())
        {
            self.trigger_navigation
                .observe_firmware(self.device.product_id, firmware);
        }
        self.device.observe_read_values(values.clone());
        self.saved.observe_read_values(values);
        let device = self.device.clone();
        self.help
            .update(cx, |help, cx| help.set_device(&device, cx));
        cx.notify();
    }
    pub fn observe_pairing_mode(&mut self, is_dongle: Option<bool>, cx: &mut Context<Self>) {
        self.device.dashboard.readonly_pairing_is_dongle = is_dongle;
        self.saved.dashboard.readonly_pairing_is_dongle = is_dongle;
        cx.notify();
    }
    pub(super) fn mount_local_runtime(&mut self, cx: &mut Context<Self>) {
        self.device.mount_local_runtime();
        self.saved.mount_local_runtime();
        cx.notify();
    }

    pub fn observe_connection(
        &mut self,
        observation: Option<razer_model::model::DeviceConnectionObservation>,
        cx: &mut Context<Self>,
    ) {
        let connection_changed = self.device.dashboard.connection_observation != observation;
        let legacy_connection_changed = self.device.product_id == 70 && connection_changed;
        if connection_changed {
            self.mapping_connection_generation = self.mapping_connection_generation.wrapping_add(1);
            self.invalidate_keyboard_profile_inventory(observation.is_some());
            self.invalidate_controller_initialization(cx);
            self.receiver_profile_selection.borrow_mut().invalidate();
            self.cancel_profile_transfer_requests();
            self.cancel_keyboard_brightness_connection(cx);
            if let FamilyBody::Keyboard(body) = &self.body {
                body.update(cx, |body, _| {
                    body.set_mapping_callback_generation(self.mapping_connection_generation);
                    body.set_legacy_mapping_connection(None);
                });
            }
            if let FamilyBody::Gamepad(body) = &self.body {
                body.update(cx, |body, cx| {
                    body.observe_controller_connection(observation.is_some(), cx);
                    body.observe_controller_sleep_connection(observation.is_some(), cx);
                });
            }
            if let FamilyBody::Mouse(body) = &self.body {
                body.update(cx, |body, cx| {
                    body.invalidate_mouse_polling_connection();
                    body.invalidate_current_power_connection();
                    body.invalidate_mouse_rotation();
                    body.cancel_calibration_requests();
                    body.observe_dynamic_connection(observation.is_some(), cx);
                });
            }
            if let FamilyBody::Controls(body) = &self.body {
                body.update(cx, |body, _| body.cancel_brightness_connection());
            }
            if let FamilyBody::Audio(body) = &self.body {
                body.update(cx, |body, cx| {
                    body.invalidate_volume();
                    body.invalidate_mixer();
                    body.cancel_seiren_eq_connection();
                    body.cancel_seiren_stream_mixer_connection();
                    body.cancel_seiren_audio_capture();
                    body.cancel_oled_editor(cx);
                    body.cancel_leviathan(cx);
                });
            }
        }
        self.device.observe_connection(observation.clone());
        self.saved.observe_connection(observation);
        if connection_changed
            && matches!(self.device.product_id, 1422 | 1446)
            && matches!(
                self.device.dashboard.connection_observation,
                Some(
                    razer_model::model::DeviceConnectionObservation::UsbPresent
                        | razer_model::model::DeviceConnectionObservation::HidPresent
                )
            )
            && let FamilyBody::Audio(body) = &self.body
        {
            body.update(cx, |body, cx| body.request_seiren_volume_read(cx));
        }
        if legacy_connection_changed {
            use razer_model::model::DeviceConnectionObservation;
            if matches!(
                self.device.dashboard.connection_observation,
                Some(
                    DeviceConnectionObservation::UsbPresent
                        | DeviceConnectionObservation::HidPresent
                )
            ) {
                self.persist_source_profile(true, cx);
            }
        }
        let device = self.device.clone();
        self.help
            .update(cx, |help, cx| help.set_device(&device, cx));
        self.sync_keyboard_read_activity(cx);
        self.sync_mouse_mapping_activity(cx);
        self.sync_audio_volume_activity(cx);
        self.sync_audio_mixer_activity(cx);
        self.sync_seiren_audio_activity(cx);
        self.sync_receiver_read_activity(cx);
        self.sync_receiver_profile_initialization(cx);
        self.sync_camera_activity(cx);
        cx.notify();
    }

    pub fn observe_receiver_pairing(
        &mut self,
        observation: super::ReceiverPairingObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != 179 {
            return false;
        }
        if let FamilyBody::Controls(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_receiver_pairing(observation, cx)
            })
        } else if let Some(body) = &self.supplement {
            body.update(cx, |body, cx| {
                body.observe_receiver_pairing(observation, cx)
            })
        } else {
            false
        }
    }
    pub fn observe_receiver_devices(
        &mut self,
        devices: super::ReceiverDevicesObservation,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Controls(body) = &self.body {
            body.update(cx, |body, cx| body.observe_receiver_devices(devices, cx));
        } else if let Some(body) = &self.supplement {
            body.update(cx, |body, cx| body.observe_receiver_devices(devices, cx));
        }
    }
    pub fn observe_dock_pairing(
        &mut self,
        observation: super::DockPairingObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id == 192 {
            if let Some(Ok(payload)) = observation.result_ref() {
                if let FamilyBody::Mouse(body) = &self.body {
                    body.update(cx, |body, cx| {
                        body.observe_legacy_receiver_pairing(observation.kind(), payload, cx)
                    });
                }
            }
        }
        if let Some(dock) = &self.dock_pairing {
            return dock.update(cx, |dock, cx| dock.observe_pairing(observation, cx));
        }
        if let Some(widget) = &self.dual_link_widget {
            return widget.update(cx, |widget, cx| widget.observe_pairing(observation, cx));
        }
        false
    }
    pub fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut Context<Self>) {
        let active = active && !self.exiting;
        if !active {
            self.dismiss_profile_dialog(window, cx);
        }
        self.active = active;
        if active
            && self.device.product_id == razer_device::hanbo_cooler::PRODUCT_ID
            && !self.hanbo_telemetry_requested
        {
            self.hanbo_telemetry_requested = true;
            cx.emit(WorkspaceEvent::HanboTelemetryActivated);
        }
        self.help_owner.update(cx, |owner, cx| {
            owner.set_active(active, cx);
            if active {
                owner.mount(cx);
            }
        });
        self.sync_receiver_read_activity(cx);
        self.sync_camera_activity(cx);
        self.sync_mouse_active(window, cx);
        self.sync_keyboard_read_activity(cx);
        self.sync_audio_volume_activity(cx);
        self.sync_audio_mixer_activity(cx);
        self.sync_seiren_audio_activity(cx);
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.set_keypad_active(active, cx));
        }
        if active
            && let Some(page) = self.current_page()
            && page.role() == ProductPageRole::Help
        {
            self.help.update(cx, |help, cx| {
                help.set_device(&self.device, cx);
                help.set_page(page.offset(), cx);
            });
        }
        if !active {
            if let FamilyBody::Controls(body) = &self.body {
                body.update(cx, |body, cx| body.dismiss_oled_editor(window, cx));
            }
            if let Some(supplement) = &self.supplement {
                supplement.update(cx, |body, cx| body.dismiss_oled_editor(window, cx));
            }
            self.help.update(cx, |help, cx| {
                if self.exiting && matches!(self.device.product_id, 164 | 241) {
                    help.prepare_receiver_exit(cx);
                } else {
                    help.leave_page(cx);
                }
            });
            self.cancel_keyboard_calibration_v2(cx);
            if let FamilyBody::Keyboard(body) = &self.body {
                body.update(cx, |body, cx| body.dismiss_editors(window, cx));
            }
            if let FamilyBody::Audio(body) = &self.body {
                body.update(cx, |body, cx| {
                    body.cancel_leviathan(cx);
                    body.cancel_oled_editor(cx);
                    body.stop_media(window, cx);
                });
            }
            if let FamilyBody::Gamepad(body) = &self.body {
                body.update(cx, |body, cx| {
                    body.cancel_range_edits(cx);
                    body.leave_calibration(window, cx);
                });
            }
        }
    }

    fn sync_seiren_audio_activity(&self, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            let ready = matches!(
                self.device.dashboard.connection_observation,
                Some(
                    razer_model::model::DeviceConnectionObservation::UsbPresent
                        | razer_model::model::DeviceConnectionObservation::HidPresent
                )
            );
            let eq_page = self
                .current_page()
                .is_some_and(|page| page.kind().key() == "TAB_EQ");
            body.update(cx, |body, cx| {
                body.set_seiren_audio_activity(self.active, ready, eq_page, cx)
            });
        }
    }
    fn sync_audio_volume_activity(&self, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            let active = self.active
                && self
                    .current_page()
                    .is_some_and(|page| page.kind().key() == "TAB_SOUND")
                && matches!(
                    self.device.dashboard.connection_observation,
                    Some(
                        razer_model::model::DeviceConnectionObservation::UsbPresent
                            | razer_model::model::DeviceConnectionObservation::HidPresent
                    )
                );
            body.update(cx, |body, cx| {
                body.set_volume_active(active, cx);
                body.set_leviathan_active(active, cx);
                let surround_active = self.active
                    && !self.exiting
                    && matches!(
                        self.device.dashboard.connection_observation,
                        Some(
                            razer_model::model::DeviceConnectionObservation::UsbPresent
                                | razer_model::model::DeviceConnectionObservation::HidPresent
                        )
                    );
                body.set_surround_active(surround_active, &self.device.device_container_id, cx);
            });
        }
    }
    fn sync_camera_activity(&self, cx: &mut Context<Self>) {
        if let FamilyBody::Controls(body) = &self.body {
            let active = !self.exiting
                && matches!(self.device.product_id, 3594..=3596)
                && self
                    .current_page()
                    .is_some_and(|p| p.role() != ProductPageRole::Help)
                && matches!(
                    self.device.dashboard.connection_observation,
                    Some(
                        razer_model::model::DeviceConnectionObservation::UsbPresent
                            | razer_model::model::DeviceConnectionObservation::HidPresent
                    )
                )
                && !self
                    .device
                    .serial_number
                    .to_ascii_uppercase()
                    .starts_with("PREVIEW-")
                && !self
                    .device
                    .serial_number
                    .to_ascii_uppercase()
                    .starts_with("DEMO-");
            let scope = active.then(|| razer_device::camera_submission::CameraScope {
                product_id: self.device.product_id,
                container_id: self.device.device_container_id.clone(),
            });
            body.update(cx, |body, cx| {
                body.set_camera_host_tab_selected(self.active, cx);
                body.set_camera_scope(scope, cx);
            });
        }
    }

    pub fn set_camera_host_window_status(
        &self,
        status: Option<razer_model::host_window_focus::HostWindowStatus>,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.device.product_id, 3594..=3596) {
            if let FamilyBody::Controls(body) = &self.body {
                body.update(cx, |body, cx| {
                    body.set_camera_host_window_status(status, cx)
                });
            }
        }
    }

    fn sync_audio_mixer_activity(&self, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            let active = self.device.product_id == 1342
                && matches!(
                    self.device.dashboard.connection_observation,
                    Some(
                        razer_model::model::DeviceConnectionObservation::UsbPresent
                            | razer_model::model::DeviceConnectionObservation::HidPresent
                    )
                );
            body.update(cx, |body, cx| body.set_mixer_active(active, cx));
        }
    }
    pub fn audio_mixer_page(&self) -> Option<Entity<super::audio_products::AudioProductWorkspace>> {
        match &self.body {
            FamilyBody::Audio(body) => Some(body.clone()),
            _ => None,
        }
    }
    fn sync_mouse_mapping_activity(&self, cx: &mut Context<Self>) {
        if !matches!(self.device.product_id, 70 | 158) || self.exiting {
            return;
        }
        let FamilyBody::Mouse(body) = &self.body else {
            return;
        };
        let container = mapping_reconnect::ordinary_mouse_owner(&self.device);
        let document = if container.is_some() {
            match self.mapping_reconnect.document(&self.saved) {
                Ok(document) => document,
                Err(error) => {
                    eprintln!("[saved mouse mapping compilation] {error:#}");
                    None
                }
            }
        } else {
            None
        };
        body.update(cx, |body, cx| {
            if self.device.product_id == 158 {
                body.set_receiver_mapping_connection_generation(
                    container.clone(),
                    self.mapping_connection_generation,
                );
            } else {
                body.set_legacy_mapping_connection_generation(
                    container.clone(),
                    self.mapping_connection_generation,
                );
            }
            if let (Some(container), Some(document)) = (container, document) {
                if self.device.product_id == 158 {
                    body.publish_receiver_mapping(container, document, cx);
                } else {
                    body.publish_legacy_mapping(container, document, cx);
                }
            }
        });
    }

    fn sync_keyboard_read_activity(&self, cx: &mut Context<Self>) {
        if let FamilyBody::Keyboard(body) = &self.body {
            let observation_target = (!self.exiting
                && matches!(
                    self.device.dashboard.connection_observation,
                    Some(
                        razer_model::model::DeviceConnectionObservation::UsbPresent
                            | razer_model::model::DeviceConnectionObservation::HidPresent
                    )
                ))
            .then(|| {
                razer_ipc::mapping_observation::Target::new(
                    self.device.product_id,
                    self.device.device_container_id.clone(),
                )
            });
            let observing = self.active
                && self
                    .current_page()
                    .is_some_and(|page| page.kind().key() == "ACTUATION");
            if !self.exiting && matches!(self.device.product_id, 515 | 679 | 740 | 746) {
                let connected = matches!(
                    self.device.dashboard.connection_observation,
                    Some(
                        razer_model::model::DeviceConnectionObservation::UsbPresent
                            | razer_model::model::DeviceConnectionObservation::HidPresent
                    )
                );
                let container = connected.then(|| self.device.device_container_id.clone());
                let document = match self.mapping_reconnect.document(&self.saved) {
                    Ok(document) => document,
                    Err(error) => {
                        eprintln!("[saved keyboard mapping compilation] {error:#}");
                        None
                    }
                };
                body.update(cx, |body, cx| {
                    body.set_legacy_mapping_connection(container.clone());
                    if let (Some(container), Some(document)) = (container, document) {
                        body.publish_legacy_mapping(container, document, cx);
                    }
                });
            }
            body.update(cx, |body, cx| {
                body.set_snap_observation_target(observation_target.clone(), observing, cx);
                body.set_gamepad_observation_target(observation_target.clone(), self.active, cx);
                body.set_keypad_observation_target(observation_target.clone(), cx);
                body.set_analog_observation_target(observation_target, self.active, cx);
            });
            let active = self.active
                && self
                    .current_page()
                    .is_some_and(|page| page.kind().key() == "TAB_LIGHTING")
                && matches!(
                    self.device.dashboard.connection_observation,
                    Some(
                        razer_model::model::DeviceConnectionObservation::UsbPresent
                            | razer_model::model::DeviceConnectionObservation::HidPresent
                            | razer_model::model::DeviceConnectionObservation::ReceiverPeer(1)
                    )
                );
            body.update(cx, |body, cx| body.set_brightness_read_active(active, cx));
        }
    }
    fn sync_receiver_read_activity(&self, cx: &mut Context<Self>) {
        let active = self.active
            && self
                .current_page()
                .is_some_and(|page| page.kind().key() == "TAB_CUSTOMIZE")
            && receiver_read_owner(&self.device);
        if let FamilyBody::Controls(body) = &self.body {
            body.update(cx, |body, cx| body.set_receiver_active(active, cx));
            let brightness_active = self.active
                && matches!(self.device.product_id, 164 | 241)
                && self
                    .current_page()
                    .is_some_and(|page| page.kind().key() == "TAB_LIGHTING")
                && uuid::Uuid::parse_str(&self.device.device_container_id)
                    .is_ok_and(|id| !id.is_nil())
                && matches!(
                    self.device.dashboard.connection_observation,
                    Some(
                        razer_model::model::DeviceConnectionObservation::UsbPresent
                            | razer_model::model::DeviceConnectionObservation::HidPresent
                    )
                );
            body.update(cx, |body, cx| {
                body.set_brightness_read_active(brightness_active, cx)
            });
        } else if let Some(body) = &self.supplement {
            body.update(cx, |body, cx| body.set_receiver_active(active, cx));
        }
        if let Some(dock) = &self.dock_pairing {
            let serial = self.device.serial_number.to_ascii_uppercase();
            let active = self.active
                && self.current_page().is_some_and(|page| {
                    super::dock_pairing::supports_page(self.device.product_id, page.kind().key())
                })
                && !serial.starts_with("PREVIEW-")
                && !serial.starts_with("DEMO-")
                && uuid::Uuid::parse_str(&self.device.device_container_id)
                    .is_ok_and(|id| !id.is_nil())
                && matches!(
                    self.device.dashboard.connection_observation,
                    Some(
                        razer_model::model::DeviceConnectionObservation::UsbPresent
                            | razer_model::model::DeviceConnectionObservation::HidPresent
                    )
                );
            dock.update(cx, |dock, cx| dock.set_active(active, cx));
        }
        if let Some(widget) = &self.dual_link_widget {
            let active = self.active
                && self
                    .current_page()
                    .is_some_and(|page| page.kind().key() == "TAB_CUSTOMIZE")
                && !self
                    .device
                    .serial_number
                    .to_ascii_uppercase()
                    .starts_with("PREVIEW-")
                && !self
                    .device
                    .serial_number
                    .to_ascii_uppercase()
                    .starts_with("DEMO-")
                && matches!(
                    self.device.dashboard.connection_observation,
                    Some(
                        razer_model::model::DeviceConnectionObservation::UsbPresent
                            | razer_model::model::DeviceConnectionObservation::HidPresent
                    )
                );
            widget.update(cx, |widget, cx| widget.set_active(active, cx));
        }
    }

    pub fn calibration_generation(&self, cx: &App) -> Option<u64> {
        match &self.body {
            FamilyBody::Gamepad(body) => body.read(cx).calibration_generation(),
            _ => None,
        }
    }

    pub fn trigger_calibration_generation(&self, cx: &App) -> Option<u64> {
        match &self.body {
            FamilyBody::Gamepad(body) => body.read(cx).trigger_calibration_generation(),
            _ => None,
        }
    }
    pub fn trigger_calibration_focus_pending(&self, cx: &App) -> bool {
        match &self.body {
            FamilyBody::Gamepad(body) => body.read(cx).trigger_calibration_focus_pending(),
            _ => false,
        }
    }
    pub fn observe_trigger_calibration(
        &mut self,
        observation: super::gamepad_products::TriggerCalibrationObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.active {
            return false;
        }
        match &self.body {
            FamilyBody::Gamepad(body) => body.update(cx, |body, cx| {
                body.observe_trigger_calibration(observation, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_trigger_calibration_focus(
        &mut self,
        generation: u64,
        active: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            FamilyBody::Gamepad(body) => body.update(cx, |body, cx| {
                body.observe_trigger_calibration_focus(generation, active, window, cx)
            }),
            _ => false,
        }
    }
    pub fn finish_trigger_calibration_submission(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            FamilyBody::Gamepad(body) => body.update(cx, |body, cx| {
                body.finish_trigger_calibration_submission(generation, result, cx)
            }),
            _ => false,
        }
    }

    pub fn calibration_intent(
        &self,
        cx: &App,
    ) -> Option<super::gamepad_products::CalibrationIntent> {
        match &self.body {
            FamilyBody::Gamepad(body) => body.read(cx).calibration_intent().cloned(),
            _ => None,
        }
    }

    pub fn observe_calibration(
        &mut self,
        observation: super::gamepad_products::CalibrationObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        // Hm/Zl mount Mm/cc from THUMBSTICKS too. The family body checks
        // the actual page or mounted popup; an outer tab-only filter drops
        // genuine progress from that source caller.
        if !self.active {
            return false;
        }
        match &self.body {
            FamilyBody::Gamepad(body) => body.update(cx, |body, cx| {
                body.observe_calibration(observation, window, cx)
            }),
            _ => false,
        }
    }

    fn sync_mouse_active(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(body) = &self.body {
            let active = self.active
                && self
                    .current_page()
                    .is_some_and(|page| page.role() != ProductPageRole::Help);
            body.update(cx, |body, cx| body.set_active(active, window, cx));
        }
    }

    pub fn legacy_dpi_request_matches(
        &self,
        request: &super::mouse_products::MouseLegacyDpiRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).legacy_dpi_request_matches(request, cx))
    }
    pub fn seiren_eq_request_matches(
        &self,
        request: &super::audio_products::SeirenEqRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Audio(body) if body.read(cx).seiren_eq_request_matches(request, cx))
    }
    pub fn seiren_audio_capture_request_matches(
        &self,
        request: &super::audio_products::SeirenAudioCaptureRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Audio(body) if body.read(cx).seiren_audio_capture_request_matches(request, cx))
    }
    pub fn observe_seiren_audio_capture(
        &mut self,
        request: &super::audio_products::SeirenAudioCaptureRequested,
        frame: razer_device::seiren_audio_capture::CaptureFrame,
        cx: &mut Context<Self>,
    ) -> bool {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_seiren_audio_capture(request, frame, cx)
            })
        } else {
            false
        }
    }
    pub fn finish_seiren_audio_capture(
        &mut self,
        request: &super::audio_products::SeirenAudioCaptureRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_seiren_audio_capture(request, error, cx)
            });
        }
    }
    pub fn cancel_seiren_audio_capture(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, _| body.cancel_seiren_audio_capture());
        }
    }
    pub fn observe_seiren_host_visibility(&mut self, visible: bool, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_seiren_host_visibility(visible, cx)
            });
        }
    }
    pub fn observe_seiren_stream_mixer(
        &mut self,
        observation: super::audio_products::SeirenStreamMixerObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_seiren_stream_mixer(observation, window, cx)
            });
        }
    }
    pub fn seiren_stream_mixer_request_matches(
        &self,
        request: &super::audio_products::SeirenStreamMixerRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body,FamilyBody::Audio(body) if body.read(cx).seiren_stream_mixer_request_matches(request))
    }
    pub fn seiren_stream_mixer_capture_matches(
        &self,
        request: &super::audio_products::SeirenStreamMixerCaptureChanged,
        cx: &App,
    ) -> bool {
        matches!(&self.body,FamilyBody::Audio(body) if body.read(cx).seiren_stream_mixer_capture_matches(request))
    }
    pub fn observe_seiren_document_version(&mut self, version: u64, cx: &mut Context<Self>) {
        if matches!(self.device.product_id, 1422 | 1446) {
            self.device
                .source_device_settings
                .get_or_insert_with(|| serde_json::json!({}))["version"] =
                serde_json::json!(version);
            self.saved
                .source_device_settings
                .get_or_insert_with(|| serde_json::json!({}))["version"] =
                serde_json::json!(version);
            cx.notify();
        }
    }
    pub fn observe_seiren_stream_mixer_mode(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_seiren_stream_mixer_mode(enabled, cx)
            });
        }
    }
    pub fn finish_seiren_stream_mixer(
        &mut self,
        request: &super::audio_products::SeirenStreamMixerRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_seiren_stream_mixer(request, error, cx)
            });
        }
    }
    pub fn observe_seiren_live(
        &mut self,
        observed: razer_device::seiren_mixer::LiveState,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_seiren_live(observed, window, cx)
            });
        }
    }
    pub fn record_seiren_live(
        &mut self,
        observed: razer_device::seiren_mixer::LiveState,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| body.record_seiren_live(observed, cx));
        }
    }
    pub fn record_seiren_native_event(&mut self, event: serde_json::Value, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| body.record_seiren_native_event(event, cx));
        }
    }
    pub fn record_seiren_stream_mixer(
        &mut self,
        observation: super::audio_products::SeirenStreamMixerObservation,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.record_seiren_stream_mixer(observation, cx)
            });
        }
    }
    pub fn sync_seiren_audio_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| body.sync_seiren_audio_controls(window, cx));
        }
    }
    pub fn observe_seiren_native_event(
        &mut self,
        event: serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_seiren_native_event(event, window, cx)
            });
        }
    }
    pub fn observe_seiren_sampling_ready(&mut self, ready: bool, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| body.observe_seiren_sampling_ready(ready, cx));
        }
    }
    pub fn set_seiren_microphone_setup_seen(
        &mut self,
        seen: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.set_seiren_microphone_setup_seen(seen, window, cx)
            });
        }
    }
    pub fn seiren_audio_playback_request_matches(
        &self,
        request: &super::audio_products::SeirenAudioPlaybackRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body,FamilyBody::Audio(body) if body.read(cx).seiren_audio_playback_request_matches(request))
    }
    pub fn observe_seiren_audio_playback_started(
        &mut self,
        request: &super::audio_products::SeirenAudioPlaybackRequested,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_seiren_audio_playback_started(request, cx)
            });
        }
    }
    pub fn finish_seiren_audio_playback(
        &mut self,
        request: &super::audio_products::SeirenAudioPlaybackRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_seiren_audio_playback(request, error, cx)
            });
        }
    }
    pub fn observe_seiren_native_levels(
        &mut self,
        levels: &razer_device::seiren_eq::NativeLevels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_seiren_native_levels(levels, window, cx)
            });
        }
    }
    pub fn finish_seiren_eq(
        &mut self,
        request: &super::audio_products::SeirenEqRequested,
        observed: Option<razer_device::seiren_eq::Report>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| body.finish_seiren_eq(request, observed, cx));
        }
    }
    pub fn prepare_seiren_eq_exit(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, _| body.prepare_seiren_eq_exit());
        }
    }
    pub fn seiren_audio_exit_pending(&self, cx: &App) -> bool {
        matches!(&self.body, FamilyBody::Audio(body) if body.read(cx).seiren_audio_exit_pending())
    }
    pub fn resume_seiren_audio_after_cancelled_exit(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, _| {
                body.resume_seiren_audio_after_cancelled_exit()
            });
        }
    }
    pub fn controller_trigger_request_matches(
        &self,
        request: &super::gamepad_products::ControllerTriggerRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Gamepad(body) if body.read(cx).controller_trigger_request_matches(request))
    }
    pub fn controller_sleep_request_matches(
        &self,
        request: &super::gamepad_products::ControllerSleepRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Gamepad(body) if body.read(cx).controller_sleep_request_matches(request))
    }
    pub fn finish_controller_sleep(
        &mut self,
        request: &super::gamepad_products::ControllerSleepRequested,
        observed: Option<razer_device::controller_sleep::SleepResult>,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_controller_sleep(request, observed, error, cx)
            });
        }
    }
    pub fn observe_controller_sleep_recording(&mut self, recording: bool, cx: &mut Context<Self>) {
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_controller_sleep_recording(recording, cx)
            });
        }
    }
    pub fn controller_lighting_request_matches(
        &self,
        request: &super::gamepad_products::ControllerLightingRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Gamepad(body) if body.read(cx).controller_lighting_request_matches(request))
    }
    pub fn accepts_controller_chroma(
        &self,
        request: &super::gamepad_products::ControllerChromaRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Gamepad(body) if body.read(cx).accepts_controller_chroma(request))
    }
    pub fn observe_controller_chroma(
        &mut self,
        observation: super::gamepad_products::ControllerChromaObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_controller_chroma(observation, window, cx);
            });
        }
    }
    pub fn apply_controller_sync_quick(
        &mut self,
        source_effect: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        match &self.body {
            FamilyBody::Gamepad(body) => body.update(cx, |body, cx| {
                body.apply_controller_sync_quick(source_effect, window, cx)
            }),
            _ => Ok(false),
        }
    }
    pub fn complete_controller_chroma(
        &mut self,
        request: &super::gamepad_products::ControllerChromaRequested,
        result: Result<Option<super::gamepad_products::ControllerChromaObservation>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| {
                body.complete_controller_chroma(request, result, window, cx);
            });
        }
    }
    pub fn finish_controller_lighting(
        &mut self,
        request: &super::gamepad_products::ControllerLightingRequested,
        result: Result<razer_device::controller_lighting::LightingResult, String>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_controller_lighting(request, result, window, cx)
            });
        }
    }
    pub fn observe_controller_input(
        &mut self,
        sample: Option<&razer_device::controller_input::Sample>,
        events: &[razer_device::controller_input::Event],
        cx: &mut Context<Self>,
    ) {
        let mut status_changed = false;
        if sample.is_none() && events.is_empty() {
            status_changed = self.device.clear_controller_input_observations();
            self.saved.clear_controller_input_observations();
        } else {
            for event in events {
                status_changed |= self.device.observe_controller_notification(event);
                self.saved.observe_controller_notification(event);
            }
        }
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_controller_input(sample, events, cx)
            });
        }
        if status_changed {
            cx.notify();
        }
    }
    pub fn finish_controller_trigger(
        &mut self,
        request: &super::gamepad_products::ControllerTriggerRequested,
        observed: Option<Value>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_controller_trigger(request, observed, window, cx)
            });
        }
    }
    pub fn current_power_request_matches(
        &self,
        request: &super::mouse_products::MouseCurrentPowerRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).current_power_request_matches(request, cx))
    }
    pub fn finish_current_power(
        &mut self,
        request: &super::mouse_products::MouseCurrentPowerRequested,
        observed: Option<razer_device::mouse_current_power::Report>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_current_power(request, observed, cx)
            });
        }
    }
    pub fn prepare_current_power_exit(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<super::mouse_products::MouseCurrentPowerRequested> {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.prepare_current_power_exit(cx))
        } else {
            None
        }
    }
    pub fn current_dpi_request_matches(
        &self,
        request: &super::mouse_products::MouseCurrentDpiRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).current_dpi_request_matches(request, cx))
    }
    pub fn mouse_scroll_request_matches(
        &self,
        request: &super::mouse_products::MouseScrollRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).mouse_scroll_request_matches(request, cx))
    }
    pub fn mouse_scroll_save_matches(
        &self,
        request: &super::mouse_products::MouseScrollRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).mouse_scroll_save_matches(request, cx))
    }
    pub fn finish_mouse_scroll(
        &mut self,
        request: &super::mouse_products::MouseScrollRequested,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.finish_mouse_scroll(request, cx));
        }
    }
    pub fn prepare_mouse_scroll_exit(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, _| body.prepare_mouse_scroll_exit());
        }
    }
    pub fn resume_mouse_scroll_after_cancelled_exit(&mut self, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, _| {
                body.resume_mouse_scroll_after_cancelled_exit()
            });
        }
    }
    pub fn finish_mouse_scroll_report(
        &mut self,
        request: &super::mouse_products::MouseScrollRequested,
        report: Option<&razer_device::mouse_scroll::SubmissionReport>,
        cleanup_succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_mouse_scroll_report(request, report, cleanup_succeeded, cx)
            });
        }
    }
    pub fn sync_mouse_scroll_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.sync_mouse_scroll_controls(window, cx));
        }
    }
    pub fn finish_current_dpi(
        &mut self,
        request: &super::mouse_products::MouseCurrentDpiRequested,
        observed: Option<razer_device::mouse_current_dpi::DpiReport>,
        cx: &mut Context<Self>,
    ) {
        self.finish_current_dpi_result(
            request,
            observed.ok_or_else(|| "DPI 设备操作未完成".to_owned()),
            cx,
        );
    }
    pub fn finish_current_dpi_result(
        &mut self,
        request: &super::mouse_products::MouseCurrentDpiRequested,
        result: Result<razer_device::mouse_current_dpi::DpiReport, String>,
        cx: &mut Context<Self>,
    ) {
        let mut source_serial_changed = false;
        if self.current_dpi_request_matches(request, cx)
            && let Some(serial) = result
                .as_ref()
                .ok()
                .and_then(|report| report.serial_number.as_ref())
        {
            self.device.serial_number = serial.clone();
            // Identity telemetry survives profile Cancel/restore. It is not a
            // profile edit and must not be reverted to the pre-query serial.
            self.saved.serial_number = serial.clone();
            if razer_storage::host_local_profile::supported(self.device.product_id)
                && !serial.is_empty()
                && self.mouse_profile_observed_serial.as_ref() != Some(serial)
            {
                self.mouse_profile_observed_serial = Some(serial.clone());
                source_serial_changed = true;
            }
        }
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_current_dpi_result(request, result, cx)
            });
        }
        if source_serial_changed {
            cx.emit(WorkspaceEvent::MouseProfileSerialObserved);
        }
    }
    pub fn sync_current_dpi_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.sync_current_dpi_controls(window, cx));
        }
    }
    pub fn accept_current_dpi_local_profile(
        &mut self,
        request: &super::mouse_products::MouseCurrentDpiRequested,
        baseline: &Device,
        serial: Option<&str>,
        result: Result<Option<razer_storage::host_local_profile::LocalProfiles>, String>,
        cx: &mut Context<Self>,
    ) {
        let FamilyBody::Mouse(body) = &self.body else {
            return;
        };
        if !body.read(cx).current_dpi_profile_read_matches(request, cx) {
            return;
        }
        if let Some(serial) = serial {
            self.device.serial_number = serial.to_owned();
            self.saved.serial_number = serial.to_owned();
        }
        let mut changed = false;
        let result = result.and_then(|source| {
            if let Some(source) = source {
                changed =
                    razer_storage::host_local_profile::import(&mut self.device, baseline, &source)
                        .map_err(|e| format!("{e:#}"))?;
            } else {
                return razer_storage::host_local_profile::stages_without_host(&self.device)
                    .map_err(|e| format!("{e:#}"));
            }
            razer_storage::host_local_profile::known_stages(&self.device)
                .map_err(|e| format!("{e:#}"))
        });
        body.update(cx, |body, cx| {
            body.restore_current_dpi_local_profile(result, cx)
        });
        if changed {
            self.persist_source_profile(false, cx);
            if self.mouse_profile_observed_serial.as_deref()
                == Some(self.device.serial_number.as_str())
            {
                cx.emit(WorkspaceEvent::MouseProfileSerialObserved);
            }
        }
    }
    pub fn restore_mouse_profile_after_serial(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !razer_storage::host_local_profile::supported(self.device.product_id)
            || self.mouse_profile_observed_serial.as_deref()
                != Some(self.device.serial_number.as_str())
        {
            return;
        }
        self.refresh_profile_choices(window, cx);
        self.restore_active_with_dpi_observation(true, window, cx);
        cx.notify();
    }
    fn sync_mouse_profile_selection(&mut self) {
        if self.mouse_profile_observed_serial.as_deref() == Some(self.device.serial_number.as_str())
        {
            mouse_profile_source::sync_selection(&mut self.device, true);
        }
    }
    pub fn initialize_current_mouse_profile(
        &mut self,
        key: Option<String>,
        macro_recording: bool,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, _| {
                body.observe_current_dpi_macro_recording(macro_recording)
            });
        }
        if !razer_storage::host_local_profile::supported(self.device.product_id) {
            if let (FamilyBody::Mouse(body), Some(profile)) =
                (&self.body, self.device.active_profile_obj())
            {
                body.update(cx, |body, cx| {
                    body.initialize_current_mouse_polling(key.as_deref(), &profile.guid, cx);
                    body.observe_current_dpi_connection(
                        key.map(|key| format!("{key}:{}", profile.guid)),
                        cx,
                    );
                });
            }
            return;
        }
        if self.mouse_profile_connection_key != key {
            self.mouse_profile_connection_key = key.clone();
            self.mouse_profile_observed_serial = None;
        }
        if key.is_some() && self.mouse_profile_observed_serial.is_none() {
            if let FamilyBody::Mouse(body) = &self.body {
                body.update(cx, |body, cx| body.request_current_dpi_identity(cx));
            }
            return;
        }
        if let FamilyBody::Mouse(body) = &self.body {
            let Some(profile) = self.device.active_profile_obj() else {
                return;
            };
            body.update(cx, |body, cx| {
                // An absent application cache does not prove first-use device
                // initialization. Observe before any explicit profile action.
                body.initialize_current_mouse_polling(key.as_deref(), &profile.guid, cx);
                body.observe_current_dpi_connection(
                    key.map(|key| format!("{key}:{}", profile.guid)),
                    cx,
                );
            });
        }
    }
    pub fn current_dpi_profile_save_boundary(
        &self,
        scope: super::mouse_polling::MousePollingScope,
        cx: &App,
    ) -> Option<super::mouse_products::MouseCurrentDpiProfileSaveBoundary> {
        if let FamilyBody::Mouse(body) = &self.body {
            body.read(cx).current_dpi_profile_save_boundary(scope, cx)
        } else {
            None
        }
    }
    pub fn current_dpi_matcher_stages_saved(
        &mut self,
        boundary: &super::mouse_products::MouseCurrentDpiProfileSaveBoundary,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.current_dpi_matcher_stages_saved(boundary, cx)
            });
        }
    }
    pub fn prepare_current_dpi_exit(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<Option<super::mouse_products::MouseCurrentDpiRequested>, String> {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.prepare_current_dpi_exit(cx))
        } else {
            Ok(None)
        }
    }
    pub fn current_dpi_exit_pending(&self, cx: &App) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).current_dpi_exit_pending())
    }
    pub fn prepare_legacy_dpi_exit(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<Option<super::mouse_products::MouseLegacyDpiRequested>, String> {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.prepare_legacy_dpi_exit(cx))
        } else {
            Ok(None)
        }
    }
    pub fn legacy_dpi_exit_pending(&self, cx: &App) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).legacy_dpi_exit_pending())
    }
    pub fn finish_legacy_dpi_result(
        &mut self,
        request: &super::mouse_products::MouseLegacyDpiRequested,
        confirmed: bool,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_legacy_dpi_result(request, confirmed, cx)
            });
        }
    }
    pub fn finish_legacy_dpi(
        &mut self,
        request: &super::mouse_products::MouseLegacyDpiRequested,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.finish_legacy_dpi(request, cx));
        }
    }

    pub fn mouse_polling_scope(&self, cx: &App) -> Option<super::mouse_polling::MousePollingScope> {
        match &self.body {
            FamilyBody::Mouse(body) => body.read(cx).mouse_polling_scope(cx),
            _ => None,
        }
    }
    pub fn mouse_polling_device_hz(
        &self,
        scope: super::mouse_polling::MousePollingScope,
        field: super::mouse_polling::PollingField,
        hz: u32,
        cx: &App,
    ) -> Option<u32> {
        match &self.body {
            FamilyBody::Mouse(body) => body.read(cx).mouse_polling_device_hz(scope, field, hz),
            _ => None,
        }
    }
    pub fn mouse_polling_synchronizes_profile(
        &self,
        scope: super::mouse_polling::MousePollingScope,
        field: super::mouse_polling::PollingField,
        hz: u32,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).mouse_polling_synchronizes_profile(scope, field, hz))
    }
    pub fn mouse_polling_request_matches(
        &self,
        scope: super::mouse_polling::MousePollingScope,
        field: super::mouse_polling::PollingField,
        hz: u32,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body)
            if body.read(cx).mouse_polling_request_matches(scope, field, hz, cx))
    }
    pub fn mouse_polling_in_flight_matches(
        &self,
        scope: super::mouse_polling::MousePollingScope,
        field: super::mouse_polling::PollingField,
        hz: u32,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body)
            if body.read(cx).mouse_polling_in_flight_matches(scope, field, hz))
    }
    pub fn mouse_polling_write_guard(
        &self,
        scope: super::mouse_polling::MousePollingScope,
        field: super::mouse_polling::PollingField,
        hz: u32,
        cx: &App,
    ) -> Option<std::sync::Arc<std::sync::atomic::AtomicBool>> {
        match &self.body {
            FamilyBody::Mouse(body) => body.read(cx).mouse_polling_write_guard(scope, field, hz),
            _ => None,
        }
    }
    pub fn finish_mouse_polling(
        &mut self,
        scope: super::mouse_polling::MousePollingScope,
        field: super::mouse_polling::PollingField,
        hz: u32,
        observed: Option<u32>,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| {
                body.finish_mouse_polling(scope, field, hz, observed, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_mouse_polling(
        &mut self,
        scope: super::mouse_polling::MousePollingScope,
        observation: super::mouse_polling::MousePollingObservation,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_mouse_polling(scope, observation, cx)
            });
        }
    }

    pub fn observe_dpi_editing_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_dpi_editing_enabled(enabled, window, cx)
            });
        }
    }
    pub fn observe_scroll_wheel(
        &mut self,
        observation: super::ScrollWheelObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_scroll_wheel(observation, window, cx)
            });
        }
    }
    pub fn captures_snap_keys(&self, cx: &App) -> bool {
        self.current_page()
            .is_some_and(|page| page.role() != ProductPageRole::Help)
            && matches!(&self.body, FamilyBody::Keyboard(body) if body.read(cx).captures_snap_keys())
    }
    pub fn capture_snap_key_up(&mut self, event: &KeyUpEvent, cx: &mut Context<Self>) {
        if !self.captures_snap_keys(cx) {
            return;
        }
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.capture_snap_key_up(event, cx));
        }
    }
    pub fn observe_snap_tap(
        &mut self,
        observation: super::SnapTapObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_snap_tap(observation, window, cx)
            });
        }
    }
    pub fn observe_stream_mixer(
        &mut self,
        observation: super::StreamMixerObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_stream_mixer(observation, window, cx)
            });
        }
    }

    /// Complete current monitor-service snapshot, independent from profiles.
    pub fn observe_monitor_runtime(
        &mut self,
        runtime: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.device.product_id, 3858 | 3880) {
            if let FamilyBody::AccessorySystem(body) = &self.body {
                body.update(cx, |body, cx| body.set_monitor_runtime(runtime, window, cx));
            }
        }
    }
    /// Reserved for the current host's observed validDevices stream, never preview data.
    #[expect(
        dead_code,
        reason = "Current host runtime publisher is not yet connected"
    )]
    pub fn observe_pod_runtime_devices(
        &mut self,
        devices: Vec<super::audio_products::RuntimeAudioDevice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_pod_runtime_devices(devices, window, cx)
            });
        }
    }
    pub fn observe_oled_runtime(
        &mut self,
        observation: super::OledRuntimeObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_oled_runtime(observation, window, cx)
            });
        }
    }
    /// Report pages backed by entities this workspace actually owns. A help
    /// renderer or a registered navigation with only Pending content is not a
    /// locally implemented product destination.
    pub fn has_local_page(&self) -> bool {
        let Some(navigation) = product::registered(self.device.product_id)
            .and_then(|product| product.primary_navigation())
        else {
            return false;
        };
        navigation.pages().iter().any(|page| {
            if page.role() == ProductPageRole::Help {
                return false;
            }
            let key = page.kind().key();
            if self.dock_pairing.is_some()
                && super::dock_pairing::supports_page(self.device.product_id, key)
            {
                return true;
            }
            if self
                .accessory
                .as_ref()
                .is_some_and(|view| view.supports_page(self.device.product_id, key))
                || (self.supplement.is_some() && self.use_supplement_for(key))
            {
                return true;
            }
            match &self.body {
                FamilyBody::Audio(_) => {
                    super::audio_products::supports_page(self.device.product_id, key)
                }
                FamilyBody::AccessorySystem(_) => {
                    super::accessory_system_products::supports_page(self.device.product_id, key)
                }
                FamilyBody::Controls(_) => {
                    super::source_controls::supports_page(self.device.product_id, key)
                }
                FamilyBody::Pending => false,
                FamilyBody::Mouse(_)
                | FamilyBody::Keyboard(_)
                | FamilyBody::Gamepad(_)
                | FamilyBody::System(_)
                | FamilyBody::Hue(_) => true,
            }
        })
    }

    pub(super) fn select_profile(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.select_source_profile(id, true, window, cx);
    }

    pub(super) fn change_profile_metadata(
        &mut self,
        id: &str,
        change: super::product_workspace::ProfileMetadata,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if super::product_workspace::edit_profile_metadata(&mut self.device, id, change) {
            self.sync_mouse_profile_selection();
            self.profile.update(cx, |state, cx| {
                state.set_items(
                    self.device
                        .profiles
                        .iter()
                        .map(|p| Choice::new(&p.id, &p.name))
                        .collect(),
                    window,
                    cx,
                )
            });
            self.sync_keyboard_mapping_profiles(window, cx);
            self.persist_source_profile(false, cx);
            cx.emit(WorkspaceEvent::Changed);
            cx.notify();
        }
    }
    pub(super) fn change_profile_collection(
        &mut self,
        action: super::product_workspace::ProfileCollectionAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        let active = self.device.active_profile.clone();
        if !matches!(self.device.product_id, 70 | 182 | 756 | 3893)
            && matches!(&action, super::product_workspace::ProfileCollectionAction::Delete { id, .. } if id == &active)
            && active_profile_settings_dirty(&self.device, &self.saved)
        {
            return Err("请先保存或取消当前配置的本地设置，再删除活动配置文件。".into());
        }
        if matches!(&action, super::product_workspace::ProfileCollectionAction::Delete { id, .. } if id == &active)
            && let FamilyBody::Audio(body) = &self.body
            && body.read(cx).pod_editor_dirty(cx)
        {
            return Err("请先保存或取消当前音频编辑，再删除活动配置文件".into());
        }
        let removed_guid =
            if let super::product_workspace::ProfileCollectionAction::Delete { id, .. } = &action {
                self.device
                    .profiles
                    .iter()
                    .find(|profile| &profile.id == id)
                    .map(|profile| profile.guid.clone())
            } else {
                None
            };
        let target = super::product_workspace::edit_profile_collection(
            &mut self.device,
            &self.saved,
            action,
        )?;
        let hanbo_profile_deleted = self.device.product_id
            == razer_device::hanbo_cooler::PRODUCT_ID
            && removed_guid.is_some();
        if let Some(guid) = removed_guid {
            mouse_profile_source::removed_profile(&mut self.device, &guid);
        }
        self.sync_mouse_profile_selection();
        self.refresh_profile_choices(window, cx);
        if active != self.device.active_profile {
            self.keyboard_linked_base_selected(&self.device.active_profile.clone(), cx);
            self.dismiss_profile_dialog(window, cx);
            self.restore_active(window, cx);
            self.restore_seiren_selected_profile(cx);
        }
        self.persist_source_profile(
            active != self.device.active_profile || hanbo_profile_deleted,
            cx,
        );
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
        Ok(target)
    }

    fn restore_seiren_selected_profile(&self, cx: &mut Context<Self>) {
        if !matches!(
            self.device.dashboard.connection_observation,
            Some(
                razer_model::model::DeviceConnectionObservation::UsbPresent
                    | razer_model::model::DeviceConnectionObservation::HidPresent
            )
        ) {
            return;
        }
        if let FamilyBody::Audio(body) = &self.body {
            body.update(cx, |body, cx| body.request_seiren_profile_restore(cx));
        }
    }
    fn persist_source_profile(&mut self, apply: bool, cx: &mut Context<Self>) {
        if apply && razer_storage::host_local_profile::supported(self.device.product_id) {
            let result = (|| -> Result<_, String> {
                let profile = self
                    .device
                    .active_profile_obj()
                    .ok_or("active DPI profile is missing")?;
                let id = profile.id.clone();
                let source = profile
                    .source_settings
                    .clone()
                    .ok_or("selected DPI profile has no settings")?;
                razer_storage::host_local_profile::mark_edited(&mut self.device, &id, &source)
                    .map_err(|error| format!("{error:#}"))?;
                razer_storage::host_local_profile::known_stages(&self.device)
                    .map_err(|error| format!("{error:#}"))
            })();
            let failed = result.is_err();
            if let FamilyBody::Mouse(body) = &self.body {
                body.update(cx, |body, cx| {
                    body.restore_current_dpi_local_profile(result, cx)
                });
            }
            if failed {
                return;
            }
        }
        if apply {
            if let FamilyBody::Mouse(body) = &self.body {
                body.update(cx, |body, cx| body.apply_mouse_rotation_profile(cx));
            }
        }
        if matches!(self.device.product_id, 164 | 241) {
            if apply {
                self.request_receiver_profile_selection(cx);
            }
            return;
        }
        if self.device.product_id == razer_device::hanbo_cooler::PRODUCT_ID {
            cx.emit(WorkspaceEvent::HanboProfileRequested(apply.then_some(
                razer_device::hanbo_settings::ProfileRestore::Changes,
            )));
            return;
        }
        if self.device.product_id == 182 || matches!(self.device.product_id, 158 | 190) && !apply {
            if let Some(scope) = self.mouse_polling_scope(cx) {
                cx.emit(WorkspaceEvent::MouseLegacyProfileRequested { scope, apply });
            }
            return;
        }
        if razer_device::mouse_current_dpi::capability(self.device.product_id).is_some() && apply {
            if let FamilyBody::Mouse(body) = &self.body {
                body.update(cx, |body, cx| body.apply_current_dpi_profile(cx));
            }
            return;
        }
        if matches!(self.device.product_id, 515 | 679 | 740 | 746 | 756) {
            cx.emit(WorkspaceEvent::KeyboardLegacyProfileRequested);
            return;
        }
        if self.device.product_id != 70 {
            return;
        }
        if let Some(scope) = self.mouse_polling_scope(cx) {
            cx.emit(WorkspaceEvent::MouseLegacyProfileRequested { scope, apply });
        }
    }

    pub fn apply_legacy_performance_profile(
        &mut self,
        scope: super::mouse_polling::MousePollingScope,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                if self.device.product_id == 182 {
                    if let Some(profile) = self.device.active_profile_obj() {
                        body.initialize_mouse182_profile(
                            self.mouse_profile_connection_key.clone(),
                            &profile.guid,
                            false,
                            cx,
                        );
                    }
                } else {
                    body.apply_legacy_performance_profile(scope, cx)
                }
            });
        }
    }

    pub fn keyboard_preview_page(
        &self,
    ) -> Option<Entity<crate::features::keyboard_products::KeyboardProductWorkspace>> {
        match &self.body {
            FamilyBody::Keyboard(view) => Some(view.clone()),
            _ => None,
        }
    }
    pub fn aether_preview_page(
        &self,
    ) -> Option<Entity<crate::features::aether_strip::AetherStrip>> {
        match &self.accessory {
            Some(accessory::AccessoryPage::Aether { strip: view, .. }) => Some(view.clone()),
            _ => None,
        }
    }
    /// Whether this source product's family renderer can produce its lighting
    /// page. Audio products additionally need an audited root-to-page match:
    /// Hammerhead V3 Chroma calls that page `LIGHTING`, not `TAB_LIGHTING`.
    pub fn supports_lighting_page(&self) -> bool {
        if matches!(&self.body, FamilyBody::Audio(_)) {
            return super::audio_products::supports_chroma_lighting_page(self.device.product_id);
        }
        if !matches!(
            &self.body,
            FamilyBody::Mouse(_)
                | FamilyBody::Keyboard(_)
                | FamilyBody::Gamepad(_)
                | FamilyBody::System(_)
        ) {
            return false;
        }
        product::registered(self.device.product_id)
            .and_then(|product| product.primary_navigation())
            .is_some_and(|navigation| {
                navigation
                    .pages()
                    .iter()
                    .any(|page| page.kind().key() == "TAB_LIGHTING")
            })
    }
    pub fn set_chroma_popup_open(&mut self, owner: EntityId, open: bool, cx: &mut Context<Self>) {
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| body.set_chroma_popup_open(owner, open, cx));
        }
    }
    /// The `displayMode=chromaApp` popup content for a source product: its
    /// lighting page built from the family renderer, without the product chrome.
    pub fn lighting_page_element(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.supports_lighting_page() {
            return None;
        }
        match &self.body {
            FamilyBody::Mouse(view) => {
                Some(view.update(cx, |view, cx| view.lighting_element(window, cx)))
            }
            FamilyBody::Keyboard(view) => {
                Some(view.update(cx, |view, cx| view.lighting_element(window, cx)))
            }
            FamilyBody::Gamepad(view) => {
                Some(view.update(cx, |view, cx| view.lighting_element(window, cx)))
            }
            FamilyBody::System(view) => Some(view.update(cx, |view, cx| view.lighting_element(cx))),
            FamilyBody::Audio(view) => {
                view.update(cx, |view, cx| view.lighting_element(window, cx))
            }
            _ => None,
        }
    }
    /// Whether the source product has an implemented Armory root. Some roots
    /// mount a product image or CoolerInfo instead of the Customize page.
    pub fn supports_mapping_page(&self) -> bool {
        if super::armory_product::supports(&self.device) {
            return true;
        }
        if !matches!(
            &self.body,
            FamilyBody::Mouse(_)
                | FamilyBody::Keyboard(_)
                | FamilyBody::Gamepad(_)
                | FamilyBody::System(_)
        ) {
            return false;
        }
        product::registered(self.device.product_id)
            .and_then(|product| product.primary_navigation())
            .is_some_and(|navigation| {
                navigation
                    .pages()
                    .iter()
                    .any(|page| page.kind().key() == "TAB_CUSTOMIZE")
            })
    }
    /// Keep the independent Armory root active until its actual owner closes.
    pub fn set_armory_mapping_open(&mut self, owner: EntityId, open: bool, cx: &mut Context<Self>) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.set_armory_mapping_open(owner, open, cx));
        }
    }

    /// Select the audited independent Armory component before mapping roots.
    pub fn mapping_page_element(
        &self,
        owner: EntityId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if super::armory_product::supports(&self.device) {
            if self.device.product_id == razer_device::hanbo_cooler::PRODUCT_ID {
                if let FamilyBody::AccessorySystem(body) = &self.body {
                    return Some(body.update(cx, |body, cx| {
                        body.hanbo_armory_element(&self.device, window, cx)
                    }));
                }
            }
            return Some(
                super::armory_product::ArmoryProduct::new(&self.device).into_any_element(),
            );
        }
        if !self.supports_mapping_page() {
            return None;
        }
        match &self.body {
            FamilyBody::Mouse(view) => {
                Some(view.update(cx, |view, cx| view.customize_element(window, cx)))
            }
            FamilyBody::Keyboard(view) => {
                Some(view.update(cx, |view, cx| view.customize_element(owner, window, cx)))
            }
            FamilyBody::Gamepad(view) => {
                Some(view.update(cx, |view, cx| view.customize_element(cx)))
            }
            FamilyBody::System(view) => {
                Some(view.update(cx, |view, cx| view.customize_element(cx)))
            }
            _ => None,
        }
    }
    pub fn new(mut device: Device, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let registered = product::registered(device.product_id);
        Self::migrate_device_settings(&mut device);
        let page = registered
            .and_then(|p| p.primary_navigation())
            .and_then(|n| n.pages().iter().find(|p| p.role() != ProductPageRole::Help))
            .map(|p| p.id());
        let mut subscriptions = vec![];
        let body_scroll = ScrollHandle::new();
        let body = if device.product_id == 769 {
            let body = cx.new(|cx| super::hue::HueWorkspace::new(window, cx));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, _, action: &super::hue::HueChanged, cx| {
                    let Some(settings) = this
                        .device
                        .profiles
                        .iter_mut()
                        .find(|profile| profile.id == this.device.active_profile)
                        .and_then(|profile| profile.source_settings.as_mut())
                    else {
                        return;
                    };
                    match action.apply(settings) {
                        Ok(true) => {
                            cx.emit(WorkspaceEvent::Changed);
                            cx.notify();
                        }
                        Ok(false) => {}
                        Err(error) => eprintln!("[Hue source action] {error}"),
                    }
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_, _, _: &super::hue::HueChromaRequested, cx| {
                    cx.emit(WorkspaceEvent::OpenChroma);
                },
            ));
            FamilyBody::Hue(body)
        } else if super::mouse_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| {
                super::mouse_products::MouseProductWorkspace::new(device.product_id, window, cx)
            });
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, event: &super::MappingCallbackRequested, cx| {
                    cx.emit(WorkspaceEvent::MappingCallbackRequested(event.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, _: &super::mouse_products::MouseMappingMacroRequested, cx| {
                    cx.emit(WorkspaceEvent::OpenMacro);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::mouse_products::MouseMappingTargetRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::MouseMappingTargetRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |this: &mut Self,
                 body,
                 _: &super::mouse_products::MouseProductChanged,
                 window,
                 cx| {
                    body.update(cx, |body, cx| body.refresh_legacy_dpi_controls(window, cx));
                    body.update(cx, |body, cx| body.sync_mouse_matcher_dpi(cx));
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 _: &super::mouse_products::MouseLegacyMappingCommitted,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                    this.persist_source_profile(false, cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, event: &super::mouse_products::MouseLegacyProfileNavigate, cx| {
                    cx.emit(WorkspaceEvent::MouseLegacyProfileNavigate(event.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 request: &super::mouse_products::MousePollingRequested,
                 cx| {
                    if request.is_initialization() {
                        cx.emit(WorkspaceEvent::MousePollingRestoreRequested {
                            scope: request.scope(),
                            field: request.field(),
                            hz: request.hz(),
                        });
                        return;
                    }
                    // Capture the exact optimistic draft before the shell's
                    // serialized local persistence/device-write continuation.
                    this.capture(body.read(cx).snapshot(), cx);
                    cx.emit(WorkspaceEvent::MousePollingRequested {
                        scope: request.scope(),
                        field: request.field(),
                        hz: request.hz(),
                    });
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 _: &super::mouse_products::MouseInGamePollingChanged,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                    cx.emit(WorkspaceEvent::MouseInGamePollingRequested);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::mouse_products::MouseReceiverIndicatorRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::MouseReceiverIndicatorRequested(
                        request.clone(),
                    ));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::mouse_products::MouseReceiverMonitorRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::MouseReceiverMonitorRequested(
                        request.clone(),
                    ));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 request: &super::mouse_products::MouseLegacyDpiRequested,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                    cx.emit(WorkspaceEvent::MouseLegacyDpiRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 request: &super::mouse_products::MouseCurrentPowerRequested,
                 cx| {
                    if !request.is_read() {
                        this.capture(body.read(cx).snapshot(), cx);
                    }
                    cx.emit(WorkspaceEvent::MouseCurrentPowerRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |_: &mut Self,
                 body,
                 read: &super::mouse_products::MouseCurrentPowerRead,
                 window,
                 cx| {
                    body.update(cx, |body, cx| {
                        body.observe_current_power_read(read, window, cx)
                    });
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 request: &super::mouse_products::MouseCurrentDpiRequested,
                 cx| {
                    if request.profile().is_some() {
                        this.capture(body.read(cx).snapshot(), cx);
                    }
                    cx.emit(WorkspaceEvent::MouseCurrentDpiRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 request: &super::mouse_products::MouseScrollRequested,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                    cx.emit(WorkspaceEvent::MouseScrollRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |this: &mut Self,
                 _,
                 navigation: &super::mouse_products::MouseMappingNavigation,
                 window,
                 cx| {
                    match navigation {
                        super::mouse_products::MouseMappingNavigation::Page(key) => {
                            this.set_page_key(key, window, cx)
                        }
                        super::mouse_products::MouseMappingNavigation::History(forward) => {
                            this.step_page_history(*forward, window, cx)
                        }
                    }
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, request: &super::mouse_products::MouseMatcherRequest, cx| {
                    cx.emit(WorkspaceEvent::MouseMatcherRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, request: &super::mouse_products::MouseDynamicRequested, cx| {
                    cx.emit(WorkspaceEvent::MouseDynamicRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, request: &super::mouse_products::MouseRotationRequested, cx| {
                    cx.emit(WorkspaceEvent::MouseRotationRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, error: &super::mouse_products::MouseMappingSaveRejected, cx| {
                    cx.emit(WorkspaceEvent::MouseMappingSaveRejected(*error));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, _: &super::mouse_products::MouseCharacterMapRequested, cx| {
                    // Existing Rust shell command opens the Windows character map.
                    cx.emit(WorkspaceEvent::KeyboardCharacterMapRequested);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 event: &super::mouse_products::MouseDynamicTutorialChanged,
                 cx| {
                    cx.emit(WorkspaceEvent::MouseDynamicTutorialChanged(event.visible()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 event: &super::mouse_products::MouseCalibrationWelcomeChanged,
                 cx| {
                    cx.emit(WorkspaceEvent::MouseCalibrationWelcomeChanged(event.show()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::mouse_products::MouseCalibrationRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::MouseCalibrationRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, request: &super::mouse_products::MouseLightingRequested, cx| {
                    cx.emit(WorkspaceEvent::MouseLightingRequested(request.clone()));
                },
            ));
            FamilyBody::Mouse(body)
        } else if super::keyboard_products::source_product(device.product_id).is_some() {
            let factory_default_profile =
                keyboard_local_profiles::active_keyboard_factory_profile(&device);
            let body = cx.new(|cx| {
                super::keyboard_products::KeyboardProductWorkspace::new(
                    device.product_id,
                    factory_default_profile,
                    window,
                    cx,
                )
            });
            body.update(cx, |body, cx| {
                body.bind_mapping_page_scroll(body_scroll.clone());
                body.observe_source_artwork(
                    super::keyboard_products::SourceArtworkObservation::for_device(&device),
                    cx,
                );
            });
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 _: &super::keyboard_products::KeyboardProductChanged,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                    this.persist_source_profile(false, cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 event: &super::keyboard_products::KeyboardLegacyRuntimeStatus,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(event.0.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, event: &super::MappingCallbackRequested, cx| {
                    cx.emit(WorkspaceEvent::MappingCallbackRequested(event.clone()));
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |this: &mut Self,
                 _,
                 event: &super::keyboard_products::KeyboardLegacyProfileNavigation,
                 window,
                 cx| {
                    if this.device.product_id != 515 {
                        return;
                    }
                    let mut profiles = this.device.profiles.iter().collect::<Vec<_>>();
                    profiles.sort_by(|a, b| {
                        a.name
                            .to_uppercase()
                            .encode_utf16()
                            .cmp(b.name.to_uppercase().encode_utf16())
                    });
                    let Some(index) = profiles
                        .iter()
                        .position(|p| p.id == this.device.active_profile)
                    else {
                        return;
                    };
                    let output = &event.0;
                    let target = if output["type"] == "switchProfile" {
                        profiles
                            .iter()
                            .position(|p| Some(p.guid.as_str()) == output["guid"].as_str())
                    } else {
                        match output["name"].as_str() {
                            Some("NextProfile") => Some((index + 1).min(profiles.len() - 1)),
                            Some("PreviousProfile") => Some(index.saturating_sub(1)),
                            Some("CycleUp") => Some((index + 1) % profiles.len()),
                            Some("CycleDown") => {
                                Some((index + profiles.len() - 1) % profiles.len())
                            }
                            _ => None,
                        }
                    };
                    if let Some(target) = target {
                        let id = profiles[target].id.clone();
                        this.select_profile(&id, window, cx);
                    }
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 event: &super::keyboard_products::KeyboardActuationRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardActuationRequested(event.clone()));
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |this: &mut Self,
                 _,
                 navigation: &super::keyboard_products::KeyboardNavigationRequested,
                 window,
                 cx| {
                    if let Some(index) = navigation.history_index() {
                        this.restore_keyboard_mapping_history(
                            navigation.page_key(),
                            index,
                            window,
                            cx,
                        );
                    } else {
                        this.set_page_key(navigation.page_key(), window, cx);
                    }
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::keyboard_products::KeyboardIndicatorLedRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardIndicatorLedRequested(
                        request.clone(),
                    ));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 event: &super::keyboard_products::KeyboardBrightnessRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardBrightnessRequested {
                        generation: event.generation(),
                        percent: event.percent(),
                    });
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 event: &super::keyboard_products::KeyboardBrightnessReadRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardBrightnessReadRequested {
                        generation: event.generation(),
                    });
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::keyboard_products::KeyboardLightingRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardLightingRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::keyboard_products::KeyboardMappingInterDeviceConfigureRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardMappingInterDeviceConfigureRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 _: &super::keyboard_products::KeyboardMappingLoginWarningReadRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardMappingLoginWarningReadRequested);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 _: &super::keyboard_products::KeyboardMappingLoginWarningHidden,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardMappingLoginWarningHidden);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 event: &super::keyboard_products::KeyboardMappingDialReadRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardMappingDialReadRequested(
                        event.clone(),
                    ));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 event: &super::keyboard_products::KeyboardMappingBrightnessReadRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardMappingBrightnessReadRequested(
                        event.clone(),
                    ));
                },
            ));
            subscriptions.push(cx.subscribe(&body, |_: &mut Self, _, event: &super::keyboard_products::KeyboardMappingSensitivityWarningReadRequested, cx| {
                cx.emit(WorkspaceEvent::KeyboardMappingSensitivityWarningReadRequested { product_id: event.product_id });
            }));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 event: &super::keyboard_products::KeyboardMappingSensitivityWarningHidden,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardMappingSensitivityWarningHidden {
                        product_id: event.product_id,
                    });
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 _: &super::keyboard_products::KeyboardCharacterMapRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardCharacterMapRequested);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::keyboard_products::KeyboardLegacyCalibrationHostRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::KeyboardLegacyCalibrationHostRequested(
                        request.clone(),
                    ));
                },
            ));
            FamilyBody::Keyboard(body)
        } else if super::gamepad_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| {
                super::gamepad_products::GamepadProductWorkspace::new(
                    device.product_id,
                    device.layout_id,
                    window,
                    cx,
                )
            });
            body.update(cx, |body, cx| body.set_edition_id(device.edition_id, cx));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, _: &super::gamepad_products::GamepadProductChanged, cx| {
                    this.device.controller_power_saving = body.read(cx).controller_power_saving();
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |this: &mut Self,
                 _,
                 request: &super::gamepad_products::GamepadNavigationRequested,
                 window,
                 cx| {
                    this.set_page_key(request.page_key(), window, cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_, _, request: &super::gamepad_products::TriggerCalibrationIntent, cx| {
                    cx.emit(WorkspaceEvent::TriggerCalibrationRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 request: &super::gamepad_products::ControllerTriggerRequested,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                    cx.emit(WorkspaceEvent::ControllerTriggerRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 request: &super::gamepad_products::ControllerLightingRequested,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                    cx.emit(WorkspaceEvent::ControllerLightingRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 _,
                 request: &super::gamepad_products::ControllerSleepRequested,
                 cx| {
                    if request.requires_persistence()
                        && let Some(setting) = request.setting()
                    {
                        this.device.controller_power_saving = Some(setting);
                    }
                    cx.emit(WorkspaceEvent::ControllerSleepRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 request: &super::gamepad_products::ControllerChromaRequested,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                    cx.emit(WorkspaceEvent::ControllerChromaRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, _, intent: &super::gamepad_products::ControllerSocdEdited, cx| {
                    this.edit_controller_socd(intent.0, cx);
                },
            ));
            FamilyBody::Gamepad(body)
        } else if super::audio_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| {
                super::audio_products::AudioProductWorkspace::new(device.product_id, window, cx)
            });
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::audio_products::SeirenStreamMixerRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::SeirenStreamMixerRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::audio_products::SeirenStreamMixerCaptureChanged,
                 cx| {
                    cx.emit(WorkspaceEvent::SeirenStreamMixerCaptureChanged(
                        request.clone(),
                    ));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::audio_products::SeirenAudioPlaybackRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::SeirenAudioPlaybackRequested(
                        request.clone(),
                    ));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, event: &super::audio_products::SeirenMicrophoneSetupSeen, cx| {
                    cx.emit(WorkspaceEvent::SeirenMicrophoneSetupSeen(event.seen()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, request: &super::audio_products::SeirenEqRequested, cx| {
                    if request.persists_local_settings() {
                        this.capture(body.read(cx).snapshot(), cx);
                    }
                    cx.emit(WorkspaceEvent::SeirenEqRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::audio_products::SeirenAudioCaptureRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::SeirenAudioCaptureRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, request: &super::audio_products::AudioVolumeRequest, cx| {
                    cx.emit(WorkspaceEvent::AudioVolumeRequested { request: *request })
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, request: &super::audio_products::LeviathanRequest, cx| {
                    cx.emit(WorkspaceEvent::LeviathanRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, request: &super::audio_products::SurroundRequest, cx| {
                    cx.emit(WorkspaceEvent::SurroundRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, request: &super::audio_products::AudioMixerRequest, cx| {
                    cx.emit(WorkspaceEvent::AudioMixerRequested {
                        request: request.clone(),
                    })
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, request: &super::audio_products::PresetShortcutRequest, cx| {
                    cx.emit(WorkspaceEvent::AudioPresetShortcutRequested {
                        request: request.clone(),
                    });
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, event: &super::OledRuntimeRequested, cx| cx.emit(event.clone()),
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, _: &super::audio_products::AudioStudioRequested, cx| {
                    cx.emit(WorkspaceEvent::OpenStudio)
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, _: &super::audio_products::AudioProductChanged, cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |this: &mut Self,
                 _,
                 navigation: &super::audio_products::AudioNavigation,
                 window,
                 cx| {
                    match navigation {
                        super::audio_products::AudioNavigation::Page(page) => {
                            this.set_page(*page, window, cx)
                        }
                        super::audio_products::AudioNavigation::History(forward) => {
                            this.step_page_history(*forward, window, cx)
                        }
                    }
                },
            ));
            FamilyBody::Audio(body)
        } else if super::system_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| {
                super::system_products::SystemProductWorkspace::new(
                    device.product_id,
                    device.edition_id,
                    window,
                    cx,
                )
            });
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, _: &super::system_products::SystemProductChanged, cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            FamilyBody::System(body)
        } else if super::accessory_system_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| {
                let mut body =
                    super::accessory_system_products::AccessorySystemProductWorkspace::new(
                        device.product_id,
                        window,
                        cx,
                    );
                body.set_device_artwork(device.edition_id, device.layout_id, cx);
                body
            });
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 body,
                 _: &super::accessory_system_products::AccessorySystemProductChanged,
                 cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 _,
                 change: &super::accessory_system_products::HanboLocalSettingsChanged,
                 cx| {
                    this.capture_hanbo_settings(change, cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_, _, _: &super::accessory_system_products::HanboResourceChanged, cx| {
                    cx.emit(WorkspaceEvent::ResourceStateChanged);
                },
            ));
            FamilyBody::AccessorySystem(body)
        } else if super::source_controls::supports(device.product_id) {
            let body = cx.new(|cx| {
                let mut body =
                    super::source_controls::SourceControls::new(device.product_id, window, cx);
                body.observe_camera_firmware(
                    device
                        .dashboard
                        .readonly_values
                        .as_ref()
                        .and_then(|values| values.firmware.as_deref()),
                    cx,
                );
                body
            });
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self, _, request: &super::source_controls::HyperfluxRequested, cx| {
                    cx.emit(WorkspaceEvent::HyperfluxRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_: &mut Self,
                 _,
                 request: &super::source_controls::RipsawMonitoringRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::RipsawMonitoringRequested(request.clone()));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_, _, event: &super::ReceiverPairingEvent, cx| {
                    cx.emit(event.clone());
                },
            ));
            subscriptions.push(
                cx.subscribe(&body, |_, _, event: &super::ReceiverDeviceRequested, cx| {
                    cx.emit(event.clone())
                }),
            );
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self, body, _: &super::source_controls::SourceControlsChanged, cx| {
                    this.capture(body.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_, _, event: &super::HyperpollingIndicatorRequested, cx| {
                    cx.emit(WorkspaceEvent::HyperpollingIndicatorRequested(
                        event.clone(),
                    ));
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_, _, request: &super::ReceiverBrightnessRequested, cx| {
                    cx.emit(WorkspaceEvent::ReceiverBrightnessRequested(*request))
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |_, _, request: &super::ReceiverBrightnessReadRequested, cx| {
                    cx.emit(WorkspaceEvent::ReceiverBrightnessReadRequested(*request))
                },
            ));
            subscriptions.push(cx.subscribe(
                &body,
                |this: &mut Self,
                 _: Entity<super::source_controls::SourceControls>,
                 _: &super::source_controls::SourceControlsPairingRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::PairingRequested(this.device.clone()));
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &body,
                window,
                |this: &mut Self,
                 _,
                 _: &super::source_controls::SourceControlsHelpRequested,
                 window,
                 cx| {
                    this.set_page_key("HELP", window, cx);
                },
            ));
            FamilyBody::Controls(body)
        } else {
            FamilyBody::Pending
        };
        let supplement = if !matches!(body, FamilyBody::Controls(_))
            && super::source_controls::supports(device.product_id)
        {
            let controls = cx.new(|cx| {
                super::source_controls::SourceControls::new(device.product_id, window, cx)
            });
            subscriptions.push(cx.subscribe(
                &controls,
                |_, _, event: &super::ReceiverPairingEvent, cx| {
                    cx.emit(event.clone());
                },
            ));
            subscriptions.push(cx.subscribe(
                &controls,
                |this: &mut Self,
                 controls,
                 _: &super::source_controls::SourceControlsChanged,
                 cx| {
                    this.capture_supplement(controls.read(cx).snapshot(), cx);
                },
            ));
            subscriptions.push(cx.subscribe(
                &controls,
                |this: &mut Self,
                 _: Entity<super::source_controls::SourceControls>,
                 _: &super::source_controls::SourceControlsPairingRequested,
                 cx| {
                    cx.emit(WorkspaceEvent::PairingRequested(this.device.clone()));
                },
            ));
            subscriptions.push(cx.subscribe_in(
                &controls,
                window,
                |this: &mut Self,
                 _,
                 _: &super::source_controls::SourceControlsHelpRequested,
                 window,
                 cx| {
                    this.set_page_key("HELP", window, cx);
                },
            ));
            Some(controls)
        } else {
            None
        };
        let (profile_choices, profile_status) = match profile_dropdown::choices(&device, None) {
            Ok(choices) => (choices, None),
            Err(error) => (Vec::new(), Some(format!("Profile list: {error:#}"))),
        };
        let profile_decorations = profile_dropdown::decorations(&device, None, &profile_choices);
        let profile = cx.new(|cx| SelectState::new(profile_choices.clone(), None, window, cx));
        profile.update(cx, |state, cx| {
            state.set_selected_value(&device.active_profile, window, cx)
        });
        subscriptions.push(cx.subscribe_in(
            &profile,
            window,
            |this: &mut Self, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(id)) = event {
                    this.select_profile(id, window, cx);
                }
            },
        ));
        let name_limit = profile_menu::spec(device.product_id).map(|spec| spec.name_limit());
        let profile_name = cx.new(|cx| match name_limit {
            Some(limit) => razer_widgets::source_input::input_state(limit, window, cx),
            None => InputState::new(window, cx),
        });
        subscriptions.push(cx.subscribe_in(
            &profile_name,
            window,
            |this: &mut Self, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    this.finish_profile_rename(window, cx);
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.profile.update(cx, |state, cx| state.focus(window, cx));
                    }
                }
            },
        ));
        let owner = cx.entity().downgrade();
        let profile_menu =
            cx.new(|cx| ListState::new(profile_menu::ProfileCommands::new(owner), window, cx));
        let help = cx.new(|cx| super::source_help::SourceHelp::new(device.clone(), cx));
        let help_owner =
            cx.new(|cx| super::source_help_owner::SourceHelpOwner::new(device.product_id, cx));
        if let FamilyBody::Audio(audio) = &body {
            audio.update(cx, |audio, cx| {
                audio.set_source_owner(help_owner.clone(), cx)
            });
        }
        subscriptions.push(cx.subscribe_in(
            &help_owner,
            window,
            |this: &mut Self,
             _,
             request: &super::source_help_owner::HelpOwnerPageRequested,
             window,
             cx| {
                this.set_page_key(request.0, window, cx);
            },
        ));
        subscriptions.push(cx.subscribe(
            &help_owner,
            |_: &mut Self, _, request: &super::HelpOwnerRequested, cx| {
                cx.emit(WorkspaceEvent::HelpOwnerRequested(request.clone()));
            },
        ));
        subscriptions.push(cx.subscribe(
            &help,
            |this: &mut Self, _, event: &super::source_help_owner::HelpOwnerOpenRequested, cx| {
                this.help_owner
                    .update(cx, |owner, cx| owner.show(event.0, cx));
            },
        ));
        subscriptions.push(cx.subscribe(
            &help,
            |_: &mut Self, _, request: &super::HelpSystemInfoRequested, cx| {
                cx.emit(WorkspaceEvent::HelpSystemInfoRequested(request.clone()));
            },
        ));
        subscriptions.push(cx.subscribe(
            &help,
            |_: &mut Self, _, request: &super::HelpCamoRequested, cx| {
                cx.emit(WorkspaceEvent::HelpCamoRequested(request.clone()));
            },
        ));
        subscriptions.push(cx.subscribe(
            &help,
            |_, _, event: &super::source_help::HelpResetEvent, cx| {
                cx.emit(match event {
                    super::source_help::HelpResetEvent::Requested(request) => {
                        WorkspaceEvent::HelpResetRequested(*request)
                    }
                    super::source_help::HelpResetEvent::Canceled(request) => {
                        WorkspaceEvent::HelpResetCanceled(*request)
                    }
                });
            },
        ));
        subscriptions.push(cx.subscribe(
            &help,
            |this: &mut Self, _, event: &super::source_help::HelpScrollRequested, cx| {
                this.request_help_scroll(event.distance, cx);
            },
        ));
        let dock_pairing = matches!(device.product_id, 164 | 241)
            .then(|| cx.new(|_| super::dock_pairing::DockPairing::new(&device)));
        if let Some(dock) = &dock_pairing {
            subscriptions.push(cx.subscribe(
                dock,
                |_, _, event: &super::DockPairingUtilityRequested, cx| cx.emit(event.clone()),
            ));
            subscriptions.push(
                cx.subscribe(dock, |_, _, event: &super::DockPairingEvent, cx| {
                    cx.emit(event.clone())
                }),
            );
            // 源配对文案里的设备名在命中应用设备列表时可点，点击切到该设备工作区。
            subscriptions.push(cx.subscribe(
                dock,
                |_, _, event: &super::dock_pairing::DeviceLinkRequested, cx| {
                    cx.emit(WorkspaceEvent::OpenDevice {
                        product_id: event.product_id,
                        edition_id: event.edition_id,
                    });
                },
            ));
        }
        let accessory = AccessoryPage::new(&device, window, cx, &mut subscriptions);
        let dual_link_widget = (device.product_id == 756)
            .then(|| cx.new(|_| super::dock_pairing::DualLinkWidget::new(&device)));
        if let Some(widget) = &dual_link_widget {
            subscriptions.push(
                cx.subscribe(widget, |_, _, event: &super::DockPairingEvent, cx| {
                    cx.emit(event.clone())
                }),
            );
            if let FamilyBody::Keyboard(body) = &body {
                body.update(cx, |body, cx| {
                    body.attach_dual_link_widget(widget.clone(), cx)
                });
            }
        }
        let mut trigger_navigation = super::gamepad_products::TriggerNavigation::default();
        if let Some(firmware) = device
            .dashboard
            .readonly_values
            .as_ref()
            .and_then(|values| values.firmware.as_deref())
        {
            trigger_navigation.observe_firmware(device.product_id, firmware);
        }
        let mut this = Self {
            saved: device.clone(),
            mouse_profile_observed_serial: None,
            mouse_profile_connection_key: None,
            mouse_scroll_connection_key: None,
            mapping_connection_generation: 0,
            mapping_reconnect: Default::default(),
            keyboard_latest_ble: false,
            keyboard_profile_inventory: Default::default(),
            controller_initialization: Default::default(),
            controller_socd: Default::default(),
            device,
            page,
            page_history: page.into_iter().collect(),
            page_history_index: 0,
            active: false,
            hanbo_telemetry_requested: false,
            hanbo_profile_revision: 0,
            receiver_profile_selection: Default::default(),
            exiting: false,
            trigger_navigation,
            body,
            help,
            help_owner,
            body_scroll,
            keymap_bar_width: 0.,
            help_scroll: None,
            supplement,
            dock_pairing,
            dual_link_widget,
            accessory,
            profile,
            profile_choices,
            profile_decorations,
            profile_status,
            profile_name,
            profile_rename: None,
            local_mapping_macros: Vec::new(),
            profile_menu,
            profile_confirmation: None,
            profile_confirm_focus: cx.focus_handle(),
            profile_confirm_task: None,
            profile_transfer: None,
            profile_transfer_subscription: None,
            profile_transfer_request: None,
            profile_export_task: None,
            profile_import: None,
            profile_linked_games: None,
            profile_linked_games_subscription: None,
            _subscriptions: subscriptions,
        };
        this.restore_active(window, cx);
        this.select_body_page(window, cx);
        let owner = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |this, cx| this.sync_receiver_profile_initialization(cx));
        });
        this
    }
    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn finish_hanbo_command(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::AccessorySystem(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_hanbo_command(generation, result, cx)
            });
        }
    }

    pub fn observe_hanbo_telemetry(
        &mut self,
        telemetry: &razer_device::hanbo_cooler::Telemetry,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::AccessorySystem(body) = &self.body {
            body.update(cx, |body, cx| body.observe_hanbo_telemetry(telemetry, cx));
        }
    }

    pub fn hanbo_telemetry_requested(&self) -> bool {
        self.hanbo_telemetry_requested
    }

    pub fn hanbo_profile_revision(&self) -> u64 {
        self.hanbo_profile_revision
    }

    pub fn observe_hanbo_system_sample(
        &mut self,
        sample: &razer_device::system_monitor::SystemSample,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::AccessorySystem(body) = &self.body {
            body.update(cx, |body, cx| body.observe_hanbo_system_sample(sample, cx));
        }
    }

    pub fn flush_hanbo_curve(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Vec<super::accessory_system_products::HanboLocalSettingsChanged> {
        let requests = match &self.body {
            FamilyBody::AccessorySystem(body) => {
                body.update(cx, |body, cx| body.flush_hanbo_curve(cx))
            }
            _ => return Vec::new(),
        };
        requests
            .into_iter()
            .filter(|request| self.apply_hanbo_settings(request, cx))
            .collect()
    }

    fn capture_hanbo_settings(
        &mut self,
        request: &super::accessory_system_products::HanboLocalSettingsChanged,
        cx: &mut Context<Self>,
    ) {
        if self.apply_hanbo_settings(request, cx) {
            cx.emit(WorkspaceEvent::HanboLocalSettingsChanged(request.clone()));
        }
    }

    fn apply_hanbo_settings(
        &mut self,
        request: &super::accessory_system_products::HanboLocalSettingsChanged,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != razer_device::hanbo_cooler::PRODUCT_ID {
            return false;
        }
        let change = &request.change;
        let mut device_settings = self.device.source_device_settings.clone();
        let profile = self
            .device
            .profiles
            .iter_mut()
            .find(|profile| profile.id == self.device.active_profile);
        if matches!(
            change,
            razer_device::hanbo_settings::LocalChange::Thermal { .. }
        ) && profile.is_none()
        {
            eprintln!("[hanbo local settings] active profile is absent");
            return false;
        }
        let mut profile_settings = profile
            .as_ref()
            .and_then(|profile| profile.source_settings.clone());
        if let Err(error) = change.apply(&mut profile_settings, &mut device_settings) {
            eprintln!("[hanbo local settings] {error:#}");
            return false;
        }
        self.device.source_device_settings = device_settings;
        if let Some(profile) = profile {
            profile.source_settings = profile_settings;
        }
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
        true
    }

    pub fn hanbo_command_matches(&self, generation: u64, cx: &App) -> bool {
        matches!(&self.body, FamilyBody::AccessorySystem(body) if body.read(cx).hanbo_command_matches(generation))
    }
    pub fn receiver_brightness_matches(
        &self,
        generation: u64,
        percent: Option<u8>,
        current: bool,
        cx: &App,
    ) -> bool {
        let FamilyBody::Controls(body) = &self.body else {
            return false;
        };
        let body = body.read(cx);
        match (percent, current) {
            (Some(percent), true) => body.brightness_request_current(generation, percent),
            (Some(percent), false) => body.brightness_request_matches(generation, percent),
            (None, true) => body.brightness_read_scope_current(generation),
            (None, false) => body.brightness_read_matches(generation),
        }
    }
    pub fn finish_receiver_brightness(
        &mut self,
        generation: u64,
        percent: Option<u8>,
        outcome: Option<super::HelpResetOutcome>,
        submitted_requested: Option<u8>,
        observed: Option<u8>,
        error: Option<String>,
        scope_current: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.receiver_brightness_matches(generation, percent, false, cx) {
            return;
        }
        if scope_current {
            if let Some(outcome) = outcome {
                self.publish_receiver_brightness_profile(generation, percent, outcome, window, cx);
            }
        }
        if let FamilyBody::Controls(body) = &self.body {
            body.update(cx, |body, cx| match percent {
                Some(percent) => body.finish_brightness(
                    generation,
                    percent,
                    submitted_requested,
                    observed,
                    error,
                    scope_current,
                    cx,
                ),
                None => body.finish_brightness_read(generation, observed, error, scope_current, cx),
            });
        }
        cx.notify();
    }
    /// Original local/profile publication precedes device task completion.
    /// This updates current page state; it is not a host WindowStorage event.
    pub fn publish_receiver_brightness_profile(
        &mut self,
        generation: u64,
        percent: Option<u8>,
        outcome: super::HelpResetOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match razer_storage::receiver_reset::ReceiverProfileSnapshot::parse(
            self.device.product_id,
            self.device.edition_id,
            &outcome.serial_number,
            &outcome.source_document,
        ) {
            Ok(snapshot) => {
                self.publish_receiver_brightness_snapshot(
                    generation, percent, snapshot, window, cx,
                );
            }
            Err(error) => eprintln!("[receiver profile boundary] {error:#}"),
        }
        cx.notify();
    }
    pub fn help_reset_matches(&self, request: super::HelpResetRequest, cx: &App) -> bool {
        self.help.read(cx).reset_matches(request)
    }
    pub fn finish_help_reset(
        &mut self,
        request: super::HelpResetRequest,
        outcome: Option<super::HelpResetOutcome>,
        mut error: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.help_reset_matches(request, cx) {
            return;
        }
        if let Some(outcome) = outcome {
            let result = razer_storage::receiver_reset::ReceiverProfileSnapshot::parse(
                self.device.product_id,
                self.device.edition_id,
                &outcome.serial_number,
                &outcome.source_document,
            )
            .map(|snapshot| self.publish_receiver_help_profile(request, snapshot, window, cx));
            if !matches!(result, Ok(true)) {
                let detail = match result {
                    Err(error) => format!("接收器配置回执无效：{error:#}"),
                    Ok(_) => "接收器配置回执所属页面已变化。".into(),
                };
                error = Some(error.map_or(detail.clone(), |error| format!("{error}; {detail}")));
            }
        }
        if let Some(error) = &error {
            window.push_notification(error.clone(), cx);
        }
        self.help
            .update(cx, |help, cx| help.finish_reset(request, error, cx));
        cx.notify();
    }
    pub fn saved_snapshot(&self) -> Device {
        self.saved.clone()
    }
    pub fn publish_legacy_mapping(&mut self, document: Value, cx: &mut Context<Self>) {
        if !self.exiting {
            self.publish_saved_mapping(document, cx);
        }
    }

    /// A captured Save remains admitted while its file write drains at exit.
    pub fn publish_saved_mapping(&mut self, document: Value, cx: &mut Context<Self>) {
        use razer_model::model::DeviceConnectionObservation;
        if !razer_device::mapping_document::MappingProduct::for_product_id(self.device.product_id)
            .is_some_and(|product| product.uses_retained_mapping_owner())
            || !matches!(
                self.device.dashboard.connection_observation,
                Some(
                    DeviceConnectionObservation::UsbPresent
                        | DeviceConnectionObservation::HidPresent
                )
            )
        {
            return;
        }
        let container = self.device.device_container_id.clone();
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| {
                if self.device.product_id == 158 {
                    if mapping_reconnect::ordinary_mouse_owner(&self.device).is_some() {
                        body.set_receiver_mapping_connection_generation(
                            Some(container.clone()),
                            self.mapping_connection_generation,
                        );
                        body.publish_receiver_mapping(container, document, cx);
                    }
                } else {
                    body.set_legacy_mapping_connection_generation(
                        Some(container.clone()),
                        self.mapping_connection_generation,
                    );
                    body.publish_legacy_mapping(container, document, cx);
                }
            });
        } else if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.publish_legacy_mapping(container, document, cx)
            });
        }
    }
    /// Current756 middleware candidate publication, separate from a hardware
    /// read and from the older mapping runtime's connection owner.
    pub fn observe_keyboard_latest_document(&mut self, document: Value, cx: &mut Context<Self>) {
        if self.device.product_id != 756 {
            return;
        }
        let Some(profiles) = document["profiles"].as_array() else {
            return;
        };
        for profile in &mut self.device.profiles {
            if let Some(candidate) = profiles.iter().find(|value| value["guid"] == profile.guid) {
                let settings = profile
                    .source_settings
                    .get_or_insert_with(|| serde_json::json!({}));
                if let (Some(target), Some(candidate)) =
                    (settings.as_object_mut(), candidate.as_object())
                {
                    target.extend(candidate.clone());
                }
            }
        }
        let settings = self
            .device
            .source_device_settings
            .get_or_insert_with(|| serde_json::json!({}));
        for &field in razer_device::keyboard_latest_mapping::features::DEVICE_FIELDS {
            if let Some(value) = document.get(field) {
                settings[field] = value.clone();
            }
        }
        settings[razer_device::keyboard_latest_restore::STORAGE_FIELD] = document.clone();
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_latest_source_document(&document, cx)
            });
        }
        cx.notify();
    }
    pub fn set_keyboard_latest_connected(&mut self, connected: bool, cx: &mut Context<Self>) {
        if self.device.product_id == 756 {
            if let FamilyBody::Keyboard(body) = &self.body {
                body.update(cx, |body, cx| {
                    body.set_latest_mapping_connected(connected, cx)
                });
            }
        }
    }
    pub fn set_keyboard_latest_ble(&mut self, ble: bool, _cx: &mut Context<Self>) {
        self.keyboard_latest_ble = self.device.product_id == 756 && ble;
    }
    pub fn observe_keyboard_latest_mapping_event(&mut self, event: Value, cx: &mut Context<Self>) {
        if self.device.product_id != 756 {
            return;
        }
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.observe_latest_mapping_event(event, cx));
        }
    }
    pub fn dirty(&self) -> bool {
        self.device.source_device_settings != self.saved.source_device_settings
            || self.device.controller_power_saving != self.saved.controller_power_saving
            || self.device.active_profile != self.saved.active_profile
            || self.device.profiles.len() != self.saved.profiles.len()
            || self
                .device
                .profiles
                .iter()
                .zip(&self.saved.profiles)
                .any(|(a, b)| {
                    a.id != b.id
                        || a.name != b.name
                        || a.source_settings != b.source_settings
                        || a.settings != b.settings
                })
    }
    pub fn mark_saved(&mut self, snapshot: Device, cx: &mut Context<Self>) {
        let mapping_current = razer_model::mapping::matches_saved_revision(&self.device, &snapshot);
        let pending_import = self.profile_import.as_ref().is_some_and(|request| {
            snapshot.profiles == request.candidate().profiles
                && snapshot.source_device_settings == request.candidate().source_device_settings
        });
        if !pending_import {
            if let Err(error) = razer_storage::host_local_profile::accept_saved(
                &mut self.device,
                &self.saved,
                &snapshot,
            ) {
                eprintln!("[DPI local profile] {error:#}");
            }
            mouse_profile_source::accept_saved_version(&mut self.device, &self.saved, &snapshot);
            hyperpolling_runtime::accept_saved_version(&mut self.device, &self.saved, &snapshot);
        }
        if let Some(product) =
            razer_device::mapping_document::MappingProduct::for_product_id(self.device.product_id)
                .filter(|_| !pending_import && mapping_current)
        {
            let field = product.storage_field();
            if let Some(document) = snapshot
                .source_device_settings
                .as_ref()
                .and_then(|settings| settings.get(field))
            {
                self.device
                    .source_device_settings
                    .get_or_insert_with(|| serde_json::json!({}))[field] = document.clone();
            }
        }
        self.saved = snapshot;
        cx.notify();
    }
    pub fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_profile_dialog(window, cx);
        self.device = self.saved.clone();
        self.refresh_profile_choices(window, cx);
        self.restore_active(window, cx);
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    fn capture(&mut self, mut value: Value, cx: &mut Context<Self>) {
        let previous_marci = (self.device.product_id == 3942)
            .then(|| {
                self.device
                    .active_profile_obj()
                    .and_then(|profile| profile.source_settings.as_ref())
                    .and_then(|settings| settings.pointer("/profile/quickEffects"))
                    .cloned()
            })
            .flatten();
        seiren_local_settings::capture(
            self.device.product_id,
            &mut self.device.source_device_settings,
            &mut value,
        );
        if let Err(error) = audio_mixer_local_settings::capture(
            self.device.product_id,
            &mut self.device.source_device_settings,
            &mut value,
        ) {
            eprintln!("[Audio Mixer local presets] {error:#}");
            return;
        }
        self.capture_device_settings(&mut value);
        if let Some(p) = self
            .device
            .profiles
            .iter_mut()
            .find(|p| p.id == self.device.active_profile)
        {
            for key in ["_supplement", "_accessory"] {
                if let Some(nested) = p.source_settings.as_ref().and_then(|s| s.get(key)).cloned() {
                    value[key] = nested;
                }
            }
            p.source_settings = Some(value);
        }
        self.sync_mouse_profile_selection();
        if self.device.product_id == 3942 {
            let current = self
                .device
                .active_profile_obj()
                .and_then(|profile| profile.source_settings.as_ref())
                .and_then(|settings| settings.pointer("/profile/quickEffects"));
            if current != previous_marci.as_ref() {
                if let Some(current) = current {
                    match razer_device::receiver_lighting::QuickEffect::parse(current) {
                        Ok(effect) => cx.emit(WorkspaceEvent::MarciLightingRequested(effect)),
                        Err(error) => eprintln!("[Marci lighting request] {error:#}"),
                    }
                }
            }
        }
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    fn capture_supplement(&mut self, mut value: Value, cx: &mut Context<Self>) {
        self.capture_device_settings(&mut value);
        if let Some(p) = self
            .device
            .profiles
            .iter_mut()
            .find(|p| p.id == self.device.active_profile)
        {
            let settings = p
                .source_settings
                .get_or_insert_with(|| serde_json::json!({}));
            settings["_supplement"] = value;
        }
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    fn capture_accessory(&mut self, value: Value, cx: &mut Context<Self>) {
        if accessory::device_layout(self.device.product_id) {
            self.device
                .source_device_settings
                .get_or_insert_with(|| serde_json::json!({}))["_accessory"] = value;
        } else if let Some(profile) = self
            .device
            .profiles
            .iter_mut()
            .find(|p| p.id == self.device.active_profile)
        {
            let settings = profile
                .source_settings
                .get_or_insert_with(|| serde_json::json!({}));
            settings["_accessory"] = value;
        }
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    fn restore_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.restore_active_with_dpi_observation(false, window, cx);
    }

    fn restore_active_with_dpi_observation(
        &mut self,
        preserve_current_dpi_observation: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.device.product_id == razer_device::hanbo_cooler::PRODUCT_ID {
            self.hanbo_profile_revision = self.hanbo_profile_revision.wrapping_add(1);
        }
        if let Some(view) = &self.dock_pairing {
            view.update(cx, |view, cx| view.dismiss(window, cx));
        }
        if let Some(view) = &self.dual_link_widget {
            view.update(cx, |view, cx| view.dismiss(window, cx));
        }
        if let Some(view) = &self.accessory {
            view.dismiss(window, cx);
        }
        let mut value = self
            .device
            .profiles
            .iter()
            .find(|p| p.id == self.device.active_profile)
            .and_then(|p| p.source_settings.clone());
        self.seed_keyboard_profile_identity(&mut value);
        if matches!(self.device.product_id, 182 | 190) {
            let profile = value.get_or_insert_with(|| serde_json::json!({}));
            if let Some(profile) = profile.as_object_mut() {
                if let Some(active) = self
                    .device
                    .profiles
                    .iter()
                    .find(|profile| profile.id == self.device.active_profile)
                {
                    profile.insert("guid".into(), serde_json::json!(active.guid));
                    profile.insert("name".into(), serde_json::json!(active.name));
                }
            }
        }
        // Views consume the source reducer shape; persistence splits device fields
        // back out. The selected profile can never override the device mirror.
        if let Some(fields) = self
            .device
            .source_device_settings
            .as_ref()
            .and_then(Value::as_object)
        {
            let target = value.get_or_insert_with(|| serde_json::json!({}));
            let target = if self.supplement.is_some() {
                target
                    .as_object_mut()
                    .expect("source profile object")
                    .entry("_supplement")
                    .or_insert_with(|| serde_json::json!({}))
            } else {
                target
            };
            if let Some(target) = target.as_object_mut() {
                target.extend(
                    fields
                        .iter()
                        .filter(|(key, _)| {
                            !razer_device::mapping_document::MappingProduct::owns_storage_field(key)
                                && !razer_storage::host_local_profile::owns_field(key)
                        })
                        .map(|(key, value)| (key.clone(), value.clone())),
                );
            }
        }
        if let Some(view) = &self.accessory {
            let settings = if accessory::device_layout(self.device.product_id) {
                self.device.source_device_settings.as_ref()
            } else {
                value.as_ref()
            };
            view.restore(settings.and_then(|v| v.get("_accessory")), window, cx);
        }
        seiren_local_settings::project(
            self.device.product_id,
            self.device.source_device_settings.as_ref(),
            &mut value,
        );
        self.sync_controller_socd(cx);
        match &self.body {
            FamilyBody::Mouse(e) => {
                let dpi = (!preserve_current_dpi_observation).then(|| {
                    razer_storage::host_local_profile::known_stages(&self.device)
                        .map_err(|error| format!("{error:#}"))
                });
                e.update(cx, |v, cx| {
                    v.restore(value.as_ref(), window, cx);
                    if let Some(dpi) = dpi {
                        v.restore_current_dpi_local_profile(dpi, cx);
                    }
                });
            }
            FamilyBody::Keyboard(e) => {
                let factory_default_profile =
                    keyboard_local_profiles::active_keyboard_factory_profile(&self.device);
                e.update(cx, |v, cx| {
                    v.set_factory_default_profile(factory_default_profile, window, cx);
                    v.restore(value.as_ref(), window, cx);
                });
            }
            FamilyBody::Gamepad(e) => {
                e.update(cx, |v, cx| {
                    v.restore(value.as_ref(), window, cx);
                    v.restore_controller_sleep(self.device.controller_power_saving, cx);
                    v.observe_controller_socd(self.controller_socd_for_active(), cx);
                });
            }
            FamilyBody::Controls(e) => {
                e.update(cx, |v, cx| {
                    v.set_connection(self.device.use_ble, cx);
                    v.restore(value.as_ref(), window, cx);
                });
            }
            FamilyBody::Audio(e) => {
                e.update(cx, |v, cx| v.restore(value.as_ref(), window, cx));
            }
            FamilyBody::System(e) => {
                e.update(cx, |v, cx| v.restore(value.as_ref(), window, cx));
            }
            FamilyBody::AccessorySystem(e) => {
                let celsius = razer_device::hanbo_settings::temperature_unit(
                    self.device.source_device_settings.as_ref(),
                );
                e.update(cx, |v, cx| {
                    v.restore(value.as_ref(), window, cx);
                    match celsius {
                        Ok(Some(celsius)) => v.restore_hanbo_celsius(celsius, cx),
                        Ok(None) => {}
                        Err(error) => eprintln!("[hanbo temperature unit] {error:#}"),
                    }
                });
            }
            FamilyBody::Hue(e) => {
                e.update(cx, |v, cx| v.restore(value.as_ref(), window, cx));
            }
            FamilyBody::Pending => {}
        }
        if let Some(controls) = &self.supplement {
            controls.update(cx, |v, cx| {
                v.set_connection(self.device.use_ble, cx);
                v.restore(
                    value
                        .as_ref()
                        .and_then(|v| v.get("_supplement").or(Some(v))),
                    window,
                    cx,
                )
            });
        }
        self.sync_keyboard_mapping_profiles(window, cx);
        self.sync_mouse_scroll_profile(cx);
    }
    fn capture_device_settings(&mut self, value: &mut Value) {
        if self.device.product_id == razer_device::mouse_philips::PRODUCT_ID {
            if let Some(calibration) = value.get("calibration") {
                match razer_device::mouse_philips::capture_projection(
                    &mut self.device.source_device_settings,
                    calibration,
                ) {
                    Ok(()) => {
                        value
                            .as_object_mut()
                            .expect("source profile object")
                            .remove("calibration");
                    }
                    Err(error) => {
                        eprintln!("[mouse-philips] Calibration projection failed: {error:#}")
                    }
                }
            }
        }
        if razer_device::mouse_tracking::capability(self.device.product_id).is_some() {
            if let Some(smart) = value.get("smartTracking").cloned() {
                match serde_json::from_value::<razer_device::mouse_tracking::SmartTracking>(
                    smart.clone(),
                ) {
                    Ok(smart) => {
                        let mut projection = value
                            .get("calibration")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!({}));
                        if let Some(calibration) = projection.as_object_mut() {
                            calibration
                                .insert("smartTracking".into(), value["smartTracking"].clone());
                        }
                        if let Err(error) = smart.merge_local_settings_from(
                            &mut self.device.source_device_settings,
                            Some(&projection),
                        ) {
                            eprintln!("[mouse-tracking] Local setting capture failed: {error:#}");
                        } else if let Some(object) = value.as_object_mut() {
                            object.remove("smartTracking");
                            // These products keep calibration in the device
                            // record. The view's projected copy is not a profile.
                            object.remove("calibration");
                        }
                    }
                    Err(error) => {
                        eprintln!("[mouse-tracking] Invalid local setting capture: {error}")
                    }
                }
            }
        }
        super::mouse_products::filter_power_persistence(self.device.product_id, value);
        if let Some(object) = value.as_object_mut() {
            for product in razer_device::mapping_document::MappingProduct::ALL {
                object.remove(product.storage_field());
            }
        }
        if accessory::device_layout(self.device.product_id) {
            if let Some(object) = value.as_object_mut() {
                object.remove("_accessory");
            }
        }
        for field in super::source_controls::device_fields(self.device.product_id)
            .iter()
            .map(String::as_str)
            .chain((self.device.product_id == 190).then_some("sensitivityMatcher"))
            .chain(
                (self.device.product_id == 756)
                    .then_some(razer_device::keyboard_latest_mapping::features::DEVICE_FIELDS)
                    .into_iter()
                    .flatten()
                    .copied(),
            )
            .chain(
                super::mouse_products::power_device_fields(self.device.product_id)
                    .iter()
                    .copied(),
            )
            .chain(mouse_profile_source::device_fields(self.device.product_id))
            .chain(
                razer_device::hanbo_settings::device_fields(self.device.product_id)
                    .iter()
                    .copied(),
            )
        {
            if let Some(value) = value.as_object_mut().and_then(|v| v.remove(field)) {
                let settings = self
                    .device
                    .source_device_settings
                    .get_or_insert_with(|| serde_json::json!({}));
                settings
                    .as_object_mut()
                    .expect("device settings object")
                    .insert(field.into(), value);
            }
        }
    }
    fn migrate_device_settings(device: &mut Device) {
        let fields: Vec<&str> = super::source_controls::device_fields(device.product_id)
            .iter()
            .map(String::as_str)
            .chain((device.product_id == 190).then_some("sensitivityMatcher"))
            .chain(
                (device.product_id == 756)
                    .then_some(razer_device::keyboard_latest_mapping::features::DEVICE_FIELDS)
                    .into_iter()
                    .flatten()
                    .copied(),
            )
            .chain(
                super::mouse_products::power_device_fields(device.product_id)
                    .iter()
                    .copied(),
            )
            .chain(mouse_profile_source::device_fields(device.product_id))
            .chain(
                razer_device::hanbo_settings::device_fields(device.product_id)
                    .iter()
                    .copied(),
            )
            .collect();
        if let Some(settings) = device.source_device_settings.as_mut() {
            super::mouse_products::filter_power_persistence(device.product_id, settings);
        }
        // Prefer the last active profile for legacy conflicting values. Existing
        // device settings always win; inactive profiles only fill absent fields.
        let mut order: Vec<usize> = (0..device.profiles.len()).collect();
        order.sort_by_key(|&i| device.profiles[i].id != device.active_profile);
        for i in order {
            if let Some(profile) = device.profiles[i].source_settings.as_mut() {
                super::mouse_products::filter_power_persistence(device.product_id, profile);
                if let Some(supplement) = profile.get_mut("_supplement") {
                    super::mouse_products::filter_power_persistence(device.product_id, supplement);
                }
                seiren_local_settings::migrate_profile(
                    device.product_id,
                    &mut device.source_device_settings,
                    profile,
                );
                audio_mixer_local_settings::migrate_profile(
                    device.product_id,
                    &mut device.source_device_settings,
                    profile,
                );
                if accessory::device_layout(device.product_id) {
                    if let Some(layout) =
                        profile.as_object_mut().and_then(|p| p.remove("_accessory"))
                    {
                        let settings = device
                            .source_device_settings
                            .get_or_insert_with(|| serde_json::json!({}));
                        settings
                            .as_object_mut()
                            .expect("device settings object")
                            .entry("_accessory")
                            .or_insert(layout);
                    }
                }
                for &field in &fields {
                    let direct = profile.as_object_mut().and_then(|p| p.remove(field));
                    let nested = profile
                        .get_mut("_supplement")
                        .and_then(Value::as_object_mut)
                        .and_then(|p| p.remove(field));
                    if let Some(value) = direct.or(nested) {
                        let settings = device
                            .source_device_settings
                            .get_or_insert_with(|| serde_json::json!({}));
                        settings
                            .as_object_mut()
                            .expect("device settings object")
                            .entry(field)
                            .or_insert(value);
                    }
                }
            }
        }
    }
    fn current_page(&self) -> Option<&'static ProductPage> {
        product::registered(self.device.product_id)?.page(self.page?)
    }

    fn active_navigation(&self, cx: &App) -> Option<&'static product::ProductNavigation> {
        let product = product::registered(self.device.product_id)?;
        if self.device.product_id == 756
            && matches!(&self.body, FamilyBody::Keyboard(body) if body.read(cx).observed_bluetooth() == Some(true))
        {
            return product
                .navigations()
                .iter()
                .find(|navigation| navigation.key().starts_with("product-756-bluetooth-"));
        }
        product.primary_navigation()
    }

    fn page_available(&self, page: &ProductPage) -> bool {
        (page.kind().key() != "TRIGGERS" || self.trigger_navigation.available())
            && mouse_connection_navigation(
                self.device.product_id,
                page.kind().key(),
                self.device.use_ble,
            )
            && (page.role() != ProductPageRole::Help
                || !super::source_help::is_internet_only(self.device.product_id, page.offset()))
    }

    /// 把应用设备列表转给 164/241 配对页（源 `Es` 的比对列表）。
    pub fn set_known_devices(&mut self, devices: Vec<(u32, u32)>, cx: &mut Context<Self>) {
        if let Some(dock) = &self.dock_pairing {
            dock.update(cx, |dock, cx| dock.set_known_devices(devices, cx));
        }
    }
    pub fn set_device_context(&mut self, devices: &[Device], cx: &mut Context<Self>) {
        if let Some(dock) = &self.dock_pairing {
            dock.update(cx, |dock, cx| dock.set_device_context(devices, cx));
        }
        if let Some(widget) = &self.dual_link_widget {
            widget.update(cx, |widget, cx| widget.set_device_context(devices, cx));
        }
    }
    pub fn set_multi_pairing_feature(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Some(dock) = &self.dock_pairing {
            dock.update(cx, |dock, cx| dock.set_multi_pairing_feature(enabled, cx));
        }
    }
    pub fn set_pairing_cache(&mut self, records: Value, cx: &mut Context<Self>) {
        if let Some(widget) = &self.dual_link_widget {
            widget.update(cx, |widget, cx| {
                widget.set_pairing_cache(records.clone(), cx)
            });
        }
        if let Some(dock) = &self.dock_pairing {
            dock.update(cx, |dock, cx| dock.set_pairing_cache(records, cx));
        }
    }
    pub fn set_page_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(navigation) = self.active_navigation(cx) else {
            return;
        };
        let normalized = key.to_ascii_uppercase();
        let page = navigation
            .pages()
            .iter()
            .filter(|p| self.page_available(p))
            .find(|p| {
                p.id().key() == key
                    || p.kind().key() == normalized
                    || p.kind().key().strip_prefix("TAB_") == Some(normalized.as_str())
            });
        if let Some(page) = page {
            self.set_page(page.id(), window, cx);
        }
    }
    fn set_page(&mut self, page: ProductPageId, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .active_navigation(cx)
            .and_then(|navigation| navigation.page(page))
            .is_none_or(|target| !self.page_available(target))
        {
            return;
        }
        if self.page == Some(page) {
            return;
        }
        if let FamilyBody::Keyboard(body) = &self.body {
            let Some(target) =
                product::registered(self.device.product_id).and_then(|product| product.page(page))
            else {
                return;
            };
            if body.update(cx, |body, cx| {
                body.defer_calibration_v2_page(target.kind().key(), cx)
            }) {
                return;
            }
            if body.update(cx, |body, cx| {
                body.defer_mapping_page(target.kind().key(), cx)
            }) {
                return;
            }
        }
        if let FamilyBody::Mouse(body) = &self.body {
            if body.update(cx, |body, cx| {
                body.defer_mapping_page(page.key(), window, cx)
            }) {
                return;
            }
        }
        if let FamilyBody::Audio(body) = &self.body {
            if body.update(cx, |body, cx| {
                body.defer_pod_navigation(
                    super::audio_products::AudioNavigation::Page(page),
                    window,
                    cx,
                )
            }) {
                return;
            }
        }
        self.page_history.truncate(self.page_history_index + 1);
        self.page_history.push(page);
        self.page_history_index = self.page_history.len() - 1;
        self.page = Some(page);
        self.select_body_page(window, cx);
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    pub fn can_step_history(&self, forward: bool) -> bool {
        self.next_page_history_index(forward).is_some()
    }
    fn next_page_history_index(&self, forward: bool) -> Option<usize> {
        let product = product::registered(self.device.product_id)?;
        let available = |index: &usize| {
            product
                .page(self.page_history[*index])
                .is_some_and(|page| self.page_available(page))
        };
        if forward {
            (self.page_history_index + 1..self.page_history.len()).find(available)
        } else {
            (0..self.page_history_index).rev().find(available)
        }
    }
    pub fn step_page_history(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(next_index) = self.next_page_history_index(forward) else {
            return;
        };
        if let FamilyBody::Keyboard(body) = &self.body {
            let index = next_index;
            let Some(target) = product::registered(self.device.product_id)
                .and_then(|product| product.page(self.page_history[index]))
            else {
                return;
            };
            if body.update(cx, |body, cx| {
                body.defer_mapping_history_page(target.kind().key(), index, cx)
            }) {
                return;
            }
        }
        if let FamilyBody::Mouse(body) = &self.body {
            if body.update(cx, |body, cx| {
                body.defer_mapping_history(forward, window, cx)
            }) {
                return;
            }
        }
        if let FamilyBody::Audio(body) = &self.body {
            if body.update(cx, |body, cx| {
                body.defer_pod_navigation(
                    super::audio_products::AudioNavigation::History(forward),
                    window,
                    cx,
                )
            }) {
                return;
            }
        }
        self.page_history_index = next_index;
        self.page = Some(self.page_history[self.page_history_index]);
        self.select_body_page(window, cx);
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    fn restore_keyboard_mapping_history(
        &mut self,
        key: &str,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.page_history.get(index).copied() else {
            return;
        };
        let Some(page) = self
            .active_navigation(cx)
            .and_then(|navigation| navigation.page(id))
        else {
            return;
        };
        if page.kind().key() != key || !matches!(&self.body, FamilyBody::Keyboard(_)) {
            return;
        }
        self.page_history_index = index;
        self.page = Some(id);
        self.select_body_page(window, cx);
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
    /// GR's concrete .arrow.back/forward click after its history handler.
    /// Programmatic continuations call step_page_history without inventing a
    /// second browser document click.
    pub fn mapping_history_clicked(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Mouse(mouse) = &self.body {
            mouse.update(cx, |mouse, cx| {
                mouse.current_mapping_document_click(
                    &["arrow", if forward { "forward" } else { "back" }],
                    &["navigation"],
                    &[],
                    false,
                    window,
                    cx,
                )
            });
        }
    }
    fn select_body_page(&self, window: &mut Window, cx: &mut Context<Self>) {
        // Current BodyWrapper module 73358 resets only when activeView changes.
        // This method runs after accepted navigation, including history; rejected
        // dirty-draft continuations leave the existing viewport untouched.
        self.body_scroll.set_offset(Point::default());
        self.sync_camera_activity(cx);
        self.sync_keyboard_read_activity(cx);
        self.sync_audio_volume_activity(cx);
        self.sync_audio_mixer_activity(cx);
        self.sync_seiren_audio_activity(cx);
        self.sync_receiver_read_activity(cx);
        self.sync_mouse_active(window, cx);
        if let Some(view) = &self.accessory {
            view.dismiss(window, cx);
        }
        if let Some(view) = &self.dock_pairing {
            view.update(cx, |view, cx| view.dismiss(window, cx));
        }
        if let Some(view) = &self.dual_link_widget {
            view.update(cx, |view, cx| view.dismiss(window, cx));
        }
        if let FamilyBody::Hue(body) = &self.body {
            body.update(cx, |view, cx| view.dismiss_transient(cx));
        }
        let Some(page) = self.current_page() else {
            return;
        };
        if page.role() == ProductPageRole::Help {
            match &self.body {
                FamilyBody::Mouse(body) => body.update(cx, |body, cx| {
                    body.stop_dynamic_monitor(cx);
                    body.set_page("HELP", window, cx);
                }),
                FamilyBody::Keyboard(body) => body.update(cx, |body, cx| {
                    body.source_history_page_bypass(page.kind().key(), window, cx);
                    body.leave_snap_tap(window, cx);
                    body.leave_analog_observation(cx);
                    body.leave_gamepad_observation(cx);
                }),
                FamilyBody::Gamepad(body) => body.update(cx, |body, cx| {
                    body.cancel_range_edits(cx);
                    body.leave_calibration(window, cx);
                }),
                FamilyBody::Audio(body) => {
                    body.update(cx, |body, cx| body.set_page("HELP", window, cx));
                }
                FamilyBody::AccessorySystem(body) => {
                    body.update(cx, |body, cx| body.set_page("HELP", window, cx));
                }
                FamilyBody::Controls(body) if self.device.product_id == 3331 => {
                    body.update(cx, |body, cx| body.set_page("HELP", window, cx));
                }
                _ => {}
            }
            self.help.update(cx, |help, cx| {
                help.set_device(&self.device, cx);
                help.set_page(page.offset(), cx);
            });
            return;
        }
        self.help.update(cx, |help, cx| help.leave_page(cx));
        if let Some(controls) = &self.supplement {
            controls.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx));
        }
        match &self.body {
            FamilyBody::Mouse(e) => e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::Keyboard(e) => {
                // Explicit page clicks were gated before changing outer state.
                // Source history already ran its separate mapping guard and
                // retains the original keymap visibility flag. Calibration's
                // own source history bypass remains independent.
                e.update(cx, |v, cx| {
                    v.source_history_page_bypass(page.kind().key(), window, cx)
                })
            }
            FamilyBody::Gamepad(e) => {
                e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx))
            }
            FamilyBody::Controls(e) => {
                e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx))
            }
            FamilyBody::Audio(e) => e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::System(e) => {
                e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx))
            }
            FamilyBody::AccessorySystem(e) => {
                e.update(cx, |v, cx| v.set_page(page.kind().key(), window, cx))
            }
            FamilyBody::Hue(_) | FamilyBody::Pending => {}
        }
    }
    fn use_supplement_for(&self, key: &str) -> bool {
        if !super::source_controls::supports_page(self.device.product_id, key) {
            return false;
        }
        match &self.body {
            FamilyBody::Keyboard(_) => key == "OLED",
            FamilyBody::Audio(_) => {
                !super::audio_products::supports_page(self.device.product_id, key)
            }
            FamilyBody::AccessorySystem(_) => {
                !super::accessory_system_products::supports_page(self.device.product_id, key)
            }
            FamilyBody::Pending => true,
            _ => false,
        }
    }
}

fn active_profile_settings_dirty(device: &Device, saved: &Device) -> bool {
    let Some(current) = device
        .profiles
        .iter()
        .find(|profile| profile.id == device.active_profile)
    else {
        return false;
    };
    match saved
        .profiles
        .iter()
        .find(|profile| profile.id == device.active_profile)
    {
        Some(saved) => {
            current.settings != saved.settings || current.source_settings != saved.source_settings
        }
        None => current.settings.is_some() || current.source_settings.is_some(),
    }
}
impl Render for SourceProductWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let navigation = self.active_navigation(cx);
        let pages: Vec<_> = navigation
            .into_iter()
            .flat_map(|n| n.pages())
            .filter(|page| page.role() != ProductPageRole::Help && self.page_available(page))
            .collect();
        let has_help = navigation.into_iter().flat_map(|n| n.pages()).any(|page| {
            page.role() == ProductPageRole::Help
                && !super::source_help::is_internet_only(self.device.product_id, page.offset())
        });
        let has_dynamic_tutorial = self.active
            && matches!(&self.body, FamilyBody::Mouse(mouse) if mouse.read(cx).has_dynamic_tutorial());
        let keymap_bar = match &self.body {
            FamilyBody::Keyboard(body) => body.update(cx, |body, cx| body.keymap_bar_element(cx)),
            _ => None,
        };
        let keymap_bar_visible = keymap_bar.is_some();
        let available = f32::from(window.viewport_size().width) * 16.
            / f32::from(window.rem_size())
            - self.profile_bar_width()
            - if keymap_bar_visible {
                self.keymap_bar_width + 20.
            } else {
                0.
            }
            - surface::device_right_width(&self.device, has_help, window)
            - surface::NAV_MORE_WIDTH
            - surface::NAV_MORE_MARGIN
            - if has_dynamic_tutorial { 34. } else { 0. }
            - 10.;
        let (visible, hidden) = surface::split_navs(&pages, |page| page.label(), available, window);
        let overflow = if hidden.is_empty() {
            None
        } else {
            let owner = cx.entity().downgrade();
            let items = hidden
                .iter()
                .map(|page| (page.label(), self.page == Some(page.id())))
                .collect();
            Some(surface::nav_overflow(
                "source-nav-overflow",
                items,
                move |index, window, cx| {
                    let _ =
                        owner.update(cx, |this, cx| this.set_page(hidden[index].id(), window, cx));
                },
                window,
                cx,
            ))
        };
        let is_help = self
            .current_page()
            .is_some_and(|page| page.role() == ProductPageRole::Help);
        self.advance_help_scroll(is_help, window, cx);
        let (dynamic_tutorial_icon, dynamic_tutorial_popup) = match &self.body {
            FamilyBody::Mouse(mouse) if self.active => {
                let icon = mouse
                    .read(cx)
                    .has_dynamic_tutorial()
                    .then(|| mouse.update(cx, |mouse, cx| mouse.dynamic_tutorial_icon(cx)));
                let popup = mouse.read(cx).dynamic_tutorial_visible().then(|| {
                    mouse.update(cx, |mouse, cx| mouse.dynamic_tutorial_popup(window, cx))
                });
                (icon, popup)
            }
            _ => (None, None),
        };
        let seiren_tutorial = match &self.body {
            FamilyBody::Audio(audio) if self.active => {
                audio.update(cx, |audio, cx| audio.seiren_eq_tutorial(window, cx))
            }
            _ => None,
        };
        let audio_floating = match &self.body {
            FamilyBody::Audio(audio) if self.active => audio.read(cx).floating_demo(cx),
            _ => None,
        };
        let surround_tutorial = match &self.body {
            FamilyBody::Audio(audio) if self.active => {
                audio.update(cx, |audio, cx| audio.surround_tutorial(window, cx))
            }
            _ => None,
        };
        let mut body_layout = super::product_surface::BodyLayout::Document;
        let body = if is_help {
            self.help.clone().into_any_element()
        } else if let Some(view) = self.dock_pairing.as_ref().filter(|_| {
            self.current_page().is_some_and(|page| {
                super::dock_pairing::supports_page(self.device.product_id, page.kind().key())
            })
        }) {
            view.clone().into_any_element()
        } else if let Some(view) = self.accessory.as_ref().filter(|view| {
            self.current_page()
                .is_some_and(|page| view.supports_page(self.device.product_id, page.kind().key()))
        }) {
            view.element(self.current_page().map_or("", |page| page.kind().key()))
        } else if let Some(controls) = self.supplement.as_ref().filter(|_| {
            self.current_page()
                .is_some_and(|page| self.use_supplement_for(page.kind().key()))
        }) {
            controls.clone().into_any_element()
        } else {
            match &self.body {
                FamilyBody::Mouse(e) => {
                    body_layout = e.read(cx).body_layout();
                    e.clone().into_any_element()
                }
                FamilyBody::Keyboard(e) => e.clone().into_any_element(),
                FamilyBody::Gamepad(e) => e.clone().into_any_element(),
                FamilyBody::Controls(e) => e.clone().into_any_element(),
                FamilyBody::Audio(e) => e.clone().into_any_element(),
                FamilyBody::System(e) => e.clone().into_any_element(),
                FamilyBody::AccessorySystem(e) => e.clone().into_any_element(),
                FamilyBody::Hue(e) => e.clone().into_any_element(),
                FamilyBody::Pending => {
                    surface::note("此页面的原生控件仍在接入。", cx).into_any_element()
                }
            }
        };
        let mapping_body_bounds = match &self.body {
            FamilyBody::Mouse(mouse) => Some(mouse.read(cx).mapping_190_body_bounds_handle()),
            _ => None,
        };
        let keyboard_body_bounds = match &self.body {
            FamilyBody::Keyboard(keyboard) => Some(keyboard.read(cx).mapping_viewport_bounds()),
            _ => None,
        };
        let audio_preview = match &self.body {
            FamilyBody::Audio(body) => body.update(cx, |body, cx| body.mixer_preview_indicator(cx)),
            _ => None,
        };
        let content = div()
            .id("source-product-content")
            .on_prepaint(move |bounds, _, _| {
                if let Some(handle) = &mapping_body_bounds {
                    // Source openFunctionsView compares against BodyWrapper's
                    // viewport, whose content has its own retained scroll owner.
                    handle.set(bounds);
                }
                if let Some(handle) = &keyboard_body_bounds {
                    handle.set(bounds);
                }
            })
            .flex_1()
            .min_h_0()
            .min_w_0()
            .w_full()
            .bg(razer_widgets::theme::ProductSurfaceColors::background())
            .child(body);
        let content = match body_layout {
            super::product_surface::BodyLayout::Document => content
                .scrollable_both()
                .track_scroll(&self.body_scroll)
                .into_any_element(),
            super::product_surface::BodyLayout::SplitViewport => {
                content.flex().flex_col().into_any_element()
            }
        };
        let keymap_owner = cx.entity().downgrade();
        v_flex()
            .id("source-product-workspace")
            .test_support()
            .relative()
            .size_full()
            .min_h_0()
            .min_w_0()
            .child(
                h_flex()
                    .h(surface::css(48.))
                    .when(self.profile_linked_games.is_some(), |bar| bar.opacity(0.5))
                    .flex_shrink_0()
                    .border_b_2()
                    .border_color(cx.theme().title_bar)
                    .child(
                        surface::nav_left()
                            .items_center()
                            .child(self.profile_bar(window, cx))
                            .children(audio_preview)
                            .children(keymap_bar.map(|bar| {
                                div()
                                    .id("source-keymap-bar-slot")
                                    .flex_shrink_0()
                                    .ml(surface::css(20.))
                                    .child(bar)
                                    .on_prepaint(move |bounds, window, cx| {
                                        let width = f32::from(bounds.size.width) * 16.
                                            / f32::from(window.rem_size());
                                        let owner = keymap_owner.clone();
                                        window.defer(cx, move |_, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                if (this.keymap_bar_width - width).abs() > 0.01 {
                                                    this.keymap_bar_width = width;
                                                    cx.notify();
                                                }
                                            });
                                        });
                                    })
                            })),
                    )
                    .child(
                        gpui_kit::base::Tabs::new("source-product-navigation")
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_grow(1.)
                            .flex_shrink_0()
                            .gap(surface::css(20.))
                            .children(visible.into_iter().map(|page| {
                                let id = page.id();
                                surface::navigation_button(
                                    SharedString::from(format!("product-page-{}", id.key())),
                                    page.label(),
                                    self.page == Some(id),
                                    cx,
                                )
                                .role(Role::Tab)
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.set_page(id, window, cx);
                                        if let FamilyBody::Mouse(mouse) = &this.body {
                                            // Current d_ renders .nav directly
                                            // inside an's .navs-wrapper. Run
                                            // windowClick after the local
                                            // navigation handler; an existing
                                            // dirty continuation owns the click.
                                            mouse.update(cx, |mouse, cx| {
                                                mouse.current_mapping_document_click(
                                                    &["nav"],
                                                    &["navs-wrapper"],
                                                    &[],
                                                    false,
                                                    window,
                                                    cx,
                                                )
                                            });
                                        }
                                    },
                                ))
                            }))
                            .children(overflow),
                    )
                    .child(
                        surface::nav_right()
                            .id("source-navigation-right")
                            .min_w_0()
                            .children(dynamic_tutorial_icon)
                            .children(razer_widgets::battery::element(&self.device, cx))
                            .children(
                                navigation
                                    .into_iter()
                                    .flat_map(|n| n.pages())
                                    .filter(|p| {
                                        p.role() == ProductPageRole::Help
                                            && !super::source_help::is_internet_only(
                                                self.device.product_id,
                                                p.offset(),
                                            )
                                    })
                                    .map(|page| {
                                        let id = page.id();
                                        div()
                                            .id(SharedString::from(format!(
                                                "source-help-{}",
                                                id.key()
                                            )))
                                            .child(
                                                surface::asset_button(
                                                    "source-product-help",
                                                    if self.page == Some(id) {
                                                        "synapse/help-active.svg"
                                                    } else {
                                                        "synapse/help-default.svg"
                                                    },
                                                    page.label(),
                                                    cx,
                                                )
                                                .size(surface::css(24.))
                                                .mr(surface::css(10.))
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.set_page(id, window, cx)
                                                    },
                                                )),
                                            )
                                    }),
                            ),
                    ),
            )
            .child(content)
            .children(self.profile_transfer.clone())
            .children(self.profile_linked_games.clone())
            .children(dynamic_tutorial_popup)
            .children(seiren_tutorial)
            .child(self.help_owner.clone())
            .children(audio_floating)
            .children(surround_tutorial)
    }
}

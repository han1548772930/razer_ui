//! Chroma reducer observations and the original host/middleware boundaries.
//! A service cache does not complete `checkIsFullyInstall`; None stays invisible.
use super::*;
use crate::features::controls::Choice;
use std::cell::{Cell, RefCell};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct State {
    generation: u64,
    next_request: Cell<u64>,
    last_mutation: Cell<u64>,
    pending: RefCell<BTreeMap<u64, KeyboardLightingCommand>>,
    last_error: Option<String>,
    sync_started: Cell<Option<std::time::Instant>>,
    sync_animation: Cell<u64>,
    custom_colors_draft: Option<Vec<Value>>,
    pub(super) advanced_draft: Option<bool>,
    pub(super) chroma_profile_draft: Option<String>,
    guard: Arc<AtomicBool>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            generation: 0,
            next_request: Cell::new(0),
            last_mutation: Cell::new(0),
            pending: RefCell::new(BTreeMap::new()),
            last_error: None,
            sync_started: Cell::new(None),
            sync_animation: Cell::new(0),
            custom_colors_draft: None,
            advanced_draft: None,
            chroma_profile_draft: None,
            guard: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.guard.store(false, Ordering::Release);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyboardLightingOwner {
    pub product_id: u32,
    pub generation: u64,
    pub profile_guid: Option<String>,
}

#[derive(Clone, Debug)]
pub struct KeyboardLightingRequested {
    pub request_id: u64,
    pub owner: KeyboardLightingOwner,
    pub command: KeyboardLightingCommand,
    guard: Arc<AtomicBool>,
}

impl KeyboardLightingRequested {
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.guard.clone()
    }
}

/// One complete host read. Missing fields are unobserved, independently of
/// device/reducer notifications and user drafts.
#[derive(Clone, Default)]
pub struct KeyboardLightingHostObservation {
    fully_installed: Option<bool>,
    module_installed: Option<bool>,
    configuration: Option<HostConfiguration>,
    dynamic_lighting: Option<bool>,
    connected_products: Option<Vec<u32>>,
}
#[derive(Clone)]
struct HostConfiguration {
    profiles: Vec<Value>,
    selected_profile: Option<String>,
    enabled: bool,
}
impl KeyboardLightingHostObservation {
    pub fn with_fully_installed(mut self, installed: bool) -> Self {
        self.fully_installed = Some(installed);
        self
    }
    pub fn with_module_installed(mut self, installed: bool) -> Self {
        self.module_installed = Some(installed);
        self
    }
    pub fn with_configuration(
        mut self,
        profiles: Vec<Value>,
        selected: Option<String>,
        enabled: bool,
    ) -> Self {
        self.configuration = Some(HostConfiguration {
            profiles,
            selected_profile: selected,
            enabled,
        });
        self
    }
    pub fn with_dynamic_lighting(mut self, enabled: bool) -> Self {
        self.dynamic_lighting = Some(enabled);
        self
    }
    pub fn with_connected_products(mut self, products: Vec<u32>) -> Self {
        self.connected_products = Some(products);
        self
    }
}

#[derive(Clone, Default)]
pub struct KeyboardChromaPeer {
    pub product_id: u32,
    pub support_effects: Vec<u64>,
}

#[derive(Clone, Default)]
pub struct KeyboardLightingObservation {
    pub check_is_fully_installed: Option<bool>,
    pub chroma_installed_module: Option<bool>,
    pub chroma_profiles: Vec<Value>,
    pub selected_chroma_profile_guid: Option<String>,
    pub is_chroma_enabled: Option<bool>,
    pub is_chroma_visualizer_enabled: bool,
    /// Original chromaConnectApp appName/appIcon observation.
    pub chroma_connect_app: Option<Value>,
    pub use_power_saving_quick_effect: bool,
    pub system_power_status: Option<String>,
    pub is_dynamic_lighting_enabled: Option<bool>,
    /// PID 728 deviceReducer initializes this to true; later transport
    /// observations may select the Mac branch of the factory warning.
    pub is_windows_mode: Option<bool>,
    pub nano_leaf_enabled: Option<bool>,
    /// `lighting-engine-chroma-devices` memory-storage response, not a catalog.
    pub connected_chroma_devices: Option<Vec<KeyboardChromaPeer>>,
    /// Source validDevices observation used by Radiate's capability filter.
    pub valid_devices: Vec<KeyboardChromaPeer>,
    pub selected_effect_id: Option<u64>,
    pub effect_settings: Option<Vec<Value>>,
    /// Original switchOffLighting reducer response, kept independent of effects.
    pub switch_off_lighting: Option<Value>,
    pub custom_colors: Vec<Value>,
    /// Source installer status callback. Completion is represented by a new
    /// check_is_fully_installed observation; it is never inferred from a click.
    pub installer_busy: bool,
}

impl KeyboardLightingObservation {
    fn replace_chroma_configuration(
        &mut self,
        configuration: &razer_storage::chroma_profiles::Configuration,
    ) {
        self.chroma_profiles = configuration
            .profiles()
            .iter()
            .map(|profile| json!({"id": profile.id(), "name": profile.name()}))
            .collect();
        self.selected_chroma_profile_guid = configuration.selected_profile().map(str::to_owned);
        self.is_chroma_enabled = Some(configuration.enabled());
    }
    fn replace_host(&mut self, host: KeyboardLightingHostObservation) {
        self.check_is_fully_installed = host.fully_installed;
        self.chroma_installed_module = host.module_installed;
        self.is_dynamic_lighting_enabled = host.dynamic_lighting;
        self.chroma_profiles.clear();
        self.selected_chroma_profile_guid = None;
        self.is_chroma_enabled = None;
        if let Some(configuration) = host.configuration {
            self.chroma_profiles = configuration.profiles;
            self.selected_chroma_profile_guid = configuration.selected_profile;
            self.is_chroma_enabled = Some(configuration.enabled);
        }
        self.replace_connected_products(host.connected_products);
    }

    fn replace_connected_products(&mut self, products: Option<Vec<u32>>) {
        self.connected_chroma_devices = products.map(|products| {
            products
                .into_iter()
                .map(|product_id| KeyboardChromaPeer {
                    product_id,
                    support_effects: Vec::new(),
                })
                .collect()
        });
    }
}

#[cfg(test)]
mod host_observation_tests {
    use super::{KeyboardLightingHostObservation, KeyboardLightingObservation};
    use serde_json::json;

    #[test]
    fn keyboard_shared_selection_preserves_installation_and_physical_device_observations() {
        let configuration = serde_json::from_value(json!({
            "profiles":[{"id":"local-b","name":"Local B"}],
            "currentProfile":"local-b",
            "enabled":true,
        }))
        .unwrap();
        let mut observation = KeyboardLightingObservation {
            selected_effect_id: Some(19),
            check_is_fully_installed: Some(false),
            chroma_installed_module: Some(true),
            is_dynamic_lighting_enabled: Some(false),
            custom_colors: vec![json!({"picked":true,"color":"#123456"})],
            ..Default::default()
        };
        observation.replace_connected_products(Some(vec![756, 226]));
        observation.replace_chroma_configuration(&configuration);
        assert_eq!(
            observation.selected_chroma_profile_guid.as_deref(),
            Some("local-b")
        );
        assert_eq!(observation.is_chroma_enabled, Some(true));
        assert_eq!(observation.selected_effect_id, Some(19));
        assert_eq!(observation.check_is_fully_installed, Some(false));
        assert_eq!(observation.chroma_installed_module, Some(true));
        assert_eq!(observation.is_dynamic_lighting_enabled, Some(false));
        assert_eq!(
            observation.connected_chroma_devices.as_ref().unwrap()[1].product_id,
            226
        );
        assert_eq!(
            observation.custom_colors,
            [json!({"picked":true,"color":"#123456"})]
        );
    }

    #[test]
    fn keyboard_host_read_clears_only_host_fields_without_replaying_device_values() {
        let mut observation = KeyboardLightingObservation {
            selected_effect_id: Some(19),
            switch_off_lighting: Some(json!({"idleMinutes":7})),
            custom_colors: vec![json!({"color":"#123456","picked":true})],
            nano_leaf_enabled: Some(false),
            ..Default::default()
        };
        observation.replace_host(
            KeyboardLightingHostObservation::default()
                .with_fully_installed(true)
                .with_module_installed(true)
                .with_configuration(
                    vec![json!({"id":"actual","name":"Actual"})],
                    Some("actual".into()),
                    true,
                )
                .with_dynamic_lighting(false)
                .with_connected_products(vec![756, 226]),
        );
        observation
            .replace_host(KeyboardLightingHostObservation::default().with_dynamic_lighting(true));
        assert_eq!(observation.check_is_fully_installed, None);
        assert_eq!(observation.chroma_installed_module, None);
        assert!(observation.chroma_profiles.is_empty());
        assert_eq!(observation.selected_chroma_profile_guid, None);
        assert_eq!(observation.is_chroma_enabled, None);
        assert_eq!(observation.is_dynamic_lighting_enabled, Some(true));
        assert!(observation.connected_chroma_devices.is_none());
        assert_eq!(observation.selected_effect_id, Some(19));
        assert_eq!(
            observation.switch_off_lighting,
            Some(json!({"idleMinutes":7}))
        );
        assert_eq!(
            observation.custom_colors,
            vec![json!({"color":"#123456","picked":true})]
        );
        assert_eq!(observation.nano_leaf_enabled, Some(false));
    }

    #[test]
    fn keyboard_memory_response_does_not_invent_or_clear_other_host_values() {
        let mut observation = KeyboardLightingObservation::default();
        observation.replace_host(
            KeyboardLightingHostObservation::default()
                .with_fully_installed(false)
                .with_configuration(Vec::new(), None, false),
        );
        observation.replace_connected_products(Some(vec![756, 226]));
        assert_eq!(observation.check_is_fully_installed, Some(false));
        assert_eq!(observation.is_chroma_enabled, Some(false));
        assert_eq!(observation.chroma_installed_module, None);
        assert_eq!(
            observation.connected_chroma_devices.as_ref().unwrap()[1].product_id,
            226
        );
        observation.replace_connected_products(None);
        assert!(observation.connected_chroma_devices.is_none());
        assert_eq!(observation.is_chroma_enabled, Some(false));
    }
}

#[derive(Clone, Debug)]
pub enum KeyboardLightingCommand {
    /// `checkIsFullyInstall`, installedModules, profile/effect cache and the
    /// memory-storage Chroma devices observations must complete independently.
    Observe,
    /// Original `window.mwBroadcastChannel.postMessage` envelope.
    Middleware(Value),
    /// Current installer instance `triggerInstall(chromaAppFeatures)`.
    InstallChroma,
    /// Original Qt/Ft helper: activate/focus Studio when present, otherwise
    /// open the Chroma host dashboard with the launch-Studio subtab/action.
    LaunchStudio,
    /// MapLighting's Configure Studio helper, including the installer-focus
    /// route when the Chroma app host window does not exist.
    OpenMappingChromaStudio,
    /// MapMacro's existing-window activation or /synapse/macro/ launch.
    OpenMappingMacro,
    /// Original same-window settings redirect from the WDL warning.
    OpenSettings {
        chroma: bool,
    },
    ManageChromaApps {
        install_required: bool,
    },
}

impl EventEmitter<KeyboardLightingRequested> for KeyboardProductWorkspace {}

impl KeyboardProductWorkspace {
    /// This shared local configuration does not acknowledge a device effect.
    pub fn observe_chroma_configuration(
        &mut self,
        configuration: &razer_storage::chroma_profiles::Configuration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut observation = self.lighting_observation.clone().unwrap_or_default();
        observation.replace_chroma_configuration(configuration);
        self.lighting_observation = Some(observation);
        if self.lighting_runtime.advanced_draft == Some(configuration.enabled()) {
            self.lighting_runtime.advanced_draft = None;
        }
        if self.lighting_runtime.chroma_profile_draft.as_deref() == configuration.selected_profile()
        {
            self.lighting_runtime.chroma_profile_draft = None;
        }
        self.reconcile_mapping_chroma_targets();
        self.init_mapping_targets(0, window, cx);
        self.init_mapping_targets(1, window, cx);
        self.sync_rgb_controls(window, cx);
        cx.notify();
    }
    pub fn accepts_lighting_request(&self, request: &KeyboardLightingRequested) -> bool {
        request.guard.load(Ordering::Acquire)
            && Arc::ptr_eq(&request.guard, &self.lighting_runtime.guard)
            && request.owner == self.lighting_owner()
            && self
                .lighting_runtime
                .pending
                .borrow()
                .contains_key(&request.request_id)
    }

    /// Cleanup can outlive a window, but cannot remove a replacement request.
    pub fn abandon_lighting_request(
        &mut self,
        request: &KeyboardLightingRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !Arc::ptr_eq(&request.guard, &self.lighting_runtime.guard)
            || request.owner != self.lighting_owner()
        {
            return false;
        }
        let Some(command) = self
            .lighting_runtime
            .pending
            .borrow_mut()
            .remove(&request.request_id)
        else {
            return false;
        };
        if matches!(command, KeyboardLightingCommand::InstallChroma)
            && let Some(observation) = &mut self.lighting_observation
        {
            observation.installer_busy = false;
        }
        if request.request_id >= self.lighting_runtime.last_mutation.get()
            && let Some(error) = error
        {
            self.lighting_runtime.last_error = Some(error);
        }
        cx.notify();
        true
    }

    pub fn lighting_owner(&self) -> KeyboardLightingOwner {
        KeyboardLightingOwner {
            product_id: self.spec.product_id,
            generation: self.lighting_runtime.generation,
            profile_guid: self.draft["guid"].as_str().map(str::to_owned),
        }
    }

    pub(super) fn cancel_lighting_requests(&mut self) {
        self.lighting_runtime.guard.store(false, Ordering::Release);
        self.lighting_runtime.guard = Arc::new(AtomicBool::new(true));
        self.lighting_runtime.generation = self.lighting_runtime.generation.wrapping_add(1);
        self.lighting_runtime.pending.borrow_mut().clear();
        self.lighting_runtime.last_error = None;
        self.lighting_runtime.sync_started.set(None);
        self.lighting_runtime.custom_colors_draft = None;
        self.lighting_runtime.advanced_draft = None;
        self.lighting_runtime.chroma_profile_draft = None;
        if let Some(observation) = &mut self.lighting_observation {
            observation.installer_busy = false;
            observation.replace_host(KeyboardLightingHostObservation::default());
        }
    }

    pub fn lighting_error(&self) -> Option<&str> {
        self.lighting_runtime.last_error.as_deref()
    }

    pub(super) fn request_lighting_command(
        &self,
        command: KeyboardLightingCommand,
        cx: &mut Context<Self>,
    ) {
        let request_id = self.lighting_runtime.next_request.get().wrapping_add(1);
        self.lighting_runtime.next_request.set(request_id);
        if matches!(command, KeyboardLightingCommand::Middleware(_)) {
            self.lighting_runtime.last_mutation.set(request_id);
        }
        self.lighting_runtime
            .pending
            .borrow_mut()
            .insert(request_id, command.clone());
        cx.emit(KeyboardLightingRequested {
            request_id,
            owner: self.lighting_owner(),
            command,
            guard: self.lighting_runtime.guard.clone(),
        });
    }

    pub fn complete_host_lighting_request(
        &mut self,
        request: &KeyboardLightingRequested,
        host: KeyboardLightingHostObservation,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_lighting_request(request) {
            return false;
        }
        if request.request_id >= self.lighting_runtime.last_mutation.get() {
            let mut observation = self.lighting_observation.clone().unwrap_or_default();
            observation.replace_host(host);
            self.lighting_observation = Some(observation);
            self.reconcile_mapping_chroma_targets();
            self.init_mapping_targets(0, window, cx);
            self.init_mapping_targets(1, window, cx);
            self.sync_rgb_controls(window, cx);
        }
        self.complete_lighting_request(request, result.map(|()| None), window, cx)
    }

    /// Called for both delivered responses and dispatch rejection. A local
    /// dispatch failure cannot leave the original install button pending.
    pub fn complete_lighting_request(
        &mut self,
        request: &KeyboardLightingRequested,
        result: Result<Option<KeyboardLightingObservation>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_lighting_request(request) {
            return false;
        }
        let Some(command) = self
            .lighting_runtime
            .pending
            .borrow_mut()
            .remove(&request.request_id)
        else {
            return false;
        };
        match result {
            Ok(observation) => {
                self.lighting_runtime.last_error = None;
                if request.request_id >= self.lighting_runtime.last_mutation.get() {
                    if let Some(observation) = observation {
                        self.observe_lighting(&request.owner, observation, window, cx);
                    }
                }
            }
            Err(error) => {
                if matches!(command, KeyboardLightingCommand::InstallChroma) {
                    if let Some(observation) = &mut self.lighting_observation {
                        observation.installer_busy = false;
                    }
                }
                if let KeyboardLightingCommand::Middleware(message) = command {
                    if message["type"] == "TOGGLE_CHROMA"
                        && request.request_id >= self.lighting_runtime.last_mutation.get()
                    {
                        self.lighting_runtime.advanced_draft = None;
                    }
                }
                self.lighting_runtime.last_error = Some(error);
            }
        }
        cx.notify();
        true
    }

    pub fn refresh_lighting(&self, cx: &mut Context<Self>) {
        self.request_lighting_command(KeyboardLightingCommand::Observe, cx);
    }

    pub fn observe_lighting(
        &mut self,
        owner: &KeyboardLightingOwner,
        observation: KeyboardLightingObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if owner != &self.lighting_owner() {
            return false;
        }
        if observation.is_chroma_enabled.is_some()
            && observation.is_chroma_enabled == self.lighting_runtime.advanced_draft
        {
            self.lighting_runtime.advanced_draft = None;
        }
        if observation.selected_chroma_profile_guid.is_some()
            && observation.selected_chroma_profile_guid
                == self.lighting_runtime.chroma_profile_draft
        {
            self.lighting_runtime.chroma_profile_draft = None;
        }
        if let Some(id) = observation.selected_effect_id {
            self.draft["quickEffects"]["selectedEffectId"] = json!(id);
        }
        if let Some(settings) = &observation.effect_settings {
            self.draft["quickEffects"]["effectSettings"] = json!(settings);
            if let Some(setting) = settings
                .iter()
                .find(|s| s["effectId"] == self.rgb_effect_id())
            {
                self.draft["quickEffects"]["selectedEffectSetting"] = setting.clone();
            }
        }
        if let Some(setting) = &observation.switch_off_lighting {
            self.draft["switchOffLighting"] = setting.clone();
            if let (Some(slider), Some(minutes)) = (
                self.sliders.get("/switchOffLighting/idleMinutes"),
                setting["idleMinutes"].as_f64(),
            ) {
                self.syncing = true;
                slider.update(cx, |slider, cx| {
                    slider.set_value(minutes as f32, window, cx)
                });
                self.syncing = false;
            }
        }
        // Only a complete source 16-slot palette is a palette observation.
        // An unrelated partial Lighting observation must not erase local colors.
        if observation.custom_colors.len() == 16 {
            let mut slots = [None; 16];
            let mut valid = true;
            for (slot, entry) in slots.iter_mut().zip(&observation.custom_colors) {
                valid &= entry["picked"].is_boolean();
                if entry["picked"] == true {
                    let color = entry["color"]
                        .as_str()
                        .and_then(|color| color.strip_prefix('#'));
                    let parsed = color.and_then(|color| match color.len() {
                        3 => u16::from_str_radix(color, 16).ok().map(|value| {
                            [
                                ((value >> 8) as u8 & 15) * 17,
                                ((value >> 4) as u8 & 15) * 17,
                                (value as u8 & 15) * 17,
                            ]
                        }),
                        6 => u32::from_str_radix(color, 16)
                            .ok()
                            .map(|value| [(value >> 16) as u8, (value >> 8) as u8, value as u8]),
                        _ => None,
                    });
                    valid &= parsed.is_some();
                    *slot = parsed;
                }
            }
            if valid {
                razer_state::CustomColors::replace(slots, cx);
                self.lighting_runtime.custom_colors_draft = None;
            }
        }
        self.lighting_observation = Some(observation);
        self.reconcile_mapping_chroma_targets();
        self.init_mapping_targets(0, window, cx);
        self.init_mapping_targets(1, window, cx);
        self.sync_rgb_controls(window, cx);
        cx.notify();
        true
    }

    pub(super) fn chroma_profile_choices(&self) -> Vec<Choice> {
        self.lighting_observation
            .as_ref()
            .into_iter()
            .flat_map(|observation| &observation.chroma_profiles)
            .filter_map(|profile| {
                Some(Choice::new(
                    profile["id"].as_str()?,
                    profile["name"].as_str()?,
                ))
            })
            .collect()
    }

    pub(super) fn chroma_mode(&self) -> Option<bool> {
        self.lighting_runtime.advanced_draft.or_else(|| {
            self.lighting_observation
                .as_ref()
                .and_then(|observation| observation.is_chroma_enabled)
        })
    }

    pub(super) fn selected_chroma_profile(&self) -> Option<&String> {
        self.lighting_runtime
            .chroma_profile_draft
            .as_ref()
            .or_else(|| {
                self.lighting_observation
                    .as_ref()
                    .and_then(|observation| observation.selected_chroma_profile_guid.as_ref())
            })
    }

    pub(super) fn rgb_can_sync(&self) -> bool {
        let Some(observation) = &self.lighting_observation else {
            return false;
        };
        if self.rgb_effect_id() == 17 {
            return observation.valid_devices.iter().any(|peer| {
                peer.product_id != self.spec.product_id && peer.support_effects.contains(&17)
            });
        }
        observation
            .connected_chroma_devices
            .as_ref()
            .is_some_and(|peers| {
                peers.len() > 1
                    && peers
                        .iter()
                        .any(|peer| peer.product_id != self.spec.product_id)
            })
    }

    pub(super) fn rgb_disabled(&self) -> bool {
        self.lighting_controls_disabled() || self.rgb_effects_overridden()
    }

    pub(super) fn rgb_effects_overridden(&self) -> bool {
        self.rgb_nanoleaf_disabled() || self.rgb_visualizer_disabled()
    }

    pub(super) fn rgb_nanoleaf_disabled(&self) -> bool {
        self.lighting_observation
            .as_ref()
            .is_some_and(|observation| observation.nano_leaf_enabled == Some(false))
    }

    pub(super) fn rgb_visualizer_disabled(&self) -> bool {
        !self.rgb_bluetooth_keyboard()
            && self
                .lighting_observation
                .as_ref()
                .is_some_and(|observation| observation.is_chroma_visualizer_enabled)
    }

    pub(super) fn rgb_power_saving_disabled(&self) -> bool {
        self.lighting_observation
            .as_ref()
            .is_some_and(|observation| {
                observation.use_power_saving_quick_effect
                    && observation.system_power_status.as_deref() == Some("battery")
            })
    }

    pub(super) fn rgb_quick_disabled(&self) -> bool {
        self.rgb_disabled()
            || self.rgb_power_saving_disabled()
            || (!self.rgb_hardware_effect() && self.chroma_mode() != Some(false))
    }

    pub(super) fn rgb_bluetooth_keyboard(&self) -> bool {
        self.observed_bluetooth() == Some(true)
            && self.spec.config["DeviceInfo"]["category"] == "KEYBOARD"
    }

    pub(super) fn request_rgb_effect(&self, cx: &mut Context<Self>) {
        let payload = json!({"selectedEffectId":self.rgb_effect_id(),
            "selectedEffectSetting":self.draft["quickEffects"]["selectedEffectSetting"], "regionId":0});
        self.request_lighting_message("ON_SET_EFFECTS", json!({"effectSetting":payload}), cx);
    }

    pub(super) fn commit_rgb_custom_colors(
        &mut self,
        owner: &KeyboardLightingOwner,
        edit: razer_model::preferences::CustomColorEdit,
        cx: &mut Context<Self>,
    ) {
        if owner != &self.lighting_owner() || self.rgb_quick_disabled() {
            return;
        }
        use razer_model::preferences::ProfileCustomColors;
        let source = self
            .lighting_runtime
            .custom_colors_draft
            .as_ref()
            .or_else(|| {
                self.lighting_observation
                    .as_ref()
                    .map(|observation| &observation.custom_colors)
            });
        let colors = (|| -> anyhow::Result<_> {
            let colors = if let Some(source) = source.filter(|source| !source.is_empty()) {
                let mut colors: ProfileCustomColors = serde_json::from_value(json!(source))?;
                colors.apply_edit(edit).map_err(anyhow::Error::msg)?;
                colors
            } else {
                ProfileCustomColors::from_slots(cx.global::<razer_state::CustomColors>().colors())
            };
            let Value::Array(colors) = serde_json::to_value(colors.slot_records())? else {
                unreachable!("profile color slots serialize as an array");
            };
            Ok(colors)
        })();
        let colors = match colors {
            Ok(colors) => colors,
            Err(error) => {
                eprintln!("[keyboard profile custom colors] {error:#}");
                return;
            }
        };
        self.lighting_runtime.custom_colors_draft = Some(colors.clone());
        self.request_lighting_message("SET_CUSTOM_COLORS", json!({"customColors":colors}), cx);
    }

    pub(super) fn request_switch_off_lighting(&self, cx: &mut Context<Self>) {
        if self.lighting_controls_disabled() {
            return;
        }
        // The source event uses window.currentTimerTick || Date.now(). Each
        // native UI action has a fresh timestamp; transport acknowledgement
        // must return a reducer observation, never a locally invented success.
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_millis() as u64)
            .unwrap_or(0);
        self.request_lighting_command(
            KeyboardLightingCommand::Middleware(json!({
                "type":"ON_SWITCH_OFF_SETTING", "timerTick":tick,
                "payload":{"switchOffLighting":self.draft["switchOffLighting"]}
            })),
            cx,
        );
    }

    pub(super) fn request_lighting_message(
        &self,
        kind: &str,
        payload: Value,
        cx: &mut Context<Self>,
    ) {
        self.request_lighting_command(
            KeyboardLightingCommand::Middleware(json!({"type":kind,"payload":payload})),
            cx,
        );
    }

    pub(super) fn sync_rgb_effect(&self, cx: &mut Context<Self>) {
        if !self.rgb_can_sync() || self.rgb_quick_disabled() {
            return;
        }
        if self
            .lighting_runtime
            .sync_started
            .get()
            .is_none_or(|start| start.elapsed() >= std::time::Duration::from_millis(1200))
        {
            self.lighting_runtime
                .sync_started
                .set(Some(std::time::Instant::now()));
            self.lighting_runtime
                .sync_animation
                .set(self.lighting_runtime.sync_animation.get().wrapping_add(1));
        }
        let id = self.rgb_effect_id();
        let setting = self.rgb_setting();
        let source_colors = self
            .lighting_runtime
            .custom_colors_draft
            .as_ref()
            .or_else(|| {
                self.lighting_observation
                    .as_ref()
                    .map(|observation| &observation.custom_colors)
            });
        let custom_colors: Vec<Value> = source_colors
            .into_iter()
            .flatten()
            .filter(|entry| {
                entry["picked"] == true
                    && setting.as_object().is_some_and(|setting| {
                        setting.iter().any(|(key, value)| {
                            key.to_ascii_lowercase().starts_with("color")
                                && value == &entry["color"]
                        })
                    })
            })
            .cloned()
            .collect();
        let mut payload = json!({"selectedEffectId":id,"effectSettings":setting,
            "customColors":custom_colors});
        if id == 4 {
            // All 64 audited keyboard callers have LeftRight (or no override).
            // Source ct maps LeftRight value 2 to sync direction index 1.
            payload["indexDirection"] = json!(if setting["direction"] == 2 { 1 } else { 0 });
            payload["numberOfWaveDirection"] = json!(2);
        }
        self.request_lighting_message("ON_SYNC_QUICK_EFFECT", payload, cx);
        cx.notify();
    }

    pub(super) fn rgb_sync_icon(&self) -> AnyElement {
        let icon = img("synapse/chroma_sync_v3_static.svg")
            .size(surface::css(26.))
            .mr(surface::css(10.));
        if self
            .lighting_runtime
            .sync_started
            .get()
            .is_some_and(|start| start.elapsed() < std::time::Duration::from_millis(1200))
        {
            div()
                .size(surface::css(26.))
                .mr(surface::css(10.))
                .with_animation(
                    SharedString::from(format!(
                        "keyboard-chroma-sync-turn-{}",
                        self.lighting_runtime.sync_animation.get()
                    )),
                    Animation::new(std::time::Duration::from_millis(1200))
                        .with_easing(gpui_kit::linear),
                    |icon, phase| {
                        let frame = ((phase * 108.).floor() as usize) % 36;
                        icon.child(
                            img(SharedString::from(format!(
                                "synapse/keyboard-chroma-sync-{frame:02}.png"
                            )))
                            .size_full(),
                        )
                    },
                )
                .into_any_element()
        } else {
            icon.into_any_element()
        }
    }
}

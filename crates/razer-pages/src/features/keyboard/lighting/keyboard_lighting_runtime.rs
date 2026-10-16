//! Chroma reducer observations and the original host/middleware boundaries.
//! A service cache does not complete `checkIsFullyInstall`; None stays invisible.
use super::*;
use crate::features::controls::Choice;
use std::cell::{Cell, RefCell};

#[derive(Default)]
pub(super) struct State {
    generation: u64,
    next_request: Cell<u64>,
    last_mutation: Cell<u64>,
    pending: RefCell<BTreeMap<u64, KeyboardLightingCommand>>,
    last_error: Option<String>,
    sync_started: Cell<Option<std::time::Instant>>,
    sync_animation: Cell<u64>,
    custom_colors_draft: Option<Vec<Value>>,
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
}

#[derive(Clone, Default)]
pub struct KeyboardChromaPeer {
    pub product_id: u32,
    pub support_effects: Vec<u64>,
}

#[derive(Clone, Default)]
pub struct KeyboardLightingObservation {
    pub check_is_fully_installed: Option<bool>,
    pub chroma_installed_module: bool,
    pub chroma_profiles: Vec<Value>,
    pub selected_chroma_profile_guid: Option<String>,
    pub is_chroma_enabled: bool,
    pub is_chroma_visualizer_enabled: bool,
    /// Original chromaConnectApp appName/appIcon observation.
    pub chroma_connect_app: Option<Value>,
    pub use_power_saving_quick_effect: bool,
    pub system_power_status: Option<String>,
    pub is_dynamic_lighting_enabled: bool,
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
    pub fn accepts_lighting_request(&self, request: &KeyboardLightingRequested) -> bool {
        request.owner == self.lighting_owner()
            && self
                .lighting_runtime
                .pending
                .borrow()
                .contains_key(&request.request_id)
    }

    pub fn lighting_owner(&self) -> KeyboardLightingOwner {
        KeyboardLightingOwner {
            product_id: self.spec.product_id,
            generation: self.lighting_runtime.generation,
            profile_guid: self.draft["guid"].as_str().map(str::to_owned),
        }
    }

    pub(super) fn cancel_lighting_requests(&mut self) {
        self.lighting_runtime.generation = self.lighting_runtime.generation.wrapping_add(1);
        self.lighting_runtime.pending.borrow_mut().clear();
        self.lighting_runtime.last_error = None;
        self.lighting_runtime.sync_started.set(None);
        self.lighting_runtime.custom_colors_draft = None;
        if let Some(observation) = &mut self.lighting_observation {
            observation.installer_busy = false;
            observation.check_is_fully_installed = None;
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
        });
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
                        self.analog_gamepad.advanced = self
                            .lighting_observation
                            .as_ref()
                            .is_some_and(|observation| observation.is_chroma_enabled);
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
        self.analog_gamepad.advanced = observation.is_chroma_enabled;
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
        self.rgb_disabled() || self.rgb_power_saving_disabled()
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
        colors: &razer_state::CustomColorSlots,
        cx: &mut Context<Self>,
    ) {
        if owner != &self.lighting_owner() || self.rgb_quick_disabled() {
            return;
        }
        let colors: Vec<Value> = colors
            .iter()
            .map(|color| match color {
                Some([red, green, blue]) => {
                    json!({"color":format!("#{red:02x}{green:02x}{blue:02x}"),"picked":true})
                }
                None => json!({"color":"#111","picked":false}),
            })
            .collect();
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

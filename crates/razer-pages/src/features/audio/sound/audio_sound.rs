//! Current six headset Sound roots. Current Cloud EQ starts empty; local drafts
//! and original requests are distinct from native observation/acknowledgement.
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderTrack};
use razer_widgets::theme::AudioEnhancementColors as Colors;

#[path = "audio_sound_profile.rs"]
mod profile_spatial;

#[derive(Deserialize)]
pub(super) struct Product {
    product_id: u32,
    pub(super) configs: Value,
    cloud_initial: Value,
    pub(super) locales: BTreeMap<String, BTreeMap<String, String>>,
    pub(super) media: Value,
    pub(super) actions: BTreeMap<String, String>,
    native: BTreeMap<String, Native>,
    // Historical extraction called these helpers "queue"; current complete
    // helper receipts establish they construct telemetry, not device tasks.
    #[serde(alias = "queue")]
    telemetry: BTreeMap<String, String>,
    initials: Value,
}
#[derive(Deserialize)]
struct Native {
    #[serde(rename = "type")]
    kind: String,
    timer_tick: bool,
}
pub(super) fn product(pid: u32) -> Option<&'static Product> {
    static DATA: OnceLock<Vec<Product>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("audio_sound_data.json"))
            .expect("current independent Sound mount receipts")
    })
    .iter()
    .find(|row| row.product_id == pid)
}
pub(super) fn message(row: &Product, key: &str) -> String {
    message_for_locale(row, key, &razer_i18n::locale())
}
pub(super) fn message_for_locale(row: &Product, key: &str, locale: &str) -> String {
    row.locales
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(locale))
        .and_then(|(_, values)| values.get(key))
        .or_else(|| row.locales.get("en").and_then(|values| values.get(key)))
        .cloned()
        .unwrap_or_default()
}
pub(super) fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
pub(super) fn tick() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}
#[derive(Default)]
pub(super) struct State {
    preview_source: BTreeMap<String, Value>,
    game_preview: Option<f32>,
    game_last_visible: Option<f32>,
    game_pressed: bool,
    thx_help_hovered: bool,
    pub(super) image_edition: Option<u64>,
    pub(super) image_assets: Option<Value>,
    pub(super) customizer_hover: Option<usize>,
    pub(super) mapping: super::sound_mapping::MappingState,
    retained_device: Option<Value>,
    pub(super) edit_eq_shown: bool,
    pub(super) edit_eq_changed: bool,
    pub(super) eq_remove: Option<Value>,
    pub(super) eq_reset: bool,
    pub(super) eq_drag: Option<usize>,
    pub(super) eq_hover: Option<usize>,
    pub(super) eq_hover_range: Option<Value>,
    pub(super) eq_disable_reset: Option<bool>,
    pub(super) eq_zero_hovered: bool,
    pub(super) eq_zero_pressed: bool,
    pub(super) eq_preview: Option<Vec<f32>>,
    pub(super) eq_band_data: Option<Vec<f32>>,
    pub(super) eq_local_enabled: Option<Vec<Value>>,
    pub(super) eq_search: Option<Entity<gpui_kit::component::input::InputState>>,
    pub(super) apps_shown: bool,
    pub(super) apps_refresh: bool,
    pub(super) apps_searching: bool,
    pub(super) apps_refresh_hovered: bool,
    pub(super) apps_search_hovered: bool,
    pub(super) apps_clear_hovered: bool,
    pub(super) apps_search: Option<Entity<gpui_kit::component::input::InputState>>,
    pub(super) apps_dropdown: Option<Value>,
    pub(super) apps_remove: Option<Value>,
    pub(super) apps_remove_shown: bool,
    pub(super) apps_remove_generation: u64,
    pub(super) apps_remove_task: Option<Task<()>>,
    pub(super) apps_cloud_observed: bool,
    pub(super) apps_hover_name: Option<usize>,
    pub(super) apps_hover_icon: Option<usize>,
    pub(super) apps_thx_hover: Option<usize>,
    pub(super) apps_refresh_generation: u64,
    pub(super) apps_refresh_task: Option<Task<()>>,
}
impl AudioProductWorkspace {
    pub(super) fn initialize_sound(&mut self) {
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        self.draft["device"]["volume"] = row.initials["volume"].clone();
        self.draft["sound"] = json!({
            "thxSpatialAudio":row.initials["spatial"],
            "gameChatBalance":row.initials["game_chat"],
            "cloudEQ":row.cloud_initial,
            "applicationList":[],"generalApplications":[],
            "installedAppList":[],"specificInputSetting":[],
            "isShowGameChatTutorial":false,
        });
        // The former descriptor's four presets are historical, not a current
        // native Cloud EQ observation. The MIC equalizer has its own source.
        if let Some(equalizers) = self.draft["equalizers"].as_object_mut() {
            equalizers.remove("audio");
        }
    }
    pub(super) fn initialize_sound_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        self.add_slider(
            "/sound/gameChatBalance/balanceMode",
            0.,
            row.configs["volume"]["gameChatBalanceConfig"]["max"]
                .as_f64()
                .unwrap_or(20.) as f32,
            1.,
            window,
            cx,
        );
    }
    pub(super) fn capture_sound_device(&mut self) {
        self.sound.retained_device =
            Some(json!({"volume":self.draft["device"]["volume"],"sound":self.draft["sound"]}));
    }
    pub(super) fn restore_sound_device(&mut self, saved: Option<&Value>) {
        if product(self.spec.product_id).is_none() {
            return;
        }
        if let Some(state) = &self.sound.retained_device {
            self.draft["device"]["volume"] = state["volume"].clone();
            self.draft["sound"] = state["sound"].clone();
        }
        // Current owner loadDeviceLocalData and SET_SPATIAL_ENABLE.restoreDeviceData
        // publish the active profile's complete spatial payload. Retained device
        // state cannot overwrite that profile with the previous profile's toggle.
        // This is a local-profile/UI restore, independent of device setters/getters.
        // Freeze this restore behavior to the six independently audited owners;
        // later Sound descriptors must not widen the evidence boundary.
        if matches!(
            self.spec.product_id,
            1398 | 1401 | 2638 | 2641 | 4124 | 4126
        ) {
            profile_spatial::restore_profile_spatial(&mut self.draft, saved);
        }
        self.capture_sound_device();
        self.remount_sound();
    }
    pub(super) fn remount_sound(&mut self) {
        self.sound.preview_source.clear();
        self.sound.game_preview = None;
        self.sound.game_last_visible = None;
        self.sound.game_pressed = false;
        self.sound.thx_help_hovered = false;
        self.sound.image_edition = None;
        self.sound.image_assets = None;
        self.sound.customizer_hover = None;
        self.sound.mapping = super::sound_mapping::MappingState::default();
        self.sound.edit_eq_shown = false;
        self.sound.eq_drag = None;
        self.sound.eq_hover = None;
        self.sound.eq_hover_range = None;
        self.sound.eq_zero_hovered = false;
        self.sound.eq_zero_pressed = false;
        self.sound.eq_preview = None;
        self.sound.eq_band_data = None;
        self.sound.eq_disable_reset = None;
        self.sound.eq_local_enabled = None;
        self.sound.apps_shown = false;
        self.sound.apps_searching = false;
        self.sound.apps_refresh_hovered = false;
        self.sound.apps_search_hovered = false;
        self.sound.apps_clear_hovered = false;
        self.sound.apps_dropdown = None;
        self.sound.apps_thx_hover = None;
        self.sound.apps_remove = None;
        self.sound.apps_remove_shown = false;
        self.sound.apps_remove_task = None;
        self.sound.apps_refresh_task = None;
        self.sound.apps_refresh = false;
        self.sound.apps_refresh_generation = self.sound.apps_refresh_generation.wrapping_add(1);
        self.sound.apps_remove_generation = self.sound.apps_remove_generation.wrapping_add(1);
    }
    pub(super) fn preserves_sound_preview(&self, path: &str) -> bool {
        self.sound
            .preview_source
            .get(path)
            .is_some_and(|value| self.draft.pointer(path) == Some(value))
    }
    pub(super) fn emit_sound(&self, messages: Vec<Value>, cx: &mut Context<Self>) {
        if let Some(owner) = &self.source_owner {
            owner.update(cx, |owner, cx| owner.request_audio_sound(messages, cx));
        }
    }
    pub(super) fn emit_sound_action(&self, action: &str, payload: Value, cx: &mut Context<Self>) {
        let Some(kind) = product(self.spec.product_id).and_then(|p| p.actions.get(action)) else {
            return;
        };
        let mut redux = json!({"type":kind});
        if !payload.is_null() {
            redux["payload"] = payload;
        }
        self.emit_sound(vec![json!({"redux":redux})], cx);
    }
    fn submit_sound_control(
        &mut self,
        action: &str,
        payload: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        let Some(native) = row.native.get(action) else {
            return;
        };
        let thx_was_enabled = self.draft["sound"]["thxSpatialAudio"]["isEnabled"] == json!(true);
        let native_payload = match action {
            "setVolume" => {
                self.draft["device"]["volume"] = payload.clone();
                json!({"volume":payload})
            }
            "setGameChatBalance" => {
                self.draft["sound"]["gameChatBalance"]["balanceMode"] = payload.clone();
                json!({"balanceMode":payload})
            }
            "setTHXSpatialEnable" => {
                self.draft["sound"]["thxSpatialAudio"]["isEnabled"] = payload.clone();
                // SET_SPATIAL_ENABLE.updateStorage replaces the complete active
                // profile value; the sound reducer only updates its isEnabled.
                self.draft["profile"]["thxSpatialAudio"] = json!({"isEnabled":payload});
                json!({"thxEnabled":payload})
            }
            _ => return,
        };
        let mut messages = Vec::new();
        if let Some(kind) = row.telemetry.get(action) {
            // These helpers feed source telemetry, not the device task queue.
            // Preserve the source metadata locally; local-first excludes sending it.
            messages.push(json!({"sourceTelemetry":{"type":kind,"payload":payload}}));
        }
        let mut transport = json!({"type":native.kind,"payload":native_payload});
        if native.timer_tick {
            transport["timerTick"] = json!(tick());
        }
        messages.push(
            json!({"redux":{"type":row.actions[action],"payload":payload},"transport":transport}),
        );
        self.emit_sound(messages, cx);
        if action == "setTHXSpatialEnable" && !thx_was_enabled && payload == json!(true) {
            self.sound_thx_preset_fallback(cx);
        }
        self.capture_sound_device();
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    pub(super) fn sound_slider_event(
        &mut self,
        path: &str,
        event: &SliderEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if product(self.spec.product_id).is_none() {
            return false;
        }
        if path == "/sound/gameChatBalance/balanceMode" {
            // Original fN owns only input.onMouseUp. Base's window Release
            // must not dispatch a device write after releasing outside it.
            if let SliderEvent::Change(value) = event {
                let maximum = product(self.spec.product_id)
                    .and_then(|row| row.configs["volume"]["gameChatBalanceConfig"]["max"].as_f64())
                    .unwrap_or(20.) as f32;
                let old = self
                    .sound
                    .game_last_visible
                    .unwrap_or_else(|| {
                        self.draft
                            .pointer(path)
                            .and_then(Value::as_f64)
                            .unwrap_or(50.) as f32
                    })
                    .clamp(0., maximum);
                let next = value.start().clamp(0., maximum);
                // Native range onChange does not run when the DOM's clamped
                // value stays at 20, even though React's local state is 50.
                if next == old {
                    return true;
                }
                self.sound.game_last_visible = Some(next);
                self.sound.game_preview = Some(next);
                if let Some(value) = self.draft.pointer(path) {
                    self.sound.preview_source.insert(path.into(), value.clone());
                }
                cx.notify();
            }
            return true;
        }
        if path != "/device/volume/value" {
            return false;
        }
        match event {
            SliderEvent::Change(_) => {
                self.sound
                    .preview_source
                    .insert(path.into(), self.draft["device"]["volume"]["value"].clone());
                cx.notify();
            }
            SliderEvent::Release(value) => {
                self.sound.preview_source.remove(path);
                let value = value.start().clamp(0., 100.).round() as u32;
                self.submit_sound_control(
                    "setVolume",
                    json!({"isEnabled":value != 0,"value":value}),
                    window,
                    cx,
                );
            }
        }
        true
    }
    fn commit_sound_balance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sound.game_pressed = false;
        let value = self
            .sound
            .game_preview
            .take()
            .map(|value| json!(value.round() as u32))
            .unwrap_or_else(|| self.draft["sound"]["gameChatBalance"]["balanceMode"].clone());
        self.sound
            .preview_source
            .remove("/sound/gameChatBalance/balanceMode");
        self.submit_sound_control("setGameChatBalance", value, window, cx);
    }
    pub(crate) fn observe_sound(
        &mut self,
        key: &str,
        payload: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if product(self.spec.product_id).is_none() {
            return false;
        }
        match key {
            "mappingList" if payload.is_array() => {
                self.draft["sound"]["mappingList"] = payload;
                self.sound.mapping.observed = true;
                self.refresh_sound_mapping(window, cx);
            }
            "currentFWVersion" if payload.is_string() => {
                self.draft["sound"]["currentFWVersion"] = payload;
                self.refresh_sound_mapping(window, cx);
            }
            "audioApplicationList" if payload.is_array() => {
                let fallback = product(self.spec.product_id)
                    .and_then(|row| row.media["placeholder_game_icon"].as_str())
                    .unwrap_or_default();
                let mut rows = Vec::new();
                for source in payload
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|source| !js_truthy(&source["isMissing"]))
                {
                    let mut row = source.clone();
                    let signature = &source["gameSignature"];
                    // JavaScript || accepts arrays too; render handles an
                    // exeName string or array. Undefined derived keys are absent.
                    let exe = signature
                        .get("default_exe")
                        .filter(|value| js_truthy(value))
                        .or_else(|| {
                            signature
                                .get("default_exe32_bit")
                                .filter(|value| js_truthy(value))
                        })
                        .or_else(|| signature.get("default_exe64_bit"));
                    if let Some(exe) = exe {
                        row["exeName"] = exe.clone();
                    } else if let Some(row) = row.as_object_mut() {
                        row.remove("exeName");
                    }
                    let game_id = if source.get("gameSettings").is_some_and(js_truthy) {
                        source["gameSettings"].get("INFO_EntryGUID")
                    } else {
                        source.get("id")
                    };
                    if let Some(game_id) = game_id {
                        row["gameId"] = game_id.clone();
                    } else if let Some(row) = row.as_object_mut() {
                        row.remove("gameId");
                    }
                    row["icon"] =
                        if matches!(signature["source_ex"].as_str(), Some("STEAM" | "UWP")) {
                            signature["cover_art"].clone()
                        } else {
                            source
                                .get("icon")
                                .filter(|value| value.as_str().is_some_and(|v| !v.is_empty()))
                                .cloned()
                                .unwrap_or(json!(fallback))
                        };
                    // Source's trailing `...t` overrides the derived fields.
                    for (name, value) in source.as_object().into_iter().flatten() {
                        row[name] = value.clone();
                    }
                    rows.push(row);
                }
                self.draft["sound"]["applicationList"] = json!(rows);
                self.draft["sound"]["generalApplications"] = json!([]);
            }
            "volume"
                if payload.get("isEnabled").and_then(Value::as_bool).is_some()
                    && payload.get("value").and_then(Value::as_f64).is_some() =>
            {
                self.draft["device"]["volume"] = payload
            }
            "balanceMode" if payload.as_f64().is_some_and(|value| value >= 0.) => {
                if self.draft["sound"]["gameChatBalance"]["balanceMode"] != payload {
                    self.sound.game_preview = None;
                    self.sound.game_last_visible = None;
                    self.sound
                        .preview_source
                        .remove("/sound/gameChatBalance/balanceMode");
                }
                self.draft["sound"]["gameChatBalance"]["balanceMode"] = payload
            }
            "thxSpatialAudio" if payload.get("isEnabled").and_then(Value::as_bool).is_some() => {
                let was_enabled =
                    self.draft["sound"]["thxSpatialAudio"]["isEnabled"] == json!(true);
                let enabled = payload["isEnabled"] == json!(true);
                self.draft["sound"]["thxSpatialAudio"] = payload;
                if !was_enabled && enabled {
                    self.sound_thx_preset_fallback(cx);
                }
            }
            "cloudEQ" if payload.is_object() => {
                let cloud = &mut self.draft["sound"]["cloudEQ"];
                for (key, value) in payload.as_object().into_iter().flatten() {
                    if cloud.get(key).is_some() || key == "bandValues" {
                        cloud[key] = value.clone();
                    }
                }
                self.sound.eq_preview = None;
                self.sound.eq_drag = None;
                self.sound.eq_band_data = None;
                self.sound.eq_disable_reset = None;
                self.sound.eq_local_enabled = None;
                if payload.get("enabledPresets").is_some() {
                    self.sound.apps_cloud_observed = true;
                }
            }
            "applicationList"
            | "generalApplications"
            | "specificInputSetting"
            | "installedAppList"
                if payload.is_array() =>
            {
                if key == "installedAppList" && self.sound.apps_refresh {
                    let generation = self.sound.apps_refresh_generation;
                    self.sound.apps_refresh_task =
                        Some(cx.spawn_in(window, async move |owner, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(300))
                                .await;
                            let _ = owner.update_in(cx, |this, _, cx| {
                                if this.sound.apps_refresh_generation == generation {
                                    this.sound.apps_refresh = false;
                                    cx.notify();
                                }
                            });
                        }));
                }
                self.draft["sound"][key] = payload
            }
            _ => return false,
        }
        self.capture_sound_device();
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        cx.notify();
        true
    }
    fn sound_balance_slider(
        &self,
        row: &Product,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = &self.sliders["/sound/gameChatBalance/balanceMode"];
        let maximum = row.configs["volume"]["gameChatBalanceConfig"]["max"]
            .as_f64()
            .unwrap_or(20.) as f32;
        let progress = (state.read(cx).value().start() / maximum).clamp(0., 1.);
        div()
            .id("current-game-chat-keyboard-guard")
            .capture_key_down(|event, _, cx| {
                if matches!(
                    event.keystroke.key.as_str(),
                    "home" | "end" | "up" | "down" | "left" | "right"
                ) {
                    cx.stop_propagation();
                }
            })
            .child(
                BaseSlider::new(state)
                    .relative()
                    .w_full()
                    .h(surface::css(53.))
                    .child(
                        div()
                            .absolute()
                            .bottom(surface::css(25.))
                            .w_full()
                            .h(surface::css(6.))
                            .rounded(surface::css(3.))
                            .bg(rgba(0x44d62c4d)),
                    )
                    .child(
                        SliderTrack::new(state)
                            .absolute()
                            .bottom(surface::css(20.))
                            .w_full()
                            .h(surface::css(16.))
                            .capture_any_mouse_down(cx.listener(
                                |this, event: &MouseDownEvent, _, cx| {
                                    if event.button == MouseButton::Left {
                                        this.sound.game_pressed = true;
                                        cx.notify();
                                    }
                                },
                            ))
                            .capture_any_mouse_up(cx.listener(
                                |this, event: &MouseUpEvent, window, cx| {
                                    if event.button == MouseButton::Left {
                                        this.commit_sound_balance(window, cx);
                                    }
                                },
                            ))
                            .child(
                                SliderIndicator::new(state)
                                    .absolute()
                                    .left(surface::css(8.))
                                    .right(surface::css(8.))
                                    .h_full()
                                    .child(
                                        razer_widgets::source_slider::source_thumb(
                                            state, true, window, cx,
                                        )
                                        .absolute()
                                        .left(relative(progress))
                                        .ml(surface::css(-8.))
                                        .size(surface::css(16.))
                                        .rounded(surface::css(8.)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom(surface::css(22.))
                            .left(relative(0.5))
                            .text_color(Colors::secondary())
                            .child("|"),
                    )
                    .child(
                        div().absolute().left_0().top(surface::css(37.)).child(
                            img(row.media["css"]["chat-more"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string())
                            .size(surface::css(24.)),
                        ),
                    )
                    .child(
                        div().absolute().right_0().top(surface::css(37.)).child(
                            img(row.media["css"]["game-more"]
                                .as_str()
                                .unwrap_or_default()
                                .to_string())
                            .size(surface::css(24.)),
                        ),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top(surface::css(70.))
                            .text_size(surface::css(12.))
                            .text_color(Colors::secondary())
                            .child(message(row, "MORE_GAME").to_uppercase()),
                    )
                    .child(
                        div()
                            .absolute()
                            .right_0()
                            .top(surface::css(70.))
                            .text_size(surface::css(12.))
                            .text_color(Colors::secondary())
                            .child(message(row, "MORE_CHAT").to_uppercase()),
                    ),
            )
            .into_any_element()
    }
    fn current_sound_volume(
        &self,
        row: &Product,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let enabled = self.draft["device"]["volume"]["isEnabled"] == json!(true);
        let state = &self.sliders["/device/volume/value"];
        let value = state.read(cx).value().start();
        let owner = cx.entity();
        surface::panel_with_title_switch(
            message(row, "VOLUME_HEADER"),
            surface::SynapseSwitch::new("current-headset-volume-switch")
                .checked(enabled)
                .accessibility_label(message(row, "VOLUME_HEADER"))
                .on_change(move |enabled, window, cx| {
                    owner.update(cx, |this, cx| {
                        let mut payload = this.draft["device"]["volume"].clone();
                        payload["isEnabled"] = json!(*enabled);
                        this.submit_sound_control("setVolume", payload, window, cx);
                    })
                }),
            surface::help_control(
                "current-headset-volume-tip",
                message(row, "VOLUME_NOMMO_TOOLTIP"),
            ),
            cx,
        )
        .child(
            div()
                .relative()
                .capture_key_down(|event, _, cx| {
                    if matches!(
                        event.keystroke.key.as_str(),
                        "home" | "end" | "left" | "right" | "up" | "down"
                    ) {
                        cx.stop_propagation();
                    }
                })
                .child(
                    razer_widgets::source_slider::SourceSlider::new(state, value / 100.)
                        .enabled(enabled)
                        .tip(Some(format!("{value:.0}"))),
                )
                .child(
                    div()
                        .absolute()
                        .bottom_0()
                        .w_full()
                        .when(!enabled, |tags| tags.opacity(0.3))
                        .child(surface::slider_tags("0", None, "100", None)),
                ),
        )
        .child(self.sound_windows_link(
            "current-volume-mixer-link",
            "OPEN_WINDOW_VOLUME_MIXER",
            false,
            row,
            cx,
        ))
        .child(
            h_flex()
                .mt(surface::css(10.))
                .gap(surface::css(6.))
                .child(message(row, "GAME_CHAT_BALANCE").to_uppercase())
                .child(surface::help_control(
                    "current-game-chat-tip",
                    message(row, "GAME_CHAT_BALANCE_TIP"),
                )),
        )
        .child(self.sound_balance_slider(row, window, cx))
        .child(
            gpui_kit::base::Button::new("current-game-chat-tutorial")
                .mt(surface::css(45.))
                .p_0()
                .underline()
                .text_color(Colors::text())
                .hover(|s| s.text_color(Colors::selected()))
                .child(message(row, "OPEN_TUTORIAL_HEADSET_1"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.draft["sound"]["isShowGameChatTutorial"] = json!(true);
                    // This action is not SHOW_THX_NEWS. Current owner mounting
                    // of this distinct flag remains a recorded source gap.
                    this.emit_sound_action("showGameChatTutorial", json!(true), cx);
                    cx.notify();
                })),
        )
        .into_any_element()
    }
    fn sound_windows_link(
        &self,
        id: &'static str,
        label: &str,
        properties: bool,
        row: &Product,
        cx: &Context<Self>,
    ) -> AnyElement {
        gpui_kit::base::Button::new(id)
            .p_0()
            .self_start()
            .text_color(Colors::text())
            .hover(|s| s.text_color(Colors::selected()))
            .child(message(row, label))
            .child(img("synapse/audio-volume-external-sprite.svg#link").size(surface::css(16.)))
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(owner) = &this.source_owner {
                    owner.update(cx, |owner, cx| {
                        owner.request_sound_windows(properties, cx);
                    });
                }
            }))
            .into_any_element()
    }
    fn current_sound_output(&self, row: &Product, cx: &Context<Self>) -> AnyElement {
        let version = razer_platform::system::windows_major_version().unwrap_or("11");
        let icon = if version == "11" {
            "common-windows-11"
        } else {
            "window-big-icon"
        };
        surface::panel_with_help(
            message(row, "SOUND_PROPERTIES"),
            surface::help_control(
                "current-sound-properties-tip",
                message(row, "SOUND_PROPERTIES_TOOLTIP"),
            ),
            cx,
        )
        .child(
            h_flex()
                .gap(surface::css(20.))
                .child(
                    img(row.media["css"][icon]
                        .as_str()
                        .unwrap_or_default()
                        .to_string())
                    .size(surface::css(44.))
                    .object_fit(ObjectFit::Contain),
                )
                .child(self.sound_windows_link(
                    "current-sound-properties-link",
                    "OPEN_WINDOW_PROPERTIES_SOUND",
                    true,
                    row,
                    cx,
                )),
        )
        .into_any_element()
    }
    fn current_thx_toggle(&self, row: &Product, cx: &Context<Self>) -> AnyElement {
        let active = self.draft["sound"]["thxSpatialAudio"]["isEnabled"] == json!(true);
        let overhead = row.configs["support_overhead_speaker"] == json!(true);
        let mut help = v_flex()
            .max_w(surface::css(300.))
            .gap(surface::css(14.))
            .child(message(
                row,
                if overhead {
                    "SURROUND_SOUND_714_DESC"
                } else {
                    "DEMO_TOOLTIP1"
                },
            ));
        if overhead {
            if let Some(image) = row.media["thx_help"].as_str() {
                help = help.child(img(image.to_string()).size(surface::css(180.)));
            }
        } else {
            help = help.child(message(row, "DEMO_TOOLTIP2"));
        }
        h_flex()
            .justify_center()
            .mt(surface::css(20.))
            .mb(surface::css(10.))
            .px(surface::css(10.))
            .child(
                gpui_kit::base::Button::new("current-stereo-thx-toggle")
                    .h(surface::css(36.))
                    .px(surface::css(4.))
                    .rounded(surface::css(18.))
                    .border(surface::css(1.5))
                    .border_color(Colors::border())
                    .bg(Colors::background())
                    .hover(|s| s.border_color(Colors::selected()))
                    .active(|s| s.bg(rgb(0x5d5d5d)))
                    .child(h_flex().gap(surface::css(5.)).children(
                        [(false, "STEREO"), (true, "THX_SPATIAL_AUDIO")].map(|(selected, key)| {
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .min_w(surface::css(152.5))
                                .min_h(surface::css(26.))
                                .px(surface::css(10.))
                                .rounded(surface::css(14.))
                                .when(active == selected, |s| s.bg(Colors::selected()))
                                .text_size(surface::css(14.))
                                .line_height(surface::css(14.))
                                .font_weight(FontWeight::NORMAL)
                                .text_color(if active == selected {
                                    rgb(0x212121).into()
                                } else {
                                    Colors::text()
                                })
                                .child(
                                    div()
                                        .mt(surface::css(1.))
                                        .child(message(row, key).to_uppercase()),
                                )
                        }),
                    ))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.submit_sound_control("setTHXSpatialEnable", json!(!active), window, cx)
                    })),
            )
            .child(
                div()
                    .relative()
                    .m(surface::css(10.))
                    .size(surface::css(14.))
                    .child(
                        div()
                            .id("current-thx-help")
                            .size_full()
                            .on_hover(cx.listener(|this, hovered, _, cx| {
                                this.sound.thx_help_hovered = *hovered;
                                cx.notify();
                            }))
                            .child(img("synapse/automation-tooltip_questionmark.svg").size_full()),
                    )
                    .when(self.sound.thx_help_hovered, |body| {
                        body.child(
                            deferred(
                                div()
                                    .absolute()
                                    .top(surface::css(24.))
                                    .right_0()
                                    .w(surface::css(300.))
                                    .px(surface::css(10.))
                                    .py(surface::css(8.))
                                    .border_1()
                                    .border_color(Colors::border())
                                    .bg(rgb(0x000000))
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(18.))
                                    .text_color(Colors::text())
                                    .child(help),
                            )
                            .with_priority(105),
                        )
                    }),
            )
            .into_any_element()
    }
    pub(super) fn current_sound_page(
        &self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if key != "TAB_SOUND" {
            return None;
        }
        let row = product(self.spec.product_id)?;
        let mapping_root = self.sound.mapping.root.clone();
        Some(
            v_flex()
                .relative()
                .w_full()
                .min_w(surface::css(1220.))
                .gap(surface::css(10.))
                .text_color(Colors::text())
                .child(
                    canvas(
                        move |bounds, _, _| mapping_root.set(bounds),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .child(self.current_headset_image(row, cx))
                .child(self.current_thx_toggle(row, cx))
                .child(self.current_cloud_eq(row, window, cx))
                .child(
                    h_flex()
                        .items_start()
                        .gap(surface::css(20.))
                        .child(
                            v_flex()
                                .flex_1()
                                .child(self.current_sound_applications(row, cx)),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .gap(surface::css(20.))
                                .child(self.current_sound_volume(row, window, cx))
                                .child(self.current_sound_output(row, cx)),
                        ),
                )
                .when(self.sound.edit_eq_shown, |body| {
                    body.child(self.current_cloud_eq_editor(row, window, cx))
                })
                .when(self.sound.apps_shown, |body| {
                    body.child(self.current_sound_apps_modal(row, window, cx))
                })
                .child(self.sound_mapping_overlays(row, cx))
                .into_any_element(),
        )
    }
}

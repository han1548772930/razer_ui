//! Current six headset Sound roots. Current Cloud EQ starts empty; local drafts
//! and original requests are distinct from native observation/acknowledgement.
use super::*;
use gpui_kit::base::{Slider as BaseSlider, SliderIndicator, SliderTrack};
use razer_widgets::theme::AudioEnhancementColors as Colors;

#[path = "audio_sound_profile.rs"]
mod profile_spatial;

#[path = "audio_sound_payload_schema.rs"]
pub(super) mod fixed;
use fixed::*;

#[derive(Deserialize)]
pub(super) struct Product {
    product_id: u32,
    pub(super) configs: SoundPayloadProductsitemConfigs,
    cloud_initial: SoundPayloadProductsitemCloudInitial,
    pub(super) locales: BTreeMap<String, BTreeMap<String, String>>,
    pub(super) media: SoundPayloadProductsitemMedia,
    pub(super) actions: BTreeMap<String, String>,
    native: BTreeMap<String, Native>,
    // These source helpers construct telemetry separately from native requests.
    #[serde(alias = "queue")]
    telemetry: BTreeMap<String, String>,
    initials: SoundPayloadProductsitemInitials,
}
#[derive(Deserialize)]
struct Native {
    #[serde(rename = "type")]
    kind: String,
    timer_tick: bool,
}
pub(super) fn product(pid: u32) -> Option<&'static Product> {
    static DATA: OnceLock<Vec<Product>> = OnceLock::new();
    DATA.get_or_init(|| read_audio_sound_catalog())
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
    pub(super) image_assets: Option<BTreeMap<String, String>>,
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
        self.draft["device"]["volume"] = json!(row.initials.volume);
        self.draft["sound"] = json!({
            "thxSpatialAudio":row.initials.spatial,
            "gameChatBalance":row.initials.game_chat,
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
            row.configs.volume.game_chat_balance_config.max as f32,
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
                    .map(|row| row.configs.volume.game_chat_balance_config.max as f64)
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
                    .and_then(|row| Some(row.media.placeholder_game_icon.as_str()))
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
        let maximum = row.configs.volume.game_chat_balance_config.max as f32;
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
                            img(row.media.css.get("chat-more").cloned().unwrap_or_default())
                                .size(surface::css(24.)),
                        ),
                    )
                    .child(
                        div().absolute().right_0().top(surface::css(37.)).child(
                            img(row.media.css.get("game-more").cloned().unwrap_or_default())
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
                    img(row.media.css.get(icon).cloned().unwrap_or_default())
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
        let overhead = row.configs.support_overhead_speaker;
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
            if let Some(image) = Some(row.media.thx_help.as_str()) {
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
                            razer_assets::deferred_resource(
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

// Generated catalog reader: razer-pages/audio_sound_data.
fn read_audio_sound_catalog() -> Vec<Product> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Product>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Product> {
        let value = match r.tag()? {
            0 => Product {
                product_id: r.child(decode_2)?,
                configs: r.child(decode_3)?,
                cloud_initial: r.child(decode_79)?,
                locales: r.child(decode_87)?,
                media: r.child(decode_90)?,
                actions: r.child(decode_89)?,
                native: r.child(decode_92)?,
                telemetry: r.child(decode_89)?,
                initials: r.child(decode_95)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigs> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigs {
                tutorial: r.child(decode_4)?,
                customizer: r.child(decode_12)?,
                thx_switch: r.child(decode_13)?,
                equalizer: r.child(decode_14)?,
                applications: r.child(decode_22)?,
                volume: r.child(decode_23)?,
                output: r.child(decode_25)?,
                application_outputs: r.child(decode_26)?,
                renderer_geometry: r.child(decode_28)?,
                device_info: r.child(decode_30)?,
                application_thx_dataset: r.child(decode_32)?,
                customizer_groups: r.child(decode_34)?,
                support_overhead_speaker: r.child(decode_5)?,
                mapping_exports: r.child(decode_18)?,
                mapping_action_types: r.child(decode_39)?,
                mapping_audio_constants: r.child(decode_40)?,
                mapping_audio_options: r.child(decode_49)?,
                mapping_assignment_table: r.child(decode_51)?,
                mapping_audio_notes: r.child(decode_39)?,
                product_exports: r.child(decode_18)?,
                product_defaultmappings: r.child(decode_18)?,
                product_defaultprofile: r.child(decode_53)?,
                product_mappings: r.child(decode_18)?,
                product_groups: r.child(decode_18)?,
                product_functions: r.child(decode_18)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsTutorial> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsTutorial {
                tutorial_by_image: r.child(decode_5)?,
                tutorial_list: r.child(decode_6)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SoundPayloadProductsitemConfigsTutorialTutoriallistitem>>
    {
        let value = match r.tag()? {
            0 => r.sequence(decode_7)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsTutorialTutoriallistitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsTutorialTutoriallistitem {
                video_name: r.child(decode_8)?,
                main_title: r.child(decode_8)?,
                contents: r.child(decode_9)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::vec::Vec<SoundPayloadProductsitemConfigsTutorialTutoriallistitemContentsitem>,
    > {
        let value = match r.tag()? {
            0 => r.sequence(decode_10)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsTutorialTutoriallistitemContentsitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsTutorialTutoriallistitemContentsitem {
                head: r.child(decode_11)?,
                body: r.child(decode_8)?,
                r#type: r.child(decode_11)?,
            },
            1 => SoundPayloadProductsitemConfigsTutorialTutoriallistitemContentsitem {
                head: r.child(decode_11)?,
                body: r.child(decode_8)?,
                r#type: None,
            },
            2 => SoundPayloadProductsitemConfigsTutorialTutoriallistitemContentsitem {
                head: None,
                body: r.child(decode_8)?,
                r#type: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_8)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => Default::default(),
            1 => r.map(decode_8, decode_8)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsThxSwitch> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsThxSwitch {
                show_help: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsEqualizer> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsEqualizer {
                compact_mode: r.child(decode_5)?,
                band_data_config: r.child(decode_15)?,
                y_axis_title: r.child(decode_18)?,
                show_sequence_number: r.child(decode_5)?,
                line_config: r.child(decode_19)?,
                frequencies: r.child(decode_18)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_15(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsEqualizerBanddataconfig> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsEqualizerBanddataconfig {
                min_value: r.child(decode_16)?,
                max_value: r.child(decode_17)?,
                total_steps: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_16(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<i64> {
        let value = match r.tag()? {
            0 => r.i64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_17(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u64> {
        let value = match r.tag()? {
            0 => r.u64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_18(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_8)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_19(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SoundPayloadProductsitemConfigsEqualizerLineConfigitem>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_20)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_20(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsEqualizerLineConfigitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsEqualizerLineConfigitem {
                border_width: r.child(decode_17)?,
                label: r.child(decode_8)?,
                fill: r.child(decode_5)?,
                point_border_color: r.child(decode_18)?,
                border_color: r.child(decode_8)?,
                point_background_color: r.child(decode_18)?,
                point_radius: r.child(decode_17)?,
                point_hover_radius: r.child(decode_17)?,
                hover_radius: r.child(decode_17)?,
                hover_border_width: r.child(decode_17)?,
                line_tension: r.child(decode_17)?,
                data: r.child(decode_21)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_21(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<i64>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_16)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_22(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsApplications> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsApplications {
                follow_current_eq: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_23(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsVolume> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsVolume {
                show_game_chat_balance: r.child(decode_5)?,
                game_chat_balance_config: r.child(decode_24)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_24(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsVolumeGamechatbalanceconfig> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsVolumeGamechatbalanceconfig {
                max: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_25(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsOutput> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsOutput {
                window_big_icon: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_26(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SoundPayloadProductsitemConfigsApplicationOutputsitem>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_27)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_27(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsApplicationOutputsitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsApplicationOutputsitem {
                name: r.child(decode_8)?,
                guid: r.child(decode_11)?,
                r#type: r.child(decode_11)?,
            },
            1 => SoundPayloadProductsitemConfigsApplicationOutputsitem {
                name: r.child(decode_8)?,
                guid: None,
                r#type: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_28(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsRendererGeometry> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsRendererGeometry {
                cvs_h: r.child(decode_17)?,
                cvs_w: r.child(decode_17)?,
                dot_radius: r.child(decode_29)?,
                circle_radius: r.child(decode_29)?,
                x_left: r.child(decode_17)?,
                x_right: r.child(decode_17)?,
                y_start: r.child(decode_17)?,
                line_length: r.child(decode_17)?,
                dot_color: r.child(decode_8)?,
                active_color: r.child(decode_8)?,
                hyper_color: r.child(decode_8)?,
                dot_gap: r.child(decode_17)?,
                x_center: r.child(decode_29)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_29(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f64> {
        let value = match r.tag()? {
            0 => r.f64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_30(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsDeviceInfo> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsDeviceInfo {
                product_id: r.child(decode_17)?,
                ble_id: r.child(decode_31)?,
                dongle_id: r.child(decode_31)?,
                vendor_id: r.child(decode_17)?,
                device_name: r.child(decode_8)?,
                master_guide_url: r.child(decode_8)?,
                support_page: r.child(decode_8)?,
                virtual_route_device_name: r.child(decode_8)?,
                is_game_chat_audio_function: r.child(decode_5)?,
                hide_lighting_idle: r.child(decode_5)?,
                category: r.child(decode_8)?,
                has_default_eq_in_game_profile: r.child(decode_5)?,
                support_overhead_speaker: r.child(decode_5)?,
                show_competitive_mode: r.child(decode_5)?,
                thx_version: r.child(decode_8)?,
                feature_min_fw: r.child(decode_12)?,
                x_box_id: None,
                is_xbox: None,
            },
            1 => SoundPayloadProductsitemConfigsDeviceInfo {
                product_id: r.child(decode_17)?,
                ble_id: r.child(decode_31)?,
                dongle_id: r.child(decode_31)?,
                vendor_id: r.child(decode_17)?,
                device_name: r.child(decode_8)?,
                master_guide_url: r.child(decode_8)?,
                support_page: r.child(decode_8)?,
                virtual_route_device_name: r.child(decode_8)?,
                is_game_chat_audio_function: r.child(decode_5)?,
                hide_lighting_idle: r.child(decode_5)?,
                category: r.child(decode_8)?,
                has_default_eq_in_game_profile: r.child(decode_5)?,
                support_overhead_speaker: r.child(decode_5)?,
                show_competitive_mode: r.child(decode_5)?,
                thx_version: r.child(decode_8)?,
                feature_min_fw: r.child(decode_12)?,
                x_box_id: r.child(decode_31)?,
                is_xbox: r.child(decode_83)?,
            },
            2 => SoundPayloadProductsitemConfigsDeviceInfo {
                product_id: r.child(decode_17)?,
                ble_id: r.child(decode_31)?,
                dongle_id: r.child(decode_31)?,
                vendor_id: r.child(decode_17)?,
                device_name: r.child(decode_8)?,
                master_guide_url: r.child(decode_8)?,
                support_page: r.child(decode_8)?,
                virtual_route_device_name: r.child(decode_8)?,
                is_game_chat_audio_function: r.child(decode_5)?,
                hide_lighting_idle: r.child(decode_5)?,
                category: r.child(decode_8)?,
                has_default_eq_in_game_profile: r.child(decode_5)?,
                support_overhead_speaker: r.child(decode_5)?,
                show_competitive_mode: r.child(decode_5)?,
                thx_version: r.child(decode_8)?,
                feature_min_fw: r.child(decode_12)?,
                x_box_id: None,
                is_xbox: r.child(decode_83)?,
            },
            3 => SoundPayloadProductsitemConfigsDeviceInfo {
                product_id: r.child(decode_17)?,
                ble_id: None,
                dongle_id: None,
                vendor_id: r.child(decode_17)?,
                device_name: r.child(decode_8)?,
                master_guide_url: r.child(decode_8)?,
                support_page: r.child(decode_8)?,
                virtual_route_device_name: r.child(decode_8)?,
                is_game_chat_audio_function: r.child(decode_5)?,
                hide_lighting_idle: r.child(decode_5)?,
                category: r.child(decode_8)?,
                has_default_eq_in_game_profile: r.child(decode_5)?,
                support_overhead_speaker: r.child(decode_5)?,
                show_competitive_mode: r.child(decode_5)?,
                thx_version: r.child(decode_8)?,
                feature_min_fw: r.child(decode_12)?,
                x_box_id: None,
                is_xbox: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_31(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u64>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_17)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_32(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<
            String,
            SoundPayloadProductsitemConfigsApplicationThxDatasetentry,
        >,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_8, decode_33)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_33(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsApplicationThxDatasetentry> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsApplicationThxDatasetentry {
                imm: r.child(decode_11)?,
                cmm: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_34(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SoundPayloadProductsitemConfigsCustomizerGroupsitem>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_35)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_35(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsCustomizerGroupsitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsCustomizerGroupsitem {
                group: r.child(decode_36)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_36(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsCustomizerGroupsitemGroup> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsCustomizerGroupsitemGroup {
                group_name: r.child(decode_8)?,
                button_list: r.child(decode_37)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_37(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::vec::Vec<SoundPayloadProductsitemConfigsCustomizerGroupsitemGroupButtonlistitem>,
    > {
        let value = match r.tag()? {
            0 => r.sequence(decode_38)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_38(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsCustomizerGroupsitemGroupButtonlistitem>
    {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsCustomizerGroupsitemGroupButtonlistitem {
                button_key: r.child(decode_8)?,
                default_value: r.child(decode_8)?,
                is_focused: r.child(decode_5)?,
                is_mapped: r.child(decode_5)?,
                is_enabled: r.child(decode_5)?,
                input_type: r.child(decode_8)?,
                mapping_type: r.child(decode_8)?,
                input_id: r.child(decode_8)?,
                counter: r.child(decode_8)?,
                assignment: r.child(decode_8)?,
                assignment_value: r.child(decode_8)?,
                button_pos: r.child(decode_8)?,
                function_list: r.child(decode_18)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_39(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, std::vec::Vec<String>>> {
        let value = match r.tag()? {
            0 => r.map(decode_8, decode_18)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_40(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsMappingAudioConstants> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsMappingAudioConstants {
                m: r.child(decode_8)?,
                y: r.child(decode_8)?,
                h: r.child(decode_8)?,
                g: r.child(decode_8)?,
                k: r.child(decode_5)?,
                b: r.child(decode_41)?,
                n: r.child(decode_16)?,
                z: r.child(decode_43)?,
                field_: r.child(decode_45)?,
                i: r.child(decode_18)?,
                e: r.child(decode_47)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_41(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsBitem>>
    {
        let value = match r.tag()? {
            0 => r.sequence(decode_42)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_42(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsMappingAudioConstantsBitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsMappingAudioConstantsBitem {
                id: r.child(decode_8)?,
                content: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_43(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsZitem>>
    {
        let value = match r.tag()? {
            0 => r.sequence(decode_44)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_44(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsMappingAudioConstantsZitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsMappingAudioConstantsZitem {
                id: r.child(decode_8)?,
                content: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_45(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsItem>>
    {
        let value = match r.tag()? {
            0 => r.sequence(decode_46)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_46(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsMappingAudioConstantsItem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsMappingAudioConstantsItem {
                id: r.child(decode_8)?,
                content: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_47(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SoundPayloadProductsitemConfigsMappingAudioConstantsEitem>>
    {
        let value = match r.tag()? {
            0 => r.sequence(decode_48)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_48(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsMappingAudioConstantsEitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsMappingAudioConstantsEitem {
                content: r.child(decode_8)?,
                id: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_49(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SoundPayloadProductsitemConfigsMappingAudioOptionsitem>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_50)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_50(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsMappingAudioOptionsitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsMappingAudioOptionsitem {
                id: r.child(decode_8)?,
                content: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_51(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SoundPayloadProductsitemConfigsMappingAssignmentTableitem>>
    {
        let value = match r.tag()? {
            0 => r.sequence(decode_52)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_52(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsMappingAssignmentTableitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsMappingAssignmentTableitem {
                id: r.child(decode_8)?,
                content: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_53(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofile> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofile {
                guid: r.child(decode_8)?,
                name: r.child(decode_8)?,
                brightness: r.child(decode_54)?,
                switch_off_lighting: r.child(decode_55)?,
                quick_effects: r.child(decode_56)?,
                bass_boost: r.child(decode_57)?,
                sound_normalization: r.child(decode_58)?,
                voice_clarity: r.child(decode_59)?,
                mic_side_tone: r.child(decode_60)?,
                mic_volume_normalization: r.child(decode_61)?,
                mic_voice_clarity: r.child(decode_62)?,
                mic_sensitivity: r.child(decode_63)?,
                mic_equalizer: r.child(decode_64)?,
                mic_preview: r.child(decode_65)?,
                audio_equalizer: r.child(decode_66)?,
                ambient_noise_reduction: r.child(decode_67)?,
                thx_spatial_audio: r.child(decode_69)?,
                applications: r.child(decode_70)?,
                calibration: r.child(decode_74)?,
                mic_ambient_noise_reduction: None,
            },
            1 => SoundPayloadProductsitemConfigsProductDefaultprofile {
                guid: r.child(decode_8)?,
                name: r.child(decode_8)?,
                brightness: r.child(decode_54)?,
                switch_off_lighting: r.child(decode_55)?,
                quick_effects: r.child(decode_56)?,
                bass_boost: r.child(decode_57)?,
                sound_normalization: r.child(decode_58)?,
                voice_clarity: r.child(decode_59)?,
                mic_side_tone: r.child(decode_60)?,
                mic_volume_normalization: r.child(decode_61)?,
                mic_voice_clarity: r.child(decode_62)?,
                mic_sensitivity: r.child(decode_63)?,
                mic_equalizer: r.child(decode_64)?,
                mic_preview: r.child(decode_65)?,
                audio_equalizer: r.child(decode_66)?,
                ambient_noise_reduction: None,
                thx_spatial_audio: r.child(decode_69)?,
                applications: r.child(decode_70)?,
                calibration: r.child(decode_74)?,
                mic_ambient_noise_reduction: r.child(decode_98)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_54(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileBrightness> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileBrightness {
                is_enabled: r.child(decode_5)?,
                value: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_55(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileSwitchofflighting> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileSwitchofflighting {
                is_display_on: r.child(decode_5)?,
                is_idle_enabled: r.child(decode_5)?,
                idle_minutes: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_56(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileQuickeffects> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileQuickeffects {
                selected_effect_id: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_57(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileBassboost> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileBassboost {
                is_enabled: r.child(decode_5)?,
                value: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_58(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileSoundnormalization>
    {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileSoundnormalization {
                is_enabled: r.child(decode_5)?,
                value: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_59(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileVoiceclarity> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileVoiceclarity {
                is_enabled: r.child(decode_5)?,
                value: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_60(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileMicsidetone> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileMicsidetone {
                is_enabled: r.child(decode_5)?,
                value: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_61(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileMicvolumenormalization>
    {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileMicvolumenormalization {
                is_enabled: r.child(decode_5)?,
                value: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_62(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileMicvoiceclarity> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileMicvoiceclarity {
                is_enabled: r.child(decode_5)?,
                value: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_63(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileMicsensitivity> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileMicsensitivity {
                is_enabled: r.child(decode_5)?,
                value: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_64(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileMicequalizer> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileMicequalizer {
                mode: r.child(decode_8)?,
                custom_bands: r.child(decode_18)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_65(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileMicpreview> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileMicpreview {
                is_enabled: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_66(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileAudioequalizer> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileAudioequalizer {
                mode: r.child(decode_8)?,
                custom_bands: r.child(decode_18)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_67(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::option::Option<
            SoundPayloadProductsitemConfigsProductDefaultprofileAmbientnoisereduction,
        >,
    > {
        let value = match r.tag()? {
            0 => Some(r.child(decode_68)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_68(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileAmbientnoisereduction>
    {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileAmbientnoisereduction {
                is_enabled: r.child(decode_5)?,
                value: r.child(decode_17)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_69(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileThxspatialaudio> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileThxspatialaudio {
                is_enabled: r.child(decode_5)?,
                mode: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_70(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileApplications> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileApplications {
                general_input_setting: r.child(decode_71)?,
                specific_input_setting: r.child(decode_18)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_71(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::vec::Vec<
            SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitem,
        >,
    > {
        let value = match r.tag()? {
            0 => r.sequence(decode_72)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_72(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitem,
    > {
        let value = match r.tag()? {0 => {SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitem{app_type:r.child(decode_8)?,output:r.child(decode_73)?}}, _ => anyhow::bail!("Invalid typed catalog alternative")};
        r.finish()?;
        Ok(value)
    }
    fn decode_73(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitemOutput>{
        let value = match r.tag()? {0 => {SoundPayloadProductsitemConfigsProductDefaultprofileApplicationsGeneralinputsettingitemOutput{r#type:r.child(decode_8)?,guid:r.child(decode_8)?}}, _ => anyhow::bail!("Invalid typed catalog alternative")};
        r.finish()?;
        Ok(value)
    }
    fn decode_74(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileCalibration> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileCalibration {
                speakers: r.child(decode_75)?,
                speakers_overhead: r.child(decode_77)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_75(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::vec::Vec<SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersitem>,
    > {
        let value = match r.tag()? {
            0 => r.sequence(decode_76)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_76(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersitem>
    {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersitem {
                id: r.child(decode_8)?,
                angle: r.child(decode_16)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_77(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::vec::Vec<
            SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersoverheaditem,
        >,
    > {
        let value = match r.tag()? {
            0 => r.sequence(decode_78)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_78(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersoverheaditem,
    > {
        let value = match r.tag()? {0 => {SoundPayloadProductsitemConfigsProductDefaultprofileCalibrationSpeakersoverheaditem{id:r.child(decode_8)?,angle:r.child(decode_16)?}}, _ => anyhow::bail!("Invalid typed catalog alternative")};
        r.finish()?;
        Ok(value)
    }
    fn decode_79(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemCloudInitial> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemCloudInitial {
                selected_preset: r.child(decode_17)?,
                enabled_presets: r.child(decode_18)?,
                disabled_presets: r.child(decode_18)?,
                temp_enabled_presets: r.child(decode_18)?,
                temp_disabled_presets: r.child(decode_18)?,
                temp_active_preset: r.child(decode_80)?,
                actions_from_ui: r.child(decode_18)?,
                actions_from_local_storage: r.child(decode_18)?,
                error_actions: r.child(decode_18)?,
                available_esports_presets: r.child(decode_81)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_80(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<()> {
        let value = match r.tag()? {
            0 => (),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_81(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::vec::Vec<SoundPayloadProductsitemCloudInitialAvailableesportspresetsitem>,
    > {
        let value = match r.tag()? {
            0 => r.sequence(decode_82)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_82(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemCloudInitialAvailableesportspresetsitem> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemCloudInitialAvailableesportspresetsitem {
                equalizer_id: r.child(decode_17)?,
                equalizer_category: r.child(decode_17)?,
                equalizer_icon: r.child(decode_8)?,
                equalizer_svg_icon: r.child(decode_8)?,
                is_oobe: r.child(decode_5)?,
                is_oobe_enabled: r.child(decode_83)?,
                is_allow_swapping: r.child(decode_5)?,
                equalizer_name: r.child(decode_12)?,
                band_values: r.child(decode_21)?,
                footstep_values: r.child(decode_84)?,
                band_label_ids: r.child(decode_85)?,
            },
            1 => SoundPayloadProductsitemCloudInitialAvailableesportspresetsitem {
                equalizer_id: r.child(decode_17)?,
                equalizer_category: r.child(decode_17)?,
                equalizer_icon: r.child(decode_8)?,
                equalizer_svg_icon: r.child(decode_8)?,
                is_oobe: r.child(decode_5)?,
                is_oobe_enabled: None,
                is_allow_swapping: r.child(decode_5)?,
                equalizer_name: r.child(decode_12)?,
                band_values: r.child(decode_21)?,
                footstep_values: r.child(decode_84)?,
                band_label_ids: r.child(decode_85)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_83(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<bool>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_5)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_84(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<u64>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_17)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_85(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::vec::Vec<
            SoundPayloadProductsitemCloudInitialAvailableesportspresetsitemBandLabelIdsitem,
        >,
    > {
        let value = match r.tag()? {
            0 => r.sequence(decode_86)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_86(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        SoundPayloadProductsitemCloudInitialAvailableesportspresetsitemBandLabelIdsitem,
    > {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemCloudInitialAvailableesportspresetsitemBandLabelIdsitem {
                label_id: r.child(decode_16)?,
                band_index: r.child(decode_84)?,
                icon_start_index_position: r.child(decode_17)?,
                label_icon_position: r.child(decode_8)?,
                label_arrow_position: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_87(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_88, decode_89)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_88(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_89(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_88, decode_88)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_90(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemMedia> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemMedia {
                editions: r.child(decode_91)?,
                css: r.child(decode_12)?,
                reset_link: r.child(decode_8)?,
                eq_icons: r.child(decode_12)?,
                sword_added_icon: r.child(decode_8)?,
                search_empty: r.child(decode_8)?,
                icon_add: r.child(decode_8)?,
                placeholder_game_icon: r.child(decode_8)?,
                icon_close_enclosed_small: r.child(decode_8)?,
                icon_close_enclosed_a_small: r.child(decode_8)?,
                appmix_gamemode: r.child(decode_8)?,
                appmix_thx: r.child(decode_8)?,
                thx_help: r.child(decode_8)?,
                mapping_footstep: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_91(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_8, decode_12)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_92(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, Native>> {
        let value = match r.tag()? {
            0 => r.map(decode_88, decode_93)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_93(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Native> {
        let value = match r.tag()? {
            0 => Native {
                kind: r.child(decode_88)?,
                timer_tick: r.child(decode_94)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_94(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_95(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemInitials> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemInitials {
                spatial: r.child(decode_96)?,
                volume: r.child(decode_12)?,
                game_chat: r.child(decode_97)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_96(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemInitialsSpatial> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemInitialsSpatial {
                is_enabled: r.child(decode_5)?,
                mode: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_97(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemInitialsGameChat> {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemInitialsGameChat {
                balance_mode: r.child(decode_17)?,
                actions_from_ui: r.child(decode_18)?,
                actions_from_local_storage: r.child(decode_18)?,
                error_actions: r.child(decode_18)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_98(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::option::Option<
            SoundPayloadProductsitemConfigsProductDefaultprofileMicambientnoisereduction,
        >,
    > {
        let value = match r.tag()? {
            0 => Some(r.child(decode_99)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_99(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<SoundPayloadProductsitemConfigsProductDefaultprofileMicambientnoisereduction>
    {
        let value = match r.tag()? {
            0 => SoundPayloadProductsitemConfigsProductDefaultprofileMicambientnoisereduction {
                is_enabled: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_AUDIO_SOUND_DATA,
        0,
        "d9711f5c1fd69324c175418d9256baeac1d6811ae033603817dc968f6bb73bcb",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod audio_sound_data_catalog_tests {
    #[test]
    fn packaged_catalog_audio_sound_data_decodes() {
        let _ = super::read_audio_sound_catalog();
    }
}
// End generated catalog reader: razer-pages/audio_sound_data.

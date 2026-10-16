//! PID515/PID679 retained mapping publication after local profile persistence.
//! Source task failures never roll back the optimistic reducer draft.
use super::*;
use crate::features::mapping_device_runtime::{Owner, Reply};
use std::time::{Duration, Instant};

pub struct KeyboardLegacyProfileNavigation(pub Value);
pub struct KeyboardLegacyRuntimeStatus(pub String);
impl EventEmitter<KeyboardLegacyProfileNavigation> for KeyboardProductWorkspace {}
impl EventEmitter<KeyboardLegacyRuntimeStatus> for KeyboardProductWorkspace {}
impl EventEmitter<crate::features::MappingCallbackRequested> for KeyboardProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    container: Option<String>,
    owner: Option<Owner>,
    poll: Option<Task<()>>,
    epoch: u64,
    revision: u64,
    document: Option<Value>,
    error: Option<String>,
    published: Option<u64>,
    running_games: Option<usize>,
    linked_game_active: Option<bool>,
    held_brightness: BTreeMap<String, Instant>,
    window: Option<AnyWindowHandle>,
    latest_gaming_mode: Option<Value>,
    latest_connected: bool,
}
impl State {
    pub(super) fn new(window: &Window) -> Self {
        Self {
            window: Some(window.window_handle()),
            ..Default::default()
        }
    }
}
impl KeyboardProductWorkspace {
    pub(super) fn restore_latest_source_gaming_mode(&mut self) {
        self.legacy_mapping.latest_gaming_mode = None;
        if self.spec.product_id != 756 {
            return;
        }
        self.legacy_mapping.latest_gaming_mode = Some(self.draft["gamingMode"].clone());
        if let Some(dim) = self.draft.get("dimLighting").cloned() {
            self.draft["dimKeyboardLighting"] = dim;
        }
        self.draft["gamingMode"] =
            razer_device::keyboard_latest_mapping::features::effective_gaming_mode(
                &self.draft,
                self.draft["gameModeEnabled"].as_bool(),
            );
    }
    pub(super) fn latest_source_snapshot(&self, snapshot: &mut Value) {
        if self.spec.product_id == 756 {
            snapshot
                .as_object_mut()
                .unwrap()
                .remove("dimKeyboardLighting");
            if let Some(mode) = &self.legacy_mapping.latest_gaming_mode {
                snapshot["gamingMode"] = mode.clone();
            }
        }
    }
    pub(super) fn set_latest_custom_game_mode(&mut self, payload: &Value) {
        let mut gaming = self
            .legacy_mapping
            .latest_gaming_mode
            .clone()
            .unwrap_or_else(|| self.draft["gamingMode"].clone());
        gaming["customGameMode"] = payload.clone();
        self.legacy_mapping.latest_gaming_mode = Some(gaming);
        self.draft["gamingMode"]["customGameMode"] = payload.clone();
    }
    pub(super) fn set_latest_source_game_mode(
        &mut self,
        incoming: &Value,
        from_ui: bool,
        cx: &mut Context<Self>,
    ) {
        let mut profile = self.draft.clone();
        if let Some(saved) = &self.legacy_mapping.latest_gaming_mode {
            profile["gamingMode"] = saved.clone();
        }
        let guid = profile["guid"]
            .as_str()
            .unwrap_or("mounted-keyboard")
            .to_owned();
        profile["guid"] = json!(guid);
        let mut document = json!({"version":1,"activeProfile":guid,"gameModeEnabled":self.draft["gameModeEnabled"],"profiles":[profile]});
        match razer_device::keyboard_latest_mapping::features::set_global_gaming_mode(
            &mut document,
            incoming,
            from_ui,
        ) {
            Ok(effective) => {
                self.legacy_mapping.latest_gaming_mode =
                    Some(document["profiles"][0]["gamingMode"].clone());
                self.draft["gamingMode"] = effective;
                self.draft["gameModeEnabled"] = document["gameModeEnabled"].clone();
            }
            Err(error) => self.mapping_failure(format!("游戏模式本地状态：{error:#}"), cx),
        }
    }
    /// Current restore candidate observation. This updates only source-owned
    /// reducer fields; it does not remount the editor or acknowledge a setter.
    pub fn observe_latest_source_document(&mut self, document: &Value, cx: &mut Context<Self>) {
        if self.spec.product_id != 756 {
            return;
        }
        for &field in razer_device::keyboard_latest_mapping::features::DEVICE_FIELDS {
            if let Some(value) = document.get(field) {
                self.draft[field] = value.clone();
            }
        }
        if let Some(dim) = document.get("dimLighting") {
            self.draft["dimKeyboardLighting"] = dim.clone();
        }
        if let Ok(profile) = razer_device::keyboard_latest_mapping::features::profile(document) {
            if self.draft["guid"].is_null() || self.draft["guid"] == profile["guid"] {
                self.legacy_mapping.latest_gaming_mode = Some(profile["gamingMode"].clone());
                self.draft["gamingMode"] =
                    razer_device::keyboard_latest_mapping::features::effective_gaming_mode(
                        profile,
                        document["gameModeEnabled"].as_bool(),
                    );
                for field in [
                    "isEnableKeyboardKeys",
                    "isGamepadOverride",
                    "bottomDeadzone",
                    "interruptKey",
                    "singleKeySnapTap",
                ] {
                    if let Some(value) = profile.get(field) {
                        self.draft[field] = value.clone();
                    }
                }
                self.observe_latest_analog_controls(cx);
            }
        }
        cx.notify();
    }
    pub fn set_latest_mapping_connected(&mut self, connected: bool, cx: &mut Context<Self>) {
        if self.spec.product_id == 756 && self.legacy_mapping.latest_connected != connected {
            self.legacy_mapping.latest_connected = connected;
            if !connected {
                self.legacy_mapping.held_brightness.clear();
            }
            cx.notify();
        }
    }
    /// Source gaming reducer's transient GMS action. Mapping publication uses
    /// the separately observed complete running-game count, not this UI flag.
    pub fn observe_linked_game_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.legacy_mapping.linked_game_active = Some(active);
        if self.draft["gamingMode"]["isGameOn"].as_bool() == Some(active) {
            return;
        }
        if let Some(mode) = self.draft["gamingMode"].as_object_mut() {
            mode.insert("isGameOn".into(), json!(active));
            cx.notify();
        }
    }

    pub(super) fn restore_linked_game_active(&mut self, cx: &mut Context<Self>) {
        if let Some(active) = self.legacy_mapping.linked_game_active {
            self.observe_linked_game_active(active, cx);
        }
    }

    /// Actual host observation, retained independently of the USB owner. This
    /// republishes temporary engine/LED state without saving a new local draft.
    pub fn observe_running_games(&mut self, count: Option<usize>, cx: &mut Context<Self>) {
        if self.legacy_mapping.running_games == count {
            return;
        }
        self.legacy_mapping.running_games = count;
        if self.legacy_mapping.owner.is_some() {
            self.submit_legacy_mapping(cx);
        }
    }

    pub fn set_legacy_mapping_connection(
        &mut self,
        container: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.spec.product_id, 515 | 679) || self.legacy_mapping.container == container
        {
            return;
        }
        self.legacy_mapping.owner = None;
        self.legacy_mapping.poll = None;
        self.legacy_mapping.epoch = self.legacy_mapping.epoch.wrapping_add(1);
        self.legacy_mapping.container = container;
        self.legacy_mapping.published = None;
        self.legacy_mapping.error = None;
        self.legacy_mapping.held_brightness.clear();
        if self.legacy_mapping.container.is_some() && self.legacy_mapping.document.is_some() {
            self.start_legacy_mapping_owner(cx);
        }
    }

    pub fn publish_legacy_mapping(
        &mut self,
        container: String,
        document: Value,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.spec.product_id, 515 | 679) {
            return;
        }
        self.set_legacy_mapping_connection(Some(container), cx);
        if self.legacy_mapping.document.as_ref() == Some(&document)
            && self.legacy_mapping.owner.is_some()
        {
            return;
        }
        self.legacy_mapping.document = Some(document);
        if self.legacy_mapping.owner.is_none() {
            self.start_legacy_mapping_owner(cx);
        } else {
            self.submit_legacy_mapping(cx);
        }
    }

    fn mapping_failure(&mut self, error: String, cx: &mut Context<Self>) {
        self.legacy_mapping.error = Some(error.clone());
        cx.emit(KeyboardLegacyRuntimeStatus(format!(
            "键盘本地映射未确认：{error}"
        )));
        cx.notify();
    }
    fn start_legacy_mapping_owner(&mut self, cx: &mut Context<Self>) {
        let Some(container) = self.legacy_mapping.container.clone() else {
            return;
        };
        let (owner, replies) = match Owner::start(self.spec.product_id, container) {
            Ok(value) => value,
            Err(error) => {
                self.mapping_failure(format!("{error:#}"), cx);
                return;
            }
        };
        self.legacy_mapping.owner = Some(owner);
        let epoch = self.legacy_mapping.epoch;
        self.legacy_mapping.poll = Some(cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                let messages = replies.try_iter().collect::<Vec<_>>();
                let keep_running = view
                    .update(cx, |this, cx| {
                        if this.legacy_mapping.epoch != epoch {
                            return false;
                        }
                        let mut running = true;
                        for reply in messages {
                            match reply {
                                Reply::Ready => this.submit_legacy_mapping(cx),
                                Reply::Published {
                                    revision,
                                    result,
                                    indicator,
                                } => {
                                    if revision != this.legacy_mapping.revision {
                                        continue;
                                    }
                                    match result {
                                        Ok(()) => {
                                            this.legacy_mapping.published = Some(revision);
                                            this.legacy_mapping.error = None;
                                        }
                                        Err(error) => this.mapping_failure(error, cx),
                                    }
                                    if let Some(Err(error)) = indicator {
                                        this.mapping_failure(
                                            format!("游戏模式指示灯：{error}"),
                                            cx,
                                        );
                                    }
                                }
                                Reply::Events(events) => {
                                    for event in events {
                                        this.legacy_mapping_event(event, cx);
                                    }
                                }
                                Reply::Failed(error) => this.mapping_failure(error, cx),
                                Reply::Stopped(result) => {
                                    this.legacy_mapping.owner = None;
                                    this.legacy_mapping.held_brightness.clear();
                                    if let Err(error) = result {
                                        this.mapping_failure(error, cx);
                                    }
                                    running = false;
                                }
                            }
                        }
                        let now = Instant::now();
                        let repeating = this
                            .legacy_mapping
                            .held_brightness
                            .iter()
                            .filter(|(_, next)| **next <= now)
                            .map(|(name, _)| name.clone())
                            .collect::<Vec<_>>();
                        for name in repeating {
                            this.legacy_mapping
                                .held_brightness
                                .insert(name.clone(), now + Duration::from_millis(110));
                            this.legacy_brightness_action(&name, cx);
                        }
                        running
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        }));
    }

    fn submit_legacy_mapping(&mut self, cx: &mut Context<Self>) {
        let Some(document) = self.legacy_mapping.document.as_ref() else {
            return;
        };
        let mut context = razer_device::mapping_engine::EngineContext {
            default_turbos: document["defaultTurbos"].as_array().cloned(),
            synapse_turbos: document["synapseTurbos"].as_array().cloned(),
            fresh_guids: (0..256).map(|_| uuid::Uuid::new_v4().to_string()).collect(),
            ..Default::default()
        };
        let compiled = if self.spec.product_id == 679 {
            razer_device::keyboard_current_mapping::compile(
                document,
                self.legacy_mapping.running_games,
                (0..256).map(|_| uuid::Uuid::new_v4().to_string()),
            )
        } else {
            razer_device::keyboard_legacy_mapping::compile(
                document,
                &razer_device::keyboard_legacy_mapping::source_defaults(),
                &mut context,
                self.legacy_mapping.running_games,
            )
        };
        let document = match compiled {
            Ok(document) => document,
            Err(error) => {
                self.mapping_failure(format!("{error:#}"), cx);
                return;
            }
        };
        let indicator = match razer_device::keyboard_gaming_mapping::indicator_enabled(
            &document,
            self.spec.product_id,
            self.legacy_mapping.running_games,
        ) {
            Ok(indicator) => indicator,
            Err(error) => {
                self.mapping_failure(format!("{error:#}"), cx);
                return;
            }
        };
        self.legacy_mapping.revision = self.legacy_mapping.revision.wrapping_add(1);
        if let Some(owner) = self.legacy_mapping.owner.as_ref() {
            if let Err(error) =
                owner.publish(self.legacy_mapping.revision, document, Some(indicator))
            {
                self.mapping_failure(error, cx);
            }
        }
    }

    fn legacy_mapping_event(&mut self, event: Value, cx: &mut Context<Self>) {
        let event = &event["jsonEvent"];
        if event["type"] == "error" {
            self.mapping_failure(event.to_string(), cx);
            return;
        }
        if event["type"] != "unsupportedmapping" {
            return;
        }
        let Some(output) = event["output"]
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
        else {
            self.mapping_failure("映射引擎返回的按键动作不是有效 JSON".into(), cx);
            return;
        };
        match output["type"].as_str() {
            Some("snapTap") => self.apply_device_snap_toggle(&output, cx),
            Some("interdevice" | "lightpac") => cx.emit(
                crate::features::MappingCallbackRequested::new(event.clone()),
            ),
            Some("gameMode") => {
                let mut settings = self.draft["gamingMode"].clone();
                if settings["state"] == 2 || settings["enableInGame"] == true {
                    return;
                }
                let enabled = settings["state"] == 0;
                settings["state"] = json!(u8::from(enabled));
                settings["isWindowsKeyDisabled"] = json!(enabled);
                if self.spec.product_id == 756 {
                    self.set_latest_source_game_mode(&settings, false, cx);
                } else {
                    self.draft["gamingMode"] = settings;
                }
                cx.emit(KeyboardProductChanged);
                cx.notify();
            }
            Some("backlight") => {
                let Some(name) = output["name"].as_str() else {
                    return;
                };
                if output["flag"] == 1 {
                    self.legacy_mapping.held_brightness.remove(name);
                    if name == "BrightnessToggle" {
                        self.legacy_brightness_action(name, cx);
                    }
                } else if output["flag"] == 0 && name != "BrightnessToggle" {
                    self.legacy_brightness_action(name, cx);
                    self.legacy_mapping
                        .held_brightness
                        .insert(name.into(), Instant::now() + Duration::from_millis(800));
                }
            }
            Some("switchProfile" | "navigateProfile") => {
                cx.emit(KeyboardLegacyProfileNavigation(output))
            }
            _ => {}
        }
    }
    /// Actual latest756 native events use the same source reducer handlers,
    /// while their registration/queue/lifetime remains owned by the shell.
    pub fn observe_latest_mapping_event(&mut self, event: Value, cx: &mut Context<Self>) {
        if self.spec.product_id == 756 {
            self.legacy_mapping_event(event, cx);
        }
    }
    pub fn consume_interdevice_mapping(&mut self, mut raw: Value, cx: &mut Context<Self>) -> bool {
        if !(matches!(self.spec.product_id, 515 | 679) && self.legacy_mapping.owner.is_some()
            || self.spec.product_id == 756 && self.legacy_mapping.latest_connected)
        {
            return false;
        }
        raw["type"] = json!("unsupportedmapping");
        self.legacy_mapping_event(json!({"jsonEvent":raw}), cx);
        true
    }

    fn legacy_brightness_action(&mut self, name: &str, cx: &mut Context<Self>) {
        let mut brightness = self.draft["brightness"].clone();
        let value = brightness["value"].as_u64().unwrap_or(100).min(100);
        if name == "BrightnessToggle" {
            brightness["isEnabled"] = json!(brightness["isEnabled"] != true);
        } else {
            if (name == "BrightnessUp" && value == 100) || (name == "BrightnessDown" && value == 0)
            {
                return;
            }
            let value = match name {
                "BrightnessUp" => (value + 5).min(100),
                "BrightnessDown" => value.saturating_sub(5),
                _ => return,
            };
            brightness["value"] = json!(value);
            brightness["isEnabled"] = json!(value > 0);
        }
        let value = brightness["value"].as_u64().unwrap_or(100) as f32;
        self.draft["brightness"] = brightness;
        cx.emit(KeyboardProductChanged);
        cx.notify();
        if let Some(handle) = self.legacy_mapping.window {
            let owner = cx.entity().downgrade();
            cx.defer(move |cx| {
                let _ = cx.update_window(handle, |_, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        this.syncing = true;
                        if let Some(slider) = this.sliders.get("/brightness/value") {
                            slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
                        }
                        this.syncing = false;
                    });
                });
            });
        }
        self.request_brightness(cx);
    }
}

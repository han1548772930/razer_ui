//! Current 1422/1446 UI5235 `xe` and main `ni` reducer. UI state follows
//! source optimistic actions; middleware cache observations remain distinct
//! from native device acknowledgements. See audio-MapAudio-audit.json.
use super::*;
use crate::features::source_key::SourceKey;
use gpui_kit::component::{
    IndexPath,
    input::{InputEvent, InputState},
    select::SelectItem,
};
use razer_device::seiren_stream_mixer::Action;
use std::{
    cell::Cell,
    collections::BTreeSet,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

#[path = "audio_seiren_stream_mixer_shortcut.rs"]
mod shortcut;
#[path = "audio_seiren_stream_mixer_view.rs"]
mod view;

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
struct MixerChoice {
    id: String,
    title: SharedString,
    disabled: bool,
}
impl MixerChoice {
    fn new(id: impl Into<String>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            disabled: false,
        }
    }
}
impl SelectItem for MixerChoice {
    type Value = String;
    fn value(&self) -> &String {
        &self.id
    }
    fn title(&self) -> SharedString {
        self.title.clone()
    }
    fn disabled(&self) -> bool {
        self.disabled
    }
}

#[derive(Clone)]
pub struct SeirenStreamMixerRequested {
    owner: EntityId,
    product_id: u32,
    generation: u64,
    sequence: u64,
    action: Action,
    valid: Arc<AtomicBool>,
}
impl SeirenStreamMixerRequested {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn action(&self) -> &Action {
        &self.action
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
}

#[derive(Clone)]
pub struct SeirenStreamMixerCaptureChanged {
    owner: EntityId,
    product_id: u32,
    generation: u64,
    capturing: bool,
    valid: Arc<AtomicBool>,
}
impl SeirenStreamMixerCaptureChanged {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn capturing(&self) -> bool {
        self.capturing
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
    fn matches_owner(
        &self,
        product_id: u32,
        owner: EntityId,
        generation: u64,
        capturing: bool,
        guard: &Arc<AtomicBool>,
    ) -> bool {
        self.product_id == product_id
            && self.owner == owner
            && self.generation == generation
            && self.capturing == capturing
            && self.is_valid()
            && Arc::ptr_eq(&self.valid, guard)
    }
}

pub enum SeirenStreamMixerObservation {
    Cache(Value),
    ActiveMixer(bool),
    Devices {
        playback: Vec<String>,
        record: Vec<String>,
    },
    LevelMeter(Value),
    Error(String),
}
impl EventEmitter<SeirenStreamMixerRequested> for AudioProductWorkspace {}
impl EventEmitter<SeirenStreamMixerCaptureChanged> for AudioProductWorkspace {}

pub(super) struct State {
    stopped: bool,
    owner: EntityId,
    data: Value,
    generation: u64,
    sequence: u64,
    valid: Arc<AtomicBool>,
    pending: BTreeSet<u64>,
    error: Option<String>,
    presets: Entity<SelectState<Vec<MixerChoice>>>,
    playback: Entity<SelectState<Vec<MixerChoice>>>,
    external: BTreeMap<String, Entity<SelectState<Vec<MixerChoice>>>>,
    faders: BTreeMap<String, Entity<SliderState>>,
    syncing: bool,
    rename: Entity<InputState>,
    renaming: bool,
    reset_open: bool,
    reset_timer: Option<Task<()>>,
    add_open: bool,
    add_bounds: Rc<Cell<Bounds<Pixels>>>,
    warning_open: bool,
    capture: FocusHandle,
    capturing: bool,
    capture_ready: bool,
    capture_timer: Option<Task<()>>,
    capture_hypershift: bool,
    capture_guard: Arc<AtomicBool>,
    capture_generation: u64,
    capture_keys: shortcut::CaptureKeys,
    raw_capture: Option<razer_platform::keyboard_capture::KeyboardCapture>,
    raw_capture_task: Option<Task<()>>,
    shortcut_warning: bool,
    shortcut_warning_value: Option<String>,
    drag_channel: Option<String>,
    drag_over: Option<String>,
    linked_gesture: Option<(String, String, f64, f64)>,
    last_fader_emit: BTreeMap<String, std::time::Instant>,
}
impl Drop for State {
    fn drop(&mut self) {
        self.valid.store(false, Ordering::Release);
        self.capture_guard.store(false, Ordering::Release);
    }
}

fn supported(pid: u32) -> bool {
    matches!(pid, 1422 | 1446)
}
fn channels(data: &Value) -> &[Value] {
    data["inputChannels"].as_array().map_or(&[], Vec::as_slice)
}
fn presets(data: &Value) -> &[Value] {
    data["presets"].as_array().map_or(&[], Vec::as_slice)
}
fn selected(data: &Value) -> &str {
    data["selectedPresetGuid"].as_str().unwrap_or("")
}
fn enabled(data: &Value) -> bool {
    data["isStreamMixerEnabled"] == true
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recipe {
    native_device_ids: BTreeMap<String, u64>,
    name_to_device_id: BTreeMap<String, String>,
    device_name: String,
    capture_keys: Vec<SourceKey>,
}

fn recipe(pid: u32) -> &'static Recipe {
    static RECIPES: OnceLock<BTreeMap<String, Recipe>> = OnceLock::new();
    &RECIPES.get_or_init(|| read_seiren_stream_mixer_catalog())[&pid.to_string()]
}
fn source_initial() -> Value {
    json!({"levelMeter":[],"waitingForLevelMeter":false,"inputChannels":[],
        "exterInputChannels":[],"isStreamMixerEnabled":false,"streamMixerEnablePendingMwAck":false,
        "isStreamMixMonitor":true,"streamMixVolume":{"isEnabled":false,"value":0},
        "playbackMixVolume":{"isEnabled":false,"value":0},"playbackMixDevice":"",
        "shortcutMapping":{},"playbackDevices":[],"recordDevices":[],"isDeviceReady":true,
        "activeStreamMixerStatus":false,"presets":[{"guid":"6633f08d-8467-4fad-b854-635acf9c2315","name":"Preset"}],
        "selectedPresetGuid":"6633f08d-8467-4fad-b854-635acf9c2315","microphoneMonitorValue":0})
}
fn merge(target: &mut Value, incoming: &Value) {
    if let (Some(target), Some(incoming)) = (target.as_object_mut(), incoming.as_object()) {
        for (key, value) in incoming {
            target.insert(key.clone(), value.clone());
        }
    }
}
fn sort_presets(data: &mut Value) {
    if let Some(items) = data["presets"].as_array_mut() {
        items.sort_by_key(|item| item["name"].as_str().unwrap_or("").to_uppercase());
    }
}

impl AudioProductWorkspace {
    pub(super) fn initialize_seiren_stream_mixer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !supported(self.spec.product_id) {
            return;
        }
        let preset_select =
            cx.new(|cx| SelectState::new(Vec::<MixerChoice>::new(), None, window, cx));
        let playback = cx.new(|cx| SelectState::new(Vec::<MixerChoice>::new(), None, window, cx));
        let rename = cx.new(|cx| {
            InputState::new(window, cx).validate(|value, _| value.encode_utf16().count() <= 32)
        });
        let capture = cx.focus_handle().tab_stop(true);
        self.subscriptions
            .push(cx.on_focus_out(&capture, window, |this, _, _, cx| {
                this.stop_seiren_stream_mixer_capture(cx)
            }));
        self.subscriptions
            .push(cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.stop_seiren_stream_mixer_capture(cx);
                }
            }));
        self.subscriptions.push(cx.subscribe_in(
            &preset_select,
            window,
            |this, _, event, window, cx| {
                if this.seiren_stream.as_ref().is_none_or(|s| s.syncing) {
                    return;
                }
                if let SelectEvent::Confirm(Some(id)) = event {
                    this.select_seiren_stream_mixer_preset(id.as_str(), window, cx);
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &playback,
            window,
            |this, _, event, window, cx| {
                if this.seiren_stream.as_ref().is_none_or(|s| s.syncing) {
                    return;
                }
                if let SelectEvent::Confirm(Some(name)) = event {
                    this.submit_seiren_stream_mixer(
                        "ON_CHANGE_PLAY_BACK_MIX_DEVICE",
                        json!({"changePlayBackMixDevice":name.as_str()}),
                        window,
                        cx,
                    );
                }
            },
        ));
        self.subscriptions.push(cx.subscribe_in(
            &rename,
            window,
            |this, input, event, window, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                    let value = input.read(cx).value().trim().to_owned();
                    this.rename_seiren_stream_mixer(&value, window, cx);
                }
            },
        ));
        self.seiren_stream = Some(State {
            stopped: false,
            owner: cx.entity_id(),
            data: source_initial(),
            generation: NEXT_GENERATION.fetch_add(1, Ordering::Relaxed),
            sequence: 0,
            valid: Arc::new(AtomicBool::new(true)),
            pending: BTreeSet::new(),
            error: None,
            presets: preset_select,
            playback,
            external: BTreeMap::new(),
            faders: BTreeMap::new(),
            syncing: false,
            rename,
            renaming: false,
            reset_open: false,
            reset_timer: None,
            add_open: false,
            add_bounds: Rc::new(Cell::new(Bounds::default())),
            warning_open: false,
            capture,
            capturing: false,
            capture_ready: false,
            capture_timer: None,
            capture_hypershift: false,
            capture_guard: Arc::new(AtomicBool::new(true)),
            capture_generation: 0,
            shortcut_warning: false,
            shortcut_warning_value: None,
            capture_keys: shortcut::CaptureKeys::default(),
            raw_capture: None,
            raw_capture_task: None,
            drag_channel: None,
            drag_over: None,
            linked_gesture: None,
            last_fader_emit: BTreeMap::new(),
        });
        self.sync_seiren_stream_mixer(window, cx);
    }

    pub fn seiren_stream_mixer_request_matches(
        &self,
        request: &SeirenStreamMixerRequested,
    ) -> bool {
        request.product_id == self.spec.product_id
            && request.is_valid()
            && self.seiren_stream.as_ref().is_some_and(|state| {
                state.owner == request.owner
                    && state.generation == request.generation
                    && state.pending.contains(&request.sequence)
                    && Arc::ptr_eq(&state.valid, &request.valid)
            })
    }
    pub fn seiren_stream_mixer_capture_matches(
        &self,
        request: &SeirenStreamMixerCaptureChanged,
    ) -> bool {
        self.seiren_stream.as_ref().is_some_and(|state| {
            request.matches_owner(
                self.spec.product_id,
                state.owner,
                state.capture_generation,
                state.capture_ready,
                &state.capture_guard,
            )
        })
    }
    pub(super) fn dismiss_seiren_stream_mixer(&mut self, cx: &mut Context<Self>) {
        self.stop_seiren_stream_mixer_capture(cx);
        if let Some(state) = &mut self.seiren_stream {
            state.add_open = false;
            state.reset_open = false;
            state.reset_timer = None;
            state.warning_open = false;
            state.drag_channel = None;
            state.drag_over = None;
            state.capture_keys = shortcut::CaptureKeys::default();
            state.capture_hypershift = false;
        }
    }
    pub fn finish_seiren_stream_mixer(
        &mut self,
        request: &SeirenStreamMixerRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !self.seiren_stream_mixer_request_matches(request) {
            return;
        }
        let state = self.seiren_stream.as_mut().expect("matching mixer owner");
        state.pending.remove(&request.sequence);
        state.error = error;
        cx.notify();
    }
    pub fn cancel_seiren_stream_mixer_connection(&mut self) {
        if let Some(state) = &mut self.seiren_stream {
            state.valid.store(false, Ordering::Release);
            state.capture_guard.store(false, Ordering::Release);
            state.generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
            state.valid = Arc::new(AtomicBool::new(true));
            state.capture_guard = Arc::new(AtomicBool::new(true));
            state.pending.clear();
            state.error = None;
            state.capturing = false;
            state.capture_ready = false;
            state.capture_hypershift = false;
            state.capture_timer = None;
            state.raw_capture = None;
            state.raw_capture_task = None;
            state.capture_keys = shortcut::CaptureKeys::default();
            state.add_open = false;
            state.reset_open = false;
            state.reset_timer = None;
            state.linked_gesture = None;
            state.last_fader_emit.clear();
            state.data["levelMeter"] = json!([]);
        }
    }
    pub(super) fn stop_seiren_stream_mixer_admission(&mut self) {
        if let Some(state) = &mut self.seiren_stream {
            state.stopped = true;
        }
    }
    pub(super) fn resume_seiren_stream_mixer_admission(&mut self) {
        if let Some(state) = &mut self.seiren_stream {
            state.stopped = false;
        }
    }
    pub(super) fn seiren_stream_mixer_pending(&self) -> bool {
        self.seiren_stream
            .as_ref()
            .is_some_and(|state| !state.pending.is_empty())
    }
    pub fn observe_seiren_stream_mixer(
        &mut self,
        observation: SeirenStreamMixerObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.record_seiren_stream_mixer(observation, cx);
        self.sync_seiren_stream_mixer(window, cx);
    }
    pub fn record_seiren_stream_mixer(
        &mut self,
        observation: SeirenStreamMixerObservation,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        match observation {
            SeirenStreamMixerObservation::Cache(mut incoming) => {
                preserve_channel_order(&state.data, &mut incoming);
                if let Some(active) = presets(&incoming)
                    .iter()
                    .find(|preset| preset["guid"] == incoming["selectedPresetGuid"])
                {
                    if let Some(mapping) = active.get("shortcutMapping") {
                        incoming["shortcutMapping"] = mapping.clone();
                    }
                }
                merge(&mut state.data, &incoming);
                state.data["streamMixerEnablePendingMwAck"] = json!(false);
            }
            SeirenStreamMixerObservation::ActiveMixer(active) => {
                state.data["activeStreamMixerStatus"] = json!(active)
            }
            SeirenStreamMixerObservation::Devices { playback, record } => {
                state.data["playbackDevices"] = json!(
                    playback
                        .into_iter()
                        .map(|name| json!({"name":name}))
                        .collect::<Vec<_>>()
                );
                state.data["recordDevices"] = json!(
                    record
                        .into_iter()
                        .map(|name| json!({"name":name}))
                        .collect::<Vec<_>>()
                );
            }
            SeirenStreamMixerObservation::LevelMeter(levels) => {
                state.data["levelMeter"] = levels;
                cx.notify();
                return;
            }
            SeirenStreamMixerObservation::Error(error) => {
                state.error = Some(error);
                cx.notify();
                return;
            }
        }
        cx.notify();
    }
    pub fn observe_seiren_stream_mixer_mode(&mut self, hypershift: bool, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.seiren_stream {
            if state.capture_ready {
                state.capture_hypershift = hypershift;
            }
        }
        cx.notify();
    }

    fn submit_seiren_stream_mixer(
        &mut self,
        event: &str,
        payload: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        if state.stopped {
            return;
        }
        let action = match Action::new(event, payload.clone()) {
            Ok(action) => action,
            Err(error) => {
                state.error = Some(format!("{error:#}"));
                cx.notify();
                return;
            }
        };
        optimistic(&mut state.data, event, &payload);
        state.sequence = state.sequence.wrapping_add(1);
        state.pending.insert(state.sequence);
        let request = SeirenStreamMixerRequested {
            owner: cx.entity_id(),
            product_id: self.spec.product_id,
            generation: state.generation,
            sequence: state.sequence,
            action,
            valid: state.valid.clone(),
        };
        state.error = None;
        let sync_choices = matches!(
            event,
            "ON_SELECTED_STREAM_MIXER_PRESET"
                | "ON_ADD_STREAM_MIXER_PRESET"
                | "ON_DUPLICATE_STREAM_MIXER_PRESET"
                | "ON_RENAME_STREAM_MIXER_PRESET"
                | "ON_DELETE_STREAM_MIXER_PRESET"
                | "ON_CHANGE_PLAY_BACK_MIX_DEVICE"
        ) || event == "ON_MODIFY_INPUT_CHANNEL"
            && (payload["action"] != "edit"
                || payload["modifyInputChannel"]
                    .get("exterInputDevice")
                    .is_some());
        self.sync_seiren_stream_mixer_controls(sync_choices, window, cx);
        cx.emit(request);
        cx.notify();
    }

    pub(super) fn sync_seiren_stream_mixer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_seiren_stream_mixer_controls(true, window, cx);
    }
    fn sync_seiren_stream_mixer_controls(
        &mut self,
        sync_choices: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        state.syncing = true;
        if sync_choices {
            let rows = presets(&state.data)
                .iter()
                .map(|preset| {
                    MixerChoice::new(
                        preset["guid"].as_str().unwrap_or(""),
                        preset["name"].as_str().unwrap_or(""),
                    )
                })
                .collect::<Vec<_>>();
            let index = rows
                .iter()
                .position(|row| row.value().as_str() == selected(&state.data))
                .unwrap_or(0);
            state.presets.update(cx, |select, cx| {
                select.set_items(rows, window, cx);
                select.set_selected_index(Some(IndexPath::new(index)), window, cx);
            });
            let rows = state.data["playbackDevices"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item["name"].as_str())
                .map(|name| MixerChoice::new(name, name))
                .collect::<Vec<_>>();
            let playback_index = rows.iter().position(|row| {
                state.data["playbackMixDevice"].as_str() == Some(row.value().as_str())
            });
            state.playback.update(cx, |select, cx| {
                select.set_items(rows, window, cx);
                select.set_selected_index(playback_index.map(IndexPath::new), window, cx);
            });
        }
        let mut values = vec![
            (
                "output/streamMixVolume".to_owned(),
                state.data["streamMixVolume"]["value"]
                    .as_f64()
                    .unwrap_or(0.),
            ),
            (
                "output/playbackMixVolume".to_owned(),
                state.data["playbackMixVolume"]["value"]
                    .as_f64()
                    .unwrap_or(0.),
            ),
        ];
        let inputs = channels(&state.data).to_vec();
        for channel in &inputs {
            let Some(name) = channel["name"].as_str() else {
                continue;
            };
            for field in ["streamMixerInputVolume", "playbackMixInputVolume"] {
                values.push((
                    format!("{name}/{field}"),
                    channel[field]["value"].as_f64().unwrap_or(0.),
                ));
            }
            if !sync_choices || channel["inputChannelType"] != "configurable" {
                continue;
            }
            if !state.external.contains_key(name) {
                let select =
                    cx.new(|cx| SelectState::new(Vec::<MixerChoice>::new(), None, window, cx));
                let channel_name = name.to_owned();
                self.subscriptions.push(cx.subscribe_in(
                    &select,
                    window,
                    move |this, _, event, window, cx| {
                        if this
                            .seiren_stream
                            .as_ref()
                            .is_none_or(|state| state.syncing)
                        {
                            return;
                        }
                        if let SelectEvent::Confirm(Some(device)) = event {
                            this.change_seiren_external_device(
                                &channel_name,
                                device.as_str(),
                                window,
                                cx,
                            );
                        }
                    },
                ));
                state.external.insert(name.to_owned(), select);
            }
            let current = channel["exterInputDevice"].as_str().unwrap_or("");
            let mut rows = state.data["recordDevices"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item["name"].as_str())
                .map(|name| MixerChoice::new(name, name))
                .collect::<Vec<_>>();
            if !current.is_empty() && !rows.iter().any(|row| row.value().as_str() == current) {
                rows.push(MixerChoice {
                    id: current.into(),
                    title: current.into(),
                    disabled: true,
                });
            }
            let index = rows.iter().position(|row| row.value().as_str() == current);
            state.external[name].update(cx, |select, cx| {
                select.set_items(rows, window, cx);
                select.set_selected_index(index.map(IndexPath::new), window, cx);
            });
        }
        for (key, value) in values {
            if let Some(slider) = state.faders.get(&key) {
                slider.update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
            } else {
                let slider = cx.new(|_| {
                    SliderState::new()
                        .min(0.)
                        .max(100.)
                        .step(1.)
                        .default_value(value as f32)
                });
                let path = key.clone();
                self.subscriptions.push(cx.subscribe_in(
                    &slider,
                    window,
                    move |this, _, event, window, cx| {
                        if this
                            .seiren_stream
                            .as_ref()
                            .is_none_or(|state| state.syncing)
                        {
                            return;
                        }
                        match event {
                            SliderEvent::Change(value) => this.move_seiren_stream_mixer_fader(
                                &path,
                                f64::from(value.start()),
                                window,
                                cx,
                            ),
                            SliderEvent::Release(_) => {
                                if let Some(state) = &mut this.seiren_stream {
                                    state.linked_gesture = None;
                                    state.last_fader_emit.remove(&path);
                                }
                            }
                        }
                    },
                ));
                state.faders.insert(key, slider);
            }
        }
        state.syncing = false;
    }

    fn select_seiren_stream_mixer_preset(
        &mut self,
        guid: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.seiren_stream.as_ref().is_some_and(|state| {
            selected(&state.data) != guid
                && presets(&state.data)
                    .iter()
                    .any(|preset| preset["guid"] == guid)
        }) {
            self.stop_seiren_stream_mixer_capture(cx);
            self.submit_seiren_stream_mixer(
                "ON_SELECTED_STREAM_MIXER_PRESET",
                json!({"selectedPresetGuid":guid}),
                window,
                cx,
            );
        }
    }
    fn rename_seiren_stream_mixer(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        if !state.renaming {
            return;
        }
        state.renaming = false;
        let valid = !name.is_empty()
            && name.encode_utf16().count() <= 32
            && !presets(&state.data)
                .iter()
                .any(|preset| preset["name"] == name);
        if valid {
            let guid = selected(&state.data).to_owned();
            self.submit_seiren_stream_mixer(
                "ON_RENAME_STREAM_MIXER_PRESET",
                json!({"newName":name,"selectedPresetGuid":guid}),
                window,
                cx,
            );
        } else {
            cx.notify();
        }
    }
    fn change_seiren_external_device(
        &mut self,
        name: &str,
        device: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(channel) = self
            .seiren_stream
            .as_ref()
            .and_then(|state| {
                channels(&state.data)
                    .iter()
                    .find(|channel| channel["name"] == name)
            })
            .cloned()
        else {
            return;
        };
        if channel["exterInputDevice"] == device {
            return;
        }
        self.submit_seiren_stream_mixer("ON_MODIFY_INPUT_CHANNEL",json!({"action":"edit","modifyInputChannel":{"name":name,"inputChannelType":channel["inputChannelType"],"exterInputDevice":device}}),window,cx);
    }

    pub(super) fn observe_seiren_stream_mixer_native(
        &mut self,
        event: &Value,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        if event["eventType"] != "levelMeterPumpup" {
            return;
        }
        let Some(entries) = event
            .pointer("/levelMeters/meterInfo")
            .and_then(Value::as_array)
        else {
            return;
        };
        let ids = &recipe(self.spec.product_id).native_device_ids;
        let levels = entries
            .iter()
            .filter_map(|entry| {
                let id = entry["deviceId"].as_u64().filter(|id| *id != 0)?;
                let name = ids.iter().find(|(_, value)| **value == id)?.0;
                let mut observed = entry.clone();
                observed["deviceIdEnum"] = json!(name);
                Some(observed)
            })
            .collect::<Vec<_>>();
        state.data["levelMeter"] = json!(levels);
        cx.notify();
    }
    pub(super) fn clear_seiren_stream_mixer_meters(&mut self) {
        if let Some(state) = &mut self.seiren_stream {
            state.data["levelMeter"] = Value::Null;
        }
    }
    fn set_seiren_stream_mixer_input(
        &mut self,
        name: &str,
        field: &str,
        mut value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &self.seiren_stream else {
            return;
        };
        if !enabled(&state.data) {
            return;
        }
        let Some(channel) = channels(&state.data)
            .iter()
            .find(|channel| channel["name"] == name)
        else {
            return;
        };
        if field != "isLinked" {
            value["isToggle"] = json!(channel[field]["isEnabled"] != value["isEnabled"]);
        }
        self.submit_seiren_stream_mixer(
            "ON_MODIFY_INPUT_CHANNEL",
            json!({"action":"edit","modifyInputChannel":{"name":name,field:value}}),
            window,
            cx,
        );
    }
    fn move_seiren_stream_mixer_fader(
        &mut self,
        key: &str,
        value: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((name, field)) = key.split_once('/') else {
            return;
        };
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        if !enabled(&state.data) {
            self.sync_seiren_stream_mixer_controls(false, window, cx);
            return;
        }
        let current = if name == "output" {
            state.data[field].clone()
        } else {
            channels(&state.data)
                .iter()
                .find(|channel| channel["name"] == name)
                .map(|channel| channel[field].clone())
                .unwrap_or(Value::Null)
        };
        if current["isEnabled"] != true {
            return;
        }
        let value = value.round().clamp(0., 100.);
        let linked = name != "output"
            && channels(&state.data)
                .iter()
                .any(|channel| channel["name"] == name && channel["isLinked"] == true);
        let now = std::time::Instant::now();
        if state.last_fader_emit.get(key).is_some_and(|last| {
            now.duration_since(*last) < Duration::from_millis(if linked { 100 } else { 50 })
        }) {
            self.sync_seiren_stream_mixer_controls(false, window, cx);
            return;
        }
        state.last_fader_emit.insert(key.into(), now);
        if name == "output" {
            let mut changed = current;
            changed["value"] = json!(value);
            changed["isToggle"] = json!(false);
            self.submit_seiren_stream_mixer(
                if field == "streamMixVolume" {
                    "ON_SET_STREAM_MIX_VOLUME"
                } else {
                    "ON_SET_PLAYBACK_MIX_VOLUME"
                },
                json!({field:changed}),
                window,
                cx,
            );
            return;
        }
        let mut changed = current;
        changed["value"] = json!(value);
        if !linked {
            self.set_seiren_stream_mixer_input(name, field, changed, window, cx);
            return;
        }
        if state
            .linked_gesture
            .as_ref()
            .is_none_or(|gesture| gesture.0 != name || gesture.1 != field)
        {
            let channel = channels(&state.data)
                .iter()
                .find(|channel| channel["name"] == name)
                .expect("current channel");
            state.linked_gesture = Some((
                name.into(),
                field.into(),
                channel["streamMixerInputVolume"]["value"]
                    .as_f64()
                    .unwrap_or(0.),
                channel["playbackMixInputVolume"]["value"]
                    .as_f64()
                    .unwrap_or(0.),
            ));
        }
        let gesture = state.linked_gesture.as_ref().unwrap();
        let (own_initial, peer_initial, peer_field) = if field == "streamMixerInputVolume" {
            (gesture.2, gesture.3, "playbackMixInputVolume")
        } else {
            (gesture.3, gesture.2, "streamMixerInputVolume")
        };
        let Some(peer) = linked_peer(own_initial, peer_initial, value) else {
            self.sync_seiren_stream_mixer_controls(false, window, cx);
            return;
        };
        let mut peer_value = channels(&state.data)
            .iter()
            .find(|channel| channel["name"] == name)
            .unwrap()[peer_field]
            .clone();
        peer_value["value"] = json!(peer);
        self.set_seiren_stream_mixer_input(name, field, changed, window, cx);
        self.set_seiren_stream_mixer_input(name, peer_field, peer_value, window, cx);
    }
    fn toggle_seiren_stream_mixer_mute(
        &mut self,
        name: &str,
        field: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &self.seiren_stream else {
            return;
        };
        if !enabled(&state.data) {
            return;
        }
        let mut value = if name == "output" {
            state.data[field].clone()
        } else {
            channels(&state.data)
                .iter()
                .find(|channel| channel["name"] == name)
                .map(|channel| channel[field].clone())
                .unwrap_or(Value::Null)
        };
        value["isEnabled"] = json!(value["isEnabled"] != true);
        if name == "output" {
            value["isToggle"] = json!(true);
            self.submit_seiren_stream_mixer(
                if field == "streamMixVolume" {
                    "ON_SET_STREAM_MIX_VOLUME"
                } else {
                    "ON_SET_PLAYBACK_MIX_VOLUME"
                },
                json!({field:value}),
                window,
                cx,
            );
        } else {
            self.set_seiren_stream_mixer_input(name, field, value, window, cx);
        }
    }
    fn add_seiren_stream_mixer_channel(
        &mut self,
        channel: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(state) = &mut self.seiren_stream {
            state.add_open = false;
        }
        self.submit_seiren_stream_mixer(
            "ON_MODIFY_INPUT_CHANNEL",
            json!({"action":"add","modifyInputChannel":channel}),
            window,
            cx,
        );
    }
    fn add_seiren_stream_mixer_external(
        &mut self,
        device: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &self.seiren_stream else {
            return;
        };
        let Some(name) = (1..=3).map(|index| format!("Aux{index}")).find(|name| {
            !channels(&state.data)
                .iter()
                .any(|channel| channel["isVisible"] == true && channel["name"] == *name)
        }) else {
            return;
        };
        self.add_seiren_stream_mixer_channel(json!({"name":name,"inputChannelType":"configurable","streamMixerInputVolume":{"isEnabled":true,"value":50},"playbackMixInputVolume":{"isEnabled":true,"value":50},"isLinked":false,"isVisible":true,"exterInputDevice":device}),window,cx);
    }
    fn remove_seiren_stream_mixer_channel(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(channel) = self
            .seiren_stream
            .as_ref()
            .and_then(|state| {
                channels(&state.data)
                    .iter()
                    .find(|channel| channel["name"] == name)
            })
            .cloned()
        else {
            return;
        };
        self.submit_seiren_stream_mixer(
            "ON_MODIFY_INPUT_CHANNEL",
            json!({"action":"remove","modifyInputChannel":channel}),
            window,
            cx,
        );
    }
    fn reorder_seiren_stream_mixer(
        &mut self,
        from: &str,
        to: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        state.drag_channel = None;
        state.drag_over = None;
        let mut visible = channels(&state.data)
            .iter()
            .filter(|channel| channel["isVisible"] == true)
            .cloned()
            .collect::<Vec<_>>();
        let Some(from_index) = visible.iter().position(|channel| channel["name"] == from) else {
            return;
        };
        let Some(to_index) = visible.iter().position(|channel| channel["name"] == to) else {
            return;
        };
        let moved = visible.remove(from_index);
        visible.insert(to_index, moved);
        visible.extend(
            channels(&state.data)
                .iter()
                .filter(|channel| channel["isVisible"] != true)
                .cloned(),
        );
        for (index, channel) in visible.iter_mut().enumerate() {
            channel["order"] = json!(index);
        }
        self.submit_seiren_stream_mixer(
            "ON_MODIFY_INPUT_CHANNEL",
            json!({"action":"reorder","inputChannels":visible}),
            window,
            cx,
        );
    }
    fn seiren_stream_mixer_preset_command(
        &mut self,
        command: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.seiren_stream else {
            return;
        };
        let guid = selected(&state.data).to_owned();
        match command {
            "rename" => {
                let name = presets(&state.data)
                    .iter()
                    .find(|preset| preset["guid"] == guid)
                    .and_then(|preset| preset["name"].as_str())
                    .unwrap_or("")
                    .to_owned();
                state.renaming = true;
                state.rename.update(cx, |input, cx| {
                    input.set_value(name, window, cx);
                    input.focus(window, cx);
                });
                cx.notify();
            }
            "reset" => {
                state.reset_timer = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let _ = this.update(cx, |this, cx| {
                        if let Some(state) = &mut this.seiren_stream {
                            state.reset_open = true;
                        }
                        cx.notify();
                    });
                }));
            }
            "delete" => {
                if presets(&state.data).len() <= 1 {
                    return;
                }
                let next = presets(&state.data)
                    .iter()
                    .find(|preset| preset["guid"] != guid)
                    .map(|preset| preset["guid"].clone())
                    .unwrap_or(json!(""));
                self.submit_seiren_stream_mixer(
                    "ON_DELETE_STREAM_MIXER_PRESET",
                    json!({"deletedPresetId":guid,"selectedPresetGuid":next}),
                    window,
                    cx,
                );
            }
            "add" | "duplicate" => {
                let duplicate = command == "duplicate";
                let base = if duplicate {
                    presets(&state.data)
                        .iter()
                        .find(|preset| preset["guid"] == guid)
                        .and_then(|preset| preset["name"].as_str())
                        .unwrap_or("")
                        .to_owned()
                } else {
                    std::env::var("COMPUTERNAME")
                        .ok()
                        .filter(|name| !name.is_empty())
                        .map(|name| format!("{name}-Default"))
                        .unwrap_or_else(|| "Default".into())
                };
                let name = source_unique_name(presets(&state.data), &base, duplicate);
                let preset = json!({"name":name,"guid":uuid::Uuid::new_v4().to_string()});
                let payload = if duplicate {
                    json!({"newPreset":preset,"selectedPresetGuid":preset["guid"]})
                } else {
                    json!({"newPreset":preset})
                };
                self.submit_seiren_stream_mixer(
                    if duplicate {
                        "ON_DUPLICATE_STREAM_MIXER_PRESET"
                    } else {
                        "ON_ADD_STREAM_MIXER_PRESET"
                    },
                    payload,
                    window,
                    cx,
                );
            }
            _ => {}
        }
    }
}

fn preserve_channel_order(current: &Value, incoming: &mut Value) {
    let order = channels(current)
        .iter()
        .filter_map(|channel| {
            Some((
                channel["name"].as_str()?.to_owned(),
                channel["order"].as_f64()?,
            ))
        })
        .collect::<BTreeMap<_, _>>();
    if order.is_empty() {
        return;
    }
    if let Some(items) = incoming["inputChannels"].as_array_mut() {
        for item in items.iter_mut() {
            item["order"] = json!(
                order
                    .get(item["name"].as_str().unwrap_or(""))
                    .copied()
                    .unwrap_or(9999.)
            );
        }
        items.sort_by(|a, b| {
            a["order"]
                .as_f64()
                .unwrap_or(9999.)
                .total_cmp(&b["order"].as_f64().unwrap_or(9999.))
        });
    }
}

fn linked_peer(own: f64, peer: f64, next: f64) -> Option<f64> {
    let next_peer = if (peer - own).abs() < 1. {
        next
    } else {
        peer + next - own
    };
    (0. ..=100.).contains(&next_peer).then_some(next_peer)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn stream_mixer_capture_guard_rejects_other_owners_generations_and_replaced_leases() {
        let guard = Arc::new(AtomicBool::new(true));
        let owner = EntityId::from(1);
        let request = SeirenStreamMixerCaptureChanged {
            owner,
            product_id: 1422,
            generation: 7,
            capturing: true,
            valid: guard.clone(),
        };
        assert!(request.matches_owner(1422, owner, 7, true, &guard));
        assert!(!request.matches_owner(1446, owner, 7, true, &guard));
        assert!(!request.matches_owner(1422, EntityId::from(2), 7, true, &guard));
        assert!(!request.matches_owner(1422, owner, 8, true, &guard));
        assert!(!request.matches_owner(1422, owner, 7, false, &guard));
        assert!(!request.matches_owner(1422, owner, 7, true, &Arc::new(AtomicBool::new(true))));
        guard.store(false, Ordering::Release);
        assert!(!request.matches_owner(1422, owner, 7, true, &guard));
    }

    #[test]
    fn stream_mixer_linked_offsets_reject_out_of_range_moves() {
        assert_eq!(linked_peer(30., 70., 50.), Some(90.));
        assert_eq!(linked_peer(30., 70., 80.), None);
        assert_eq!(linked_peer(70., 30., 20.), None);
        assert_eq!(linked_peer(50., 50.5, 40.), Some(40.));
    }

    #[test]
    fn stream_mixer_reducer_retains_current_source_mapping_and_reset_quirks() {
        let mut data = source_initial();
        data["shortcutMapping"] = json!({"inputID":"KEY_A"});
        let old = selected(&data).to_owned();
        optimistic(
            &mut data,
            "ON_DUPLICATE_STREAM_MIXER_PRESET",
            &json!({"newPreset":{"guid":"new-guid","name":"New"},"selectedPresetGuid":"new-guid"}),
        );
        assert_eq!(selected(&data), "new-guid");
        assert_eq!(presets(&data).len(), 2);
        assert_eq!(data["shortcutMapping"], json!({}));
        optimistic(
            &mut data,
            "ON_CHANGE_STREAM_MIXER_PRESET_MAPPING",
            &json!({"newShortcutMapping":{"inputID":"KEY_B"},"changePresetGUID":"new-guid"}),
        );
        assert_eq!(data["shortcutMapping"], json!({}));
        let changed = presets(&data)
            .iter()
            .find(|preset| preset["guid"] == "new-guid")
            .unwrap();
        assert_eq!(changed["mapping"]["inputID"], "KEY_B");
        assert!(changed.get("shortcutMapping").is_none());
        let before = data.clone();
        optimistic(
            &mut data,
            "ON_RESET_STREAM_MIXER_PRESET",
            &json!({"selectedPresetGuid":old}),
        );
        assert_eq!(data, before);
    }

    #[test]
    fn stream_mixer_cache_keeps_ui_order_and_stable_new_channel_order() {
        let current =
            json!({"inputChannels":[{"name":"Music","order":1},{"name":"Microphone","order":0}]});
        let mut incoming = json!({"inputChannels":[{"name":"Music","value":35},{"name":"Game"},{"name":"Microphone","value":87},{"name":"Browser"}]});
        preserve_channel_order(&current, &mut incoming);
        assert_eq!(
            channels(&incoming)
                .iter()
                .map(|channel| channel["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["Microphone", "Music", "Game", "Browser"]
        );
        assert_eq!(channels(&incoming)[0]["value"], 87);
        let without_order = source_initial();
        let before = incoming.clone();
        preserve_channel_order(&without_order, &mut incoming);
        assert_eq!(before, incoming);
    }

    #[test]
    fn stream_mixer_source_names_preserve_case_and_duplicate_suffixes() {
        let names = vec![
            json!({"name":"Preset"}),
            json!({"name":"Preset 1"}),
            json!({"name":"Preset (1)"}),
        ];
        assert_eq!(source_unique_name(&names, "preset", false), "preset");
        assert_eq!(source_unique_name(&names, "Preset", false), "Preset 2");
        assert_eq!(source_unique_name(&names, "Preset (1)", true), "Preset (2)");
    }
}
fn source_unique_name(items: &[Value], base: &str, duplicate: bool) -> String {
    let exists = |name: &str| {
        items
            .iter()
            .any(|item| item["name"] == name || duplicate && item["content"] == name)
    };
    if !duplicate {
        let mut next = base.to_owned();
        for index in 1..=100 {
            if !exists(&next) {
                return next;
            }
            next = format!("{base} {index}");
        }
        return next;
    }
    let base = base
        .strip_suffix(')')
        .and_then(|name| name.rsplit_once(" ("))
        .filter(|(_, suffix)| !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()))
        .map_or(base, |(name, _)| name.trim());
    if !exists(base) {
        return base.into();
    }
    (1..)
        .map(|index| format!("{base} ({index})"))
        .find(|name| !exists(name))
        .expect("finite preset library")
}

fn optimistic(data: &mut Value, event: &str, payload: &Value) {
    match event {
        "ON_SET_STREAM_MIXER_ENABLE" => {
            data["isStreamMixerEnabled"] = payload["isStreamMixerEnabled"].clone();
            data["streamMixerEnablePendingMwAck"] = json!(true);
            data["activeStreamMixerStatus"] = json!(false);
            data["waitingForLevelMeter"] = payload["isStreamMixerEnabled"].clone();
            if payload["isStreamMixerEnabled"] != true {
                data["levelMeter"] = json!([]);
            }
        }
        "ON_SET_STREAM_MIX_VOLUME" => data["streamMixVolume"] = payload["streamMixVolume"].clone(),
        "ON_SET_PLAYBACK_MIX_VOLUME" => {
            data["playbackMixVolume"] = payload["playbackMixVolume"].clone()
        }
        "ON_SET_STREAM_MIX_MONITOR" => {
            data["isStreamMixMonitor"] = payload["isStreamMixMonitor"].clone()
        }
        "ON_CHANGE_PLAY_BACK_MIX_DEVICE" => {
            data["playbackMixDevice"] = payload["changePlayBackMixDevice"].clone()
        }
        "ON_MODIFY_INPUT_CHANNEL" => {
            if payload["action"] == "reorder" {
                data["inputChannels"] = payload["inputChannels"].clone();
            } else if let Some(inputs) = data["inputChannels"].as_array_mut() {
                let change = &payload["modifyInputChannel"];
                if let Some(channel) = inputs
                    .iter_mut()
                    .find(|channel| channel["name"] == change["name"])
                {
                    merge(channel, change);
                    if payload["action"] == "remove" {
                        channel["isVisible"] = json!(false);
                    }
                } else if payload["action"] == "add" {
                    inputs.push(change.clone());
                }
            }
        }
        "ON_SELECTED_STREAM_MIXER_PRESET" => {
            data["selectedPresetGuid"] = payload["selectedPresetGuid"].clone();
            data["shortcutMapping"] = presets(data)
                .iter()
                .find(|preset| preset["guid"] == payload["selectedPresetGuid"])
                .and_then(|preset| preset.get("shortcutMapping"))
                .cloned()
                .unwrap_or_else(|| json!({}));
        }
        "ON_ADD_STREAM_MIXER_PRESET" | "ON_DUPLICATE_STREAM_MIXER_PRESET" => {
            if let Some(items) = data["presets"].as_array_mut() {
                items.push(payload["newPreset"].clone());
            }
            data["selectedPresetGuid"] = payload["newPreset"]["guid"].clone();
            data["shortcutMapping"] = json!({});
            sort_presets(data);
        }
        "ON_RENAME_STREAM_MIXER_PRESET" => {
            if let Some(items) = data["presets"].as_array_mut() {
                for item in items {
                    if item["guid"] == payload["selectedPresetGuid"] {
                        item["name"] = payload["newName"].clone();
                    }
                }
            }
            sort_presets(data);
        }
        "ON_DELETE_STREAM_MIXER_PRESET" => {
            if let Some(items) = data["presets"].as_array_mut() {
                items.retain(|item| item["guid"] != payload["deletedPresetId"]);
            }
            sort_presets(data);
            data["selectedPresetGuid"] = payload["selectedPresetGuid"].clone();
            data["shortcutMapping"] = presets(data)
                .first()
                .and_then(|preset| preset.get("shortcutMapping"))
                .cloned()
                .unwrap_or_else(|| json!({}));
        }
        // Current reducer VbO stores `mapping` on its preset, and does not
        // replace shortcutMapping until MW_INIT_STREAM_MIXER arrives.
        "ON_CHANGE_STREAM_MIXER_PRESET_MAPPING" => {
            if let Some(items) = data["presets"].as_array_mut() {
                for item in items {
                    if item["guid"] == payload["changePresetGUID"] {
                        item["mapping"] = payload["newShortcutMapping"].clone();
                    }
                }
            }
        }
        // R3K constructs an empty shortcutMapping and then passes the original
        // state to M(e,...). Preserve that source behavior.
        "ON_RESET_STREAM_MIXER_PRESET" => {}
        _ => unreachable!("Action validates all current stream mixer events"),
    }
}

// Generated catalog reader: razer-pages/seiren_stream_mixer.
fn read_seiren_stream_mixer_catalog() -> BTreeMap<String, Recipe> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, Recipe>> {
        let value = match r.tag()? {
            0 => r.map(decode_1, decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Recipe> {
        let value = match r.tag()? {
            0 => Recipe {
                native_device_ids: r.child(decode_3)?,
                name_to_device_id: r.child(decode_5)?,
                device_name: r.child(decode_1)?,
                capture_keys: r.child(decode_6)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, u64>> {
        let value = match r.tag()? {
            0 => r.map(decode_1, decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u64> {
        let value = match r.tag()? {
            0 => r.u64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_1, decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<SourceKey>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_7)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<SourceKey> {
        let value = match r.tag()? {
            0 => SourceKey {
                name: r.child(decode_8)?,
                input_id: r.child(decode_9)?,
                kind: r.child(decode_8)?,
                scancode: r.child(decode_10)?,
                virtual_key: r.child(decode_8)?,
                key_code: r.child(decode_8)?,
                flag: r.child(decode_10)?,
                page_id: r.child(decode_9)?,
                hid: r.child(decode_9)?,
                extended_key: None,
                modifier: None,
                output_flag: None,
            },
            1 => SourceKey {
                name: r.child(decode_8)?,
                input_id: r.child(decode_9)?,
                kind: r.child(decode_8)?,
                scancode: r.child(decode_10)?,
                virtual_key: r.child(decode_8)?,
                key_code: r.child(decode_8)?,
                flag: r.child(decode_10)?,
                page_id: None,
                hid: None,
                extended_key: None,
                modifier: None,
                output_flag: None,
            },
            2 => SourceKey {
                name: r.child(decode_8)?,
                input_id: r.child(decode_9)?,
                kind: r.child(decode_8)?,
                scancode: r.child(decode_10)?,
                virtual_key: r.child(decode_8)?,
                key_code: r.child(decode_8)?,
                flag: r.child(decode_10)?,
                page_id: r.child(decode_9)?,
                hid: r.child(decode_9)?,
                extended_key: r.child(decode_9)?,
                modifier: None,
                output_flag: None,
            },
            3 => SourceKey {
                name: r.child(decode_8)?,
                input_id: r.child(decode_9)?,
                kind: r.child(decode_8)?,
                scancode: r.child(decode_10)?,
                virtual_key: r.child(decode_8)?,
                key_code: r.child(decode_8)?,
                flag: r.child(decode_10)?,
                page_id: r.child(decode_9)?,
                hid: r.child(decode_9)?,
                extended_key: r.child(decode_9)?,
                modifier: None,
                output_flag: r.child(decode_11)?,
            },
            4 => SourceKey {
                name: r.child(decode_8)?,
                input_id: r.child(decode_9)?,
                kind: r.child(decode_8)?,
                scancode: r.child(decode_10)?,
                virtual_key: r.child(decode_8)?,
                key_code: r.child(decode_8)?,
                flag: r.child(decode_10)?,
                page_id: r.child(decode_9)?,
                hid: r.child(decode_9)?,
                extended_key: None,
                modifier: None,
                output_flag: r.child(decode_11)?,
            },
            5 => SourceKey {
                name: r.child(decode_8)?,
                input_id: r.child(decode_9)?,
                kind: r.child(decode_8)?,
                scancode: r.child(decode_10)?,
                virtual_key: r.child(decode_8)?,
                key_code: r.child(decode_8)?,
                flag: r.child(decode_10)?,
                page_id: None,
                hid: r.child(decode_9)?,
                extended_key: r.child(decode_9)?,
                modifier: None,
                output_flag: r.child(decode_11)?,
            },
            6 => SourceKey {
                name: r.child(decode_8)?,
                input_id: None,
                kind: r.child(decode_8)?,
                scancode: r.child(decode_10)?,
                virtual_key: r.child(decode_8)?,
                key_code: r.child(decode_8)?,
                flag: r.child(decode_10)?,
                page_id: None,
                hid: None,
                extended_key: None,
                modifier: r.child(decode_9)?,
                output_flag: None,
            },
            7 => SourceKey {
                name: r.child(decode_8)?,
                input_id: r.child(decode_9)?,
                kind: r.child(decode_8)?,
                scancode: r.child(decode_10)?,
                virtual_key: r.child(decode_8)?,
                key_code: r.child(decode_8)?,
                flag: r.child(decode_10)?,
                page_id: r.child(decode_9)?,
                hid: r.child(decode_9)?,
                extended_key: None,
                modifier: r.child(decode_9)?,
                output_flag: None,
            },
            8 => SourceKey {
                name: r.child(decode_8)?,
                input_id: None,
                kind: r.child(decode_8)?,
                scancode: r.child(decode_10)?,
                virtual_key: r.child(decode_8)?,
                key_code: r.child(decode_8)?,
                flag: r.child(decode_10)?,
                page_id: r.child(decode_9)?,
                hid: r.child(decode_9)?,
                extended_key: None,
                modifier: r.child(decode_9)?,
                output_flag: None,
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
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_8)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u64> {
        let value = match r.tag()? {
            0 => r.u64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u64>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_10)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_SEIREN_STREAM_MIXER,
        0,
        "efda3ac7d7bf5ac417d7444a98ee7d3009e177c8764e373849ded8f408a0f54e",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod seiren_stream_mixer_catalog_tests {
    #[test]
    fn packaged_catalog_seiren_stream_mixer_decodes() {
        let _ = super::read_seiren_stream_mixer_catalog();
    }
}
// End generated catalog reader: razer-pages/seiren_stream_mixer.

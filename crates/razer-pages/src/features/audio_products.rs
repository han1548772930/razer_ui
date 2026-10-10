//! Native audio controls backed by each product's current mounted source.
//! The snapshot is a local draft: it never claims an audio driver acknowledgement.
use super::Choice;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::{
    ActiveTheme, Disableable, Selectable, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    select::{Select, SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_device::audio_mixer::MixerValue;
use razer_i18n::t;
use razer_widgets::surface;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::OnceLock,
};

#[path = "control_pod_audio.rs"]
mod control_pod_audio;
pub use control_pod_audio::RuntimeAudioDevice;
#[path = "audio_mixer_effects_page.rs"]
mod audio_mixer_effects_page;
#[path = "audio_mixer_presets.rs"]
mod audio_mixer_presets;
pub use audio_mixer_presets::{PresetShortcutBinding, PresetShortcutRequest};
#[path = "audio_demo.rs"]
mod demo;
#[path = "audio_nommo.rs"]
mod nommo;
#[path = "audio_nommo_effects.rs"]
mod nommo_effects;
#[path = "audio_oled.rs"]
mod oled;
#[path = "audio_oled_home.rs"]
mod oled_home;
#[path = "stream_mixer.rs"]
mod stream_mixer;
#[path = "stream_mixer_number.rs"]
mod stream_mixer_number;
pub use oled_home::{OledRuntimeObservation, OledRuntimeRequested};
pub use stream_mixer::StreamMixerObservation;
#[path = "audio_volume.rs"]
mod audio_volume;
pub use super::audio_mixer::{
    AudioMixerCompletion, AudioMixerOperation, AudioMixerReply, AudioMixerRequest, AudioMixerState,
    path as mixer_path,
};
pub use audio_volume::{
    AudioVolumeCompletion, AudioVolumeOperation, AudioVolumeReply, AudioVolumeRequest,
};

#[derive(Deserialize)]
struct AudioOption {
    label: String,
    value: Value,
    /// 原版 OLED 屏保选项自带预览图（1383 `Kv`、691 屏保），值本身仍是数字。
    #[serde(default)]
    image: Option<String>,
}
#[derive(Deserialize)]
struct AudioControl {
    path: String,
    label: String,
    kind: String,
    /// 原版“选择后按 APPLY 才提交”的行（1383 `Bv` 的 OLED 语言）：按钮文案键。
    #[serde(default)]
    apply_label: Option<String>,
    #[serde(default)]
    min: f32,
    #[serde(default)]
    max: f32,
    #[serde(default = "one")]
    step: f32,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    options: Vec<AudioOption>,
    #[serde(default)]
    enabled_by: Option<String>,
    /// Additional source-confirmed gates, combined with the local bus switch.
    #[serde(default)]
    enabled_all: Vec<String>,
    #[serde(default)]
    exclusive_with: Vec<String>,
    #[serde(default)]
    url: Option<String>,
}
fn one() -> f32 {
    1.
}
#[derive(Deserialize)]
struct AudioSection {
    title: String,
    #[serde(default)]
    suffix: String,
    /// `.widget` 右上角帮助按钮的提示文案键（原版 `tips`）。
    #[serde(default)]
    tips: Option<String>,
    #[serde(default)]
    description: Option<String>,
    controls: Vec<AudioControl>,
    #[serde(default)]
    equalizer: Option<String>,
    #[serde(default)]
    visible_when: Option<AudioCondition>,
}
#[derive(Deserialize)]
struct AudioCondition {
    path: String,
    value: Value,
}
#[derive(Deserialize)]
struct AudioPage {
    key: String,
    sections: Vec<AudioSection>,
}
#[derive(Deserialize)]
struct AudioPreset {
    key: String,
    label: String,
    bands: Vec<f32>,
}
#[derive(Deserialize)]
struct AudioEqualizer {
    key: String,
    frequencies: Vec<String>,
    presets: Vec<AudioPreset>,
    selected: String,
    min: f32,
    max: f32,
    #[serde(default = "one")]
    step: f32,
    #[serde(default)]
    edit_custom: bool,
}
#[derive(Deserialize)]
pub struct AudioProductSpec {
    product_id: u32,
    draft: Value,
    pages: Vec<AudioPage>,
    #[serde(default)]
    equalizers: Vec<AudioEqualizer>,
}
pub fn source_product(pid: u32) -> Option<&'static AudioProductSpec> {
    static PRODUCTS: OnceLock<Vec<AudioProductSpec>> = OnceLock::new();
    PRODUCTS
        .get_or_init(|| {
            serde_json::from_str(include_str!("audio_products_data.json"))
                .expect("validated current audio specifications")
        })
        .iter()
        .find(|p| p.product_id == pid)
}
pub fn supports_page(pid: u32, key: &str) -> bool {
    if key == "TAB_DEMO" && demo::supports(pid) {
        return true;
    }
    source_product(pid).is_some_and(|p| {
        p.pages
            .iter()
            .any(|p| p.key == key && !p.sections.is_empty())
    })
}

#[derive(Deserialize)]
struct ChromaLightingPage {
    product_id: u32,
    page: String,
    body_min_width: f32,
}

/// The current product root must resolve to its own registered lighting
/// component. A generic audio page, or a root name alone, is insufficient.
/// The generated receipt includes the independent `LIGHTING` key of 1465.
fn chroma_lighting_page(pid: u32) -> Option<&'static ChromaLightingPage> {
    static PAGES: OnceLock<Vec<ChromaLightingPage>> = OnceLock::new();
    PAGES
        .get_or_init(|| {
            serde_json::from_str(include_str!("audio_chroma_modes.json"))
                .expect("validated audio chroma root-to-page routes")
        })
        .iter()
        .find(|entry| entry.product_id == pid && supports_page(pid, &entry.page))
}

pub fn supports_chroma_lighting_page(pid: u32) -> bool {
    chroma_lighting_page(pid).is_some()
}
pub struct AudioProductChanged;
pub struct AudioStudioRequested;
#[derive(Clone)]
pub enum AudioNavigation {
    Page(razer_catalog::ProductPageId),
    History(bool),
}
pub struct AudioProductWorkspace {
    spec: &'static AudioProductSpec,
    page: String,
    draft: Value,
    sliders: BTreeMap<String, Entity<SliderState>>,
    mixer_numbers: BTreeMap<String, Entity<stream_mixer_number::MixerNumber>>,
    mixer: Option<stream_mixer::MixerState>,
    selects: BTreeMap<String, Entity<SelectState<Vec<Choice>>>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
    /// 仅暂存、等待 APPLY 提交的选择值（原版 `Bv` 的 `useState`）。
    staged: BTreeMap<String, Value>,
    selected_region: usize,
    nommo_brightness_dragging: bool,
    nommo_effects: Option<nommo_effects::NommoEffectsState>,
    oled_home: Option<oled_home::OledHomeState>,
    demo: Option<Entity<demo::AudioDemo>>,
    pod_audio_editor: Option<(String, Entity<control_pod_audio::AudioEditor>)>,
    pod_audio_subscription: Option<Subscription>,
    pod_runtime_devices: Vec<RuntimeAudioDevice>,
    volume: audio_volume::State,
    pub(super) mixer_io: AudioMixerState,
    mixer_observed: BTreeMap<String, MixerValue>,
    mixer_read_queue: VecDeque<String>,
    mixer_write_queue: VecDeque<(String, MixerValue)>,
    mixer_eq_queue: Option<[i32; 10]>,
    mixer_revisions: BTreeMap<String, u64>,
    mixer_error: Option<String>,
    mixer_echo_timer: Option<Task<()>>,
    mixer_echo_generation: u64,
    effect_presets: Option<audio_mixer_presets::State>,
}
impl EventEmitter<AudioProductChanged> for AudioProductWorkspace {}
impl EventEmitter<AudioStudioRequested> for AudioProductWorkspace {}
impl EventEmitter<AudioMixerRequest> for AudioProductWorkspace {}
impl EventEmitter<AudioNavigation> for AudioProductWorkspace {}

impl AudioProductWorkspace {
    pub fn new(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = source_product(pid).expect("audited audio product");
        let mut this = Self {
            spec,
            page: spec.pages.first().map_or(String::new(), |p| p.key.clone()),
            draft: spec.draft.clone(),
            sliders: BTreeMap::new(),
            mixer_numbers: BTreeMap::new(),
            mixer: stream_mixer::MixerState::new(pid, window, cx),
            selects: BTreeMap::new(),
            subscriptions: Vec::new(),
            syncing: false,
            staged: BTreeMap::new(),
            selected_region: 0,
            nommo_brightness_dragging: false,
            nommo_effects: nommo_effects::NommoEffectsState::new(pid, window, cx),
            oled_home: oled_home::OledHomeState::new(pid),
            demo: demo::AudioDemo::for_product(pid, cx),
            pod_audio_editor: None,
            pod_audio_subscription: None,
            pod_runtime_devices: Vec::new(),
            volume: audio_volume::State::default(),
            mixer_io: AudioMixerState::default(),
            mixer_observed: BTreeMap::new(),
            mixer_read_queue: VecDeque::new(),
            mixer_write_queue: VecDeque::new(),
            mixer_eq_queue: None,
            mixer_revisions: BTreeMap::new(),
            mixer_error: None,
            mixer_echo_timer: None,
            mixer_echo_generation: 0,
            effect_presets: None,
        };
        this.initialize_equalizers();
        this.initialize_nommo_draft();
        this.initialize_oled_home(window);
        this.subscribe_nommo_effects(window, cx);
        this.subscribe_mixer(window, cx);
        for control in spec
            .pages
            .iter()
            .flat_map(|p| &p.sections)
            .flat_map(|s| &s.controls)
        {
            if control.kind == "slider" {
                this.add_slider(
                    &control.path,
                    control.min,
                    control.max,
                    control.step,
                    window,
                    cx,
                );
                this.add_mixer_number(control, window, cx);
            } else if control.kind == "select" && !this.selects.contains_key(&control.path) {
                // Option identity is its serialized source value, not its translated label.
                let choices = control
                    .options
                    .iter()
                    .map(|o| Choice::new(o.value.to_string(), t(&o.label)))
                    .collect::<Vec<_>>();
                let state = cx.new(|cx| SelectState::new(choices, None, window, cx));
                let path = control.path.clone();
                this.subscriptions.push(cx.subscribe_in(
                    &state,
                    window,
                    move |this, _, event, window, cx| {
                        if this.syncing {
                            return;
                        }
                        if let SelectEvent::Confirm(Some(value)) = event {
                            if let Some(value) = this
                                .control(&path)
                                .and_then(|c| {
                                    c.options
                                        .iter()
                                        .find(|o| o.value.to_string() == value.as_str())
                                })
                                .map(|o| o.value.clone())
                            {
                                // 原版 `Bv` 只把选择写进本地 state，按 APPLY 才提交；
                                // 其他 select 行选中即写（原版 `ut.A` 的 selectOption）。
                                if this.control(&path).is_some_and(|c| c.apply_label.is_some()) {
                                    this.staged.insert(path.clone(), value);
                                    cx.notify();
                                } else {
                                    this.edit(&path, value, window, cx);
                                }
                            }
                        }
                    },
                ));
                this.selects.insert(control.path.clone(), state);
            }
        }
        for eq in &spec.equalizers {
            for index in 0..eq.frequencies.len() {
                this.add_slider(
                    &format!("/equalizers/{}/bands/{index}", eq.key),
                    eq.min,
                    eq.max,
                    eq.step,
                    window,
                    cx,
                );
            }
        }
        this.initialize_effect_presets(window, cx);
        this.sync(window, cx);
        this
    }
    fn initialize_equalizers(&mut self) {
        self.draft["equalizers"] = json!({});
        for eq in &self.spec.equalizers {
            let selected = eq
                .presets
                .iter()
                .find(|p| p.key == eq.selected)
                .or_else(|| eq.presets.first());
            if let Some(selected) = selected {
                self.draft["equalizers"][&eq.key] = json!({"selected":selected.key, "bands":selected.bands,
                    "presets":eq.presets.iter().map(|p| (p.key.clone(), json!(p.bands))).collect::<serde_json::Map<_,_>>()});
            }
        }
        if self.spec.product_id == 1342 {
            self.draft["equalizers"]["mic"]["useMode"] = json!(0);
        }
    }
    pub fn snapshot(&self) -> Value {
        let mut snapshot = self.draft.clone();
        self.mixer_snapshot(&mut snapshot);
        self.effect_preset_snapshot(&mut snapshot);
        snapshot
    }
    pub fn restore(&mut self, saved: Option<&Value>, window: &mut Window, cx: &mut Context<Self>) {
        self.invalidate_volume();
        self.invalidate_mixer();
        self.restore_mixer(saved, window, cx);
        let staged_oled_language = self.staged.get("/device/oledLanguage").cloned();
        self.pod_audio_editor = None;
        self.pod_audio_subscription = None;
        self.draft = self.spec.draft.clone();
        self.selected_region = 0;
        self.staged.clear();
        if let Some(value) = staged_oled_language {
            self.staged.insert("/device/oledLanguage".into(), value);
        }
        self.nommo_brightness_dragging = false;
        self.initialize_equalizers();
        self.initialize_nommo_draft();
        self.initialize_oled_home(window);
        if let Some(saved) = saved.filter(|v| v.is_object()) {
            merge_known(&mut self.draft, saved);
            if self.spec.product_id == 1382 {
                self.restore_pod_audio(saved);
            }
            self.restore_oled_home_saved(saved);
        }
        self.normalize_nommo_draft();
        self.restore_effect_presets(saved, window, cx);
        self.normalize_oled_home(window);
        // Restored snapshots never widen a current source's supported range.
        for control in self
            .spec
            .pages
            .iter()
            .flat_map(|p| &p.sections)
            .flat_map(|s| &s.controls)
        {
            if control.kind == "slider" {
                if let Some(target) = self.draft.pointer_mut(&control.path) {
                    if let Some(value) = target.as_f64() {
                        *target = normalized(value as f32, control.min, control.max, control.step);
                    }
                }
            } else if matches!(
                control.kind.as_str(),
                "select" | "options" | "image_options" | "presets"
            ) {
                let valid = self
                    .draft
                    .pointer(&control.path)
                    .is_some_and(|v| control.options.iter().any(|o| o.value == *v));
                if !valid {
                    if let (Some(default), Some(target)) = (
                        self.spec.draft.pointer(&control.path),
                        self.draft.pointer_mut(&control.path),
                    ) {
                        *target = default.clone();
                    }
                }
            }
        }
        for eq in &self.spec.equalizers {
            let state = &mut self.draft["equalizers"][&eq.key];
            if !eq
                .presets
                .iter()
                .any(|p| state["selected"].as_str() == Some(&p.key))
            {
                state["selected"] = json!(eq.selected);
            }
            for field in ["bands"] {
                if let Some(bands) = state[field].as_array_mut() {
                    for value in bands {
                        if let Some(n) = value.as_f64() {
                            *value = normalized(n as f32, eq.min, eq.max, eq.step);
                        }
                    }
                }
            }
            if let Some(presets) = state["presets"].as_object_mut() {
                for bands in presets.values_mut().filter_map(Value::as_array_mut) {
                    for value in bands {
                        if let Some(n) = value.as_f64() {
                            *value = normalized(n as f32, eq.min, eq.max, eq.step);
                        }
                    }
                }
            }
        }
        self.sync(window, cx);
        self.reset_mixer_numbers(window, cx);
        self.request_volume_read(cx);
        cx.notify();
    }
    pub fn set_page(&mut self, page: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != page {
            self.dismiss_mixer_warning(window, cx);
            self.pod_audio_editor = None;
            self.pod_audio_subscription = None;
            if self.page == "TAB_OLED" || page == "TAB_OLED" {
                // Bv's staged selection is local to the mounted OLED page.
                self.staged.remove("/device/oledLanguage");
                self.sync_oled_runtime_select(window, cx);
            }
            self.page = page.into();
            self.reset_mixer_numbers(window, cx);
            if page == "TAB_OLED" {
                self.request_oled_runtime_data(cx);
            }
            cx.notify();
        }
    }
    fn control(&self, path: &str) -> Option<&'static AudioControl> {
        self.spec
            .pages
            .iter()
            .flat_map(|p| &p.sections)
            .flat_map(|s| &s.controls)
            .find(|c| c.path == path)
    }
    fn enabled(&self, control: &AudioControl) -> bool {
        if self.spec.product_id == 1342 {
            for base in ["/device/noiseGate/", "/device/compressor/"] {
                if control.path.starts_with(base)
                    && !control.path.ends_with("/isEnabled")
                    && self
                        .draft
                        .pointer(&format!("{base}isEnabled"))
                        .and_then(Value::as_bool)
                        != Some(true)
                {
                    return false;
                }
            }
        }
        if self.spec.product_id == 1352
            && matches!(
                control.path.as_str(),
                "/device/volume/value" | "/device/volume/isEnabled"
            )
        {
            // Current bU makes the volume active on pointer-down even while
            // muted; releasing a non-zero value unmutes via XU.changeValue.
            return self.volume_active();
        }
        self.draft.pointer(&control.path).is_some()
            && control
                .enabled_by
                .as_ref()
                .is_none_or(|p| self.draft.pointer(p).and_then(Value::as_bool) == Some(true))
            && control
                .enabled_all
                .iter()
                .all(|p| self.draft.pointer(p).and_then(Value::as_bool) == Some(true))
    }
    /// 暂存值优先，否则当前值：原版选择框显示的是已提交值，未按 APPLY 前不被改写。
    fn selection_value(&self, control: &AudioControl) -> Option<Value> {
        self.staged
            .get(&control.path)
            .cloned()
            .or_else(|| self.draft.pointer(&control.path).cloned())
    }
    fn add_slider(
        &mut self,
        path: &str,
        min: f32,
        max: f32,
        step: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.sliders.contains_key(path) {
            return;
        }
        let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) else {
            return;
        };
        let state = cx.new(|_| {
            SliderState::new()
                .min(min)
                .max(max)
                .step(step)
                .default_value(value as f32)
        });
        let path = path.to_owned();
        let key = path.clone();
        self.subscriptions.push(cx.subscribe_in(
            &state,
            window,
            move |this, _, event, window, cx| {
                if this.syncing {
                    return;
                }
                if this.spec.product_id == 1352 && key == "/device/volume/value" {
                    match event {
                        SliderEvent::Change(value) => {
                            this.preview_volume(value.start().clamp(0., 100.).round() as u8, cx)
                        }
                        SliderEvent::Release(value) => this.commit_volume(
                            value.start().clamp(0., 100.).round() as u8,
                            window,
                            cx,
                        ),
                    }
                    return;
                }
                if this.spec.product_id == 1342 && key.starts_with("/device/echoReverb/") {
                    // Current O handler previews each drag change and resets
                    // its 500ms timer; it does not wait for pointer release.
                    if let SliderEvent::Change(value) = event {
                        this.edit(&key, normalized(value.start(), min, max, step), window, cx);
                    }
                    return;
                }
                if this.spec.product_id == 1342
                    && (mixer_path(&key).is_some()
                        || key.starts_with("/equalizers/mic_basic/bands/"))
                {
                    match event {
                        SliderEvent::Change(_) => {
                            let revision = this.mixer_io.edit_revision.wrapping_add(1);
                            this.mixer_io.edit_revision = revision;
                            this.mixer_revisions.insert(key.clone(), revision);
                            if key.starts_with("/equalizers/mic") {
                                // A newer drag has no submitted array until
                                // release. Do not send a previous queued array
                                // while the current edit is still in progress.
                                this.mixer_eq_queue = None;
                                this.mixer_revisions
                                    .insert("/equalizers/mic".into(), revision);
                                if this.mixer_io.pending.as_ref().is_some_and(|request| {
                                    request.path.starts_with("/equalizers/mic")
                                }) {
                                    if let Some(cancel) = &this.mixer_io.cancellation {
                                        cancel.store(true, std::sync::atomic::Ordering::Release);
                                    }
                                }
                            }
                            cx.notify();
                        }
                        SliderEvent::Release(value) => {
                            this.edit(&key, normalized(value.start(), min, max, step), window, cx);
                        }
                    }
                    return;
                }
                if matches!(this.spec.product_id, 1303 | 1304) && key == "/profile/brightness/value"
                {
                    match event {
                        SliderEvent::Change(_) => cx.notify(),
                        SliderEvent::Release(value) => {
                            this.nommo_brightness_dragging = false;
                            this.commit_nommo_brightness(value.start(), window, cx);
                        }
                    }
                    return;
                }
                if let SliderEvent::Change(value) = event {
                    this.edit(&key, normalized(value.start(), min, max, step), window, cx);
                }
            },
        ));
        self.sliders.insert(path, state);
    }
    fn edit(&mut self, path: &str, value: Value, window: &mut Window, cx: &mut Context<Self>) {
        if self.control(path).is_some_and(|c| !self.enabled(c)) {
            return;
        }
        if self
            .pod_audio_editor
            .as_ref()
            .is_some_and(|(editor_path, _)| {
                editor_path.strip_suffix("/outputType").is_some_and(|base| {
                    path == base
                        || path.starts_with(&format!("{base}/"))
                        || base.starts_with(&format!("{path}/"))
                })
            })
        {
            self.pod_audio_editor = None;
            self.pod_audio_subscription = None;
        }
        if self.draft.pointer(path) == Some(&value) {
            cx.notify();
            return;
        }
        let enabled = value == Value::Bool(true);
        if let Some(target) = self.draft.pointer_mut(path) {
            *target = value;
        } else {
            return;
        }
        if self.spec.product_id == 1342 && path == "/device/echoReverb/activeMode" {
            let state = &mut self.draft["device"]["echoReverb"];
            let mode = state["activeMode"].as_str().unwrap_or("library");
            let bands = if mode == "custom" {
                state["customValues"].clone()
            } else {
                razer_device::audio_mixer::echo_presets()
                    .get(mode)
                    .map(|bands| json!(bands))
                    .unwrap_or_else(|| json!(razer_device::audio_mixer::echo_presets()["library"]))
            };
            state["modeValues"] = bands;
        } else if self.spec.product_id == 1342 && path.starts_with("/device/echoReverb/modeValues/")
        {
            let state = &mut self.draft["device"]["echoReverb"];
            state["activeMode"] = json!("custom");
            state["customValues"] = state["modeValues"].clone();
        }
        if enabled {
            if let Some(control) = self.control(path) {
                for peer in &control.exclusive_with {
                    if let Some(target) = self.draft.pointer_mut(peer) {
                        *target = Value::Bool(false);
                    }
                }
            }
        }
        self.sync_haptic_regions(path);
        // Linked stream/playback channel faders are one source-level operation.
        if let Some((channel, rest)) = path
            .rsplit_once("/microphone/")
            .or_else(|| path.rsplit_once("/headphone/"))
        {
            if channel.contains("/otherCategories/")
                && self
                    .draft
                    .pointer(&format!("{channel}/isLinked"))
                    .and_then(Value::as_bool)
                    == Some(true)
            {
                let peer = if path.contains("/microphone/") {
                    "headphone"
                } else {
                    "microphone"
                };
                if let Some(value) = self.draft.pointer(path).cloned() {
                    if let Some(target) =
                        self.draft.pointer_mut(&format!("{channel}/{peer}/{rest}"))
                    {
                        *target = value;
                    }
                }
            }
        }
        if let Some(rest) = path.strip_prefix("/equalizers/") {
            if let Some((key, _)) = rest.split_once("/bands/") {
                if let Some(eq) = self.spec.equalizers.iter().find(|e| e.key == key) {
                    let state = &mut self.draft["equalizers"][key];
                    if eq.edit_custom && eq.presets.iter().any(|p| p.key == "custom") {
                        state["selected"] = json!("custom");
                    }
                    if let Some(selected) = state["selected"].as_str().map(str::to_owned) {
                        state["presets"][selected] = state["bands"].clone();
                    }
                }
            }
        }
        if self.spec.product_id == 1342 {
            self.retain_active_effect(path);
        }
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        if self.spec.product_id == 1342 {
            if path.starts_with("/device/echoReverb/") {
                self.schedule_echo_write(window, cx);
            } else {
                self.request_mixer_write(path, self.draft.pointer(path).cloned(), cx);
            }
        }
        cx.notify();
    }

    fn schedule_echo_write(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.mixer_echo_generation = self.mixer_echo_generation.wrapping_add(1);
        let generation = self.mixer_echo_generation;
        let epoch = self.mixer_io.epoch;
        self.mixer_io.edit_revision = self.mixer_io.edit_revision.wrapping_add(1);
        let revision = self.mixer_io.edit_revision;
        for path in std::iter::once("/device/echoReverb/isEnabled".to_owned())
            .chain((0..4).map(|index| format!("/device/echoReverb/modeValues/{index}")))
        {
            self.mixer_revisions.insert(path, revision);
        }
        self.mixer_write_queue
            .retain(|(path, _)| !path.starts_with("/device/echoReverb/"));
        if self
            .mixer_io
            .pending
            .as_ref()
            .is_some_and(|request| request.path.starts_with("/device/echoReverb/"))
        {
            if let Some(cancel) = &self.mixer_io.cancellation {
                cancel.store(true, std::sync::atomic::Ordering::Release);
            }
        }
        self.mixer_echo_timer = Some(cx.spawn_in(window, async move |owner, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            let _ = owner.update_in(cx, |this, _, cx| {
                if this.mixer_io.active
                    && this.mixer_io.epoch == epoch
                    && this.mixer_echo_generation == generation
                {
                    this.request_mixer_write("/device/echoReverb/isEnabled", None, cx);
                }
            });
        }));
    }

    /// Emit a source-verified HID DSP write for controls backed by
    /// `MixerSDKLib_PropertyControl`. AudioCamy virtual endpoints are omitted
    /// by `mixer_path` and remain local until their own adapter is connected.
    fn request_mixer_write(&mut self, path: &str, _value: Option<Value>, cx: &mut Context<Self>) {
        if path.starts_with("/equalizers/mic/bands/")
            || path.starts_with("/equalizers/mic_basic/bands/")
        {
            self.request_mic_eq_write(cx);
            return;
        }
        if !self.mixer_io.active {
            return;
        }
        let Ok(actions) = super::audio_mixer::write_plan(path, &self.draft) else {
            return;
        };
        if actions.is_empty() {
            return;
        }
        self.mixer_io.edit_revision = self.mixer_io.edit_revision.wrapping_add(1);
        if let Some(base) = [
            "/device/vocalFading/",
            "/device/voiceChanger/",
            "/device/echoReverb/",
        ]
        .into_iter()
        .find(|base| path.starts_with(base))
        {
            // Source effect callers receive the entire retained object. A
            // newer switch/mode supersedes unsent fields from the old object.
            self.mixer_write_queue
                .retain(|(queued_path, _)| !queued_path.starts_with(base));
            for field in ["isEnabled", "value"] {
                self.mixer_revisions
                    .insert(format!("{base}{field}"), self.mixer_io.edit_revision);
            }
            if self
                .mixer_io
                .pending
                .as_ref()
                .is_some_and(|request| request.path.starts_with(base))
            {
                if let Some(cancel) = &self.mixer_io.cancellation {
                    cancel.store(true, std::sync::atomic::Ordering::Release);
                }
            }
        }
        for (path, value) in actions {
            self.mixer_revisions
                .insert(path.clone(), self.mixer_io.edit_revision);
            if let Some((_, queued_value)) = self
                .mixer_write_queue
                .iter_mut()
                .find(|(queued_path, _)| queued_path == &path)
            {
                *queued_value = value;
            } else {
                self.mixer_write_queue.push_back((path, value));
            }
        }
        self.dispatch_next_mixer_read(cx);
    }

    fn request_mic_eq_write(&mut self, cx: &mut Context<Self>) {
        if !self.mixer_io.active {
            return;
        }
        let bands = match super::audio_mixer::mic_eq_bands(&self.draft) {
            Ok(bands) => bands,
            Err(error) => {
                self.mixer_error = Some(error.to_string());
                return;
            }
        };
        self.mixer_io.edit_revision = self.mixer_io.edit_revision.wrapping_add(1);
        let revision = self.mixer_io.edit_revision;
        for path in std::iter::once("/equalizers/mic".to_owned())
            .chain(std::iter::once("/equalizers/mic/isEnabled".to_owned()))
            .chain((0..10).map(|index| format!("/equalizers/mic/bands/{index}")))
        {
            self.mixer_revisions.insert(path, revision);
        }
        if self
            .mixer_io
            .pending
            .as_ref()
            .is_some_and(|request| request.path.starts_with("/equalizers/mic"))
        {
            if let Some(cancel) = &self.mixer_io.cancellation {
                cancel.store(true, std::sync::atomic::Ordering::Release);
            }
        }
        self.mixer_eq_queue = Some(bands);
        self.dispatch_next_mixer_read(cx);
    }

    fn dispatch_mixer_write(&mut self, path: &str, value: MixerValue, cx: &mut Context<Self>) {
        let Some(mapped) = mixer_path(path) else {
            return;
        };
        self.mixer_io.generation = self.mixer_io.generation.wrapping_add(1);
        let request = AudioMixerRequest {
            generation: self.mixer_io.generation,
            operation: AudioMixerOperation::Write,
            path: path.to_owned(),
            target: mapped.target(),
            value: Some(value),
            eq_bands: None,
            epoch: self.mixer_io.epoch,
            edit_revision: self.mixer_revisions.get(path).copied().unwrap_or(0),
        };
        self.mixer_io.pending = Some(request.clone());
        self.mixer_io.cancellation = Some(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
            false,
        )));
        cx.emit(request);
    }

    pub fn mixer_request_matches(&self, request: &AudioMixerRequest) -> bool {
        self.mixer_io.pending.as_ref().is_some_and(|pending| {
            pending.generation == request.generation
                && pending.path == request.path
                && pending.operation == request.operation
        })
    }
    pub fn mixer_request_current(&self, request: &AudioMixerRequest) -> bool {
        self.mixer_request_matches(request)
            && self.mixer_io.active
            && self.mixer_io.epoch == request.epoch
            && self
                .mixer_revisions
                .get(&request.path)
                .copied()
                .unwrap_or(0)
                == request.edit_revision
    }

    /// Queue one source-named DSP read. The caller supplies the currently
    /// mounted control; no catalog default is treated as a hardware value.
    pub fn request_mixer_read(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.spec.product_id != 1342 || self.mixer_io.pending.is_some() {
            return;
        }
        let Some(mapped) = mixer_path(path) else {
            return;
        };
        self.mixer_io.generation = self.mixer_io.generation.wrapping_add(1);
        let request = AudioMixerRequest {
            generation: self.mixer_io.generation,
            operation: AudioMixerOperation::Read,
            path: path.to_owned(),
            target: mapped.target(),
            value: None,
            eq_bands: None,
            epoch: self.mixer_io.epoch,
            edit_revision: self.mixer_revisions.get(path).copied().unwrap_or(0),
        };
        self.mixer_io.pending = Some(request.clone());
        self.mixer_io.cancellation = Some(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
            false,
        )));
        cx.emit(request);
    }

    pub fn set_mixer_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.spec.product_id != 1342 {
            return;
        }
        if self.mixer_io.active == active {
            return;
        }
        self.mixer_io.active = active;
        self.invalidate_mixer();
        self.mixer_read_queue.clear();
        if active {
            self.submit_restored_effect_preset(cx);
            self.dispatch_next_mixer_read(cx);
        }
    }

    fn dispatch_next_mixer_read(&mut self, cx: &mut Context<Self>) {
        if !self.mixer_io.active || self.mixer_io.pending.is_some() {
            return;
        }
        if let Some(bands) = self.mixer_eq_queue.take() {
            self.mixer_io.generation = self.mixer_io.generation.wrapping_add(1);
            let request = AudioMixerRequest {
                generation: self.mixer_io.generation,
                operation: AudioMixerOperation::MicEqWrite,
                path: "/equalizers/mic".into(),
                target: super::audio_mixer::AudioMixerPath::MicEqEnabled.target(),
                value: None,
                eq_bands: Some(bands),
                epoch: self.mixer_io.epoch,
                edit_revision: self
                    .mixer_revisions
                    .get("/equalizers/mic")
                    .copied()
                    .unwrap_or(0),
            };
            self.mixer_io.pending = Some(request.clone());
            self.mixer_io.cancellation = Some(std::sync::Arc::new(
                std::sync::atomic::AtomicBool::new(false),
            ));
            cx.emit(request);
            return;
        }
        if let Some((path, value)) = self.mixer_write_queue.pop_front() {
            self.dispatch_mixer_write(&path, value, cx);
            return;
        }
        let Some(path) = self.mixer_read_queue.pop_front() else {
            return;
        };
        self.request_mixer_read(&path, cx);
    }

    pub fn invalidate_mixer(&mut self) {
        if let Some(cancel) = &self.mixer_io.cancellation {
            cancel.store(true, std::sync::atomic::Ordering::Release);
        }
        self.mixer_io.epoch = self.mixer_io.epoch.wrapping_add(1);
        self.mixer_read_queue.clear();
        self.mixer_write_queue.clear();
        self.mixer_eq_queue = None;
        self.mixer_echo_timer = None;
        self.mixer_echo_generation = self.mixer_echo_generation.wrapping_add(1);
        self.mixer_observed.clear();
        self.mixer_error = None;
    }

    pub fn mixer_request_cancellation(
        &self,
        request: &AudioMixerRequest,
    ) -> Option<std::sync::Arc<std::sync::atomic::AtomicBool>> {
        self.mixer_request_matches(request)
            .then(|| self.mixer_io.cancellation.clone())
            .flatten()
    }

    pub fn finish_mixer(
        &mut self,
        request: AudioMixerRequest,
        result: Result<AudioMixerCompletion, String>,
        scope_current: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.mixer_request_matches(&request) {
            return;
        }
        let previous_error = self.mixer_error.clone();
        let current = scope_current
            && self.mixer_io.active
            && request.epoch == self.mixer_io.epoch
            && self
                .mixer_revisions
                .get(&request.path)
                .copied()
                .unwrap_or(0)
                == request.edit_revision;
        if current {
            match result {
                Ok(completion) => {
                    self.mixer_error = completion.warning;
                    let observed = match completion.reply {
                        AudioMixerReply::Read(value) => Some(value),
                        AudioMixerReply::Write(write) => {
                            if !write.transport_completed {
                                self.mixer_error =
                                    Some("DSP setter transport did not complete".into());
                            }
                            None
                        }
                        AudioMixerReply::MicEqWrite(results) => {
                            if results.len() != 11
                                || !results.iter().all(|write| write.transport_completed)
                            {
                                self.mixer_error =
                                    Some("Mic EQ source setter chain did not complete".into());
                            }
                            None
                        }
                    };
                    if let Some(value) = observed {
                        self.mixer_observed.insert(request.path.clone(), value);
                    }
                }
                Err(error) => {
                    self.mixer_error = Some(error);
                    if request.operation == AudioMixerOperation::Write {
                        // Abort the remainder of this source caller's cascade.
                        self.mixer_write_queue.retain(|(path, _)| {
                            if request.path == "/equalizers/mic/isEnabled" {
                                !path.starts_with("/equalizers/mic/")
                            } else if request.path == "/device/vocalFading/isEnabled" {
                                !path.starts_with("/device/vocalFading/")
                            } else if request.path == "/device/voiceChanger/isEnabled" {
                                !path.starts_with("/device/voiceChanger/")
                            } else if request.path == "/device/echoReverb/isEnabled" {
                                !path.starts_with("/device/echoReverb/")
                            } else {
                                self.mixer_revisions.get(path).copied().unwrap_or(0)
                                    != request.edit_revision
                            }
                        });
                    }
                }
            }
        } else if request.epoch == self.mixer_io.epoch && !scope_current {
            self.invalidate_mixer();
            self.mixer_error = Some("DSP 连接已变化；设备可能已更新，当前状态未确认".into());
        }
        self.mixer_io.pending = None;
        self.mixer_io.cancellation = None;
        if self.mixer_error != previous_error {
            if let Some(error) = &self.mixer_error {
                window.push_notification(error.clone(), cx);
            }
        }
        self.dispatch_next_mixer_read(cx);
        self.sync(window, cx);
        cx.notify();
    }
    fn sync_haptic_regions(&mut self, path: &str) {
        if !path.starts_with("/device/sensa/") {
            return;
        }
        let Some(state) = self.draft.pointer_mut("/device/sensa") else {
            return;
        };
        if let Some(rest) = path.strip_prefix("/device/sensa/regions/") {
            if let Some((index, field)) = rest.split_once('/') {
                if let Ok(index) = index.parse::<usize>() {
                    self.selected_region = index;
                    if state["applyToAllRegions"] == Value::Bool(true) {
                        let value = state["regions"][index][field].clone();
                        if let Some(regions) = state["regions"].as_array_mut() {
                            for region in regions {
                                region[field] = value.clone();
                            }
                        }
                    }
                }
            }
        }
        if path == "/device/sensa/applyToAllRegions"
            && state["applyToAllRegions"] == Value::Bool(true)
        {
            let selected = state["regions"][self.selected_region].clone();
            if let Some(regions) = state["regions"].as_array_mut() {
                for region in regions {
                    region["isEnabled"] = selected["isEnabled"].clone();
                    region["percentage"] = selected["percentage"].clone();
                }
            }
        }
        if path.starts_with("/device/sensa/regions") {
            state["hapticIntensity"]["isEnabled"] = json!(
                state["regions"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|r| r["isEnabled"] == Value::Bool(true)))
            );
        }
        if path == "/device/sensa/hapticIntensity/value"
            && state["hapticIntensity"]["value"].as_f64() == Some(0.)
        {
            state["hapticIntensity"]["isEnabled"] = Value::Bool(false);
        }
        if path == "/device/sensa/hapticIntensity/isEnabled"
            && state["hapticIntensity"]["isEnabled"] == Value::Bool(true)
            && state["hapticIntensity"]["value"].as_f64() == Some(0.)
        {
            state["hapticIntensity"]["value"] =
                self.spec.draft["device"]["sensa"]["hapticIntensity"]["value"].clone();
        }
        if state["hapticIntensity"]["isEnabled"] == Value::Bool(false) {
            if let Some(regions) = state["regions"].as_array_mut() {
                for region in regions {
                    region["isEnabled"] = Value::Bool(false);
                }
            }
        }
    }
    fn change_mic_eq_mode(&mut self, basic: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.draft["equalizers"]["mic"]["useMode"] = json!(u8::from(basic));
        self.mixer_eq_queue = None;
        self.mixer_io.edit_revision = self.mixer_io.edit_revision.wrapping_add(1);
        self.mixer_revisions
            .insert("/equalizers/mic".into(), self.mixer_io.edit_revision);
        if self
            .mixer_io
            .pending
            .as_ref()
            .is_some_and(|request| request.path.starts_with("/equalizers/mic"))
        {
            if let Some(cancel) = &self.mixer_io.cancellation {
                cancel.store(true, std::sync::atomic::Ordering::Release);
            }
        }
        // Current mode task persists useMode, then reads the misspelled
        // micBaicEqualizer profile field. No source writes that alias, so it
        // cannot be fabricated as a successful automatic Basic submission.
        if basic {
            window.push_notification("未能应用麦克风均衡器模式，请重新选择预设。", cx);
        } else {
            self.request_mic_eq_write(cx);
        }
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        cx.notify();
    }

    fn select_preset(
        &mut self,
        key: &str,
        preset: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .spec
            .equalizers
            .iter()
            .any(|e| e.key == key && e.presets.iter().any(|p| p.key == preset))
        {
            return;
        }
        let state = &mut self.draft["equalizers"][key];
        state["selected"] = json!(preset);
        state["bands"] = state["presets"][preset].clone();
        if self.spec.product_id == 1342 && matches!(key, "mic" | "mic_basic") {
            if preset != "custom" {
                if let Some(original) = self
                    .spec
                    .equalizers
                    .iter()
                    .find(|eq| eq.key == key)
                    .and_then(|eq| eq.presets.iter().find(|item| item.key == preset))
                {
                    state["bands"] = json!(original.bands);
                }
            }
            self.request_mic_eq_write(cx);
        }
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    fn reset_equalizer(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id == 1342 && matches!(key, "mic" | "mic_basic") {
            // LM.reset always selects Custom with the Default array, preserving
            // every named preset. The reducer replaces only customBands.
            let Some(default) = self
                .spec
                .equalizers
                .iter()
                .find(|eq| eq.key == key)
                .and_then(|eq| eq.presets.iter().find(|preset| preset.key == "default"))
            else {
                return;
            };
            let state = &mut self.draft["equalizers"][key];
            state["selected"] = json!("custom");
            state["bands"] = json!(default.bands);
            state["presets"]["custom"] = json!(default.bands);
            self.request_mic_eq_write(cx);
            self.sync(window, cx);
            cx.emit(AudioProductChanged);
            cx.notify();
            return;
        }
        let state = &mut self.draft["equalizers"][key];
        if let Some(preset) = self
            .spec
            .equalizers
            .iter()
            .find(|e| e.key == key)
            .and_then(|e| {
                e.presets
                    .iter()
                    .find(|p| Some(p.key.as_str()) == state["selected"].as_str())
            })
        {
            state["bands"] = json!(preset.bands);
            state["presets"][&preset.key] = json!(preset.bands);
            self.sync(window, cx);
            cx.emit(AudioProductChanged);
            cx.notify();
        }
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        self.sync_mixer(window, cx);
        self.sync_mixer_numbers(window, cx);
        for (path, state) in &self.sliders {
            let visible = self
                .volume_value()
                .filter(|_| self.spec.product_id == 1352 && path == "/device/volume/value")
                .map(f64::from);
            if let Some(value) =
                visible.or_else(|| self.draft.pointer(path).and_then(Value::as_f64))
            {
                state.update(cx, |s, cx| s.set_value(value as f32, window, cx));
            }
        }
        for (path, state) in &self.selects {
            if let Some(value) = self
                .control(path)
                .and_then(|control| self.selection_value(control))
            {
                state.update(cx, |s, cx| {
                    s.set_selected_value(&value.to_string(), window, cx)
                });
            }
        }
        self.sync_nommo_effects(window, cx);
        self.sync_oled_runtime_select(window, cx);
        self.syncing = false;
    }
    fn render_control(&self, control: &AudioControl, cx: &mut Context<Self>) -> AnyElement {
        let path = control.path.clone();
        let label = t(&control.label);
        let enabled = self.enabled(control);
        match control.kind.as_str() {
            "link" => gpui_kit::base::Link::new(SharedString::from(format!(
                "audio-link-{path}-{}",
                control.label
            )))
            .href(control.url.clone().unwrap_or_default())
            .accessibility_label(label.clone())
            .open_with(|url, _, _, cx| cx.open_url(url))
            .text_color(cx.theme().primary)
            .child(div().underline().child(label))
            .into_any_element(),
            "unavailable" => Button::new(SharedString::from(format!(
                "audio-unavailable-{path}-{}",
                control.label
            )))
            .small()
            .label(label)
            .disabled(true)
            .into_any_element(),
            "reset" => Button::new(SharedString::from(format!("audio-reset-{path}")))
                .small()
                .ghost()
                .label(label)
                .disabled(!enabled)
                .on_click(cx.listener(move |this, _, window, cx| {
                    if let Some(value) = this.spec.draft.pointer(&path).cloned() {
                        this.edit(&path, value, window, cx);
                    }
                }))
                .into_any_element(),
            "toggle" => Checkbox::new(SharedString::from(format!("audio-{path}")))
                .label(label)
                .checked(
                    self.volume_enabled()
                        .filter(|_| {
                            self.spec.product_id == 1352 && path == "/device/volume/isEnabled"
                        })
                        .unwrap_or_else(|| {
                            self.draft
                                .pointer(&path)
                                .and_then(Value::as_bool)
                                .unwrap_or(false)
                        }),
                )
                .disabled(!enabled)
                .on_click(cx.listener(move |this, value, window, cx| {
                    if this.spec.product_id == 1352 && path == "/device/volume/isEnabled" {
                        this.toggle_volume(*value, window, cx);
                        return;
                    }
                    if this.mixer.is_some()
                        && path == "/device/streamMixerSettings/isStreamMixerEnabled"
                    {
                        this.request_mixer_enable(*value, window, cx);
                        return;
                    }
                    this.edit(&path, json!(value), window, cx);
                }))
                .into_any_element(),
            "select" => v_flex()
                .gap_2()
                .child(label)
                .when_some(self.selects.get(&path), |d, state| {
                    d.child(Select::new(state).disabled(!enabled).w_full())
                })
                .when(self.has_pod_audio_editor(&path), |d| {
                    if let Some((editor_path, editor)) = &self.pod_audio_editor {
                        if editor_path == &path {
                            return d.child(editor.clone());
                        }
                    }
                    d.child(
                        Button::new(SharedString::from(format!("pod-audio-edit-{path}")))
                            .label(t("AUDIO_FUNCTION"))
                            .disabled(!enabled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_pod_audio(&path, window, cx)
                            })),
                    )
                })
                .into_any_element(),
            "presets" => v_flex()
                .gap_2()
                .child(label)
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .children(control.options.iter().map(|option| {
                            let path = path.clone();
                            let value = option.value.clone();
                            Button::new(SharedString::from(format!("audio-preset-{path}-{value}")))
                                .small()
                                .outline()
                                .label(t(&option.label))
                                .selected(self.draft.pointer(&path) == Some(&value))
                                .disabled(!enabled)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.edit(&path, value.clone(), window, cx);
                                }))
                        })),
                )
                .into_any_element(),
            "slider" => {
                let value = self
                    .volume_preview()
                    .or_else(|| self.volume_value())
                    .filter(|_| self.spec.product_id == 1352 && path == "/device/volume/value")
                    .map(f64::from)
                    .unwrap_or_else(|| {
                        self.draft
                            .pointer(&path)
                            .and_then(Value::as_f64)
                            .unwrap_or(f64::from(control.min))
                    });
                let digits = if control.step.fract() == 0. {
                    0
                } else {
                    (-control.step.log10()).ceil().clamp(1., 4.) as usize
                };
                let rendered = format!("{value:.digits$}{}", control.unit);
                v_flex()
                    .gap_3()
                    .child(h_flex().justify_between().child(label).child(
                        if let Some(number) = self.mixer_numbers.get(&path) {
                            number.clone().into_any_element()
                        } else {
                            div().child(rendered).into_any_element()
                        },
                    ))
                    .when_some(self.sliders.get(&path), |d, state| {
                        d.child(Slider::new(state).disabled(!enabled))
                    })
                    .child(
                        h_flex()
                            .justify_between()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{}{}", control.min, control.unit))
                            .child(format!("{}{}", control.max, control.unit)),
                    )
                    .into_any_element()
            }
            _ => div().into_any_element(),
        }
    }
    fn render_equalizer(&self, key: &str, cx: &mut Context<Self>) -> AnyElement {
        let current_mic = self.spec.product_id == 1342 && key == "mic";
        let basic = self
            .draft
            .pointer("/equalizers/mic/useMode")
            .and_then(Value::as_u64)
            == Some(1);
        let key = if current_mic && basic {
            "mic_basic"
        } else {
            key
        };
        let Some(eq) = self.spec.equalizers.iter().find(|e| e.key == key) else {
            return div().into_any_element();
        };
        let state = &self.draft["equalizers"][key];
        let presets = h_flex()
            .gap_2()
            .flex_wrap()
            .children(eq.presets.iter().map(|preset| {
                let key = key.to_owned();
                let name = preset.key.clone();
                Button::new(SharedString::from(format!("audio-eq-{key}-{name}")))
                    .small()
                    .outline()
                    .label(t(&preset.label))
                    .selected(state["selected"].as_str() == Some(&preset.key))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_preset(&key, &name, window, cx)
                    }))
            }));
        let bands = h_flex()
            .items_start()
            .gap_2()
            .children(eq.frequencies.iter().enumerate().map(|(index, frequency)| {
                let path = format!("/equalizers/{key}/bands/{index}");
                v_flex()
                    .id(SharedString::from(format!("audio-eq-{key}-{frequency}")))
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_3()
                    .when(!current_mic || !basic, |view| {
                        view.child(div().text_xs().child(format!(
                            "{:+.0}",
                            state["bands"][index].as_f64().unwrap_or(0.)
                        )))
                    })
                    .when(current_mic && basic, |view| {
                        view.child(div().text_sm().child(t(frequency)))
                    })
                    .when_some(self.sliders.get(&path), |d, s| {
                        d.child(Slider::new(s).vertical().h(surface::css(if current_mic {
                            if basic { 140. } else { 300. }
                        } else {
                            180.
                        })))
                    })
                    .when(!current_mic || !basic, |view| {
                        view.child(div().text_xs().child(frequency.clone()))
                    })
                    .when(current_mic && basic, |view| {
                        view.child(
                            div()
                                .text_xs()
                                .child(format!("{}", state["bands"][index].as_f64().unwrap_or(0.))),
                        )
                        .child(div().text_xs().child("dB"))
                    })
            }));
        let key = key.to_owned();
        v_flex()
            .gap_5()
            .child(presets)
            .child(bands)
            .when(!current_mic || !basic, |view| {
                view.child(
                    h_flex()
                        .justify_between()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{} … {:+} dB", eq.min, eq.max)),
                        )
                        .child(
                            Button::new(SharedString::from(format!("audio-eq-reset-{key}")))
                                .small()
                                .ghost()
                                .label(t("RESET"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.reset_equalizer(&key, window, cx)
                                })),
                        ),
                )
            })
            .when(current_mic, |view| {
                view.child(
                    Button::new("audio-mic-eq-mode")
                        .ghost()
                        .label(t(if basic { "SHOW_MORE" } else { "SHOW_LESS" }))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.change_mic_eq_mode(!basic, window, cx)
                        })),
                )
            })
            .into_any_element()
    }
}
impl AudioProductWorkspace {
    /// Shared body only: the popup must neither mutate the selected main tab
    /// nor show the local product-name heading. The source root applies
    /// `.body-wrapper{padding:10px 20px 20px}`. Nommo 1303/1304 retain the
    /// body's 600px minimum; their root only unsets `.main-container`.
    pub fn lighting_element(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let route = chroma_lighting_page(self.spec.product_id)?;
        Some(
            super::product_surface::body()
                .min_w(surface::css(route.body_min_width))
                .child(self.page_body(&route.page, window, cx))
                .into_any_element(),
        )
    }

    fn page_body(&self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // 1383 的 TAB_OLED 由自己的 OLED 根（`xx`）渲染：两列 `.widget-col` 与
        // 屏保网格都不是通用音频控件的形态，见 audio_oled.rs。
        if self.spec.product_id == 1383 && key == "TAB_OLED" {
            return self.kraken_oled_page(window, cx);
        }
        if self.spec.product_id == 1342 && key == "EFFECTS" {
            return self.mixer_effects_page(window, cx);
        }
        let page = self.spec.pages.iter().find(|p| p.key == key);
        let mut sections = Vec::new();
        if let Some(page) = page {
            for section in &page.sections {
                if let Some(panel) = self.mixer_section(section, cx) {
                    sections.push(panel);
                    continue;
                }
                if matches!(self.spec.product_id, 1303 | 1304) && key == "TAB_LIGHTING" {
                    if section.title == "EFFECTS" {
                        sections.push(self.render_nommo_effects(window, cx));
                        continue;
                    }
                    if let Some(panel) = self.nommo_lighting_section(section, window, cx) {
                        sections.push(panel);
                        continue;
                    }
                }
                if section
                    .visible_when
                    .as_ref()
                    .is_some_and(|c| self.draft.pointer(&c.path) != Some(&c.value))
                {
                    continue;
                }
                let mut panel =
                    surface::panel(format!("{}{}", t(&section.title), section.suffix), cx).gap_5();
                if let Some(description) = &section.description {
                    panel = panel.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(t(description)),
                    );
                }
                let dsp_mode_base = if self.spec.product_id == 1342 {
                    match section.title.as_str() {
                        "NOISE_GATE" => Some("/device/noiseGate"),
                        "COMPRESSOR" => Some("/device/compressor"),
                        _ => None,
                    }
                } else {
                    None
                };
                panel = panel.children(
                    section
                        .controls
                        .iter()
                        .filter(|control| {
                            dsp_mode_base.is_none()
                                || super::audio_mixer::accepts_mode(&control.path, &self.draft)
                        })
                        .map(|c| self.render_control(c, cx)),
                );
                if let Some(base) = dsp_mode_base {
                    let path = format!("{base}/useMode");
                    let advanced = self.draft.pointer(&path).and_then(Value::as_u64) == Some(1);
                    panel = panel.child(
                        Button::new(SharedString::from(format!("audio-dsp-mode-{base}")))
                            .ghost()
                            .label(t(if advanced { "SHOW_LESS" } else { "SHOW_MORE" }))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit(&path, json!(u8::from(!advanced)), window, cx);
                            })),
                    );
                }
                if self.spec.product_id == 1352 && section.title == "VOLUME_HEADER" {
                    panel = panel.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.volume_status()),
                    );
                }
                if self.mixer.is_some()
                    && key == "STREAM_MIXER_HEADER"
                    && section.title == "PLAYBACK_MIX"
                {
                    panel = panel.child(self.mixer_playback_selector(cx));
                }
                if let Some(key) = &section.equalizer {
                    panel = panel.child(self.render_equalizer(key, cx));
                }
                sections.push(panel.into_any_element());
            }
        }
        if sections.is_empty() {
            sections.push(surface::note("此页面尚未完成。", cx).into_any_element());
        }
        // Current Nommo wm / kM: the left Il / bl column holds brightness
        // followed by switch-off-lighting; the right column holds effects.
        // The former descriptor renderer incorrectly created three columns.
        if matches!(self.spec.product_id, 1303 | 1304)
            && key == "TAB_LIGHTING"
            && sections.len() == 3
        {
            let effects = sections.pop().unwrap();
            return surface::page_columns()
                .child(surface::page_column(v_flex().children(sections)))
                .child(surface::page_column(effects))
                .into_any_element();
        }
        surface::page_columns()
            .children(sections.into_iter().map(surface::page_column))
            .into_any_element()
    }
}
impl Render for AudioProductWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if matches!(self.spec.product_id, 1303 | 1304) && self.page == "TAB_LIGHTING" {
            if let Some(lighting) = self.lighting_element(window, cx) {
                return lighting;
            }
        }
        if self.page == "TAB_DEMO" {
            if let Some(demo) = &self.demo {
                return demo.clone().into_any_element();
            }
        }
        div()
            .id(SharedString::from(format!(
                "audio-product-page-{}-{}",
                self.spec.product_id, self.page
            )))
            .test_support()
            .child(
                super::product_surface::body()
                    .child(self.page_body(&self.page, window, cx))
                    .children(self.oled_home_dialog())
                    .children(self.mixer_dialog(window, cx)),
            )
            .into_any_element()
    }
}
fn normalized(value: f32, min: f32, max: f32, step: f32) -> Value {
    let value = (min + ((value.clamp(min, max) - min) / step).round() * step).clamp(min, max);
    if step.fract() == 0. {
        json!(value as i64)
    } else {
        json!(value)
    }
}
fn merge_known(target: &mut Value, saved: &Value) {
    match (target, saved) {
        (Value::Object(target), Value::Object(saved)) => {
            for (key, value) in target {
                if let Some(saved) = saved.get(key) {
                    merge_known(value, saved);
                }
            }
        }
        (Value::Array(target), Value::Array(saved)) if target.len() == saved.len() => {
            for (target, saved) in target.iter_mut().zip(saved) {
                merge_known(target, saved);
            }
        }
        (target, saved)
            if target.is_boolean() && saved.is_boolean()
                || target.is_number() && saved.is_number()
                || target.is_string() && saved.is_string() =>
        {
            *target = saved.clone()
        }
        _ => {}
    }
}

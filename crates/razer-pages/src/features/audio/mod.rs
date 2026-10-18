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

#[path = "control_pod/control_pod_audio.rs"]
mod control_pod_audio;
pub use control_pod_audio::RuntimeAudioDevice;
#[path = "mixer/audio_mixer_channels.rs"]
mod audio_mixer_channels;
#[path = "mixer/audio_mixer_customize.rs"]
mod audio_mixer_customize;
#[path = "mixer/audio_mixer_dynamics.rs"]
mod audio_mixer_dynamics;
#[path = "mixer/audio_mixer_effects_page.rs"]
mod audio_mixer_effects_page;
#[path = "mixer/audio_mixer_hardware.rs"]
mod audio_mixer_hardware;
#[path = "mixer/audio_mixer_knob.rs"]
mod audio_mixer_knob;
#[path = "mixer/audio_mixer_lighting.rs"]
mod audio_mixer_lighting;
#[path = "mixer/audio_mixer_local_state.rs"]
mod audio_mixer_local_state;
#[path = "mixer/audio_mixer_meter.rs"]
mod audio_mixer_meter;
#[path = "mixer/audio_mixer_mic.rs"]
mod audio_mixer_mic;
#[path = "mixer/audio_mixer_mic_eq.rs"]
mod audio_mixer_mic_eq;
#[path = "mixer/audio_mixer_number.rs"]
mod audio_mixer_number;
#[path = "mixer/audio_mixer_presets.rs"]
mod audio_mixer_presets;
#[path = "mixer/audio_mixer_theme.rs"]
mod audio_mixer_theme;
#[path = "seiren/audio_seiren_audio_runtime.rs"]
mod audio_seiren_audio_runtime;
#[path = "seiren/audio_seiren_eq.rs"]
mod audio_seiren_eq;
#[path = "seiren/audio_seiren_eq_write.rs"]
mod audio_seiren_eq_write;
#[path = "seiren/audio_seiren_microphone.rs"]
mod audio_seiren_microphone;
#[path = "seiren/audio_seiren_stream_mixer.rs"]
mod audio_seiren_stream_mixer;
pub use audio_mixer_presets::{PresetShortcutBinding, PresetShortcutRequest};
pub use audio_seiren_audio_runtime::SeirenAudioCaptureRequested;
pub use audio_seiren_eq_write::SeirenEqRequested;
pub use audio_seiren_microphone::{SeirenAudioPlaybackRequested, SeirenMicrophoneSetupSeen};
pub use audio_seiren_stream_mixer::{
    SeirenStreamMixerCaptureChanged, SeirenStreamMixerObservation, SeirenStreamMixerRequested,
};
mod audio_media;
#[path = "audio_demo.rs"]
mod demo;
#[path = "nommo/audio_nommo.rs"]
mod nommo;
#[path = "nommo/audio_nommo_effects.rs"]
mod nommo_effects;
#[path = "oled/audio_oled.rs"]
mod oled;
#[path = "oled/audio_oled_home.rs"]
mod oled_home;
#[path = "mixer/stream_mixer.rs"]
mod stream_mixer;
#[path = "mixer/stream_mixer_number.rs"]
mod stream_mixer_number;
pub use oled_home::{OledRuntimeObservation, OledRuntimeRequested};
pub use stream_mixer::StreamMixerObservation;
#[path = "audio_leviathan.rs"]
mod audio_leviathan;
#[path = "audio_volume.rs"]
mod audio_volume;
#[path = "audio_cloud_eq.rs"]
mod cloud_eq;
#[path = "audio_enhancements.rs"]
mod enhancements;
#[path = "sound/audio_sound.rs"]
mod sound;
#[path = "sound/audio_sound_applications.rs"]
mod sound_applications;
#[path = "sound/audio_sound_customizer.rs"]
mod sound_customizer;
#[path = "sound/audio_sound_mapping.rs"]
mod sound_mapping;
pub use super::audio_mixer::{
    AudioMixerCompletion, AudioMixerOperation, AudioMixerReply, AudioMixerRequest, AudioMixerState,
    path as mixer_path,
};
pub use audio_leviathan::{LeviathanObservation, LeviathanOperation, LeviathanRequest};
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
    static PRODUCTS: super::source_catalog::SourceCatalog<AudioProductSpec> =
        super::source_catalog::SourceCatalog::new(|| read_audio_products_catalog());
    PRODUCTS.get(pid)
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
        .get_or_init(|| read_audio_chroma_modes_catalog())
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
    mixer_room_input: Option<Entity<gpui_kit::component::input::InputState>>,
    mixer_dynamics: audio_mixer_dynamics::State,
    mixer_mic: Option<audio_mixer_mic::State>,
    mixer_customize: audio_mixer_customize::State,
    mixer_lighting: audio_mixer_lighting::State,
    mixer_channels: audio_mixer_channels::State,
    mixer_meter: audio_mixer_meter::State,
    mixer_hardware: audio_mixer_hardware::State,
    mixer: Option<stream_mixer::MixerState>,
    selects: BTreeMap<String, Entity<SelectState<Vec<Choice>>>>,
    subscriptions: Vec<Subscription>,
    source_owner: Option<Entity<super::source_help_owner::SourceHelpOwner>>,
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
    leviathan: audio_leviathan::State,
    pub(super) mixer_io: AudioMixerState,
    mixer_observed: BTreeMap<String, MixerValue>,
    mixer_read_queue: VecDeque<String>,
    mixer_write_queue: VecDeque<(String, MixerValue)>,
    mixer_eq_queue: Option<[i32; 10]>,
    mixer_local_queue: VecDeque<(String, Value)>,
    mixer_profile_routing: Option<audio_mixer_local_state::ProfileRouting>,
    mixer_local_receipts: BTreeMap<String, Vec<razer_device::audio_mixer_local::Receipt>>,
    mixer_revisions: BTreeMap<String, u64>,
    mixer_error: Option<String>,
    mixer_echo_timer: Option<Task<()>>,
    mixer_echo_generation: u64,
    mixer_echo_committed: Option<Value>,
    effect_presets: Option<audio_mixer_presets::State>,
    enhancements: enhancements::State,
    sound: sound::State,
    seiren_eq: audio_seiren_eq::State,
    seiren_eq_write: audio_seiren_eq_write::State,
    seiren_audio: audio_seiren_audio_runtime::State,
    seiren_mic: audio_seiren_microphone::State,
    seiren_stream: Option<audio_seiren_stream_mixer::State>,
}
impl EventEmitter<AudioProductChanged> for AudioProductWorkspace {}
impl EventEmitter<AudioStudioRequested> for AudioProductWorkspace {}
impl EventEmitter<AudioMixerRequest> for AudioProductWorkspace {}
impl EventEmitter<AudioNavigation> for AudioProductWorkspace {}

impl AudioProductWorkspace {
    pub fn floating_demo(&self, cx: &App) -> Option<AnyElement> {
        self.demo
            .as_ref()
            .filter(|demo| demo.read(cx).is_floating())
            .map(|demo| demo.clone().into_any_element())
    }
    pub fn stop_media(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(demo) = &self.demo {
            demo.update(cx, |demo, cx| demo.stop(window, cx));
        }
        if let Some(media) = &self.leviathan.media {
            media.update(cx, |media, cx| media.stop(window, cx));
        }
    }
    pub(super) fn set_source_owner(
        &mut self,
        owner: Entity<super::source_help_owner::SourceHelpOwner>,
        cx: &mut Context<Self>,
    ) {
        self.subscriptions
            .push(cx.observe(&owner, |_, _, cx| cx.notify()));
        self.source_owner = Some(owner);
        if self.page == "TAB_SOUND" {
            self.request_current_cloud_eq(cx);
        }
    }
    pub fn new(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = source_product(pid).expect("audited audio product");
        let mut this = Self {
            spec,
            page: spec.pages.first().map_or(String::new(), |p| p.key.clone()),
            draft: spec.draft.clone(),
            sliders: BTreeMap::new(),
            mixer_numbers: BTreeMap::new(),
            mixer_room_input: None,
            mixer_dynamics: audio_mixer_dynamics::State::default(),
            mixer_mic: None,
            mixer_customize: audio_mixer_customize::State::default(),
            mixer_lighting: audio_mixer_lighting::State::default(),
            mixer_channels: audio_mixer_channels::State::default(),
            mixer_meter: audio_mixer_meter::State::default(),
            mixer_hardware: audio_mixer_hardware::State::default(),
            mixer: stream_mixer::MixerState::new(pid, window, cx),
            selects: BTreeMap::new(),
            subscriptions: Vec::new(),
            source_owner: None,
            syncing: false,
            staged: BTreeMap::new(),
            selected_region: 0,
            nommo_brightness_dragging: false,
            nommo_effects: nommo_effects::NommoEffectsState::new(pid, window, cx),
            oled_home: oled_home::OledHomeState::new(pid),
            demo: demo::AudioDemo::for_product(pid, window, cx),
            pod_audio_editor: None,
            pod_audio_subscription: None,
            pod_runtime_devices: Vec::new(),
            volume: audio_volume::State::default(),
            leviathan: audio_leviathan::State::new(pid, window, cx),
            mixer_io: AudioMixerState::default(),
            mixer_observed: BTreeMap::new(),
            mixer_read_queue: VecDeque::new(),
            mixer_write_queue: VecDeque::new(),
            mixer_eq_queue: None,
            mixer_local_queue: VecDeque::new(),
            mixer_profile_routing: None,
            mixer_local_receipts: BTreeMap::new(),
            mixer_revisions: BTreeMap::new(),
            mixer_error: None,
            mixer_echo_timer: None,
            mixer_echo_generation: 0,
            mixer_echo_committed: None,
            effect_presets: None,
            enhancements: enhancements::State::default(),
            sound: sound::State::default(),
            seiren_eq: audio_seiren_eq::State::default(),
            seiren_eq_write: audio_seiren_eq_write::State::default(),
            seiren_audio: audio_seiren_audio_runtime::State::default(),
            seiren_mic: audio_seiren_microphone::State::default(),
            seiren_stream: None,
        };
        this.initialize_seiren_eq_draft();
        this.initialize_mixer_mic_draft();
        this.initialize_mixer_lighting_draft();
        this.initialize_mixer_channels(window, cx);
        this.initialize_equalizers();
        this.initialize_enhancements();
        this.initialize_sound();
        this.initialize_sound_sliders(window, cx);
        this.initialize_nommo_draft();
        this.initialize_oled_home(window, cx);
        this.subscribe_nommo_effects(window, cx);
        this.subscribe_mixer(window, cx);
        this.subscribe_leviathan(window, cx);
        for control in spec
            .pages
            .iter()
            .flat_map(|p| &p.sections)
            .flat_map(|s| &s.controls)
        {
            if pid == 1342 && control.path.starts_with("/device/mixerBind/") {
                continue;
            }
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
                    .map(|o| Choice::localized(o.value.to_string(), o.label.clone()))
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
        this.initialize_mixer_customize(window, cx);
        this.initialize_mixer_lighting(window, cx);
        this.initialize_mixer_effect_inputs(window, cx);
        this.initialize_mixer_dynamics(window, cx);
        this.initialize_mixer_mic(window, cx);
        this.initialize_seiren_eq(window, cx);
        this.initialize_seiren_microphone(window, cx);
        this.initialize_seiren_stream_mixer(window, cx);
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
        if let Some(routing) = &self.mixer_profile_routing {
            routing.apply(&mut snapshot);
        }
        if let Some(committed) = &self.mixer_echo_committed {
            snapshot["device"]["echoReverb"] = committed.clone();
        }
        self.mixer_snapshot(&mut snapshot);
        self.effect_preset_snapshot(&mut snapshot);
        self.seiren_eq_snapshot(&mut snapshot);
        snapshot
    }
    pub fn restore(&mut self, saved: Option<&Value>, window: &mut Window, cx: &mut Context<Self>) {
        self.restore_profile(saved, true, window, cx);
    }
    /// Refresh the local reducer after reset persistence without starting a
    /// getter while the separate firmware reset is still outstanding.
    pub fn restore_after_hardware_reset(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.restore_profile(saved, false, window, cx);
    }
    fn restore_profile(
        &mut self,
        saved: Option<&Value>,
        refresh_volume: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_seiren_eq(cx);
        self.cancel_seiren_eq_profile();
        self.cancel_leviathan(cx);
        self.invalidate_volume();
        self.invalidate_mixer();
        self.mixer_profile_routing = None;
        self.restore_mixer(saved, window, cx);
        let staged_oled_language = self.staged.get("/device/oledLanguage").cloned();
        self.pod_audio_editor = None;
        self.pod_audio_subscription = None;
        self.draft = self.spec.draft.clone();
        self.initialize_seiren_eq_draft();
        self.initialize_mixer_mic_draft();
        self.initialize_mixer_lighting_draft();
        self.selected_region = 0;
        self.staged.clear();
        if let Some(value) = staged_oled_language {
            self.staged.insert("/device/oledLanguage".into(), value);
        }
        self.nommo_brightness_dragging = false;
        self.initialize_equalizers();
        self.initialize_enhancements();
        self.initialize_sound();
        self.initialize_nommo_draft();
        self.initialize_oled_home(window, cx);
        if let Some(saved) = saved.filter(|v| v.is_object()) {
            merge_known(&mut self.draft, saved);
            if self.spec.product_id == 1382 {
                self.restore_pod_audio(saved);
            }
            self.restore_oled_home_saved(saved);
        }
        if self.spec.product_id == 1342 {
            // Current il()'s micPreview restore always publishes false; it is
            // never resumed from a previously saved preview flag.
            self.draft["device"]["micPreview"] = json!(false);
        }
        self.restore_seiren_eq(saved, window, cx);
        self.restore_leviathan(saved, window, cx);
        self.normalize_nommo_draft();
        self.restore_mixer_lighting(saved);
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
            if matches!(self.spec.product_id, 1422 | 1446)
                && control.path.starts_with("/device/parametric/")
            {
                continue;
            }
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
                if self.spec.product_id == 1342 && control.path.starts_with("/device/mixerBind/") {
                    // bP's buttons also accept local Effects preset GUIDs and
                    // wP only falls back visually for unresolved identities.
                    continue;
                }
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
        self.restore_enhancement_device();
        self.restore_sound_device(saved);
        if self.spec.product_id == 1342 && self.mixer_io.active {
            self.submit_restored_effect_preset(cx);
            self.sync_mixer_meter_activity(cx);
            self.sync_mixer_hardware_activity(cx);
        }
        self.sync(window, cx);
        self.reset_mixer_numbers(window, cx);
        if refresh_volume {
            self.request_volume_read(cx);
        }
        cx.notify();
    }
    pub fn set_page(&mut self, page: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != page {
            if let Some(demo) = &self.demo {
                demo.update(cx, |demo, cx| demo.set_page(page == "TAB_DEMO", window, cx));
            }
            if self.page == "TAB_EQ" {
                self.dismiss_seiren_eq(cx);
            }
            self.dismiss_mixer_lighting();
            self.stop_preset_shortcut_capture(cx);
            self.dismiss_seiren_stream_mixer(cx);
            self.dismiss_effect_preset_confirmation();
            if self.page == "TAB_SOUND" && self.spec.product_id == 1352 {
                if let Some(media) = &self.leviathan.media {
                    media.update(cx, |media, cx| media.stop(window, cx));
                }
                self.dismiss_leviathan_page();
            }
            self.dismiss_mixer_warning(window, cx);
            self.pod_audio_editor = None;
            self.pod_audio_subscription = None;
            if self.page == "TAB_OLED" || page == "TAB_OLED" {
                self.cancel_oled_editor(cx);
                // Bv's staged selection is local to the mounted OLED page.
                self.staged.remove("/device/oledLanguage");
                self.sync_oled_runtime_select(window, cx);
            }
            self.page = page.into();
            if page == "TAB_LIGHTING" {
                self.refresh_mixer_lighting_previews(cx);
            }
            self.sync_seiren_audio_runtime(cx);
            if page == "TAB_EQ" {
                self.sync_seiren_eq(window, cx);
            }
            if page == "TAB_CUSTOMIZE" && self.spec.product_id == 1342 {
                self.sync(window, cx);
            }
            if page == "TAB_MIXER" && self.spec.product_id == 1342 {
                self.mixer_channels.remount();
                self.sync(window, cx);
            }
            self.sync_mixer_meter_activity(cx);
            self.sync_mixer_hardware_activity(cx);
            if page == "TAB_ENHANCEMENT" {
                self.remount_enhancements();
                self.sync(window, cx);
            }
            if page == "TAB_SOUND" && sound::product(self.spec.product_id).is_some() {
                self.remount_sound();
                self.request_current_cloud_eq(cx);
                self.sync(window, cx);
            }
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
                if this.enhancement_slider_event(&key, event, window, cx) {
                    return;
                }
                if this.sound_slider_event(&key, event, window, cx) {
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
                if this.mixer_dynamics_slider(&key, event, window, cx) {
                    return;
                }
                if this.spec.product_id == 1342
                    && (key.starts_with("/device/echoReverb/")
                        || matches!(
                            key.as_str(),
                            "/device/keyShifter/value"
                                | "/device/vocalFading/value"
                                | "/device/micMonitor/value"
                        ))
                {
                    // Current bU/VU dispatch each cP change; useWU's O previews
                    // each change and resets its 500ms timer. None waits for release.
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
        if !self.mixer_dynamics.dispatching && self.control(path).is_some_and(|c| !self.enabled(c))
        {
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
        let echo_preview = self.spec.product_id == 1342 && path.starts_with("/device/echoReverb/");
        if echo_preview {
            let committed = self
                .mixer_echo_committed
                .get_or_insert_with(|| self.draft["device"]["echoReverb"].clone());
            // useWU.O starts every edit from E (the Redux value), not from
            // another unsent knob's local preview. Its shared timer keeps only
            // the last operation's source array.
            if path.starts_with("/device/echoReverb/modeValues/") {
                self.draft["device"]["echoReverb"]["modeValues"] = committed["modeValues"].clone();
            } else if path == "/device/echoReverb/isEnabled" {
                self.draft["device"]["echoReverb"] = committed.clone();
            }
        }
        let enabled = value == Value::Bool(true);
        if self.spec.product_id == 1342 {
            self.mixer_profile_routing.get_or_insert_with(|| {
                audio_mixer_local_state::ProfileRouting::from_draft(&self.draft)
            });
        }
        if let Some(target) = self.draft.pointer_mut(path) {
            *target = value;
        } else {
            return;
        }
        if let Some(routing) = &mut self.mixer_profile_routing {
            routing.retain_edit(path, &self.draft);
        }
        if path.starts_with("/device/outputMixerReducer/") {
            self.mixer_channels.observe_output();
        }
        if path == "/device/micPreview" {
            if let Some(mic) = &mut self.mixer_mic {
                mic.preview_popup = false;
            }
        }
        if self.spec.product_id == 1342 && path == "/device/echoReverb/activeMode" {
            let state = &mut self.draft["device"]["echoReverb"];
            let mode = state["activeMode"].as_str().unwrap_or("library");
            let bands = if mode == "custom" {
                state
                    .get("customValues")
                    .filter(|value| value.as_array().is_some_and(|values| !values.is_empty()))
                    .cloned()
                    .unwrap_or_else(|| json!(razer_device::audio_mixer::echo_presets()["library"]))
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
        if self.spec.product_id == 1342 && !echo_preview {
            self.retain_active_effect(path);
        }
        self.sync(window, cx);
        if !echo_preview {
            cx.emit(AudioProductChanged);
        }
        if self.spec.product_id == 1342 {
            if path.starts_with("/device/echoReverb/") {
                self.schedule_echo_write(window, cx);
            } else {
                self.request_mixer_write(path, self.draft.pointer(path).cloned(), cx);
            }
        }
        self.dispatch_seiren_microphone_control(path, cx);
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
                if this.mixer_io.epoch == epoch && this.mixer_echo_generation == generation {
                    this.mixer_echo_committed = None;
                    this.retain_active_effect("/device/echoReverb/isEnabled");
                    cx.emit(AudioProductChanged);
                    this.request_mixer_write("/device/echoReverb/isEnabled", None, cx);
                }
            });
        }));
    }

    /// Emit a source-verified HID DSP write for controls backed by
    /// the actual AudioMixer instance. Other endpoints are omitted by
    /// `mixer_path` until their distinct source caller chains are connected.
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
        if razer_device::audio_mixer_local::handles(path) {
            self.mixer_profile_routing.get_or_insert_with(|| {
                audio_mixer_local_state::ProfileRouting::from_draft(&self.draft)
            });
            let saved = self.snapshot();
            self.mixer_io.edit_revision = self.mixer_io.edit_revision.wrapping_add(1);
            self.mixer_revisions
                .insert(path.into(), self.mixer_io.edit_revision);
            // Keep one complete latest profile for each unsent source action.
            if let Some((_, profile)) = self
                .mixer_local_queue
                .iter_mut()
                .find(|(queued, _)| queued == path)
            {
                *profile = saved;
            } else {
                self.mixer_local_queue.push_back((path.into(), saved));
            }
            self.dispatch_next_mixer_read(cx);
            return;
        }
        let actions = match super::audio_mixer::write_plan(path, &self.draft) {
            Ok(actions) => actions,
            Err(error) => {
                eprintln!("Audio Mixer source action {path}: {error:#}");
                self.mixer_error = Some(format!("{error:#}"));
                return;
            }
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
            local_profile: None,
            local_lifetime: None,
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
            local_profile: None,
            local_lifetime: None,
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
        self.sync_mixer_meter_activity(cx);
        self.sync_mixer_hardware_activity(cx);
    }

    fn dispatch_next_mixer_read(&mut self, cx: &mut Context<Self>) {
        if !self.mixer_io.active || self.mixer_io.pending.is_some() {
            return;
        }
        // Local setup/routing, DSP writes and EQ share the source task queue.
        // Choose the oldest submitted action instead of always prioritizing
        // one transport, which reordered profile restore (monitor after EQ).
        let local_revision = self
            .mixer_local_queue
            .front()
            .map(|(path, _)| self.mixer_revisions.get(path).copied().unwrap_or(0));
        let eq_revision = self.mixer_eq_queue.as_ref().map(|_| {
            self.mixer_revisions
                .get("/equalizers/mic")
                .copied()
                .unwrap_or(0)
        });
        let dsp_revision = self
            .mixer_write_queue
            .front()
            .map(|(path, _)| self.mixer_revisions.get(path).copied().unwrap_or(0));
        let next_revision = [local_revision, eq_revision, dsp_revision]
            .into_iter()
            .flatten()
            .min();
        if local_revision.is_some() && local_revision == next_revision {
            let (path, profile) = self
                .mixer_local_queue
                .pop_front()
                .expect("queued local action");
            // MW Li and bi explicitly publish these reducer changes. They do
            // not replace the local profile with the temporary preview values.
            audio_mixer_local_state::apply_source_overlay(&mut self.draft, &profile, &path);
            if path == "/device/micPreview" {
                self.mixer_channels.observe_output();
            }
            self.mixer_io.generation = self.mixer_io.generation.wrapping_add(1);
            let request = AudioMixerRequest {
                generation: self.mixer_io.generation,
                operation: AudioMixerOperation::LocalWrite,
                edit_revision: self.mixer_revisions.get(&path).copied().unwrap_or(0),
                path,
                target: razer_device::audio_mixer::MixerTarget {
                    control: razer_device::audio_mixer::MixerControl::MicrophoneVolume,
                    band: None,
                    channel: razer_device::audio_mixer::MixerChannel::Both,
                },
                value: None,
                eq_bands: None,
                local_profile: Some(profile),
                local_lifetime: Some(self.mixer_io.local_lifetime.clone()),
                epoch: self.mixer_io.epoch,
            };
            self.mixer_io.pending = Some(request.clone());
            self.mixer_io.cancellation = Some(std::sync::Arc::new(
                std::sync::atomic::AtomicBool::new(false),
            ));
            cx.emit(request);
            return;
        }
        if eq_revision.is_some() && eq_revision == next_revision {
            let bands = self.mixer_eq_queue.take().expect("queued EQ action");
            self.mixer_io.generation = self.mixer_io.generation.wrapping_add(1);
            let request = AudioMixerRequest {
                generation: self.mixer_io.generation,
                operation: AudioMixerOperation::MicEqWrite,
                path: "/equalizers/mic".into(),
                target: super::audio_mixer::AudioMixerPath::MicEqEnabled.target(),
                value: None,
                eq_bands: Some(bands),
                local_profile: None,
                local_lifetime: None,
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
        self.mixer_channels.cancel();
        self.mixer_meter.cancel();
        self.mixer_hardware.cancel();
        self.mixer_io
            .local_lifetime
            .store(true, std::sync::atomic::Ordering::Release);
        self.mixer_io.local_lifetime =
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        if let Some(cancel) = &self.mixer_io.cancellation {
            cancel.store(true, std::sync::atomic::Ordering::Release);
        }
        self.mixer_io.epoch = self.mixer_io.epoch.wrapping_add(1);
        self.mixer_read_queue.clear();
        self.mixer_write_queue.clear();
        self.mixer_eq_queue = None;
        self.mixer_local_queue.clear();
        self.mixer_local_receipts.clear();
        self.mixer_echo_timer = None;
        self.mixer_echo_committed = None;
        self.mixer_dynamics.pending.clear();
        self.mixer_echo_generation = self.mixer_echo_generation.wrapping_add(1);
        self.mixer_observed.clear();
        self.mixer_error = None;
    }

    pub fn prepare_exit(&mut self, cx: &mut Context<Self>) {
        self.cancel_oled_editor(cx);
        self.stop_preset_shortcut_capture(cx);
        self.dismiss_seiren_stream_mixer(cx);
        self.invalidate_mixer();
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
        if scope_current && request.epoch == self.mixer_io.epoch {
            if let Ok(AudioMixerCompletion {
                reply: AudioMixerReply::LocalWrite(receipts),
                ..
            }) = &result
            {
                if let Some(routing) = &mut self.mixer_profile_routing {
                    let changed = receipts.iter().fold(false, |changed, receipt| {
                        routing.retain_service_updates(&receipt.profile_updates) || changed
                    });
                    if changed {
                        cx.emit(AudioProductChanged);
                    }
                }
            }
        }
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
                        AudioMixerReply::LocalWrite(receipts) => {
                            if let Some(profile) = &request.local_profile {
                                if audio_mixer_local_state::apply_setup_completion(
                                    &mut self.draft,
                                    profile,
                                    &request.path,
                                    &receipts,
                                ) {
                                    self.mixer_channels.observe_output();
                                }
                            }
                            self.mixer_hardware.receipts(&receipts);
                            if let Some(failed) =
                                receipts.iter().find(|receipt| !receipt.transport_completed)
                            {
                                self.mixer_error = Some(format!(
                                    "Mixer local command {:?}: {}",
                                    failed.command, failed.response
                                ));
                            }
                            self.mixer_local_receipts
                                .insert(request.path.clone(), receipts);
                            None
                        }
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
                eprintln!("Audio Mixer DSP: {error}");
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
        self.refresh_enhancements();
        self.syncing = true;
        self.sync_mixer(window, cx);
        self.sync_mixer_effect_inputs(window, cx);
        self.sync_mixer_dynamics(window, cx);
        self.sync_mixer_mic(window, cx);
        self.sync_mixer_customize(window, cx);
        self.sync_mixer_lighting(window, cx);
        self.sync_mixer_channels(window, cx);
        self.sync_mixer_numbers(window, cx);
        for (path, state) in &self.sliders {
            if self.preserves_enhancement_preview(path)
                || self.preserves_sound_preview(path)
                || self.preserves_mixer_dynamics(path)
            {
                continue;
            }
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
        self.sync_seiren_eq(window, cx);
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
        let key = if current_mic && basic { "mic_basic" } else { key };
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
                .child(self.page_body(&route.page, true, window, cx))
                .into_any_element(),
        )
    }

    fn page_body(
        &self,
        key: &str,
        lighting_popup: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(page) = self.seiren_eq_page(key, window, cx) {
            return page;
        }
        if let Some(page) = self.seiren_stream_mixer_page(key, window, cx) {
            return page;
        }
        if self.spec.product_id == 1342 && key == "TAB_CUSTOMIZE" {
            return self.mixer_customize_page(window, cx);
        }
        if self.spec.product_id == 1342 && key == "TAB_LIGHTING" {
            return self.mixer_lighting_page(window, cx);
        }
        if let Some(page) = self.current_sound_page(key, window, cx) {
            return page;
        }
        if let Some(page) = self.enhancements_page(key, cx) {
            return page;
        }
        // 1383 的 TAB_OLED 由自己的 OLED 根（`xx`）渲染：两列 `.widget-col` 与
        // 屏保网格都不是通用音频控件的形态，见 audio_oled.rs。
        if self.spec.product_id == 1383 && key == "TAB_OLED" {
            return self.kraken_oled_page(window, cx);
        }
        if self.spec.product_id == 1342 && key == "TAB_MIC" {
            return self.mixer_mic_page(cx);
        }
        if self.spec.product_id == 1342 && key == "TAB_MIXER" {
            return self.mixer_channels_page(cx);
        }
        if self.spec.product_id == 1342 && key == "EFFECTS" {
            return self.mixer_effects_page(window, cx);
        }
        if self.spec.product_id == 1352 && key == "TAB_SOUND" {
            return self.leviathan_sound_page(window, cx);
        }
        let page = self.spec.pages.iter().find(|p| p.key == key);
        let mut sections = Vec::new();
        if let Some(page) = page {
            for section in &page.sections {
                if self.spec.product_id == 1342
                    && matches!(section.title.as_str(), "NOISE_GATE" | "COMPRESSOR")
                {
                    sections.push(self.mixer_dynamics_panel(section, cx));
                    continue;
                }
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
        // Seiren V3 Pro's normal root loads chunk 684 before READY; its CSS
        // changes every .body-widgets to justify-content:start at <=1200px.
        // ChromaApp loads only the lighting chunks, without this override.
        // Complete callers and styles are retained in audio-shared-page-layout-audit.json.
        let seiren_compact = matches!(self.spec.product_id, 1422 | 1446)
            && !lighting_popup
            && f32::from(window.viewport_size().width) * 16. / f32::from(window.rem_size()).max(1.)
                <= 1200.;
        surface::page_columns()
            .when(seiren_compact, |columns| columns.justify_start())
            .children(sections.into_iter().map(surface::page_column))
            .into_any_element()
    }
}
impl Render for AudioProductWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.page == "TAB_SOUND" {
            self.refresh_sound_images();
        }
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
        if self.seiren_microphone_mounted() {
            let body = self.page_body(&self.page, false, window, cx);
            let footer = self.seiren_microphone_footer(cx);
            let wizard = self.seiren_microphone_wizard(window, cx);
            return v_flex()
                .id("seiren-page-with-footer")
                .size_full()
                .min_h_0()
                .child(
                    div()
                        .id("seiren-scrolling-page")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .child(super::product_surface::body().child(body)),
                )
                .children(footer)
                .children(wizard)
                .into_any_element();
        }
        div()
            .id(SharedString::from(format!(
                "audio-product-page-{}-{}",
                self.spec.product_id, self.page
            )))
            .test_support()
            .child(
                super::product_surface::body()
                    .child(self.page_body(&self.page, false, window, cx))
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

// Generated catalog reader: razer-pages/audio_chroma_modes.
fn read_audio_chroma_modes_catalog() -> Vec<ChromaLightingPage> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<ChromaLightingPage>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<ChromaLightingPage> {
        let value = match r.tag()? {
            0 => ChromaLightingPage {
                product_id: r.child(decode_2)?,
                page: r.child(decode_3)?,
                body_min_width: r.child(decode_4)?,
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
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_AUDIO_CHROMA_MODES,
        0,
        "81937cc2534015101066d09ea4cbedc9c5c7b276bdc592f36a569736c7fac517",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod audio_chroma_modes_catalog_tests {
    #[test]
    fn packaged_catalog_audio_chroma_modes_decodes() {
        let _ = super::read_audio_chroma_modes_catalog();
    }
}
// End generated catalog reader: razer-pages/audio_chroma_modes.

// Generated catalog reader: razer-pages/audio_products_data.
fn read_audio_products_catalog() -> Vec<(u32, fn() -> AudioProductSpec)> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<AudioProductSpec> {
        let value = match r.tag()? {
            0 => AudioProductSpec {
                product_id: r.child(decode_1)?,
                draft: r.child(decode_2)?,
                pages: r.child(decode_3)?,
                equalizers: r.child(decode_14)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<AudioPage>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<AudioPage> {
        let value = match r.tag()? {
            0 => AudioPage {
                key: r.child(decode_5)?,
                sections: r.child(decode_6)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<AudioSection>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_7)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<AudioSection> {
        let value = match r.tag()? {
            0 => AudioSection {
                title: r.child(decode_5)?,
                suffix: Default::default(),
                tips: None,
                description: None,
                controls: r.child(decode_8)?,
                equalizer: None,
                visible_when: None,
            },
            1 => AudioSection {
                title: r.child(decode_5)?,
                suffix: Default::default(),
                tips: None,
                description: r.child(decode_11)?,
                controls: r.child(decode_8)?,
                equalizer: None,
                visible_when: None,
            },
            2 => AudioSection {
                title: r.child(decode_5)?,
                suffix: Default::default(),
                tips: None,
                description: None,
                controls: r.child(decode_8)?,
                equalizer: r.child(decode_11)?,
                visible_when: None,
            },
            3 => AudioSection {
                title: r.child(decode_5)?,
                suffix: r.child(decode_5)?,
                tips: None,
                description: None,
                controls: r.child(decode_8)?,
                equalizer: None,
                visible_when: None,
            },
            4 => AudioSection {
                title: r.child(decode_5)?,
                suffix: Default::default(),
                tips: r.child(decode_11)?,
                description: r.child(decode_11)?,
                controls: r.child(decode_8)?,
                equalizer: None,
                visible_when: None,
            },
            5 => AudioSection {
                title: r.child(decode_5)?,
                suffix: Default::default(),
                tips: None,
                description: None,
                controls: r.child(decode_8)?,
                equalizer: None,
                visible_when: r.child(decode_21)?,
            },
            6 => AudioSection {
                title: r.child(decode_5)?,
                suffix: r.child(decode_5)?,
                tips: None,
                description: None,
                controls: r.child(decode_8)?,
                equalizer: None,
                visible_when: r.child(decode_21)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<AudioControl>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_9)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<AudioControl> {
        let value = match r.tag()? {
            0 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: Default::default(),
                max: Default::default(),
                step: one(),
                unit: Default::default(),
                options: Default::default(),
                enabled_by: None,
                enabled_all: Default::default(),
                exclusive_with: Default::default(),
                url: None,
            },
            1 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: r.child(decode_10)?,
                max: r.child(decode_10)?,
                step: r.child(decode_10)?,
                unit: r.child(decode_5)?,
                options: Default::default(),
                enabled_by: r.child(decode_11)?,
                enabled_all: Default::default(),
                exclusive_with: Default::default(),
                url: None,
            },
            2 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: Default::default(),
                max: Default::default(),
                step: one(),
                unit: Default::default(),
                options: Default::default(),
                enabled_by: r.child(decode_11)?,
                enabled_all: Default::default(),
                exclusive_with: Default::default(),
                url: None,
            },
            3 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: Default::default(),
                max: Default::default(),
                step: one(),
                unit: Default::default(),
                options: r.child(decode_12)?,
                enabled_by: None,
                enabled_all: Default::default(),
                exclusive_with: Default::default(),
                url: None,
            },
            4 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: r.child(decode_10)?,
                max: r.child(decode_10)?,
                step: r.child(decode_10)?,
                unit: Default::default(),
                options: Default::default(),
                enabled_by: r.child(decode_11)?,
                enabled_all: r.child(decode_16)?,
                exclusive_with: Default::default(),
                url: None,
            },
            5 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: Default::default(),
                max: Default::default(),
                step: one(),
                unit: Default::default(),
                options: Default::default(),
                enabled_by: None,
                enabled_all: Default::default(),
                exclusive_with: Default::default(),
                url: r.child(decode_11)?,
            },
            6 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: r.child(decode_10)?,
                max: r.child(decode_10)?,
                step: r.child(decode_10)?,
                unit: Default::default(),
                options: Default::default(),
                enabled_by: r.child(decode_11)?,
                enabled_all: Default::default(),
                exclusive_with: Default::default(),
                url: None,
            },
            7 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: Default::default(),
                max: Default::default(),
                step: one(),
                unit: Default::default(),
                options: r.child(decode_12)?,
                enabled_by: r.child(decode_11)?,
                enabled_all: Default::default(),
                exclusive_with: Default::default(),
                url: None,
            },
            8 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: Default::default(),
                max: Default::default(),
                step: one(),
                unit: Default::default(),
                options: Default::default(),
                enabled_by: None,
                enabled_all: Default::default(),
                exclusive_with: r.child(decode_16)?,
                url: None,
            },
            9 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: r.child(decode_10)?,
                max: r.child(decode_10)?,
                step: r.child(decode_10)?,
                unit: Default::default(),
                options: Default::default(),
                enabled_by: None,
                enabled_all: Default::default(),
                exclusive_with: Default::default(),
                url: None,
            },
            10 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: r.child(decode_11)?,
                min: Default::default(),
                max: Default::default(),
                step: one(),
                unit: Default::default(),
                options: r.child(decode_12)?,
                enabled_by: None,
                enabled_all: Default::default(),
                exclusive_with: Default::default(),
                url: None,
            },
            11 => AudioControl {
                path: r.child(decode_5)?,
                label: r.child(decode_5)?,
                kind: r.child(decode_5)?,
                apply_label: None,
                min: r.child(decode_10)?,
                max: r.child(decode_10)?,
                step: r.child(decode_10)?,
                unit: r.child(decode_5)?,
                options: Default::default(),
                enabled_by: None,
                enabled_all: Default::default(),
                exclusive_with: Default::default(),
                url: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_5)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<AudioOption>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_13)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<AudioOption> {
        let value = match r.tag()? {
            0 => AudioOption {
                label: r.child(decode_5)?,
                value: r.child(decode_2)?,
                image: None,
            },
            1 => AudioOption {
                label: r.child(decode_5)?,
                value: r.child(decode_2)?,
                image: r.child(decode_11)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<AudioEqualizer>> {
        let value = match r.tag()? {
            0 => Default::default(),
            1 => r.sequence(decode_15)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_15(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<AudioEqualizer> {
        let value = match r.tag()? {
            0 => AudioEqualizer {
                key: r.child(decode_5)?,
                frequencies: r.child(decode_16)?,
                presets: r.child(decode_17)?,
                selected: r.child(decode_5)?,
                min: r.child(decode_10)?,
                max: r.child(decode_10)?,
                step: r.child(decode_10)?,
                edit_custom: r.child(decode_20)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_16(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_17(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<AudioPreset>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_18)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_18(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<AudioPreset> {
        let value = match r.tag()? {
            0 => AudioPreset {
                key: r.child(decode_5)?,
                label: r.child(decode_5)?,
                bands: r.child(decode_19)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_19(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<f32>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_10)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_20(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_21(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<AudioCondition>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_22)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_22(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<AudioCondition> {
        let value = match r.tag()? {
            0 => AudioCondition {
                path: r.child(decode_5)?,
                value: r.child(decode_2)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn product_1303() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1303u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1304() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1304u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1306() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1306u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1308() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1308u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1310() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1310u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1312() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1312u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1313() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1313u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1318() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1318u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1319() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1319u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1320() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1320u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1321() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1321u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1325() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1325u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1328() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1328u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1330() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1330u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1331() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1331u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1332() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1332u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1335() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1335u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1337() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1337u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1339() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1339u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1342() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1342u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1346() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1346u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1347() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1347u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1352() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1352u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1353() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1353u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1354() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1354u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1364() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1364u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1365() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1365u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1370() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1370u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1372() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1372u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1374() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1374u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1376() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1376u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1378() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1378u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1383() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1383u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1386() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1386u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1387() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1387u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1389() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1389u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1390() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1390u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1391() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1391u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1392() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1392u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1396() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1396u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1398() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1398u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1401() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1401u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1404() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1404u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1406() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1406u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1407() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1407u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1411() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1411u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1412() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1412u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1415() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1415u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1420() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1420u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1422() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1422u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1427() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1427u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1439() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1439u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1442() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1442u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1443() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1443u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1446() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1446u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1453() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1453u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1462() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1462u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1465() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1465u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2638() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            2638u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2641() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            2641u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2644() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            2644u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2657() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            2657u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2660() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            2660u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2664() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            2664u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2668() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            2668u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_2689() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            2689u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_3867() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            3867u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_4124() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            4124u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_4126() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            4126u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_4130() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            4130u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_1382() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            1382u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_3334() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            3334u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_3337() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            3337u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_3909() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            3909u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_3942() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            3942u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    fn product_3949() -> AudioProductSpec {
        razer_resource_db::read_document(
            razer_resource_db::catalogs::RAZER_PAGES_AUDIO_PRODUCTS_DATA,
            3949u32,
            "af1e2fa43695ed83984db2b7467c6c72f446ac5ed45201bc824fcfe34823c2f9",
            decode_0,
        )
        .expect("Packaged typed catalog must match its domain schema")
    }
    vec![
        (1303u32, product_1303 as fn() -> AudioProductSpec),
        (1304u32, product_1304 as fn() -> AudioProductSpec),
        (1306u32, product_1306 as fn() -> AudioProductSpec),
        (1308u32, product_1308 as fn() -> AudioProductSpec),
        (1310u32, product_1310 as fn() -> AudioProductSpec),
        (1312u32, product_1312 as fn() -> AudioProductSpec),
        (1313u32, product_1313 as fn() -> AudioProductSpec),
        (1318u32, product_1318 as fn() -> AudioProductSpec),
        (1319u32, product_1319 as fn() -> AudioProductSpec),
        (1320u32, product_1320 as fn() -> AudioProductSpec),
        (1321u32, product_1321 as fn() -> AudioProductSpec),
        (1325u32, product_1325 as fn() -> AudioProductSpec),
        (1328u32, product_1328 as fn() -> AudioProductSpec),
        (1330u32, product_1330 as fn() -> AudioProductSpec),
        (1331u32, product_1331 as fn() -> AudioProductSpec),
        (1332u32, product_1332 as fn() -> AudioProductSpec),
        (1335u32, product_1335 as fn() -> AudioProductSpec),
        (1337u32, product_1337 as fn() -> AudioProductSpec),
        (1339u32, product_1339 as fn() -> AudioProductSpec),
        (1342u32, product_1342 as fn() -> AudioProductSpec),
        (1346u32, product_1346 as fn() -> AudioProductSpec),
        (1347u32, product_1347 as fn() -> AudioProductSpec),
        (1352u32, product_1352 as fn() -> AudioProductSpec),
        (1353u32, product_1353 as fn() -> AudioProductSpec),
        (1354u32, product_1354 as fn() -> AudioProductSpec),
        (1364u32, product_1364 as fn() -> AudioProductSpec),
        (1365u32, product_1365 as fn() -> AudioProductSpec),
        (1370u32, product_1370 as fn() -> AudioProductSpec),
        (1372u32, product_1372 as fn() -> AudioProductSpec),
        (1374u32, product_1374 as fn() -> AudioProductSpec),
        (1376u32, product_1376 as fn() -> AudioProductSpec),
        (1378u32, product_1378 as fn() -> AudioProductSpec),
        (1383u32, product_1383 as fn() -> AudioProductSpec),
        (1386u32, product_1386 as fn() -> AudioProductSpec),
        (1387u32, product_1387 as fn() -> AudioProductSpec),
        (1389u32, product_1389 as fn() -> AudioProductSpec),
        (1390u32, product_1390 as fn() -> AudioProductSpec),
        (1391u32, product_1391 as fn() -> AudioProductSpec),
        (1392u32, product_1392 as fn() -> AudioProductSpec),
        (1396u32, product_1396 as fn() -> AudioProductSpec),
        (1398u32, product_1398 as fn() -> AudioProductSpec),
        (1401u32, product_1401 as fn() -> AudioProductSpec),
        (1404u32, product_1404 as fn() -> AudioProductSpec),
        (1406u32, product_1406 as fn() -> AudioProductSpec),
        (1407u32, product_1407 as fn() -> AudioProductSpec),
        (1411u32, product_1411 as fn() -> AudioProductSpec),
        (1412u32, product_1412 as fn() -> AudioProductSpec),
        (1415u32, product_1415 as fn() -> AudioProductSpec),
        (1420u32, product_1420 as fn() -> AudioProductSpec),
        (1422u32, product_1422 as fn() -> AudioProductSpec),
        (1427u32, product_1427 as fn() -> AudioProductSpec),
        (1439u32, product_1439 as fn() -> AudioProductSpec),
        (1442u32, product_1442 as fn() -> AudioProductSpec),
        (1443u32, product_1443 as fn() -> AudioProductSpec),
        (1446u32, product_1446 as fn() -> AudioProductSpec),
        (1453u32, product_1453 as fn() -> AudioProductSpec),
        (1462u32, product_1462 as fn() -> AudioProductSpec),
        (1465u32, product_1465 as fn() -> AudioProductSpec),
        (2638u32, product_2638 as fn() -> AudioProductSpec),
        (2641u32, product_2641 as fn() -> AudioProductSpec),
        (2644u32, product_2644 as fn() -> AudioProductSpec),
        (2657u32, product_2657 as fn() -> AudioProductSpec),
        (2660u32, product_2660 as fn() -> AudioProductSpec),
        (2664u32, product_2664 as fn() -> AudioProductSpec),
        (2668u32, product_2668 as fn() -> AudioProductSpec),
        (2689u32, product_2689 as fn() -> AudioProductSpec),
        (3867u32, product_3867 as fn() -> AudioProductSpec),
        (4124u32, product_4124 as fn() -> AudioProductSpec),
        (4126u32, product_4126 as fn() -> AudioProductSpec),
        (4130u32, product_4130 as fn() -> AudioProductSpec),
        (1382u32, product_1382 as fn() -> AudioProductSpec),
        (3334u32, product_3334 as fn() -> AudioProductSpec),
        (3337u32, product_3337 as fn() -> AudioProductSpec),
        (3909u32, product_3909 as fn() -> AudioProductSpec),
        (3942u32, product_3942 as fn() -> AudioProductSpec),
        (3949u32, product_3949 as fn() -> AudioProductSpec),
    ]
}

#[cfg(test)]
mod audio_products_data_catalog_tests {
    #[test]
    fn packaged_catalog_audio_products_data_decodes() {
        for (_, factory) in super::read_audio_products_catalog() {
            let _ = factory();
        }
    }
}
// End generated catalog reader: razer-pages/audio_products_data.

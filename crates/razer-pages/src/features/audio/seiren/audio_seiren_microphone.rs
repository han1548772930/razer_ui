//! Current UI7405 sticky microphone owner. Live PCM, record/play and the setup
//! wizard have independent lifetimes and never synthesize device observations.
use super::*;
use audio_seiren_audio_runtime::SeirenAudioCaptureRequested;
use razer_device::{
    seiren_audio_capture::CaptureFrame,
    seiren_microphone::{self as mic, CaptureOptions, CapturePurpose, PcmClip},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

#[path = "audio_seiren_microphone_view.rs"]
mod view;
#[path = "audio_seiren_microphone_waveform.rs"]
mod waveform;
#[path = "audio_seiren_microphone_wizard.rs"]
mod wizard;

#[derive(Deserialize)]
struct Presentation {
    labels: BTreeMap<String, String>,
    assets: BTreeMap<String, String>,
}
fn presentation(pid: u32) -> &'static Presentation {
    static DATA: OnceLock<BTreeMap<String, Presentation>> = OnceLock::new();
    &DATA.get_or_init(|| {
        serde_json::from_str(include_str!("audio_seiren_microphone_data.json"))
            .expect("current microphone data")
    })[&pid.to_string()]
}
fn text(pid: u32, key: &str) -> SharedString {
    t(presentation(pid)
        .labels
        .get(key)
        .map(String::as_str)
        .unwrap_or(""))
    .into()
}
fn asset(pid: u32, key: &str) -> SharedString {
    presentation(pid).assets[key].clone().into()
}
fn supported(pid: u32) -> bool {
    matches!(pid, 1422 | 1446)
}

#[derive(Clone)]
pub struct SeirenMicrophoneSetupSeen(bool);
impl SeirenMicrophoneSetupSeen {
    pub fn seen(&self) -> bool {
        self.0
    }
}
impl EventEmitter<SeirenMicrophoneSetupSeen> for AudioProductWorkspace {}

static NEXT_PLAYBACK: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
pub struct SeirenAudioPlaybackRequested {
    owner: EntityId,
    session_id: u64,
    product_id: u32,
    clip: Arc<PcmClip>,
    canceled: Arc<AtomicBool>,
}
impl SeirenAudioPlaybackRequested {
    pub fn session_id(&self) -> u64 {
        self.session_id
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn clip(&self) -> &PcmClip {
        &self.clip
    }
    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }
    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.canceled.clone()
    }
    fn cancel(&self) {
        self.canceled.store(true, Ordering::Release);
    }
    fn same_owner(&self, other: &Self) -> bool {
        self.owner == other.owner && self.session_id == other.session_id
    }
}
impl EventEmitter<SeirenAudioPlaybackRequested> for AudioProductWorkspace {}

struct CaptureOwner {
    request: SeirenAudioCaptureRequested,
    rate: Option<u32>,
    samples: Vec<f32>,
    started: Option<Instant>,
    stopped: Option<Instant>,
    dispatched: bool,
    accept_final: bool,
    error: Option<String>,
}
impl CaptureOwner {
    fn new(request: SeirenAudioCaptureRequested) -> Self {
        Self {
            request,
            rate: None,
            samples: Vec::new(),
            started: None,
            stopped: None,
            dispatched: true,
            accept_final: false,
            error: None,
        }
    }
    fn stop(&mut self, accept_final: bool) {
        self.stopped.get_or_insert_with(Instant::now);
        self.accept_final = accept_final;
        self.request.cancel();
    }
}
pub(super) struct State {
    mounted: bool,
    access_delay: bool,
    delay: Option<Task<()>>,
    live: bool,
    native_capture_enabled: bool,
    preview: Option<CaptureOwner>,
    record: Option<CaptureOwner>,
    measurement: Option<CaptureOwner>,
    retry: Option<Task<()>>,
    retry_count: u8,
    waveform: waveform::State,
    waveform_task: Option<Task<()>>,
    clip: Option<Arc<PcmClip>>,
    playback: Option<SeirenAudioPlaybackRequested>,
    playing: bool,
    setup_seen: Option<bool>,
    wizard: Option<wizard::State>,
    wizard_defaults: Option<Task<()>>,
    measurement_timer: Option<Task<()>>,
    measurement_start_delay: Option<Task<()>>,
    measurement_last_stop: Option<Instant>,
    diagnostics: Vec<String>,
    recording_error: Option<String>,
    focus: Option<FocusHandle>,
    trigger_focus: Option<FocusHandle>,
    window: Option<AnyWindowHandle>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            mounted: false,
            access_delay: false,
            delay: None,
            live: true,
            native_capture_enabled: true,
            preview: None,
            record: None,
            measurement: None,
            retry: None,
            retry_count: 0,
            waveform: Default::default(),
            waveform_task: None,
            clip: None,
            playback: None,
            playing: false,
            setup_seen: None,
            wizard: None,
            wizard_defaults: None,
            measurement_timer: None,
            measurement_start_delay: None,
            measurement_last_stop: None,
            diagnostics: Vec::new(),
            recording_error: None,
            focus: None,
            trigger_focus: None,
            window: None,
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        for capture in [&self.preview, &self.record, &self.measurement]
            .into_iter()
            .flatten()
        {
            capture.request.cancel();
        }
        if let Some(playback) = &self.playback {
            playback.cancel();
        }
    }
}

impl AudioProductWorkspace {
    pub(super) fn initialize_seiren_microphone(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if supported(self.spec.product_id) {
            self.seiren_mic.window = Some(window.window_handle());
            self.seiren_mic.focus = Some(cx.focus_handle());
            self.seiren_mic.trigger_focus = Some(cx.focus_handle().tab_stop(true));
        }
    }
    pub(in crate::features) fn observe_seiren_sampling_ready(
        &mut self,
        ready: bool,
        cx: &mut Context<Self>,
    ) {
        self.seiren_audio.ready = ready;
        self.sync_seiren_audio_runtime(cx);
        cx.notify();
    }
    pub(in crate::features) fn set_seiren_microphone_setup_seen(
        &mut self,
        seen: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if supported(self.spec.product_id) && self.seiren_mic.setup_seen.is_none() {
            self.seiren_mic.setup_seen = Some(seen);
            if !seen {
                self.open_seiren_microphone_wizard(window, cx);
            }
            cx.notify();
        }
    }
    pub(super) fn seiren_microphone_mounted(&self) -> bool {
        supported(self.spec.product_id)
            && matches!(
                self.page.as_str(),
                "TAB_EQ" | "TAB_EFFECTS" | "STREAM_MIXER_HEADER" | "TAB_STREAM_MIX"
            )
    }
    pub(super) fn sync_seiren_microphone_runtime(&mut self, cx: &mut Context<Self>) {
        if !supported(self.spec.product_id) {
            return;
        }
        let mounted = self.seiren_microphone_mounted();
        if self.seiren_mic.mounted != mounted {
            self.seiren_mic.mounted = mounted;
            self.seiren_mic.delay = None;
            self.seiren_mic.access_delay = false;
            if mounted {
                self.seiren_mic.delay = Some(cx.spawn(async move |owner, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(500))
                        .await;
                    let _ = owner.update(cx, |this, cx| {
                        this.seiren_mic.access_delay = true;
                        this.sync_seiren_microphone_runtime(cx);
                        cx.notify();
                    });
                }));
            } else {
                self.cancel_seiren_microphone();
                self.seiren_mic.clip = None;
                self.seiren_mic.recording_error = None;
            }
        }
        let desired = mounted
            && self.seiren_mic.access_delay
            && self.seiren_microphone_can_access_audio()
            && self.seiren_mic.live
            && self.seiren_mic.native_capture_enabled;
        if !desired {
            self.seiren_mic.waveform_task = None;
            self.seiren_mic.retry = None;
            self.seiren_mic.retry_count = 0;
            if let Some(preview) = &mut self.seiren_mic.preview {
                preview.stop(false);
            }
            self.seiren_mic.waveform.clear();
            return;
        }
        if self.seiren_mic.preview.is_none()
            && self.seiren_mic.retry.is_none()
            && self.seiren_mic.retry_count < 5
        {
            let request = SeirenAudioCaptureRequested::new(
                cx.entity_id(),
                self.spec.product_id,
                CaptureOptions::new(CapturePurpose::LivePreview, Some(48_000))
                    .expect("source live capture"),
            );
            self.seiren_mic.preview = Some(CaptureOwner::new(request.clone()));
            self.seiren_mic.waveform_task = Some(cx.spawn(async move |owner, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                    if !matches!(
                        owner.update(cx, |this, cx| {
                            if this.seiren_mic.preview.is_none() {
                                return false;
                            }
                            if this.seiren_mic.waveform.tick() {
                                cx.notify();
                            }
                            true
                        }),
                        Ok(true)
                    ) {
                        break;
                    }
                }
            }));
            cx.emit(request);
        }
    }
    pub(super) fn seiren_microphone_capture_matches(
        &self,
        request: &SeirenAudioCaptureRequested,
    ) -> bool {
        if !supported(self.spec.product_id) || request.product_id() != self.spec.product_id {
            return false;
        }
        [
            &self.seiren_mic.preview,
            &self.seiren_mic.record,
            &self.seiren_mic.measurement,
        ]
        .into_iter()
        .flatten()
        .any(|owner| {
            owner.request.same_owner(request) && (!request.is_canceled() || owner.accept_final)
        })
    }
    pub(super) fn observe_seiren_microphone_capture(
        &mut self,
        request: &SeirenAudioCaptureRequested,
        frame: CaptureFrame,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.seiren_microphone_capture_matches(request) {
            return false;
        }
        for diagnostic in &frame.diagnostics {
            if !self.seiren_mic.diagnostics.contains(diagnostic) {
                self.seiren_mic.diagnostics.push(diagnostic.clone());
            }
        }
        let purpose = request.options().purpose();
        if purpose == CapturePurpose::LivePreview {
            if let Some(preview) = &mut self.seiren_mic.preview {
                preview.started.get_or_insert_with(Instant::now);
            }
            let gain = mic::percent_to_db(
                self.draft["profile"]["micVolume"]["value"]
                    .as_f64()
                    .unwrap_or(0.),
            );
            self.seiren_mic.waveform.observe(&frame, gain);
            self.seiren_mic.retry_count = 0;
        } else {
            let begin_timer =
                purpose != CapturePurpose::Recording
                    && self.seiren_mic.measurement.as_ref().is_some_and(|owner| {
                        owner.started.is_none() && !owner.request.is_canceled()
                    });
            let owner = if purpose == CapturePurpose::Recording {
                self.seiren_mic.record.as_mut()
            } else {
                self.seiren_mic.measurement.as_mut()
            };
            if let Some(owner) = owner {
                owner.started.get_or_insert_with(Instant::now);
                if owner
                    .rate
                    .is_some_and(|rate| rate != frame.capture_sample_rate)
                {
                    owner.error =
                        Some("Microphone capture sample rate changed during recording".into());
                    owner.stop(false);
                    cx.notify();
                    return true;
                }
                owner.rate = Some(frame.capture_sample_rate);
                owner.samples.extend_from_slice(&frame.mono_pcm);
                if purpose != CapturePurpose::Recording {
                    if let Some(wizard) = &mut self.seiren_mic.wizard {
                        let peak = owner
                            .samples
                            .iter()
                            .rev()
                            .take(1024)
                            .map(|sample| sample.abs())
                            .fold(0_f32, f32::max);
                        let db = 20. * peak.max(1e-8).log10();
                        let db = if db == 0. { -60. } else { db };
                        let level = ((db + 60.) / 60. * 100.).clamp(0., 100.);
                        wizard.levels = [level; 2];
                    }
                }
            }
            if begin_timer {
                self.start_seiren_wizard_measurement_timer(request.clone(), cx);
            }
        }
        cx.notify();
        true
    }
    pub(super) fn finish_seiren_microphone_capture(
        &mut self,
        request: &SeirenAudioCaptureRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let purpose = request.options().purpose();
        let slot = match purpose {
            CapturePurpose::LivePreview => &mut self.seiren_mic.preview,
            CapturePurpose::Recording => &mut self.seiren_mic.record,
            _ => &mut self.seiren_mic.measurement,
        };
        if !slot
            .as_ref()
            .is_some_and(|owner| owner.request.same_owner(request))
        {
            return;
        }
        let owner = slot.take().unwrap();
        if matches!(
            purpose,
            CapturePurpose::Speech | CapturePurpose::Environment
        ) {
            self.seiren_mic.measurement_timer = None;
            self.seiren_mic.measurement_start_delay = None;
            self.seiren_mic.measurement_last_stop =
                Some(owner.stopped.unwrap_or_else(Instant::now));
        }
        if purpose == CapturePurpose::LivePreview {
            self.seiren_mic.waveform_task = None;
            self.seiren_mic.waveform.clear();
            if let Some(error) = error {
                self.seiren_mic.diagnostics.push(error);
                let attempt = self.seiren_mic.retry_count;
                self.seiren_mic.retry_count = attempt.saturating_add(1);
                if !request.is_canceled() && self.seiren_mic.retry_count < 5 {
                    self.seiren_mic.retry = Some(cx.spawn(async move |owner, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis((1000u64 << attempt.min(3)).min(8000)))
                            .await;
                        let _ = owner.update(cx, |this, cx| {
                            this.seiren_mic.retry = None;
                            this.sync_seiren_microphone_runtime(cx);
                        });
                    }));
                }
            }
            self.sync_seiren_microphone_runtime(cx);
        } else if let Some(error) = error.or(owner.error) {
            self.seiren_mic.recording_error = Some(error);
        } else if owner.accept_final {
            if let Some(rate) = owner.rate {
                match PcmClip::new(rate, owner.samples) {
                    Ok(clip) if purpose == CapturePurpose::Recording => {
                        self.seiren_mic.clip = Some(Arc::new(clip))
                    }
                    Ok(clip) => self.finish_seiren_microphone_measurement(purpose, &clip, cx),
                    Err(error) => self.seiren_mic.recording_error = Some(format!("{error:#}")),
                }
            }
        }
        cx.notify();
    }
    pub(super) fn cancel_seiren_microphone(&mut self) {
        self.seiren_mic.waveform_task = None;
        self.seiren_mic.retry = None;
        self.seiren_mic.measurement_timer = None;
        self.stop_seiren_wizard_measurement(false);
        for owner in [
            &mut self.seiren_mic.preview,
            &mut self.seiren_mic.record,
            &mut self.seiren_mic.measurement,
        ]
        .into_iter()
        .flatten()
        {
            owner.stop(false);
        }
        if let Some(playback) = &self.seiren_mic.playback {
            playback.cancel();
        }
        self.seiren_mic.waveform.clear();
    }
    fn toggle_seiren_recording(&mut self, cx: &mut Context<Self>) {
        if let Some(owner) = &mut self.seiren_mic.record {
            owner.stop(true);
            cx.notify();
            return;
        }
        if self.seiren_mic.playback.is_some() {
            return;
        }
        self.seiren_mic.recording_error = None;
        let rate = self.draft["profile"]["samplingRate"]
            .as_u64()
            .unwrap_or(48_000) as u32;
        let request = SeirenAudioCaptureRequested::new(
            cx.entity_id(),
            self.spec.product_id,
            CaptureOptions::new(CapturePurpose::Recording, Some(rate)).expect("source record rate"),
        );
        self.seiren_mic.record = Some(CaptureOwner::new(request.clone()));
        cx.emit(request);
        cx.notify();
    }
    fn toggle_seiren_playback(&mut self, cx: &mut Context<Self>) {
        if let Some(playback) = &self.seiren_mic.playback {
            playback.cancel();
            return;
        }
        if self.seiren_mic.record.is_some() {
            return;
        }
        let Some(clip) = self.seiren_mic.clip.clone() else {
            return;
        };
        let request = SeirenAudioPlaybackRequested {
            owner: cx.entity_id(),
            session_id: NEXT_PLAYBACK.fetch_add(1, Ordering::Relaxed),
            product_id: self.spec.product_id,
            clip,
            canceled: Arc::new(AtomicBool::new(false)),
        };
        self.seiren_mic.playback = Some(request.clone());
        self.seiren_mic.playing = false;
        cx.emit(request);
        cx.notify();
    }
    pub(in crate::features) fn seiren_audio_playback_request_matches(
        &self,
        request: &SeirenAudioPlaybackRequested,
    ) -> bool {
        !request.is_canceled()
            && self
                .seiren_mic
                .playback
                .as_ref()
                .is_some_and(|pending| pending.same_owner(request))
    }
    pub(in crate::features) fn observe_seiren_audio_playback_started(
        &mut self,
        request: &SeirenAudioPlaybackRequested,
        cx: &mut Context<Self>,
    ) {
        if self.seiren_audio_playback_request_matches(request) {
            self.seiren_mic.playing = true;
            cx.notify();
        }
    }
    pub(in crate::features) fn finish_seiren_audio_playback(
        &mut self,
        request: &SeirenAudioPlaybackRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self
            .seiren_mic
            .playback
            .as_ref()
            .is_some_and(|pending| pending.same_owner(request))
        {
            self.seiren_mic.playback = None;
            self.seiren_mic.playing = false;
            if let Some(error) = error {
                self.seiren_mic.diagnostics.push(error);
            }
            cx.notify();
        }
    }
    fn seiren_sampling_rate(&mut self, rate: u32, cx: &mut Context<Self>) {
        self.draft["profile"]["samplingRate"] = json!(rate);
        self.observe_seiren_sampling_ready(false, cx);
        self.submit_seiren_eq_change(razer_device::seiren_eq::Change::SamplingRate(rate), cx);
    }
    pub(super) fn set_seiren_microphone_external_level(&mut self, level: f32) {
        self.seiren_mic.waveform.set_external(level);
    }
    pub(in crate::features) fn observe_seiren_native_event(
        &mut self,
        event: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !supported(self.spec.product_id) {
            return;
        }
        match event["eventType"].as_str() {
            Some("levelMeterPumpup") => {
                self.observe_seiren_stream_mixer_native(&event, cx);
                self.observe_seiren_meter_event(&event, cx);
            }
            Some("deviceVolume" | "deviceMute") => {
                // Native callbacks contain one endpoint observation. Updating
                // it never fills the unobserved endpoint with default values.
                let is_enabled = event["mute"] == 0;
                // Current MW handleAddLevelMeterListener toggles background
                // recording before selecting the endpoint, including HP mute.
                if event["eventType"] == "deviceMute" {
                    self.seiren_mic.native_capture_enabled = is_enabled;
                    self.sync_seiren_microphone_runtime(cx);
                }
                let path = match event["deviceId"].as_u64() {
                    Some(262145) => "/profile/micVolume",
                    Some(524289) => "/profile/volume",
                    _ => return,
                };
                if let Some(volume) = self.draft.pointer_mut(path) {
                    volume["isEnabled"] = json!(is_enabled);
                    // Source handler explicitly uses level ?? 0 for this
                    // observed endpoint; it does not synthesize its peer.
                    volume["value"] = event
                        .get("level")
                        .filter(|level| !level.is_null())
                        .cloned()
                        .unwrap_or(json!(0));
                }
                self.sync_seiren_eq(window, cx);
                self.defer_seiren_microphone_volume_sync(cx);
                cx.notify();
            }
            _ => {}
        }
    }
    fn seiren_microphone_set_volume(&mut self, value: Value, cx: &mut Context<Self>) {
        let operation = if self.draft["profile"]["micVolume"]["isEnabled"] != value["isEnabled"] {
            "ON_SET_MIC_VOLUME_ENABLED"
        } else {
            "ON_SET_MIC_VOLUME_VALUE"
        };
        self.draft["profile"]["micVolume"] = value.clone();
        self.submit_seiren_eq_payload(operation, value, cx);
        self.defer_seiren_microphone_volume_sync(cx);
        cx.notify();
    }
    fn defer_seiren_microphone_volume_sync(&self, cx: &mut Context<Self>) {
        let Some(handle) = self.seiren_mic.window else {
            return;
        };
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = cx.update_window(handle, |_, window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.sync_seiren_eq(window, cx);
                    if let Some(fader) = this
                        .seiren_mic
                        .wizard
                        .as_ref()
                        .and_then(|wizard| wizard.fader.as_ref())
                    {
                        let value = this.draft["profile"]["micVolume"]["value"]
                            .as_f64()
                            .unwrap_or(0.) as f32;
                        fader.update(cx, |fader, cx| fader.set_value(value, window, cx));
                    }
                });
            });
        });
    }
    pub(super) fn dispatch_seiren_microphone_control(
        &mut self,
        path: &str,
        cx: &mut Context<Self>,
    ) {
        if !supported(self.spec.product_id) {
            return;
        }
        for (field, module) in [
            ("noise_gate", 2),
            ("ai_noise_suppression", 3),
            ("mic_compressor", 4),
            ("mic_limiter", 5),
            ("mic_reverb", 6),
        ] {
            if path.starts_with(&format!("/device/{field}/")) {
                self.submit_seiren_eq_change(
                    razer_device::seiren_eq::Change::MicrophoneDsp {
                        module,
                        payload: self.draft["device"][field].clone(),
                    },
                    cx,
                );
                break;
            }
        }
    }
}

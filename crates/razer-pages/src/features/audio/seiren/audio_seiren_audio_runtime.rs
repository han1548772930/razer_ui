//! EQ6007 Q::U owns the analyser only while EQ is mounted, deviceReady,
//! host-active and document-visible. Native samples remain observations;
//! they never acknowledge a profile save or a Marie device-volume write.
use super::*;
use razer_device::seiren_audio_capture::CaptureFrame;
use razer_device::seiren_microphone::{CaptureOptions, CapturePurpose};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct SeirenAudioCaptureRequested {
    owner: EntityId,
    session_id: u64,
    product_id: u32,
    options: CaptureOptions,
    canceled: Arc<AtomicBool>,
}

impl SeirenAudioCaptureRequested {
    pub(super) fn new(owner: EntityId, product_id: u32, options: CaptureOptions) -> Self {
        Self {
            owner,
            session_id: NEXT_SESSION.fetch_add(1, Ordering::Relaxed),
            product_id,
            options,
            canceled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn options(&self) -> &CaptureOptions {
        &self.options
    }
    pub(super) fn same_owner(&self, other: &Self) -> bool {
        self.owner == other.owner && self.session_id == other.session_id
    }
    pub fn session_id(&self) -> u64 {
        self.session_id
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::Acquire)
    }
    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.canceled.clone()
    }
    pub(super) fn cancel(&self) {
        self.canceled.store(true, Ordering::Release);
    }
}

impl EventEmitter<SeirenAudioCaptureRequested> for AudioProductWorkspace {}

/// A canceled initialization still owns the slot until its service confirms
/// cleanup. A rapid tab/focus change cannot overlap two WASAPI sessions.
#[derive(Default)]
struct Lifetime {
    desired: bool,
    pending: bool,
    failed: bool,
}
impl Lifetime {
    fn set_desired(&mut self, desired: bool) {
        if self.desired != desired {
            self.failed = false;
        }
        self.desired = desired;
    }
    fn reserve(&mut self) -> bool {
        if !self.desired || self.pending || self.failed {
            return false;
        }
        self.pending = true;
        true
    }
    fn finish(&mut self, failed: bool, canceled: bool) {
        self.pending = false;
        self.failed = failed && !canceled;
    }
}

#[derive(Default)]
pub(super) struct State {
    pub(super) active: bool,
    pub(super) ready: bool,
    eq_page: bool,
    // Shell queries native BrowserWindow-equivalent visibility before Start.
    // Source 1107 keeps an unfocused, shown selected tab active; GPUI focus or
    // occlusion cannot stand in for that observation.
    host_visible: Option<bool>,
    lifetime: Lifetime,
    pending: Option<SeirenAudioCaptureRequested>,
    last_observation: Option<CaptureFrame>,
    error: Option<String>,
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some(request) = &self.pending {
            request.cancel();
        }
    }
}

impl AudioProductWorkspace {
    pub(in crate::features) fn set_seiren_audio_activity(
        &mut self,
        active: bool,
        ready: bool,
        eq_page: bool,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.spec.product_id, 1422 | 1446) {
            return;
        }
        self.seiren_audio.active = active;
        self.seiren_audio.ready = ready;
        self.seiren_audio.eq_page = eq_page;
        self.sync_seiren_audio_runtime(cx);
    }

    pub(super) fn sync_seiren_audio_runtime(&mut self, cx: &mut Context<Self>) {
        self.sync_seiren_microphone_runtime(cx);
        let state = &mut self.seiren_audio;
        let desired =
            state.active && state.ready && state.eq_page && state.host_visible != Some(false);
        state.lifetime.set_desired(desired);
        if !desired {
            if let Some(request) = &state.pending {
                request.cancel();
            }
            if self.seiren_eq.clear_capture() {
                cx.notify();
            }
            return;
        }
        if !state.lifetime.reserve() {
            return;
        }
        let request = SeirenAudioCaptureRequested::new(
            cx.entity_id(),
            self.spec.product_id,
            CaptureOptions::default(),
        );
        state.error = None;
        state.last_observation = None;
        state.pending = Some(request.clone());
        cx.emit(request);
    }

    pub(in crate::features) fn seiren_audio_capture_request_matches(
        &self,
        request: &SeirenAudioCaptureRequested,
        _: &App,
    ) -> bool {
        if request.options.purpose() != CapturePurpose::Equalizer {
            return self.seiren_microphone_capture_matches(request);
        }
        request.product_id == self.spec.product_id
            && !request.is_canceled()
            && self.seiren_audio.lifetime.desired
            && self.seiren_audio.pending.as_ref().is_some_and(|pending| {
                pending.owner == request.owner && pending.session_id == request.session_id
            })
    }

    pub(in crate::features) fn observe_seiren_audio_capture(
        &mut self,
        request: &SeirenAudioCaptureRequested,
        frame: CaptureFrame,
        cx: &mut Context<Self>,
    ) -> bool {
        if request.options.purpose() != CapturePurpose::Equalizer {
            return self.observe_seiren_microphone_capture(request, frame, cx);
        }
        if !self.seiren_audio_capture_request_matches(request, cx) {
            return false;
        }
        self.observe_seiren_eq_pcm(
            frame.output_sample_rate,
            frame.capture_sample_rate,
            &frame.mono_pcm,
            frame.microphone_levels,
            frame.headphone_levels,
            cx,
        );
        self.seiren_audio.last_observation = Some(frame);
        true
    }

    pub(in crate::features) fn finish_seiren_audio_capture(
        &mut self,
        request: &SeirenAudioCaptureRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if request.options.purpose() != CapturePurpose::Equalizer {
            self.finish_seiren_microphone_capture(request, error, cx);
            return;
        }
        if !self.seiren_audio.pending.as_ref().is_some_and(|pending| {
            pending.owner == request.owner && pending.session_id == request.session_id
        }) {
            return;
        }
        self.seiren_audio.pending = None;
        self.seiren_audio
            .lifetime
            .finish(error.is_some(), request.is_canceled());
        self.seiren_audio.error = error;
        self.sync_seiren_audio_runtime(cx);
    }

    pub(in crate::features) fn cancel_seiren_audio_capture(&mut self) {
        self.cancel_seiren_microphone();
        self.seiren_audio.active = false;
        self.seiren_audio.lifetime.set_desired(false);
        if let Some(request) = &self.seiren_audio.pending {
            request.cancel();
        }
        self.seiren_eq.clear_capture();
    }

    pub(in crate::features) fn observe_seiren_host_visibility(
        &mut self,
        visible: bool,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.spec.product_id, 1422 | 1446) {
            return;
        }
        self.seiren_audio.host_visible = Some(visible);
        self.sync_seiren_audio_runtime(cx);
    }

    pub(super) fn seiren_microphone_can_access_audio(&self) -> bool {
        self.seiren_audio.active
            && self.seiren_audio.ready
            && self.seiren_audio.host_visible != Some(false)
            && matches!(
                self.page.as_str(),
                "TAB_EQ" | "TAB_EFFECTS" | "TAB_STREAM_MIX" | "STREAM_MIXER_HEADER"
            )
    }
}

#[cfg(test)]
mod tests {
    use super::Lifetime;

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn seiren_eq_capture_reopen_waits_for_canceled_initialization_cleanup() {
        let mut state = Lifetime::default();
        state.set_desired(true);
        assert!(state.reserve());
        state.set_desired(false);
        state.set_desired(true);
        assert!(!state.reserve());
        state.finish(false, true);
        assert!(state.reserve());
    }

    #[test]
    fn seiren_eq_capture_failure_waits_for_a_new_activity_transition() {
        let mut state = Lifetime::default();
        state.set_desired(true);
        assert!(state.reserve());
        state.finish(true, false);
        state.set_desired(true);
        assert!(!state.reserve());
        state.set_desired(false);
        state.set_desired(true);
        assert!(state.reserve());
    }
}

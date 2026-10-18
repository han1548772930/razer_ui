//! Current 3594/3595/3596 source pickers and retained preview consumers.
//! Chromium enumeration and getUserMedia are replaced by local OS capture;
//! video facts never become property-read acknowledgements or profile values.
use super::camera_preview::CameraPreviewState;
use super::*;
use razer_device::camera_submission::CameraScope;
use razer_platform::camera_preview::{Event, Preferences, Session, Source, VideoMode};
use razer_platform::session_changes::SessionChange;
use std::{
    collections::BTreeSet,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct MediaOwner(u64);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct CaptureOwner {
    media: MediaOwner,
    generation: u64,
}

#[derive(Default)]
pub(super) struct State {
    scope: Option<CameraScope>,
    scope_revision: u64,
    session: Option<Session>,
    task: Option<Task<()>>,
    frame: Option<Arc<RenderImage>>,
    preferences: Preferences,
    reconnect: bool,
    capture_holds: BTreeSet<CaptureOwner>,
    restart_after: Option<Instant>,
    focus_after: Option<Instant>,
    focused: bool,
    host_tab_selected: bool,
    host_window_status: Option<razer_model::host_window_focus::HostWindowStatus>,
    starting: bool,
    playing: bool,
    lifecycle: Option<Lifecycle>,
    cleanup: razer_service::camera::lifecycle::Owner,
    retiring: Option<mpsc::Receiver<Result<(), String>>>,
}

impl State {
    fn replace_scope(&mut self, scope: Option<CameraScope>) -> bool {
        if self.scope == scope {
            return false;
        }
        self.scope_revision = self
            .scope_revision
            .checked_add(1)
            .expect("camera media scope exhausted");
        self.scope = scope;
        self.capture_holds.clear();
        true
    }
    fn owner(&self) -> Option<MediaOwner> {
        self.scope.as_ref().map(|_| MediaOwner(self.scope_revision))
    }
    fn owns(&self, owner: MediaOwner) -> bool {
        self.owner() == Some(owner)
    }
    fn hold_capture(&mut self, owner: CaptureOwner) -> bool {
        self.owns(owner.media) && self.capture_holds.insert(owner)
    }
    fn release_capture(&mut self, owner: CaptureOwner) -> bool {
        self.owns(owner.media) && self.capture_holds.remove(&owner) && !self.control_locked()
    }
    fn control_locked(&self) -> bool {
        !self.capture_holds.is_empty()
    }
    fn retirement_ready(&mut self) -> Result<bool, ()> {
        let Some(retiring) = &self.retiring else {
            return Ok(true);
        };
        match retiring.try_recv() {
            Ok(result) => {
                if let Err(error) = result {
                    eprintln!("camera previous preview cleanup: {error}");
                }
                self.retiring = None;
                Ok(true)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(false),
            Err(mpsc::TryRecvError::Disconnected) => {
                eprintln!("camera previous preview cleanup receipt disconnected");
                Err(())
            }
        }
    }
    fn retire_session(&mut self) {
        if let Some(session) = self.session.take() {
            // Closing admission is an in-memory channel send; the I/O owner
            // waits for capture and COM teardown away from the GUI thread.
            let _ = session.request_shutdown();
            match self
                .cleanup
                .cleanup("camera-preview-release", move || session.shutdown())
            {
                Ok(receipt) => self.retiring = Some(receipt),
                Err(error) => eprintln!("camera preview cleanup admission: {error:#}"),
            }
        }
    }
    fn retire_lifecycle(&mut self) {
        if let Some(lifecycle) = self.lifecycle.take() {
            lifecycle
                .canceled
                .store(true, std::sync::atomic::Ordering::Release);
            drop(lifecycle._task);
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.task.take();
        self.retire_session();
        self.retire_lifecycle();
    }
}

struct Lifecycle {
    canceled: Arc<std::sync::atomic::AtomicBool>,
    _task: Task<()>,
}

pub(super) fn supported(product_id: u32) -> bool {
    matches!(product_id, 3594..=3596)
}

fn checks_usb_capacity(product_id: u32) -> bool {
    matches!(product_id, 3594 | 3595)
}

fn resolution_values(options: &[OptionSpec], maximum: Option<VideoMode>) -> Vec<Value> {
    options
        .iter()
        .filter(|option| {
            mode(&option.value).is_some_and(|mode| {
                maximum.is_none_or(|maximum| {
                    mode.width <= maximum.width
                        && mode.height <= maximum.height
                        && mode.fps <= maximum.fps
                })
            })
        })
        .map(|option| option.value.clone())
        .collect()
}

fn reconcile_resolution(profile: &mut Value, resolutions: &[Value]) -> bool {
    if let Some(first) = resolutions.first()
        && !resolutions
            .iter()
            .any(|value| Some(value) == profile.pointer("/camera/resolution"))
    {
        profile["camera"]["resolution"] = first.clone();
        return true;
    }
    false
}
fn disable_third_party_framing(profile: &mut Value, third_party: bool) -> bool {
    if !third_party || profile["camera"]["autoFraming"]["isEnabled"] != true {
        return false;
    }
    profile["camera"]["autoFraming"]["isEnabled"] = false.into();
    true
}

fn mode(value: &Value) -> Option<VideoMode> {
    Some(VideoMode {
        width: u32::try_from(value.get("width")?.as_u64()?).ok()?,
        height: u32::try_from(value.get("height")?.as_u64()?).ok()?,
        fps: u32::try_from(value.get("fps")?.as_u64()?).ok()?,
    })
}

/// The current picker includes this physical camera plus installed Camo and
/// NVIDIA virtual cameras. MF exposes the USB identity in the symbolic link;
/// Chromium appends the same VID/PID to its label. ContainerId disambiguates
/// two identical physical cameras without guessing their enumeration order.
fn eligible_sources(sources: Vec<Source>, scope: &CameraScope) -> Vec<Source> {
    let hardware: Vec<_> = sources
        .iter()
        .filter(|source| {
            let id = source.id.to_ascii_lowercase();
            id.contains("vid_1532")
                && id.contains(&format!("pid_{:04x}", scope.product_id))
                && source
                    .container_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(&scope.container_id))
        })
        .cloned()
        .collect();
    if hardware.len() != 1 {
        return Vec::new();
    }
    hardware
        .into_iter()
        .chain(sources.into_iter().filter(|source| {
            source.name.contains("Camo") || source.name.contains("NVIDIA Broadcast")
        }))
        .collect()
}

impl SourceControls {
    pub(super) fn prepare_camera_media_exit(&mut self) {
        if let Some(state) = &mut self.camera_preview {
            state.media.task.take();
            state.media.retire_session();
            state.media.retire_lifecycle();
        }
    }
    pub(super) fn init_camera_media_subscription(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !supported(self.spec.product_id) {
            return;
        }
        let entity = cx.entity();
        self.subscriptions.push(cx.subscribe_in(
            &entity,
            window,
            |this, _, action: &CameraLocalActionRequested, window, cx| {
                this.consume_camera_media_action(action, window, cx);
            },
        ));
        self.start_camera_media_lifecycle(cx);
    }

    fn start_camera_media_lifecycle(&mut self, cx: &mut Context<Self>) {
        if self
            .camera_preview
            .as_ref()
            .is_none_or(|state| state.media.lifecycle.is_some())
        {
            return;
        }
        let canceled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let signal = canceled.clone();
        let (outgoing, events) = async_channel::unbounded();
        let jobs = &self.camera_preview.as_ref().unwrap().media.cleanup;
        match jobs.spawn("camera-session-observation", canceled.clone(), move || {
            if let Err(error) = razer_service::camera::commands::observe_sessions(signal, outgoing)
            {
                eprintln!("camera session observation: {error}");
            }
        }) {
            Ok(_) => {
                let task = cx.spawn(async move |view, cx| {
                    while let Ok(event) = events.recv().await {
                        match event {
                            SessionChange::Unlocked | SessionChange::Resumed => {
                                if view
                                    .update(cx, |this, cx| {
                                        // EU g(): f(); F(). The adapter serializes
                                        // stop/start; no synthetic playback receipt.
                                        this.pause_camera_media_for_save(cx);
                                        this.restart_camera_media(cx);
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            SessionChange::Failed(error) => {
                                eprintln!("camera session observation: {error}");
                                break;
                            }
                            SessionChange::Locked | SessionChange::Suspended => {}
                        }
                    }
                });
                if let Some(state) = &mut self.camera_preview {
                    state.media.lifecycle = Some(Lifecycle {
                        canceled,
                        _task: task,
                    });
                }
            }
            Err(error) => eprintln!("camera session observation: {error}"),
        }
    }

    pub fn set_camera_host_tab_selected(&mut self, selected: bool, cx: &mut Context<Self>) {
        let Some(state) = &mut self.camera_preview else {
            return;
        };
        if state.media.host_tab_selected == selected {
            return;
        }
        state.media.host_tab_selected = selected;
        // GU's activeTabChanged listener updates l directly; selecting this
        // tab additionally schedules F(true) after one second.
        Self::camera_focus_changed(state, selected);
        if selected {
            state.media.restart_after = Some(Instant::now() + Duration::from_secs(1));
        }
        cx.notify();
    }
    pub fn set_camera_host_window_status(
        &mut self,
        status: Option<razer_model::host_window_focus::HostWindowStatus>,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.camera_preview else {
            return;
        };
        if state.media.host_window_status == status {
            return;
        }
        state.media.host_window_status = status;
        let focused = state.media.host_tab_selected
            && status.is_some_and(|status| {
                status.is_visible != Some(false) && status.is_minimized != Some(true)
            });
        Self::camera_focus_changed(state, focused);
        cx.notify();
    }
    fn camera_focus_changed(state: &mut CameraPreviewState, focused: bool) {
        if state.media.focused == focused {
            return;
        }
        state.media.focused = focused;
        if state.media.scope.is_some() {
            state.observation.video_loading = true;
            state.media.focus_after = Some(Instant::now() + Duration::from_secs(2));
        }
    }
    pub(super) fn camera_source_focus(&self) -> bool {
        self.camera_preview
            .as_ref()
            .is_some_and(|state| state.media.focused && state.media.host_tab_selected)
    }

    pub(super) fn set_camera_media_scope(
        &mut self,
        scope: Option<CameraScope>,
        cx: &mut Context<Self>,
    ) {
        if !supported(self.spec.product_id) {
            return;
        }
        let Some(state) = &mut self.camera_preview else {
            return;
        };
        if !state.media.replace_scope(scope) {
            return;
        }
        state.media.restart_after = None;
        state.media.focus_after = None;
        state.media.starting = false;
        state.media.playing = false;
        state.media.frame = None;
        state.media.task.take();
        // Replacing the session also discards callbacks queued for an older
        // source/scope. Its worker receives Shutdown and releases capture.
        state.media.retire_session();
        state.observation.sources.clear();
        state.observation.selected_device_id = None;
        state.observation.selected_app_name.clear();
        state.observation.third_party_selected = false;
        state.observation.windows_properties_open = false;
        state.observation.resolutions.clear();
        state.observation.higher_gen_cable_required = false;
        state.observation.video_loading = false;
        if state.media.scope.is_none() {
            cx.notify();
            return;
        }
        let Some(handle) = self.camera_runtime.window else {
            return;
        };
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = cx.update_window(handle, |_, window, cx| {
                let _ = owner.update(cx, |this, cx| this.start_camera_media_consumer(window, cx));
            });
        });
    }

    fn start_camera_media_consumer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Ok(product_id) = u16::try_from(self.spec.product_id) else {
            eprintln!("camera media product ID is outside the device protocol range");
            return;
        };
        let Some(state) = &mut self.camera_preview else {
            return;
        };
        if state.media.scope.is_none() || state.media.session.is_some() {
            return;
        }
        match state.media.retirement_ready() {
            Ok(true) => {}
            Ok(false) => {
                state.media.task = Some(cx.spawn_in(window, async move |view, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(16))
                            .await;
                        let waiting = view.update_in(cx, |this, window, cx| {
                            let Some(state) = &mut this.camera_preview else {
                                return false;
                            };
                            if state.media.scope.is_none() {
                                return false;
                            }
                            match state.media.retirement_ready() {
                                Ok(false) => true,
                                Err(()) => false,
                                Ok(true) => {
                                    this.start_camera_media_consumer(window, cx);
                                    false
                                }
                            }
                        });
                        if !matches!(waiting, Ok(true)) {
                            break;
                        }
                    }
                }));
                return;
            }
            Err(()) => return,
        }
        let session = match Session::new() {
            Ok(session) => session,
            Err(error) => {
                eprintln!("camera media adapter: {error:#}");
                return;
            }
        };
        if let Err(error) = session
            .load_preferences(product_id)
            .and_then(|_| session.refresh())
        {
            eprintln!("camera media initialization: {error:#}");
            state.media.session = Some(session);
            state.media.retire_session();
            return;
        }
        state.media.session = Some(session);
        state.media.reconnect = true;
        state.media.task = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let alive =
                    view.update_in(cx, |this, window, cx| this.poll_camera_media(window, cx));
                if !matches!(alive, Ok(true)) {
                    break;
                }
            }
        }));
        self.start_camera_media_lifecycle(cx);
    }

    fn consume_camera_media_action(
        &mut self,
        action: &CameraLocalActionRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Ok(product_id) = u16::try_from(self.spec.product_id) else {
            eprintln!("camera media product ID is outside the device protocol range");
            return;
        };
        let Some(state) = &mut self.camera_preview else {
            return;
        };
        match action {
            CameraLocalActionRequested::SetPreviewEnabled(enabled) => {
                state.enabled = Some(*enabled);
                state.observation.preview_enabled = Some(*enabled);
                state.media.preferences.enabled = Some(*enabled);
                if let Some(session) = &state.media.session {
                    if let Err(error) =
                        session.save_preferences(product_id, state.media.preferences.clone())
                    {
                        eprintln!("camera preview preference: {error:#}");
                    }
                }
                self.disable_camera_third_party_framing(window, cx);
                self.restart_camera_media(cx);
            }
            CameraLocalActionRequested::RefreshSources => {
                state.media.reconnect = true;
                state.media.frame = None;
                state.media.starting = false;
                state.media.playing = false;
                state.observation.video_loading = true;
                // EU refreshCamera effect stops now, reacquires after 1000ms.
                state.media.restart_after = Some(Instant::now() + Duration::from_secs(1));
                if let Some(session) = &state.media.session {
                    if let Err(error) = session.stop().and_then(|_| session.refresh()) {
                        eprintln!("camera media refresh: {error:#}");
                    }
                }
            }
            CameraLocalActionRequested::SelectSource { device_id, name } => {
                if !state
                    .observation
                    .sources
                    .iter()
                    .any(|source| source.device_id == *device_id)
                {
                    return;
                }
                state.observation.selected_device_id = Some(device_id.clone());
                state.observation.third_party_selected =
                    name.contains("Camo") || name.contains("NVIDIA Broadcast");
                state.observation.selected_app_name = name.clone();
                if let Some(scope) = &state.media.scope {
                    state
                        .media
                        .preferences
                        .sources
                        .insert(scope.container_id.clone(), device_id.clone());
                }
                if let Some(session) = &state.media.session {
                    if let Err(error) =
                        session.save_preferences(product_id, state.media.preferences.clone())
                    {
                        eprintln!("camera selected-source preference: {error:#}");
                    }
                }
                self.disable_camera_third_party_framing(window, cx);
                self.restart_camera_media(cx);
            }
            CameraLocalActionRequested::OpenWindowsSettings => {
                self.request_camera_properties(window, cx);
            }
            CameraLocalActionRequested::LaunchInstalledApp(name) => {
                let name = name.clone();
                let canceled = Arc::new(std::sync::atomic::AtomicBool::new(false));
                let signal = canceled.clone();
                if let Err(error) =
                    self.camera_runtime
                        .jobs
                        .spawn("camera-app-launch", canceled, move || {
                            if signal.load(std::sync::atomic::Ordering::Acquire) {
                                return;
                            }
                            let result =
                                razer_platform::camera_apps::CameraApp::from_source_name(&name)
                                    .and_then(|app| {
                                        if signal.load(std::sync::atomic::Ordering::Acquire) {
                                            return Ok(());
                                        }
                                        razer_platform::camera_apps::launch_installed(app)
                                    });
                            if let Err(error) = result {
                                eprintln!("camera application launch: {error:#}");
                            }
                        })
                {
                    eprintln!("camera application worker: {error}");
                }
            }
        }
        cx.notify();
    }

    fn disable_camera_third_party_framing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let third_party = self.camera_third_party_selected();
        if !disable_third_party_framing(&mut self.draft, third_party) {
            return;
        }
        // MU dispatches the complete autoFraming object with only this field
        // changed. Keep the optimistic reducer value if native write-back fails.
        self.request_camera_edit(
            &format!("{}:auto-framing", self.spec.product_id),
            window,
            cx,
        );
        self.sync(window, cx);
        cx.emit(SourceControlsChanged);
    }

    pub(super) fn restart_camera_media(&mut self, cx: &mut Context<Self>) {
        let requested = self.draft.pointer("/camera/resolution").and_then(mode);
        let Some(state) = &mut self.camera_preview else {
            return;
        };
        if state.media.control_locked() {
            return;
        }
        state.media.restart_after = None;
        let Some(session) = &state.media.session else {
            return;
        };
        // Current F awaits getFocusTab. A non-selected mounted tab must not
        // acquire a stream or discard an already attached stream here.
        if state.enabled == Some(true) && !state.media.host_tab_selected {
            return;
        }
        state.media.frame = None;
        state.observation.video_loading = false;
        state.media.starting = false;
        state.media.playing = false;
        if state.enabled != Some(true) || state.media.scope.is_none() {
            if let Err(error) = session.stop() {
                eprintln!("camera preview stop: {error:#}");
            }
            cx.notify();
            return;
        }
        let Some(source) = state.observation.selected_device_id.clone() else {
            // Source F handles absent deviceId without inventing a stream or
            // the used-by-another-app state reserved for acquisition errors.
            let _ = session.stop();
            cx.notify();
            return;
        };
        state.observation.video_loading = true;
        state.observation.used_by_other_app = false;
        state.media.starting = true;
        let requested = if state.observation.third_party_selected {
            None
        } else {
            requested
        };
        if let Err(error) = session.start(source, requested) {
            state.media.starting = false;
            state.observation.video_loading = false;
            state.observation.used_by_other_app = true;
            eprintln!("camera preview start: {error:#}");
        }
        cx.notify();
    }

    /// Current MW Bn waits for initUITurnOffCameraLocker before touching HDR.
    /// The MF worker supplies a receipt after Flush/OnFlush/Shutdown; holding
    /// Each request retains its hold until its own completion, so independent
    /// source task names cannot reopen capture while another write is active.
    pub(super) fn camera_media_owner(&self) -> Option<MediaOwner> {
        self.camera_preview.as_ref()?.media.owner()
    }

    pub(super) fn camera_media_owner_is_current(&self, owner: MediaOwner) -> bool {
        self.camera_preview
            .as_ref()
            .is_some_and(|state| state.media.owns(owner))
    }

    pub(super) fn camera_capture_owner(&self, generation: u64) -> Option<CaptureOwner> {
        Some(CaptureOwner {
            media: self.camera_media_owner()?,
            generation,
        })
    }

    pub(super) fn lock_camera_capture(
        &mut self,
        owner: CaptureOwner,
        cx: &mut Context<Self>,
    ) -> Result<Option<mpsc::Receiver<Result<(), String>>>, String> {
        let Some(state) = &mut self.camera_preview else {
            return Ok(None);
        };
        if !state.media.hold_capture(owner) {
            return Err(
                "camera capture request belongs to a retired scope or is already held".into(),
            );
        }
        state.media.restart_after = None;
        state.media.frame = None;
        state.media.starting = false;
        state.media.playing = false;
        state.observation.video_loading = true;
        let result = state
            .media
            .session
            .as_ref()
            .map(Session::stop_for_control)
            .transpose()
            .map_err(|error| format!("{error:#}"));
        cx.notify();
        result
    }

    pub(super) fn finish_camera_capture_control(
        &mut self,
        owner: CaptureOwner,
        cx: &mut Context<Self>,
    ) {
        if let Some(state) = &mut self.camera_preview
            && state.media.release_capture(owner)
        {
            // Current MW xn broadcasts restart even after failure. EU waits
            // 3000ms before F; retaining a deadline also permits cancellation.
            state.media.restart_after = Some(Instant::now() + Duration::from_secs(3));
        }
        cx.notify();
    }

    pub(super) fn schedule_camera_media_restart(&mut self, cx: &mut Context<Self>) {
        self.pause_camera_media_for_save(cx);
        if let Some(state) = &mut self.camera_preview {
            state.media.restart_after = Some(Instant::now() + Duration::from_secs(3));
        }
        cx.notify();
    }

    pub(super) fn pause_camera_media_for_save(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.camera_preview {
            state.media.frame = None;
            state.media.starting = false;
            state.media.playing = false;
            state.observation.video_loading = true;
            if let Some(session) = &state.media.session {
                if let Err(error) = session.stop() {
                    eprintln!("camera restart stop: {error:#}");
                }
            }
            state.media.restart_after = None;
        }
        cx.notify();
    }

    fn poll_camera_media(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(state) = &mut self.camera_preview else {
            return false;
        };
        let Some(session) = &state.media.session else {
            return false;
        };
        let events = session.events();
        let frame = session.take_frame();
        let mut changed = false;
        let mut resolution_changed = false;
        let mut restart = !state.media.control_locked()
            && state
                .media
                .restart_after
                .is_some_and(|when| Instant::now() >= when);
        if state
            .media
            .focus_after
            .is_some_and(|when| Instant::now() >= when)
        {
            state.media.focus_after = None;
            if state.enabled == Some(true) && state.media.focused {
                // F skips in-flight acquisition and never replaces a stream
                // already attached to the source video element.
                restart |= !state.media.starting && !state.media.playing;
            } else if state.media.starting {
                if let Err(error) = session.stop() {
                    eprintln!("camera focus stop: {error:#}");
                }
                state.media.starting = false;
                state.media.playing = false;
                state.media.frame = None;
                changed = true;
            }
        }
        for event in events {
            match event {
                Event::Preferences(preferences) => {
                    // The reducer initially holds null. The mounted product
                    // then uses true when the local globalDeviceConfig has no
                    // saved isPreview value; a saved false remains false.
                    let mut preferences = preferences;
                    preferences.enabled = Some(preferences.enabled.unwrap_or(true));
                    state.enabled = preferences.enabled;
                    state.observation.preview_enabled = preferences.enabled;
                    state.media.preferences = preferences;
                    changed = true;
                }
                Event::Sources(sources) => {
                    let Some(scope) = &state.media.scope else {
                        continue;
                    };
                    let sources = eligible_sources(sources, scope);
                    if checks_usb_capacity(self.spec.product_id)
                        && state.observation.resolutions.is_empty()
                        && !state.media.control_locked()
                    {
                        if let Some(hardware) = sources.first() {
                            if let Err(error) = session.probe(hardware.id.clone()) {
                                eprintln!("camera capability probe: {error:#}");
                            }
                        }
                    } else if self.spec.product_id == 3596 && !sources.is_empty() {
                        // Current UU mounts MU(checkUSBCapacity:false). Its
                        // declared list is available without a probe stream.
                        if let Some(control) = self
                            .spec
                            .pages
                            .iter()
                            .flat_map(|page| &page.sections)
                            .flat_map(|section| &section.controls)
                            .find(|control| control.path == "/camera/resolution")
                        {
                            state.observation.resolutions =
                                resolution_values(&control.options, None);
                            resolution_changed |= reconcile_resolution(
                                &mut self.draft,
                                &state.observation.resolutions,
                            );
                        }
                    }
                    let preferred = state
                        .observation
                        .selected_device_id
                        .as_ref()
                        .or_else(|| state.media.preferences.sources.get(&scope.container_id));
                    let selected = preferred
                        .and_then(|id| sources.iter().find(|source| source.id == *id))
                        .or_else(|| sources.first());
                    state.observation.selected_device_id = selected.map(|source| source.id.clone());
                    state.observation.selected_app_name = selected
                        .map(|source| source.name.clone())
                        .unwrap_or_default();
                    state.observation.third_party_selected = selected.is_some_and(|source| {
                        source.name.contains("Camo") || source.name.contains("NVIDIA Broadcast")
                    });
                    state.observation.sources = sources
                        .into_iter()
                        .map(|source| CameraPreviewSource {
                            device_id: source.id,
                            name: source.name,
                        })
                        .collect();
                    restart |= state.media.reconnect && state.media.restart_after.is_none();
                    state.media.reconnect = false;
                    changed = true;
                }
                Event::Capabilities { source, modes } => {
                    if !checks_usb_capacity(self.spec.product_id) {
                        continue;
                    }
                    let Some(scope) = &state.media.scope else {
                        continue;
                    };
                    let source = source.to_ascii_lowercase();
                    if !source.contains("vid_1532")
                        || !source.contains(&format!("pid_{:04x}", scope.product_id))
                    {
                        continue;
                    }
                    // useZm probes track capability maxima, then keeps only
                    // source-defined options within those maxima (not a new list).
                    let maximum = VideoMode {
                        width: modes.iter().map(|m| m.width).max().unwrap_or(0),
                        height: modes.iter().map(|m| m.height).max().unwrap_or(0),
                        fps: modes.iter().map(|m| m.fps).max().unwrap_or(0),
                    };
                    {
                        if let Some(control) = self
                            .spec
                            .pages
                            .iter()
                            .flat_map(|page| &page.sections)
                            .flat_map(|section| &section.controls)
                            .find(|control| control.path == "/camera/resolution")
                        {
                            state.observation.resolutions =
                                resolution_values(&control.options, Some(maximum));
                            state.observation.higher_gen_cable_required =
                                state.observation.resolutions.len() < control.options.len();
                            if reconcile_resolution(&mut self.draft, &state.observation.resolutions)
                            {
                                resolution_changed = true;
                                restart = true;
                            }
                        }
                    }
                    changed = true;
                }
                Event::Playing => {
                    if state.media.control_locked() {
                        continue;
                    }
                    state.observation.video_loading = false;
                    state.media.starting = false;
                    state.media.playing = true;
                    state.observation.used_by_other_app = false;
                    changed = true;
                }
                Event::Stopped => {
                    state.media.frame = None;
                    state.media.playing = false;
                    changed = true;
                }
                Event::Failed(error) => {
                    eprintln!("camera media acquisition: {error}");
                    state.observation.video_loading = false;
                    state.media.starting = false;
                    state.media.playing = false;
                    state.observation.used_by_other_app = true;
                    state.media.frame = None;
                    changed = true;
                }
                Event::Diagnostic(error) => eprintln!("camera media diagnostic: {error}"),
            }
        }
        if let Some(frame) = frame
            .filter(|_| state.enabled == Some(true) && !restart && !state.media.control_locked())
        {
            if let Some(image) = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra) {
                state.media.frame =
                    Some(Arc::new(RenderImage::new(vec![image::Frame::new(image)])));
                changed = true;
            }
        }
        let observation = changed.then(|| state.observation.clone());
        if let Some(observation) = observation {
            self.observe_camera_preview(observation, window, cx);
        }
        // MU also reruns the effect when a profile changes autoFraming while
        // the third-party source itself remains selected.
        self.disable_camera_third_party_framing(window, cx);
        if resolution_changed {
            cx.emit(SourceControlsChanged);
        }
        if restart {
            self.restart_camera_media(cx);
        }
        true
    }

    pub(super) fn render_camera_media(&self) -> AnyElement {
        let frame = self
            .camera_preview
            .as_ref()
            .and_then(|state| state.media.frame.clone());
        let Some(frame) = frame else {
            return div().into_any_element();
        };
        img(frame)
            .size_full()
            .object_fit(ObjectFit::Contain)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    fn capture_scope() -> super::CameraScope {
        super::CameraScope {
            product_id: 3594,
            container_id: "{camera}".into(),
        }
    }

    #[test]
    fn independent_control_tasks_keep_capture_closed_until_the_last_receipt() {
        let mut state = super::State::default();
        state.replace_scope(Some(capture_scope()));
        let media = state.owner().unwrap();
        let hdr = super::CaptureOwner {
            media,
            generation: 1,
        };
        let ldc = super::CaptureOwner {
            media,
            generation: 2,
        };
        assert!(state.hold_capture(hdr));
        assert!(state.hold_capture(ldc));
        assert!(!state.release_capture(hdr));
        assert!(state.control_locked());
        assert!(!state.release_capture(hdr));
        assert!(state.release_capture(ldc));
        assert!(!state.control_locked());
    }

    #[test]
    fn reconnecting_the_same_camera_rejects_old_control_and_save_receipts() {
        let mut state = super::State::default();
        state.replace_scope(Some(capture_scope()));
        let old_media = state.owner().unwrap();
        let old = super::CaptureOwner {
            media: old_media,
            generation: 1,
        };
        assert!(state.hold_capture(old));
        assert!(state.replace_scope(None));
        assert!(state.replace_scope(Some(capture_scope())));
        let new_media = state.owner().unwrap();
        let new = super::CaptureOwner {
            media: new_media,
            generation: 1,
        };
        assert!(state.hold_capture(new));
        assert!(!state.owns(old_media));
        assert!(!state.release_capture(old));
        assert!(state.control_locked());
        assert!(state.release_capture(new));
    }

    #[test]
    fn profile_invalidation_keeps_the_mounted_control_cleanup_owner() {
        let mut state = super::State::default();
        let scope = capture_scope();
        state.replace_scope(Some(scope.clone()));
        let owner = super::CaptureOwner {
            media: state.owner().unwrap(),
            generation: 1,
        };
        assert!(state.hold_capture(owner));
        let mut queue = razer_device::camera_queue::CameraIntentQueue::default();
        queue.set_scope(Some(scope.clone()));
        queue.invalidate();
        assert!(!state.replace_scope(Some(scope)));
        assert!(state.release_capture(owner));
    }

    #[test]
    fn preview_replacement_waits_for_the_actual_cleanup_receipt() {
        let mut state = super::State::default();
        let (reply, receipt) = std::sync::mpsc::channel();
        state.retiring = Some(receipt);
        assert_eq!(state.retirement_ready(), Ok(false));
        reply
            .send(Err("simulated flush failure after thread release".into()))
            .unwrap();
        assert_eq!(state.retirement_ready(), Ok(true));
        assert!(state.retiring.is_none());
    }

    #[test]
    fn missing_preview_cleanup_receipt_does_not_admit_a_replacement() {
        let mut state = super::State::default();
        let (reply, receipt) = std::sync::mpsc::channel();
        state.retiring = Some(receipt);
        drop(reply);
        assert_eq!(state.retirement_ready(), Err(()));
        assert!(state.retiring.is_some());
    }

    use super::{
        CameraScope, Source, VideoMode, checks_usb_capacity, eligible_sources,
        reconcile_resolution, resolution_values,
    };
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn third_party_effect_changes_only_enable_and_preserves_full_autoframing_payload() {
        let mut profile = serde_json::json!({"camera":{"autoFraming":{"isEnabled":true,"trackingSpeed":"Gradual","view":"Wide","trackingSubject":"People"}}});
        let original = profile.clone();
        assert!(!super::disable_third_party_framing(&mut profile, false));
        assert_eq!(profile, original);
        assert!(super::disable_third_party_framing(&mut profile, true));
        let mut expected = original;
        expected["camera"]["autoFraming"]["isEnabled"] = false.into();
        assert_eq!(profile, expected);
        assert!(!super::disable_third_party_framing(&mut profile, true));
    }

    #[test]
    fn each_current_camera_selects_its_container_before_local_virtual_sources() {
        for product_id in [3594, 3595, 3596] {
            let scope = CameraScope {
                product_id,
                container_id: "{selected}".into(),
            };
            let source = |name: &str, container: &str| Source {
                id: format!("usb#vid_1532&pid_{product_id:04x}#{container}"),
                name: name.into(),
                container_id: Some(container.into()),
            };
            let sources = eligible_sources(
                vec![
                    source("Other physical camera", "{other}"),
                    source("Selected physical camera", "{selected}"),
                    Source {
                        id: "virtual-camo".into(),
                        name: "Camo".into(),
                        container_id: None,
                    },
                    Source {
                        id: "virtual-nvidia".into(),
                        name: "NVIDIA Broadcast".into(),
                        container_id: None,
                    },
                    // Current picker K enumerates Camo/NVIDIA only, despite
                    // retaining a separate XSplit launcher branch.
                    Source {
                        id: "virtual-vcam".into(),
                        name: "VCam Camera".into(),
                        container_id: None,
                    },
                ],
                &scope,
            );
            assert_eq!(
                sources.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
                ["Selected physical camera", "Camo", "NVIDIA Broadcast"]
            );
            assert!(eligible_sources(sources[1..].to_vec(), &scope).is_empty());
        }
    }

    #[test]
    fn kiyo_v2_filters_declared_modes_but_v2_x_does_not_open_a_probe_stream() {
        let maximum = VideoMode {
            width: 1920,
            height: 1080,
            fps: 60,
        };
        for product_id in [3594, 3595, 3596] {
            let spec = super::super::specs()
                .iter()
                .find(|s| s.product_id == product_id)
                .unwrap();
            let control = spec
                .pages
                .iter()
                .flat_map(|p| &p.sections)
                .flat_map(|s| &s.controls)
                .find(|c| c.path == "/camera/resolution")
                .unwrap();
            let resolutions = resolution_values(
                &control.options,
                checks_usb_capacity(product_id).then_some(maximum),
            );
            if product_id == 3596 {
                assert_eq!(resolutions.len(), control.options.len());
                assert!(
                    resolutions
                        .iter()
                        .any(|v| v["width"] == 2560 && v["fps"] == 60)
                );
                assert!(!resolutions.iter().any(|v| v["width"] == 3840));
            } else {
                assert!(resolutions.len() < control.options.len());
                assert!(
                    resolutions
                        .iter()
                        .all(|v| v["width"].as_u64().unwrap() <= 1920)
                );
            }
            let mut profile = spec.profile.clone();
            profile["camera"]["resolution"] = serde_json::json!({"width":1,"height":1,"fps":1});
            assert!(reconcile_resolution(&mut profile, &resolutions));
            assert_eq!(profile["camera"]["resolution"], resolutions[0]);
            assert!(!reconcile_resolution(&mut profile, &resolutions));
            let current = profile.clone();
            assert!(!reconcile_resolution(&mut profile, &[]));
            assert_eq!(profile, current);
        }
    }
    #[test]
    fn identical_products_select_by_container_and_keep_only_source_virtual_branches() {
        let scope = CameraScope {
            product_id: 3594,
            container_id: "{one}".into(),
        };
        let source = |id: &str, name: &str, container: &str| Source {
            id: id.into(),
            name: name.into(),
            container_id: Some(container.into()),
        };
        let sources = eligible_sources(
            vec![
                source("usb#vid_1532&pid_0e0a#two", "Other Kiyo", "{two}"),
                source("usb#vid_1532&pid_0e0a#one", "Kiyo", "{one}"),
                source("virtual-camo", "Camo", ""),
                source("other", "Other webcam", ""),
            ],
            &scope,
        );
        assert_eq!(
            sources.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["Kiyo", "Camo"]
        );
    }
}

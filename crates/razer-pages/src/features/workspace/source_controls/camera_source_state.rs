//! Current MW Je/fe -> YQJ/w9U -> usePU/Sr/useMU. These observations have
//! explicit source reducers and remain separate from Rust diagnostic reads.
use super::*;
use razer_device::{
    camera::{CameraControlProperty as C, StandardCameraProperty as P, VideoProcAmpProperty as V},
    camera_exposure_family::{CameraFamilyExposureRequest as X, CameraFamilyExposureValue},
    camera_extensions::{CameraExtensionRequest as E, CameraExtensionValue},
    camera_submission::{
        CameraCommand, CameraCommandOutcome, CameraScope, CameraSubmissionPlan,
        CameraSubmissionProgress,
    },
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
enum Query {
    Cache {
        publish: bool,
        footer_preview_request: Option<u64>,
    },
    Iso,
}
struct Job {
    kind: Query,
    scope: CameraScope,
    canceled: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Result<CameraSubmissionProgress, String>>,
    settled: bool,
}
impl Drop for Job {
    fn drop(&mut self) {
        if !self.settled {
            self.canceled.store(true, Ordering::Release);
        }
    }
}
#[derive(Default)]
pub(super) struct State {
    scope: Option<CameraScope>,
    job: Option<Job>,
    task: Option<Task<()>>,
    cache_request: Option<(Instant, bool)>,
    choice_shutter: Option<(Instant, Value)>,
    next_iso: Option<Instant>,
    cache: Option<Value>,
    preview_cache_requested: bool,
    footer_enable_after_cache: Option<u64>,
    footer_preview_generation: u64,
    pub(super) auto_iso: Option<Value>,
    dialog: Option<Value>,
    dialog_focus: Option<FocusHandle>,
    // Source writes the session key after either choice; it is not a save ACK.
    selected_settings: bool,
}

impl State {
    pub(super) fn request_footer_preview(&mut self) {
        self.footer_preview_generation = self.footer_preview_generation.wrapping_add(1);
        self.footer_enable_after_cache = Some(self.footer_preview_generation);
    }

    pub(super) fn cancel_footer_preview(&mut self) {
        self.footer_enable_after_cache = None;
    }

    fn settle_footer_preview(&mut self, request: Option<u64>, succeeded: bool) -> bool {
        // A click must await the cache read queued for that click. An older
        // in-flight read or failure cannot settle a newer preview request.
        if request.is_some() && self.footer_enable_after_cache == request {
            self.footer_enable_after_cache = None;
            succeeded
        } else {
            false
        }
    }

    pub(super) fn set_scope(&mut self, scope: Option<CameraScope>) -> bool {
        if self.scope == scope {
            return false;
        }
        // An explicit query may already be waiting before the first device
        // binding. Only a previous binding makes a pending request stale.
        if self.scope.is_some() {
            self.cache_request = None;
            self.cancel_footer_preview();
        }
        self.job.take();
        self.scope = scope;
        self.cache = None;
        self.auto_iso = None;
        self.dialog = None;
        self.preview_cache_requested = false;
        self.next_iso = None;
        self.choice_shutter = None;
        true
    }

    fn preview_requests_cache(&mut self, focused: bool, enabled: bool, source_known: bool) -> bool {
        // Current EU's G ref is reset by blur/preview-off, not by changing
        // resolution or source while the preview stays focused and enabled.
        if !focused || !enabled {
            self.preview_cache_requested = false;
            return false;
        }
        if source_known && !self.preview_cache_requested {
            self.preview_cache_requested = true;
            return true;
        }
        false
    }
}

fn cache_plan() -> CameraSubmissionPlan {
    CameraSubmissionPlan {
        commands: [
            P::VideoProcAmp(V::Brightness),
            P::VideoProcAmp(V::Contrast),
            P::VideoProcAmp(V::Saturation),
            P::VideoProcAmp(V::WhiteBalance),
            P::CameraControl(C::Zoom),
            P::CameraControl(C::Pan),
            P::CameraControl(C::Tilt),
            P::CameraControl(C::Focus),
            P::CameraControl(C::Exposure),
            P::CameraControl(C::LowLightCompensation),
            P::VideoProcAmp(V::AntiFlicker),
        ]
        .into_iter()
        .map(CameraCommand::StandardRead)
        .collect(),
    }
}
/// Je/Ze issue two consecutive Promise.all groups (4 image + 7 camera).
/// The first rejected getter rejects its group; already-issued siblings keep
/// their own service/COM cleanup and do not become a partial UI cache.
fn run_cache_getters(
    jobs: razer_service::camera::lifecycle::Spawner,
    signal: Arc<AtomicBool>,
    execute: impl Fn(CameraCommand) -> Result<CameraSubmissionProgress, String> + Send + Sync + 'static,
) -> Result<CameraSubmissionProgress, String> {
    let execute = Arc::new(execute);
    let commands = cache_plan().commands;
    let mut progress = CameraSubmissionProgress::default();
    for batch in [&commands[..4], &commands[4..]] {
        let (sender, receiver) = mpsc::channel();
        for (index, command) in batch.iter().cloned().enumerate() {
            let sender = sender.clone();
            let execute = execute.clone();
            jobs.spawn("camera-source-getter", signal.clone(), move || {
                let _ = sender.send((index, execute(command)));
            })
            .map_err(|error| format!("camera getter worker: {error}"))?;
        }
        drop(sender);
        let mut results: Vec<Option<CameraSubmissionProgress>> = vec![None; batch.len()];
        for _ in 0..batch.len() {
            let (index, result) = receiver
                .recv()
                .map_err(|_| "camera getter worker disconnected")?;
            let result = result?;
            if !result.completed || result.error.is_some() {
                return Err(result
                    .error
                    .unwrap_or_else(|| "camera getter returned an incomplete receipt".into()));
            }
            results[index] = Some(result);
        }
        for result in results.into_iter().flatten() {
            progress.outcomes.extend(result.outcomes);
        }
    }
    progress.completed = true;
    Ok(progress)
}
fn cache_from_receipts(
    product_id: u32,
    progress: &CameraSubmissionProgress,
) -> anyhow::Result<Value> {
    anyhow::ensure!(
        matches!(product_id, 3594..=3596),
        "camera cache model is unaudited"
    );
    anyhow::ensure!(progress.completed, "camera source cache read is incomplete");
    let read = |property| -> anyhow::Result<_> {
        progress
            .outcomes
            .iter()
            .find_map(|outcome| match outcome {
                CameraCommandOutcome::Read(read) if read.property == property => {
                    Some(read.observation)
                }
                _ => None,
            })
            .ok_or_else(|| anyhow::anyhow!("camera source cache omits {property:?}"))
    };
    let white_balance = read(P::VideoProcAmp(V::WhiteBalance))?;
    let focus = read(P::CameraControl(C::Focus))?;
    let exposure = read(P::CameraControl(C::Exposure))?;
    // Current Je/Ze mask the standard value only for the two models whose
    // DeviceInfo enables hasExtendedUVCExposure. X retains its signed LONG.
    let exposure_value = if exposure.is_auto() || product_id != 3596 {
        0
    } else {
        exposure.value
    };
    Ok(serde_json::json!({
        "currentViewPreset":{"zoom":read(P::CameraControl(C::Zoom))?.value,"pan":read(P::CameraControl(C::Pan))?.value,"tilt":read(P::CameraControl(C::Tilt))?.value},
        "autoFocus":{"isEnabled":focus.is_auto(),"manualFocus":focus.value,"lightingType":0},
        "autoExposure":{"isEnabled":exposure.is_auto(),"metering":0,"compensation":0,"isoSensitivity":0,"shutterSpeed":0,"exposureValue":exposure_value},
        "lowLightCompensation":read(P::CameraControl(C::LowLightCompensation))?.value!=0,
        "antiFlicker":read(P::VideoProcAmp(V::AntiFlicker))?.value,
        "currentImageSettings":{"name":"custom","brightness":read(P::VideoProcAmp(V::Brightness))?.value,"contrast":read(P::VideoProcAmp(V::Contrast))?.value,"saturation":read(P::VideoProcAmp(V::Saturation))?.value,"whiteBalance":{"value":white_balance.value,"isAuto":white_balance.is_auto()}}
    }))
}
fn camera_zoom(value: f64) -> f64 {
    if (1. ..=4.).contains(&value) {
        (value * 100.).round()
    } else {
        value
    }
}
fn different(cache: &Value, profile: &Value) -> bool {
    let view = profile["camera"]["viewPresets"]["data"]
        .as_array()
        .and_then(|presets| {
            presets
                .iter()
                .find(|p| p["mode"] == profile["camera"]["viewPresets"]["viewMode"])
        });
    let Some(view) = view else {
        return false;
    };
    let zoom = view["zoom"].as_f64().map(|v| (v * 100.).round());
    let source_zoom = cache["currentViewPreset"]["zoom"].as_f64();
    if let (Some(zoom), Some(source)) = (zoom, source_zoom) {
        if source != 0. && camera_zoom(source) != zoom {
            return true;
        }
        if zoom != 100.
            && ["pan", "tilt"].into_iter().any(|k| {
                !cache["currentViewPreset"][k].is_null() && cache["currentViewPreset"][k] != view[k]
            })
        {
            return true;
        }
    }
    let camera = &profile["camera"];
    for (path, gate) in [
        ("/autoFocus/isEnabled", true),
        (
            "/autoFocus/manualFocus",
            cache["autoFocus"]["isEnabled"] == false,
        ),
        ("/autoExposure/isEnabled", true),
        (
            "/autoExposure/exposureValue",
            cache["autoExposure"]["exposureValue"]
                .as_f64()
                .is_some_and(|v| v != 0.),
        ),
        ("/lowLightCompensation", true),
        ("/antiFlicker", true),
    ] {
        if gate
            && cache
                .pointer(path)
                .is_some_and(|v| !v.is_null() && Some(v) != camera.pointer(path))
        {
            return true;
        }
    }
    let Some(image) = profile["image"]
        .as_array()
        .and_then(|images| images.get(profile["imageMode"].as_u64().unwrap_or(0) as usize))
        .map(|v| &v["dataSet"])
    else {
        return false;
    };
    for path in [
        "/brightness",
        "/contrast",
        "/saturation",
        "/sharpness",
        "/gain",
        "/whiteBalance/isAuto",
    ] {
        if cache["currentImageSettings"]
            .pointer(path)
            .is_some_and(|v| !v.is_null() && Some(v) != image.pointer(path))
        {
            return true;
        }
    }
    cache["currentImageSettings"]["whiteBalance"]["isAuto"] == false
        && cache["currentImageSettings"]["whiteBalance"]["value"] != image["whiteBalance"]["value"]
}
fn iso_reducer(iso: u32) -> Option<Value> {
    // Preserve current Sr -> ye.hj verbatim semantics, including its log2
    // conversion; the native Pro getter returns 100..6400, not selector codes.
    let code = if iso == 0 {
        1.
    } else {
        (f64::from(iso).log2() - 10.).trunc()
    };
    (1. ..=7.).contains(&code).then(|| Value::from(code as u64))
}

/// Current MW focus task persists these legacy companions alongside its full
/// autoFocus payload. They remain local profile intent, independent of ACKs.
pub(super) fn sync_focus_legacy(profile: &mut Value) {
    if profile
        .pointer("/camera/autoFocus/isEnabled")
        .and_then(Value::as_bool)
        == Some(true)
    {
        profile["camera"]["cameraMode"] = "auto".into();
    } else if profile
        .pointer("/camera/autoFocus/isEnabled")
        .and_then(Value::as_bool)
        == Some(false)
    {
        profile["camera"]["cameraMode"] = "manual".into();
        profile["camera"]["cameraValue"] = profile["camera"]["autoFocus"]["manualFocus"].clone();
    }
}

/// Current mounted product loadActiveProfileSettings gives cameraMode and
/// cameraValue precedence over the stored autoFocus isEnabled/manualFocus.
pub(super) fn restore_focus_legacy(profile: &mut Value) {
    if profile
        .pointer("/camera/autoFocus")
        .and_then(Value::as_object)
        .is_none()
    {
        return;
    }
    let automatic = profile
        .pointer("/camera/cameraMode")
        .and_then(Value::as_str)
        == Some("auto");
    profile["camera"]["autoFocus"]["isEnabled"] = automatic.into();
    if let Some(value) = profile.pointer("/camera/cameraValue").cloned() {
        profile["camera"]["autoFocus"]["manualFocus"] = value;
    }
}

/// PU's CAMERA choice replays selected source actions; Je's zero-valued
/// companion defaults must not replace the full persisted exposure object.
/// MW taskMakerSetAutoFocus also maintains its legacy cameraMode/cameraValue.
fn settings_choice_steps(
    profile: &mut Value,
    cache: &Value,
    use_camera: bool,
    product_id: u32,
) -> Vec<(&'static str, Value)> {
    let mut actions = Vec::new();
    if !use_camera {
        actions.push(("zoom", profile.clone()));
        actions.push(("pan-tilt", profile.clone()));
        actions.push(("autofocus", profile.clone()));
        actions.push(("auto-exposure", profile.clone()));
        if profile["camera"]["autoExposure"]["isEnabled"] == false && product_id == 3596 {
            actions.push(("manual-exposure", profile.clone()));
        }
        actions.push(("focus", profile.clone()));
        actions.push(("lowLightCompensation", profile.clone()));
        actions.push(("anti-flicker", profile.clone()));
        actions.push(("image-preset", profile.clone()));
        return actions;
    }
    let mode = profile["camera"]["viewPresets"]["viewMode"].clone();
    if let Some(view) = profile["camera"]["viewPresets"]["data"]
        .as_array_mut()
        .and_then(|presets| presets.iter_mut().find(|preset| preset["mode"] == mode))
    {
        for key in ["zoom"] {
            let value = &cache["currentViewPreset"][key];
            view[key] = if key == "zoom" {
                Value::from(
                    (camera_zoom(value.as_f64().unwrap_or(100.)) / 100. * 100.).round() / 100.,
                )
            } else {
                value.clone()
            };
        }
    }
    actions.push(("zoom", profile.clone()));
    if let Some(view) = profile["camera"]["viewPresets"]["data"]
        .as_array_mut()
        .and_then(|presets| presets.iter_mut().find(|preset| preset["mode"] == mode))
    {
        for key in ["pan", "tilt"] {
            view[key] = cache["currentViewPreset"][key].clone();
        }
    }
    actions.push(("pan-tilt", profile.clone()));
    profile["camera"]["autoFocus"]["isEnabled"] = cache["autoFocus"]["isEnabled"].clone();
    profile["camera"]["cameraMode"] = if profile["camera"]["autoFocus"]["isEnabled"] == true {
        "auto".into()
    } else {
        profile["camera"]["cameraValue"] = profile["camera"]["autoFocus"]["manualFocus"].clone();
        "manual".into()
    };
    actions.push(("autofocus", profile.clone()));
    // The toggle carries the old full reducer companions. Only the following
    // exposureValue action adopts the getter cache's signed standard LONG.
    profile["camera"]["autoExposure"]["isEnabled"] = cache["autoExposure"]["isEnabled"].clone();
    actions.push(("auto-exposure", profile.clone()));
    if profile["camera"]["autoExposure"]["isEnabled"] == false {
        profile["camera"]["autoExposure"]["exposureValue"] =
            cache["autoExposure"]["exposureValue"].clone();
        if product_id == 3596 {
            actions.push(("manual-exposure", profile.clone()));
        }
    }
    profile["camera"]["autoFocus"]["manualFocus"] = cache["autoFocus"]["manualFocus"].clone();
    if profile["camera"]["autoFocus"]["isEnabled"] == false {
        profile["camera"]["cameraValue"] = profile["camera"]["autoFocus"]["manualFocus"].clone();
    }
    actions.push(("focus", profile.clone()));
    profile["camera"]["lowLightCompensation"] = cache["lowLightCompensation"].clone();
    actions.push(("lowLightCompensation", profile.clone()));
    profile["camera"]["antiFlicker"] = cache["antiFlicker"].clone();
    actions.push(("anti-flicker", profile.clone()));
    if let Some(index) = profile["image"]
        .as_array()
        .and_then(|items| items.iter().position(|item| item["name"] == "custom"))
    {
        let mut image = cache["currentImageSettings"].clone();
        image["userSettings"] = cache["currentImageSettings"].clone();
        profile["image"][index]["dataSet"] = image;
        profile["imageMode"] = index.into();
    }
    actions.push(("image-preset", profile.clone()));
    actions
}

#[cfg(test)]
fn apply_settings_choice(profile: &mut Value, cache: &Value, use_camera: bool) -> bool {
    let previous_view = profile["camera"]["viewPresets"].clone();
    settings_choice_steps(profile, cache, use_camera, 3596);
    previous_view != profile["camera"]["viewPresets"]
}

fn choice_shutter(
    product_id: u32,
    use_camera: bool,
    cache: &Value,
    profile: &Value,
) -> Option<Value> {
    if !matches!(product_id, 3595 | 3596) || profile["camera"]["autoExposure"]["isEnabled"] != false
    {
        return None;
    }
    let exposure = if use_camera {
        &cache["autoExposure"]
    } else {
        &profile["camera"]["autoExposure"]
    };
    // Current Hh/Eh schedule this even when the cached value is zero.
    exposure
        .get("shutterSpeed")
        .filter(|v| !v.is_null())
        .cloned()
}

impl SourceControls {
    pub(super) fn request_camera_cache(&mut self, publish: bool, delay: Duration) {
        if matches!(self.spec.product_id, 3594..=3596) {
            let requested = Instant::now() + delay;
            self.camera_source.cache_request = Some(match self.camera_source.cache_request {
                Some((when, existing_publish)) => {
                    (when.min(requested), existing_publish || publish)
                }
                None => (requested, publish),
            });
        }
    }
    pub(super) fn init_camera_source_state(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.spec.product_id, 3594..=3596) {
            return;
        }
        self.camera_source.dialog_focus = Some(cx.focus_handle());
        self.camera_source.task = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                if view
                    .update_in(cx, |this, window, cx| this.poll_camera_source(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        }));
    }
    fn poll_camera_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scope = self.camera_runtime.queue.scope().cloned();
        if self.camera_source.set_scope(scope.clone()) {
            cx.notify();
        }
        let Some(scope) = scope else {
            return;
        };
        if self
            .camera_source
            .choice_shutter
            .as_ref()
            .is_some_and(|(when, _)| *when <= Instant::now())
        {
            let (_, value) = self.camera_source.choice_shutter.take().unwrap();
            self.draft["camera"]["autoExposure"]["shutterSpeed"] = value;
            if self.draft["camera"]["autoExposure"]["isEnabled"] == false {
                self.request_camera_edit(&format!("{}:shutter", self.spec.product_id), window, cx);
            }
            self.sync(window, cx);
            cx.emit(SourceControlsChanged);
        }
        let (enabled, source_known) =
            self.camera_preview
                .as_ref()
                .map_or((false, false), |state| {
                    (
                        state.enabled == Some(true),
                        state.observation.selected_device_id.is_some(),
                    )
                });
        let source_focused = self.camera_source_focus();
        if self
            .camera_source
            .preview_requests_cache(source_focused, enabled, source_known)
        {
            self.request_camera_cache(true, Duration::ZERO);
        }
        if let Some(job) = &self.camera_source.job {
            let result = match job.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("camera source reader disconnected".into()))
                }
            };
            if let Some(result) = result {
                let mut job = self.camera_source.job.take().unwrap();
                job.settled = true;
                if job.scope == scope && !job.canceled.load(Ordering::Acquire) {
                    match result {
                        Ok(progress) => match job.kind {
                            Query::Cache {
                                publish,
                                footer_preview_request,
                            } => match cache_from_receipts(scope.product_id, &progress) {
                                Ok(cache) => {
                                    if publish && different(&cache, &self.draft) {
                                        self.camera_source.dialog = Some(cache.clone());
                                    }
                                    self.camera_source.cache = Some(cache);
                                    if self
                                        .camera_source
                                        .settle_footer_preview(footer_preview_request, true)
                                    {
                                        self.commit_camera_preview_enabled(true, cx);
                                    }
                                }
                                Err(error) => {
                                    self.camera_source
                                        .settle_footer_preview(footer_preview_request, false);
                                    eprintln!("camera source cache: {error:#}");
                                }
                            },
                            Query::Iso => {
                                if progress.completed {
                                    for result in progress.outcomes {
                                        let iso = match result {
                                            CameraCommandOutcome::Extension(p) => match p.value {
                                                Some(CameraExtensionValue::IsoSensitivity {
                                                    iso,
                                                    ..
                                                }) => Some(iso.unwrap_or(0)),
                                                _ => None,
                                            },
                                            CameraCommandOutcome::FamilyExposure(p) => {
                                                match p.value {
                                                    Some(
                                                        CameraFamilyExposureValue::IsoSensitivity {
                                                            iso,
                                                        },
                                                    ) => Some(iso),
                                                    _ => None,
                                                }
                                            }
                                            _ => None,
                                        };
                                        if let Some(value) = iso.and_then(iso_reducer) {
                                            self.camera_source.auto_iso = Some(value);
                                            self.sync(window, cx);
                                        }
                                    }
                                } else if let Some(error) = progress.error {
                                    eprintln!("camera source ISO: {error}");
                                }
                            }
                        },
                        Err(error) => {
                            if let Query::Cache {
                                footer_preview_request,
                                ..
                            } = job.kind
                            {
                                self.camera_source
                                    .settle_footer_preview(footer_preview_request, false);
                            }
                            eprintln!("camera source read: {error}");
                        }
                    }
                    cx.notify();
                }
            }
        }
        let poll_iso = matches!(self.spec.product_id, 3594 | 3595)
            && self.draft["camera"]["autoExposure"]["isEnabled"] == true
            && self
                .camera_preview
                .as_ref()
                .is_some_and(|s| s.enabled == Some(true));
        if !poll_iso {
            self.camera_source.next_iso = None;
        } else if self.camera_source.next_iso.is_none() {
            self.camera_source.next_iso = Some(Instant::now() + Duration::from_secs(3));
        }
        if self.camera_source.job.is_some() {
            return;
        }
        let now = Instant::now();
        let kind = if self
            .camera_source
            .cache_request
            .is_some_and(|(when, _)| when <= now)
        {
            Query::Cache {
                publish: self.camera_source.cache_request.take().unwrap().1,
                footer_preview_request: self.camera_source.footer_enable_after_cache,
            }
        } else if self.camera_source.next_iso.is_some_and(|when| when <= now) {
            self.camera_source.next_iso = Some(now + Duration::from_secs(3));
            Query::Iso
        } else {
            return;
        };
        let plan = match kind {
            Query::Cache { .. } => cache_plan(),
            Query::Iso => CameraSubmissionPlan {
                commands: vec![if self.spec.product_id == 3595 {
                    CameraCommand::FamilyExposure(X::GetIsoSensitivity)
                } else {
                    CameraCommand::Extension(E::GetIsoSensitivity)
                }],
            },
        };
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let target = scope.clone();
        let jobs = self.camera_runtime.jobs.spawner();
        match self
            .camera_runtime
            .jobs
            .spawn(
                "camera-source-query",
                canceled.clone(),
                move || match kind {
                    Query::Cache { .. } => {
                        run_cache_getters(jobs, signal.clone(), move |command| {
                            razer_service::camera::commands::execute(
                                target.clone(),
                                CameraSubmissionPlan {
                                    commands: vec![command],
                                },
                                signal.clone(),
                                None,
                            )
                        })
                    }
                    Query::Iso => {
                        razer_service::camera::commands::execute(target, plan, signal, None)
                    }
                },
            ) {
            Ok(receiver) => {
                self.camera_source.job = Some(Job {
                    kind,
                    scope,
                    canceled,
                    receiver,
                    settled: false,
                })
            }
            Err(error) => {
                if let Query::Cache {
                    footer_preview_request,
                    ..
                } = kind
                {
                    self.camera_source
                        .settle_footer_preview(footer_preview_request, false);
                }
                eprintln!("camera source query worker: {error}");
            }
        }
    }

    fn choose_camera_settings(
        &mut self,
        camera: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(cache) = self.camera_source.dialog.take() else {
            return;
        };
        if let Some(iso) = &self.camera_source.auto_iso {
            self.draft["camera"]["autoExposure"]["isoSensitivity"] = iso.clone();
        }
        self.camera_source.auto_iso = None;
        let previous_view = self.draft["camera"]["viewPresets"].clone();
        let actions = settings_choice_steps(&mut self.draft, &cache, camera, self.spec.product_id);
        self.camera_footer.changed |= previous_view != self.draft["camera"]["viewPresets"];
        self.camera_source.selected_settings = true;
        for (action, profile) in actions {
            self.request_camera_profile_edit(
                &format!("{}:{action}", self.spec.product_id),
                &profile,
                window,
                cx,
            );
        }
        self.camera_source.choice_shutter =
            choice_shutter(self.spec.product_id, camera, &cache, &self.draft)
                .map(|value| (Instant::now() + Duration::from_millis(500), value));
        self.request_camera_cache(false, Duration::from_millis(500));
        self.sync(window, cx);
        cx.emit(SourceControlsChanged);
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub(super) fn render_camera_settings_difference(&self, cx: &mut Context<Self>) -> AnyElement {
        use razer_widgets::theme::CameraProductColors as Colors;
        if self.camera_source.dialog.is_none() {
            return div().into_any_element();
        }
        let button = |id: &'static str, label: &str, camera: bool| {
            razer_widgets::keyboard_navigation::Button::new(id)
                .accessibility_label(razer_i18n::t(label))
                .h(surface::css(27.))
                .min_w(surface::css(90.))
                .px(surface::css(12.))
                .py(surface::css(6.))
                .mx(surface::css(5.))
                .rounded(surface::css(3.))
                .bg(Colors::border())
                .text_color(Colors::text())
                .text_size(surface::css(12.))
                .line_height(surface::css(16.))
                .cursor_pointer()
                .hover(|b| b.opacity(0.8))
                .focus_visible(|b| b.border_1().border_color(Colors::focus()))
                .child(razer_i18n::t(label).to_uppercase())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.choose_camera_settings(camera, window, cx)
                }))
        };
        let body = v_flex()
            .w_full()
            .max_w(surface::css(400.))
            .px(surface::css(30.))
            .py(surface::css(20.))
            .bg(Colors::background())
            .border_1()
            .border_color(Colors::focus())
            .rounded(surface::css(5.))
            .text_color(Colors::text())
            .font_family("Roboto")
            .text_center()
            .text_size(surface::css(14.))
            .child(
                div()
                    .mb(surface::css(20.))
                    .child(razer_i18n::t("CAMERA_SETTINGS_DIFFERENCE_DESC")),
            )
            .child(
                h_flex()
                    .justify_center()
                    .child(button("camera-use-device-settings", "CAMERA", true))
                    .child(button("camera-use-profile-settings", "SYNAPSE", false)),
            );
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.camera_source.dialog_focus.as_ref().unwrap().clone())
            .close_on_escape(false)
            .close_on_backdrop_press(false)
            .on_cancel(|_, _, _| false)
            .on_ok(|_, _, _| false)
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .bg(super::camera_theme::modal_backdrop()),
            )
            .popup(razer_assets::resource_scope(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .px(surface::css(15.))
                    .py(surface::css(30.))
                    .child(body),
            ))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        State, apply_settings_choice, cache_from_receipts, cache_plan, choice_shutter, different,
        iso_reducer, restore_focus_legacy, sync_focus_legacy,
    };
    use razer_device::{
        camera::{
            CameraControlProperty as C, CameraLongObservation, StandardCameraProperty as P,
            StandardCameraRead, VideoProcAmpProperty as V,
        },
        camera_extensions::CameraExtensionRequest as E,
        camera_submission::{
            CameraCommand, CameraCommandOutcome, CameraCommandTransport, CameraScope,
            CameraSubmissionProgress, compile_camera_edit, execute_camera_submission,
        },
    };
    use std::{sync::atomic::AtomicBool, time::Instant};

    #[test]
    fn camera_focus_profile_legacy_fields_follow_edit_and_load_source_precedence() {
        let mut profile = serde_json::json!({"camera": {
            "autoFocus": {"isEnabled": false, "manualFocus": 73, "mode": 1, "tracking": 1, "lightingType": 0},
            "cameraMode": "auto", "cameraValue": 100
        }});
        sync_focus_legacy(&mut profile);
        assert_eq!(profile["camera"]["cameraMode"], "manual");
        assert_eq!(profile["camera"]["cameraValue"], 73);
        profile["camera"]["autoFocus"]["isEnabled"] = true.into();
        profile["camera"]["autoFocus"]["manualFocus"] = 99.into();
        sync_focus_legacy(&mut profile);
        assert_eq!(profile["camera"]["cameraMode"], "auto");
        assert_eq!(profile["camera"]["cameraValue"], 73);
        restore_focus_legacy(&mut profile);
        assert_eq!(profile["camera"]["autoFocus"]["isEnabled"], true);
        assert_eq!(profile["camera"]["autoFocus"]["manualFocus"], 73);
        profile["camera"]["cameraMode"] = "manual".into();
        profile["camera"]["cameraValue"] = 0.into();
        restore_focus_legacy(&mut profile);
        assert_eq!(profile["camera"]["autoFocus"]["isEnabled"], false);
        assert_eq!(profile["camera"]["autoFocus"]["manualFocus"], 0);
        assert_eq!(profile["camera"]["autoFocus"]["mode"], 1);
    }

    fn scope(container: &str) -> CameraScope {
        CameraScope {
            product_id: 3594,
            container_id: container.into(),
        }
    }

    #[test]
    fn first_device_binding_keeps_an_explicit_pending_cache_request() {
        let mut state = State::default();
        let request = Some((Instant::now(), true));
        state.cache_request = request;
        state.request_footer_preview();
        let preview = state.footer_enable_after_cache;
        assert!(state.set_scope(Some(scope("first"))));
        assert_eq!(state.cache_request, request);
        assert_eq!(state.footer_enable_after_cache, preview);
        assert!(!state.set_scope(Some(scope("first"))));
        assert_eq!(state.cache_request, request);
    }

    #[test]
    fn family_footer_preview_waits_for_its_own_read_and_discards_failed_or_canceled_reads() {
        let mut state = State::default();
        state.set_scope(Some(scope("first")));
        state.request_footer_preview();
        let first = state.footer_enable_after_cache;
        assert!(!state.settle_footer_preview(None, true));
        assert_eq!(state.footer_enable_after_cache, first);
        state.cancel_footer_preview();
        state.request_footer_preview();
        let second = state.footer_enable_after_cache;
        assert_ne!(first, second);
        for succeeded in [false, true] {
            assert!(!state.settle_footer_preview(first, succeeded));
            assert_eq!(state.footer_enable_after_cache, second);
        }
        assert!(!state.settle_footer_preview(second, false));
        assert!(state.footer_enable_after_cache.is_none());
        state.request_footer_preview();
        let third = state.footer_enable_after_cache;
        assert!(state.settle_footer_preview(third, true));
        assert!(state.footer_enable_after_cache.is_none());
        assert!(!state.settle_footer_preview(third, true));
        state.request_footer_preview();
        let canceled = state.footer_enable_after_cache;
        state.cancel_footer_preview();
        assert!(!state.settle_footer_preview(canceled, true));
    }

    #[test]
    fn device_replacement_and_removal_discard_previous_scope_cache_requests() {
        let mut state = State::default();
        state.set_scope(Some(scope("first")));
        for next in [Some(scope("second")), None] {
            state.cache_request = Some((Instant::now(), true));
            state.cache = Some(serde_json::json!({"brightness": 17}));
            state.dialog = state.cache.clone();
            state.auto_iso = Some(2.into());
            state.preview_cache_requested = true;
            state.choice_shutter = Some((Instant::now(), 0.into()));
            state.request_footer_preview();
            let preview = state.footer_enable_after_cache;
            assert!(state.set_scope(next));
            assert!(state.cache_request.is_none());
            assert!(state.cache.is_none());
            assert!(state.dialog.is_none());
            assert!(state.auto_iso.is_none());
            assert!(!state.preview_cache_requested);
            assert!(state.choice_shutter.is_none());
            assert!(state.footer_enable_after_cache.is_none());
            assert!(!state.settle_footer_preview(preview, true));
        }
    }

    #[derive(Default)]
    struct SourceGetters {
        fail: Option<P>,
    }
    impl CameraCommandTransport for SourceGetters {
        fn execute(
            &mut self,
            command: &CameraCommand,
            validate: &impl Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<CameraCommandOutcome> {
            validate()?;
            let CameraCommand::StandardRead(property) = command else {
                anyhow::bail!("unexpected setting in source cache query");
            };
            anyhow::ensure!(
                Some(*property) != self.fail,
                "simulated DirectShow Get failure"
            );
            let value = match property {
                P::VideoProcAmp(V::Brightness) => 17,
                P::VideoProcAmp(V::Contrast) => 18,
                P::VideoProcAmp(V::Saturation) => 19,
                P::VideoProcAmp(V::WhiteBalance) => 3000,
                P::VideoProcAmp(V::AntiFlicker) => 2,
                P::CameraControl(C::Zoom) => 150,
                P::CameraControl(C::Pan) => 5,
                P::CameraControl(C::Tilt) => -5,
                P::CameraControl(C::Focus) => 43,
                P::CameraControl(C::Exposure) => -7,
                P::CameraControl(C::LowLightCompensation) => 1,
                _ => anyhow::bail!("unexpected source getter"),
            };
            Ok(CameraCommandOutcome::Read(StandardCameraRead {
                property: *property,
                observation: CameraLongObservation::from_native(value, 2, 0)?,
            }))
        }
    }

    #[test]
    fn property_close_source_cache_camera_and_synapse_choices_keep_correct_writeback_values() {
        let mut profile = super::super::specs()
            .iter()
            .find(|p| p.product_id == 3594)
            .unwrap()
            .profile
            .clone();
        let original = profile.clone();
        let progress =
            execute_camera_submission(&mut SourceGetters::default(), 3594, &cache_plan(), &|| {
                Ok(())
            });
        assert!(progress.completed);
        let cache = cache_from_receipts(3594, &progress).unwrap();
        // Je deliberately masks standard exposure for this extended-UVC model.
        assert_eq!(cache["autoExposure"]["exposureValue"], 0);
        assert!(different(&cache, &profile));
        assert!(!apply_settings_choice(&mut profile, &cache, false));
        assert_eq!(profile, original);
        assert!(apply_settings_choice(&mut profile, &cache, true));
        assert!(!different(&cache, &profile));
        assert_eq!(profile["camera"]["viewPresets"]["data"][0]["zoom"], 1.5);
        assert_eq!(
            profile["camera"]["viewPresets"]["data"][1],
            original["camera"]["viewPresets"]["data"][1]
        );
        assert_eq!(profile["camera"]["cameraMode"], "manual");
        assert_eq!(profile["camera"]["cameraValue"], 43);
        assert_eq!(
            profile["camera"]["autoExposure"]["isoSensitivity"],
            original["camera"]["autoExposure"]["isoSensitivity"]
        );
        assert_eq!(
            profile["camera"]["autoExposure"]["shutterSpeed"],
            original["camera"]["autoExposure"]["shutterSpeed"]
        );
        let focus = compile_camera_edit(3594, "autofocus", &profile).unwrap();
        assert!(matches!(focus.commands.as_slice(), [
            CameraCommand::StandardWrite(setting),
            CameraCommand::Extension(E::SetFdafMode { mode: 1, light_condition: 0 }),
        ] if setting.setting.value == 43 && !setting.setting.is_auto));
        let exposure = compile_camera_edit(3594, "auto-exposure", &profile).unwrap();
        assert!(matches!(exposure.commands.as_slice(), [
            CameraCommand::StandardWrite(setting),
            CameraCommand::Extension(E::SetIsoSensitivity { code: 4 }),
            CameraCommand::Extension(E::SetShutterSpeed { value: 16666 }),
        ] if setting.setting.value == 0 && !setting.setting.is_auto));
        assert_eq!(profile["imageMode"], 4);
        assert_eq!(profile["image"][0], original["image"][0]);
        let image = compile_camera_edit(3594, "image-preset", &profile).unwrap();
        let image_values: Vec<_> = image
            .commands
            .iter()
            .map(|command| match command {
                CameraCommand::StandardWrite(setting) => {
                    (setting.setting.value, setting.setting.is_auto)
                }
                _ => panic!("unexpected nonstandard source image command"),
            })
            .collect();
        assert_eq!(
            image_values,
            [(17, false), (18, false), (19, false), (3000, false)]
        );
        assert_eq!(
            profile["image"][4]["dataSet"]["userSettings"],
            cache["currentImageSettings"]
        );
    }

    #[test]
    fn failed_source_getter_does_not_become_a_partial_camera_choice() {
        let progress = execute_camera_submission(
            &mut SourceGetters {
                fail: Some(P::VideoProcAmp(V::Contrast)),
            },
            3594,
            &cache_plan(),
            &|| Ok(()),
        );
        assert!(!progress.completed);
        assert_eq!(progress.outcomes.len(), 1);
        assert!(cache_from_receipts(3594, &progress).is_err());
    }

    #[test]
    fn current_v2_and_x_cache_choices_keep_each_models_exposure_value_and_delayed_shutter() {
        for product_id in [3595, 3596] {
            let progress = execute_camera_submission(
                &mut SourceGetters::default(),
                product_id,
                &cache_plan(),
                &|| Ok(()),
            );
            let cache = cache_from_receipts(product_id, &progress).unwrap();
            assert_eq!(
                cache["autoExposure"]["exposureValue"],
                if product_id == 3596 { -7 } else { 0 }
            );
            let mut profile = super::super::specs()
                .iter()
                .find(|spec| spec.product_id == product_id)
                .unwrap()
                .profile
                .clone();
            let original = profile.clone();
            assert!(different(&cache, &profile));
            assert!(!apply_settings_choice(&mut profile, &cache, false));
            assert_eq!(profile, original);
            assert!(apply_settings_choice(&mut profile, &cache, true));
            assert!(!different(&cache, &profile));
            for companion in ["metering", "compensation", "isoSensitivity", "shutterSpeed"] {
                assert_eq!(
                    profile["camera"]["autoExposure"][companion],
                    original["camera"]["autoExposure"][companion]
                );
            }
            // The UI's delayed source action writes the getter cache's zero,
            // without assigning its other placeholder companions to storage.
            assert_eq!(
                choice_shutter(product_id, true, &cache, &profile),
                Some(0.into())
            );
            assert_eq!(
                choice_shutter(product_id, false, &cache, &profile),
                Some(16666.into())
            );
            if product_id == 3596 {
                let manual = compile_camera_edit(product_id, "manual-exposure", &profile).unwrap();
                assert!(
                    matches!(manual.commands.as_slice(), [CameraCommand::StandardWrite(setting)]
                    if setting.setting.value == -7 && !setting.setting.is_auto)
                );
            }
            profile["camera"]["autoExposure"]["isEnabled"] = true.into();
            assert_eq!(choice_shutter(product_id, true, &cache, &profile), None);
        }
        assert_eq!(
            choice_shutter(3594, true, &serde_json::json!({}), &serde_json::json!({})),
            None
        );
    }

    #[test]
    fn automatic_standard_exposure_never_publishes_its_manual_long_to_x_cache() {
        let mut progress =
            execute_camera_submission(&mut SourceGetters::default(), 3596, &cache_plan(), &|| {
                Ok(())
            });
        for result in &mut progress.outcomes {
            if let CameraCommandOutcome::Read(read) = result
                && read.property == P::CameraControl(C::Exposure)
            {
                read.observation = CameraLongObservation::from_native(-7, 1, 0).unwrap();
            }
        }
        let cache = cache_from_receipts(3596, &progress).unwrap();
        assert_eq!(cache["autoExposure"]["isEnabled"], true);
        assert_eq!(cache["autoExposure"]["exposureValue"], 0);
    }

    #[test]
    fn source_cache_preview_effect_requires_focus_enabled_preview_and_known_source() {
        let mut state = State::default();
        assert!(!state.preview_requests_cache(false, true, true));
        assert!(!state.preview_requests_cache(true, false, true));
        assert!(!state.preview_requests_cache(true, true, false));
        assert!(state.preview_requests_cache(true, true, true));
        // Source/resolution changes do not clear EU's request ref.
        assert!(!state.preview_requests_cache(true, true, false));
        assert!(!state.preview_requests_cache(true, true, true));
        assert!(!state.preview_requests_cache(false, true, true));
        assert!(state.preview_requests_cache(true, true, true));
        assert!(!state.preview_requests_cache(true, false, true));
        assert!(state.preview_requests_cache(true, true, true));
    }
    #[test]
    fn camera_choice_snapshots_toggle_old_companions_before_cached_partial_actions() {
        for product_id in [3594, 3595, 3596] {
            let mut profile = super::super::specs()
                .iter()
                .find(|p| p.product_id == product_id)
                .unwrap()
                .profile
                .clone();
            profile["camera"]["autoFocus"]["isEnabled"] = true.into();
            profile["camera"]["autoFocus"]["manualFocus"] = 27.into();
            profile["camera"]["autoExposure"]["isEnabled"] = true.into();
            profile["camera"]["autoExposure"]["exposureValue"] = (-3).into();
            let cache = cache_from_receipts(
                product_id,
                &execute_camera_submission(
                    &mut SourceGetters::default(),
                    product_id,
                    &cache_plan(),
                    &|| Ok(()),
                ),
            )
            .unwrap();
            let steps = super::settings_choice_steps(&mut profile, &cache, true, product_id);
            let names: Vec<_> = steps.iter().map(|(name, _)| *name).collect();
            let mut expected = vec!["zoom", "pan-tilt", "autofocus", "auto-exposure"];
            if product_id == 3596 {
                expected.push("manual-exposure");
            }
            expected.extend([
                "focus",
                "lowLightCompensation",
                "anti-flicker",
                "image-preset",
            ]);
            assert_eq!(names, expected);
            let focus = &steps
                .iter()
                .find(|(name, _)| *name == "autofocus")
                .unwrap()
                .1;
            assert_eq!(focus["camera"]["autoFocus"]["manualFocus"], 27);
            assert_eq!(focus["camera"]["autoFocus"]["isEnabled"], false);
            let exposure = &steps
                .iter()
                .find(|(name, _)| *name == "auto-exposure")
                .unwrap()
                .1;
            assert_eq!(exposure["camera"]["autoExposure"]["exposureValue"], -3);
            let later_focus = &steps.iter().find(|(name, _)| *name == "focus").unwrap().1;
            assert_eq!(later_focus["camera"]["autoFocus"]["manualFocus"], 43);
            assert_eq!(
                profile["camera"]["autoExposure"]["exposureValue"],
                cache["autoExposure"]["exposureValue"]
            );
        }
    }
    #[test]
    fn cache_getter_batches_issue_all_siblings_before_advancing_to_the_next_group() {
        use std::sync::{Arc, Condvar, Mutex, mpsc};
        use std::time::Duration;
        let gate = Arc::new((Mutex::new(0usize), Condvar::new()));
        let worker_gate = gate.clone();
        let (entered, observed) = mpsc::channel();
        let owner = razer_service::camera::lifecycle::Owner::default();
        let jobs = owner.spawner();
        let worker = std::thread::spawn(move || {
            super::run_cache_getters(jobs, Arc::new(AtomicBool::new(false)), move |command| {
                let CameraCommand::StandardRead(property) = command else {
                    panic!("getter expected")
                };
                let group = if [
                    P::VideoProcAmp(V::Brightness),
                    P::VideoProcAmp(V::Contrast),
                    P::VideoProcAmp(V::Saturation),
                    P::VideoProcAmp(V::WhiteBalance),
                ]
                .contains(&property)
                {
                    1
                } else {
                    2
                };
                entered.send(group).unwrap();
                let (lock, ready) = &*worker_gate;
                let value = lock.lock().unwrap();
                let (guard, timeout) = ready
                    .wait_timeout_while(value, Duration::from_secs(3), |value| *value < group)
                    .unwrap();
                drop(guard);
                assert!(!timeout.timed_out());
                Ok(CameraSubmissionProgress {
                    completed: true,
                    ..Default::default()
                })
            })
        });
        for _ in 0..4 {
            assert_eq!(observed.recv_timeout(Duration::from_secs(2)).unwrap(), 1);
        }
        assert!(observed.try_recv().is_err());
        *gate.0.lock().unwrap() = 1;
        gate.1.notify_all();
        for _ in 0..7 {
            assert_eq!(observed.recv_timeout(Duration::from_secs(2)).unwrap(), 2);
        }
        *gate.0.lock().unwrap() = 2;
        gate.1.notify_all();
        assert!(worker.join().unwrap().unwrap().completed);
    }
    #[test]
    fn cache_group_rejection_does_not_wait_for_or_cancel_issued_siblings() {
        use std::sync::{Arc, Condvar, Mutex, mpsc};
        use std::time::Duration;
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_gate = gate.clone();
        let (entered, observed) = mpsc::channel();
        let (done, result) = mpsc::channel();
        let owner = razer_service::camera::lifecycle::Owner::default();
        let jobs = owner.spawner();
        let worker = std::thread::spawn(move || {
            done.send(super::run_cache_getters(
                jobs,
                Arc::new(AtomicBool::new(false)),
                move |command| {
                    let CameraCommand::StandardRead(property) = command else {
                        panic!("getter expected")
                    };
                    entered.send(property).unwrap();
                    if property == P::VideoProcAmp(V::Contrast) {
                        return Err("simulated group rejection".into());
                    }
                    let (lock, ready) = &*worker_gate;
                    let (guard, timeout) = ready
                        .wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(3), |value| {
                            !*value
                        })
                        .unwrap();
                    drop(guard);
                    assert!(!timeout.timed_out());
                    Ok(CameraSubmissionProgress {
                        completed: true,
                        ..Default::default()
                    })
                },
            ))
            .unwrap();
        });
        for _ in 0..4 {
            observed.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        assert_eq!(
            result
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .unwrap_err(),
            "simulated group rejection"
        );
        assert!(observed.try_recv().is_err());
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        worker.join().unwrap();
    }
    #[test]
    fn source_iso_conversion_retains_current_logarithmic_reducer() {
        assert_eq!(iso_reducer(100), None);
        assert_eq!(iso_reducer(1600), None);
        assert_eq!(iso_reducer(3200), Some(1.into()));
        assert_eq!(iso_reducer(6400), Some(2.into()));
        assert_eq!(iso_reducer(0), Some(1.into()));
    }
    #[test]
    fn source_cache_requires_complete_real_getter_receipts() {
        assert!(cache_from_receipts(3594, &CameraSubmissionProgress::default()).is_err());
        let plan = cache_plan();
        assert_eq!(plan.commands.len(), 11);
        let progress = CameraSubmissionProgress {
            completed: true,
            ..Default::default()
        };
        assert!(cache_from_receipts(3594, &progress).is_err());
    }
    #[test]
    fn source_difference_ignores_pan_at_one_times_and_manual_white_value_when_auto() {
        let mut profile = super::super::specs()
            .iter()
            .find(|p| p.product_id == 3594)
            .unwrap()
            .profile
            .clone();
        let image = &profile["image"][profile["imageMode"].as_u64().unwrap() as usize]["dataSet"];
        let mut cache = serde_json::json!({"currentViewPreset":{"zoom":100,"pan":9,"tilt":-9},"autoFocus":profile["camera"]["autoFocus"],"autoExposure":profile["camera"]["autoExposure"],"lowLightCompensation":profile["camera"]["lowLightCompensation"],"antiFlicker":profile["camera"]["antiFlicker"],"currentImageSettings":image});
        cache["currentImageSettings"]["whiteBalance"]["value"] = 7500.into();
        assert!(!different(&cache, &profile));
        profile["camera"]["viewPresets"]["data"][0]["zoom"] = 2.into();
        cache["currentViewPreset"]["zoom"] = 200.into();
        assert!(different(&cache, &profile));
        profile["camera"]["viewPresets"]["data"][0]["pan"] = 9.into();
        profile["camera"]["viewPresets"]["data"][0]["tilt"] = (-9).into();
        assert!(!different(&cache, &profile));
        cache["currentImageSettings"]["brightness"] = 0.into();
        assert!(different(&cache, &profile));
    }
}

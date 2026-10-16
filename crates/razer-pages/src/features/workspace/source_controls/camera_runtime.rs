//! Camera controls -> retained intent queue -> IPC -> direct Rust COM/KS adapter.
//! Local profile drafts, ACKs and independent observations remain distinct.
use super::*;
use razer_device::{
    camera::{CameraControlProperty as C, StandardCameraProperty as P, VideoProcAmpProperty as V},
    camera_queue::CameraIntentQueue,
    camera_submission::{
        CameraCommand, CameraCommandOutcome, CameraOperationStatus, CameraScope,
        CameraSubmissionPlan, CameraSubmissionProgress, compile_camera_edit,
    },
};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Default)]
pub(super) struct State {
    pub queue: CameraIntentQueue,
    pub observations: BTreeMap<String, Value>,
    pub window: Option<AnyWindowHandle>,
    workers: BTreeMap<u64, Worker>,
    properties_canceled: Option<Arc<AtomicBool>>,
}
struct Worker {
    canceled: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}
impl Drop for State {
    fn drop(&mut self) {
        for worker in self.workers.values() {
            worker.canceled.store(true, Ordering::Release);
        }
        if let Some(signal) = &self.properties_canceled {
            signal.store(true, Ordering::Release);
        }
    }
}
impl State {
    pub(super) fn new(window: AnyWindowHandle) -> Self {
        let mut state = Self::default();
        state.window = Some(window);
        state
    }
    pub(super) fn invalidate(&mut self) {
        self.queue.invalidate();
        self.observations.clear();
        // No source cancelAllTasks caller exists for page/profile replacement.
        // The native property frame belongs to the mounted device, not its
        // active setting tab or profile. Current MW Ve/He keeps its close
        // listener across both changes. Scope replacement and Drop cancel it.
    }

    fn finish_properties(
        &mut self,
        scope: &CameraScope,
        canceled: &Arc<AtomicBool>,
        result: &Result<CameraSubmissionProgress, String>,
    ) -> bool {
        if !self
            .properties_canceled
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, canceled))
        {
            return false;
        }
        self.properties_canceled = None;
        self.queue.scope() == Some(scope)
            && !canceled.load(Ordering::Acquire)
            && result.as_ref().is_ok_and(|progress| {
                progress
                    .outcomes
                    .iter()
                    .any(|outcome| matches!(outcome, CameraCommandOutcome::PropertiesClosed))
            })
    }
}

pub(super) fn execute_job(
    scope: CameraScope,
    plan: CameraSubmissionPlan,
    signal: Arc<AtomicBool>,
    stopped: Option<mpsc::Receiver<Result<(), String>>>,
) -> Result<CameraSubmissionProgress, String> {
    execute_task_job(scope, plan, signal, stopped, None)
}
fn execute_task_job(
    scope: CameraScope,
    plan: CameraSubmissionPlan,
    signal: Arc<AtomicBool>,
    stopped: Option<mpsc::Receiver<Result<(), String>>>,
    task_name: Option<String>,
) -> Result<CameraSubmissionProgress, String> {
    let run = || -> anyhow::Result<CameraSubmissionProgress> {
        anyhow::ensure!(
            !signal.load(Ordering::Acquire),
            "camera operation canceled before connection"
        );
        if let Some(stopped) = stopped {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                anyhow::ensure!(
                    !signal.load(Ordering::Acquire),
                    "camera operation canceled while stopping capture"
                );
                match stopped.recv_timeout(Duration::from_millis(40)) {
                    Ok(result) => {
                        result.map_err(anyhow::Error::msg)?;
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        anyhow::bail!("camera stop receipt disconnected")
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        anyhow::ensure!(Instant::now() < deadline, "camera capture stop timed out")
                    }
                }
            }
        }
        let properties = plan
            .commands
            .iter()
            .any(|c| matches!(c, CameraCommand::ShowProperties));
        let mut client = ServiceClient::spawn()?;
        let id = uuid::Uuid::new_v4().to_string();
        let named = task_name.is_some();
        let result = (|| -> anyhow::Result<CameraSubmissionProgress> {
            client.request(match task_name {
                Some(task_name) => ServiceRequest::CameraTaskStart {
                    operation_id: id.clone(),
                    scope,
                    task_name,
                    plan,
                },
                None => ServiceRequest::CameraStart {
                    operation_id: id.clone(),
                    scope,
                    plan,
                },
            })?;
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut cancel_sent = false;
            loop {
                if signal.load(Ordering::Acquire) && !cancel_sent {
                    client.request(ServiceRequest::CameraCancel {
                        operation_id: id.clone(),
                    })?;
                    cancel_sent = true;
                }
                let status: CameraOperationStatus =
                    serde_json::from_value(client.request(ServiceRequest::CameraPoll {
                        operation_id: id.clone(),
                    })?)?;
                if let CameraOperationStatus::Finished(p) = status {
                    return Ok(p);
                }
                anyhow::ensure!(
                    properties || named || Instant::now() < deadline,
                    "camera response timed out; device outcome remains unconfirmed"
                );
                thread::sleep(Duration::from_millis(40));
            }
        })();
        // The worker joins the camera thread and releases COM before its reply.
        let cleanup = client.request(ServiceRequest::Shutdown);
        match (result, cleanup) {
            (Ok(mut p), Err(e)) => {
                p.completed = false;
                p.error = Some(format!(
                    "{}camera cleanup failed: {e:#}",
                    p.error.map(|s| format!("{s}; ")).unwrap_or_default()
                ));
                Ok(p)
            }
            (Err(e), Err(cleanup)) => anyhow::bail!("{e:#}; camera cleanup failed: {cleanup:#}"),
            (result, Ok(_)) => result,
        }
    };
    run().map_err(|e| format!("{e:#}"))
}

impl SourceControls {
    pub(super) fn request_camera_properties(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Native DisplayCameraPropertiesDialog starts an independent thread.
        // Keep its modal lifetime out of the setting queue so edits can still
        // reach the camera while the Windows property window remains open.
        if self.camera_runtime.properties_canceled.is_some() {
            return;
        }
        let Some(scope) = self.camera_runtime.queue.scope().cloned() else {
            return;
        };
        let completed_scope = scope.clone();
        let signal = Arc::new(AtomicBool::new(false));
        let canceled = signal.clone();
        let (send, receive) = mpsc::sync_channel(1);
        let worker = match thread::Builder::new()
            .name("camera-properties-ipc".into())
            .spawn(move || {
                let result = execute_job(
                    scope,
                    CameraSubmissionPlan {
                        commands: vec![CameraCommand::ShowProperties],
                    },
                    signal,
                    None,
                );
                let _ = send.send(result);
            }) {
            Ok(worker) => worker,
            Err(error) => {
                eprintln!("camera properties worker: {error}");
                return;
            }
        };
        self.camera_runtime.properties_canceled = Some(canceled.clone());
        if let Some(state) = &mut self.camera_preview {
            state.observation.windows_properties_open = true;
        }
        self.sync(window, cx);
        cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                let result = match receive.try_recv() {
                    Ok(result) => result,
                    Err(mpsc::TryRecvError::Empty) => {
                        if view.upgrade().is_none() {
                            break;
                        }
                        continue;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Err("camera property worker disconnected".into())
                    }
                };
                match &result {
                    Err(error) => eprintln!("camera property dialog: {error}"),
                    Ok(progress) => {
                        if let Some(error) = &progress.error {
                            eprintln!("camera property dialog: {error}");
                        }
                    }
                }
                let _ = worker.join();
                let _ = view.update_in(cx, |this, window, cx| {
                    if this
                        .camera_runtime
                        .finish_properties(&completed_scope, &canceled, &result)
                    {
                        if let Some(state) = &mut this.camera_preview {
                            state.observation.windows_properties_open = false;
                        }
                        // Current MW He handles the native close event with
                        // Je -> YQJ. Real getters feed PU's difference choice;
                        // closing a property frame is never a write/save ACK.
                        this.request_camera_cache(true, Duration::ZERO);
                        this.sync(window, cx);
                    }
                    cx.notify();
                });
                break;
            }
        })
        .detach();
    }
    pub fn set_camera_scope(&mut self, scope: Option<CameraScope>, cx: &mut Context<Self>) {
        if !matches!(self.spec.product_id, 3594..=3596) {
            return;
        }
        self.set_camera_media_scope(scope.clone(), cx);
        self.set_camera_microphone_scope(scope.clone(), cx);
        let changed = self.camera_runtime.queue.set_scope(scope);
        if changed {
            // Bind the source owner before another action can enqueue a query;
            // the preview poll may run up to one timer interval later.
            self.camera_source
                .set_scope(self.camera_runtime.queue.scope().cloned());
            self.camera_runtime.observations.clear();
            for worker in self.camera_runtime.workers.values() {
                worker.canceled.store(true, Ordering::Release);
            }
            if let Some(signal) = &self.camera_runtime.properties_canceled {
                signal.store(true, Ordering::Release);
            }
        }
        self.defer_camera_pump(cx);
        cx.notify();
    }
    fn defer_camera_pump(&self, cx: &mut Context<Self>) {
        let Some(handle) = self.camera_runtime.window else {
            return;
        };
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = cx.update_window(handle, |_, window, cx| {
                let _ = owner.update(cx, |this, cx| this.pump_camera(window, cx));
            });
        });
    }
    pub(super) fn request_camera_edit(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.spec.product_id, 3594..=3596) {
            return;
        }
        if key.ends_with(":resolution") || key.ends_with(":shortcut-key") {
            // Resolution is applied by the retained MF preview session.
            return;
        }
        // Current camera reducers and middleware task makers submit profile values.
        // Independent DirectShow diagnostics have no audited broadcast back into
        // those values, so they must not replace the user's source-shaped intent.
        let mut profile = self.draft.clone();
        if let Some(iso) = &self.camera_source.auto_iso {
            profile["camera"]["autoExposure"]["isoSensitivity"] = iso.clone();
        }
        self.request_camera_profile_edit(key, &profile, window, cx);
    }
    pub(super) fn request_camera_profile_edit(
        &mut self,
        key: &str,
        profile: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = compile_camera_edit(self.spec.product_id, key, profile)
            .and_then(|plan| self.camera_runtime.queue.edit(key.into(), plan));
        if let Err(e) = result {
            self.camera_runtime.queue.error = Some(e.to_string());
            eprintln!("camera submission diagnostic: {e:#}");
        }
        self.pump_camera(window, cx);
        cx.notify();
    }
    pub(super) fn pump_camera(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.camera_runtime.queue.scope().is_none() {
            return;
        }
        if self.camera_runtime.queue.read_needed()
            && !self.camera_runtime.queue.has_waiting_intents()
            && !self.camera_runtime.queue.pending()
        {
            let properties = match self.page.as_str() {
                "IMAGE" => vec![
                    P::VideoProcAmp(V::Brightness),
                    P::VideoProcAmp(V::Contrast),
                    P::VideoProcAmp(V::Saturation),
                    P::VideoProcAmp(V::WhiteBalance),
                    P::VideoProcAmp(V::AntiFlicker),
                ],
                "CAMERA" => vec![
                    P::CameraControl(C::Zoom),
                    P::CameraControl(C::Pan),
                    P::CameraControl(C::Tilt),
                    P::CameraControl(C::Focus),
                    P::CameraControl(C::Exposure),
                ],
                "PROCESSING" => vec![P::CameraControl(C::LowLightCompensation)],
                _ => vec![],
            };
            if !properties.is_empty() {
                if let Err(e) = self.camera_runtime.queue.read(CameraSubmissionPlan {
                    commands: properties
                        .into_iter()
                        .map(CameraCommand::StandardRead)
                        .collect(),
                }) {
                    self.camera_runtime.queue.observation_error = Some(e.to_string());
                    cx.notify();
                    return;
                }
            }
        }
        while let Some(intent) = self.camera_runtime.queue.next() {
            let capture_control = intent.plan.commands.iter().any(|command| {
                matches!(
                    command,
                    CameraCommand::Extension(
                        razer_device::camera_extensions::CameraExtensionRequest::SetHdr { .. }
                    )
                    | CameraCommand::FamilyProcessing(
                        razer_device::camera_processing_family::CameraFamilyProcessingRequest::SetHdr { .. }
                        | razer_device::camera_processing_family::CameraFamilyProcessingRequest::SetLdc { .. }
                        | razer_device::camera_processing_family::CameraFamilyProcessingRequest::SetAdaptiveMjpegQuality { .. }
                        | razer_device::camera_processing_family::CameraFamilyProcessingRequest::SetWatermark { .. }
                    )
                )
            });
            let stopped = if capture_control {
                match self.lock_camera_capture(cx) {
                    Ok(receipt) => receipt,
                    Err(error) => {
                        self.camera_runtime.queue.finish(&intent, Err(error));
                        self.finish_camera_capture_control(cx);
                        self.defer_camera_pump(cx);
                        continue;
                    }
                }
            } else {
                None
            };
            let signal = Arc::new(AtomicBool::new(false));
            let canceled = signal.clone();
            let (sender, receiver) = mpsc::sync_channel(1);
            let scope = intent.scope.clone();
            let plan = intent.plan.clone();
            // Restore defaults is a direct source handler, outside RzTaskManager.
            let task_name = (intent.key != "read" && !intent.key.ends_with(":restore-camera"))
                .then(|| intent.task_name.clone());
            match thread::Builder::new()
                .name("camera-page-ipc".into())
                .spawn(move || {
                    let _ = sender.send(execute_task_job(scope, plan, signal, stopped, task_name));
                }) {
                Ok(thread) => {
                    self.camera_runtime
                        .workers
                        .insert(intent.generation, Worker { canceled, thread });
                }
                Err(e) => {
                    self.camera_runtime
                        .queue
                        .finish(&intent, Err(e.to_string()));
                    if capture_control {
                        self.finish_camera_capture_control(cx);
                    }
                    cx.notify();
                    continue;
                }
            }
            cx.spawn_in(window, async move |view, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(40))
                        .await;
                    let result = match receiver.try_recv() {
                        Ok(result) => Some(result),
                        Err(mpsc::TryRecvError::Empty) => None,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            Some(Err("camera page worker disconnected".into()))
                        }
                    };
                    let Some(result) = result else {
                        if view.upgrade().is_none() {
                            break;
                        }
                        continue;
                    };
                    let _ = view.update_in(cx, |this, window, cx| {
                        let progress = result.as_ref().ok().cloned();
                        if capture_control
                            && this.camera_runtime.queue.scope() == Some(&intent.scope)
                        {
                            this.finish_camera_capture_control(cx);
                        }
                        if intent.key.ends_with(":save-camera")
                            && this.camera_runtime.queue.scope() == Some(&intent.scope)
                        {
                            this.schedule_camera_media_restart(cx);
                        }
                        if this.camera_runtime.queue.finish(&intent, result) {
                            if intent.key.ends_with(":restore-camera")
                                && progress.as_ref().is_some_and(|p| p.completed)
                            {
                                this.camera_restore_completed(window, cx);
                            }
                            if let Some(p) = progress {
                                this.publish_camera_observations(&p);
                            }
                            // This is a separate Rust observation request, not an
                            // implicit setter ACK or proof of persistent storage.
                            // The queue retains refresh-needed after both writes
                            // and failed/partial reads, without retrying a failed
                            // current getter in this completion callback.
                            if let Some(error) = this.camera_runtime.queue.error.as_ref().or(this
                                .camera_runtime
                                .queue
                                .observation_error
                                .as_ref())
                            {
                                eprintln!("camera transport diagnostic: {error}");
                            }
                            this.sync(window, cx);
                        }
                        if let Some(worker) = this.camera_runtime.workers.remove(&intent.generation)
                        {
                            let _ = worker.thread.join();
                        }
                        if this.camera_runtime.queue.continue_after_completion() {
                            // RzTaskManager schedules same-name pending selection
                            // on the next event-loop turn (setTimeout).
                            this.defer_camera_pump(cx);
                        }
                        cx.notify();
                    });
                    break;
                }
            })
            .detach();
        }
    }
    fn publish_camera_observations(&mut self, progress: &CameraSubmissionProgress) {
        for outcome in &progress.outcomes {
            let CameraCommandOutcome::Read(read) = outcome else {
                continue;
            };
            let o = read.observation;
            let bindings = match read.property {
                P::VideoProcAmp(V::Brightness) => {
                    vec![("@image/brightness", serde_json::json!(o.value))]
                }
                P::VideoProcAmp(V::Contrast) => {
                    vec![("@image/contrast", serde_json::json!(o.value))]
                }
                P::VideoProcAmp(V::Saturation) => {
                    vec![("@image/saturation", serde_json::json!(o.value))]
                }
                P::VideoProcAmp(V::WhiteBalance) => vec![
                    ("@image/whiteBalance/value", serde_json::json!(o.value)),
                    ("@image/whiteBalance/isAuto", Value::Bool(o.is_auto())),
                ],
                P::VideoProcAmp(V::AntiFlicker) => {
                    vec![("/camera/antiFlicker", serde_json::json!(o.value))]
                }
                P::CameraControl(C::Zoom) => {
                    vec![("@view/zoom", serde_json::json!(f64::from(o.value) / 100.))]
                }
                P::CameraControl(C::Pan) => vec![("@view/pan", serde_json::json!(o.value))],
                P::CameraControl(C::Tilt) => vec![("@view/tilt", serde_json::json!(o.value))],
                P::CameraControl(C::Focus) => vec![
                    ("/camera/autoFocus/manualFocus", serde_json::json!(o.value)),
                    ("/camera/autoFocus/isEnabled", Value::Bool(o.is_auto())),
                ],
                P::CameraControl(C::Exposure) => {
                    vec![("/camera/autoExposure/isEnabled", Value::Bool(o.is_auto()))]
                }
                P::CameraControl(C::LowLightCompensation) => {
                    vec![("/camera/lowLightCompensation", Value::Bool(o.value != 0))]
                }
                _ => vec![],
            };
            for (path, value) in bindings {
                self.camera_runtime
                    .observations
                    .insert(self.resolve_path(path), value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use razer_device::camera_submission::{
        CameraCommandOutcome, CameraScope, CameraSubmissionProgress,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    fn scope() -> CameraScope {
        CameraScope {
            product_id: 3594,
            container_id: "{01234567-89ab-cdef-0123-456789abcdef}".into(),
        }
    }
    fn state() -> (State, Arc<AtomicBool>) {
        let signal = Arc::new(AtomicBool::new(false));
        let mut state = State::default();
        state.queue.set_scope(Some(scope()));
        state.properties_canceled = Some(signal.clone());
        (state, signal)
    }
    fn closed() -> Result<CameraSubmissionProgress, String> {
        Ok(CameraSubmissionProgress {
            outcomes: vec![CameraCommandOutcome::PropertiesClosed],
            completed: true,
            ..Default::default()
        })
    }

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn properties_close_requests_source_getters_after_tab_or_profile_replacement() {
        let (mut state, signal) = state();
        state.invalidate();
        assert!(!signal.load(Ordering::Acquire));
        assert!(state.finish_properties(&scope(), &signal, &closed()));
        assert!(state.properties_canceled.is_none());
        // A duplicate callback cannot request another getter round.
        assert!(!state.finish_properties(&scope(), &signal, &closed()));
    }

    #[test]
    fn properties_cleanup_error_preserves_a_real_close_observation() {
        let (mut state, signal) = state();
        let mut result = closed().unwrap();
        result.completed = false;
        result.error = Some("simulated service cleanup failure after frame close".into());
        assert!(state.finish_properties(&scope(), &signal, &Ok(result)));
    }

    #[test]
    fn properties_open_failure_and_missing_close_do_not_fabricate_refresh() {
        for result in [
            Err("simulated OleCreatePropertyFrame failure".into()),
            Ok(CameraSubmissionProgress {
                completed: true,
                ..Default::default()
            }),
        ] {
            let (mut state, signal) = state();
            assert!(!state.finish_properties(&scope(), &signal, &result));
            assert!(state.properties_canceled.is_none());
        }
    }

    #[test]
    fn canceled_or_old_device_properties_close_cannot_publish_to_current_device() {
        let (mut state, signal) = state();
        signal.store(true, Ordering::Release);
        assert!(!state.finish_properties(&scope(), &signal, &closed()));
        let (mut state, signal) = self::state();
        let mut other = scope();
        other.container_id = "{fedcba98-7654-3210-fedc-ba9876543210}".into();
        state.queue.set_scope(Some(other));
        assert!(!state.finish_properties(&scope(), &signal, &closed()));
        assert!(state.properties_canceled.is_none());
    }

    #[test]
    fn stale_properties_completion_keeps_new_operation_owner_and_drop_cancels_it() {
        let (mut state, signal) = state();
        let old = Arc::new(AtomicBool::new(false));
        assert!(!state.finish_properties(&scope(), &old, &closed()));
        assert!(Arc::ptr_eq(
            state.properties_canceled.as_ref().unwrap(),
            &signal
        ));
        drop(state);
        assert!(signal.load(Ordering::Acquire));
    }
}

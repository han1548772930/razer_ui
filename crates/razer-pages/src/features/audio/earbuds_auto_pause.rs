//! The retained Enhancement switch owns draft, persistence and device results.
use super::*;
use razer_device::{
    earbuds_auto_pause::{Operation, Report},
    earbuds_controls::{EarbudsControls, EarbudsTask},
    earbuds_protocol40::{AutoPauseObservation, WriteOutcome},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct EarbudsAutoPauseRequested {
    id: Uuid,
    profile_id: String,
    initialize_profile: bool,
    operation: Operation,
    active: Arc<AtomicBool>,
    monitor: Arc<AtomicBool>,
}

#[cfg(test)]
#[path = "earbuds_auto_pause_tests.rs"]
mod tests;
impl EarbudsAutoPauseRequested {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
    pub fn initializes_profile(&self) -> bool {
        self.initialize_profile
    }
    pub fn operation(&self) -> Operation {
        self.operation
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.active.clone()
    }
    pub fn monitor_guard(&self) -> Arc<AtomicBool> {
        self.monitor.clone()
    }
}

#[derive(Default)]
pub(super) struct State {
    controls: Option<EarbudsControls>,
    identity: String,
    profile_id: String,
    selected_profile_id: String,
    initialize_profile: bool,
    active: bool,
    connected: bool,
    pending: Option<EarbudsAutoPauseRequested>,
    queued: Option<Operation>,
    error: Option<String>,
    initialized: bool,
    monitor: Option<Arc<AtomicBool>>,
    was_off: bool,
    reconnect_restores: u8,
    reread_after_restore: bool,
}
impl State {
    fn cancel(&mut self) {
        if let Some(guard) = self.monitor.take() {
            guard.store(false, Ordering::Release);
        }
        if let Some(request) = &self.pending {
            request.active.store(false, Ordering::Release);
        }
        self.queued = None;
        self.initialized = false;
        self.was_off = false;
        self.reconnect_restores = 0;
        self.reread_after_restore = false;
        if let Some(controls) = &mut self.controls {
            controls.disconnect();
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl EventEmitter<EarbudsAutoPauseRequested> for AudioProductWorkspace {}

impl AudioProductWorkspace {
    pub(super) fn earbuds_snapshot(&self, snapshot: &mut Value) {
        let Some(controls) = &self.earbuds.controls else {
            return;
        };
        if let Some(device) = snapshot.get_mut("device").and_then(Value::as_object_mut) {
            device.remove("autoPause");
        }
        if let Some(profile) = snapshot.get_mut("profile").and_then(Value::as_object_mut) {
            if let Some(enabled) = controls.auto_pause_persisted {
                let field = profile
                    .entry("audioAutoPauseControl")
                    .or_insert_with(|| json!({}));
                if let Some(field) = field.as_object_mut() {
                    field.insert("isEnabled".into(), Value::Bool(enabled));
                }
            }
        }
    }

    pub fn set_earbuds_context(
        &mut self,
        device: &razer_model::model::Device,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        if !razer_device::earbuds_protocol40::supports_auto_pause(self.spec.product_id) {
            return;
        }
        let connected = matches!(
            device.dashboard.connection_observation,
            Some(
                razer_model::model::DeviceConnectionObservation::UsbPresent
                    | razer_model::model::DeviceConnectionObservation::HidPresent
            )
        );
        let initializes_profile = device.profiles.is_empty() && device.active_profile.is_empty();
        let local = if initializes_profile {
            Ok(razer_device::earbuds_protocol40::AutoPauseProfileField { is_enabled: None })
        } else {
            razer_storage::earbuds_profiles::read_auto_pause(device, &device.active_profile)
        };
        let ready = active && connected && local.is_ok();
        if let Err(error) = &local {
            self.earbuds.error = Some(format!("{error:#}"));
        }
        let changed = self.earbuds.identity != device.workspace_identity()
            || self.earbuds.selected_profile_id != device.active_profile
            || self.earbuds.connected != connected;
        let became_active = ready && !self.earbuds.active;
        if changed {
            self.earbuds.cancel();
            self.earbuds.identity = device.workspace_identity();
            self.earbuds.selected_profile_id = device.active_profile.clone();
            self.earbuds.initialize_profile = initializes_profile;
            self.earbuds.profile_id = if initializes_profile {
                Uuid::new_v4().to_string()
            } else {
                device.active_profile.clone()
            };
            self.earbuds.active = ready;
            self.earbuds.connected = connected;
            let mut controls =
                EarbudsControls::new(self.spec.product_id).expect("supported earbuds");
            match local {
                Ok(field) => controls.auto_pause_persisted = field.is_enabled,
                Err(error) => {
                    self.earbuds.error = Some(format!("{error:#}"));
                    self.earbuds.active = false;
                }
            }
            if connected {
                controls.connect();
            }
            self.earbuds.controls = Some(controls);
            if self.earbuds.active {
                self.earbuds.monitor = Some(Arc::new(AtomicBool::new(true)));
                self.earbuds.queued = Some(Operation::Read);
            }
        }
        if !changed && self.earbuds.active != ready {
            if let Some(guard) = self.earbuds.monitor.take() {
                guard.store(false, Ordering::Release);
            }
            if let Some(request) = &self.earbuds.pending {
                request.active.store(false, Ordering::Release);
            }
            self.earbuds.queued = None;
            self.earbuds.active = ready;
        }
        if became_active && !changed {
            self.earbuds.monitor = Some(Arc::new(AtomicBool::new(true)));
            self.earbuds.queued = Some(Operation::Read);
        }
        self.start_earbuds_request(cx);
    }

    fn start_earbuds_request(&mut self, cx: &mut Context<Self>) {
        if self.earbuds.pending.is_some() || !self.earbuds.active {
            return;
        }
        let Some(operation) = self.earbuds.queued.take() else {
            return;
        };
        let request = EarbudsAutoPauseRequested {
            id: Uuid::new_v4(),
            profile_id: self.earbuds.profile_id.clone(),
            initialize_profile: self.earbuds.initialize_profile,
            operation,
            active: Arc::new(AtomicBool::new(true)),
            monitor: self
                .earbuds
                .monitor
                .as_ref()
                .expect("active event owner")
                .clone(),
        };
        self.earbuds.pending = Some(request.clone());
        cx.emit(request);
    }

    pub fn earbuds_request_current(&self, request: &EarbudsAutoPauseRequested) -> bool {
        self.earbuds.active
            && request.active.load(Ordering::Acquire)
            && self.earbuds.profile_id == request.profile_id
            && self
                .earbuds
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id == request.id)
    }

    pub fn mark_earbuds_persisted(&mut self, request: &EarbudsAutoPauseRequested) -> bool {
        if !self.earbuds_request_current(request) {
            return false;
        }
        self.earbuds.selected_profile_id = request.profile_id.clone();
        self.earbuds.initialize_profile = false;
        let Some(controls) = &mut self.earbuds.controls else {
            return false;
        };
        let enabled = match request.operation {
            Operation::Write { enabled } => enabled,
            Operation::Restore { enabled } => enabled.unwrap_or(true),
            Operation::Read | Operation::Battery { .. } => return true,
        };
        controls
            .acknowledge_persistence(EarbudsTask::SetAutoPause {
                generation: controls.generation(),
                enabled,
            })
            .is_ok()
    }

    pub fn finish_earbuds_request(
        &mut self,
        request: &EarbudsAutoPauseRequested,
        result: Result<Report, String>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .earbuds
            .pending
            .as_ref()
            .is_some_and(|pending| pending.id == request.id)
        {
            return;
        }
        let current = self.earbuds_request_current(request);
        self.earbuds.pending = None;
        if current {
            match result {
                Ok(report) => {
                    self.earbuds.error = match &report.write {
                        WriteOutcome::Failed(error) => Some(error.clone()),
                        _ => None,
                    };
                    let controls = self
                        .earbuds
                        .controls
                        .as_mut()
                        .expect("earbuds control owner");
                    if let Some(connected) = report.connected {
                        if connected && self.earbuds.was_off {
                            self.earbuds.initialized = false;
                            self.earbuds.reconnect_restores =
                                if self.spec.product_id == 2689 { 1 } else { 2 };
                        }
                        self.earbuds.was_off = !connected;
                        if !connected {
                            self.earbuds.queued = None;
                        }
                    }
                    if report.connected.is_some() {
                        controls.earbud_disconnected = report.has_disconnected_earbud();
                        if controls.earbud_disconnected
                            && matches!(self.earbuds.queued, Some(Operation::Write { .. }))
                        {
                            self.earbuds.queued = None;
                        }
                    }
                    if !matches!(request.operation, Operation::Battery { .. })
                        || report.connected == Some(false)
                    {
                        controls.auto_pause_device = report.observation.clone();
                    }
                    controls.last_auto_pause_write = Some(report.write.clone());
                    if let AutoPauseObservation::Observed(enabled) = report.observation {
                        if self.earbuds.queued.is_none() {
                            controls.draft.auto_pause = enabled;
                        }
                        if matches!(request.operation, Operation::Read) && !self.earbuds.initialized
                        {
                            self.earbuds.initialized = true;
                            let local = controls.auto_pause_persisted.unwrap_or(true);
                            if local != enabled
                                || controls.auto_pause_persisted.is_none()
                                || self.earbuds.reconnect_restores > 0
                            {
                                self.earbuds.queued = Some(Operation::Restore {
                                    enabled: Some(local),
                                });
                            }
                        }
                    }
                    if matches!(request.operation, Operation::Restore { .. })
                        && self.earbuds.reconnect_restores > 0
                    {
                        self.earbuds.reconnect_restores -= 1;
                        if self.earbuds.reconnect_restores > 0 && self.earbuds.queued.is_none() {
                            self.earbuds.queued = Some(Operation::Restore {
                                enabled: controls.auto_pause_persisted,
                            });
                        } else if self.earbuds.reconnect_restores == 0
                            && self.earbuds.reread_after_restore
                        {
                            self.earbuds.reread_after_restore = false;
                            if self.earbuds.queued.is_none() {
                                self.earbuds.queued = Some(Operation::Battery { levels: None });
                            }
                        }
                    }
                    if matches!(request.operation, Operation::Battery { .. })
                        && self.earbuds.reconnect_restores > 0
                        && self.earbuds.queued.is_none()
                    {
                        if self.earbuds.initialize_profile {
                            self.earbuds.initialized = false;
                            self.earbuds.queued = Some(Operation::Read);
                        } else {
                            self.earbuds.initialized = true;
                            self.earbuds.queued = Some(Operation::Restore {
                                enabled: controls.auto_pause_persisted,
                            });
                        }
                    }
                }
                Err(error) => {
                    if let Some(controls) = &mut self.earbuds.controls {
                        match request.operation {
                            Operation::Read | Operation::Battery { .. } => {
                                controls.auto_pause_device =
                                    AutoPauseObservation::ReadFailed(error.clone())
                            }
                            _ => {
                                controls.last_auto_pause_write =
                                    Some(WriteOutcome::Failed(error.clone()))
                            }
                        }
                    }
                    self.earbuds.error = Some(error);
                }
            }
        }
        self.start_earbuds_request(cx);
        cx.notify();
    }

    pub fn observe_earbuds_notification(
        &mut self,
        owner: &Arc<AtomicBool>,
        event: razer_device::earbuds_events::Notification,
        cx: &mut Context<Self>,
    ) {
        if !self.earbuds.active
            || !owner.load(Ordering::Acquire)
            || !self
                .earbuds
                .monitor
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, owner))
        {
            return;
        }
        use razer_device::earbuds_events::Notification;
        match event {
            Notification::Connection { status: 0, .. } => {
                self.earbuds.was_off = true;
                if let Some(request) = &self.earbuds.pending {
                    request.active.store(false, Ordering::Release);
                }
                self.earbuds.queued = None;
                if let Some(controls) = &mut self.earbuds.controls {
                    controls.auto_pause_device = AutoPauseObservation::Unobserved;
                }
            }
            Notification::Connection { .. } => {
                if self.earbuds.was_off {
                    self.earbuds.was_off = false;
                    self.earbuds.reconnect_restores =
                        if self.spec.product_id == 2689 { 1 } else { 2 };
                    self.earbuds.reread_after_restore = true;
                    self.earbuds.initialized = !self.earbuds.initialize_profile;
                    if self.earbuds.queued.is_none() {
                        self.earbuds.queued = Some(if self.earbuds.initialize_profile {
                            Operation::Read
                        } else {
                            Operation::Restore {
                                enabled: self
                                    .earbuds
                                    .controls
                                    .as_ref()
                                    .and_then(|control| control.auto_pause_persisted),
                            }
                        });
                    }
                } else if self.earbuds.queued.is_none() {
                    self.earbuds.queued = Some(Operation::Battery { levels: None });
                }
                self.start_earbuds_request(cx);
            }
            Notification::Battery { left, right } => {
                // Device confirmation and the current connection gate are read by
                // the service; incoming bytes never fabricate an auto-pause value.
                if self.earbuds.queued.is_none() {
                    self.earbuds.queued = Some(Operation::Battery {
                        levels: Some([left, right]),
                    });
                }
                self.start_earbuds_request(cx);
            }
        }
        cx.notify();
    }

    pub fn finish_earbuds_monitor(
        &mut self,
        owner: &Arc<AtomicBool>,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .earbuds
            .monitor
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, owner))
        {
            return;
        }
        if let Some(error) = error {
            self.earbuds.error = Some(error);
            self.earbuds.cancel();
            self.earbuds.active = false;
        }
        cx.notify();
    }

    fn toggle_earbuds_auto_pause(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds.active {
            return;
        }
        let Some(controls) = &mut self.earbuds.controls else {
            return;
        };
        if !matches!(
            controls.auto_pause_device,
            AutoPauseObservation::Observed(_)
        ) {
            return;
        }
        if let Some(EarbudsTask::SetAutoPause { enabled, .. }) = controls.toggle_auto_pause() {
            self.earbuds.queued = Some(Operation::Write { enabled });
            self.start_earbuds_request(cx);
            cx.notify();
        }
    }

    pub(super) fn earbuds_auto_pause_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let control = self.earbuds.controls.as_ref();
        let enabled = self.earbuds.active
            && control.is_some_and(|control| {
                !control.earbud_disconnected
                    && matches!(control.auto_pause_device, AutoPauseObservation::Observed(_))
            });
        surface::panel_with_title_switch(
            t("AUTO_PAUSE"),
            surface::SynapseSwitch::new("earbuds-auto-pause")
                .checked(control.is_some_and(|control| control.draft.auto_pause))
                .disabled(!enabled)
                .accessibility_label(t("AUTO_PAUSE"))
                .on_change(cx.listener(|this, _, _, cx| this.toggle_earbuds_auto_pause(cx))),
            div(),
            cx,
        )
        .opacity(
            if control.is_some_and(|control| control.earbud_disconnected) {
                0.5
            } else {
                1.
            },
        )
        .child(
            div()
                .text_size(surface::css(14.))
                .child(t("AUTO_PAUSE_DESC")),
        )
        .into_any_element()
    }
}

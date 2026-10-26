//! Device-global Power state, independent from profile controls and storage.
//! Current NORMAL_SKIPPABLE work retains the active task and newest pending task.
use super::*;
use razer_device::controller_sleep::{
    PowerSaving, SleepObservation, SleepOperation, SleepResult, capability,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct ControllerSleepRequested {
    sequence: u64,
    revision: u64,
    operation: SleepOperation,
    device_access: bool,
    requires_persistence: bool,
    guard: Arc<AtomicBool>,
    storage_guard: Arc<AtomicBool>,
    cleanup_pending: Arc<AtomicBool>,
    initialization_restore: bool,
}
impl ControllerSleepRequested {
    pub fn operation(&self) -> &SleepOperation {
        &self.operation
    }
    pub fn setting(&self) -> Option<PowerSaving> {
        self.operation.setting()
    }
    pub fn device_access(&self) -> bool {
        self.device_access
    }
    pub fn requires_persistence(&self) -> bool {
        self.requires_persistence
    }
    pub fn without_device_access(mut self) -> Self {
        self.device_access = false;
        self
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.guard)
    }
    pub fn is_valid(&self) -> bool {
        self.guard.load(Ordering::Acquire)
    }
    pub fn set_cleanup_pending(&self, pending: bool) {
        self.cleanup_pending.store(pending, Ordering::Release);
    }
    pub fn cleanup_pending(&self) -> bool {
        self.cleanup_pending.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerSleepObservation {
    Unobserved,
    Reading,
    Observed(SleepObservation),
    Failed(String),
    Disconnected,
}

pub(super) struct SleepRuntime {
    guard: Arc<AtomicBool>,
    storage_guard: Arc<AtomicBool>,
    storage_requests: std::collections::BTreeSet<u64>,
    sequence: u64,
    revision: u64,
    active: Option<u64>,
    pending: Option<ControllerSleepRequested>,
    read_requested: bool,
    connected: bool,
    admission: bool,
    recording: bool,
    persisted: Option<PowerSaving>,
    draft: Option<PowerSaving>,
    observation: ControllerSleepObservation,
    last_result: Option<SleepResult>,
    last_error: Option<String>,
}
impl Default for SleepRuntime {
    fn default() -> Self {
        Self {
            guard: Arc::new(AtomicBool::new(true)),
            sequence: 0,
            revision: 0,
            storage_guard: Arc::new(AtomicBool::new(true)),
            storage_requests: Default::default(),
            active: None,
            pending: None,
            read_requested: false,
            connected: true,
            admission: true,
            recording: false,
            persisted: None,
            draft: None,
            observation: ControllerSleepObservation::Unobserved,
            last_result: None,
            last_error: None,
        }
    }
}
impl Drop for SleepRuntime {
    fn drop(&mut self) {
        self.guard.store(false, Ordering::Release);
        self.storage_guard.store(false, Ordering::Release);
    }
}
impl SleepRuntime {
    fn restore_import(
        &mut self,
        power: &razer_device::controller_initialization::Read<SleepObservation>,
        recording: bool,
    ) -> bool {
        self.recording = recording;
        match power {
            razer_device::controller_initialization::Read::Observed { value } => {
                self.observation = ControllerSleepObservation::Observed(*value);
                self.persisted = Some(value.power_saving());
                self.draft = None;
                if !recording && self.admission && self.connected {
                    let mut request = self.request(
                        SleepOperation::Set {
                            power_saving: value.power_saving(),
                        },
                        false,
                    );
                    request.initialization_restore = true;
                    self.pending = Some(request);
                    return true;
                }
            }
            razer_device::controller_initialization::Read::Failed { error } => {
                self.observation = ControllerSleepObservation::Failed(error.clone());
            }
            razer_device::controller_initialization::Read::Unobserved => {}
        }
        false
    }
    fn invalidate(&mut self, connected: bool) {
        self.guard.store(false, Ordering::Release);
        self.guard = Arc::new(AtomicBool::new(true));
        self.active = None;
        self.pending = None;
        self.read_requested = false;
        self.connected = connected;
        self.revision = self.revision.wrapping_add(1);
        self.draft = None;
        self.observation = if connected {
            ControllerSleepObservation::Unobserved
        } else {
            ControllerSleepObservation::Disconnected
        };
        self.last_result = None;
        self.last_error = None;
    }
    fn request(
        &mut self,
        operation: SleepOperation,
        requires_persistence: bool,
    ) -> ControllerSleepRequested {
        self.sequence = self.sequence.wrapping_add(1);
        if !operation.is_read() {
            self.revision = self.revision.wrapping_add(1);
        }
        ControllerSleepRequested {
            sequence: self.sequence,
            revision: self.revision,
            operation,
            device_access: !self.recording,
            guard: Arc::clone(&self.guard),
            storage_guard: Arc::clone(&self.storage_guard),
            cleanup_pending: Arc::new(AtomicBool::new(false)),
            requires_persistence,
            initialization_restore: false,
        }
    }
    fn enqueue(&mut self, operation: SleepOperation) {
        self.pending = Some(self.request(operation, false));
    }
    fn prepare_storage(&mut self, setting: PowerSaving) -> ControllerSleepRequested {
        let request = self.request(
            SleepOperation::Set {
                power_saving: setting,
            },
            true,
        );
        self.storage_requests.insert(request.sequence);
        request
    }
    fn next(&mut self) -> Option<ControllerSleepRequested> {
        if self.active.is_some() {
            return None;
        }
        let request = self.pending.take()?;
        self.active = Some(request.sequence);
        self.last_error = None;
        if request.operation.is_read() {
            self.observation = ControllerSleepObservation::Reading;
        }
        Some(request)
    }
    fn matches(&self, request: &ControllerSleepRequested) -> bool {
        if request.requires_persistence {
            request.storage_guard.load(Ordering::Acquire)
                && Arc::ptr_eq(&self.storage_guard, &request.storage_guard)
                && self.storage_requests.contains(&request.sequence)
        } else {
            request.is_valid()
                && Arc::ptr_eq(&self.guard, &request.guard)
                && self.active == Some(request.sequence)
        }
    }
    fn presentation(&self) -> Option<PowerSaving> {
        self.draft.or_else(|| match self.observation {
            ControllerSleepObservation::Observed(observed) => Some(observed.power_saving()),
            _ => None,
        })
    }
    fn enabled(&self) -> bool {
        self.connected
            && self.admission
            && !matches!(
                self.observation,
                ControllerSleepObservation::Unobserved
                    | ControllerSleepObservation::Reading
                    | ControllerSleepObservation::Disconnected
            )
            && self
                .presentation()
                .is_some_and(|setting| setting.is_enabled)
    }
    fn finish(
        &mut self,
        request: &ControllerSleepRequested,
        result: Option<SleepResult>,
        error: Option<String>,
    ) -> bool {
        if !self.matches(request) {
            return false;
        }
        if request.requires_persistence {
            self.storage_requests.remove(&request.sequence);
            self.last_error = error;
            if self.last_error.is_none() {
                self.persisted = request.setting();
            }
            if self.last_error.is_none()
                && request.device_access
                && !self.recording
                && self.admission
                && request.is_valid()
                && Arc::ptr_eq(&self.guard, &request.guard)
            {
                let mut device_request = request.clone();
                device_request.requires_persistence = false;
                // Only the device queue skips older pending Power operations.
                self.pending = Some(device_request);
            }
            return true;
        }
        let result = result.and_then(|value| match value.validate_for(&request.operation) {
            Ok(()) => Some(value),
            Err(error) => {
                self.last_error = Some(format!("{error:#}"));
                None
            }
        });
        if let Some(observed) = result.as_ref().and_then(SleepResult::observation) {
            self.observation = ControllerSleepObservation::Observed(observed);
            if request.operation.is_read() && request.revision == self.revision {
                self.draft = None;
            }
        } else if request.device_access {
            let failure = error
                .clone()
                .or_else(|| {
                    result.as_ref().and_then(|value| {
                        if let razer_device::controller_sleep::SleepReadStatus::Failed { error } =
                            &value.read
                        {
                            Some(error.clone())
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| "Controller sleep result was not observed".into());
            self.observation = ControllerSleepObservation::Failed(failure);
        }
        if error.is_some() {
            self.last_error = error;
        }
        if !request.device_access && request.operation.is_read() {
            self.read_requested = false;
            self.observation = ControllerSleepObservation::Unobserved;
        }
        self.last_result = result;
        if !request.cleanup_pending() {
            self.active = None;
        }
        true
    }
    pub(super) fn enter_power(&mut self) {
        // Re-entry is an explicit observation request. A failed initial GET
        // never schedules a retry by itself.
        if self.active.is_none() {
            self.read_requested = false;
        }
    }
    fn status(&self, choices: &[i64]) -> Option<String> {
        match &self.observation {
            ControllerSleepObservation::Unobserved => {
                Some(format!("{}: —", t("POWER_SAVING_HEADER")))
            }
            ControllerSleepObservation::Reading => Some(t("LOADING")),
            ControllerSleepObservation::Failed(_) => Some(t("ERROR")),
            ControllerSleepObservation::Disconnected => Some(t("DISCONNECTED")),
            ControllerSleepObservation::Observed(_) if self.last_error.is_some() => {
                Some(t("ERROR"))
            }
            ControllerSleepObservation::Observed(observed)
                if observed.seconds % 60 != 0
                    || !choices.contains(&i64::from(observed.power_saving().value)) =>
            {
                Some(format!(
                    "{} · {}",
                    t("ERROR"),
                    razer_i18n::t_value("SEC", i64::from(observed.seconds))
                ))
            }
            ControllerSleepObservation::Observed(_) => None,
        }
    }
}

impl GamepadProductWorkspace {
    pub fn restore_controller_initialization_power(
        &mut self,
        power: &razer_device::controller_initialization::Read<SleepObservation>,
        recording: bool,
        cx: &mut Context<Self>,
    ) {
        if self.sleep_runtime.restore_import(power, recording) {
            self.emit_next_sleep(cx);
            cx.notify();
            return;
        }
        // LOCALSTORAGE is ignored while the macro recorder owns the device;
        // an absent/failed root read never authorizes a cache/default setter.
        self.release_controller_initialization(cx);
    }
    pub fn prepare_controller_sleep_exit(&mut self, cx: &mut Context<Self>) {
        self.sleep_runtime.admission = false;
        self.sleep_runtime.invalidate(self.sleep_runtime.connected);
        cx.notify();
    }
    pub fn resume_controller_sleep_exit(&mut self, connected: bool, cx: &mut Context<Self>) {
        self.sleep_runtime.admission = true;
        self.sleep_runtime.invalidate(connected);
        self.request_controller_sleep(cx);
        cx.notify();
    }
    pub fn restore_controller_sleep(
        &mut self,
        setting: Option<PowerSaving>,
        cx: &mut Context<Self>,
    ) {
        self.sleep_runtime.persisted = setting;
        // Restoring a file never marks a value observed or schedules a setter.
        cx.notify();
    }
    pub fn controller_power_saving(&self) -> Option<PowerSaving> {
        self.sleep_runtime.draft.or(self.sleep_runtime.persisted)
    }
    pub fn controller_sleep_observation(&self) -> &ControllerSleepObservation {
        &self.sleep_runtime.observation
    }
    pub fn controller_sleep_result(&self) -> Option<&SleepResult> {
        self.sleep_runtime.last_result.as_ref()
    }
    pub fn observe_controller_sleep_connection(&mut self, connected: bool, cx: &mut Context<Self>) {
        self.sleep_runtime.invalidate(connected);
        if connected {
            self.request_controller_sleep(cx);
        }
        cx.notify();
    }
    pub fn observe_controller_sleep_recording(&mut self, recording: bool, cx: &mut Context<Self>) {
        if self.sleep_runtime.recording == recording {
            return;
        }
        self.sleep_runtime.recording = recording;
        if !recording {
            self.request_controller_sleep(cx);
        }
        cx.notify();
    }
    pub(super) fn request_controller_sleep(&mut self, cx: &mut Context<Self>) {
        if !self.controller_initialization_ready
            || self.page != "TAB_POWER"
            || capability(self.spec.product_id).is_none()
            || self.sleep_runtime.read_requested
            || !self.sleep_runtime.connected
            || !self.sleep_runtime.admission
            || self.sleep_runtime.recording
        {
            return;
        }
        self.sleep_runtime.read_requested = true;
        self.sleep_runtime.enqueue(SleepOperation::Read);
        self.emit_next_sleep(cx);
    }
    fn emit_next_sleep(&mut self, cx: &mut Context<Self>) {
        if let Some(request) = self.sleep_runtime.next() {
            cx.emit(request);
        }
    }
    pub fn controller_sleep_request_matches(&self, request: &ControllerSleepRequested) -> bool {
        self.sleep_runtime.matches(request)
    }
    pub fn finish_controller_sleep(
        &mut self,
        request: &ControllerSleepRequested,
        result: Option<SleepResult>,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let initialization_restore = request.initialization_restore && !request.cleanup_pending();
        if self.sleep_runtime.finish(request, result, error) {
            if initialization_restore {
                self.release_controller_initialization(cx);
            }
            self.emit_next_sleep(cx);
            cx.notify();
        }
    }
    pub(super) fn sleep_choice(&self, minutes: i64, cx: &Context<Self>) -> Button {
        let enabled = self.sleep_runtime.enabled();
        Button::new(SharedString::from(format!("controller-sleep-{minutes}")))
            .label(power_saving_label(minutes))
            .outline()
            .disabled(!enabled)
            .selected(
                self.sleep_runtime
                    .presentation()
                    .is_some_and(|setting| i64::from(setting.value) == minutes),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.sleep_runtime.enabled() {
                    return;
                }
                let Some(current) = this.sleep_runtime.presentation() else {
                    return;
                };
                let Ok(value) = u16::try_from(minutes) else {
                    return;
                };
                let Ok(setting) = PowerSaving::new(value, current.is_enabled) else {
                    return;
                };
                this.sleep_runtime.draft = Some(setting);
                let request = this.sleep_runtime.prepare_storage(setting);
                cx.emit(GamepadProductChanged);
                cx.emit(request);
                cx.notify();
            }))
    }
    pub(super) fn sleep_status(&self) -> Option<AnyElement> {
        self.sleep_runtime
            .status(&self.spec.power_minutes)
            .map(|status| {
                div()
                    .id("controller-sleep-observation")
                    .role(Role::Status)
                    .aria_label(status.clone())
                    .mt(surface::css(10.))
                    .text_size(surface::css(12.))
                    .text_color(razer_widgets::theme::GamepadTriggerColors::text())
                    .child(status)
                    .into_any_element()
            })
    }
}

#[cfg(test)]
#[path = "gamepad_sleep_tests.rs"]
mod tests;

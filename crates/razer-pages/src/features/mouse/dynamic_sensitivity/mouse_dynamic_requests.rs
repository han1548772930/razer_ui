//! Device observations and explicit edits have separate lifetimes.
use super::{
    MouseDynamicCompletion, MouseDynamicConnection, MouseDynamicRequested, MouseDynamicScope,
    Reading, Setting,
};
use gpui_kit::EntityId;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) struct Lifecycle {
    scope: MouseDynamicScope,
    pub(super) requests: Requests,
    pub(super) guard: Arc<AtomicBool>,
    pub(super) monitor_guard: Arc<AtomicBool>,
    pub(super) monitor_requested: bool,
    mouse_speed: f64,
    speed_revision: u64,
}
#[derive(Clone, Copy)]
pub(super) struct SpeedExpiry {
    connection: MouseDynamicConnection,
    revision: u64,
}
pub(super) struct CompletionEffect {
    pub(super) refresh: bool,
    pub(super) diagnostic: Option<String>,
}
impl Lifecycle {
    pub(super) fn new(owner: EntityId) -> Self {
        Self {
            scope: MouseDynamicScope {
                owner,
                profile_epoch: 0,
                connection_epoch: 0,
            },
            requests: Requests::default(),
            guard: Arc::new(AtomicBool::new(true)),
            monitor_guard: Arc::new(AtomicBool::new(false)),
            monitor_requested: false,
            mouse_speed: 0.,
            speed_revision: 0,
        }
    }
    pub(super) fn scope(&self) -> MouseDynamicScope {
        self.scope
    }
    pub(super) fn connection_is_current(&self, connection: MouseDynamicConnection) -> bool {
        self.scope.connection() == connection
    }
    pub(super) fn scope_is_current(&self, request: &MouseDynamicRequested) -> bool {
        (if request.command == "DYNAMIC_SENSITIVITY_UI_COMMAND" {
            self.connection_is_current(request.scope.connection())
        } else {
            self.scope == request.scope
        }) && request.guard.load(Ordering::Acquire)
    }
    pub(super) fn request_is_current(&self, request: &MouseDynamicRequested) -> bool {
        self.scope_is_current(request) && self.requests.current(request)
    }
    pub(super) fn complete(
        &mut self,
        request: &MouseDynamicRequested,
        completion: MouseDynamicCompletion,
    ) -> Option<CompletionEffect> {
        if !self.request_is_current(request) {
            return None;
        }
        let current_edit = request.edit_revision == self.requests.edit_revision;
        let refresh = request.command == "ON_SET_DYNAMIC_SENSITIVITY"
            && current_edit
            && matches!(&completion, MouseDynamicCompletion::Device(_));
        self.requests.complete(request, completion);
        let diagnostic = current_edit
            .then(|| self.failure().map(str::to_owned))
            .flatten();
        Some(CompletionEffect {
            refresh,
            diagnostic,
        })
    }
    pub(super) fn failure(&self) -> Option<&str> {
        match &self.requests.observation {
            Observation::Failed(error) => Some(error),
            _ => self.requests.last_failure.as_deref(),
        }
    }
    pub(super) fn restore_profile(&mut self) {
        self.guard.store(false, Ordering::Release);
        self.guard = Arc::new(AtomicBool::new(true));
        self.scope.profile_epoch = self
            .scope
            .profile_epoch
            .checked_add(1)
            .expect("Dynamic profile identity exhausted");
        let connected = !matches!(self.requests.observation, Observation::Disconnected);
        self.requests.reset_profile_observation(connected);
    }
    pub(super) fn invalidate_connection(&mut self) {
        self.guard.store(false, Ordering::Release);
        self.monitor_guard.store(false, Ordering::Release);
        self.guard = Arc::new(AtomicBool::new(true));
        self.monitor_requested = false;
        self.scope.connection_epoch = self
            .scope
            .connection_epoch
            .checked_add(1)
            .expect("Dynamic connection identity exhausted");
        self.requests.reset_observation(true);
        self.clear_speed();
    }
    pub(super) fn speed(&self) -> f64 {
        self.mouse_speed
    }
    pub(super) fn observe_speed(
        &mut self,
        connection: MouseDynamicConnection,
        speed: f64,
    ) -> Option<SpeedExpiry> {
        if !self.connection_is_current(connection) {
            return None;
        }
        let speed = super::observed_mouse_speed(speed)?;
        self.clear_speed();
        self.mouse_speed = speed;
        Some(SpeedExpiry {
            connection,
            revision: self.speed_revision,
        })
    }
    pub(super) fn expire_speed(&mut self, expiry: SpeedExpiry) -> bool {
        if !self.connection_is_current(expiry.connection) || expiry.revision != self.speed_revision
        {
            return false;
        }
        self.clear_speed();
        true
    }
    pub(super) fn clear_speed(&mut self) {
        self.mouse_speed = 0.;
        self.speed_revision = self
            .speed_revision
            .checked_add(1)
            .expect("Dynamic speed expiry identity exhausted");
    }
}

#[derive(Default)]
pub(super) enum Observation {
    #[default]
    Unobserved,
    Loading,
    Disconnected,
    Failed(String),
    Current(Reading),
}

#[derive(Default)]
pub(super) struct Requests {
    next_request: u64,
    pub(super) pending: BTreeMap<u64, &'static str>,
    pub(super) latest_mutation: Option<u64>,
    pub(super) edit_revision: u64,
    pub(super) observation: Observation,
    pub(super) edit: Option<Setting>,
    pub(super) loading: bool,
    pub(super) last_failure: Option<String>,
    write_failure: Option<String>,
}
impl Requests {
    pub(super) fn reading(&self) -> Option<&Reading> {
        match &self.observation {
            Observation::Current(reading) => Some(reading),
            _ => None,
        }
    }
    pub(super) fn enabled(&self) -> Option<bool> {
        self.edit
            .as_ref()
            .map(|edit| edit.enabled)
            .or_else(|| self.reading().and_then(Reading::enabled))
    }
    pub(super) fn mode(&self) -> Option<u32> {
        self.edit.as_ref().map(|edit| edit.mode as u32).or_else(|| {
            self.reading()
                .and_then(Reading::mode)
                .map(|mode| mode as u32)
        })
    }
    pub(super) fn reset_observation(&mut self, connected: bool) {
        self.reset(connected, false);
    }
    pub(super) fn reset_profile_observation(&mut self, connected: bool) {
        self.reset(connected, true);
    }
    fn reset(&mut self, connected: bool, keep_monitoring: bool) {
        self.observation = if connected {
            Observation::Unobserved
        } else {
            Observation::Disconnected
        };
        self.edit = None;
        self.pending
            .retain(|_, command| keep_monitoring && *command == "DYNAMIC_SENSITIVITY_UI_COMMAND");
        self.latest_mutation = None;
        self.loading = false;
        self.last_failure = None;
        self.write_failure = None;
    }
    pub(super) fn begin(&mut self, command: &'static str) -> u64 {
        self.next_request = self
            .next_request
            .checked_add(1)
            .expect("Dynamic request identity exhausted");
        let request_id = self.next_request;
        self.pending.insert(request_id, command);
        if command == "READ_DYNAMIC_SENSITIVITY" {
            self.observation = Observation::Loading;
        }
        if command == "ON_SET_DYNAMIC_SENSITIVITY" {
            self.write_failure = None;
            self.last_failure = None;
            self.edit_revision = self
                .edit_revision
                .checked_add(1)
                .expect("Dynamic edit identity exhausted");
            self.latest_mutation = Some(request_id);
        }
        request_id
    }
    pub(super) fn current(&self, request: &MouseDynamicRequested) -> bool {
        request.guard.load(Ordering::Acquire)
            && self.pending.get(&request.request_id) == Some(&request.command)
    }
    pub(super) fn retire_monitors(&mut self) {
        self.pending
            .retain(|_, command| *command != "DYNAMIC_SENSITIVITY_UI_COMMAND");
    }
    pub(super) fn complete(
        &mut self,
        request: &MouseDynamicRequested,
        completion: MouseDynamicCompletion,
    ) -> bool {
        if !self.current(request) {
            return false;
        }
        if !matches!(completion, MouseDynamicCompletion::Monitoring(true)) {
            self.pending.remove(&request.request_id);
        }
        let latest = self.latest_mutation == Some(request.request_id);
        let same_edit = request.edit_revision == self.edit_revision;
        match completion {
            MouseDynamicCompletion::Device(report) => {
                if same_edit {
                    if report.requested.is_some() {
                        self.write_failure = report.write_failure.clone();
                    }
                    let differs = report.reading.as_ref().is_some_and(|reading| {
                        self.edit
                            .as_ref()
                            .is_some_and(|edit| !reading.confirms(edit))
                    });
                    if let Some(reading) = &report.reading {
                        if self
                            .edit
                            .as_ref()
                            .is_some_and(|edit| reading.confirms(edit))
                        {
                            self.edit = None;
                        }
                        self.observation = Observation::Current(reading.clone());
                    } else if let Some(error) = &report.read_failure {
                        self.observation = Observation::Failed(error.clone());
                    }
                    let unknown = report.reading.as_ref().and_then(|reading| {
                        (reading.enabled().is_none() || reading.mode().is_none()).then(|| {
                            format!(
                                "Unknown dynamic sensitivity values: state {}, mode {}",
                                reading.raw_state, reading.raw_mode
                            )
                        })
                    });
                    self.last_failure = if self.write_failure.is_some()
                        || report.read_failure.is_some()
                        || differs
                        || report.reading.is_some() && !report.confirmed()
                    {
                        Some(self.write_failure.clone().or(report.read_failure).unwrap_or_else(|| {
                            "Independent dynamic readback differs from the requested setting".into()
                        }))
                    } else {
                        unknown
                    };
                }
                if latest {
                    self.loading = false;
                }
            }
            MouseDynamicCompletion::Failed(reason)
            | MouseDynamicCompletion::Unsupported(reason) => {
                if latest || request.command == "READ_DYNAMIC_SENSITIVITY" && same_edit {
                    self.loading = false;
                }
                if same_edit {
                    if request.command == "ON_SET_DYNAMIC_SENSITIVITY" {
                        self.write_failure = Some(reason.clone());
                    }
                    if request.command == "READ_DYNAMIC_SENSITIVITY" {
                        self.observation = Observation::Failed(reason.clone());
                    }
                    self.last_failure = Some(reason);
                }
            }
            MouseDynamicCompletion::Cancelled => {
                if request.command == "READ_DYNAMIC_SENSITIVITY" && same_edit {
                    self.observation = Observation::Unobserved;
                }
                if latest {
                    self.loading = false;
                }
            }
            MouseDynamicCompletion::Monitoring(_) | MouseDynamicCompletion::ObserverAttached => {}
        }
        true
    }
}

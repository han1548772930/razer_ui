//! Device observations and explicit edits have separate lifetimes.
use super::{MouseDynamicCompletion, MouseDynamicRequested, Reading, Setting};
use std::{collections::BTreeMap, sync::atomic::Ordering};

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
        self.observation = if connected {
            Observation::Unobserved
        } else {
            Observation::Disconnected
        };
        self.edit = None;
        self.pending.clear();
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
            MouseDynamicCompletion::Monitoring(_) => {}
        }
        true
    }
}

//! Request retirement and diagnostic publication follow the editor's revision.
use super::{MouseDynamicCompletion, MouseDynamicRequested, Reading};
use std::{collections::BTreeMap, sync::atomic::Ordering};

#[derive(Default)]
pub(super) struct Requests {
    next_request: u64,
    pub(super) pending: BTreeMap<u64, &'static str>,
    pub(super) latest_mutation: Option<u64>,
    pub(super) edit_revision: u64,
    pub(super) observed: Option<Reading>,
    pub(super) loading: bool,
    pub(super) last_failure: Option<String>,
}
impl Requests {
    pub(super) fn begin(&mut self, command: &'static str) -> u64 {
        self.next_request = self
            .next_request
            .checked_add(1)
            .expect("Dynamic request identity exhausted");
        let request_id = self.next_request;
        self.pending.insert(request_id, command);
        if command == "ON_SET_DYNAMIC_SENSITIVITY" {
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
                // Mounted source consumes the local profile optimistically;
                // independent GET diagnostics do not replace author control points.
                if same_edit {
                    self.observed = report.reading.clone();
                    self.last_failure = if report.write_failure.is_some()
                        || report.read_failure.is_some()
                        || report.reading.is_some() && !report.confirmed()
                    {
                        Some(report.write_failure.or(report.read_failure).unwrap_or_else(|| {
                            "Independent dynamic readback differs from the requested setting".into()
                        }))
                    } else {
                        None
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
                    self.last_failure = Some(reason);
                }
            }
            MouseDynamicCompletion::Cancelled => {
                if latest {
                    self.loading = false;
                }
            }
            MouseDynamicCompletion::Monitoring(_) => {}
        }
        true
    }
}

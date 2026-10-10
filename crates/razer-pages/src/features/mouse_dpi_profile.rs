//! Current 182 full-stage intent ownership. Local profiles and device reads stay
//! separate: no getter can recover hidden rows, stage enable or independent XY.
use super::*;
use crate::features::mouse_polling::MousePollingScope;
use razer_device::mouse_dpi_stages::{self, DpiStage, DpiStagesDraft, DpiStagesReading};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
struct Intent {
    scope: MousePollingScope,
    revision: u64,
    draft: DpiStagesDraft,
    valid_scope: Arc<AtomicBool>,
}

#[derive(Default)]
pub(super) struct State {
    revision: u64,
    pending: Option<Intent>,
    queued: Option<Intent>,
    reading: Option<(MousePollingScope, u64)>,
    reading_valid_scope: Option<Arc<AtomicBool>>,
    observed: Option<(MousePollingScope, DpiStagesReading)>,
    editing: bool,
    last_scope: Option<MousePollingScope>,
    basic_reads_ready: bool,
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some(pending) = &self.pending {
            pending.valid_scope.store(false, Ordering::Release);
        }
        if let Some(reading) = &self.reading_valid_scope {
            reading.store(false, Ordering::Release);
        }
    }
}

impl DeviceWorkspace {
    pub fn begin_dpi_basic_reads(&mut self) {
        self.mouse_dpi.basic_reads_ready = false;
    }
    pub fn finish_dpi_basic_reads(&mut self, cx: &mut Context<Self>) {
        self.mouse_dpi.basic_reads_ready = true;
        self.dispatch_queued_mouse_dpi(cx);
        self.request_mouse_dpi_read(cx);
    }
    pub(in crate::features) fn mouse_dpi_write_pending(&self) -> bool {
        self.mouse_dpi.pending.is_some()
    }
    pub(in crate::features) fn mouse_dpi_read_pending(&self) -> bool {
        self.mouse_dpi.reading.is_some()
    }
    pub(in crate::features) fn dpi_local_only_commit(&mut self, cx: &mut Context<Self>) {
        self.mouse_dpi.editing = false;
        // Updating only the profile flag must not discard a latest stage-table
        // intent already waiting behind a worker; it creates no extra command.
        let draft = self.dpi_draft();
        if let (Some(queued), Some(draft)) = (&mut self.mouse_dpi.queued, draft) {
            queued.revision = self.mouse_dpi.revision;
            queued.draft = draft;
        }
        self.dispatch_queued_mouse_dpi(cx);
    }

    fn observe_dpi_table(&mut self, scope: MousePollingScope, reading: DpiStagesReading) {
        let Some(row) = reading
            .records
            .iter()
            .find(|row| row.index == reading.active_stage)
        else {
            return;
        };
        let mut values = self
            .device
            .dashboard
            .readonly_values
            .clone()
            .unwrap_or_default();
        values.dpi = Some((row.x, row.y));
        values.errors.remove("dpi");
        self.device.observe_read_values(Some(values.clone()));
        self.saved.observe_read_values(Some(values));
        self.mouse_dpi.observed = Some((scope, reading));
    }

    /// Connection/profile scope changes invalidate observations, never release
    /// an actual read/write worker. Request fresh data after it really finishes.
    pub(in crate::features) fn sync_dpi_scope(&mut self, cx: &mut Context<Self>) {
        let scope = self.mouse_polling_scope(cx);
        if self.mouse_dpi.last_scope != scope {
            if let Some(pending) = &self.mouse_dpi.pending {
                pending.valid_scope.store(false, Ordering::Release);
            }
            if let Some(reading) = &self.mouse_dpi.reading_valid_scope {
                reading.store(false, Ordering::Release);
            }
            self.mouse_dpi.last_scope = scope;
            self.mouse_dpi.observed = None;
            self.mouse_dpi.queued = None;
            self.mouse_dpi.editing = false;
            self.mouse_dpi.revision = self.mouse_dpi.revision.wrapping_add(1);
        }
        if self.page == crate::nav::Tab::Performance && self.mouse_dpi.observed.is_none() {
            self.request_mouse_dpi_read(cx);
        }
    }

    fn dpi_draft(&self) -> Option<DpiStagesDraft> {
        let state = &self.settings().sensitivity;
        if state
            .stages
            .iter()
            .flatten()
            .any(|value| u16::try_from(*value).is_err())
        {
            return None;
        }
        Some(DpiStagesDraft {
            enabled: state.visible,
            active_stage: state.active + 1,
            stages: state
                .stages
                .iter()
                .zip(&state.slots)
                .map(|(value, slot)| DpiStage {
                    x: value[0] as u16,
                    y: value[1] as u16,
                    visible: slot.enabled,
                    independent: slot.independent,
                })
                .collect(),
        })
    }

    /// Any local input invalidates an older device observation, even before a
    /// slider release or text commit becomes a real hardware request.
    pub(in crate::features) fn dpi_input_changed(&mut self) {
        self.mouse_dpi.revision = self.mouse_dpi.revision.wrapping_add(1);
        self.mouse_dpi.observed = None;
        self.mouse_dpi.editing = true;
    }

    /// Original NORMAL_SKIPPABLE semantics: one retained writer and latest
    /// intent waiting behind it. This is local orchestration, not device save.
    pub(in crate::features) fn request_mouse_dpi(&mut self, cx: &mut Context<Self>) {
        if self.device.product_id != 182 {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let Some(draft) = self.dpi_draft() else {
            return;
        };
        let Some(cap) = mouse_dpi_stages::capability(182) else {
            return;
        };
        if mouse_dpi_stages::pack(cap, &draft).is_err() {
            return;
        }
        self.mouse_dpi.editing = false;
        self.mouse_dpi.queued = Some(Intent {
            scope,
            revision: self.mouse_dpi.revision,
            draft,
            valid_scope: Arc::new(AtomicBool::new(true)),
        });
        self.dispatch_queued_mouse_dpi(cx);
    }

    pub(in crate::features) fn dispatch_queued_mouse_dpi(&mut self, cx: &mut Context<Self>) {
        if self.mouse_settings_write_pending()
            || self.mouse_dpi_read_pending()
            || !self.mouse_dpi.basic_reads_ready
            || self.mouse_dpi.editing
        {
            return;
        }
        let Some(intent) = self.mouse_dpi.queued.take() else {
            return;
        };
        if self.mouse_polling_scope(cx) != Some(intent.scope)
            || self.mouse_dpi.revision != intent.revision
            || self.dpi_draft().as_ref() != Some(&intent.draft)
        {
            return;
        }
        self.mouse_dpi.pending = Some(intent.clone());
        cx.emit(WorkspaceEvent::MouseDpiStagesRequested {
            scope: intent.scope,
            revision: intent.revision,
            draft: intent.draft,
        });
        cx.notify();
    }

    pub fn mouse_dpi_in_flight_matches(
        &self,
        scope: MousePollingScope,
        revision: u64,
        draft: &DpiStagesDraft,
    ) -> bool {
        self.mouse_dpi
            .pending
            .as_ref()
            .is_some_and(|p| p.scope == scope && p.revision == revision && p.draft == *draft)
    }
    pub fn mouse_dpi_write_guard(
        &self,
        scope: MousePollingScope,
        revision: u64,
        draft: &DpiStagesDraft,
    ) -> Option<Arc<AtomicBool>> {
        self.mouse_dpi
            .pending
            .as_ref()
            .filter(|p| p.scope == scope && p.revision == revision && p.draft == *draft)
            .map(|p| p.valid_scope.clone())
    }
    pub fn mouse_dpi_read_guard(
        &self,
        scope: MousePollingScope,
        revision: u64,
    ) -> Option<Arc<AtomicBool>> {
        (self.mouse_dpi.reading == Some((scope, revision)))
            .then(|| self.mouse_dpi.reading_valid_scope.clone())
            .flatten()
    }

    /// Scope validity is distinct from input revision: an already dispatched
    /// write finishes before the newest queued intent may start.
    pub fn mouse_dpi_request_matches(
        &self,
        scope: MousePollingScope,
        revision: u64,
        draft: &DpiStagesDraft,
        cx: &App,
    ) -> bool {
        self.device.product_id == 182
            && self.mouse_polling_scope(cx) == Some(scope)
            && self.mouse_dpi_in_flight_matches(scope, revision, draft)
    }
    pub fn mouse_dpi_observation_matches(
        &self,
        scope: MousePollingScope,
        revision: u64,
        draft: &DpiStagesDraft,
        cx: &App,
    ) -> bool {
        self.mouse_dpi_request_matches(scope, revision, draft, cx)
            && self.mouse_dpi.revision == revision
            && !self.mouse_dpi.editing
            && self.dpi_draft().as_ref() == Some(draft)
    }

    pub fn finish_mouse_dpi(
        &mut self,
        scope: MousePollingScope,
        revision: u64,
        draft: &DpiStagesDraft,
        observed: Option<DpiStagesReading>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.mouse_dpi_in_flight_matches(scope, revision, draft) {
            return false;
        }
        let accepted = self.mouse_dpi_observation_matches(scope, revision, draft, cx);
        self.mouse_dpi.pending = None;
        if accepted {
            if let Some(reading) = observed {
                self.observe_dpi_table(scope, reading);
            }
        }
        self.dispatch_queued_mouse_dpi(cx);
        if !accepted && self.mouse_polling_scope(cx) != Some(scope) {
            self.request_mouse_dpi_read(cx);
        }
        cx.notify();
        true
    }

    pub fn request_mouse_dpi_read(&mut self, cx: &mut Context<Self>) {
        if self.device.product_id != 182
            || self.mouse_settings_write_pending()
            || !self.mouse_dpi.basic_reads_ready
            || self.mouse_dpi.editing
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let token = (scope, self.mouse_dpi.revision);
        if self.mouse_dpi.reading.is_some() {
            return;
        }
        self.mouse_dpi.reading = Some(token);
        self.mouse_dpi.reading_valid_scope = Some(Arc::new(AtomicBool::new(true)));
        cx.emit(WorkspaceEvent::MouseDpiStagesReadRequested {
            scope,
            revision: token.1,
        });
    }

    pub fn mouse_dpi_read_matches(
        &self,
        scope: MousePollingScope,
        revision: u64,
        cx: &App,
    ) -> bool {
        self.mouse_polling_scope(cx) == Some(scope)
            && self.mouse_dpi.reading == Some((scope, revision))
            && self.mouse_dpi.revision == revision
            && !self.mouse_dpi.editing
            && !self.mouse_settings_write_pending()
    }

    pub fn finish_mouse_dpi_read(
        &mut self,
        scope: MousePollingScope,
        revision: u64,
        observed: Option<DpiStagesReading>,
        cx: &mut Context<Self>,
    ) {
        if self.mouse_dpi_reading_token_matches(scope, revision) {
            let current = self.mouse_dpi_read_matches(scope, revision, cx);
            self.mouse_dpi.reading = None;
            self.mouse_dpi.reading_valid_scope = None;
            if current {
                if let Some(reading) = observed {
                    self.observe_dpi_table(scope, reading);
                }
            }
            self.dispatch_queued_mouse_dpi(cx);
            if !current && self.mouse_polling_scope(cx) != Some(scope) {
                self.request_mouse_dpi_read(cx);
            }
            cx.notify();
        }
    }

    pub fn mouse_dpi_reading_token_matches(&self, scope: MousePollingScope, revision: u64) -> bool {
        self.mouse_dpi.reading == Some((scope, revision))
    }
}

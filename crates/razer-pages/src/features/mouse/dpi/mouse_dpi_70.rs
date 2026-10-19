//! Current PID70 sensitivity intents: MW vt persists the profile before Xe;
//! _t persists independent-X/Y without a setter. UI cI dispatches a separate
//! value change when unlinking requires Y to follow X. ACKs never change drafts.
use super::mapping_callbacks_current::{Connection, TemporaryRestoration};
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct MouseLegacyDpiRequested {
    pub scope: MousePollingScope,
    pub sequence: u64,
    pub sensitivity: Value,
    pub send: bool,
    pub persist: bool,
    pub dpi: Option<(u16, u16)>,
    pub cleanup: bool,
    restores_temporary: bool,
    restoration_ticket: u64,
    valid: Arc<AtomicBool>,
}
impl MouseLegacyDpiRequested {
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
    fn cancel(&self) {
        self.valid.store(false, Ordering::Release);
    }
}
impl EventEmitter<MouseLegacyDpiRequested> for MouseProductWorkspace {}

#[derive(Clone)]
pub struct MouseLegacyMappingCommitted;
impl EventEmitter<MouseLegacyMappingCommitted> for MouseProductWorkspace {}

#[derive(Clone)]
pub struct MouseLegacyProfileNavigate {
    pub scope: MousePollingScope,
    pub output: Value,
}
impl EventEmitter<MouseLegacyProfileNavigate> for MouseProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    sequence: u64,
    pending: Option<MouseLegacyDpiRequested>,
    queued: Option<MouseLegacyDpiRequested>,
    pub(super) refresh_controls: bool,
    temporary_restoration: TemporaryRestoration,
}
impl State {
    pub(super) fn observe_mapping_connection(&mut self, connection: Option<Connection>) {
        self.temporary_restoration.observe_connection(connection);
    }

    fn owns_completion(&self, request: &MouseLegacyDpiRequested) -> bool {
        self.pending.as_ref().is_some_and(|pending| {
            pending.sequence == request.sequence
                && pending.scope == request.scope
                && Arc::ptr_eq(&pending.valid, &request.valid)
        })
    }

    fn enqueue(&mut self, request: MouseLegacyDpiRequested) -> Option<MouseLegacyDpiRequested> {
        if self.pending.is_some() {
            if let Some(old) = self.queued.replace(request) {
                old.cancel();
            }
            None
        } else {
            self.pending = Some(request.clone());
            Some(request)
        }
    }

    fn prepare_exit(
        &mut self,
        scope: Option<MousePollingScope>,
        sensitivity: Value,
        restore: bool,
    ) -> Result<Option<MouseLegacyDpiRequested>, String> {
        let persist = self
            .queued
            .iter()
            .chain(self.pending.iter())
            .any(|request| request.is_valid() && Some(request.scope) == scope && request.persist);
        self.cancel();
        let dpi = restore.then(|| selected_dpi(&sensitivity)).flatten();
        if restore && (scope.is_none() || dpi.is_none()) {
            return Err(
                "DPI clutch restoration has no valid current connection and selected stage".into(),
            );
        }
        let Some(scope) = scope.filter(|_| persist || restore) else {
            return Ok(None);
        };
        self.sequence = self.sequence.wrapping_add(1);
        Ok(self.enqueue(MouseLegacyDpiRequested {
            scope,
            sequence: self.sequence,
            sensitivity,
            send: restore,
            persist,
            dpi,
            cleanup: restore,
            restores_temporary: restore,
            restoration_ticket: self.temporary_restoration.ticket(),
            valid: Arc::new(AtomicBool::new(true)),
        }))
    }

    fn finish(
        &mut self,
        request: &MouseLegacyDpiRequested,
        scope: Option<MousePollingScope>,
        confirmed: bool,
    ) -> Option<MouseLegacyDpiRequested> {
        if !self.owns_completion(request) {
            return None;
        }
        if confirmed
            && request.is_valid()
            && scope == Some(request.scope)
            && request.restores_temporary
        {
            self.temporary_restoration
                .confirm(request.restoration_ticket);
        }
        self.pending.take().inspect(|pending| pending.cancel());
        if let Some(next) = self.queued.take() {
            if next.is_valid() && scope == Some(next.scope) {
                return self.enqueue(next);
            }
            next.cancel();
        }
        None
    }

    pub(super) fn cancel(&mut self) {
        if let Some(request) = &self.pending {
            request.cancel();
        }
        if let Some(request) = self.queued.take() {
            request.cancel();
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

// He filters visible stages, retains the selected stage, and uses the selected
// X/Y even with staging disabled. Invalid persisted data cannot form a packet.
fn selected_dpi(sensitivity: &Value) -> Option<(u16, u16)> {
    let index = usize::try_from(sensitivity["Active"].as_u64()?)
        .ok()?
        .checked_sub(1)?;
    let stages = sensitivity.pointer("/Stages/DPIStage")?.as_array()?;
    let stage = stages.get(index)?;
    if stage["Active"] != true {
        return None;
    }
    let x = u16::try_from(stage["X"].as_u64()?).ok()?;
    let y = u16::try_from(stage["Y"].as_u64()?).ok()?;
    ((100..=16000).contains(&x) && (100..=16000).contains(&y)).then_some((x, y))
}

impl MouseProductWorkspace {
    pub(super) fn submit_legacy_dpi_field(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.spec.product_id == 70 && path.starts_with("/DPIStages/") {
            self.submit_legacy_dpi(true, cx);
        }
    }
    pub(super) fn submit_legacy_dpi(&mut self, send: bool, cx: &mut Context<Self>) {
        if self.spec.product_id != 70 || !self.active || self.page != "TAB_PERFORMANCE" {
            return;
        }
        self.submit_legacy_dpi_profile(send, cx);
    }

    pub(super) fn submit_legacy_dpi_profile(&mut self, send: bool, cx: &mut Context<Self>) {
        if self.spec.product_id != 70 {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        // MW vt records the visible count alongside the full retained table.
        if let Some(stages) = self
            .draft
            .pointer("/DPIStages/Stages/DPIStage")
            .and_then(Value::as_array)
        {
            let count = stages
                .iter()
                .filter(|stage| stage["Active"] == true)
                .count();
            self.draft["DPIStages"]["StageNum"] = json!(count);
        }
        let sensitivity = self.draft["DPIStages"].clone();
        let dpi = selected_dpi(&sensitivity);
        self.legacy_dpi.sequence = self.legacy_dpi.sequence.wrapping_add(1);
        // A later independent-only edit still carries an earlier queued DPI
        // mutation; the task queue coalesces profiles without losing that SET.
        let must_send = send || self.legacy_dpi.queued.as_ref().is_some_and(|r| r.send);
        let request = MouseLegacyDpiRequested {
            scope,
            sequence: self.legacy_dpi.sequence,
            sensitivity,
            send: must_send,
            persist: true,
            dpi: must_send.then_some(dpi).flatten(),
            cleanup: false,
            restores_temporary: must_send && dpi.is_some(),
            restoration_ticket: self.legacy_dpi.temporary_restoration.ticket(),
            valid: Arc::new(AtomicBool::new(true)),
        };
        if let Some(request) = self.legacy_dpi.enqueue(request) {
            cx.emit(request);
        }
    }
    pub fn legacy_dpi_request_matches(&self, request: &MouseLegacyDpiRequested, cx: &App) -> bool {
        request.is_valid()
            && self.mouse_polling_scope(cx) == Some(request.scope)
            && self.legacy_dpi.owns_completion(request)
    }
    pub(super) fn submit_legacy_temporary_dpi(
        &mut self,
        dpi: (u16, u16),
        restores_temporary: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if !(100..=16000).contains(&dpi.0) || !(100..=16000).contains(&dpi.1) {
            return;
        }
        if !restores_temporary {
            self.legacy_dpi.temporary_restoration.require();
        }
        self.legacy_dpi.sequence = self.legacy_dpi.sequence.wrapping_add(1);
        // Preserve a requested local edit waiting behind an earlier setter.
        let persist = self
            .legacy_dpi
            .queued
            .as_ref()
            .is_some_and(|request| request.persist);
        let request = MouseLegacyDpiRequested {
            scope,
            sequence: self.legacy_dpi.sequence,
            sensitivity: self.draft["DPIStages"].clone(),
            send: true,
            persist,
            dpi: Some(dpi),
            cleanup: false,
            restores_temporary,
            restoration_ticket: self.legacy_dpi.temporary_restoration.ticket(),
            valid: Arc::new(AtomicBool::new(true)),
        };
        if let Some(request) = self.legacy_dpi.enqueue(request) {
            cx.emit(request);
        }
    }
    /// The current connection's clutch restoration follows the canceled worker's
    /// completed transport cleanup and any already requested profile persistence.
    pub fn prepare_legacy_dpi_exit(
        &mut self,
        cx: &App,
    ) -> Result<Option<MouseLegacyDpiRequested>, String> {
        if self.spec.product_id == 70 {
            self.legacy_mapping.close_callback_admission();
        }
        let restore = self.spec.product_id == 70
            && (self.legacy_mapping.needs_dpi_restore()
                || self.legacy_dpi.temporary_restoration.required());
        self.legacy_dpi.prepare_exit(
            self.mouse_polling_scope(cx),
            self.draft["DPIStages"].clone(),
            restore,
        )
    }

    pub fn legacy_dpi_exit_pending(&self) -> bool {
        self.legacy_dpi.pending.is_some() || self.legacy_dpi.queued.is_some()
    }
    pub fn refresh_legacy_dpi_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.spec.product_id != 70 || !std::mem::take(&mut self.legacy_dpi.refresh_controls) {
            return;
        }
        self.syncing = true;
        for (path, slider) in &self.sliders {
            if path.starts_with("/DPIStages/") {
                if let Some(value) = self.draft.pointer(path).and_then(Value::as_f64) {
                    slider.update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
                }
            }
        }
        for (path, input) in &self.inputs {
            if path.starts_with("/DPIStages/") {
                if let Some(value) = self.draft.pointer(path).and_then(Value::as_i64) {
                    input.update(cx, |input, cx| {
                        input.set_value(value.to_string(), window, cx)
                    });
                }
            }
        }
        self.syncing = false;
        self.reset_dpi_grids(window, cx);
        cx.notify();
    }
    pub fn finish_legacy_dpi(&mut self, request: &MouseLegacyDpiRequested, cx: &mut Context<Self>) {
        self.finish_legacy_dpi_result(request, false, cx);
    }

    pub fn finish_legacy_dpi_result(
        &mut self,
        request: &MouseLegacyDpiRequested,
        confirmed: bool,
        cx: &mut Context<Self>,
    ) {
        if confirmed
            && request.cleanup
            && self.legacy_dpi_request_matches(request, cx)
            && request.restoration_ticket == self.legacy_dpi.temporary_restoration.ticket()
        {
            self.legacy_mapping.finish_dpi_restore();
        }
        if let Some(next) = self
            .legacy_dpi
            .finish(request, self.mouse_polling_scope(cx), confirmed)
        {
            cx.emit(next);
        }
        cx.notify();
    }
}

#[cfg(test)]
#[path = "mouse_dpi_70_tests.rs"]
mod tests;

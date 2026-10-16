//! Current PID70 sensitivity intents: MW vt persists the profile before Xe;
//! _t persists independent-X/Y without a setter. UI cI dispatches a separate
//! value change when unlinking requires Y to follow X. ACKs never change drafts.
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
    valid: Arc<AtomicBool>,
}
impl MouseLegacyDpiRequested {
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
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
}
impl State {
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
            valid: Arc::new(AtomicBool::new(true)),
        };
        if self.legacy_dpi.pending.is_some() {
            if let Some(old) = self.legacy_dpi.queued.replace(request) {
                old.cancel();
            }
        } else {
            self.legacy_dpi.pending = Some(request.clone());
            cx.emit(request);
        }
    }
    pub fn legacy_dpi_request_matches(&self, request: &MouseLegacyDpiRequested, cx: &App) -> bool {
        request.is_valid()
            && self.mouse_polling_scope(cx) == Some(request.scope)
            && self.legacy_dpi.pending.as_ref().is_some_and(|pending| {
                pending.sequence == request.sequence && pending.scope == request.scope
            })
    }
    pub(super) fn submit_legacy_temporary_dpi(&mut self, dpi: (u16, u16), cx: &mut Context<Self>) {
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if !(100..=16000).contains(&dpi.0) || !(100..=16000).contains(&dpi.1) {
            return;
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
            valid: Arc::new(AtomicBool::new(true)),
        };
        if self.legacy_dpi.pending.is_some() {
            if let Some(old) = self.legacy_dpi.queued.replace(request) {
                old.cancel();
            }
        } else {
            self.legacy_dpi.pending = Some(request.clone());
            cx.emit(request);
        }
    }
    /// Exit cancels transport work while draining already requested automatic
    /// persistence, including the last edit waiting behind an in-flight SET.
    pub fn prepare_legacy_dpi_exit(&mut self, cx: &App) -> Option<MouseLegacyDpiRequested> {
        self.legacy_mapping.stop();
        self.mapping_current_state.keyboard.stop();
        let latest = self
            .legacy_dpi
            .queued
            .take()
            .or_else(|| self.legacy_dpi.pending.clone());
        self.legacy_dpi.cancel();
        self.legacy_dpi.pending = None;
        let mut latest = latest?;
        latest.cancel();
        if !latest.persist {
            return None;
        }
        if self.mouse_polling_scope(cx) != Some(latest.scope) {
            return None;
        }
        self.legacy_dpi.sequence = self.legacy_dpi.sequence.wrapping_add(1);
        latest.sequence = self.legacy_dpi.sequence;
        latest.valid = Arc::new(AtomicBool::new(true));
        latest.send = false;
        latest.dpi = None;
        self.legacy_dpi.pending = Some(latest.clone());
        Some(latest)
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
        if !self.legacy_dpi.pending.as_ref().is_some_and(|pending| {
            pending.sequence == request.sequence && pending.scope == request.scope
        }) {
            return;
        }
        self.legacy_dpi
            .pending
            .take()
            .inspect(|pending| pending.cancel());
        if let Some(next) = self.legacy_dpi.queued.take() {
            if next.is_valid() && self.mouse_polling_scope(cx) == Some(next.scope) {
                self.legacy_dpi.pending = Some(next.clone());
                cx.emit(next);
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::selected_dpi;
    use serde_json::json;
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn active_xy_uses_source_selection_with_stages_on_or_off() {
        let mut value = json!({"Active":2,"State":true,"Stages":{"DPIStage":[
            {"Active":false,"X":800,"Y":800}, {"Active":true,"X":1600,"Y":3200}]}});
        assert_eq!(selected_dpi(&value), Some((1600, 3200)));
        value["State"] = false.into();
        assert_eq!(selected_dpi(&value), Some((1600, 3200)));
        value["Active"] = 1.into();
        assert_eq!(selected_dpi(&value), None);
        value["Active"] = 0.into();
        assert_eq!(selected_dpi(&value), None);
    }
    #[test]
    fn invalid_dpi_cannot_become_a_device_request() {
        let value = json!({"Active":1,"Stages":{"DPIStage":[{"Active":true,"X":99,"Y":800}]}});
        assert_eq!(selected_dpi(&value), None);
    }
}

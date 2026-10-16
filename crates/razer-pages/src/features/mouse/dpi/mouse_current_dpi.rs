use super::super::mouse_polling::MousePollingScope;
use super::*;
use razer_device::mouse_current_dpi::{DpiChange, DpiPair, DpiReport};
use razer_device::mouse_dpi_stages::{DpiStage, DpiStagesDraft};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct MouseCurrentDpiRequested {
    scope: MousePollingScope,
    sequence: u64,
    profile: Option<Value>,
    change: Option<DpiChange>,
    read: bool,
    valid: Arc<AtomicBool>,
}

#[cfg(test)]
mod tests {
    use super::{
        MouseCurrentDpiRequested, MousePollingObservation, MousePollingScope,
        MouseProductWorkspace, PollingConnection, State,
    };
    use gpui_kit::{AppContext, EntityId, TestAppContext, px, size};
    use std::sync::{Arc, atomic::AtomicBool};

    // Compile-checked GPUI owner integration. No application/window acceptance
    // is inferred from this test; execution remains outside static verification.
    #[gpui_kit::test]
    fn source_mouse182_number_commit_emits_only_stage_task_and_scope_change_cancels_it(
        cx: &mut TestAppContext,
    ) {
        use gpui_kit::component::Root;
        cx.update(|cx| gpui_kit::init(cx));
        let mut page = None;
        let handle = cx.open_window(size(px(1400.), px(1100.)), |window, cx| {
            let owner = cx.new(|cx| MouseProductWorkspace::new(182, window, cx));
            page = Some(owner.clone());
            Root::new(owner, window, cx)
        });
        let page = page.unwrap();
        let request = cx
            .update_window(handle.into(), |_, window, cx| {
                page.update(cx, |owner, cx| {
                    let previous_active = owner.draft.get("dpi").cloned();
                    owner.write_number("/dpiStages/stages/2/x", 2000., window, cx);
                    let request = owner
                        .current_dpi
                        .pending
                        .as_ref()
                        .expect("actual Source DPI owner emitted edit")
                        .clone();
                    let change = request.change().unwrap();
                    assert_eq!(change.stages.as_ref().unwrap().stages[2].x, 2000);
                    assert_eq!(change.stages.as_ref().unwrap().stages.len(), 5);
                    assert!(change.active.is_none());
                    assert_eq!(owner.draft.get("dpi").cloned(), previous_active);
                    assert_eq!(
                        request.profile().unwrap()["dpiStages"]["stages"][2]["x"],
                        2000
                    );
                    assert!(owner.current_dpi_request_matches(&request, cx));
                    owner.observe_mouse_polling(
                        request.scope(),
                        MousePollingObservation::Connection(Some(PollingConnection::Dongle)),
                        cx,
                    );
                    assert!(!request.is_valid());
                    assert!(!owner.current_dpi_request_matches(&request, cx));
                    owner.finish_current_dpi(&request, None, cx);
                    assert!(owner.current_dpi.pending.is_none());
                    assert_eq!(owner.draft["dpiStages"]["stages"][2]["x"], 2000);
                    request
                })
            })
            .unwrap();
        assert!(!request.is_valid());
    }

    #[::core::prelude::v1::test]
    fn source_dpi_owner_drop_invalidates_in_flight_and_queued_guards() {
        let request = MouseCurrentDpiRequested {
            scope: MousePollingScope::new(EntityId::from(1), 2, 3),
            sequence: 1,
            profile: None,
            change: None,
            read: true,
            valid: Arc::new(AtomicBool::new(true)),
        };
        let mut queued = request.clone();
        queued.sequence = 2;
        queued.valid = Arc::new(AtomicBool::new(true));
        let state = State {
            sequence: 2,
            pending: Some(request.clone()),
            queued: Some(queued.clone()),
            observed: None,
            initialization_key: None,
            macro_recording: false,
        };
        drop(state);
        assert!(!request.is_valid());
        assert!(!queued.is_valid());
    }
}
impl MouseCurrentDpiRequested {
    pub fn scope(&self) -> MousePollingScope {
        self.scope
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn profile(&self) -> Option<&Value> {
        self.profile.as_ref()
    }
    pub fn change(&self) -> Option<&DpiChange> {
        self.change.as_ref()
    }
    pub fn is_read(&self) -> bool {
        self.read
    }
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

#[derive(Default)]
pub(super) struct State {
    sequence: u64,
    pending: Option<MouseCurrentDpiRequested>,
    queued: Option<MouseCurrentDpiRequested>,
    // Source task results never replace the optimistic source profile.
    observed: Option<DpiReport>,
    initialization_key: Option<String>,
    macro_recording: bool,
}
impl State {
    pub(super) fn cancel(&mut self) {
        if let Some(pending) = &self.pending {
            pending.cancel();
        }
        if let Some(queued) = self.queued.take() {
            queued.cancel();
        }
        self.observed = None;
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl EventEmitter<MouseCurrentDpiRequested> for MouseProductWorkspace {}

impl MouseProductWorkspace {
    pub fn initialize_mouse182_profile(
        &mut self,
        key: Option<String>,
        profile_guid: &str,
        macro_recording: bool,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 182 {
            return;
        }
        self.initialize_mouse182_polling(key.as_deref(), profile_guid, cx);
        self.current_dpi.macro_recording = macro_recording;
        if macro_recording {
            if let Some(pending) = &self.current_dpi.pending {
                pending.cancel();
            }
            if let Some(queued) = &mut self.current_dpi.queued {
                queued.change = None;
            }
            self.current_dpi.initialization_key = None;
            return;
        }
        let Some(key) = key else {
            self.current_dpi.initialization_key = None;
            return;
        };
        let key = format!("{key}:{profile_guid}");
        if self.current_dpi.initialization_key.as_ref() == Some(&key)
            || self.current_dpi.pending.is_some()
            || profile_guid.is_empty()
        {
            return;
        }
        let (Some(scope), Some(stages)) = (self.mouse_polling_scope(cx), self.current_dpi_draft())
        else {
            return;
        };
        self.current_dpi.initialization_key = Some(key);
        self.current_dpi.sequence = self.current_dpi.sequence.wrapping_add(1);
        self.enqueue_current_dpi(
            MouseCurrentDpiRequested {
                scope,
                sequence: self.current_dpi.sequence,
                profile: None,
                change: Some(DpiChange {
                    stages: Some(stages),
                    active: None,
                    profile_guid: Some(profile_guid.into()),
                    initialize: true,
                }),
                read: false,
                valid: Arc::new(AtomicBool::new(true)),
            },
            cx,
        );
    }
    pub fn apply_current_dpi_profile(&mut self, cx: &mut Context<Self>) {
        self.submit_current_dpi(true, true, cx);
    }
    fn current_dpi_draft(&self) -> Option<DpiStagesDraft> {
        let stages = self
            .draft
            .pointer(self.spec.stages_path())?
            .as_array()?
            .iter()
            .map(|stage| {
                Some(DpiStage {
                    x: u16::try_from(stage[self.spec.dpi_axis(0)].as_u64()?).ok()?,
                    y: u16::try_from(stage[self.spec.dpi_axis(1)].as_u64()?).ok()?,
                    visible: stage[self.spec.visible_key()].as_bool()?,
                    independent: stage[self.spec.independent_key()].as_bool()?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(DpiStagesDraft {
            enabled: self
                .draft
                .pointer(self.spec.stage_enable_path())?
                .as_bool()?,
            active_stage: usize::try_from(self.draft.pointer(self.spec.active_path())?.as_u64()?)
                .ok()?,
            stages,
        })
    }

    pub(super) fn submit_current_dpi_field(&mut self, path: &str, cx: &mut Context<Self>) {
        if razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none()
            || !(path.starts_with(self.spec.stages_path())
                || path == self.spec.active_path()
                || path == self.spec.stage_enable_path())
            || path.ends_with(self.spec.independent_key())
        {
            return;
        }
        // uA / cA / CA / NA update the full stage table. Selecting a stage,
        // changing a visible row, hiding/reordering or staging enable also
        // dispatches IA's independent current X/Y task.
        self.submit_current_dpi(true, true, cx);
    }

    pub(super) fn submit_current_dpi(
        &mut self,
        stages: bool,
        active: bool,
        cx: &mut Context<Self>,
    ) {
        // Current182 UI ZE -> ON_STAGES_CHANGE -> 90012.dn -> 30525.ft
        // queues only85191.Xe/setDPIStages(1). et/setDpiLevel(0) belongs to
        // separate temporary/OTFS actions and is not an extra stage edit.
        let active = active && self.spec.product_id != 182;
        if razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none() {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let Some(draft) = self.current_dpi_draft() else {
            return;
        };
        let selected = draft
            .active_stage
            .checked_sub(1)
            .and_then(|index| draft.stages.get(index))
            .map(|row| DpiPair { x: row.x, y: row.y });
        if active {
            if let Some(selected) = selected {
                if self.draft.get("DPI").is_some() {
                    self.draft["DPI"] = json!({"X":selected.x,"Y":selected.y});
                } else {
                    self.draft["dpi"] = json!({"x":selected.x,"y":selected.y});
                }
            }
        }
        if self.draft.get("DPIStages").is_some() {
            self.draft["DPIStages"]["StageNum"] =
                json!(draft.stages.iter().filter(|stage| stage.visible).count());
        } else {
            self.draft["dpiStages"]["count"] =
                json!(draft.stages.iter().filter(|stage| stage.visible).count());
        }
        self.current_dpi.sequence = self.current_dpi.sequence.wrapping_add(1);
        let old = self
            .current_dpi
            .queued
            .as_ref()
            .and_then(|request| request.change.as_ref());
        let stage_change = stages || old.is_some_and(|change| change.stages.is_some());
        let active_change = active || old.is_some_and(|change| change.active.is_some());
        let change =
            (!self.current_dpi.macro_recording && (stage_change || active_change)).then(|| {
                DpiChange {
                    stages: stage_change.then_some(draft),
                    active: active_change.then_some(selected).flatten(),
                    profile_guid: (self.spec.product_id == 182).then(|| {
                        self.draft
                            .get("guid")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned()
                    }),
                    initialize: false,
                }
            });
        let request = MouseCurrentDpiRequested {
            scope,
            sequence: self.current_dpi.sequence,
            profile: Some(self.draft.clone()),
            change,
            read: false,
            valid: Arc::new(AtomicBool::new(true)),
        };
        self.enqueue_current_dpi(request, cx);
    }

    pub fn request_current_dpi_read(&mut self, cx: &mut Context<Self>) {
        if razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none()
            || self.current_dpi.pending.is_some()
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        self.current_dpi.sequence = self.current_dpi.sequence.wrapping_add(1);
        self.enqueue_current_dpi(
            MouseCurrentDpiRequested {
                scope,
                sequence: self.current_dpi.sequence,
                profile: None,
                change: None,
                read: true,
                valid: Arc::new(AtomicBool::new(true)),
            },
            cx,
        );
    }

    /// Source90012.gn obtains the protocol serial before77846 reads the
    /// serial-indexed profile cache. This request contains no setting or
    /// profile, so the transport must perform only the identity getter.
    pub fn request_current_dpi_identity(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id != 182 || self.current_dpi.pending.is_some() {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        self.current_dpi.sequence = self.current_dpi.sequence.wrapping_add(1);
        self.enqueue_current_dpi(
            MouseCurrentDpiRequested {
                scope,
                sequence: self.current_dpi.sequence,
                profile: None,
                change: Some(DpiChange {
                    stages: None,
                    active: None,
                    profile_guid: None,
                    initialize: true,
                }),
                read: true,
                valid: Arc::new(AtomicBool::new(true)),
            },
            cx,
        );
    }

    fn enqueue_current_dpi(&mut self, request: MouseCurrentDpiRequested, cx: &mut Context<Self>) {
        if self.current_dpi.pending.is_some() {
            if let Some(previous) = self.current_dpi.queued.replace(request) {
                previous.cancel();
            }
        } else {
            self.current_dpi.pending = Some(request.clone());
            cx.emit(request);
        }
    }

    pub fn current_dpi_request_matches(
        &self,
        request: &MouseCurrentDpiRequested,
        cx: &App,
    ) -> bool {
        request.is_valid()
            && self.mouse_polling_scope(cx) == Some(request.scope)
            && self.current_dpi.pending.as_ref().is_some_and(|pending| {
                pending.sequence == request.sequence && pending.scope == request.scope
            })
    }

    pub fn finish_current_dpi(
        &mut self,
        request: &MouseCurrentDpiRequested,
        observed: Option<DpiReport>,
        cx: &mut Context<Self>,
    ) {
        if !self.current_dpi.pending.as_ref().is_some_and(|pending| {
            pending.sequence == request.sequence && pending.scope == request.scope
        }) {
            return;
        }
        if self.current_dpi_request_matches(request, cx) {
            self.current_dpi.observed = observed;
        }
        if let Some(pending) = self.current_dpi.pending.take() {
            pending.cancel();
        }
        if let Some(next) = self.current_dpi.queued.take() {
            if next.is_valid() && self.mouse_polling_scope(cx) == Some(next.scope) {
                self.current_dpi.pending = Some(next.clone());
                cx.emit(next);
            }
        }
        cx.notify();
    }

    /// Closing drains the newest requested profile through the serialized local
    /// writer while canceling all device work. No source rollback is invented.
    pub fn prepare_current_dpi_exit(&mut self, cx: &App) -> Option<MouseCurrentDpiRequested> {
        let latest = self
            .current_dpi
            .queued
            .take()
            .or_else(|| self.current_dpi.pending.clone());
        self.current_dpi.cancel();
        self.current_dpi.pending = None;
        let mut latest = latest?;
        latest.cancel();
        if latest.profile.is_none() || self.mouse_polling_scope(cx) != Some(latest.scope) {
            return None;
        }
        self.current_dpi.sequence = self.current_dpi.sequence.wrapping_add(1);
        latest.sequence = self.current_dpi.sequence;
        latest.change = None;
        latest.valid = Arc::new(AtomicBool::new(true));
        self.current_dpi.pending = Some(latest.clone());
        Some(latest)
    }
}

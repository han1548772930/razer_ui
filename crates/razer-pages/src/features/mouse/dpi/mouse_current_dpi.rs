use super::super::mouse_polling::MousePollingScope;
use super::*;
use gpui_kit::component::Sizable;
use razer_device::mouse_current_dpi::{DpiChange, DpiPair, DpiReport};
use razer_device::mouse_dpi_stages::{DpiStage, DpiStagesDraft};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[path = "mouse_current_dpi_observation.rs"]
mod observation;
use observation::{Observation, Selection};

#[derive(Clone)]
pub struct MouseCurrentDpiRequested {
    scope: MousePollingScope,
    sequence: u64,
    profile: Option<Value>,
    change: Option<DpiChange>,
    read: bool,
    connection_key: Option<String>,
    edit_revision: u64,
    selection: Option<Selection>,
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

    #[::core::prelude::v1::test]
    fn current190_source_separates_stage_table_edits_from_transient_stage_selection() {
        use super::{Edit, Tasks};
        for edit in [
            Edit::Stages,
            Edit::Profile,
            Edit::AxisMode { reset_y: true },
        ] {
            assert_eq!(
                edit.tasks(190),
                Tasks {
                    stages: true,
                    active: false
                }
            );
        }
        assert_eq!(
            Edit::SelectStage.tasks(190),
            Tasks {
                stages: true,
                active: true
            }
        );
        assert_eq!(
            Edit::AxisMode { reset_y: false }.tasks(190),
            Tasks {
                stages: false,
                active: false
            }
        );
    }

    #[::core::prelude::v1::test]
    fn current182_stage_actions_do_not_issue_the_separate_otfs_level_task() {
        use super::{Edit, Tasks};
        for edit in [
            Edit::Stages,
            Edit::SelectStage,
            Edit::Profile,
            Edit::AxisMode { reset_y: true },
        ] {
            assert_eq!(
                edit.tasks(182),
                Tasks {
                    stages: true,
                    active: false
                }
            );
        }
    }

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
            connection_key: None,
            edit_revision: 0,
            selection: None,
            valid: Arc::new(AtomicBool::new(true)),
        };
        let mut queued = request.clone();
        queued.sequence = 2;
        queued.valid = Arc::new(AtomicBool::new(true));
        let mut state = State::default();
        state.sequence = 2;
        state.pending = Some(request.clone());
        state.queued = Some(queued.clone());
        drop(state);
        assert!(!request.is_valid());
        assert!(!queued.is_valid());
    }

    fn read(sequence: u64, connection: &str) -> MouseCurrentDpiRequested {
        MouseCurrentDpiRequested {
            scope: MousePollingScope::new(EntityId::from(1), 2, 3),
            sequence,
            profile: None,
            change: None,
            read: true,
            connection_key: Some(connection.into()),
            edit_revision: 0,
            selection: None,
            valid: Arc::new(AtomicBool::new(true)),
        }
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_busy_refresh_survives_queued_profile_edit() {
        let mut owner = State::default();
        let first = read(1, "route/profile");
        assert!(owner.enqueue(first.clone()).is_some());
        let mut edit = read(2, "route/profile");
        edit.read = false;
        edit.profile = Some(serde_json::json!({"dpiStages":{"active":3}}));
        assert!(owner.enqueue(edit.clone()).is_none());
        owner.request_refresh();
        assert!(!owner.take_ready_refresh());
        assert!(owner.release(&first));
        let queued = owner.queued.take().expect("refresh retains the user edit");
        assert_eq!(queued.sequence, 2);
        assert!(owner.enqueue(queued).is_some());
        assert!(!owner.take_ready_refresh());
        assert!(owner.release(&edit));
        assert!(owner.take_ready_refresh());
        assert!(!owner.take_ready_refresh());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_edit_after_get_prevents_profile_projection() {
        let mut owner = State::default();
        owner.connection_key = Some("route/profile".into());
        let request = read(1, "route/profile");
        owner.enqueue(request.clone());
        assert!(owner.projection_current(&request));
        owner.record_edit();
        assert!(!owner.projection_current(&request));
        assert!(owner.owns_completion(&request));
        assert!(owner.release(&request));
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_old_connection_completion_cannot_release_successor() {
        let mut owner = State::default();
        let old = read(1, "old-route/profile");
        owner.enqueue(old.clone());
        owner.cancel();
        let new = read(1, "new-route/profile");
        owner.enqueue(new.clone());
        assert!(!old.is_valid());
        assert!(owner.release(&old));
        let next = owner.queued.take().unwrap();
        owner.enqueue(next);
        assert!(!owner.release(&old));
        assert!(owner.owns_completion(&new));
        assert!(new.is_valid());
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_failure_can_refresh_and_disconnect_clears_values() {
        let mut owner = State::default();
        owner.connection_key = Some("route/profile".into());
        owner.row_values[2] = Some(super::DpiPair { x: 2000, y: 2000 });
        owner.read_error = Some("getter failed".into());
        owner.request_refresh();
        assert!(owner.take_ready_refresh());
        assert!(owner.read_error.is_none());
        assert_eq!(owner.row_values[2].unwrap().x, 2000);
        owner.cancel();
        assert_eq!(owner.row_values, [None; 5]);
        assert!(owner.connection_key.is_none());
        assert!(!owner.refresh_requested);
    }

    #[::core::prelude::v1::test]
    fn current_dpi_observation_reorder_keeps_observed_identity_with_its_source_row() {
        let mut owner = State::default();
        owner.row_values[2] = Some(super::DpiPair { x: 2000, y: 2000 });
        owner.move_row(2, 4);
        assert_eq!(owner.row_values[2], None);
        assert_eq!(owner.row_values[4].unwrap().x, 2000);
        owner.move_row(4, 1);
        assert_eq!(owner.row_values[4], None);
        assert_eq!(owner.row_values[1].unwrap().x, 2000);
    }

    #[gpui_kit::test]
    fn source_current_dpi_actual_pair_syncs_original_input_and_preserves_uncommitted_text(
        cx: &mut TestAppContext,
    ) {
        use gpui_kit::component::Root;
        use serde_json::json;
        cx.update(|cx| gpui_kit::init(cx));
        let mut page = None;
        let handle = cx.open_window(size(px(1400.), px(1100.)), |window, cx| {
            let owner = cx.new(|cx| MouseProductWorkspace::new(190, window, cx));
            page = Some(owner.clone());
            Root::new(owner, window, cx)
        });
        let page = page.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            page.update(cx, |owner, cx| {
                let path = "/dpiStages/stages/2/x";
                let report = |value| {
                    serde_json::from_value(json!({
                        "requested":null, "stages":null, "active":{"x":value,"y":value},
                        "acknowledgements":[], "verified":false,
                        "read_errors":{"active":null,"stages":"table getter failed"}
                    }))
                    .unwrap()
                };
                owner.request_current_dpi_read(cx);
                let request = owner.current_dpi.pending.clone().unwrap();
                owner.finish_current_dpi_result(&request, Ok(report(2000)), cx);
                assert_eq!(owner.number(path), 2000.);
                owner.sync_current_dpi_controls(window, cx);
                assert_eq!(owner.inputs[path].read(cx).value().to_string(), "2000");
                assert!(owner.boolean("/dpiStages/enable"));
                assert!(!owner.current_dpi_row_available(0));
                owner.dpi_numbers.get_mut(path).unwrap().registered = true;
                owner.inputs[path].update(cx, |input, cx| input.set_value("2300", window, cx));
                owner.request_current_dpi_read(cx);
                let request = owner.current_dpi.pending.clone().unwrap();
                owner.finish_current_dpi_result(&request, Ok(report(2500)), cx);
                owner.sync_current_dpi_controls(window, cx);
                assert_eq!(owner.number(path), 2000.);
                assert_eq!(owner.inputs[path].read(cx).value().to_string(), "2300");
                assert_eq!(owner.current_dpi.observed.current.unwrap().x, 2500);
            });
        })
        .unwrap();
    }
}

#[cfg(test)]
#[path = "mouse_current_dpi_initialization_tests.rs"]
mod initialization_tests;

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
    initialization_queued: Option<MouseCurrentDpiRequested>,
    observed: Observation,
    row_values: [Option<DpiPair>; 5],
    control_sync: BTreeMap<String, u16>,
    connection_key: Option<String>,
    connection_scope: Option<MousePollingScope>,
    edit_revision: u64,
    refresh_requested: bool,
    read_error: Option<String>,
    initialization_key: Option<String>,
    macro_recording: bool,
}

#[derive(Clone, Copy)]
pub(super) enum Edit {
    Stages,
    SelectStage,
    AxisMode { reset_y: bool },
    Profile,
}

#[derive(Debug, PartialEq, Eq)]
struct Tasks {
    stages: bool,
    active: bool,
}

struct ProfileInitialization<'a> {
    connection_key: Option<String>,
    scope: Option<MousePollingScope>,
    profile_guid: &'a str,
    stages: Option<DpiStagesDraft>,
    selection: Option<Selection>,
    macro_recording: bool,
}

impl Edit {
    fn tasks(self, product_id: u32) -> Tasks {
        let stages = match self {
            Self::AxisMode { reset_y } => reset_y,
            _ => true,
        };
        // Current190 pA's row edits, enable, visibility and reorder emit only
        // IE/ON_STAGES_CHANGE. pA.setStage also emits IA's transient DPI task.
        // Current182 ZE has no transient task for any stage edit.
        let active =
            product_id != 182 && stages && (product_id != 190 || matches!(self, Self::SelectStage));
        Tasks { stages, active }
    }
}
impl State {
    fn enqueue(&mut self, request: MouseCurrentDpiRequested) -> Option<MouseCurrentDpiRequested> {
        if self.pending.is_some() {
            if let Some(previous) = self.queued.replace(request) {
                self.finish_initialization(&previous, false);
                previous.cancel();
            }
            None
        } else {
            self.pending = Some(request.clone());
            Some(request)
        }
    }

    fn record_edit(&mut self) {
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.control_sync.clear();
    }

    fn move_row(&mut self, from: usize, to: usize) {
        if from < to && to < self.row_values.len() {
            self.row_values[from..=to].rotate_left(1);
        } else if to < from && from < self.row_values.len() {
            self.row_values[to..=from].rotate_right(1);
        }
        self.control_sync.clear();
    }

    fn projection_current(&self, request: &MouseCurrentDpiRequested) -> bool {
        self.owns_completion(request)
            && request.connection_key == self.connection_key
            && request.edit_revision == self.edit_revision
    }

    fn owns_completion(&self, request: &MouseCurrentDpiRequested) -> bool {
        self.pending.as_ref().is_some_and(|pending| {
            pending.sequence == request.sequence
                && pending.scope == request.scope
                && pending.connection_key == request.connection_key
                && Arc::ptr_eq(&pending.valid, &request.valid)
        })
    }

    fn release(&mut self, request: &MouseCurrentDpiRequested) -> bool {
        if !self.owns_completion(request) {
            return false;
        }
        if let Some(pending) = self.pending.take() {
            pending.cancel();
        }
        true
    }

    fn request_refresh(&mut self) {
        self.refresh_requested = true;
    }

    fn install_connection(
        &mut self,
        key: Option<String>,
        scope: Option<MousePollingScope>,
    ) -> bool {
        if self.connection_key == key && self.connection_scope == scope {
            return false;
        }
        self.cancel();
        self.connection_key = key;
        self.connection_scope = scope;
        true
    }

    fn initialize_connection(
        &mut self,
        input: ProfileInitialization<'_>,
    ) -> Option<MouseCurrentDpiRequested> {
        let changed = self.install_connection(input.connection_key, input.scope);
        self.macro_recording = input.macro_recording;
        if input.macro_recording {
            if let Some(pending) = &self.pending {
                pending.cancel();
            }
            if let Some(queued) = &mut self.queued {
                queued.change = None;
            }
            if let Some(initialization) = self.initialization_queued.take() {
                initialization.cancel();
            }
            self.initialization_key = None;
            if changed && self.connection_key.is_some() {
                self.request_refresh();
            }
            return None;
        }
        self.connection_key.as_ref()?;
        if let (Some(scope), Some(stages), false) =
            (input.scope, input.stages, input.profile_guid.is_empty())
        {
            return self.initialize_profile(scope, stages, input.profile_guid, input.selection);
        }
        if changed {
            self.request_refresh();
        }
        None
    }

    /// Install the connection first, then retain the source initializer and its
    /// following GET behind any predecessor's cancellation/cleanup barrier.
    fn initialize_profile(
        &mut self,
        scope: MousePollingScope,
        stages: DpiStagesDraft,
        profile_guid: &str,
        selection: Option<Selection>,
    ) -> Option<MouseCurrentDpiRequested> {
        let key = self.connection_key.clone()?;
        if profile_guid.is_empty() || self.initialization_key.as_ref() == Some(&key) {
            return None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        let request = MouseCurrentDpiRequested {
            scope,
            sequence: self.sequence,
            profile: None,
            change: Some(DpiChange {
                stages: Some(stages),
                active: None,
                profile_guid: Some(profile_guid.into()),
                initialize: true,
            }),
            read: false,
            connection_key: Some(key.clone()),
            edit_revision: self.edit_revision,
            selection,
            valid: Arc::new(AtomicBool::new(true)),
        };
        let ready = self.enqueue_initialization(request);
        self.initialization_key = Some(key);
        self.request_refresh();
        ready
    }

    fn enqueue_initialization(
        &mut self,
        request: MouseCurrentDpiRequested,
    ) -> Option<MouseCurrentDpiRequested> {
        if self.pending.is_some() {
            if let Some(previous) = self.initialization_queued.replace(request) {
                self.finish_initialization(&previous, false);
                previous.cancel();
            }
            None
        } else {
            self.enqueue(request)
        }
    }

    fn request_identity(&mut self, scope: MousePollingScope) -> Option<MouseCurrentDpiRequested> {
        if self
            .pending
            .iter()
            .chain(self.initialization_queued.iter())
            .any(|request| {
                request.is_valid()
                    && request.scope == scope
                    && request.connection_key == self.connection_key
                    && request
                        .change
                        .as_ref()
                        .is_some_and(DpiChange::is_identity_read)
            })
        {
            return None;
        }
        self.sequence = self.sequence.wrapping_add(1);
        self.enqueue_initialization(MouseCurrentDpiRequested {
            scope,
            sequence: self.sequence,
            profile: None,
            change: Some(DpiChange {
                stages: None,
                active: None,
                profile_guid: None,
                initialize: true,
            }),
            read: true,
            connection_key: self.connection_key.clone(),
            edit_revision: self.edit_revision,
            selection: None,
            valid: Arc::new(AtomicBool::new(true)),
        })
    }

    fn finish_initialization(&mut self, request: &MouseCurrentDpiRequested, succeeded: bool) {
        if !succeeded
            && request
                .change
                .as_ref()
                .is_some_and(|change| change.initialize && !change.is_identity_read())
            && self.connection_scope == Some(request.scope)
            && request.connection_key == self.initialization_key
        {
            self.initialization_key = None;
        }
    }

    fn take_queued(
        &mut self,
        scope: Option<MousePollingScope>,
    ) -> Option<MouseCurrentDpiRequested> {
        if let Some(initialization) = self.initialization_queued.take() {
            if initialization.is_valid()
                && scope == Some(initialization.scope)
                && initialization.connection_key == self.connection_key
            {
                self.pending = Some(initialization.clone());
                return Some(initialization);
            }
            initialization.cancel();
            self.finish_initialization(&initialization, false);
        }
        if let Some(next) = self.queued.take() {
            if next.is_valid()
                && scope == Some(next.scope)
                && next.connection_key == self.connection_key
            {
                self.pending = Some(next.clone());
                return Some(next);
            }
            next.cancel();
        }
        None
    }

    fn take_ready_refresh(&mut self) -> bool {
        if self.pending.is_some() || !self.refresh_requested {
            return false;
        }
        self.refresh_requested = false;
        self.read_error = None;
        true
    }

    pub(super) fn cancel(&mut self) {
        if let Some(pending) = &self.pending {
            pending.cancel();
        }
        if let Some(queued) = self.queued.take() {
            queued.cancel();
        }
        if let Some(initialization) = self.initialization_queued.take() {
            initialization.cancel();
        }
        self.observed = Observation::default();
        self.row_values = [None; 5];
        self.control_sync.clear();
        self.connection_key = None;
        self.connection_scope = None;
        self.initialization_key = None;
        self.refresh_requested = false;
        self.read_error = None;
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl EventEmitter<MouseCurrentDpiRequested> for MouseProductWorkspace {}

impl MouseProductWorkspace {
    /// Called after the source profile/initialization owners have selected the
    /// current physical route and profile. A busy worker retains the refresh.
    pub fn observe_current_dpi_connection(&mut self, key: Option<String>, cx: &mut Context<Self>) {
        let scope = self.mouse_polling_scope(cx);
        if !self.current_dpi.install_connection(key, scope) {
            return;
        }
        if self.current_dpi.connection_key.is_some() {
            self.request_current_dpi_read(cx);
        }
        cx.notify();
    }

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
        let initialization = ProfileInitialization {
            connection_key: key.as_ref().map(|key| format!("{key}:{profile_guid}")),
            scope: self.mouse_polling_scope(cx),
            profile_guid,
            stages: self.current_dpi_draft(),
            selection: self.current_dpi_selection(),
            macro_recording,
        };
        let request = self.current_dpi.initialize_connection(initialization);
        self.initialize_current_mouse_polling(key.as_deref(), profile_guid, cx);
        if let Some(request) = request {
            cx.emit(request);
        }
        self.start_current_dpi_refresh(cx);
        cx.notify();
    }
    pub fn apply_current_dpi_profile(&mut self, cx: &mut Context<Self>) {
        self.submit_current_dpi(Edit::Profile, cx);
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

    fn current_dpi_selection(&self) -> Option<Selection> {
        Selection::capture(self.spec.product_id, &self.current_dpi_draft()?)
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
        let edit = if path == self.spec.active_path() {
            Edit::SelectStage
        } else {
            Edit::Stages
        };
        self.submit_current_dpi(edit, cx);
    }

    pub(super) fn submit_current_dpi(&mut self, edit: Edit, cx: &mut Context<Self>) {
        let Tasks { stages, active } = edit.tasks(self.spec.product_id);
        if razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none() {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let Some(draft) = self.current_dpi_draft() else {
            return;
        };
        self.current_dpi.record_edit();
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
                    profile_guid: matches!(self.spec.product_id, 182 | 190).then(|| {
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
            connection_key: self.current_dpi.connection_key.clone(),
            edit_revision: self.current_dpi.edit_revision,
            selection: self.current_dpi_selection(),
            valid: Arc::new(AtomicBool::new(true)),
        };
        self.enqueue_current_dpi(request, cx);
    }

    pub fn request_current_dpi_read(&mut self, cx: &mut Context<Self>) {
        if razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none() {
            return;
        }
        self.current_dpi.request_refresh();
        self.start_current_dpi_refresh(cx);
    }

    fn start_current_dpi_refresh(&mut self, cx: &mut Context<Self>) {
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if !self.current_dpi.take_ready_refresh() {
            return;
        }
        self.current_dpi.sequence = self.current_dpi.sequence.wrapping_add(1);
        self.enqueue_current_dpi(
            MouseCurrentDpiRequested {
                scope,
                sequence: self.current_dpi.sequence,
                profile: None,
                change: None,
                read: true,
                connection_key: self.current_dpi.connection_key.clone(),
                edit_revision: self.current_dpi.edit_revision,
                selection: self.current_dpi_selection(),
                valid: Arc::new(AtomicBool::new(true)),
            },
            cx,
        );
    }

    /// Source90012.gn obtains the protocol serial before77846 reads the
    /// serial-indexed profile cache. This request contains no setting or
    /// profile, so the transport must perform only the identity getter.
    pub fn request_current_dpi_identity(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id != 182 {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if let Some(request) = self.current_dpi.request_identity(scope) {
            cx.emit(request);
        }
    }

    fn enqueue_current_dpi(&mut self, request: MouseCurrentDpiRequested, cx: &mut Context<Self>) {
        if let Some(request) = self.current_dpi.enqueue(request) {
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
            && self.current_dpi.connection_key == request.connection_key
            && self.current_dpi.owns_completion(request)
    }

    pub fn finish_current_dpi(
        &mut self,
        request: &MouseCurrentDpiRequested,
        observed: Option<DpiReport>,
        cx: &mut Context<Self>,
    ) {
        self.accept_current_dpi_result(request, Ok(observed), cx);
    }

    fn accept_current_dpi_result(
        &mut self,
        request: &MouseCurrentDpiRequested,
        result: Result<Option<DpiReport>, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.current_dpi.owns_completion(request) {
            return;
        }
        let current = self.current_dpi_request_matches(request, cx);
        let initialized = current
            && result
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .is_some_and(|report| report.verified);
        self.current_dpi.finish_initialization(request, initialized);
        if current {
            match result {
                Ok(Some(report)) => {
                    let values = self
                        .current_dpi
                        .observed
                        .accept(request.selection.as_ref(), &report);
                    self.current_dpi.read_error = None;
                    if request.is_read()
                        && request.change.is_none()
                        && self.current_dpi.projection_current(request)
                    {
                        self.apply_current_dpi_observation(values, cx);
                    }
                }
                Err(error) => self.current_dpi.read_error = Some(error),
                Ok(None) => {}
            }
        }
        self.current_dpi.release(request);
        let scope = self.mouse_polling_scope(cx);
        if let Some(next) = self.current_dpi.take_queued(scope) {
            cx.emit(next);
        }
        self.start_current_dpi_refresh(cx);
        cx.notify();
    }

    pub fn finish_current_dpi_result(
        &mut self,
        request: &MouseCurrentDpiRequested,
        result: Result<DpiReport, String>,
        cx: &mut Context<Self>,
    ) {
        self.accept_current_dpi_result(request, result.map(Some), cx);
    }

    fn current_dpi_row_has_preview(&self, index: usize, cx: &App) -> bool {
        if self.dpi_dragged_row.is_some() {
            return true;
        }
        for axis in 0..2 {
            let path = format!(
                "{}/{index}/{}",
                self.spec.stages_path(),
                self.spec.dpi_axis(axis)
            );
            if self
                .dpi_numbers
                .get(&path)
                .is_some_and(|state| state.registered || state.typed)
            {
                return true;
            }
            if self.current_dpi.control_sync.contains_key(&path) {
                continue;
            }
            if self
                .sliders
                .get(&path)
                .is_some_and(|slider| slider.read(cx).value().start() != self.number(&path))
                || self.inputs.get(&path).is_some_and(|input| {
                    input.read(cx).value().to_string() != (self.number(&path) as i64).to_string()
                })
            {
                return true;
            }
        }
        self.dpi_single_sliders.get(&index).is_some_and(|slider| {
            let path = format!(
                "{}/{index}/{}",
                self.spec.stages_path(),
                self.spec.dpi_axis(0)
            );
            !self.current_dpi.control_sync.contains_key(&path)
                && slider.read(cx).value().start() != self.number(&path)
        })
    }

    fn apply_current_dpi_observation(&mut self, values: [Option<DpiPair>; 5], cx: &App) {
        for (index, value) in values.into_iter().enumerate() {
            let Some(value) = value else { continue };
            if self.current_dpi_row_has_preview(index, cx) {
                continue;
            }
            self.current_dpi.row_values[index] = Some(value);
            for (axis, value) in [value.x, value.y].into_iter().enumerate() {
                let path = format!(
                    "{}/{index}/{}",
                    self.spec.stages_path(),
                    self.spec.dpi_axis(axis)
                );
                if self.draft.pointer(&path).is_some() {
                    set_pointer(&mut self.draft, &path, json!(value));
                    self.current_dpi.control_sync.insert(path, value);
                }
            }
        }
    }

    /// Domain acceptance and worker settlement do not require a live window.
    /// Only the retained input/slider synchronization uses this optional step.
    pub fn sync_current_dpi_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let updates = std::mem::take(&mut self.current_dpi.control_sync);
        self.syncing = true;
        for (path, value) in updates {
            if self
                .dpi_numbers
                .get(&path)
                .is_some_and(|state| state.registered || state.typed)
            {
                continue;
            }
            if let Some(input) = self.inputs.get(&path) {
                input.update(cx, |input, cx| {
                    input.set_value(value.to_string(), window, cx)
                });
            }
            if let Some(slider) = self.sliders.get(&path) {
                slider.update(cx, |slider, cx| {
                    slider.set_value(f32::from(value), window, cx)
                });
            }
            if let Some(grid) = self.dpi_grids.get(&path) {
                grid.update(cx, |grid, cx| grid.reset(f32::from(value), window, cx));
            }
            if let Some((base, axis)) = path.rsplit_once('/') {
                if axis == self.spec.dpi_axis(0) {
                    if let Some(single) = base
                        .rsplit('/')
                        .next()
                        .and_then(|index| index.parse::<usize>().ok())
                        .and_then(|index| self.dpi_single_sliders.get(&index))
                    {
                        single.update(cx, |single, cx| {
                            single.set_value(f32::from(value), window, cx)
                        });
                    }
                }
            }
        }
        self.syncing = false;
        cx.notify();
    }

    pub(super) fn current_dpi_row_available(&self, index: usize) -> bool {
        razer_device::mouse_current_dpi::capability(self.spec.product_id).is_none()
            || self
                .current_dpi
                .row_values
                .get(index)
                .is_some_and(Option::is_some)
    }

    pub(super) fn move_current_dpi_row(&mut self, from: usize, to: usize) {
        self.current_dpi.move_row(from, to);
    }

    pub(super) fn current_dpi_unknown_number(&self, cx: &Context<Self>) -> AnyElement {
        div()
            .w(surface::css(62.))
            .h(surface::css(26.))
            .ml(surface::css(10.))
            .pl(surface::css(5.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .text_color(cx.theme().muted_foreground)
            .child("—")
            .into_any_element()
    }

    pub(super) fn current_dpi_unknown_track(&self, cx: &Context<Self>) -> AnyElement {
        div()
            .w(surface::css(250.))
            .h(surface::css(20.))
            .ml(surface::css(10.))
            .flex()
            .items_center()
            .child(div().w_full().h_px().bg(cx.theme().border))
            .into_any_element()
    }

    pub(super) fn current_dpi_status(&self, cx: &Context<Self>) -> Option<AnyElement> {
        razer_device::mouse_current_dpi::capability(self.spec.product_id)?;
        let error = self
            .current_dpi
            .read_error
            .iter()
            .map(String::as_str)
            .chain(self.current_dpi.observed.errors())
            .collect::<Vec<_>>()
            .join("; ");
        let loading = self.current_dpi.refresh_requested
            || self
                .current_dpi
                .pending
                .as_ref()
                .is_some_and(|request| request.read && request.change.is_none());
        let connected = self.current_dpi.connection_key.is_some();
        if error.is_empty()
            && !loading
            && connected
            && self.current_dpi.row_values.iter().any(Option::is_some)
        {
            return None;
        }
        let label = if !error.is_empty() {
            format!("DPI: {error}")
        } else if loading {
            t("LOADING")
        } else if !connected {
            t("DISCONNECTED")
        } else {
            "DPI: —".into()
        };
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .mb_2()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(label)
                .children((connected && !loading).then(|| {
                    Button::new("mouse-current-dpi-retry")
                        .ghost()
                        .small()
                        .label(t("RETRY"))
                        .on_click(cx.listener(|this, _, _, cx| this.request_current_dpi_read(cx)))
                }))
                .into_any_element(),
        )
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
        latest.connection_key = self.current_dpi.connection_key.clone();
        latest.valid = Arc::new(AtomicBool::new(true));
        self.current_dpi.pending = Some(latest.clone());
        Some(latest)
    }
}

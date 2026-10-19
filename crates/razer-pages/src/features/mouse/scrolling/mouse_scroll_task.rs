//! Current scroll reducer requests, serialized source profile saves and device
//! tasks. Task completion never overwrites the optimistic editor value.
use super::*;
use razer_device::mouse_scroll::{EnabledScrollModes, ScrollChange, ScrollMode, Submission};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct MouseScrollRequested {
    scope: MousePollingScope,
    sequence: u64,
    submission: Submission,
    profile: Value,
    source_events: u8,
    valid: Arc<AtomicBool>,
    saved: Arc<AtomicBool>,
    device_started: Arc<AtomicBool>,
}
impl MouseScrollRequested {
    pub fn submission(&self) -> &Submission {
        &self.submission
    }
    pub fn profile(&self) -> &Value {
        &self.profile
    }
    pub fn source_event_count(&self) -> u8 {
        self.source_events
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn is_saved(&self) -> bool {
        self.saved.load(Ordering::Acquire)
    }
    pub fn mark_saved(&self) {
        self.saved.store(true, Ordering::Release);
    }
    pub fn begin_device(&self) -> bool {
        self.is_valid()
            && self.is_saved()
            && self
                .device_started
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }
    pub fn cancel(&self) {
        self.valid.store(false, Ordering::Release);
    }
}

#[derive(Default)]
pub(super) struct State {
    sequence: u64,
    pending: Option<MouseScrollRequested>,
    queued: Option<MouseScrollRequested>,
    mode_debounce: Option<Task<()>>,
    initialized: Option<Initialization>,
    closed: bool,
}

#[derive(PartialEq, Eq)]
struct Initialization {
    scope: MousePollingScope,
    connection: String,
    guid: String,
}

impl State {
    pub(super) fn cancel(&mut self) {
        if let Some(pending) = &self.pending {
            pending.cancel();
        }
        if let Some(queued) = self.queued.take() {
            queued.cancel();
        }
        self.mode_debounce = None;
        self.initialized = None;
    }

    fn complete(
        &mut self,
        request: &MouseScrollRequested,
        scope: Option<MousePollingScope>,
        cleanup_succeeded: bool,
    ) -> Option<MouseScrollRequested> {
        if !self.pending.as_ref().is_some_and(|pending| {
            pending.sequence == request.sequence
                && pending.scope == request.scope
                && Arc::ptr_eq(&pending.valid, &request.valid)
        }) {
            return None;
        }
        if !cleanup_succeeded {
            self.cancel();
        }
        if let Some(previous) = self.pending.take() {
            previous.cancel();
        }
        let next = self.queued.take()?;
        if !next.is_valid() || scope != Some(next.scope) {
            next.cancel();
            return None;
        }
        self.pending = Some(next.clone());
        Some(next)
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl EventEmitter<MouseScrollRequested> for MouseProductWorkspace {}

fn mode(value: &Value) -> Option<ScrollMode> {
    match value.as_str()? {
        "Tactile" => Some(ScrollMode::Tactile),
        "FreeSpin" => Some(ScrollMode::FreeSpin),
        "MicroTactile" => Some(ScrollMode::MicroTactile),
        _ => None,
    }
}
fn changes(product_id: u32, previous: &Value, wheel: &Value) -> Option<Vec<ScrollChange>> {
    let cap = razer_device::mouse_scroll::capability(product_id)?;
    let changed = |key: &str| previous.get(key) != wheel.get(key);
    let mut changes = Vec::new();
    if changed("scrollMode") || changed("disabledModes") {
        let selected = mode(&wheel["scrollMode"])?;
        changes.push(if cap.supports_mode_v2() {
            let disabled = wheel["disabledModes"]
                .as_array()?
                .iter()
                .map(mode)
                .collect::<Option<Vec<_>>>()?;
            ScrollChange::ModeV2 {
                selected,
                enabled: EnabledScrollModes::from_disabled(&disabled).ok()?,
            }
        } else {
            ScrollChange::LegacyMode(selected)
        });
    }
    for (key, smart) in [("accelerationEnabled", false), ("smartReelEnabled", true)] {
        if changed(key) {
            let value = wheel[key].as_bool()?;
            changes.push(if smart {
                ScrollChange::SmartReel(value)
            } else {
                ScrollChange::VirtualAcceleration(value)
            });
        }
    }
    if cap.supports_levels() {
        for (key, smart) in [("accelerationLevel", false), ("smartReelLevel", true)] {
            if changed(key) {
                let value = u8::try_from(wheel[key].as_u64()?).ok()?;
                changes.push(if smart {
                    ScrollChange::SmartReelThreshold(value)
                } else {
                    ScrollChange::VirtualAccelerationLevel(value)
                });
            }
        }
    }
    Some(changes)
}
fn kind(change: ScrollChange) -> u8 {
    match change {
        ScrollChange::LegacyMode(_) | ScrollChange::ModeV2 { .. } => 0,
        ScrollChange::VirtualAcceleration(_) => 1,
        ScrollChange::SmartReel(_) => 2,
        ScrollChange::VirtualAccelerationLevel(_) => 3,
        ScrollChange::SmartReelThreshold(_) => 4,
    }
}
fn merge(previous: &mut Vec<ScrollChange>, next: &[ScrollChange]) {
    for change in next {
        previous.retain(|old| kind(*old) != kind(*change));
        previous.push(*change);
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoredWheel {
    scroll_mode: ScrollMode,
    acceleration_enabled: bool,
    smart_reel_enabled: bool,
    acceleration_level: Option<u8>,
    smart_reel_level: Option<u8>,
    #[serde(default)]
    disabled_modes: Vec<ScrollMode>,
}

fn restore_changes(product_id: u32, wheel: &Value) -> anyhow::Result<Vec<ScrollChange>> {
    let cap = razer_device::mouse_scroll::capability(product_id)
        .ok_or_else(|| anyhow::anyhow!("Current mouse scrolling capability is absent"))?;
    let wheel: RestoredWheel = serde_json::from_value(wheel.clone())?;
    let mut changes = Vec::new();
    if !cap.supports_mode_v2() {
        changes.push(ScrollChange::LegacyMode(wheel.scroll_mode));
    }
    changes.push(ScrollChange::SmartReel(wheel.smart_reel_enabled));
    if cap.supports_levels() {
        if let Some(level) = wheel.smart_reel_level {
            changes.push(ScrollChange::SmartReelThreshold(level));
        }
    }
    changes.push(ScrollChange::VirtualAcceleration(
        wheel.acceleration_enabled,
    ));
    if cap.supports_levels() {
        if let Some(level) = wheel.acceleration_level {
            changes.push(ScrollChange::VirtualAccelerationLevel(level));
        }
    }
    if cap.supports_mode_v2() {
        let enabled = EnabledScrollModes::from_disabled(&wheel.disabled_modes)?;
        // Generic restoreDeviceData runs after the ordinary wheel events.
        // ScrollModeV2Feature restores the mask and its resolved active mode.
        let selected = if enabled.contains(wheel.scroll_mode) {
            wheel.scroll_mode
        } else {
            [
                ScrollMode::Tactile,
                ScrollMode::FreeSpin,
                ScrollMode::MicroTactile,
            ]
            .into_iter()
            .find(|mode| enabled.contains(*mode))
            .ok_or_else(|| anyhow::anyhow!("All restored scroll modes are disabled"))?
        };
        changes.push(ScrollChange::ModeV2 { selected, enabled });
    }
    // Use the protocol owner to reject unsupported modes and out-of-range
    // levels before accepting any of the source restore events.
    Submission::new(changes.clone(), Default::default()).validate(product_id)?;
    Ok(changes)
}

impl MouseProductWorkspace {
    pub fn initialize_mouse_scroll_profile(
        &mut self,
        key: Option<&str>,
        guid: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = key else {
            self.scroll_task.cancel();
            return;
        };
        let Some(spec) = scroll_wheel::source_spec(self.spec.product_id) else {
            return;
        };
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if self.scroll_task.closed {
            return;
        }
        if guid.is_empty() {
            self.scroll_task.cancel();
            return;
        }
        let initialization = Initialization {
            scope,
            connection: key.to_owned(),
            guid: guid.to_owned(),
        };
        if self.scroll_task.initialized.as_ref() == Some(&initialization) {
            return;
        }
        if self.scroll_task.initialized.is_some() {
            // A new physical route can retain the same polling epochs. Its
            // predecessor still owns the writer slot until real cleanup.
            self.scroll_task.cancel();
        }
        let changes = match restore_changes(self.spec.product_id, &self.draft[&spec.profile_key]) {
            Ok(changes) => changes,
            Err(error) => {
                eprintln!("[mouse-scroll] Invalid restored wheel settings: {error:#}");
                return;
            }
        };
        if let Some(ScrollChange::ModeV2 { selected, .. }) = changes.last() {
            self.draft[&spec.profile_key]["scrollMode"] = json!(selected);
            if let Some(editor) = &self.scroll_editor {
                editor.update(cx, |editor, cx| editor.restore_source_mode(*selected, cx));
            }
        }
        self.scroll_task.initialized = Some(initialization);
        // Stored values already belong to this profile. Persist one atomic
        // snapshot with the same number of source version increments, then
        // execute its ordered independent tasks through the existing queue.
        // The generic V2 restore saves its resolved mask/mode without minting
        // a version; the ordinary wheel event makers each advance it once.
        let source_events = changes
            .iter()
            .filter(|change| !matches!(change, ScrollChange::ModeV2 { .. }))
            .count() as u8;
        self.enqueue_scroll_for_profile(changes, guid, source_events, cx);
    }

    pub(super) fn submit_scroll_edit(&mut self, previous: &Value, cx: &mut Context<Self>) {
        let Some(spec) = scroll_wheel::source_spec(self.spec.product_id) else {
            return;
        };
        let Some(changes) = changes(
            self.spec.product_id,
            previous,
            &self.draft[&spec.profile_key],
        ) else {
            return;
        };
        if changes.is_empty() {
            return;
        }
        // Current UI `fe` owns its 300ms scroll-mode debounce. Other controls
        // retain their immediate switch / slider-release dispatch semantics.
        if previous.get("scrollMode") != self.draft[&spec.profile_key].get("scrollMode")
            && previous.get("disabledModes") == self.draft[&spec.profile_key].get("disabledModes")
        {
            let Some(scope) = self.mouse_polling_scope(cx) else {
                return;
            };
            let previous = previous.clone();
            self.scroll_task.mode_debounce = Some(cx.spawn(async move |owner, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(300))
                    .await;
                let _ = owner.update(cx, |owner, cx| {
                    if owner.mouse_polling_scope(cx) != Some(scope) {
                        return;
                    }
                    let Some(spec) = scroll_wheel::source_spec(owner.spec.product_id) else {
                        return;
                    };
                    let Some(changes) = changes_for_mode(
                        owner.spec.product_id,
                        &previous,
                        &owner.draft[&spec.profile_key],
                    ) else {
                        return;
                    };
                    owner.enqueue_scroll(changes, cx);
                });
            }));
        } else {
            if changes.iter().any(|change| kind(*change) == 0) {
                self.scroll_task.mode_debounce = None;
            }
            self.enqueue_scroll(changes, cx);
        }
    }
    fn enqueue_scroll(&mut self, changes: Vec<ScrollChange>, cx: &mut Context<Self>) {
        let Some(guid) = self
            .scroll_task
            .initialized
            .as_ref()
            .map(|initialized| initialized.guid.as_str())
            .or_else(|| self.draft.get("guid").and_then(Value::as_str))
        else {
            return;
        };
        let guid = guid.to_owned();
        self.enqueue_scroll_for_profile(changes, &guid, 1, cx);
    }

    fn enqueue_scroll_for_profile(
        &mut self,
        changes: Vec<ScrollChange>,
        guid: &str,
        source_events: u8,
        cx: &mut Context<Self>,
    ) {
        if changes.is_empty() || self.scroll_task.closed {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let selected = self
            .draft
            .get("obmSlotId")
            .and_then(Value::as_array)
            .and_then(|slots| slots.first())
            .and_then(Value::as_u64)
            .and_then(|slot| u8::try_from(slot).ok());
        let mut submission = Submission::for_profile(changes, guid.to_owned(), selected);
        if let Some(previous) = self.scroll_task.queued.take() {
            if previous.scope == scope {
                let mut pending = previous.submission.changes.clone();
                merge(&mut pending, &submission.changes);
                submission.changes = pending;
            }
            previous.cancel();
        }
        if submission.validate(self.spec.product_id).is_err() {
            return;
        }
        self.scroll_task.sequence = self.scroll_task.sequence.wrapping_add(1);
        let request = MouseScrollRequested {
            scope,
            sequence: self.scroll_task.sequence,
            submission,
            profile: self.draft.clone(),
            source_events,
            valid: Arc::new(AtomicBool::new(true)),
            saved: Arc::new(AtomicBool::new(false)),
            device_started: Arc::new(AtomicBool::new(false)),
        };
        if self.scroll_task.pending.is_some() {
            self.scroll_task.queued = Some(request.clone());
        } else {
            self.scroll_task.pending = Some(request.clone());
        }
        // Every accepted reducer edit reaches durable storage even while an
        // older hardware operation is in flight. Only device tasks coalesce.
        cx.emit(request);
    }
    pub fn mouse_scroll_save_matches(&self, request: &MouseScrollRequested, cx: &App) -> bool {
        request.is_valid()
            && self.mouse_polling_scope(cx) == Some(request.scope)
            && [&self.scroll_task.pending, &self.scroll_task.queued]
                .into_iter()
                .any(|slot| {
                    slot.as_ref().is_some_and(|slot| {
                        slot.sequence == request.sequence
                            && slot.scope == request.scope
                            && Arc::ptr_eq(&slot.valid, &request.valid)
                    })
                })
    }
    pub fn mouse_scroll_request_matches(&self, request: &MouseScrollRequested, cx: &App) -> bool {
        request.is_valid()
            && self.mouse_polling_scope(cx) == Some(request.scope)
            && self.scroll_task.pending.as_ref().is_some_and(|pending| {
                pending.sequence == request.sequence
                    && pending.scope == request.scope
                    && Arc::ptr_eq(&pending.valid, &request.valid)
            })
    }
    pub fn finish_mouse_scroll(&mut self, request: &MouseScrollRequested, cx: &mut Context<Self>) {
        self.finish_mouse_scroll_cleanup(request, true, cx);
    }

    pub fn finish_mouse_scroll_report(
        &mut self,
        request: &MouseScrollRequested,
        report: Option<&razer_device::mouse_scroll::SubmissionReport>,
        cleanup_succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        if self.mouse_scroll_request_matches(request, cx)
            && let Some(report) = report.filter(|report| {
                report.requested == request.submission
                    && report.validate_for(self.spec.product_id).is_ok()
            })
            && let Some(editor) = &self.scroll_editor
        {
            editor.update(cx, |editor, cx| editor.record_submission(report, cx));
        }
        self.finish_mouse_scroll_cleanup(request, cleanup_succeeded, cx);
    }

    fn finish_mouse_scroll_cleanup(
        &mut self,
        request: &MouseScrollRequested,
        cleanup_succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        let scope = self.mouse_polling_scope(cx);
        if let Some(next) = self.scroll_task.complete(request, scope, cleanup_succeeded)
            && next.is_saved()
        {
            // Its first event already queued a local save. If that save has
            // not finished, its completion will start this task.
            cx.emit(next);
        }
        cx.notify();
    }

    pub fn sync_mouse_scroll_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &self.scroll_editor {
            editor.update(cx, |editor, cx| editor.sync_observations(window, cx));
        }
    }
    pub fn prepare_mouse_scroll_exit(&mut self) {
        // All accepted edits have already reached the shared save queue. Exit
        // drains that queue and stops hardware work without duplicating saves.
        self.scroll_task.cancel();
        self.scroll_task.pending = None;
        self.scroll_task.closed = true;
    }

    pub fn resume_mouse_scroll_after_cancelled_exit(&mut self) {
        self.scroll_task.closed = false;
    }
}
fn changes_for_mode(product: u32, previous: &Value, wheel: &Value) -> Option<Vec<ScrollChange>> {
    Some(
        changes(product, previous, wheel)?
            .into_iter()
            .filter(|change| kind(*change) == 0)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn v2_disabling_the_selected_mode_keeps_mask_before_resolved_mode() {
        let before = json!({"scrollMode":"FreeSpin","disabledModes":[]});
        let after = json!({"scrollMode":"Tactile","disabledModes":["FreeSpin"]});
        assert_eq!(
            changes(226, &before, &after).unwrap(),
            vec![ScrollChange::ModeV2 {
                selected: ScrollMode::Tactile,
                enabled: EnabledScrollModes::new(5).unwrap()
            }]
        );
    }
    #[::core::prelude::v1::test]
    fn queue_keeps_unrelated_switches_and_only_the_newest_value_per_task() {
        let mut queued = vec![
            ScrollChange::SmartReel(true),
            ScrollChange::VirtualAccelerationLevel(2),
        ];
        merge(
            &mut queued,
            &[
                ScrollChange::VirtualAccelerationLevel(7),
                ScrollChange::LegacyMode(ScrollMode::Tactile),
            ],
        );
        assert_eq!(
            queued,
            vec![
                ScrollChange::SmartReel(true),
                ScrollChange::VirtualAccelerationLevel(7),
                ScrollChange::LegacyMode(ScrollMode::Tactile)
            ]
        );
    }
    #[::core::prelude::v1::test]
    fn restore_keeps_source_event_order_and_v2_generic_mode_restore_last() {
        let wheel = json!({"scrollMode":"FreeSpin", "disabledModes":["FreeSpin"],
            "smartReelEnabled":true, "smartReelLevel":4,
            "accelerationEnabled":false, "accelerationLevel":7});
        for product_id in [153, 170, 203, 204, 214, 45066] {
            let mut expected = vec![
                ScrollChange::LegacyMode(ScrollMode::FreeSpin),
                ScrollChange::SmartReel(true),
            ];
            if !matches!(product_id, 153 | 170) {
                expected.push(ScrollChange::SmartReelThreshold(4));
            }
            expected.push(ScrollChange::VirtualAcceleration(false));
            if !matches!(product_id, 153 | 170) {
                expected.push(ScrollChange::VirtualAccelerationLevel(7));
            }
            assert_eq!(restore_changes(product_id, &wheel).unwrap(), expected);
        }
        assert_eq!(
            restore_changes(226, &wheel).unwrap(),
            vec![
                ScrollChange::SmartReel(true),
                ScrollChange::SmartReelThreshold(4),
                ScrollChange::VirtualAcceleration(false),
                ScrollChange::VirtualAccelerationLevel(7),
                ScrollChange::ModeV2 {
                    selected: ScrollMode::Tactile,
                    enabled: EnabledScrollModes::new(5).unwrap()
                },
            ]
        );
    }
    #[::core::prelude::v1::test]
    fn restore_rejects_malformed_source_values_and_does_not_invent_missing_levels() {
        let base = json!({"scrollMode":"Tactile", "smartReelEnabled":false,
            "accelerationEnabled":true});
        assert_eq!(restore_changes(203, &base).unwrap().len(), 3);
        for (key, value) in [
            ("scrollMode", json!("Unknown")),
            ("smartReelEnabled", json!(1)),
            ("accelerationEnabled", Value::Null),
            ("smartReelLevel", json!(256)),
            ("accelerationLevel", json!(-1)),
        ] {
            let mut wheel = base.clone();
            wheel[key] = value;
            assert!(restore_changes(203, &wheel).is_err());
        }
    }
    #[::core::prelude::v1::test]
    fn initialization_identity_contains_owner_profile_and_connection_not_only_route_text() {
        let identity = |owner, profile, connection| Initialization {
            scope: MousePollingScope::new(EntityId::from(owner), profile, connection),
            connection: "same-route".into(),
            guid: "same-guid".into(),
        };
        assert!(identity(1, 0, 0) != identity(2, 0, 0));
        assert!(identity(1, 0, 0) != identity(1, 1, 0));
        assert!(identity(1, 0, 0) != identity(1, 0, 1));
        let mut state = State::default();
        state.initialized = Some(identity(1, 0, 0));
        state.cancel();
        assert!(state.initialized.is_none());
    }
    fn request(sequence: u64) -> MouseScrollRequested {
        MouseScrollRequested {
            scope: MousePollingScope::new(EntityId::from(1), 2, 3),
            sequence,
            submission: Submission::new(vec![ScrollChange::SmartReel(true)], Default::default()),
            profile: json!({}),
            source_events: 1,
            valid: Arc::new(AtomicBool::new(true)),
            saved: Arc::new(AtomicBool::new(false)),
            device_started: Arc::new(AtomicBool::new(false)),
        }
    }
    #[::core::prelude::v1::test]
    fn exit_and_profile_cancellation_stop_both_device_intents_without_undoing_durable_saves() {
        let pending = request(1);
        let queued = request(2);
        queued.mark_saved();
        let mut state = State::default();
        state.pending = Some(pending.clone());
        state.queued = Some(queued.clone());
        state.cancel();
        assert!(!pending.is_valid());
        assert!(!queued.is_valid());
        assert!(queued.is_saved());
        assert!(!pending.is_saved());
        assert!(state.queued.is_none());
    }
    #[::core::prelude::v1::test]
    fn repeated_local_save_completion_cannot_start_a_second_device_worker() {
        let request = request(1);
        let callback = request.clone();
        request.mark_saved();
        assert!(callback.is_saved());
        assert!(request.begin_device());
        assert!(!callback.begin_device());
        callback.cancel();
        assert!(!request.is_valid());
    }

    #[::core::prelude::v1::test]
    fn mouse_scroll_cleanup_failure_cancels_successor_without_undoing_its_durable_save() {
        let pending = request(1);
        let queued = request(2);
        queued.mark_saved();
        let mut state = State::default();
        state.pending = Some(pending.clone());
        state.queued = Some(queued.clone());
        assert!(
            state
                .complete(&pending, Some(pending.scope), false)
                .is_none()
        );
        assert!(!pending.is_valid());
        assert!(!queued.is_valid());
        assert!(queued.is_saved());
        assert!(state.pending.is_none());
        assert!(state.queued.is_none());
    }

    #[::core::prelude::v1::test]
    fn mouse_scroll_late_completion_cannot_release_a_reused_sequence_from_another_owner() {
        let stale = request(1);
        let current = request(1);
        let queued = request(2);
        let mut state = State::default();
        state.pending = Some(current.clone());
        state.queued = Some(queued.clone());
        assert!(state.complete(&stale, Some(stale.scope), false).is_none());
        assert!(current.is_valid());
        assert!(queued.is_valid());
        assert!(Arc::ptr_eq(
            &state.pending.as_ref().unwrap().valid,
            &current.valid
        ));
        assert!(Arc::ptr_eq(
            &state.queued.as_ref().unwrap().valid,
            &queued.valid
        ));
    }

    #[::core::prelude::v1::test]
    fn mouse_scroll_cleanup_releases_successor_only_for_the_current_scope() {
        for same_scope in [false, true] {
            let pending = request(1);
            let queued = request(2);
            queued.mark_saved();
            let mut state = State::default();
            state.pending = Some(pending.clone());
            state.queued = Some(queued.clone());
            let next = state.complete(&pending, same_scope.then_some(pending.scope), true);
            assert_eq!(next.is_some(), same_scope);
            assert!(!pending.is_valid());
            assert_eq!(queued.is_valid(), same_scope);
            assert_eq!(state.pending.is_some(), same_scope);
        }
    }
}

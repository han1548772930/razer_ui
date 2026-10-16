//! Current scroll reducer requests, serialized source profile saves and device
//! tasks. Task completion never overwrites the optimistic editor value.
use super::*;
use razer_device::mouse_scroll::{
    EnabledScrollModes, ScrollChange, ScrollMode, ScrollProfileObservation, Submission,
};
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

impl MouseProductWorkspace {
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
        if changes.is_empty() {
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
        let profiles = ScrollProfileObservation {
            selected_obm_profile_id: selected,
            ..Default::default()
        };
        let mut submission = Submission::new(changes, profiles);
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
                        slot.sequence == request.sequence && slot.scope == request.scope
                    })
                })
    }
    pub fn mouse_scroll_request_matches(&self, request: &MouseScrollRequested, cx: &App) -> bool {
        request.is_valid()
            && self.mouse_polling_scope(cx) == Some(request.scope)
            && self.scroll_task.pending.as_ref().is_some_and(|pending| {
                pending.sequence == request.sequence && pending.scope == request.scope
            })
    }
    pub fn finish_mouse_scroll(&mut self, request: &MouseScrollRequested, cx: &mut Context<Self>) {
        if !self.scroll_task.pending.as_ref().is_some_and(|pending| {
            pending.sequence == request.sequence && pending.scope == request.scope
        }) {
            return;
        }
        if let Some(previous) = self.scroll_task.pending.take() {
            previous.cancel();
        }
        if let Some(next) = self.scroll_task.queued.take() {
            if next.is_valid() && self.mouse_polling_scope(cx) == Some(next.scope) {
                self.scroll_task.pending = Some(next.clone());
                // Its first event already queued a local save. If that save
                // has not finished, its completion will start this task.
                if next.is_saved() {
                    cx.emit(next);
                }
            }
        }
        cx.notify();
    }
    pub fn prepare_mouse_scroll_exit(&mut self) {
        // All accepted edits have already reached the shared save queue. Exit
        // drains that queue and stops hardware work without duplicating saves.
        self.scroll_task.cancel();
        self.scroll_task.pending = None;
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
    fn request(sequence: u64) -> MouseScrollRequested {
        MouseScrollRequested {
            scope: MousePollingScope::new(EntityId::from(1), 2, 3),
            sequence,
            submission: Submission::new(vec![ScrollChange::SmartReel(true)], Default::default()),
            profile: json!({}),
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
}

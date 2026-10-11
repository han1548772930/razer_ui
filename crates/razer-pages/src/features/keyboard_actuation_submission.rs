//! Exact UI middleware intents. No guessed HID packet or simulated success.
use super::*;

#[derive(Clone)]
pub struct KeyboardActuationRequested {
    generation: u64,
    payload: Value,
}
impl KeyboardActuationRequested {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Current source BroadcastChannel message, including its event type.
    pub fn payload(&self) -> &Value {
        &self.payload
    }
}
#[derive(Default)]
pub(super) struct State {
    generation: u64,
    // The inline Sync reducer (Qa/ka) emits ON_SET_KEYMAPPING then
    // ON_SYNC_GLOBAL_ACTUATION. Each request
    // retains its own identity; emitting the second cannot invalidate the
    // first callback. This bookkeeping is a Rust ownership safeguard, not a
    // recovered vendor parameter or a synthetic completion.
    pending: std::collections::BTreeSet<u64>,
    error: Option<String>,
}
impl State {
    fn begin(&mut self) -> u64 {
        if self.pending.is_empty() {
            self.error = None;
        }
        loop {
            self.generation = self.generation.wrapping_add(1);
            if self.pending.insert(self.generation) {
                return self.generation;
            }
        }
    }
    fn finish(&mut self, generation: u64, result: Result<(), String>) -> bool {
        if !self.pending.remove(&generation) {
            return false;
        }
        // A later success from the same outstanding group must not erase a
        // genuine failure of its earlier mapping submission.
        if let Err(error) = result {
            self.error = Some(error);
        }
        true
    }
}
impl EventEmitter<KeyboardActuationRequested> for KeyboardProductWorkspace {}
impl KeyboardProductWorkspace {
    pub(super) fn request_actuation_mapping(&mut self, cx: &mut Context<Self>) {
        let Some(mapping_list) = self.draft.pointer(&self.mapping_path()).cloned() else {
            return;
        };
        self.request_actuation_message(
            json!({
                "type":"ON_SET_KEYMAPPING", "payload":{"mappingList":mapping_list}
            }),
            cx,
        );
    }
    pub(super) fn request_actuation_message(&mut self, payload: Value, cx: &mut Context<Self>) {
        let state = &mut self.actuation_submission;
        let generation = state.begin();
        cx.emit(KeyboardActuationRequested {
            generation,
            payload,
        });
    }
    pub fn actuation_request_current(&self, generation: u64) -> bool {
        self.actuation_submission.pending.contains(&generation)
    }
    pub fn cancel_actuation_connection(&mut self) {
        self.actuation_submission.generation = self.actuation_submission.generation.wrapping_add(1);
        self.actuation_submission.pending.clear();
        self.actuation_submission.error = None;
    }
    pub fn finish_actuation(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if !self.actuation_submission.finish(generation, result) {
            return;
        }
        cx.notify();
    }
    pub fn actuation_submission_error(&self) -> Option<&str> {
        self.actuation_submission.error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::State;

    #[test]
    fn sync_keeps_both_requests_and_first_failure_after_second_success() {
        let mut state = State::default();
        let mapping = state.begin();
        let sync = state.begin();
        assert!(state.pending.contains(&mapping));
        assert!(state.pending.contains(&sync));
        assert!(state.finish(mapping, Err("mapping rejected".into())));
        assert!(state.finish(sync, Ok(())));
        assert_eq!(state.error.as_deref(), Some("mapping rejected"));
        assert!(state.pending.is_empty());
        let next = state.begin();
        assert!(state.error.is_none());
        assert!(!state.finish(mapping, Ok(())));
        assert!(state.pending.contains(&next));
    }

    #[test]
    fn cancelled_and_out_of_order_callbacks_do_not_complete_other_operations() {
        let mut state = State::default();
        let first = state.begin();
        let second = state.begin();
        assert!(state.finish(second, Ok(())));
        assert!(state.pending.contains(&first));
        state.pending.clear();
        let third = state.begin();
        assert!(!state.finish(first, Err("late error".into())));
        assert!(state.pending.contains(&third));
        assert!(state.error.is_none());
    }
}

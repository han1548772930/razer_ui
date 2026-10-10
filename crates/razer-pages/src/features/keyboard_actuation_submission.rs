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
    pending: Option<u64>,
    error: Option<String>,
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
        state.generation = state.generation.wrapping_add(1);
        state.pending = Some(state.generation);
        state.error = None;
        cx.emit(KeyboardActuationRequested {
            generation: state.generation,
            payload,
        });
    }
    pub fn actuation_request_current(&self, generation: u64) -> bool {
        self.actuation_submission.pending == Some(generation)
    }
    pub fn cancel_actuation_connection(&mut self) {
        self.actuation_submission.generation = self.actuation_submission.generation.wrapping_add(1);
        self.actuation_submission.pending = None;
        self.actuation_submission.error = None;
    }
    pub fn finish_actuation(
        &mut self,
        generation: u64,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if !self.actuation_request_current(generation) {
            return;
        }
        self.actuation_submission.pending = None;
        self.actuation_submission.error = result.err();
        cx.notify();
    }
    pub fn actuation_submission_error(&self) -> Option<&str> {
        self.actuation_submission.error.as_deref()
    }
}

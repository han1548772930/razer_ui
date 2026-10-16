//! Exact UI middleware intents. No guessed HID packet or simulated success.
use super::*;

#[derive(Clone)]
pub struct KeyboardActuationRequested {
    generation: u64,
    payload: Value,
    profile: Value,
}
impl KeyboardActuationRequested {
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Current source BroadcastChannel message, including its event type.
    pub fn payload(&self) -> &Value {
        &self.payload
    }
    /// Local profile at the time this source message was requested.
    pub fn profile(&self) -> &Value {
        &self.profile
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
        self.refresh_gaming_fn_sleep(cx);
    }
    pub(super) fn request_actuation_message(&mut self, payload: Value, cx: &mut Context<Self>) {
        if self.reduce_latest_source_action(&payload, cx) {
            return;
        }
        let profile = self.snapshot();
        let state = &mut self.actuation_submission;
        let generation = state.begin();
        cx.emit(KeyboardActuationRequested {
            generation,
            payload,
            profile,
        });
    }
    /// 756 local source reducers publish through its retained latest restore
    /// owner. They must not also enter the older submission protocol.
    fn reduce_latest_source_action(&mut self, message: &Value, cx: &mut Context<Self>) -> bool {
        if self.spec.product_id != 756 {
            return false;
        }
        let payload = &message["payload"];
        match message["type"].as_str() {
            Some("ON_SET_GAME_MODE") => {
                self.set_latest_source_game_mode(&payload["gamingMode"], true, cx)
            }
            Some("ON_SET_KEYMAPPING") => {
                let path = self.mapping_path();
                if let (Some(rows), Some(target)) = (
                    payload["mappingList"].as_array(),
                    self.draft.pointer_mut(&path),
                ) {
                    *target = json!(rows);
                } else {
                    self.actuation_submission.error =
                        Some("当前按键映射列表或活动映射路径无效".into());
                }
            }
            Some("ON_TOGGLE_CONTINUOUS_RAPID_TRIGGER") => {
                self.draft["continuousRapidTrigger"] = payload["value"].clone();
                if !self.draft["analogData"].is_object() {
                    self.draft["analogData"] = json!({});
                }
                if !self.draft["analogData"]["rapidTrigger"].is_object() {
                    self.draft["analogData"]["rapidTrigger"] = json!({});
                }
                self.draft["analogData"]["rapidTrigger"]["continuous"] = payload["value"].clone();
            }
            Some("ON_SET_ENABLE_KEYBOARD_KEYS") => {
                self.draft["isEnableKeyboardKeys"] = payload["isEnableKeyboardKeys"].clone()
            }
            Some("ON_SET_GAMEPAD_OVERRIDE_KEYS") => {
                self.draft["isGamepadOverride"] = payload["isGamepadOverride"].clone()
            }
            Some("ON_SET_BOTTOM_DEADZONE") => {
                self.draft["bottomDeadzone"] = payload["bottomDeadzone"].clone()
            }
            Some("ON_SET_INTERRUPT_KEYS") => {
                if self.draft["interruptKey"].is_object() {
                    self.draft["interruptKey"]["isEnabled"] = payload["isEnabled"].clone();
                    self.draft["interruptKey"]["interruptionKey"] =
                        payload["interruptionKey"].clone();
                    if let Some(keys) = payload["interrupts"].as_array() {
                        if !keys.iter().any(|key| key == "KEY_ANY") {
                            let mut unique = Vec::new();
                            for key in keys {
                                if !unique.contains(key) {
                                    unique.push(key.clone());
                                }
                            }
                            self.draft["interruptKey"]["interrupts"] = json!(unique);
                        }
                    }
                }
            }
            Some("ON_TOGGLE_SINGLE_KEY_SNAP_TAP") => {
                if !self.draft["singleKeySnapTap"].is_object() {
                    self.draft["singleKeySnapTap"] = json!({"keyList":[]});
                }
                if payload["isEnabled"].is_boolean() {
                    self.draft["singleKeySnapTap"]["isEnabled"] = payload["isEnabled"].clone();
                }
            }
            Some("ON_SET_GAME_MODE_COMBINATION_KEYS") => {
                self.set_latest_custom_game_mode(payload);
            }
            Some("ON_SET_SNAP_TAP") => {
                if let Err(error) =
                    razer_device::keyboard_submission::persist_local(message, &mut self.draft)
                {
                    self.actuation_submission.error = Some(format!("{error:#}"));
                }
            }
            _ => return false,
        }
        cx.emit(KeyboardProductChanged);
        cx.notify();
        true
    }
    pub fn actuation_request_current(&self, generation: u64) -> bool {
        self.actuation_submission.pending.contains(&generation)
    }
    pub fn cancel_actuation_connection(&mut self) {
        self.analog_controls.cancel();
        self.gaming_combined.cancel();
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

    #[::core::prelude::v1::test]
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

    #[::core::prelude::v1::test]
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

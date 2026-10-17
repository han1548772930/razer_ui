//! Current optimistic Redux controls -> local profile arrays -> ordered device
//! intent. Storage/device failures are retained separately from source UI state.
use super::*;
use razer_device::controller_triggers::{TriggerOperation, TriggerProfile};
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone)]
pub struct ControllerTriggerRequested {
    sequence: u64,
    revision: u64,
    operation: TriggerOperation,
    profile: Value,
    guard: Arc<AtomicBool>,
}
impl ControllerTriggerRequested {
    pub fn operation(&self) -> &TriggerOperation {
        &self.operation
    }
    pub fn profile(&self) -> &Value {
        &self.profile
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.guard)
    }
    pub fn is_valid(&self) -> bool {
        self.guard.load(Ordering::Acquire)
    }
}

pub(super) struct TriggerRuntime {
    guard: Arc<AtomicBool>,
    sequence: u64,
    revision: u64,
    active: Option<u64>,
    pending: VecDeque<ControllerTriggerRequested>,
    read_requested: bool,
    thumbsticks_read_requested: bool,
    last_result: Option<Value>,
}
impl Default for TriggerRuntime {
    fn default() -> Self {
        Self {
            guard: Arc::new(AtomicBool::new(true)),
            sequence: 0,
            revision: 0,
            active: None,
            pending: VecDeque::new(),
            read_requested: false,
            thumbsticks_read_requested: false,
            last_result: None,
        }
    }
}
impl Drop for TriggerRuntime {
    fn drop(&mut self) {
        self.guard.store(false, Ordering::Release);
    }
}
impl TriggerRuntime {
    pub(super) fn invalidate(&mut self) {
        self.guard.store(false, Ordering::Release);
        self.guard = Arc::new(AtomicBool::new(true));
        self.active = None;
        self.pending.clear();
        self.read_requested = false;
        self.thumbsticks_read_requested = false;
        self.last_result = None;
        self.revision = self.revision.wrapping_add(1);
    }
    fn enqueue(&mut self, operation: TriggerOperation, profile: Value) {
        self.sequence = self.sequence.wrapping_add(1);
        if !operation.is_read() {
            self.revision = self.revision.wrapping_add(1);
        }
        self.pending.push_back(ControllerTriggerRequested {
            sequence: self.sequence,
            revision: self.revision,
            operation,
            profile,
            guard: Arc::clone(&self.guard),
        });
    }
    fn next(&mut self) -> Option<ControllerTriggerRequested> {
        if self.active.is_some() {
            return None;
        }
        let request = self.pending.pop_front()?;
        self.active = Some(request.sequence);
        Some(request)
    }
    fn matches(&self, request: &ControllerTriggerRequested) -> bool {
        request.is_valid()
            && Arc::ptr_eq(&request.guard, &self.guard)
            && self.active == Some(request.sequence)
    }
}

fn profile_slot(draft: &Value) -> anyhow::Result<u8> {
    // Current obmUtilitiesController.Fk: first obmSlotId, fallback0; a controller
    // maps zero to slot1. Preserve an actual nonzero source slot when available.
    let slot = draft
        .pointer("/profile/obmSlotId/0")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Ok(u8::try_from(slot.max(1))?)
}

fn prepare_change(
    draft: &mut Value,
    root: &str,
    analog_mode: i64,
) -> anyhow::Result<TriggerOperation> {
    let controller_id = match root {
        "/controller/leftTrigger" => 1,
        "/controller/rightTrigger" => 2,
        _ => anyhow::bail!("Unknown controller trigger side"),
    };
    let settings = draft
        .pointer(root)
        .ok_or_else(|| anyhow::anyhow!("Missing trigger controls"))?
        .clone();
    let analog = settings["mode"].as_i64() == Some(analog_mode);
    let (key, array, other, other_array, selected) = if analog {
        (
            "controllerAnalogActuation",
            "analogActuations",
            "controllerDigitalActuation",
            "digitalActuations",
            json!({"controllerId":controller_id,"startRange":settings["startRange"],"endRange":settings["endRange"],"isActive":true}),
        )
    } else {
        (
            "controllerDigitalActuation",
            "digitalActuations",
            "controllerAnalogActuation",
            "analogActuations",
            json!({"controllerId":controller_id,"actuationPoint":settings["actuationPoint"],"isRapidTrigger":settings["isRapidTrigger"],"isActive":true}),
        )
    };
    let profile = draft
        .get_mut("profile")
        .filter(|p| p.is_object())
        .ok_or_else(|| anyhow::anyhow!("Active controller profile missing"))?;
    if profile[key][array].is_null() {
        profile[key] = json!({array:[]});
    }
    let entries = profile[key][array]
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("Invalid controller trigger profile array"))?;
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| entry["controllerId"] == controller_id)
    {
        *entry = selected;
    } else {
        entries.push(selected);
    }
    if let Some(entries) = profile[other][other_array].as_array_mut() {
        for entry in entries
            .iter_mut()
            .filter(|entry| entry["controllerId"] == controller_id)
        {
            entry["isActive"] = json!(false);
        }
    }
    let operation = if analog {
        TriggerOperation::Analog {
            profile_slot: profile_slot(draft)?,
            settings: serde_json::from_value(draft["profile"][key][array].clone())?,
        }
    } else {
        TriggerOperation::Digital {
            profile_slot: profile_slot(draft)?,
            settings: serde_json::from_value(draft["profile"][key][array].clone())?,
        }
    };
    operation.validate()?;
    Ok(operation)
}

fn prepare_thumbstick_change(
    draft: &mut Value,
    path: &str,
    circular_mode: i64,
    legacy: bool,
) -> anyhow::Result<TriggerOperation> {
    let profile_slot = profile_slot(draft)?;
    if path == "/controller/circularity/mode" {
        let circular = draft.pointer(path).and_then(Value::as_i64) == Some(circular_mode);
        draft["profile"]["controllerAnalogCircularity"] =
            json!(if circular { "circular" } else { "standard" });
        return Ok(TriggerOperation::Circularity {
            profile_slot,
            circular,
        });
    }
    let (side, controller_id) = if path.starts_with("/controller/leftStick/") {
        ("leftStick", 1)
    } else if path.starts_with("/controller/rightStick/") {
        ("rightStick", 2)
    } else {
        anyhow::bail!("Unknown controller thumbstick field");
    };
    let sensitivity = path.ends_with("/sensitivityValue");
    let (key, array, field) = if sensitivity {
        (
            "controllerSensitivity",
            "sensitivityValues",
            "sensitivityValue",
        )
    } else {
        ("controllerDeadzone", "deadzones", "deadzoneValue")
    };
    let value = draft["controller"][side][field].clone();
    let prevention_side = if legacy && path.ends_with("/deadzoneValue") {
        "leftStick"
    } else {
        side
    };
    let prevent = draft["controller"][prevention_side]["preventDoubleDeadzone"]
        .as_bool()
        .ok_or_else(|| anyhow::anyhow!("Missing controller deadzone flag"))?;
    if !sensitivity {
        draft["controller"][side]["preventDoubleDeadzone"] = json!(prevent);
    }
    let target = &mut draft["profile"][key];
    if !target.is_object() {
        *target = json!({array:[]});
    }
    if target[array].is_null() {
        target[array] = json!([]);
    }
    let entries = target[array]
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("Invalid controller thumbstick profile array"))?;
    let selected = json!({"controllerId":controller_id,"value":value});
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| entry["controllerId"] == controller_id)
    {
        *entry = selected;
    } else {
        entries.push(selected);
    }
    let settings = serde_json::from_value(target[array].clone())?;
    let operation = if sensitivity {
        TriggerOperation::ThumbstickSensitivity {
            profile_slot,
            settings,
        }
    } else {
        target["preventDoubleDeadzone"] = json!(prevent);
        TriggerOperation::ThumbstickDeadzone {
            profile_slot,
            settings,
            prevent_double_deadzone: prevent,
        }
    };
    operation.validate()?;
    Ok(operation)
}

fn project_thumbstick_profile(
    draft: &mut Value,
    profile: &Value,
    standard_mode: i64,
    circular_mode: i64,
) {
    for (key, array, field) in [
        ("controllerDeadzone", "deadzones", "deadzoneValue"),
        (
            "controllerSensitivity",
            "sensitivityValues",
            "sensitivityValue",
        ),
    ] {
        if let Some(entries) = profile[key][array].as_array() {
            draft["profile"][key] = profile[key].clone();
            for entry in entries {
                let side = match entry["controllerId"].as_u64() {
                    Some(1) => "leftStick",
                    Some(2) => "rightStick",
                    _ => continue,
                };
                draft["controller"][side][field] = entry["value"].clone();
                if key == "controllerDeadzone" {
                    draft["controller"][side]["preventDoubleDeadzone"] =
                        profile[key]["preventDoubleDeadzone"].clone();
                }
            }
        }
    }
    if let Some(circular) = profile["controllerAnalogCircularity"].as_str() {
        draft["profile"]["controllerAnalogCircularity"] = json!(circular);
        draft["controller"]["circularity"]["mode"] = json!(if circular == "standard" {
            standard_mode
        } else {
            circular_mode
        });
    }
}

impl GamepadProductWorkspace {
    pub fn observe_controller_connection(&mut self, connected: bool, cx: &mut Context<Self>) {
        self.trigger_runtime.invalidate();
        self.lighting_runtime.invalidate();
        self.chroma_runtime.invalidate();
        self.lighting_ui.cancel(cx);
        self.dismiss_quick_lighting_controls(cx);
        self.cancel_thumbstick_edits(cx);
        self.observe_controller_input(None, &[], cx);
        if connected {
            self.request_trigger_read(cx);
            self.request_controller_lighting(cx);
            if self.page == "TAB_LIGHTING" {
                self.refresh_controller_chroma(cx);
            }
        }
    }
    pub fn observe_controller_input(
        &mut self,
        sample: Option<&razer_device::controller_input::Sample>,
        events: &[razer_device::controller_input::Event],
        cx: &mut Context<Self>,
    ) {
        if !events.is_empty() {
            for event in events {
                if self.controller_notifications.len() == 512 {
                    self.controller_notifications.pop_front();
                }
                self.controller_notifications.push_back(event.clone());
            }
            return;
        }
        if sample.is_none() {
            self.controller_notifications.clear();
        }
        if self.controller_input_sample.as_ref() != sample {
            self.controller_input_sample = sample.cloned();
            self.refresh_trigger_visualization(cx);
            cx.notify();
        }
    }
    pub(super) fn trigger_changed(&mut self, root: &str, cx: &mut Context<Self>) {
        self.refresh_trigger_visualization(cx);
        match prepare_change(&mut self.draft, root, self.spec.analog_mode) {
            Ok(operation) => self
                .trigger_runtime
                .enqueue(operation, self.draft["profile"].clone()),
            Err(error) => {
                self.trigger_runtime.last_result =
                    Some(json!({"error":error.to_string(),"device_write_acknowledged":false}))
            }
        }
        self.emit_next_trigger(cx);
    }
    pub(super) fn thumbstick_changed(&mut self, path: &str, cx: &mut Context<Self>) {
        match prepare_thumbstick_change(
            &mut self.draft,
            path,
            self.spec.circular_mode,
            !matches!(self.spec.product_id, 2676 | 2684),
        ) {
            Ok(operation) => self
                .trigger_runtime
                .enqueue(operation, self.draft["profile"].clone()),
            Err(error) => {
                self.trigger_runtime.last_result =
                    Some(json!({"error":error.to_string(),"device_write_acknowledged":false}))
            }
        }
        self.emit_next_trigger(cx);
    }
    fn emit_next_trigger(&mut self, cx: &mut Context<Self>) {
        if let Some(request) = self.trigger_runtime.next() {
            cx.emit(request);
        }
    }
    pub(super) fn request_trigger_read(&mut self, cx: &mut Context<Self>) {
        if self.page == "THUMBSTICKS" {
            if !self.trigger_runtime.thumbsticks_read_requested {
                self.trigger_runtime.thumbsticks_read_requested = true;
                if let Ok(profile_slot) = profile_slot(&self.draft) {
                    self.trigger_runtime.enqueue(
                        TriggerOperation::ReadThumbsticks { profile_slot },
                        self.draft["profile"].clone(),
                    );
                    self.emit_next_trigger(cx);
                }
            }
            return;
        }
        if self.page != "TRIGGERS" || self.trigger_runtime.read_requested {
            return;
        }
        self.trigger_runtime.read_requested = true;
        if let Ok(profile_slot) = profile_slot(&self.draft) {
            self.trigger_runtime.enqueue(
                TriggerOperation::Read { profile_slot },
                self.draft["profile"].clone(),
            );
            self.emit_next_trigger(cx);
        }
    }
    pub fn controller_trigger_request_matches(&self, request: &ControllerTriggerRequested) -> bool {
        self.trigger_runtime.matches(request)
    }
    pub fn finish_controller_trigger(
        &mut self,
        request: &ControllerTriggerRequested,
        observed: Option<Value>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if !self.trigger_runtime.matches(request) {
            return;
        }
        if request.operation.is_read() && request.revision == self.trigger_runtime.revision {
            if let Some(profile) = observed
                .as_ref()
                .filter(|v| v["device_read_observed"] == true)
                .and_then(|v| serde_json::from_value::<TriggerProfile>(v["profile"].clone()).ok())
            {
                if let Some(thumbsticks) = profile.thumbsticks() {
                    let saved = json!({
                        "controllerDeadzone": {"deadzones":thumbsticks.deadzones(),"preventDoubleDeadzone":thumbsticks.prevent_double_deadzone()},
                        "controllerSensitivity": {"sensitivityValues":thumbsticks.sensitivity_values()},
                        "controllerAnalogCircularity": if thumbsticks.circular() { "circular" } else { "standard" }
                    });
                    project_thumbstick_profile(
                        &mut self.draft,
                        &saved,
                        self.spec.standard_mode,
                        self.spec.circular_mode,
                    );
                    self.refresh_thumbstick_artwork(cx);
                    if let Some(window) = window {
                        self.sync_thumbstick_controls(window, cx);
                    }
                    cx.emit(GamepadProductChanged);
                } else {
                    self.draft["profile"]["controllerAnalogActuation"] =
                        json!({"analogActuations":profile.analog_actuations()});
                    self.draft["profile"]["controllerDigitalActuation"] =
                        json!({"digitalActuations":profile.digital_actuations()});
                    // Current middleware publishes analog then digital. The second
                    // reducer owns the final mode; no setter acknowledgement calls a getter.
                    for (key, array, analog) in [
                        ("controllerAnalogActuation", "analogActuations", true),
                        ("controllerDigitalActuation", "digitalActuations", false),
                    ] {
                        let entries = self.draft["profile"][key][array]
                            .as_array()
                            .cloned()
                            .unwrap_or_default();
                        for entry in entries {
                            let side = match entry["controllerId"].as_u64() {
                                Some(1) => "leftTrigger",
                                Some(2) => "rightTrigger",
                                _ => continue,
                            };
                            let target = &mut self.draft["controller"][side];
                            let active = entry["isActive"].as_bool().unwrap_or(false);
                            target["mode"] = json!(if active == analog {
                                self.spec.analog_mode
                            } else {
                                self.spec.digital_mode
                            });
                            for field in if analog {
                                &["startRange", "endRange"][..]
                            } else {
                                &["actuationPoint", "isRapidTrigger"][..]
                            } {
                                target[*field] = entry[*field].clone();
                            }
                        }
                    }
                    if let Some(window) = window {
                        self.sync_sliders(window, cx);
                    }
                    self.refresh_trigger_visualization(cx);
                    cx.emit(GamepadProductChanged);
                }
            }
        }
        self.trigger_runtime.last_result = observed;
        self.trigger_runtime.active = None;
        self.emit_next_trigger(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TriggerOperation, TriggerRuntime, prepare_change, prepare_thumbstick_change,
        project_thumbstick_profile, source_product,
    };
    use serde_json::json;
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn mode_change_preserves_inactive_values_and_updates_opposite_profile_flag() {
        let spec = source_product(2629).unwrap();
        let mut draft = json!({"profile":spec.profile,"controller":spec.controller});
        // The current profile starts with null feature records. This fixture
        // represents both actual analog getter observations before switching.
        draft["profile"]["controllerAnalogActuation"] = json!({"analogActuations":[
            {"controllerId":1,"startRange":0,"endRange":100,"isActive":true},
            {"controllerId":2,"startRange":0,"endRange":100,"isActive":true}
        ]});
        draft["controller"]["leftTrigger"]["mode"] = json!(spec.digital_mode);
        let original_right =
            draft["profile"]["controllerAnalogActuation"]["analogActuations"][1].clone();
        let operation =
            prepare_change(&mut draft, "/controller/leftTrigger", spec.analog_mode).unwrap();
        assert!(matches!(
            operation,
            TriggerOperation::Digital {
                profile_slot: 1,
                ..
            }
        ));
        assert_eq!(
            draft["profile"]["controllerAnalogActuation"]["analogActuations"][0]["isActive"],
            false
        );
        assert_eq!(
            draft["profile"]["controllerAnalogActuation"]["analogActuations"][1],
            original_right
        );
        assert_eq!(
            draft["profile"]["controllerDigitalActuation"]["digitalActuations"][0]["isActive"],
            true
        );
    }
    #[test]
    fn trigger_queue_serializes_requests_and_profile_change_invalidates_inflight_io() {
        let mut state = TriggerRuntime::default();
        state.enqueue(TriggerOperation::Read { profile_slot: 1 }, json!({}));
        state.enqueue(TriggerOperation::Read { profile_slot: 2 }, json!({}));
        let first = state.next().unwrap();
        assert!(state.matches(&first));
        assert!(state.next().is_none());
        state.invalidate();
        assert!(!first.is_valid());
        assert!(!state.matches(&first));
        assert!(state.next().is_none());
    }
    #[test]
    fn thumbstick_mutation_retains_profile_list_order_and_legacy_left_flag() {
        let spec = source_product(2629).unwrap();
        let mut draft = json!({"profile":spec.profile,"controller":spec.controller});
        draft["profile"]["obmSlotId"] = json!([3]);
        draft["profile"]["controllerDeadzone"] = json!({"deadzones":[{"controllerId":2,"value":9},{"controllerId":1,"value":7}],"preventDoubleDeadzone":false});
        draft["controller"]["leftStick"]["preventDoubleDeadzone"] = json!(true);
        draft["controller"]["rightStick"]["preventDoubleDeadzone"] = json!(false);
        draft["controller"]["rightStick"]["deadzoneValue"] = json!(5);
        let operation = prepare_thumbstick_change(
            &mut draft,
            "/controller/rightStick/deadzoneValue",
            spec.circular_mode,
            true,
        )
        .unwrap();
        assert!(matches!(
            operation,
            TriggerOperation::ThumbstickDeadzone {
                profile_slot: 3,
                prevent_double_deadzone: true,
                ..
            }
        ));
        assert_eq!(
            draft["profile"]["controllerDeadzone"]["deadzones"],
            json!([{"controllerId":2,"value":5},{"controllerId":1,"value":7}])
        );
        draft["controller"]["rightStick"]["preventDoubleDeadzone"] = json!(false);
        let modern = prepare_thumbstick_change(
            &mut draft,
            "/controller/rightStick/deadzoneValue",
            spec.circular_mode,
            false,
        )
        .unwrap();
        assert!(matches!(
            modern,
            TriggerOperation::ThumbstickDeadzone {
                prevent_double_deadzone: false,
                ..
            }
        ));
    }
    #[test]
    fn restored_thumbstick_edit_keeps_the_other_side_and_the_obm_slot() {
        for pid in [2629, 2636, 2647, 2650, 2676, 2684, 4133, 4144] {
            let spec = source_product(pid).unwrap();
            let mut saved = json!({"profile":spec.profile,"controller":spec.controller});
            saved["profile"]["obmSlotId"] = json!([3]);
            saved["controller"]["rightStick"]["deadzoneValue"] = json!(9);
            prepare_thumbstick_change(
                &mut saved,
                "/controller/rightStick/deadzoneValue",
                spec.circular_mode,
                !matches!(pid, 2676 | 2684),
            )
            .unwrap();
            let mut restored = json!({"profile":spec.profile,"controller":spec.controller});
            super::super::merge_known(&mut restored, &saved);
            super::super::profile::restore_features(
                &mut restored["profile"],
                &saved["profile"],
                spec,
            )
            .unwrap();
            restored["controller"]["leftStick"]["deadzoneValue"] = json!(5);
            let operation = prepare_thumbstick_change(
                &mut restored,
                "/controller/leftStick/deadzoneValue",
                spec.circular_mode,
                !matches!(pid, 2676 | 2684),
            )
            .unwrap();
            assert!(matches!(
                operation,
                TriggerOperation::ThumbstickDeadzone {
                    profile_slot: 3,
                    ..
                }
            ));
            assert_eq!(
                restored["profile"]["controllerDeadzone"]["deadzones"],
                json!([{"controllerId":2,"value":9},{"controllerId":1,"value":5}]),
                "{pid}"
            );
        }
    }
    #[test]
    fn sensitivity_profile_update_does_not_mutate_deadzone_or_fabricate_readback() {
        let spec = source_product(2636).unwrap();
        let mut draft = json!({"profile":spec.profile,"controller":spec.controller});
        let deadzone = draft["profile"]["controllerDeadzone"].clone();
        draft["controller"]["leftStick"]["sensitivityValue"] = json!(30);
        let operation = prepare_thumbstick_change(
            &mut draft,
            "/controller/leftStick/sensitivityValue",
            spec.circular_mode,
            true,
        )
        .unwrap();
        assert!(matches!(
            operation,
            TriggerOperation::ThumbstickSensitivity {
                profile_slot: 1,
                ..
            }
        ));
        assert_eq!(draft["profile"]["controllerDeadzone"], deadzone);
        let value = draft["controller"]["leftStick"]["deadzoneValue"].clone();
        project_thumbstick_profile(
            &mut draft,
            &json!({"controllerSensitivity":{"sensitivityValues":[{"controllerId":2,"value":80}]},"controllerAnalogCircularity":"circular"}),
            spec.standard_mode,
            spec.circular_mode,
        );
        assert_eq!(draft["controller"]["rightStick"]["sensitivityValue"], 80);
        assert_eq!(draft["controller"]["leftStick"]["deadzoneValue"], value);
        assert_eq!(
            draft["controller"]["circularity"]["mode"],
            spec.circular_mode
        );
    }
}

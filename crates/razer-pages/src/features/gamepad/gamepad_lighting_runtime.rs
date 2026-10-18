//! Serialized controller lighting intents and persistence projections. Current
//! source setters keep their optimistic UI values after transport failure.
use super::lighting_state::ControllerLightingSettings;
use super::quick_lighting_state::QuickLightingSettings;
use super::{GamepadProductChanged, GamepadProductWorkspace};
use gpui_kit::*;
use razer_device::controller_lighting::{LightingOperation, LightingResult};
use serde_json::Value;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone)]
pub struct ControllerLightingRequested {
    sequence: u64,
    revision: u64,
    operation: LightingOperation,
    profile: Value,
    guard: Arc<AtomicBool>,
}
impl ControllerLightingRequested {
    pub fn operation(&self) -> &LightingOperation {
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
    /// Rebase only the submitted feature, preserving later unrelated edits.
    pub fn rebase_settings(&self, settings: &mut Value) -> anyhow::Result<()> {
        let target = settings
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Controller source settings are not an object"))?;
        let profile = target
            .entry("profile")
            .or_insert_with(|| serde_json::json!({}));
        let profile = profile
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Controller profile is not an object"))?;
        match &self.operation {
            LightingOperation::Set { .. } | LightingOperation::Initialize { .. } => {
                let saved = self
                    .profile
                    .get("controllerLightingEffects")
                    .ok_or_else(|| anyhow::anyhow!("Controller lighting settings are missing"))?;
                let validated: ControllerLightingSettings = serde_json::from_value(saved.clone())?;
                validated.protocol_effect()?;
                profile.insert("controllerLightingEffects".into(), saved.clone());
                let controller = target
                    .entry("controller")
                    .or_insert_with(|| serde_json::json!({}));
                let controller = controller
                    .as_object_mut()
                    .ok_or_else(|| anyhow::anyhow!("Controller UI state is not an object"))?;
                controller.insert("lighting".into(), serde_json::to_value(validated)?);
            }
            LightingOperation::Brightness { .. } => {
                let saved = self
                    .profile
                    .get("brightness")
                    .ok_or_else(|| anyhow::anyhow!("Controller brightness settings are missing"))?;
                let brightness: Brightness = serde_json::from_value(saved.clone())?;
                brightness.validate()?;
                profile.insert("brightness".into(), saved.clone());
            }
            LightingOperation::Software { .. } => {
                let saved = self
                    .profile
                    .get("quickEffects")
                    .ok_or_else(|| anyhow::anyhow!("Kitsune quick-effect settings are missing"))?;
                let submitted: QuickLightingSettings = serde_json::from_value(saved.clone())?;
                let current = profile
                    .get_mut("quickEffects")
                    .ok_or_else(|| anyhow::anyhow!("Current Kitsune quick effects are missing"))?;
                submitted.rebase_effect_settings(current)?;
            }
            _ => {}
        }
        Ok(())
    }
}
impl EventEmitter<ControllerLightingRequested> for GamepadProductWorkspace {}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Brightness {
    is_enabled: bool,
    value: u8,
}
impl Brightness {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.value <= 100,
            "Controller brightness exceeds its source limits"
        );
        Ok(())
    }
    fn wire_value(&self) -> u8 {
        if self.is_enabled { self.value } else { 0 }
    }
}

pub(super) struct LightingRuntime {
    pub(super) settings: Option<ControllerLightingSettings>,
    pub(super) quick: Option<QuickLightingSettings>,
    guard: Arc<AtomicBool>,
    sequence: u64,
    revision: u64,
    active: Option<u64>,
    pending: VecDeque<ControllerLightingRequested>,
    initialized: bool,
    last_result: Option<Result<LightingResult, String>>,
}
impl LightingRuntime {
    pub(super) fn new(product_id: u32) -> Self {
        Self {
            settings: (product_id == 2636).then(ControllerLightingSettings::default),
            quick: (product_id == 4115).then(QuickLightingSettings::default),
            guard: Arc::new(AtomicBool::new(true)),
            sequence: 0,
            revision: 0,
            active: None,
            pending: VecDeque::new(),
            initialized: false,
            last_result: None,
        }
    }
    pub(super) fn invalidate(&mut self) {
        self.guard.store(false, Ordering::Release);
        self.guard = Arc::new(AtomicBool::new(true));
        self.active = None;
        self.pending.clear();
        self.initialized = false;
        self.last_result = None;
        self.revision = self.revision.wrapping_add(1);
    }
    fn enqueue(&mut self, operation: LightingOperation, profile: Value) {
        self.sequence = self.sequence.wrapping_add(1);
        if !operation.is_read() {
            self.revision = self.revision.wrapping_add(1);
        }
        self.pending.push_back(ControllerLightingRequested {
            sequence: self.sequence,
            revision: self.revision,
            operation,
            profile,
            guard: Arc::clone(&self.guard),
        });
    }
    fn next(&mut self) -> Option<ControllerLightingRequested> {
        if self.active.is_some() {
            return None;
        }
        let request = self.pending.pop_front()?;
        self.active = Some(request.sequence);
        Some(request)
    }
    fn matches(&self, request: &ControllerLightingRequested) -> bool {
        request.is_valid()
            && Arc::ptr_eq(&request.guard, &self.guard)
            && self.active == Some(request.sequence)
    }
}
impl Drop for LightingRuntime {
    fn drop(&mut self) {
        self.guard.store(false, Ordering::Release);
    }
}

impl GamepadProductWorkspace {
    pub(super) fn restore_lighting_profile(&mut self) -> anyhow::Result<()> {
        if self.spec.product_id == 4115 {
            let saved = self.draft["profile"]
                .get("quickEffects")
                .ok_or_else(|| anyhow::anyhow!("Kitsune source profile has no quick effects"))?;
            self.lighting_runtime.quick = Some(serde_json::from_value(saved.clone())?);
            self.project_quick_lighting();
            return Ok(());
        }
        if self.spec.product_id != 2636 {
            return Ok(());
        }
        let settings = match self.draft["profile"].get("controllerLightingEffects") {
            Some(saved) => serde_json::from_value(saved.clone())?,
            None => ControllerLightingSettings::default(),
        };
        self.lighting_runtime.settings = Some(settings);
        self.project_controller_lighting();
        Ok(())
    }
    fn project_controller_lighting(&mut self) {
        if let Some(settings) = &self.lighting_runtime.settings {
            let value = serde_json::to_value(settings).expect("typed controller lighting settings");
            // Preserve externally stored extension fields at the file boundary.
            if let Some(target) = self.draft["profile"]["controllerLightingEffects"].as_object_mut()
            {
                target.extend(value.as_object().expect("typed settings object").clone());
            } else {
                self.draft["profile"]["controllerLightingEffects"] = value.clone();
            }
            self.draft["controller"]["lighting"] = value;
        }
    }
    fn controller_profile_slot(&self) -> anyhow::Result<u8> {
        let slot = match self.draft["profile"].get("obmSlotId") {
            Some(value) => {
                let slots: Vec<u8> = serde_json::from_value(value.clone())?;
                slots.first().copied().unwrap_or(0)
            }
            None => 0,
        };
        Ok(slot.max(1))
    }
    pub(super) fn submit_controller_effect(&mut self, cx: &mut Context<Self>) {
        self.project_controller_lighting();
        let operation = (|| {
            let settings = self.lighting_runtime.settings.as_ref().ok_or_else(|| {
                anyhow::anyhow!("This product does not mount controller preset effects")
            })?;
            Ok(LightingOperation::Set {
                profile_slot: self.controller_profile_slot()?,
                effect: settings.protocol_effect()?,
            })
        })();
        self.enqueue_lighting(operation, cx);
        cx.emit(GamepadProductChanged);
        cx.notify();
    }
    pub(super) fn controller_brightness_changed(&mut self, path: &str, cx: &mut Context<Self>) {
        if razer_device::controller_lighting::capability(self.spec.product_id).is_none() {
            return;
        }
        let operation = (|| {
            let mut brightness: Brightness =
                serde_json::from_value(self.draft["profile"]["brightness"].clone())?;
            brightness.validate()?;
            if path.ends_with("/value") {
                brightness.is_enabled = brightness.value != 0;
                self.draft["profile"]["brightness"]["isEnabled"] =
                    serde_json::json!(brightness.is_enabled);
            }
            Ok(LightingOperation::Brightness {
                value: brightness.wire_value(),
            })
        })();
        self.enqueue_lighting(operation, cx);
    }
    pub(super) fn enqueue_lighting(
        &mut self,
        operation: anyhow::Result<LightingOperation>,
        cx: &mut Context<Self>,
    ) {
        match operation {
            Ok(operation) => self
                .lighting_runtime
                .enqueue(operation, self.draft["profile"].clone()),
            Err(error) => {
                self.lighting_runtime.last_result = Some(Err(error.to_string()));
                eprintln!("[controller-lighting] {error:#}");
            }
        }
        self.emit_next_lighting(cx);
    }
    fn emit_next_lighting(&mut self, cx: &mut Context<Self>) {
        if let Some(request) = self.lighting_runtime.next() {
            cx.emit(request);
        }
    }
    pub fn request_controller_lighting(&mut self, cx: &mut Context<Self>) {
        if self.lighting_runtime.initialized
            || razer_device::controller_lighting::capability(self.spec.product_id).is_none()
        {
            return;
        }
        self.lighting_runtime.initialized = true;
        if let Some(settings) = &self.lighting_runtime.settings {
            let operation = (|| {
                Ok(LightingOperation::Initialize {
                    profile_slot: self.controller_profile_slot()?,
                    effect: settings.protocol_effect()?,
                })
            })();
            self.enqueue_lighting(operation, cx);
        }
        self.controller_brightness_changed("/profile/brightness/isEnabled", cx);
        if self.spec.product_id == 4115 && !self.chroma_runtime.advanced {
            self.submit_quick_lighting(cx);
        }
    }
    pub fn controller_lighting_request_matches(
        &self,
        request: &ControllerLightingRequested,
    ) -> bool {
        self.lighting_runtime.matches(request)
    }
    pub fn finish_controller_lighting(
        &mut self,
        request: &ControllerLightingRequested,
        result: Result<LightingResult, String>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        if !self.lighting_runtime.matches(request) {
            return;
        }
        if request.operation.is_read() && request.revision == self.lighting_runtime.revision {
            if let Ok(observed) = &result {
                if let Some(effect) = &observed.effect {
                    if let Some(settings) = &mut self.lighting_runtime.settings {
                        // Validate the entire observation before replacing source state.
                        let mut next = settings.clone();
                        match next.observe(effect) {
                            Ok(()) => {
                                *settings = next;
                                self.project_controller_lighting();
                                cx.emit(GamepadProductChanged);
                            }
                            Err(error) => eprintln!(
                                "[controller-lighting] Invalid device observation: {error:#}"
                            ),
                        }
                    }
                }
                if let Some(brightness) = &observed.brightness {
                    // Current brightness getDataFromDevice keeps the prior value
                    // at zero and derives isEnabled from the observed brightness.
                    self.draft["profile"]["brightness"]["isEnabled"] =
                        serde_json::json!(brightness.value > 0);
                    if brightness.value > 0 {
                        self.draft["profile"]["brightness"]["value"] =
                            serde_json::json!(brightness.value);
                    }
                    if let Some(window) = window {
                        self.sync_sliders(window, cx);
                    }
                    cx.emit(GamepadProductChanged);
                }
            }
        }
        if let Err(error) = &result {
            eprintln!("[controller-lighting] {error}");
        }
        self.lighting_runtime.last_result = Some(result);
        self.lighting_runtime.active = None;
        self.emit_next_lighting(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ControllerLightingSettings, LightingOperation, LightingRuntime, QuickLightingSettings,
    };
    use serde_json::json;
    #[test]
    fn controller_lighting_queue_and_generation_reject_replaced_profile_results() {
        let mut runtime = LightingRuntime::new(2636);
        runtime.enqueue(
            LightingOperation::Brightness { value: 50 },
            json!({"brightness":{"isEnabled":true,"value":50}}),
        );
        runtime.enqueue(
            LightingOperation::Brightness { value: 0 },
            json!({"brightness":{"isEnabled":false,"value":50}}),
        );
        let first = runtime.next().unwrap();
        assert!(runtime.next().is_none());
        assert!(runtime.matches(&first));
        runtime.invalidate();
        assert!(!first.is_valid());
        assert!(!runtime.matches(&first));
        assert!(runtime.next().is_none());
    }
    #[test]
    fn rebasing_a_controller_write_preserves_later_unrelated_features_and_extensions() {
        let settings = ControllerLightingSettings::default();
        let mut runtime = LightingRuntime::new(2636);
        runtime.enqueue(LightingOperation::Set { profile_slot: 3, effect: settings.protocol_effect().unwrap() },
            json!({"controllerLightingEffects":settings,"brightness":{"isEnabled":true,"value":25},"unrelated":"old"}));
        let request = runtime.next().unwrap();
        let mut target = json!({"profile":{"unrelated":"new","brightness":{"isEnabled":true,"value":75}},"controller":{"other":123}});
        request.rebase_settings(&mut target).unwrap();
        assert_eq!(target["profile"]["unrelated"], "new");
        assert_eq!(target["profile"]["brightness"]["value"], 75);
        assert_eq!(target["controller"]["other"], 123);
        assert_eq!(
            target["profile"]["controllerLightingEffects"]["effectId"],
            3
        );
        assert_eq!(target["controller"]["lighting"]["effectId"], 3);
    }
    #[test]
    fn rebasing_software_effect_keeps_later_profile_palette_and_extensions() {
        let submitted: QuickLightingSettings = serde_json::from_value(json!({
            "selectedEffectId":3,
            "selectedEffectSetting":{"duration":2},
            "customColors":[{"color":"#112233","picked":true}],
            "extension":"captured"
        }))
        .unwrap();
        let mut runtime = LightingRuntime::new(4115);
        runtime.enqueue(
            LightingOperation::Software {
                edition: 0,
                effect: submitted.setting().software(),
            },
            json!({"quickEffects": submitted}),
        );
        let request = runtime.next().unwrap();
        let mut target = json!({"profile":{"quickEffects":{
            "selectedEffectId":1,
            "selectedEffectSetting":{"color1":"#aabbcc"},
            "customColors":[{"color":"#abcdef","picked":true,"revision":2}],
            "extension":"later"
        }}});
        request.rebase_settings(&mut target).unwrap();
        let quick = &target["profile"]["quickEffects"];
        assert_eq!(quick["selectedEffectId"], 3);
        assert_eq!(quick["selectedEffectSetting"]["duration"], 2);
        assert_eq!(quick["extension"], "later");
        assert_eq!(
            quick["customColors"],
            json!([{"color":"#abcdef","picked":true,"revision":2}])
        );
    }
}

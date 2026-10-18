//! Source rotation drafts, persisted profiles and setter acknowledgements.
//! Current middleware restores the selected local profile and never consumes GET.
use super::*;
use razer_device::{
    mapping_native_rotation::RotationReply,
    mouse_rotation::{Report, Setting},
};
use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseRotationSource {
    UserEdit,
    ProfileRestore,
}

#[derive(Clone)]
pub struct MouseRotationRequested {
    scope: MousePollingScope,
    sequence: u64,
    product_id: u32,
    setting: Setting,
    source: MouseRotationSource,
    guard: Arc<AtomicBool>,
}
impl MouseRotationRequested {
    pub fn scope(&self) -> MousePollingScope {
        self.scope
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn setting(&self) -> Setting {
        self.setting
    }
    pub fn source(&self) -> MouseRotationSource {
        self.source
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.guard.clone()
    }
    pub fn is_valid(&self) -> bool {
        self.guard.load(Ordering::Acquire)
    }
}

pub enum MouseRotationCompletion {
    Direct(Report),
    Native(RotationReply),
    Failed(String),
    Cancelled,
}
impl EventEmitter<MouseRotationRequested> for MouseProductWorkspace {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Acknowledgement {
    Direct(Setting),
    Native(Setting),
}

pub(super) struct State {
    draft: Setting,
    persisted: Option<Setting>,
    profile_error: Option<String>,
    has_profile_value: bool,
    sequence: u64,
    persisted_sequence: u64,
    pending: BTreeMap<u64, MouseRotationRequested>,
    scope_guards: Vec<Weak<AtomicBool>>,
    acknowledged: Option<Acknowledgement>,
    failure: Option<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            // Current rotation reducer initial state; this is only a UI draft.
            draft: Setting {
                enabled: true,
                angle_degrees: 0,
            },
            persisted: None,
            profile_error: None,
            has_profile_value: false,
            sequence: 0,
            persisted_sequence: 0,
            pending: BTreeMap::new(),
            scope_guards: Vec::new(),
            acknowledged: None,
            failure: None,
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.invalidate();
    }
}
impl State {
    fn invalidate(&mut self) {
        // A completed request may still own a persistent native session.
        for guard in self
            .scope_guards
            .drain(..)
            .filter_map(|guard| guard.upgrade())
        {
            guard.store(false, Ordering::Release);
        }
        self.pending.clear();
        self.acknowledged = None;
        self.failure = None;
    }
    fn restore(&mut self, profile: Option<&Value>) {
        self.invalidate();
        self.persisted = None;
        self.profile_error = None;
        self.has_profile_value = profile.is_some_and(|profile| profile.get("rotation").is_some());
        // Mn only dispatches fields present in the selected source profile.
        // A missing field does not reset the reducer or authorize a device SET.
        if let Some(value) = profile.and_then(|profile| profile.get("rotation")) {
            match source_setting(value) {
                Ok(setting) => {
                    self.draft = setting;
                    self.persisted = Some(setting);
                }
                Err(error) => self.profile_error = Some(error),
            }
        }
    }
    fn begin(
        &mut self,
        scope: MousePollingScope,
        product_id: u32,
        source: MouseRotationSource,
    ) -> MouseRotationRequested {
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("Rotation request identity exhausted");
        self.acknowledged = None;
        self.failure = None;
        let request = MouseRotationRequested {
            scope,
            sequence: self.sequence,
            product_id,
            setting: self.draft,
            source,
            guard: Arc::new(AtomicBool::new(true)),
        };
        self.scope_guards.retain(|guard| guard.strong_count() != 0);
        self.scope_guards.push(Arc::downgrade(&request.guard));
        self.pending.insert(request.sequence, request.clone());
        request
    }
    fn matches(&self, request: &MouseRotationRequested) -> bool {
        request.is_valid()
            && self.pending.get(&request.sequence).is_some_and(|pending| {
                pending.scope == request.scope
                    && pending.setting == request.setting
                    && Arc::ptr_eq(&pending.guard, &request.guard)
            })
    }
    fn persisted(&mut self, request: &MouseRotationRequested) -> bool {
        if !self.matches(request) {
            return false;
        }
        if request.sequence >= self.persisted_sequence {
            self.persisted_sequence = request.sequence;
            self.persisted = Some(request.setting);
        }
        true
    }
    fn complete(
        &mut self,
        request: &MouseRotationRequested,
        completion: MouseRotationCompletion,
    ) -> Option<String> {
        if !self.matches(request) {
            return None;
        }
        self.pending.remove(&request.sequence);
        if request.sequence != self.sequence {
            return None;
        }
        let result = match completion {
            MouseRotationCompletion::Direct(report) => report
                .validate_reply(Some(&request.setting))
                .map_err(|error| format!("{error:#}"))
                .and_then(|()| {
                    if report.source_acknowledged() {
                        Ok(Some(Acknowledgement::Direct(request.setting)))
                    } else {
                        Err(report
                            .failure_reason()
                            .unwrap_or("Rotation setter was not acknowledged")
                            .to_owned())
                    }
                }),
            MouseRotationCompletion::Native(RotationReply::SetComplete) => {
                Ok(Some(Acknowledgement::Native(request.setting)))
            }
            MouseRotationCompletion::Native(RotationReply::Angle { .. }) => {
                Err("Rotation SET received a native GET result".into())
            }
            MouseRotationCompletion::Failed(error) => Err(error),
            MouseRotationCompletion::Cancelled => Ok(None),
        };
        match result {
            Ok(acknowledged) => {
                self.acknowledged = acknowledged;
                self.failure = None;
                None
            }
            Err(error) => {
                self.acknowledged = None;
                self.failure = Some(error.clone());
                Some(error)
            }
        }
    }
}

fn source_setting(value: &Value) -> Result<Setting, String> {
    #[derive(Deserialize)]
    struct Source {
        #[serde(rename = "isEnabled")]
        enabled: bool,
        value: i8,
    }
    let source: Source = serde_json::from_value(value.clone())
        .map_err(|error| format!("Invalid local rotation setting: {error}"))?;
    let setting = Setting {
        enabled: source.enabled,
        angle_degrees: source.value,
    };
    setting.validate().map_err(|error| format!("{error:#}"))?;
    Ok(setting)
}

impl MouseProductWorkspace {
    pub(super) fn restore_rotation(&mut self, value: Option<&Value>) {
        if self.spec.rotation {
            self.rotation_state.restore(value);
            // PID226 UI loadRotationSettings merges its UI DEFAULTPROFILE.
            // This projection never supplies an absent middleware profile field.
            if self.spec.product_id == 226
                && value.is_some()
                && value.and_then(|profile| profile.get("rotation")).is_none()
            {
                self.rotation_state.draft = Setting {
                    enabled: true,
                    angle_degrees: 0,
                };
            }
        }
    }
    pub(super) fn snapshot_rotation(&self, snapshot: &mut Value) {
        if self.spec.rotation && !self.rotation_state.has_profile_value {
            if let Some(fields) = snapshot.as_object_mut() {
                fields.remove("rotation");
            }
        }
    }
    pub(super) fn rotation_editable(&self) -> bool {
        self.active && self.advanced_enabled() && self.rotation_state.profile_error.is_none()
    }
    pub(super) fn rotation_enabled(&self) -> bool {
        self.rotation_state.draft.enabled
    }
    pub(super) fn rotation_degrees(&self) -> i8 {
        self.rotation_state.draft.angle_degrees
    }
    pub(super) fn edit_rotation_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if !self.rotation_editable() || self.rotation_enabled() == enabled {
            return;
        }
        self.rotation_state.draft.enabled = enabled;
        self.submit_rotation_edit(cx);
    }
    pub(super) fn edit_rotation_degrees(&mut self, value: f32, cx: &mut Context<Self>) {
        if !self.rotation_editable()
            || !self.rotation_enabled()
            || !value.is_finite()
            || value.fract() != 0.
            || !(-44. ..=44.).contains(&value)
        {
            return;
        }
        let angle = value as i8;
        if self.rotation_degrees() == angle {
            return;
        }
        self.rotation_state.draft.angle_degrees = angle;
        self.submit_rotation_edit(cx);
    }
    fn submit_rotation_edit(&mut self, cx: &mut Context<Self>) {
        let setting = self.rotation_state.draft;
        self.rotation_state.has_profile_value = true;
        set_pointer(
            &mut self.draft,
            "/rotation/isEnabled",
            json!(setting.enabled),
        );
        set_pointer(
            &mut self.draft,
            "/rotation/value",
            json!(setting.angle_degrees),
        );
        self.emit_rotation_request(MouseRotationSource::UserEdit, cx);
        cx.notify();
    }
    fn emit_rotation_request(&mut self, source: MouseRotationSource, cx: &mut Context<Self>) {
        if !self.spec.rotation
            || razer_device::mouse_rotation::capability(self.spec.product_id).is_none()
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        cx.emit(
            self.rotation_state
                .begin(scope, self.spec.product_id, source),
        );
    }
    pub fn apply_mouse_rotation_profile(&mut self, cx: &mut Context<Self>) {
        if self.rotation_state.profile_error.is_none() && self.rotation_state.persisted.is_some() {
            self.emit_rotation_request(MouseRotationSource::ProfileRestore, cx);
        }
    }
    pub fn invalidate_mouse_rotation(&mut self) {
        self.rotation_state.invalidate();
    }
    pub fn mouse_rotation_scope_is_current(
        &self,
        request: &MouseRotationRequested,
        cx: &App,
    ) -> bool {
        self.active
            && request.is_valid()
            && request.product_id == self.spec.product_id
            && self.mouse_polling_scope(cx) == Some(request.scope)
    }
    pub fn mouse_rotation_request_is_current(
        &self,
        request: &MouseRotationRequested,
        cx: &App,
    ) -> bool {
        self.mouse_rotation_scope_is_current(request, cx) && self.rotation_state.matches(request)
    }
    pub fn mark_mouse_rotation_persisted(
        &mut self,
        request: &MouseRotationRequested,
        cx: &mut Context<Self>,
    ) -> bool {
        self.mouse_rotation_scope_is_current(request, cx) && self.rotation_state.persisted(request)
    }
    pub fn complete_mouse_rotation(
        &mut self,
        request: &MouseRotationRequested,
        completion: MouseRotationCompletion,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        if !self.mouse_rotation_scope_is_current(request, cx) {
            return None;
        }
        let failure = self.rotation_state.complete(request, completion);
        cx.notify();
        failure
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn begin(state: &mut State) -> MouseRotationRequested {
        state.begin(
            MousePollingScope::new(EntityId::from(1), 0, 0),
            196,
            MouseRotationSource::UserEdit,
        )
    }
    #[::core::prelude::v1::test]
    fn mouse_rotation_owner_missing_profile_does_not_initialize_or_acknowledge() {
        let mut state = State::default();
        state.restore(Some(&json!({"rotation":{"isEnabled":false,"value":-19}})));
        assert_eq!(
            state.draft,
            Setting {
                enabled: false,
                angle_degrees: -19
            }
        );
        state.restore(Some(&json!({})));
        assert_eq!(state.draft.angle_degrees, -19);
        assert!(state.persisted.is_none());
        assert!(state.pending.is_empty());
        assert!(state.acknowledged.is_none());
        for value in [
            json!({"value":0}),
            json!({"isEnabled":0,"value":0}),
            json!({"isEnabled":true,"value":45}),
        ] {
            state.restore(Some(&json!({"rotation":value})));
            assert!(state.profile_error.is_some());
            assert!(state.persisted.is_none());
        }
    }
    #[::core::prelude::v1::test]
    fn mouse_rotation_owner_persistence_failure_and_cancel_never_confirm() {
        let mut state = State::default();
        let request = begin(&mut state);
        assert!(state.persisted(&request));
        assert!(state.acknowledged.is_none());
        assert_eq!(
            state.complete(
                &request,
                MouseRotationCompletion::Failed("device failed".into())
            ),
            Some("device failed".into())
        );
        assert_eq!(state.persisted, Some(request.setting));
        assert!(state.acknowledged.is_none());
        let request = begin(&mut state);
        state.complete(&request, MouseRotationCompletion::Cancelled);
        assert!(state.acknowledged.is_none());
    }
    #[::core::prelude::v1::test]
    fn mouse_rotation_owner_native_get_does_not_infer_enabled_or_overwrite_draft() {
        let mut state = State::default();
        state.draft = Setting {
            enabled: false,
            angle_degrees: -32,
        };
        let request = begin(&mut state);
        assert!(
            state
                .complete(
                    &request,
                    MouseRotationCompletion::Native(RotationReply::Angle {
                        raw: 0,
                        bytes: [0; 4]
                    })
                )
                .is_some()
        );
        assert_eq!(state.draft, request.setting);
        assert!(state.acknowledged.is_none());
        let request = begin(&mut state);
        state.complete(
            &request,
            MouseRotationCompletion::Native(RotationReply::SetComplete),
        );
        assert_eq!(
            state.acknowledged,
            Some(Acknowledgement::Native(request.setting))
        );
        assert_eq!(state.draft.angle_degrees, -32);
    }
    #[::core::prelude::v1::test]
    fn mouse_rotation_owner_direct_confirmation_requires_exact_setter_ack() {
        use razer_device::mouse_sensor::{SensorAcknowledgement, SensorWriteReport};
        let mut state = State::default();
        state.draft = Setting {
            enabled: true,
            angle_degrees: -23,
        };
        let request = begin(&mut state);
        let delivery = SensorWriteReport {
            attempted_writes: 1,
            acknowledgements: vec![SensorAcknowledgement {
                command: [3, 11, 20],
                payload: vec![1, 1, (-23_i8) as u8],
                data: vec![],
            }],
        };
        let report = Report::Write {
            requested: request.setting,
            delivery: delivery.clone(),
            failure: None,
        };
        assert!(
            state
                .complete(&request, MouseRotationCompletion::Direct(report))
                .is_none()
        );
        assert_eq!(
            state.acknowledged,
            Some(Acknowledgement::Direct(request.setting))
        );
        let request = begin(&mut state);
        let mut forged = delivery;
        forged.acknowledgements[0].payload[2] = 0;
        let report = Report::Write {
            requested: request.setting,
            delivery: forged,
            failure: None,
        };
        assert!(
            state
                .complete(&request, MouseRotationCompletion::Direct(report))
                .is_some()
        );
        assert!(state.acknowledged.is_none());
        assert_eq!(state.draft.angle_degrees, -23);
        let request = begin(&mut state);
        let report = Report::Read {
            reading: razer_device::mouse_rotation::Reading {
                raw_state: 1,
                raw_angle: 0,
            },
        };
        assert!(
            state
                .complete(&request, MouseRotationCompletion::Direct(report))
                .is_some()
        );
        assert!(state.acknowledged.is_none());
    }
    #[::core::prelude::v1::test]
    fn mouse_rotation_owner_stale_edits_and_lifetime_cannot_confirm_new_draft() {
        let mut state = State::default();
        let first = begin(&mut state);
        state.draft.angle_degrees = 17;
        let second = begin(&mut state);
        state.complete(
            &first,
            MouseRotationCompletion::Native(RotationReply::SetComplete),
        );
        assert!(state.acknowledged.is_none());
        state.invalidate();
        assert!(!second.is_valid());
        state.complete(
            &second,
            MouseRotationCompletion::Native(RotationReply::SetComplete),
        );
        assert!(state.acknowledged.is_none());
        let request = begin(&mut state);
        drop(state);
        assert!(!request.is_valid());
    }
    #[::core::prelude::v1::test]
    fn mouse_rotation_owner_completed_scope_expires_on_lifecycle_change() {
        let mut state = State::default();
        let first = begin(&mut state);
        state.complete(
            &first,
            MouseRotationCompletion::Native(RotationReply::SetComplete),
        );
        assert!(first.is_valid());
        assert!(!state.matches(&first));
        let second = begin(&mut state);
        assert!(first.is_valid());
        state.complete(
            &second,
            MouseRotationCompletion::Native(RotationReply::SetComplete),
        );
        assert!(second.is_valid());
        state.invalidate();
        assert!(!first.is_valid());
        assert!(!second.is_valid());

        let restored = begin(&mut state);
        state.complete(&restored, MouseRotationCompletion::Cancelled);
        assert!(restored.is_valid());
        state.restore(Some(&json!({"rotation":{"isEnabled":true,"value":7}})));
        assert!(!restored.is_valid());

        let final_request = begin(&mut state);
        state.complete(
            &final_request,
            MouseRotationCompletion::Native(RotationReply::SetComplete),
        );
        let native_guard = final_request.guard();
        drop(final_request);
        assert!(native_guard.load(Ordering::Acquire));
        drop(state);
        assert!(!native_guard.load(Ordering::Acquire));
    }
}

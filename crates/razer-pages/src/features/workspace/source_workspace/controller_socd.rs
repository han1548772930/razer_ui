//! Profile-scoped generic SOCD storage and NORMAL_SKIPPABLE device ownership.
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::{
    controller_initialization::SocdMode,
    controller_socd::{SocdOperation, SocdResult, SocdWriteStatus},
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Scope {
    profile_id: String,
    slot: u8,
    generation: u64,
}
#[derive(Clone)]
pub struct ControllerSocdRequested {
    sequence: u64,
    scope: Scope,
    operation: SocdOperation,
    requires_persistence: bool,
    guard: Arc<AtomicBool>,
    storage_guard: Arc<AtomicBool>,
    cleanup_pending: Arc<AtomicBool>,
}
impl ControllerSocdRequested {
    pub fn operation(&self) -> SocdOperation {
        self.operation
    }
    pub fn requires_persistence(&self) -> bool {
        self.requires_persistence
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.guard)
    }
    pub fn is_valid(&self) -> bool {
        self.guard.load(Ordering::Acquire)
    }
    pub fn set_cleanup_pending(&self, pending: bool) {
        self.cleanup_pending.store(pending, Ordering::Release);
    }
    pub fn project_saved(&self, device: &mut Device) -> anyhow::Result<()> {
        self.operation.validate(device.product_id)?;
        let profile = device
            .profiles
            .iter_mut()
            .find(|profile| profile.id == self.scope.profile_id)
            .context("SOCD local profile no longer exists")?;
        let (slot, _) = profile_settings(profile)?;
        ensure!(
            slot == self.scope.slot,
            "SOCD local profile's real slot changed"
        );
        super::super::gamepad_products::project_controller_socd(
            profile
                .source_settings
                .as_mut()
                .context("SOCD source profile is absent")?,
            self.operation.mode,
        )
    }
}

#[derive(serde::Deserialize)]
struct SavedProfile {
    profile: SavedFields,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedFields {
    obm_slot_id: Vec<u8>,
    #[serde(rename = "controllerDPadSOCD")]
    socd: Option<SocdMode>,
}
fn profile_settings(profile: &Profile) -> anyhow::Result<(u8, Option<SocdMode>)> {
    let saved: SavedProfile = serde_json::from_value(
        profile
            .source_settings
            .clone()
            .context("SOCD source profile is absent")?,
    )?;
    let slot = saved
        .profile
        .obm_slot_id
        .first()
        .copied()
        .context("SOCD profile has no observed real slot")?;
    Ok((slot, saved.profile.socd))
}

#[derive(Default)]
struct ActionState {
    latest_sequence: u64,
    draft: Option<SocdMode>,
    result: Option<(SocdOperation, SocdResult)>,
    error: Option<String>,
}

pub(super) struct Owner {
    scope: Option<Scope>,
    guard: Arc<AtomicBool>,
    storage_guard: Arc<AtomicBool>,
    storage_requests: BTreeMap<u64, Scope>,
    sequence: u64,
    active: Option<u64>,
    pending: Option<ControllerSocdRequested>,
    pub(super) ready: bool,
    view: super::super::gamepad_products::SocdPresentation,
    actions: BTreeMap<Scope, ActionState>,
}
impl Default for Owner {
    fn default() -> Self {
        Self {
            scope: None,
            guard: Arc::new(AtomicBool::new(true)),
            storage_guard: Arc::new(AtomicBool::new(true)),
            storage_requests: BTreeMap::new(),
            sequence: 0,
            active: None,
            pending: None,
            ready: false,
            view: Default::default(),
            actions: BTreeMap::new(),
        }
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.guard.store(false, Ordering::Release);
        self.storage_guard.store(false, Ordering::Release);
    }
}
impl Owner {
    fn bind(&mut self, scope: Option<Scope>, persisted: Option<SocdMode>) {
        if self.scope != scope {
            if self
                .scope
                .as_ref()
                .zip(scope.as_ref())
                .is_some_and(|(old, new)| old.generation != new.generation)
            {
                self.actions.clear();
            }
            self.guard.store(false, Ordering::Release);
            self.guard = Arc::new(AtomicBool::new(true));
            self.scope = scope;
            self.active = None;
            self.pending = None;
            self.view = Default::default();
            if let Some(action) = self
                .scope
                .as_ref()
                .and_then(|scope| self.actions.get(scope))
            {
                self.view.draft = action.draft;
                self.view.result = action.result.clone();
                self.view.error = action.error.clone();
            }
        }
        self.view.saving = self
            .storage_requests
            .values()
            .any(|scope| self.scope.as_ref() == Some(scope));
        self.view.persisted = persisted;
        self.view.enabled = self.scope.is_some();
    }
    fn invalidate(&mut self) {
        self.ready = false;
        self.bind(None, None);
        self.actions.clear();
    }
    fn prepare(&mut self, mode: SocdMode) -> Option<ControllerSocdRequested> {
        let scope = self.scope.clone()?;
        self.sequence = self.sequence.wrapping_add(1);
        let request = ControllerSocdRequested {
            sequence: self.sequence,
            operation: SocdOperation {
                slot: scope.slot,
                mode,
            },
            scope,
            requires_persistence: true,
            guard: Arc::clone(&self.guard),
            storage_guard: Arc::clone(&self.storage_guard),
            cleanup_pending: Arc::new(AtomicBool::new(false)),
        };
        self.storage_requests
            .insert(request.sequence, request.scope.clone());
        let action = self.actions.entry(request.scope.clone()).or_default();
        action.latest_sequence = request.sequence;
        action.draft = Some(mode);
        action.error = None;
        self.view.draft = Some(mode);
        self.view.saving = true;
        self.view.error = None;
        Some(request)
    }
    fn matches(&self, request: &ControllerSocdRequested) -> bool {
        if request.requires_persistence {
            request.storage_guard.load(Ordering::Acquire)
                && Arc::ptr_eq(&self.storage_guard, &request.storage_guard)
                && self.storage_requests.get(&request.sequence) == Some(&request.scope)
        } else {
            self.scope.as_ref() == Some(&request.scope)
                && request.is_valid()
                && Arc::ptr_eq(&self.guard, &request.guard)
                && self.active == Some(request.sequence)
        }
    }
    fn storage_finished(
        &mut self,
        request: &ControllerSocdRequested,
        succeeded: bool,
        permit_device: bool,
    ) -> bool {
        if !request.requires_persistence || !self.matches(request) {
            return false;
        }
        self.storage_requests.remove(&request.sequence);
        if let Some(action) = self.actions.get_mut(&request.scope) {
            if action.latest_sequence == request.sequence {
                action.error = (!succeeded).then(|| {
                    "Controller SOCD local save failed; no device task was submitted".into()
                });
            }
        }
        if self.scope.as_ref() == Some(&request.scope) {
            self.view.saving = self
                .storage_requests
                .values()
                .any(|scope| self.scope.as_ref() == Some(scope));
            if succeeded {
                self.view.persisted = Some(request.operation.mode);
                if permit_device && request.is_valid() && Arc::ptr_eq(&self.guard, &request.guard) {
                    let mut next = request.clone();
                    next.requires_persistence = false;
                    self.pending = Some(next);
                }
            }
            self.view.error = self
                .actions
                .get(&request.scope)
                .and_then(|action| action.error.clone());
        }
        true
    }
    fn next(&mut self) -> Option<ControllerSocdRequested> {
        if self.active.is_some() {
            return None;
        }
        let request = self.pending.take()?;
        if !request.is_valid() || self.scope.as_ref() != Some(&request.scope) {
            return None;
        }
        self.active = Some(request.sequence);
        self.view.writing = true;
        Some(request)
    }
    fn finished(
        &mut self,
        request: &ControllerSocdRequested,
        result: Result<SocdResult, String>,
    ) -> bool {
        if request.requires_persistence || !self.matches(request) {
            return false;
        }
        let action = self.actions.entry(request.scope.clone()).or_default();
        match result {
            Ok(result) => match result.validate() {
                Ok(()) => {
                    action.error = match &result.write {
                        SocdWriteStatus::Failed { error } => Some(error.clone()),
                        _ if result.canceled => {
                            Some("Controller SOCD device task was canceled".into())
                        }
                        _ => None,
                    };
                    action.result = Some((request.operation, result));
                }
                Err(error) => action.error = Some(format!("{error:#}")),
            },
            Err(error) => action.error = Some(error),
        }
        self.view.error = action.error.clone();
        self.view.result = action.result.clone();
        if !request.cleanup_pending.load(Ordering::Acquire) {
            self.active = None;
            self.view.writing = false;
        }
        true
    }
}

impl SourceProductWorkspace {
    pub(super) fn invalidate_controller_socd(&mut self, cx: &mut Context<Self>) {
        self.controller_socd.invalidate();
        self.publish_controller_socd(cx);
    }
    pub(super) fn sync_controller_socd(&mut self, cx: &mut Context<Self>) {
        if self.device.product_id != 2676 {
            return;
        }
        let metadata = self
            .device
            .active_profile_obj()
            .map(profile_settings)
            .transpose();
        let persisted = self
            .saved
            .profiles
            .iter()
            .find(|profile| profile.id == self.device.active_profile)
            .map(profile_settings)
            .transpose()
            .map(|profile| profile.and_then(|(_, mode)| mode));
        let mut error = None;
        let (scope, persisted) = match metadata {
            Ok(Some((slot, _))) => {
                let scope = (self.controller_socd.ready
                    && !self.exiting
                    && self.device.dashboard.connection_observation.is_some()
                    && self.controller_socd_for_active().is_some())
                .then(|| Scope {
                    profile_id: self.device.active_profile.clone(),
                    slot,
                    generation: self.mapping_connection_generation,
                });
                match persisted {
                    Ok(persisted) => (scope, persisted),
                    Err(failure) => {
                        error = Some(format!("{failure:#}"));
                        (None, None)
                    }
                }
            }
            Ok(None) => (None, None),
            Err(failure) => {
                error = Some(format!("{failure:#}"));
                (None, None)
            }
        };
        self.controller_socd.bind(scope, persisted);
        if error.is_some() {
            self.controller_socd.view.error = error;
        }
        self.publish_controller_socd(cx);
    }
    fn publish_controller_socd(&self, cx: &mut Context<Self>) {
        if let FamilyBody::Gamepad(body) = &self.body {
            body.update(cx, |body, cx| {
                body.present_controller_socd(self.controller_socd.view.clone(), cx)
            });
        }
    }
    pub(super) fn edit_controller_socd(&mut self, mode: SocdMode, cx: &mut Context<Self>) {
        self.sync_controller_socd(cx);
        let Some(request) = self.controller_socd.prepare(mode) else {
            return;
        };
        if let Err(error) = request.project_saved(&mut self.device) {
            self.controller_socd
                .storage_finished(&request, false, false);
            self.controller_socd.view.error = Some(format!("{error:#}"));
            self.publish_controller_socd(cx);
            return;
        }
        self.publish_controller_socd(cx);
        cx.emit(WorkspaceEvent::ControllerSocdRequested(request));
        cx.notify();
    }
    pub fn controller_socd_request_matches(&self, request: &ControllerSocdRequested) -> bool {
        self.controller_socd.matches(request)
    }
    pub fn complete_controller_socd_storage(
        &mut self,
        request: &ControllerSocdRequested,
        succeeded: bool,
        permit_device: bool,
        cx: &mut Context<Self>,
    ) {
        if self
            .controller_socd
            .storage_finished(request, succeeded, permit_device)
        {
            self.emit_next_controller_socd(cx);
        }
    }
    pub fn finish_controller_socd(
        &mut self,
        request: &ControllerSocdRequested,
        result: Result<SocdResult, String>,
        cx: &mut Context<Self>,
    ) {
        if self.controller_socd.finished(request, result) {
            self.emit_next_controller_socd(cx);
        }
    }
    fn emit_next_controller_socd(&mut self, cx: &mut Context<Self>) {
        let next = self.controller_socd.next();
        self.publish_controller_socd(cx);
        if let Some(request) = next {
            cx.emit(WorkspaceEvent::ControllerSocdRequested(request));
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;
    fn scope(profile: &str, slot: u8, generation: u64) -> Scope {
        Scope {
            profile_id: profile.into(),
            slot,
            generation,
        }
    }
    fn state() -> Owner {
        let mut owner = Owner::default();
        owner.bind(Some(scope("real-profile", 3, 1)), Some(SocdMode::Neutral));
        owner
    }
    fn acknowledged() -> SocdResult {
        SocdResult {
            write: SocdWriteStatus::Acknowledged {
                response_payload: vec![],
            },
            canceled: false,
        }
    }
    #[test]
    fn controller_socd_requires_successful_storage_before_any_device_task() {
        let mut owner = state();
        let request = owner.prepare(SocdMode::First).unwrap();
        assert_eq!(owner.view.persisted, Some(SocdMode::Neutral));
        assert_eq!(owner.view.draft, Some(SocdMode::First));
        assert!(owner.next().is_none());
        assert!(owner.storage_finished(&request, false, true));
        assert!(owner.next().is_none());
        assert!(owner.view.error.is_some());
        let request = owner.prepare(SocdMode::Last).unwrap();
        owner.storage_finished(&request, true, true);
        assert_eq!(owner.view.persisted, Some(SocdMode::Last));
        assert_eq!(owner.next().unwrap().operation.mode, SocdMode::Last);
    }
    #[test]
    fn controller_socd_retains_running_task_and_only_the_latest_saved_pending_task() {
        let mut owner = state();
        let a = owner.prepare(SocdMode::First).unwrap();
        owner.storage_finished(&a, true, true);
        let active = owner.next().unwrap();
        let b = owner.prepare(SocdMode::Last).unwrap();
        owner.storage_finished(&b, true, true);
        let c = owner.prepare(SocdMode::Neutral).unwrap();
        owner.storage_finished(&c, true, true);
        assert!(active.is_valid());
        assert!(owner.next().is_none());
        assert!(owner.finished(&active, Ok(acknowledged())));
        let next = owner.next().unwrap();
        assert_eq!(next.sequence, c.sequence);
        assert_ne!(next.sequence, b.sequence);
        assert_eq!(owner.view.draft, Some(SocdMode::Neutral));
        assert_eq!(owner.view.persisted, Some(SocdMode::Neutral));
    }
    #[test]
    fn controller_socd_profile_switch_reconnect_and_exit_reject_old_device_results() {
        for replacement in [
            Some(scope("another-profile", 4, 1)),
            Some(scope("real-profile", 3, 2)),
            None,
        ] {
            let mut owner = state();
            let save = owner.prepare(SocdMode::First).unwrap();
            owner.storage_finished(&save, true, true);
            let active = owner.next().unwrap();
            owner.bind(replacement, Some(SocdMode::Last));
            assert!(!active.is_valid());
            assert!(!owner.finished(&active, Ok(acknowledged())));
            assert!(owner.view.result.is_none());
            assert!(owner.view.draft.is_none());
        }
    }
    #[test]
    fn controller_socd_acknowledgement_is_scoped_to_profile_identity_even_when_slots_match() {
        let mut owner = state();
        let save = owner.prepare(SocdMode::First).unwrap();
        owner.storage_finished(&save, true, true);
        let active = owner.next().unwrap();
        owner.finished(&active, Ok(acknowledged()));
        assert!(owner.view.result.is_some());
        owner.bind(
            Some(scope("another-profile", 3, 1)),
            Some(SocdMode::Neutral),
        );
        assert!(owner.view.result.is_none());
        owner.bind(Some(scope("real-profile", 3, 1)), Some(SocdMode::First));
        assert!(owner.view.result.as_ref().unwrap().1.acknowledged());
        owner.bind(Some(scope("real-profile", 3, 2)), Some(SocdMode::First));
        assert!(owner.view.result.is_none());
    }
    #[test]
    fn controller_socd_explicit_draft_and_real_failure_survive_profile_roundtrip_but_not_reconnect()
    {
        let mut owner = state();
        let save = owner.prepare(SocdMode::First).unwrap();
        owner.storage_finished(&save, true, true);
        let active = owner.next().unwrap();
        owner.finished(
            &active,
            Ok(SocdResult {
                write: SocdWriteStatus::Failed {
                    error: "actual SET rejected".into(),
                },
                canceled: false,
            }),
        );
        owner.bind(
            Some(scope("another-profile", 3, 1)),
            Some(SocdMode::Neutral),
        );
        assert!(owner.view.draft.is_none());
        assert!(owner.view.result.is_none());
        assert!(owner.view.error.is_none());
        owner.bind(Some(scope("real-profile", 3, 1)), Some(SocdMode::First));
        assert_eq!(owner.view.draft, Some(SocdMode::First));
        assert_eq!(owner.view.error.as_deref(), Some("actual SET rejected"));
        assert!(!owner.view.result.as_ref().unwrap().1.acknowledged());
        owner.bind(Some(scope("real-profile", 3, 2)), Some(SocdMode::First));
        assert!(owner.view.draft.is_none());
        assert!(owner.view.result.is_none());
        assert!(owner.view.error.is_none());
    }
    #[test]
    fn controller_socd_older_storage_failure_cannot_replace_the_latest_action_status() {
        for newest_first in [false, true] {
            let mut owner = state();
            let old = owner.prepare(SocdMode::First).unwrap();
            let latest = owner.prepare(SocdMode::Last).unwrap();
            if newest_first {
                owner.storage_finished(&latest, true, true);
            }
            owner.storage_finished(&old, false, true);
            if !newest_first {
                owner.storage_finished(&latest, true, true);
            }
            assert_eq!(owner.view.draft, Some(SocdMode::Last));
            assert_eq!(owner.view.persisted, Some(SocdMode::Last));
            assert!(owner.view.error.is_none());
            assert!(!owner.view.saving);
            assert_eq!(owner.next().unwrap().sequence, latest.sequence);
        }
    }
    #[test]
    fn controller_socd_saved_action_returning_to_its_profile_never_reuses_the_canceled_device_guard()
     {
        let mut owner = state();
        let save = owner.prepare(SocdMode::First).unwrap();
        owner.bind(
            Some(scope("another-profile", 4, 1)),
            Some(SocdMode::Neutral),
        );
        owner.bind(Some(scope("real-profile", 3, 1)), Some(SocdMode::Neutral));
        assert_eq!(owner.view.draft, Some(SocdMode::First));
        assert!(owner.view.saving);
        assert!(!save.is_valid());
        owner.storage_finished(&save, true, true);
        assert_eq!(owner.view.persisted, Some(SocdMode::First));
        assert_eq!(owner.view.draft, Some(SocdMode::First));
        assert!(owner.next().is_none());
        assert!(owner.view.result.is_none());
    }
    #[test]
    fn controller_socd_storage_completion_after_disconnect_never_submits_a_device_write() {
        let mut owner = state();
        let save = owner.prepare(SocdMode::First).unwrap();
        owner.invalidate();
        assert!(
            owner.matches(&save),
            "Already authorized storage is independently owned"
        );
        assert!(owner.storage_finished(&save, true, true));
        assert!(owner.next().is_none());
        assert!(!save.is_valid());
        let mut owner = state();
        let save = owner.prepare(SocdMode::Last).unwrap();
        owner.storage_finished(&save, true, false);
        assert_eq!(owner.view.persisted, Some(SocdMode::Last));
        assert!(owner.next().is_none());
        assert!(owner.view.result.is_none());
    }
    #[test]
    fn controller_socd_failed_or_canceled_set_never_rolls_back_persisted_or_draft() {
        for result in [
            SocdResult {
                write: SocdWriteStatus::Failed {
                    error: "real SET rejection".into(),
                },
                canceled: false,
            },
            SocdResult {
                write: SocdWriteStatus::NotAttempted,
                canceled: true,
            },
        ] {
            let mut owner = state();
            let save = owner.prepare(SocdMode::First).unwrap();
            owner.storage_finished(&save, true, true);
            let active = owner.next().unwrap();
            owner.finished(&active, Ok(result));
            assert_eq!(owner.view.persisted, Some(SocdMode::First));
            assert_eq!(owner.view.draft, Some(SocdMode::First));
            assert!(owner.view.error.is_some());
            assert!(!owner.view.result.as_ref().unwrap().1.acknowledged());
        }
    }
    #[test]
    fn controller_socd_cleanup_barrier_holds_pending_until_real_release() {
        let mut owner = state();
        let a = owner.prepare(SocdMode::First).unwrap();
        owner.storage_finished(&a, true, true);
        let active = owner.next().unwrap();
        let b = owner.prepare(SocdMode::Last).unwrap();
        owner.storage_finished(&b, true, true);
        active.set_cleanup_pending(true);
        owner.finished(&active, Ok(acknowledged()));
        assert!(owner.next().is_none());
        active.set_cleanup_pending(false);
        owner.finished(&active, Ok(acknowledged()));
        assert_eq!(owner.next().unwrap().sequence, b.sequence);
    }
    #[test]
    fn controller_socd_profile_boundary_requires_an_explicit_observed_slot_and_preserves_unknown_extensions()
     {
        let mut profile = Profile {
            id: "stable-profile".into(),
            guid: "source-guid".into(),
            name: "name".into(),
            settings: None,
            source_settings: Some(
                serde_json::json!({"profile":{"obmSlotId":[0],"controllerDPadSOCD":"last","extension":{"kept":true}}}),
            ),
            dpi_stages: None,
        };
        assert_eq!(
            profile_settings(&profile).unwrap(),
            (0, Some(SocdMode::Last))
        );
        assert_eq!(
            razer_device::controller_profile::wire_slot(profile_settings(&profile).unwrap().0),
            1
        );
        for slots in [
            serde_json::json!([]),
            serde_json::json!(null),
            serde_json::json!([256]),
            serde_json::json!(["1"]),
        ] {
            profile.source_settings.as_mut().unwrap()["profile"]["obmSlotId"] = slots;
            assert!(profile_settings(&profile).is_err());
        }
        profile.source_settings.as_mut().unwrap()["profile"]
            .as_object_mut()
            .unwrap()
            .remove("obmSlotId");
        assert!(profile_settings(&profile).is_err());
        assert_eq!(
            profile.source_settings.as_ref().unwrap()["profile"]["extension"],
            serde_json::json!({"kept":true})
        );
    }
    #[test]
    fn controller_socd_saved_projection_targets_the_bound_profile_and_preserves_latest_unrelated_edits()
     {
        let mut owner = state();
        let request = owner.prepare(SocdMode::First).unwrap();
        let profile = |id: &str, slot| Profile {
            id: id.into(),
            guid: format!("source-{id}"),
            name: format!("latest-{id}"),
            settings: None,
            source_settings: Some(serde_json::json!({
                "profile":{"obmSlotId":[slot],"controllerDPadSOCD":"neutral","pollingRate":1000,"future":{"v":9}},
                "controller":{"dpad":{"socdSettings":{"value":"neutral","extension":"keep"},"other":true},"leftStick":{"deadzone":17}},
                "futureRoot":[1,2,3]
            })),
            dpi_stages: None,
        };
        let mut device = razer_model::product_preview::registered_preview(2676).unwrap();
        device.profiles = vec![profile("real-profile", 3), profile("other", 4)];
        device.active_profile = "other".into();
        device.source_device_settings = Some(serde_json::json!({"latestRoot":{"preserve":true}}));
        let before = serde_json::to_value(&device).unwrap();
        request.project_saved(&mut device).unwrap();
        let mut expected = before.clone();
        expected["profiles"][0]["source_settings"]["profile"]["controllerDPadSOCD"] =
            serde_json::json!("first");
        expected["profiles"][0]["source_settings"]["controller"]["dpad"]["socdSettings"]["value"] =
            serde_json::json!("first");
        assert_eq!(serde_json::to_value(&device).unwrap(), expected);
        device.profiles[0].source_settings.as_mut().unwrap()["profile"]["obmSlotId"] =
            serde_json::json!([5]);
        let before_rejected = serde_json::to_value(&device).unwrap();
        assert!(request.project_saved(&mut device).is_err());
        assert_eq!(serde_json::to_value(&device).unwrap(), before_rejected);
        device.profiles[0].source_settings.as_mut().unwrap()["profile"]["obmSlotId"] =
            serde_json::json!([3]);
        device.profiles[0].source_settings.as_mut().unwrap()["controller"]["dpad"] =
            serde_json::json!("malformed");
        let before_rejected = serde_json::to_value(&device).unwrap();
        assert!(request.project_saved(&mut device).is_err());
        assert_eq!(serde_json::to_value(&device).unwrap(), before_rejected);
    }
}

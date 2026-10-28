//! Audited Matcher uS -> NS ownership; current AS/dS reducers and service
//! observations.
//! UI drafts and transport failures are kept apart from device acknowledgements.
use super::*;
use crate::features::mouse_dpi_matcher::{MatcherAction, MatcherObservation, MouseDpiMatcher};
use anyhow::{Context as _, ensure};
use razer_model::model::Device;

/// Only the durable Matcher library and its profile-local selection are copied
/// back into retained drafts. Device observations and unrelated edits stay owned
/// by their existing page state.
pub(in crate::features) struct SavedMatcherProfiles {
    product_id: u32,
    device_identity: String,
    profile_id: String,
    profile_guid: String,
    library: Value,
    selected: Option<String>,
}
impl SavedMatcherProfiles {
    pub(in crate::features) fn from_device(device: &Device) -> anyhow::Result<Self> {
        let product = razer_device::dpi_matcher::Product::new(device.product_id)
            .context("Matcher saved profiles belong to an unaudited product")?;
        let profile = device
            .profiles
            .iter()
            .find(|profile| profile.id == device.active_profile)
            .context("Matcher saved active profile is missing")?;
        ensure!(
            !profile.guid.is_empty(),
            "Matcher saved profile GUID is empty"
        );
        let library = device
            .source_device_settings
            .as_ref()
            .and_then(|root| root.get("sensitivityMatcher"))
            .context("Matcher saved library is missing")?
            .clone();
        #[derive(Deserialize)]
        struct Library {
            profiles: Vec<Value>,
        }
        let Library {
            profiles: _profiles,
        } = serde_json::from_value(library.clone())?;
        let source = profile
            .source_settings
            .as_ref()
            .and_then(Value::as_object)
            .context("Matcher saved profile settings is not an object")?;
        let selected = source
            .get("selectedSensitivityMatcher")
            .map(|selected| {
                selected
                    .as_str()
                    .context("Matcher saved selection is not a GUID")
                    .map(str::to_owned)
            })
            .transpose()?;
        Ok(Self {
            product_id: product.product_id(),
            device_identity: device.workspace_identity(),
            profile_id: profile.id.clone(),
            profile_guid: profile.guid.clone(),
            library,
            selected,
        })
    }

    pub(in crate::features) fn apply_to_device(&self, device: &mut Device) -> anyhow::Result<()> {
        let product = razer_device::dpi_matcher::Product::new(device.product_id)
            .context("Matcher saved device belongs to an unaudited product")?;
        ensure!(
            product.product_id() == self.product_id
                && device.workspace_identity() == self.device_identity
                && device.active_profile == self.profile_id,
            "Matcher saved device or active profile changed"
        );
        let mut candidate = device.clone();
        let profile = candidate
            .profiles
            .iter_mut()
            .find(|profile| profile.id == self.profile_id && profile.guid == self.profile_guid)
            .context("Matcher saved profile owner changed")?;
        let source = profile
            .source_settings
            .as_mut()
            .and_then(Value::as_object_mut)
            .context("Matcher live profile settings is not an object")?;
        self.apply_selection(source);
        let root = candidate
            .source_device_settings
            .as_mut()
            .and_then(Value::as_object_mut)
            .context("Matcher live device settings is not an object")?;
        root.insert("sensitivityMatcher".into(), self.library.clone());
        *device = candidate;
        Ok(())
    }

    fn apply_to_draft(&self, draft: &mut Value) -> anyhow::Result<()> {
        let draft = draft
            .as_object_mut()
            .context("Matcher retained draft is not an object")?;
        draft.insert("sensitivityMatcher".into(), self.library.clone());
        self.apply_selection(draft);
        Ok(())
    }

    fn apply_selection(&self, target: &mut serde_json::Map<String, Value>) {
        match &self.selected {
            Some(selected) => {
                target.insert("selectedSensitivityMatcher".into(), json!(selected));
            }
            None => {
                target.remove("selectedSensitivityMatcher");
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct MouseMatcherScope {
    owner: EntityId,
    epoch: u64,
    valid: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl PartialEq for MouseMatcherScope {
    fn eq(&self, other: &Self) -> bool {
        self.retained_by(other.owner, other.epoch, &other.valid)
    }
}
impl Eq for MouseMatcherScope {}
impl MouseMatcherScope {
    pub fn is_valid(&self) -> bool {
        self.valid.load(std::sync::atomic::Ordering::Acquire)
    }

    fn retained_by(
        &self,
        owner: EntityId,
        epoch: u64,
        valid: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> bool {
        self.owner == owner && self.epoch == epoch && std::sync::Arc::ptr_eq(&self.valid, valid)
    }
}

#[derive(Clone, Debug)]
pub struct MouseMatcherRequest {
    scope: MouseMatcherScope,
    action: MatcherAction,
    profile_payload: Value,
}
impl MouseMatcherRequest {
    pub fn scope(&self) -> &MouseMatcherScope {
        &self.scope
    }
    pub fn action(&self) -> &MatcherAction {
        &self.action
    }
    /// Exact ON_UPDATE_SENSITIVITY_PROFILE payload for select/rename.
    pub fn profile_payload(&self) -> &Value {
        &self.profile_payload
    }
}
impl EventEmitter<MouseMatcherRequest> for MouseProductWorkspace {}

impl MouseProductWorkspace {
    pub(super) fn prepare_matcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // This view implements PID190 dS/AS. Sampling/storage support for other
        // products does not establish their popup, menu or resource lifecycle.
        if self.spec.product_id != razer_device::dpi_matcher::PRODUCT_ID {
            return;
        }
        let matcher = cx.new(|cx| MouseDpiMatcher::new(window, cx));
        self.subscriptions.push(cx.subscribe_in(
            &matcher,
            window,
            |this, matcher, action: &MatcherAction, window, cx| {
                if !this.active || this.page != "TAB_PERFORMANCE" {
                    return;
                }
                let payload = matcher.read(cx).state().profile_payload();
                match action {
                    MatcherAction::SetStageValueX { stages, active } => {
                        set_pointer(&mut this.draft, this.spec.stages_path(), json!(stages));
                        set_pointer(&mut this.draft, this.spec.active_path(), json!(active));
                        this.dismiss_dpi_editors(window, cx);
                        cx.emit(MouseProductChanged);
                    }
                    MatcherAction::SelectProfile(_) | MatcherAction::RenameProfile(_) => {
                        this.draft["sensitivityMatcher"] = payload["sensitivityMatcher"].clone();
                        this.draft["selectedSensitivityMatcher"] =
                            payload["selectedSensitivityMatcher"].clone();
                        cx.emit(MouseProductChanged);
                    }
                    MatcherAction::Command(_) => {}
                }
                this.matcher_error = None;
                cx.emit(MouseMatcherRequest {
                    scope: this
                        .mouse_matcher_scope(cx)
                        .expect("retained matcher owner"),
                    action: action.clone(),
                    profile_payload: payload,
                });
                cx.notify();
            },
        ));
        self.matcher = Some(matcher);
        self.sync_matcher_profile(window, cx);
    }

    pub(super) fn sync_matcher_mount(&mut self, cx: &mut Context<Self>) {
        let mounted = self.active && self.page == "TAB_PERFORMANCE";
        if mounted
            && !self
                .matcher_valid
                .load(std::sync::atomic::Ordering::Acquire)
        {
            self.matcher_epoch = self.matcher_epoch.wrapping_add(1);
            self.matcher_valid = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        } else if !mounted {
            self.matcher_valid
                .store(false, std::sync::atomic::Ordering::Release);
        }
        if let Some(matcher) = &self.matcher {
            matcher.update(cx, |matcher, cx| {
                if mounted {
                    matcher.mount(cx);
                } else {
                    matcher.unmount(cx);
                }
            });
        }
    }

    pub(super) fn sync_matcher_profile(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(matcher) = &self.matcher else {
            return;
        };
        let stages = self
            .draft
            .pointer(self.spec.stages_path())
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let active = self
            .draft
            .pointer(self.spec.active_path())
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let profiles = self
            .draft
            .pointer("/sensitivityMatcher/profiles")
            .and_then(Value::as_array)
            .cloned();
        let selected = self.draft["selectedSensitivityMatcher"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        matcher.update(cx, |matcher, cx| {
            matcher.observe(MatcherObservation::Stages { stages, active }, window, cx);
            matcher.restore_profiles(profiles, selected, window, cx);
        });
    }

    pub fn mouse_matcher_scope(&self, _cx: &App) -> Option<MouseMatcherScope> {
        if !self.active || self.page != "TAB_PERFORMANCE" {
            return None;
        }
        Some(MouseMatcherScope {
            owner: self.matcher.as_ref()?.entity_id(),
            epoch: self.matcher_epoch,
            valid: self.matcher_valid.clone(),
        })
    }

    /// An admitted durable request keeps its exact owner across exit unmount.
    /// A remount or profile restoration replaces the epoch/guard identity.
    pub fn retains_mouse_matcher_scope(&self, scope: &MouseMatcherScope) -> bool {
        self.matcher.as_ref().is_some_and(|matcher| {
            scope.retained_by(matcher.entity_id(), self.matcher_epoch, &self.matcher_valid)
        })
    }

    pub(in crate::features) fn accept_mouse_matcher_saved_profiles(
        &mut self,
        scope: &MouseMatcherScope,
        saved: &SavedMatcherProfiles,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        ensure!(
            self.retains_mouse_matcher_scope(scope),
            "Matcher saved page owner changed"
        );
        saved.apply_to_draft(&mut self.draft)?;
        cx.notify();
        Ok(())
    }

    pub fn sync_mouse_matcher_dpi(&self, cx: &mut Context<Self>) {
        let Some(matcher) = &self.matcher else {
            return;
        };
        let stages = self
            .draft
            .pointer(self.spec.stages_path())
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let active = self
            .draft
            .pointer(self.spec.active_path())
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        matcher.update(cx, |matcher, cx| matcher.observe_stages(stages, active, cx));
    }

    /// Real response ingress. Older profile/device scopes cannot alter the UI.
    pub fn observe_mouse_matcher(
        &mut self,
        scope: &MouseMatcherScope,
        observation: MatcherObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !scope.is_valid() || self.mouse_matcher_scope(cx).as_ref() != Some(scope) {
            return;
        }
        if let MatcherObservation::Profiles { profiles, selected } = &observation {
            self.draft["sensitivityMatcher"] = json!({"profiles":profiles});
            self.draft["selectedSensitivityMatcher"] = json!(selected);
            cx.emit(MouseProductChanged);
        }
        if let MatcherObservation::Stages { stages, active } = &observation {
            set_pointer(&mut self.draft, self.spec.stages_path(), json!(stages));
            set_pointer(&mut self.draft, self.spec.active_path(), json!(active));
            self.dismiss_dpi_editors(window, cx);
            cx.emit(MouseProductChanged);
        }
        if let Some(matcher) = &self.matcher {
            matcher.update(cx, |matcher, cx| matcher.observe(observation, window, cx));
        }
        cx.notify();
    }

    pub fn reject_mouse_matcher(
        &mut self,
        scope: &MouseMatcherScope,
        reason: String,
        cx: &mut Context<Self>,
    ) {
        if scope.is_valid() && self.mouse_matcher_scope(cx).as_ref() == Some(scope) {
            // A missing transport is not the source calibration-error response.
            self.matcher_error = Some(reason);
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MouseMatcherScope, SavedMatcherProfiles};
    use crate::features::mouse_dpi_matcher::{
        MatcherAction, MatcherCommand, MatcherObservation, MatcherState,
    };
    use gpui_kit::EntityId;
    use razer_model::model::{Device, Profile};
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn matcher_exit_unmount_keeps_admitted_identity_without_reopening_ui_scope() {
        let valid = Arc::new(AtomicBool::new(true));
        let scope = MouseMatcherScope {
            owner: EntityId::from(1),
            epoch: 4,
            valid: valid.clone(),
        };
        valid.store(false, Ordering::Release);
        assert!(!scope.is_valid());
        assert!(scope.retained_by(EntityId::from(1), 4, &valid));
    }

    #[test]
    fn matcher_exit_remount_or_profile_successor_never_matches_old_owner_guard() {
        let valid = Arc::new(AtomicBool::new(false));
        let scope = MouseMatcherScope {
            owner: EntityId::from(1),
            epoch: 4,
            valid: valid.clone(),
        };
        let successor = Arc::new(AtomicBool::new(true));
        assert!(!scope.retained_by(EntityId::from(1), 4, &successor));
        assert!(!scope.retained_by(EntityId::from(1), 5, &valid));
        assert!(!scope.retained_by(EntityId::from(2), 4, &valid));
        let same_count_other_owner = MouseMatcherScope {
            owner: EntityId::from(2),
            epoch: 4,
            valid,
        };
        assert_ne!(scope, same_count_other_owner);
    }

    fn saved_device_for(product_id: u32) -> Device {
        let mut device = razer_model::product_preview::registered_preview(product_id).unwrap();
        device.active_profile = "test-profile".into();
        device.profiles = vec![Profile {
            id: device.active_profile.clone(),
            guid: device.active_profile.clone(),
            name: "Test profile".into(),
            source_settings: None,
            settings: None,
            dpi_stages: None,
        }];
        device.profiles[0].source_settings = Some(json!({
            "guid":device.profiles[0].guid,
            "selectedSensitivityMatcher":"old",
            "dpiStages":{"active":1,"enable":true,"stages":[{"x":800,"y":900,"visible":true,"independent":true}]},
            "pollingRate":1000,
            "profileExtension":{"keep":true}
        }));
        device.source_device_settings = Some(json!({
            "version":7,
            "deviceExtension":{"keep":true},
            "sensitivityMatcher":{"profiles":[{"guid":"old","name":"Previous","dpi":{"x":800,"y":800}}],"libraryExtension":"keep"}
        }));
        device
    }

    fn saved_device() -> Device {
        saved_device_for(190)
    }

    fn commit(device: &mut Device, mutation: razer_device::dpi_matcher_storage::Mutation) {
        let before = razer_model::mouse_matcher::source_record(device).unwrap();
        let mutation =
            razer_model::mouse_matcher::Mutation::from_source(&before, &mutation.data).unwrap();
        mutation.apply(device).unwrap();
    }

    #[test]
    fn matcher_exit_confirmed_save_restores_latest_profiles_without_replaying_device_work() {
        let mut live = saved_device();
        let mut saved = live.clone();
        let source = razer_model::mouse_matcher::source_record(&saved).unwrap();
        let confirmed = razer_device::dpi_matcher_storage::confirm(
            source.document(),
            1450.,
            "measured",
            json!({"en":"Measured mouse"}),
        )
        .unwrap();
        commit(&mut saved, confirmed);
        live.profiles[0].source_settings.as_mut().unwrap()["pollingRate"] = json!(500);
        let live_stages = live.profiles[0].source_settings.as_ref().unwrap()["dpiStages"].clone();
        let mut draft = json!({
            "sensitivityMatcher":{"profiles":[{"guid":"old"}]},
            "selectedSensitivityMatcher":"old",
            "pollingRate":500,
            "dpiStages":{"active":1,"stages":[{"x":2100,"y":2200}]},
            "unsubmitted":"keep"
        });
        let draft_stages = draft["dpiStages"].clone();
        let profiles = SavedMatcherProfiles::from_device(&saved).unwrap();
        profiles.apply_to_device(&mut live).unwrap();
        profiles.apply_to_draft(&mut draft).unwrap();
        assert_eq!(draft["selectedSensitivityMatcher"], "measured");
        assert_eq!(
            draft["sensitivityMatcher"]["profiles"][1]["guid"],
            "measured"
        );
        assert_eq!(draft["unsubmitted"], "keep");
        assert_eq!(draft["pollingRate"], 500);
        assert_eq!(draft["dpiStages"], draft_stages);
        assert_eq!(
            live.profiles[0].source_settings.as_ref().unwrap()["dpiStages"],
            live_stages
        );
        assert_eq!(
            live.profiles[0].source_settings.as_ref().unwrap()["pollingRate"],
            500
        );
        assert_eq!(live.source_device_settings.as_ref().unwrap()["version"], 7);
        assert_eq!(
            live.source_device_settings.as_ref().unwrap()["deviceExtension"],
            json!({"keep":true})
        );
        assert_eq!(
            live.profiles[0].source_settings.as_ref().unwrap()["selectedSensitivityMatcher"],
            "measured"
        );

        let mut state = MatcherState::default();
        state.unmount();
        let actions = state.observe_unmounted(MatcherObservation::Profiles {
            profiles: draft["sensitivityMatcher"]["profiles"]
                .as_array()
                .unwrap()
                .clone(),
            selected: draft["selectedSensitivityMatcher"]
                .as_str()
                .unwrap()
                .to_owned(),
        });
        assert!(actions.is_empty());
        assert_eq!(state.selected(), "measured");
        assert_eq!(
            state.profile_payload()["sensitivityMatcher"],
            draft["sensitivityMatcher"]
        );
        assert_eq!(
            state.mount(),
            vec![MatcherAction::Command(MatcherCommand::GetState)]
        );
    }

    #[test]
    fn matcher_exit_deleted_selection_and_later_saved_library_replace_stale_retained_rows() {
        let mut live = saved_device();
        let mut saved = live.clone();
        let source = razer_model::mouse_matcher::source_record(&saved).unwrap();
        commit(
            &mut saved,
            razer_device::dpi_matcher_storage::delete(source.document(), "old").unwrap(),
        );
        let mut draft = json!({"sensitivityMatcher":{"profiles":[{"guid":"old"}]},"selectedSensitivityMatcher":"old","edit":"keep"});
        let deleted = SavedMatcherProfiles::from_device(&saved).unwrap();
        deleted.apply_to_device(&mut live).unwrap();
        deleted.apply_to_draft(&mut draft).unwrap();
        assert_eq!(draft["sensitivityMatcher"]["profiles"], json!([]));
        assert!(draft.get("selectedSensitivityMatcher").is_none());
        assert!(
            live.profiles[0]
                .source_settings
                .as_ref()
                .unwrap()
                .get("selectedSensitivityMatcher")
                .is_none()
        );
        let source = razer_model::mouse_matcher::source_record(&saved).unwrap();
        commit(
            &mut saved,
            razer_device::dpi_matcher_storage::confirm(
                source.document(),
                1700.,
                "later",
                json!("Later"),
            )
            .unwrap(),
        );
        let latest = SavedMatcherProfiles::from_device(&saved).unwrap();
        latest.apply_to_device(&mut live).unwrap();
        latest.apply_to_draft(&mut draft).unwrap();
        assert_eq!(draft["selectedSensitivityMatcher"], "later");
        assert_eq!(
            draft["sensitivityMatcher"]["profiles"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(draft["sensitivityMatcher"]["profiles"][0]["guid"], "later");
        assert_eq!(draft["edit"], "keep");
    }

    #[test]
    fn matcher_exit_saved_profiles_reject_replaced_owners_and_invalid_records_without_fallback() {
        let saved = saved_device();
        let projection = SavedMatcherProfiles::from_device(&saved).unwrap();
        for change in 0..5 {
            let mut live = saved.clone();
            match change {
                0 => live.device_container_id = "replacement-device".into(),
                1 => live.active_profile = "replacement-profile".into(),
                2 => live.profiles[0].guid = "replacement-guid".into(),
                3 => live.product_id = 192,
                _ => live.source_device_settings = Some(Value::Null),
            }
            let unchanged = serde_json::to_value(&live).unwrap();
            assert!(projection.apply_to_device(&mut live).is_err());
            assert_eq!(serde_json::to_value(live).unwrap(), unchanged);
        }
        for invalid in [json!(null), json!({}), json!({"profiles":"invalid"})] {
            let mut saved = saved.clone();
            saved.source_device_settings.as_mut().unwrap()["sensitivityMatcher"] = invalid;
            assert!(SavedMatcherProfiles::from_device(&saved).is_err());
        }
        let mut saved = saved;
        saved.profiles[0].source_settings.as_mut().unwrap()["selectedSensitivityMatcher"] =
            Value::Null;
        assert!(SavedMatcherProfiles::from_device(&saved).is_err());
    }

    #[test]
    fn matcher_saved_profiles_accept_each_audited_product_without_fallback() {
        for product_id in [190, 192, 222, 226, 229, 239] {
            let saved = saved_device_for(product_id);
            let projection = SavedMatcherProfiles::from_device(&saved).unwrap();
            let mut live = saved.clone();
            projection.apply_to_device(&mut live).unwrap();
            assert_eq!(live.product_id, product_id);
        }
    }
}

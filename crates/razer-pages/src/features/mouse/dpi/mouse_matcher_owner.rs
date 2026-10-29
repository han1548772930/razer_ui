//! Audited Matcher uS -> NS ownership; current AS/dS reducers and service
//! observations.
//! UI drafts and transport failures are kept apart from device acknowledgements.
use super::*;
use crate::features::mouse_dpi_matcher::{
    MatcherAction, MatcherCommand, MatcherObservation, MouseDpiMatcher,
};
use anyhow::{Context as _, ensure};
use razer_device::{
    dpi_matcher::Product,
    dpi_matcher_storage::{Library, ProfileUpdate},
    mouse_current_dpi::{SourceStage, SourceStages},
};
use razer_model::model::Device;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MatcherColumn {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MatcherUiSpec {
    product: Product,
    column: MatcherColumn,
}

impl MatcherUiSpec {
    pub(super) fn product(self) -> Product {
        self.product
    }

    pub(super) fn column(self) -> MatcherColumn {
        self.column
    }
}

/// Mounted Performance branches share the Matcher panel and popup, but retain
/// their source column order. The PID226 Matcher precedes system properties.
pub(super) fn source_spec(product_id: u32) -> Option<MatcherUiSpec> {
    let column = match product_id {
        190 | 192 | 222 | 226 => MatcherColumn::Right,
        229 | 239 => MatcherColumn::Left,
        _ => return None,
    };
    Some(MatcherUiSpec {
        product: Product::new(product_id).expect("reviewed Matcher UI product"),
        column,
    })
}

pub(in crate::features) fn device_fields(product_id: u32) -> &'static [&'static str] {
    if source_spec(product_id).is_some() {
        &["sensitivityMatcher"]
    } else {
        &[]
    }
}

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
    kind: ScopeKind,
    domain: MousePollingScope,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScopeKind {
    Ui,
    Normalization,
}
impl PartialEq for MouseMatcherScope {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.domain == other.domain
            && self.retained_by(other.owner, other.epoch, &other.valid)
    }
}
impl Eq for MouseMatcherScope {}
impl MouseMatcherScope {
    pub fn is_valid(&self) -> bool {
        self.valid.load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn is_normalization(&self) -> bool {
        self.kind == ScopeKind::Normalization
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

pub(super) struct NormalizationState {
    pending: Option<PendingNormalization>,
    epoch: u64,
    valid: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ready: bool,
}
struct PendingNormalization {
    domain: MousePollingScope,
    payload: ProfileUpdate,
    submitted: bool,
}
impl Default for NormalizationState {
    fn default() -> Self {
        Self {
            pending: None,
            epoch: 0,
            valid: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ready: false,
        }
    }
}
impl NormalizationState {
    fn retain(&mut self, domain: MousePollingScope, payload: ProfileUpdate) {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.domain == domain && pending.payload == payload)
        {
            return;
        }
        self.suspend();
        self.epoch = self.epoch.wrapping_add(1);
        self.valid = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(self.ready));
        self.pending = Some(PendingNormalization {
            domain,
            payload,
            submitted: false,
        });
    }

    fn activate(&mut self) {
        if !self.valid.load(std::sync::atomic::Ordering::Acquire) {
            self.epoch = self.epoch.wrapping_add(1);
            self.valid = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        }
    }

    fn bind(&mut self) {
        self.ready = true;
        self.activate();
    }

    pub(super) fn suspend(&mut self) {
        self.valid
            .store(false, std::sync::atomic::Ordering::Release);
        if let Some(pending) = &mut self.pending {
            pending.submitted = false;
        }
    }

    pub(super) fn reset(&mut self) {
        self.suspend();
        self.pending = None;
    }

    fn request(
        &mut self,
        owner: EntityId,
        domain: MousePollingScope,
    ) -> Option<MouseMatcherRequest> {
        if !self.ready || !self.valid.load(std::sync::atomic::Ordering::Acquire) {
            return None;
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.domain != domain)
        {
            self.pending = None;
        }
        let pending = self.pending.as_mut().filter(|pending| !pending.submitted)?;
        pending.submitted = true;
        Some(MouseMatcherRequest {
            scope: MouseMatcherScope {
                owner,
                epoch: self.epoch,
                valid: self.valid.clone(),
                kind: ScopeKind::Normalization,
                domain,
            },
            action: MatcherAction::NormalizeProfiles,
            profile_payload: pending.payload.clone(),
        })
    }

    fn acknowledge(&mut self, saved: &SavedMatcherProfiles) {
        if self.pending.as_ref().is_some_and(|pending| {
            pending.payload.library().file_value() == saved.library
                && pending.payload.selected() == saved.selected.as_deref()
        }) {
            self.pending = None;
        }
    }

    fn retry(&mut self) {
        if let Some(pending) = &mut self.pending {
            pending.submitted = false;
        }
    }
}
impl Drop for NormalizationState {
    fn drop(&mut self) {
        self.suspend();
    }
}

#[derive(Clone, Debug)]
pub struct MouseMatcherRequest {
    scope: MouseMatcherScope,
    action: MatcherAction,
    profile_payload: ProfileUpdate,
}
impl MouseMatcherRequest {
    pub fn scope(&self) -> &MouseMatcherScope {
        &self.scope
    }
    pub fn action(&self) -> &MatcherAction {
        &self.action
    }
    /// Exact ON_UPDATE_SENSITIVITY_PROFILE payload for select/rename.
    pub fn profile_payload(&self) -> &ProfileUpdate {
        &self.profile_payload
    }
}
impl EventEmitter<MouseMatcherRequest> for MouseProductWorkspace {}

impl MouseProductWorkspace {
    pub(super) fn prepare_matcher(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(spec) = source_spec(self.spec.product_id) else {
            return;
        };
        let matcher = cx.new(|cx| {
            MouseDpiMatcher::new(spec.product(), self.spec.block_widget_columns(), window, cx)
        });
        self.subscriptions.push(cx.subscribe_in(
            &matcher,
            window,
            |this, matcher, action: &MatcherAction, window, cx| {
                let payload = matcher.read(cx).state().profile_update();
                if matches!(action, MatcherAction::NormalizeProfiles) {
                    this.draft["sensitivityMatcher"] = payload.library().file_value();
                    this.draft["selectedSensitivityMatcher"] = json!(payload.selected());
                    if let Some(domain) = this.mouse_polling_scope(cx) {
                        this.matcher_normalization.retain(domain, payload);
                    }
                    this.matcher_error = None;
                    cx.emit(MouseProductChanged);
                    this.flush_matcher_normalization(cx);
                    cx.notify();
                    return;
                }
                if !this.active || this.page != "TAB_PERFORMANCE" {
                    return;
                }
                if matches!(
                    action,
                    MatcherAction::SelectProfile(_)
                        | MatcherAction::RenameProfile(_)
                        | MatcherAction::Command(
                            MatcherCommand::Confirm
                                | MatcherCommand::DeleteProfile(_)
                                | MatcherCommand::DeleteAllProfiles
                        )
                ) {
                    this.matcher_normalization.reset();
                }
                match action {
                    MatcherAction::SetStageValueX { stages, active } => {
                        set_pointer(
                            &mut this.draft,
                            this.spec.stages_path(),
                            Value::Array(stages.iter().map(SourceStage::file_value).collect()),
                        );
                        set_pointer(&mut this.draft, this.spec.active_path(), json!(active));
                        this.dismiss_dpi_editors(window, cx);
                        cx.emit(MouseProductChanged);
                    }
                    MatcherAction::SelectProfile(_) | MatcherAction::RenameProfile(_) => {
                        this.draft["sensitivityMatcher"] = payload.library().file_value();
                        this.draft["selectedSensitivityMatcher"] = json!(payload.selected());
                        cx.emit(MouseProductChanged);
                    }
                    MatcherAction::Command(_) => {}
                    MatcherAction::NormalizeProfiles => unreachable!("retained above"),
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

    pub(super) fn flush_matcher_normalization(&mut self, cx: &mut Context<Self>) {
        let Some(owner) = self.matcher.as_ref().map(Entity::entity_id) else {
            return;
        };
        let Some(domain) = self.mouse_polling_scope(cx) else {
            return;
        };
        if let Some(request) = self.matcher_normalization.request(owner, domain) {
            cx.emit(request);
        }
    }

    pub(super) fn activate_matcher_normalization(&mut self, cx: &mut Context<Self>) {
        self.matcher_normalization.activate();
        self.flush_matcher_normalization(cx);
    }

    /// The shell calls this after its workspace event subscription is installed.
    pub(in crate::features) fn bind_mouse_matcher_requests(&mut self, cx: &mut Context<Self>) {
        self.matcher_normalization.bind();
        self.flush_matcher_normalization(cx);
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
        let stages = match stages
            .iter()
            .map(SourceStage::from_source_file)
            .collect::<anyhow::Result<Vec<_>>>()
        {
            Ok(stages) => stages,
            Err(error) => {
                eprintln!("[dpi-matcher stages] {error:#}");
                return;
            }
        };
        let active = self
            .draft
            .pointer(self.spec.active_path())
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        let profiles = match self
            .draft
            .get("sensitivityMatcher")
            .map(Library::from_source_file)
            .transpose()
        {
            Ok(library) => library,
            Err(error) => {
                eprintln!("[dpi-matcher profiles] {error:#}");
                return;
            }
        };
        let selected = self.draft["selectedSensitivityMatcher"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        matcher.update(cx, |matcher, cx| {
            matcher.observe(MatcherObservation::Stages { stages, active }, window, cx);
            matcher.restore_profiles(profiles, selected, window, cx);
        });
    }

    pub fn mouse_matcher_scope(&self, cx: &App) -> Option<MouseMatcherScope> {
        if !self.active || self.page != "TAB_PERFORMANCE" {
            return None;
        }
        Some(MouseMatcherScope {
            owner: self.matcher.as_ref()?.entity_id(),
            epoch: self.matcher_epoch,
            valid: self.matcher_valid.clone(),
            kind: ScopeKind::Ui,
            domain: self.mouse_polling_scope(cx)?,
        })
    }

    /// Accepted saves retain their identity while exit drains them. A successor
    /// activation or profile restoration replaces the corresponding guard.
    pub fn retains_mouse_matcher_scope(&self, scope: &MouseMatcherScope, cx: &App) -> bool {
        self.mouse_polling_scope(cx) == Some(scope.domain)
            && self
                .matcher
                .as_ref()
                .is_some_and(|matcher| match scope.kind {
                    ScopeKind::Ui => scope.retained_by(
                        matcher.entity_id(),
                        self.matcher_epoch,
                        &self.matcher_valid,
                    ),
                    ScopeKind::Normalization => scope.retained_by(
                        matcher.entity_id(),
                        self.matcher_normalization.epoch,
                        &self.matcher_normalization.valid,
                    ),
                })
    }

    pub fn accepts_mouse_matcher_scope(&self, scope: &MouseMatcherScope, cx: &App) -> bool {
        scope.is_valid()
            && self.retains_mouse_matcher_scope(scope, cx)
            && (scope.is_normalization() || self.mouse_matcher_scope(cx).as_ref() == Some(scope))
    }

    pub(in crate::features) fn accept_mouse_matcher_saved_profiles(
        &mut self,
        scope: &MouseMatcherScope,
        saved: &SavedMatcherProfiles,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        ensure!(
            self.retains_mouse_matcher_scope(scope, cx),
            "Matcher saved page owner changed"
        );
        let preserve_newer_normalization = scope.is_normalization()
            && self
                .matcher_normalization
                .pending
                .as_ref()
                .is_some_and(|pending| {
                    pending.payload.library().file_value() != saved.library
                        || pending.payload.selected() != saved.selected.as_deref()
                });
        if !preserve_newer_normalization {
            saved.apply_to_draft(&mut self.draft)?;
        }
        self.matcher_normalization.acknowledge(saved);
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
        let stages = match stages
            .iter()
            .map(SourceStage::from_source_file)
            .collect::<anyhow::Result<Vec<_>>>()
        {
            Ok(stages) => stages,
            Err(error) => {
                eprintln!("[dpi-matcher stages] {error:#}");
                return;
            }
        };
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
        if !self.accepts_mouse_matcher_scope(scope, cx)
            || scope.is_normalization()
                && !matches!(observation, MatcherObservation::Profiles { .. })
        {
            return;
        }
        if let MatcherObservation::Profiles { library, selected } = &observation {
            let preserve_newer_normalization = scope.is_normalization()
                && self
                    .matcher_normalization
                    .pending
                    .as_ref()
                    .is_some_and(|pending| {
                        pending.payload.library().file_value() != library.file_value()
                            || pending.payload.selected() != Some(selected.as_str())
                    });
            if !preserve_newer_normalization {
                self.draft["sensitivityMatcher"] = library.file_value();
                self.draft["selectedSensitivityMatcher"] = json!(selected);
                cx.emit(MouseProductChanged);
            }
        }
        if let MatcherObservation::Stages { stages, active } = &observation {
            set_pointer(
                &mut self.draft,
                self.spec.stages_path(),
                Value::Array(stages.iter().map(SourceStage::file_value).collect()),
            );
            set_pointer(&mut self.draft, self.spec.active_path(), json!(active));
            self.dismiss_dpi_editors(window, cx);
            cx.emit(MouseProductChanged);
        }
        if let Some(matcher) = &self.matcher {
            matcher.update(cx, |matcher, cx| matcher.observe(observation, window, cx));
        }
        cx.notify();
    }

    pub fn submit_mouse_matcher_stages(
        &mut self,
        scope: &MouseMatcherScope,
        stages: SourceStages,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        ensure!(
            !scope.is_normalization() && self.retains_mouse_matcher_scope(scope, cx),
            "Matcher stage owner changed"
        );
        self.enqueue_matcher_stages(stages, cx)
    }

    pub fn reject_mouse_matcher(
        &mut self,
        scope: &MouseMatcherScope,
        reason: String,
        cx: &mut Context<Self>,
    ) {
        if self.accepts_mouse_matcher_scope(scope, cx) {
            if scope.is_normalization() {
                self.matcher_normalization.retry();
            }
            // A missing transport is not the source calibration-error response.
            self.matcher_error = Some(reason);
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MatcherColumn, MouseMatcherScope, NormalizationState, SavedMatcherProfiles, ScopeKind,
        device_fields, source_spec,
    };
    use crate::features::mouse_dpi_matcher::{
        MatcherAction, MatcherCommand, MatcherObservation, MatcherState,
    };
    use crate::features::mouse_polling::MousePollingScope;
    use gpui_kit::EntityId;
    use razer_model::model::{Device, Profile};
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn matcher_ui_products_follow_the_mounted_source_columns_and_device_field_scope() {
        for (product_id, column) in [
            (190, MatcherColumn::Right),
            (192, MatcherColumn::Right),
            (222, MatcherColumn::Right),
            (226, MatcherColumn::Right),
            (229, MatcherColumn::Left),
            (239, MatcherColumn::Left),
        ] {
            let spec = source_spec(product_id).unwrap();
            assert_eq!(spec.product().product_id(), product_id);
            assert_eq!(spec.column(), column);
            assert_eq!(device_fields(product_id), ["sensitivityMatcher"]);
        }
        for product_id in [0, 70, 182, 191, 193, 223, 227, 228, 230, 240, 241] {
            assert!(source_spec(product_id).is_none());
            assert!(device_fields(product_id).is_empty());
        }
    }

    #[test]
    fn matcher_exit_unmount_keeps_admitted_identity_without_reopening_ui_scope() {
        let valid = Arc::new(AtomicBool::new(true));
        let scope = MouseMatcherScope {
            owner: EntityId::from(1),
            epoch: 4,
            valid: valid.clone(),
            kind: ScopeKind::Ui,
            domain: domain(0, 0),
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
            kind: ScopeKind::Ui,
            domain: domain(0, 0),
        };
        let successor = Arc::new(AtomicBool::new(true));
        assert!(!scope.retained_by(EntityId::from(1), 4, &successor));
        assert!(!scope.retained_by(EntityId::from(1), 5, &valid));
        assert!(!scope.retained_by(EntityId::from(2), 4, &valid));
        let same_count_other_owner = MouseMatcherScope {
            owner: EntityId::from(2),
            epoch: 4,
            valid,
            kind: ScopeKind::Ui,
            domain: domain(0, 0),
        };
        assert_ne!(scope, same_count_other_owner);
    }

    fn domain(profile: u64, connection: u64) -> MousePollingScope {
        MousePollingScope::new(EntityId::from(10), profile, connection)
    }

    fn normalized_payload() -> razer_device::dpi_matcher_storage::ProfileUpdate {
        let mut state = MatcherState::default();
        let actions = state.observe_unmounted(MatcherObservation::Profiles {
            library: razer_device::dpi_matcher_storage::Library::from_source_file(&json!({
                "libraryExtension":"keep",
                "profiles":[
                    {"guid":"first","name":"Mouse"},
                    {"guid":"second","name":{"en":"Mouse","zh-cn":"Localized"}}
                ]
            }))
            .unwrap(),
            selected: "second".into(),
        });
        assert_eq!(actions, [MatcherAction::NormalizeProfiles]);
        assert_eq!(state.library().profiles()[1].name().text(), Some("Mouse 1"));
        state.profile_update()
    }

    #[test]
    fn matcher_normalization_retains_before_activation_and_outer_subscription_then_emits_once() {
        let mut owner = NormalizationState::default();
        let payload = normalized_payload();
        owner.retain(domain(1, 2), payload.clone());
        assert!(owner.request(EntityId::from(1), domain(1, 2)).is_none());
        owner.activate();
        assert!(owner.request(EntityId::from(1), domain(1, 2)).is_none());
        owner.bind();
        let request = owner.request(EntityId::from(1), domain(1, 2)).unwrap();
        assert!(request.scope().is_valid());
        assert!(request.scope().is_normalization());
        assert_eq!(request.action(), &MatcherAction::NormalizeProfiles);
        assert_eq!(request.profile_payload(), &payload);
        owner.retain(domain(1, 2), payload);
        assert!(owner.request(EntityId::from(1), domain(1, 2)).is_none());
    }

    #[test]
    fn matcher_normalization_ready_without_ui_activation_emits_the_retained_payload() {
        let mut owner = NormalizationState::default();
        owner.bind();
        owner.retain(domain(0, 0), normalized_payload());
        assert!(owner.request(EntityId::from(1), domain(0, 0)).is_some());
        assert!(owner.request(EntityId::from(1), domain(0, 0)).is_none());
    }

    #[test]
    fn matcher_normalization_profile_or_connection_change_discards_pending_payload() {
        for successor in [
            domain(2, 1),
            domain(1, 2),
            MousePollingScope::new(EntityId::from(11), 1, 1),
        ] {
            let mut owner = NormalizationState::default();
            owner.bind();
            owner.retain(domain(1, 1), normalized_payload());
            owner.activate();
            assert!(owner.request(EntityId::from(1), successor).is_none());
            assert!(owner.pending.is_none());
            assert!(owner.request(EntityId::from(1), domain(1, 1)).is_none());
        }
    }

    #[test]
    fn matcher_normalization_reactivation_and_restore_never_revive_an_old_request() {
        let mut owner = NormalizationState::default();
        owner.bind();
        owner.activate();
        owner.retain(domain(1, 1), normalized_payload());
        let old = owner.request(EntityId::from(1), domain(1, 1)).unwrap();
        owner.suspend();
        assert!(!old.scope().is_valid());
        assert!(owner.request(EntityId::from(1), domain(1, 1)).is_none());
        owner.activate();
        let successor = owner.request(EntityId::from(1), domain(1, 1)).unwrap();
        assert_ne!(old.scope(), successor.scope());
        assert!(!old.scope().is_valid());
        owner.reset();
        assert!(!successor.scope().is_valid());
        owner.activate();
        assert!(owner.request(EntityId::from(1), domain(1, 1)).is_none());
        owner.retain(domain(2, 1), normalized_payload());
        let restored = owner.request(EntityId::from(1), domain(2, 1)).unwrap();
        assert_ne!(restored.scope(), successor.scope());
        assert!(restored.scope().is_valid());
    }

    #[test]
    fn matcher_normalization_acknowledgement_only_consumes_its_exact_pending_library() {
        let mut owner = NormalizationState::default();
        let first = normalized_payload();
        let mut device = saved_device();
        device.source_device_settings.as_mut().unwrap()["sensitivityMatcher"] =
            first.library().file_value();
        device.profiles[0].source_settings.as_mut().unwrap()["selectedSensitivityMatcher"] =
            json!("second");
        let saved = SavedMatcherProfiles::from_device(&device).unwrap();
        owner.retain(domain(1, 1), first.clone());
        let second = razer_device::dpi_matcher_storage::ProfileUpdate::new(
            first.library().clone(),
            Some("first".into()),
        );
        owner.retain(domain(1, 1), second.clone());
        owner.acknowledge(&saved);
        assert_eq!(owner.pending.as_ref().unwrap().payload, second);
        device.profiles[0].source_settings.as_mut().unwrap()["selectedSensitivityMatcher"] =
            json!("first");
        owner.acknowledge(&SavedMatcherProfiles::from_device(&device).unwrap());
        assert!(owner.pending.is_none());
    }

    #[test]
    fn matcher_normalization_new_payload_or_ui_profile_action_retires_the_old_save_guard() {
        let mut owner = NormalizationState::default();
        owner.bind();
        let first = normalized_payload();
        owner.retain(domain(0, 0), first.clone());
        let old = owner.request(EntityId::from(1), domain(0, 0)).unwrap();
        let newer = razer_device::dpi_matcher_storage::ProfileUpdate::new(
            first.library().clone(),
            Some("first".into()),
        );
        owner.retain(domain(0, 0), newer.clone());
        let next = owner.request(EntityId::from(1), domain(0, 0)).unwrap();
        assert!(!old.scope().is_valid());
        assert!(
            !old.scope()
                .retained_by(EntityId::from(1), owner.epoch, &owner.valid)
        );
        assert!(next.scope().is_valid());
        assert_eq!(next.profile_payload(), &newer);
        owner.reset();
        assert!(!next.scope().is_valid());
        owner.activate();
        assert!(
            !next
                .scope()
                .retained_by(EntityId::from(1), owner.epoch, &owner.valid)
        );
        assert!(owner.request(EntityId::from(1), domain(0, 0)).is_none());
    }

    #[test]
    fn matcher_normalization_echo_does_not_queue_a_second_update() {
        let mut owner = NormalizationState::default();
        owner.bind();
        owner.activate();
        let payload = normalized_payload();
        owner.retain(domain(0, 0), payload.clone());
        assert!(owner.request(EntityId::from(1), domain(0, 0)).is_some());
        let mut state = MatcherState::default();
        assert!(
            state
                .observe_unmounted(MatcherObservation::Profiles {
                    library: payload.library().clone(),
                    selected: payload.selected().unwrap().to_owned(),
                })
                .is_empty()
        );
        assert!(owner.request(EntityId::from(1), domain(0, 0)).is_none());
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
        use razer_device::dpi_matcher_storage::Change;
        if let Some(stages) = mutation.stages() {
            let before = razer_model::mouse_matcher::source_record(device).unwrap();
            razer_model::mouse_matcher::Mutation::from_change(
                &before,
                &Change::Stages(stages.clone()),
            )
            .unwrap()
            .apply(device)
            .unwrap();
        }
        let before = razer_model::mouse_matcher::source_record(device).unwrap();
        razer_model::mouse_matcher::Mutation::from_change(
            &before,
            &Change::Profiles(mutation.response().clone()),
        )
        .unwrap()
        .apply(device)
        .unwrap();
    }

    #[test]
    fn matcher_exit_confirmed_save_restores_latest_profiles_without_replaying_device_work() {
        let mut live = saved_device();
        let mut saved = live.clone();
        let source = razer_model::mouse_matcher::source_record(&saved).unwrap();
        let confirmed = razer_device::dpi_matcher_storage::confirm(
            &razer_device::dpi_matcher_storage::SourceData::from_document(source.document())
                .unwrap(),
            1450.,
            "measured",
            serde_json::from_value(json!({"en":"Measured mouse","zh-cn":"Measured mouse"}))
                .unwrap(),
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
            library: razer_device::dpi_matcher_storage::Library::from_source_file(
                &draft["sensitivityMatcher"],
            )
            .unwrap(),
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
            razer_device::dpi_matcher_storage::delete(
                &razer_device::dpi_matcher_storage::SourceData::from_document(source.document())
                    .unwrap(),
                "old",
            )
            .unwrap(),
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
                &razer_device::dpi_matcher_storage::SourceData::from_document(source.document())
                    .unwrap(),
                1700.,
                "later",
                serde_json::from_value(json!("Later")).unwrap(),
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

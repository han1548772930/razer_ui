//! PID190 uS -> NS ownership; current AS/dS reducers and service observations.
//! UI drafts and transport failures are kept apart from device acknowledgements.
use super::*;
use crate::features::mouse_dpi_matcher::{MatcherAction, MatcherObservation, MouseDpiMatcher};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MouseMatcherScope {
    owner: EntityId,
    epoch: u64,
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
        // Other callers are inventoried in mouse-matcher-mount-current-source.json;
        // their distinct component internals have not yet passed equivalence review.
        if self.spec.product_id != 190 {
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

    pub(super) fn sync_matcher_mount(&self, cx: &mut Context<Self>) {
        if let Some(matcher) = &self.matcher {
            matcher.update(cx, |matcher, cx| {
                if self.active && self.page == "TAB_PERFORMANCE" {
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
        Some(MouseMatcherScope {
            owner: self.matcher.as_ref()?.entity_id(),
            epoch: self.matcher_epoch,
        })
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
        if self.mouse_matcher_scope(cx).as_ref() != Some(scope) {
            return;
        }
        if let MatcherObservation::Profiles { profiles, selected } = &observation {
            self.draft["sensitivityMatcher"] = json!({"profiles":profiles});
            self.draft["selectedSensitivityMatcher"] = json!(selected);
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
        if self.mouse_matcher_scope(cx).as_ref() == Some(scope) {
            // A missing transport is not the source calibration-error response.
            self.matcher_error = Some(reason);
        }
    }
}

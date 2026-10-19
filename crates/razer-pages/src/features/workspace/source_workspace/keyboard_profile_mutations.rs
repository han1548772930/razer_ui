//! Current756 reset callers. Candidate UI state precedes local persistence
//! and the queued middleware/device work, as in61350/50574.
use super::*;

impl SourceProductWorkspace {
    pub(super) fn reset_latest_keyboard_profile(
        &mut self,
        bindings_only: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.device.product_id != 756 {
            return;
        }
        let Some(index) = self
            .device
            .profiles
            .iter()
            .position(|profile| profile.id == self.device.active_profile)
        else {
            return;
        };
        let profile = &self.device.profiles[index];
        let Some(mut previous) = profile.source_settings.clone() else {
            cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(
                "Keyboard profile reset requires the current source profile for mapping cleanup"
                    .into(),
            ));
            return;
        };
        previous["guid"] = serde_json::json!(profile.guid);
        previous["name"] = serde_json::json!(profile.name);
        let candidate = match razer_device::keyboard_latest_restore::reset_profile_candidate(
            &previous,
            bindings_only,
        ) {
            Ok(candidate) => candidate,
            Err(error) => {
                cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(format!(
                    "{error:#}"
                )));
                return;
            }
        };
        self.device.profiles[index].source_settings = Some(candidate);
        self.dismiss_profile_dialog(window, cx);
        self.restore_active(window, cx);
        cx.emit(WorkspaceEvent::KeyboardLatestProfileMutationRequested(
            razer_device::keyboard_latest_restore::ProfileMutation::Reset {
                previous_profile: previous,
                bindings_only,
            },
        ));
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }
}

//! Current linked-game runtime selection and ordinary UI base selection have
//! different origins. Both reuse the mounted keyboard's existing restore path.
use super::*;

impl SourceProductWorkspace {
    pub(super) fn keyboard_linked_base_selected(&self, id: &str, cx: &mut Context<Self>) {
        if !matches!(self.device.product_id, 515 | 679 | 740 | 746 | 756) {
            return;
        }
        if let Some(profile) = self.device.profiles.iter().find(|profile| profile.id == id) {
            let guid = source_guid(profile);
            if !guid.is_empty() {
                cx.emit(WorkspaceEvent::KeyboardLinkedBaseSelected(guid.into()));
            }
        }
    }

    pub(super) fn select_source_profile(
        &mut self,
        id: &str,
        from_ui: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.device.product_id == 756 && self.keyboard_latest_ble && from_ui {
            self.profile.update(cx, |state, cx| {
                state.set_selected_value(&self.device.active_profile, window, cx)
            });
            if let Some(profile) = self.device.profiles.iter().find(|profile| profile.id == id) {
                if self.device.active_profile != id {
                    cx.emit(WorkspaceEvent::KeyboardLatestProfileSwitchRequested {
                        guid: source_guid(profile).to_owned(),
                        defer_persistence: false,
                    });
                }
            }
            return;
        }
        self.select_source_profile_inner(id, from_ui, true, window, cx);
    }

    pub(super) fn select_source_profile_inner(
        &mut self,
        id: &str,
        from_ui: bool,
        publish: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.device.profiles.iter().any(|profile| profile.id == id) {
            return;
        }
        if self.device.active_profile == id {
            if publish && !from_ui {
                self.persist_source_profile(true, cx);
            }
            return;
        }
        if from_ui {
            self.keyboard_linked_base_selected(id, cx);
        }
        self.dismiss_profile_dialog(window, cx);
        self.device.active_profile = id.to_owned();
        self.sync_mouse_profile_selection();
        let factory_default_profile = self
            .device
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .is_some_and(|profile| {
                super::super::keyboard_products::is_factory_profile(
                    self.device.product_id,
                    &profile.guid,
                )
            });
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |keyboard, cx| {
                keyboard.set_factory_default_profile(factory_default_profile, window, cx)
            });
        }
        self.profile.update(cx, |state, cx| {
            state.set_selected_value(&id.to_owned(), window, cx)
        });
        self.restore_active(window, cx);
        self.restore_seiren_selected_profile(cx);
        if publish {
            self.persist_source_profile(true, cx);
        }
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
    }

    /// SET_RUNTIME_PROFILE selects the source UI optimistically and does not
    /// broadcast another user ON_SWITCH_PROFILE back to middleware.
    pub fn observe_keyboard_runtime_profile(
        &mut self,
        guid: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.keyboard_runtime_profile(guid, false, window, cx)
    }
    /// The latest BLE owner calls this only after the actual slot ACK. A
    /// manual switch updates the linked-profile base at that same boundary.
    pub fn confirm_keyboard_latest_profile(
        &mut self,
        guid: &str,
        manual: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != 756 {
            return false;
        }
        let Some(id) = self
            .device
            .profiles
            .iter()
            .find(|profile| source_guid(profile) == guid)
            .map(|profile| profile.id.clone())
        else {
            return false;
        };
        self.select_source_profile_inner(&id, manual, false, window, cx);
        true
    }
    /// The subsequent SYNAPSE_SYS_ACTION uses the ordinary local profile save
    /// and native mapping/LED publication. It never updates the stack's base.
    pub fn select_keyboard_runtime_profile(
        &mut self,
        guid: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.keyboard_runtime_profile(guid, true, window, cx)
    }
    fn keyboard_runtime_profile(
        &mut self,
        guid: &str,
        publish: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.device.product_id, 515 | 679 | 740 | 746 | 756)
            || !matches!(&self.body, FamilyBody::Keyboard(_))
        {
            return false;
        }
        let Some(id) = self
            .device
            .profiles
            .iter()
            .find(|profile| source_guid(profile) == guid)
            .map(|profile| profile.id.clone())
        else {
            return false;
        };
        if self.device.product_id == 756 && self.keyboard_latest_ble && publish {
            cx.emit(WorkspaceEvent::KeyboardLatestProfileSwitchRequested {
                guid: guid.to_owned(),
                defer_persistence: true,
            });
            return true;
        }
        self.select_source_profile_inner(&id, false, publish, window, cx);
        true
    }
    pub fn observe_keyboard_linked_game_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if let FamilyBody::Keyboard(body) = &self.body {
            body.update(cx, |body, cx| body.observe_linked_game_active(active, cx));
        }
    }
}

fn source_guid(profile: &Profile) -> &str {
    profile
        .source_settings
        .as_ref()
        .and_then(|source| source["guid"].as_str())
        .filter(|guid| !guid.is_empty())
        .unwrap_or(&profile.guid)
}

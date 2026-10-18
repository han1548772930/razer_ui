use super::*;
use crate::features::{HelpOwnerCommand, HelpOwnerRequested};

fn publish_reset_profile_snapshot(current: &mut Device, candidate: Device) -> bool {
    if current.product_id != 1398
        || candidate.product_id != current.product_id
        || candidate.real_product_id != current.real_product_id
        || candidate.device_container_id != current.device_container_id
        || candidate.serial_number != current.serial_number
        || candidate.layout_id != current.layout_id
        || candidate.use_ble != current.use_ble
        || candidate.dashboard.connection_observation != current.dashboard.connection_observation
        || candidate.profiles.len() != current.profiles.len() + 1
        || !candidate.profiles.starts_with(&current.profiles)
        || candidate.profiles.last().map(|profile| &profile.id) != Some(&candidate.active_profile)
        || current.profiles.iter().any(|profile| {
            profile.id == candidate.active_profile || profile.guid == candidate.active_profile
        })
    {
        return false;
    }
    let Some(profile) = candidate.active_profile_obj() else {
        return false;
    };
    if profile.id.is_empty()
        || profile.id != profile.guid
        || !profile.source_settings.as_ref().is_some_and(|source| {
            source.get("guid").and_then(Value::as_str) == Some(profile.guid.as_str())
                && source.get("name").and_then(Value::as_str) == Some(profile.name.as_str())
        })
    {
        return false;
    }
    current.profiles = candidate.profiles;
    current.active_profile = candidate.active_profile;
    current.source_device_settings = candidate.source_device_settings;
    true
}

impl SourceProductWorkspace {
    pub(super) fn publish_receiver_snapshot(
        &mut self,
        snapshot: razer_storage::receiver_reset::ReceiverProfileSnapshot,
        restore: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let candidate = match snapshot.project(&self.device) {
            Ok(candidate) => candidate,
            Err(error) => {
                eprintln!("[receiver profile publication] {error:#}");
                return false;
            }
        };
        self.device.profiles = candidate.profiles;
        self.device.active_profile = candidate.active_profile;
        self.device.source_device_settings = candidate.source_device_settings;
        self.device.serial_number = candidate.serial_number;
        self.help.update(cx, |help, cx| {
            help.accept_source_serial(&self.device.serial_number, cx)
        });
        self.refresh_profile_choices(window, cx);
        if restore {
            self.restore_active(window, cx);
        }
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
        true
    }

    /// The persisted profile precedes the independent source refresh tasks.
    pub fn publish_receiver_help_profile(
        &mut self,
        request: super::super::HelpResetRequest,
        snapshot: razer_storage::receiver_reset::ReceiverProfileSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        request.source_action() == Some("ON_RESET_DEVICE")
            && self.help_reset_matches(request, cx)
            && self.publish_receiver_snapshot(snapshot, true, window, cx)
    }

    pub fn publish_receiver_brightness_snapshot(
        &mut self,
        generation: u64,
        percent: Option<u8>,
        snapshot: razer_storage::receiver_reset::ReceiverProfileSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.receiver_brightness_matches(generation, percent, true, cx)
            && self.publish_receiver_snapshot(snapshot, false, window, cx)
    }

    /// Publish the persisted reset profile before the separate firmware command.
    /// Updating this local snapshot does not acknowledge a hardware reset.
    pub fn publish_help_hardware_reset(
        &mut self,
        request: super::super::HelpResetRequest,
        candidate: Device,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.exiting
            || request.hardware_reset_message().is_none()
            || !self.help_reset_matches(request, cx)
            || !matches!(&self.body, FamilyBody::Audio(_))
            || !publish_reset_profile_snapshot(&mut self.device, candidate)
        {
            return false;
        }
        self.refresh_profile_choices(window, cx);
        let profile = self
            .device
            .active_profile_obj()
            .and_then(|profile| profile.source_settings.as_ref());
        if let FamilyBody::Audio(audio) = &self.body {
            audio.update(cx, |audio, cx| {
                audio.restore_after_hardware_reset(profile, window, cx)
            });
        }
        self.help
            .update(cx, |help, cx| help.set_device(&self.device, cx));
        cx.emit(WorkspaceEvent::Changed);
        cx.notify();
        true
    }

    pub fn observe_help_product_names(
        &mut self,
        device: &Device,
        request: &HelpOwnerRequested,
        names: Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != device.product_id
            || self.device.serial_number != device.serial_number
            || self.device.device_container_id != device.device_container_id
            || !self.accepts_help_owner(request, cx)
        {
            return false;
        }
        self.help
            .update(cx, |help, cx| help.observe_product_names(names, cx))
    }
    pub fn help_owner_observation_request(&self, cx: &App) -> Option<HelpOwnerRequested> {
        let owner = self.help_owner.read(cx);
        let request = owner.request(HelpOwnerCommand::Observe);
        owner.accepts(&request).then_some(request)
    }
    pub fn accepts_help_owner(&self, request: &HelpOwnerRequested, cx: &App) -> bool {
        self.device.product_id == request.product_id() && self.help_owner.read(cx).accepts(request)
    }
    pub fn accepts_help_audio_sound(&self, request: &HelpOwnerRequested, cx: &App) -> bool {
        self.device.product_id == request.product_id()
            && self.help_owner.read(cx).accepts_audio_sound(request)
    }
    pub fn observe_help_simultaneous_audio(
        &mut self,
        device: &Device,
        request: &HelpOwnerRequested,
        payload: Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != device.product_id
            || self.device.serial_number != device.serial_number
            || self.device.device_container_id != device.device_container_id
            || !self.accepts_help_owner(request, cx)
        {
            return false;
        }
        self.help_owner.update(cx, |owner, cx| {
            owner.observe_simultaneous_audio(request, payload, cx)
        })
    }
    pub fn observe_help_audio_enhancements(
        &mut self,
        device: &Device,
        request: &HelpOwnerRequested,
        key: &str,
        payload: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != device.product_id
            || self.device.serial_number != device.serial_number
            || self.device.device_container_id != device.device_container_id
            || !self.accepts_help_owner(request, cx)
        {
            return false;
        }
        match &self.body {
            FamilyBody::Audio(audio) => audio.update(cx, |audio, cx| {
                audio.observe_enhancements(key, payload, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_help_audio_sound(
        &mut self,
        device: &Device,
        request: &HelpOwnerRequested,
        key: &str,
        payload: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != device.product_id
            || self.device.serial_number != device.serial_number
            || self.device.device_container_id != device.device_container_id
            || !self.accepts_help_audio_sound(request, cx)
        {
            return false;
        }
        match &self.body {
            FamilyBody::Audio(audio) => audio.update(cx, |audio, cx| {
                audio.observe_sound(key, payload, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_help_whats_new_dismissal(
        &mut self,
        request: &HelpOwnerRequested,
        value: Option<Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_help_owner(request, cx) {
            return false;
        }
        self.help_owner.update(cx, |owner, cx| {
            owner.observe_whats_new_dismissal(request, value, cx)
        })
    }
    pub fn observe_help_tutorial_dismissal(
        &mut self,
        request: &HelpOwnerRequested,
        value: Option<Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_help_owner(request, cx) {
            return false;
        }
        self.help_owner.update(cx, |owner, cx| {
            owner.observe_tutorial_dismissal(request, value, cx)
        })
    }
    pub fn observe_help_render_device_name(
        &mut self,
        device: &Device,
        request: &HelpOwnerRequested,
        name: String,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.serial_number != device.serial_number
            || self.device.product_id != device.product_id
            || self.device.device_container_id != device.device_container_id
            || !self.accepts_help_owner(request, cx)
        {
            return false;
        }
        self.help_owner.update(cx, |owner, cx| {
            owner.observe_render_device_name(request, name, cx)
        })
    }
    pub fn observe_help_host_focus(
        &mut self,
        request: &HelpOwnerRequested,
        focus_tab: &str,
        window_name: &str,
        is_visible: Option<bool>,
        is_minimized: Option<bool>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_help_owner(request, cx) {
            return false;
        }
        self.help_owner.update(cx, |owner, cx| {
            owner.observe_host_focus(
                request,
                focus_tab,
                window_name,
                is_visible,
                is_minimized,
                cx,
            )
        })
    }
    pub fn observe_help_initial_host_failure(
        &mut self,
        request: &HelpOwnerRequested,
        document_hidden: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_help_owner(request, cx) {
            return false;
        }
        self.help_owner.update(cx, |owner, cx| {
            owner.observe_initial_host_failure(request, document_hidden, cx)
        })
    }
    pub fn observe_help_active_tab(
        &mut self,
        request: &HelpOwnerRequested,
        focus_tab: &str,
        window_name: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_help_owner(request, cx) {
            return false;
        }
        self.help_owner.update(cx, |owner, cx| {
            owner.observe_active_tab(request, focus_tab, window_name, cx)
        })
    }
}

#[cfg(test)]
#[path = "help_owner_tests.rs"]
mod tests;

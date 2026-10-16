use super::*;
use crate::features::{HelpOwnerCommand, HelpOwnerRequested};
impl SourceProductWorkspace {
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

use super::*;
use crate::features::HelpOwnerRequested;
impl ProductWorkspace {
    pub fn observe_help_product_names(
        &mut self,
        device: &Device,
        request: &HelpOwnerRequested,
        names: serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_product_names(device, request, names, cx)
            }),
            _ => false,
        }
    }
    pub fn help_owner_observation_request(&self, cx: &App) -> Option<HelpOwnerRequested> {
        match &self.body {
            Body::Source(body) => body.read(cx).help_owner_observation_request(cx),
            _ => None,
        }
    }
    pub fn accepts_help_owner(&self, request: &HelpOwnerRequested, cx: &App) -> bool {
        matches!(&self.body, Body::Source(body) if body.read(cx).accepts_help_owner(request, cx))
    }
    pub fn accepts_help_audio_sound(&self, request: &HelpOwnerRequested, cx: &App) -> bool {
        matches!(&self.body, Body::Source(body) if body.read(cx).accepts_help_audio_sound(request, cx))
    }
    pub fn observe_help_simultaneous_audio(
        &mut self,
        device: &Device,
        request: &HelpOwnerRequested,
        payload: serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_simultaneous_audio(device, request, payload, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_help_whats_new_dismissal(
        &mut self,
        request: &HelpOwnerRequested,
        value: Option<serde_json::Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_whats_new_dismissal(request, value, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_help_audio_enhancements(
        &mut self,
        device: &Device,
        request: &HelpOwnerRequested,
        key: &str,
        payload: serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_audio_enhancements(device, request, key, payload, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_help_audio_sound(
        &mut self,
        device: &Device,
        request: &HelpOwnerRequested,
        key: &str,
        payload: serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_audio_sound(device, request, key, payload, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_help_tutorial_dismissal(
        &mut self,
        request: &HelpOwnerRequested,
        value: Option<serde_json::Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_tutorial_dismissal(request, value, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_help_render_device_name(
        &mut self,
        device: &Device,
        request: &HelpOwnerRequested,
        name: String,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_render_device_name(device, request, name, cx)
            }),
            _ => false,
        }
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
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_host_focus(
                    request,
                    focus_tab,
                    window_name,
                    is_visible,
                    is_minimized,
                    cx,
                )
            }),
            _ => false,
        }
    }
    pub fn observe_help_initial_host_failure(
        &mut self,
        request: &HelpOwnerRequested,
        document_hidden: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_initial_host_failure(request, document_hidden, cx)
            }),
            Body::Existing(_) => false,
        }
    }
    pub fn observe_help_active_tab(
        &mut self,
        request: &HelpOwnerRequested,
        focus_tab: &str,
        window_name: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_active_tab(request, focus_tab, window_name, cx)
            }),
            _ => false,
        }
    }
}

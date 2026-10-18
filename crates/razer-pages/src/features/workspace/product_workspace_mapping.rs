//! Mapping observations require the same product, physical owner and profile.
use super::*;
use crate::features::keyboard_products::{
    KeyboardMappingAiObservation, KeyboardMappingInterDeviceObservation,
    KeyboardMappingMacroObservation,
};

impl ProductWorkspace {
    /// Advances when a connection is replaced, including a batched reconnect
    /// whose final setupStatus is unchanged. Never persisted as a device fact.
    pub fn connection_revision(&self) -> u64 {
        self.connection_revision
    }

    pub fn observe_running_games(&mut self, count: Option<usize>, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.observe_running_games(count, cx));
        }
    }

    pub fn observe_polling_game_event(&mut self, started: bool, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.observe_polling_game_event(started, cx));
        }
    }

    pub fn consume_interdevice_mapping(
        &mut self,
        raw: serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => {
                body.update(cx, |body, cx| body.consume_interdevice_mapping(raw, cx))
            }
            _ => false,
        }
    }
    pub fn set_local_mapping_targets(
        &mut self,
        devices: &[Device],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.set_local_mapping_targets(devices, window, cx)
            });
        }
    }
    pub fn accepts_keyboard_mapping_brightness(
        &self,
        request: &crate::features::keyboard_products::KeyboardMappingBrightnessReadRequested,
        cx: &App,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body
                .read(cx)
                .accepts_keyboard_mapping_brightness(request, cx),
            _ => false,
        }
    }
    pub fn observe_keyboard_mapping_brightness(
        &mut self,
        owner: &Device,
        observation: crate::features::keyboard_products::KeyboardMappingBrightnessObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_keyboard_mapping_brightness(owner, observation, window, cx)
            }),
            _ => false,
        }
    }
    pub fn accepts_keyboard_mapping_dial(
        &self,
        request: &crate::features::keyboard_products::KeyboardMappingDialReadRequested,
        cx: &App,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.read(cx).accepts_keyboard_mapping_dial(request, cx),
            _ => false,
        }
    }
    pub fn observe_keyboard_mapping_dial(
        &mut self,
        owner: &Device,
        observation: crate::features::keyboard_products::KeyboardMappingDialObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_keyboard_mapping_dial(owner, observation, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_keyboard_mapping_sensitivity_warning(
        &mut self,
        owner: &Device,
        preference: Option<serde_json::Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_keyboard_mapping_sensitivity_warning(owner, preference, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_keyboard_mapping_system_keyboard_layout(
        &mut self,
        owner: &Device,
        layout: u32,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_keyboard_mapping_system_keyboard_layout(owner, layout, cx)
            }),
            _ => false,
        }
    }

    pub fn consume_keyboard_mapping_input_redirect(
        &mut self,
        owner: &Device,
        input_product_id: u32,
        source_input_json: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.consume_keyboard_mapping_input_redirect(
                    owner,
                    input_product_id,
                    source_input_json,
                    cx,
                )
            }),
            _ => false,
        }
    }

    pub fn observe_keyboard_mapping_login_warning_hidden(
        &mut self,
        hidden: bool,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_keyboard_mapping_login_warning_hidden(hidden, cx)
            });
        }
    }
    pub fn accepts_keyboard_mapping_inter_device_configure(
        &self,
        request: &crate::features::keyboard_products::KeyboardMappingInterDeviceConfigureRequested,
        cx: &App,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body
                .read(cx)
                .accepts_keyboard_mapping_inter_device_configure(request, cx),
            _ => false,
        }
    }
    pub fn observe_keyboard_mapping_inter_device(
        &mut self,
        owner: &Device,
        observation: KeyboardMappingInterDeviceObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_keyboard_mapping_inter_device(owner, observation, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_keyboard_mapping_ai(
        &mut self,
        owner: &Device,
        observation: KeyboardMappingAiObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_keyboard_mapping_ai(owner, observation, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_keyboard_mapping_macros(
        &mut self,
        owner: &Device,
        observation: KeyboardMappingMacroObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_keyboard_mapping_macros(owner, observation, window, cx)
            }),
            _ => false,
        }
    }
}

//! Product-scoped lighting requests and observations. A local draft is not a reply.
use super::*;
use crate::features::keyboard_products::{
    KeyboardLightingHostObservation, KeyboardLightingObservation, KeyboardLightingOwner,
    KeyboardLightingRequested,
};
use crate::features::mouse_products::MouseLightingRequested;

impl SourceProductWorkspace {
    pub fn abandon_keyboard_lighting(
        &mut self,
        request: &KeyboardLightingRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            FamilyBody::Keyboard(body) => body.update(cx, |body, cx| {
                body.abandon_lighting_request(request, error, cx)
            }),
            _ => false,
        }
    }
    pub fn abandon_mouse_lighting(
        &mut self,
        request: &MouseLightingRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| {
                body.abandon_mouse_lighting_request(request, error, cx)
            }),
            _ => false,
        }
    }
    pub fn mouse_lighting_request_matches(
        &self,
        request: &MouseLightingRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).mouse_lighting_request_matches(request, body.entity_id()))
    }
    pub fn complete_keyboard_host_lighting(
        &mut self,
        request: &KeyboardLightingRequested,
        observation: KeyboardLightingHostObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            FamilyBody::Keyboard(body) => body.update(cx, |body, cx| {
                body.complete_host_lighting_request(request, observation, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_mouse_customize(
        &mut self,
        owner: &Device,
        snapshot: &Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
        {
            return false;
        }
        match &self.body {
            FamilyBody::Mouse(body) => {
                body.update(cx, |body, cx| body.observe_mouse_customize(snapshot, cx));
                true
            }
            _ => false,
        }
    }
    /// Adjustment state belongs to the physical product, including non-lighting
    /// tabs. Reject observations for an entity whose device has been replaced.
    pub fn observe_mouse_adjustment_mode(
        &mut self,
        owner: &Device,
        running: bool,
        show_prompt: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
        {
            return false;
        }
        match &self.body {
            FamilyBody::Mouse(body) => {
                body.update(cx, |body, cx| {
                    body.observe_mouse_adjustment_mode(running, Some(show_prompt), cx)
                });
                true
            }
            _ => false,
        }
    }
    pub fn keyboard_lighting_owner(&self, cx: &App) -> Option<KeyboardLightingOwner> {
        match &self.body {
            FamilyBody::Keyboard(body) => Some(body.read(cx).lighting_owner()),
            _ => None,
        }
    }
    pub fn accepts_keyboard_lighting(&self, request: &KeyboardLightingRequested, cx: &App) -> bool {
        matches!(&self.body, FamilyBody::Keyboard(body) if body.read(cx).accepts_lighting_request(request))
    }
    pub fn observe_keyboard_lighting(
        &mut self,
        owner: &KeyboardLightingOwner,
        observation: KeyboardLightingObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            FamilyBody::Keyboard(body) => body.update(cx, |body, cx| {
                body.observe_lighting(owner, observation, window, cx)
            }),
            _ => false,
        }
    }
    pub fn complete_keyboard_lighting(
        &mut self,
        request: &KeyboardLightingRequested,
        result: Result<Option<KeyboardLightingObservation>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            FamilyBody::Keyboard(body) => body.update(cx, |body, cx| {
                body.complete_lighting_request(request, result, window, cx)
            }),
            _ => false,
        }
    }
    pub fn accepts_mouse_lighting(
        &mut self,
        request: &MouseLightingRequested,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| {
                body.accepts_mouse_lighting_request(request, cx)
            }),
            _ => false,
        }
    }
    pub fn complete_mouse_lighting(
        &mut self,
        request: &MouseLightingRequested,
        result: Result<Option<&Value>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| match result {
                Ok(snapshot) => body.complete_mouse_lighting_request(request, snapshot, window, cx),
                Err(error) => body.reject_mouse_lighting_request(request, error, window, cx),
            }),
            _ => false,
        }
    }
}

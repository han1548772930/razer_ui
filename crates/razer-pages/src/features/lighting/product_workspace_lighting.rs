//! Shell-facing lighting boundary. Validate the original product/profile owner.
use super::*;
use crate::features::keyboard_products::{
    KeyboardLightingHostObservation, KeyboardLightingObservation, KeyboardLightingOwner,
    KeyboardLightingRequested,
};
use crate::features::mouse_products::MouseLightingRequested;
use serde_json::Value;

impl ProductWorkspace {
    pub fn observe_chroma_configuration(
        &mut self,
        configuration: &razer_storage::chroma_profiles::Configuration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.observe_chroma_configuration(configuration, window, cx)
            });
        }
    }
    pub fn abandon_keyboard_lighting(
        &mut self,
        request: &KeyboardLightingRequested,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.abandon_keyboard_lighting(request, error, cx)
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
            Body::Source(body) => body.update(cx, |body, cx| {
                body.abandon_mouse_lighting(request, error, cx)
            }),
            _ => false,
        }
    }
    pub fn mouse_lighting_request_matches(
        &self,
        request: &MouseLightingRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, Body::Source(body) if body.read(cx).mouse_lighting_request_matches(request, cx))
    }
    pub fn complete_keyboard_host_lighting(
        &mut self,
        request: &KeyboardLightingRequested,
        observation: KeyboardLightingHostObservation,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.complete_keyboard_host_lighting(request, observation, result, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_mouse_customize(
        &mut self,
        owner: &razer_model::model::Device,
        snapshot: &Value,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_mouse_customize(owner, snapshot, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_mouse_adjustment_mode(
        &mut self,
        owner: &razer_model::model::Device,
        running: bool,
        show_prompt: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_mouse_adjustment_mode(owner, running, show_prompt, cx)
            }),
            _ => false,
        }
    }
    pub fn keyboard_lighting_owner(&self, cx: &App) -> Option<KeyboardLightingOwner> {
        match &self.body {
            Body::Source(body) => body.read(cx).keyboard_lighting_owner(cx),
            _ => None,
        }
    }
    pub fn accepts_keyboard_lighting(&self, request: &KeyboardLightingRequested, cx: &App) -> bool {
        matches!(&self.body, Body::Source(body) if body.read(cx).accepts_keyboard_lighting(request,cx))
    }
    pub fn observe_keyboard_lighting(
        &mut self,
        owner: &KeyboardLightingOwner,
        observation: KeyboardLightingObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_keyboard_lighting(owner, observation, window, cx)
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
            Body::Source(body) => body.update(cx, |body, cx| {
                body.complete_keyboard_lighting(request, result, window, cx)
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
            Body::Source(body) => {
                body.update(cx, |body, cx| body.accepts_mouse_lighting(request, cx))
            }
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
            Body::Source(body) => body.update(cx, |body, cx| {
                body.complete_mouse_lighting(request, result, window, cx)
            }),
            _ => false,
        }
    }
    pub fn complete_mouse_host_lighting(
        &mut self,
        request: &MouseLightingRequested,
        snapshot: &Value,
        result: Result<(), String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.complete_mouse_host_lighting(request, snapshot, result, window, cx)
            }),
            _ => false,
        }
    }
}

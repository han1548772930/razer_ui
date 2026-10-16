use super::*;
use crate::features::source_controls::HyperfluxRequested;

impl ProductWorkspace {
    pub fn observe_hyperflux_connected_device_update(
        &mut self,
        owner: &Device,
        source: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_hyperflux_connected_device_update(owner, source, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_hyperflux_pairing_response(
        &mut self,
        owner: &Device,
        message: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_hyperflux_pairing_response(owner, message, cx)
            }),
            _ => false,
        }
    }
    pub fn accepts_hyperflux(&self, request: &HyperfluxRequested, cx: &App) -> bool {
        match &self.body {
            Body::Source(body) => body.read(cx).accepts_hyperflux(request, cx),
            _ => false,
        }
    }
    pub fn observe_hyperflux(
        &mut self,
        owner: &Device,
        source: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_hyperflux(owner, source, window, cx)
            }),
            _ => false,
        }
    }
    pub fn observe_hyperflux_connected_device(
        &mut self,
        owner: &Device,
        source: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_hyperflux_connected_device(owner, source, cx)
            }),
            _ => false,
        }
    }
}

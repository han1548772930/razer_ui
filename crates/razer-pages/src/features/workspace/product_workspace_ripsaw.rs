use super::*;
use crate::features::RipsawMonitoringRequested;

impl ProductWorkspace {
    pub fn accepts_ripsaw_monitoring(&self, request: &RipsawMonitoringRequested, cx: &App) -> bool {
        match &self.body {
            Body::Source(body) => body.read(cx).accepts_ripsaw_monitoring(request, cx),
            _ => false,
        }
    }
    pub fn observe_ripsaw_monitoring(
        &mut self,
        owner: &Device,
        source_payload: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_ripsaw_monitoring(owner, source_payload, window, cx)
            }),
            _ => false,
        }
    }
}

use super::*;
use crate::features::HelpSystemInfoRequested;

impl ProductWorkspace {
    pub fn accepts_help_system_info(&self, request: &HelpSystemInfoRequested, cx: &App) -> bool {
        match &self.body {
            Body::Source(body) => body.read(cx).accepts_help_system_info(request, cx),
            _ => false,
        }
    }

    pub fn observe_help_system_info(
        &mut self,
        owner: &Device,
        request: &HelpSystemInfoRequested,
        source: serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_system_info(owner, request, source, cx)
            }),
            _ => false,
        }
    }
}

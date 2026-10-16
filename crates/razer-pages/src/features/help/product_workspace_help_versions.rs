use super::*;
use crate::features::{HelpRuntimeVersionsObservation, HelpRuntimeVersionsRequest};

impl ProductWorkspace {
    pub fn help_runtime_versions_request(&self, cx: &App) -> Option<HelpRuntimeVersionsRequest> {
        match &self.body {
            Body::Source(body) => body.read(cx).help_runtime_versions_request(cx),
            _ => None,
        }
    }
    pub fn observe_help_runtime_versions(
        &mut self,
        owner: &Device,
        observation: HelpRuntimeVersionsObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_runtime_versions(owner, observation, cx)
            }),
            _ => false,
        }
    }
}

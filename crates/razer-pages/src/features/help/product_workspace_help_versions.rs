use super::*;
use crate::features::{
    HelpRuntimeVersionsObservation, HelpRuntimeVersionsRequest, HelpRuntimeVersionsStatus,
};

impl ProductWorkspace {
    pub fn help_runtime_versions_request(&self, cx: &App) -> Option<HelpRuntimeVersionsRequest> {
        match &self.body {
            Body::Source(body) => body.read(cx).help_runtime_versions_request(cx),
            _ => None,
        }
    }
    pub fn accepts_help_runtime_versions(
        &self,
        request: &HelpRuntimeVersionsRequest,
        cx: &App,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.read(cx).accepts_help_runtime_versions(request, cx),
            _ => false,
        }
    }
    pub fn help_runtime_versions_status(&self, cx: &App) -> Option<HelpRuntimeVersionsStatus> {
        match &self.body {
            Body::Source(body) => Some(body.read(cx).help_runtime_versions_status(cx)),
            _ => None,
        }
    }
    pub fn refresh_help_runtime_versions(&mut self, cx: &mut Context<Self>) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| body.refresh_help_runtime_versions(cx));
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

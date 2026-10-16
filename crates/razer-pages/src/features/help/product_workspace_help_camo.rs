use super::*;
use crate::features::{HelpCamoObservation, HelpCamoRequested};

impl ProductWorkspace {
    pub fn accepts_help_camo(&self, request: &HelpCamoRequested, cx: &App) -> bool {
        match &self.body {
            Body::Source(body) => body.read(cx).accepts_help_camo(request, cx),
            _ => false,
        }
    }
    pub fn observe_help_camo(
        &mut self,
        owner: &Device,
        observation: HelpCamoObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_help_camo(owner, observation, cx)
            }),
            _ => false,
        }
    }
}

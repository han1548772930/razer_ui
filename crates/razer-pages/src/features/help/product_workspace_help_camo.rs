use super::*;
use crate::features::CameraIdentityRequested;
use crate::features::{HelpCamoObservation, HelpCamoRequested};

impl ProductWorkspace {
    pub fn camera_identity_request(&self, cx: &App) -> Option<CameraIdentityRequested> {
        match &self.body {
            Body::Source(body) => body.read(cx).camera_identity_request().cloned(),
            _ => None,
        }
    }

    pub fn accepts_camera_identity(&self, request: &CameraIdentityRequested, cx: &App) -> bool {
        match &self.body {
            Body::Source(body) => body.read(cx).accepts_camera_identity(request),
            _ => false,
        }
    }

    pub fn observe_camera_identity(
        &mut self,
        request: &CameraIdentityRequested,
        result: Result<razer_device::camera_serial::CameraSerialProgress, String>,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.observe_camera_identity(request, result, cx)
            }),
            _ => false,
        }
    }

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

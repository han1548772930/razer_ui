use super::*;
use crate::features::{HelpCamoObservation, HelpCamoRequested};

impl SourceProductWorkspace {
    pub fn accepts_help_camo(&self, request: &HelpCamoRequested, cx: &App) -> bool {
        self.device.product_id == request.product_id()
            && self.help.read(cx).accepts_camo_request(request)
    }
    pub fn observe_help_camo(
        &mut self,
        owner: &Device,
        observation: HelpCamoObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
            || !self.accepts_help_camo(observation.request(), cx)
        {
            return false;
        }
        self.help
            .update(cx, |help, cx| help.observe_camo(observation, cx))
    }
}

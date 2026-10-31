use super::*;
use crate::features::{HelpCamoObservation, HelpCamoRequested};

impl SourceProductWorkspace {
    pub fn accepts_help_camo(&self, request: &HelpCamoRequested, cx: &App) -> bool {
        self.active
            && !self.exiting
            && self.device.product_id == request.product_id()
            && self.help.read(cx).accepts_camo_request(request)
    }
    pub fn observe_help_camo(
        &mut self,
        owner: &Device,
        observation: HelpCamoObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.real_product_id != owner.real_product_id
            || self.device.device_container_id != owner.device_container_id
            || self.device.edition_id != owner.edition_id
            || self.device.layout_id != owner.layout_id
            || !self.accepts_help_camo(observation.request(), cx)
        {
            return false;
        }
        if let Some(result) = observation.serial_result() {
            if result
                .as_ref()
                .is_ok_and(|progress| progress.product_id() != self.device.product_id)
            {
                return false;
            }
            let state = match result.clone() {
                Ok(progress) => razer_model::model::CameraSerialObservation::Read(progress),
                Err(error) => razer_model::model::CameraSerialObservation::Failed(error),
            };
            self.publish_camera_serial(state, cx);
        }
        self.help
            .update(cx, |help, cx| help.observe_camo(observation, cx))
    }
}

//! Current RipSaw monitoring owner: observations and requests stay device-scoped.
use super::*;
use crate::features::RipsawMonitoringRequested;

impl SourceProductWorkspace {
    pub fn accepts_ripsaw_monitoring(&self, request: &RipsawMonitoringRequested, cx: &App) -> bool {
        let FamilyBody::Controls(body) = &self.body else {
            return false;
        };
        self.device.product_id == 3331
            && body
                .read(cx)
                .accepts_ripsaw_monitoring(request, body.entity_id())
    }
    pub fn observe_ripsaw_monitoring(
        &mut self,
        owner: &Device,
        source_payload: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != 3331
            || owner.product_id != self.device.product_id
            || owner.serial_number != self.device.serial_number
            || owner.device_container_id != self.device.device_container_id
        {
            return false;
        }
        let FamilyBody::Controls(body) = &self.body else {
            return false;
        };
        body.update(cx, |body, cx| {
            body.observe_ripsaw_monitoring(source_payload, window, cx)
        });
        true
    }
}

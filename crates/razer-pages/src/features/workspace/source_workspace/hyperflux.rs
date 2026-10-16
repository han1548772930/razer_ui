use super::*;
use crate::features::source_controls::HyperfluxRequested;

impl SourceProductWorkspace {
    pub fn observe_hyperflux_connected_device_update(
        &mut self,
        owner: &Device,
        source: &Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.hyperflux_owner_matches(owner) {
            return false;
        }
        let FamilyBody::Controls(body) = &self.body else {
            return false;
        };
        body.update(cx, |body, cx| {
            body.observe_hyperflux_connected_device_update(source, cx)
        })
    }
    pub fn observe_hyperflux_pairing_response(
        &mut self,
        owner: &Device,
        message: &Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.hyperflux_owner_matches(owner) {
            return false;
        }
        let FamilyBody::Controls(body) = &self.body else {
            return false;
        };
        body.update(cx, |body, cx| {
            body.observe_hyperflux_pairing_response(message, cx)
        })
    }
    pub fn accepts_hyperflux(&self, request: &HyperfluxRequested, cx: &App) -> bool {
        let FamilyBody::Controls(body) = &self.body else {
            return false;
        };
        self.device.product_id == 207 && body.read(cx).accepts_hyperflux(request, body.entity_id())
    }
    pub fn observe_hyperflux(
        &mut self,
        owner: &Device,
        source: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.hyperflux_owner_matches(owner) {
            return false;
        }
        let FamilyBody::Controls(body) = &self.body else {
            return false;
        };
        body.update(cx, |body, cx| body.observe_hyperflux(source, window, cx))
    }
    pub fn observe_hyperflux_connected_device(
        &mut self,
        owner: &Device,
        source: &Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.hyperflux_owner_matches(owner) {
            return false;
        }
        let FamilyBody::Controls(body) = &self.body else {
            return false;
        };
        body.update(cx, |body, cx| {
            body.observe_hyperflux_connected_device(source, cx)
        })
    }
    fn hyperflux_owner_matches(&self, owner: &Device) -> bool {
        self.device.product_id == 207
            && owner.product_id == 207
            && self.device.serial_number == owner.serial_number
            && self.device.device_container_id == owner.device_container_id
    }
}

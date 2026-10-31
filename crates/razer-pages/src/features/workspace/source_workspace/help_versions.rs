use super::*;
use crate::features::{
    HelpRuntimeVersionsObservation, HelpRuntimeVersionsRequest, HelpRuntimeVersionsStatus,
};

impl SourceProductWorkspace {
    pub fn help_runtime_versions_request(&self, cx: &App) -> Option<HelpRuntimeVersionsRequest> {
        self.help.read(cx).runtime_versions_request()
    }
    pub fn accepts_help_runtime_versions(
        &self,
        request: &HelpRuntimeVersionsRequest,
        cx: &App,
    ) -> bool {
        self.help.read(cx).accepts_runtime_versions(request)
    }
    pub fn help_runtime_versions_status(&self, cx: &App) -> HelpRuntimeVersionsStatus {
        self.help.read(cx).runtime_versions_status().clone()
    }
    pub fn refresh_help_runtime_versions(&mut self, cx: &mut Context<Self>) {
        if self.active && !self.exiting {
            self.help
                .update(cx, |help, cx| help.mount_runtime_versions(cx));
        }
    }
    pub fn observe_help_runtime_versions(
        &mut self,
        owner: &Device,
        observation: HelpRuntimeVersionsObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.real_product_id != owner.real_product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
            || self.device.edition_id != owner.edition_id
            || self.device.layout_id != owner.layout_id
        {
            return false;
        }
        self.help.update(cx, |help, cx| {
            help.observe_runtime_versions(observation, cx)
        })
    }
}

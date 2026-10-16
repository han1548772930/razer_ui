use super::*;
use crate::features::{HelpRuntimeVersionsObservation, HelpRuntimeVersionsRequest};

impl SourceProductWorkspace {
    pub fn help_runtime_versions_request(&self, cx: &App) -> Option<HelpRuntimeVersionsRequest> {
        self.help.read(cx).runtime_versions_request()
    }
    pub fn observe_help_runtime_versions(
        &mut self,
        owner: &Device,
        observation: HelpRuntimeVersionsObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
        {
            return false;
        }
        self.help.update(cx, |help, cx| {
            help.observe_runtime_versions(observation, cx)
        })
    }
}

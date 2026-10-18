use super::*;
use crate::features::HelpSystemInfoRequested;

impl SourceProductWorkspace {
    pub fn accepts_help_system_info(&self, request: &HelpSystemInfoRequested, cx: &App) -> bool {
        self.device.product_id == request.product_id()
            && self.help.read(cx).accepts_system_info_request(request)
    }

    /// Accept the actual source systemInfoReducer object for this mounted Help.
    pub fn observe_help_system_info(
        &mut self,
        owner: &Device,
        request: &HelpSystemInfoRequested,
        source: Result<razer_device::system_info::SystemDetails, String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.device.product_id != owner.product_id
            || self.device.serial_number != owner.serial_number
            || self.device.device_container_id != owner.device_container_id
            || self.device.edition_id != owner.edition_id
            || self.device.layout_id != owner.layout_id
            || !self.accepts_help_system_info(request, cx)
        {
            return false;
        }
        self.help
            .update(cx, |help, cx| help.observe_system_info(request, source, cx))
    }
}

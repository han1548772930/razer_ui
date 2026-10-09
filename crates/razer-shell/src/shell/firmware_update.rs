//! Host tab lifecycle for the independent firmware preview/read-only view.
use gpui_kit::*;
use razer_app_pages::firmware_update::{CloseRequested, FirmwareUpdate};
use razer_model::model::Device;
impl super::AppShell {
    pub(super) fn open_firmware_update(
        &mut self,
        device: Option<Device>,
        preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some((page, _)) = &self.firmware_update {
            if !page.update(cx, |page, cx| page.allow_close(window, cx)) {
                self.navigate(super::Location::FirmwareUpdate, window, cx);
                return;
            }
        }
        self.initialize_firmware_update(device, preview, window, cx);
        self.navigate(super::Location::FirmwareUpdate, window, cx);
    }
    pub(super) fn initialize_firmware_update(
        &mut self,
        device: Option<Device>,
        preview: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = cx.new(|cx| FirmwareUpdate::new(device, preview, window, cx));
        if preview {
            page.update(cx, |page, cx| page.show_welcome_preview(window, cx));
        }
        let subscription =
            cx.subscribe_in(&page, window, |this, _, _: &CloseRequested, window, cx| {
                this.close_host_tab(super::host_tabs::HostTab::FirmwareUpdate, window, cx);
            });
        self.firmware_update = Some((page, subscription));
    }
}

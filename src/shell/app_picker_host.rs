//! Adapter from local workspace snapshots to the independent app picker.
use super::{AppShell, Location, Tab, app_picker::*, iot_popup};
use crate::{model::SetupStatus, resources};
use gpui_kit::*;

impl AppShell {
    pub(super) fn sync_app_picker(&self, window: &mut Window, cx: &mut Context<Self>) {
        let dashboard = self.dashboard_state.read(cx).snapshot();
        let order = dashboard.items_order.get("devices");
        let devices = self
            .devices
            .iter()
            .enumerate()
            .map(|(index, workspace)| {
                let workspace = workspace.read(cx);
                let device = workspace.device(cx);
                let icon = resources::dashboard_image(
                    device.product_id,
                    device.edition_id,
                    device.layout_id,
                )
                .unwrap_or("");
                let card_id = format!("open-{}", workspace.identity(cx));
                let position = order
                    .and_then(|order| order.iter().position(|id| *id == card_id))
                    .unwrap_or_else(|| order.map_or(0, Vec::len) + index);
                let mut item = PickerDevice::new(
                    device.product_id,
                    device.device_container_id.clone(),
                    device.display_name(),
                    icon,
                )
                .ready(device.setup_status == SetupStatus::Ready)
                .powered_off(
                    device
                        .power_status
                        .as_ref()
                        .is_some_and(|power| power.charging_status == "off"),
                )
                .section_position(position)
                .launchable(crate::features::has_product_workspace(device.product_id));
                for (locale, name) in &device.name.values {
                    item = item.localized_name(locale.clone(), name.clone());
                }
                item
            })
            .collect();
        // Page availability is a capability. No installed/native module results
        // have been read, so those catalog fields deliberately remain Unknown.
        let catalog = AppPickerCatalog::new(PickerApp::Synapse)
            .devices(devices)
            .bundled_modules([
                PickerModule::Alexa,
                PickerModule::AddWifi,
                PickerModule::ProfileMigration,
            ])
            .launchable_modules([
                PickerModule::Alexa,
                PickerModule::AddWifi,
                PickerModule::ProfileMigration,
            ])
            .launchable_apps([PickerApp::Synapse]);
        self.app_picker
            .update(cx, |picker, cx| picker.set_catalog(catalog, window, cx));
    }

    pub(super) fn handle_app_picker(
        &mut self,
        event: &AppPickerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            AppPickerEvent::Open(PickerTarget::Device {
                product_id,
                container_id,
            }) => {
                let key = self.devices.iter().find_map(|workspace| {
                    let workspace = workspace.read(cx);
                    let device = workspace.device(cx);
                    (device.product_id == *product_id
                        && device.device_container_id == *container_id
                        && crate::features::has_product_workspace(device.product_id))
                    .then(|| workspace.identity(cx))
                });
                if let Some(key) = key {
                    self.navigate(Location::Device(key), window, cx);
                }
            }
            AppPickerEvent::Open(PickerTarget::Module(PickerModule::Alexa)) => {
                self.navigate(Location::Alexa, window, cx);
            }
            AppPickerEvent::Open(PickerTarget::Module(PickerModule::ProfileMigration)) => {
                self.navigate(Location::ProfileMigration, window, cx);
            }
            AppPickerEvent::AddWifiDevice
            | AppPickerEvent::Open(PickerTarget::Module(PickerModule::AddWifi)) => {
                self.iot_popup = Some(iot_popup::open(iot_popup::DeviceKind::General, window, cx));
                cx.notify();
            }
            AppPickerEvent::Open(PickerTarget::App(PickerApp::Synapse)) => {
                self.navigate(Location::Main(Tab::Home), window, cx);
            }
            AppPickerEvent::Open(_) => {
                self.status = "此应用的窗口服务尚未连接。".into();
                cx.notify();
            }
            AppPickerEvent::Install(app) => {
                self.status = format!("{} 的安装服务尚未连接。", app.key());
                cx.notify();
            }
            AppPickerEvent::DiscoverMore => cx.open_url("https://razer.com/pc/software"),
            // The picker retains its local read state. Writing the real host's
            // readFeatures store requires a connected service.
            AppPickerEvent::ReadFeature(_) => {}
            AppPickerEvent::UnreadCleared => {}
        }
    }
}

//! Adapter from local workspace snapshots to the independent app picker.
use super::{AppShell, Location, Tab, app_picker::*, iot_popup, service_pages};
use crate::{model::SetupStatus, resources};
use gpui_kit::*;

impl AppShell {
    pub(super) fn sync_app_picker(&self, window: &mut Window, cx: &mut Context<Self>) {
        let dashboard = self.dashboard_state.read(cx).snapshot();
        let order = dashboard.items_order.get("devices");
        // The Hue module page is product 769's workspace; remember the local
        // device key so the picker can open it directly.
        let hue_device = self.devices.iter().find_map(|workspace| {
            let workspace = workspace.read(cx);
            (workspace.device(cx).product_id == 769).then(|| workspace.identity(cx))
        });
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
                        .is_some_and(|power| power.charging_status.eq_ignore_ascii_case("off")),
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
        // Modules whose page is implemented in this host open directly instead of
        // showing the installer gate. Chroma Studio is a separate source
        // application (/synapse/chroma-studio/), not the Chroma Dashboard.
        let mut bundled_modules = vec![
            PickerModule::Alexa,
            PickerModule::AddWifi,
            PickerModule::Macro,
            PickerModule::LinkedGames,
            PickerModule::Armory,
            // The Dashboard's FEEDBACK box targets the named
            // `feedback-synapse` window.  The local shell now has the
            // same page, so keep it visible and launchable without
            // waiting for the external installer/service state.
            PickerModule::Feedback,
            PickerModule::ProfileMigration,
        ];
        let mut launchable_modules = bundled_modules.clone();
        // The Philips Hue module page is implemented as product 769's workspace,
        // so it can only open directly while that device exists locally. The
        // Chroma application's other module pages are not implemented here and
        // keep their gate.
        if hue_device.is_some() {
            bundled_modules.push(PickerModule::PhilipsHue);
            launchable_modules.push(PickerModule::PhilipsHue);
        }
        let catalog = AppPickerCatalog::new(PickerApp::Synapse)
            .devices(devices)
            .bundled_modules(bundled_modules)
            // Chroma is a separately named local application. Keep it in the
            // picker as an installed/launchable app so its entry opens the
            // policy=5 `chroma-app` window without an installer service.
            .installed_modules([PickerApp::Chroma.key()])
            .native_apps([PickerApp::Chroma])
            .launchable_modules(launchable_modules)
            .launchable_apps([PickerApp::Synapse, PickerApp::Chroma]);
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
                self.open_module_tab(service_pages::ModulePage::Alexa, window, cx);
            }
            AppPickerEvent::Open(PickerTarget::Module(PickerModule::ProfileMigration)) => {
                self.navigate(Location::ProfileMigration, window, cx);
            }
            AppPickerEvent::Open(PickerTarget::Module(PickerModule::LinkedGames)) => {
                self.open_module_tab(service_pages::ModulePage::Profiles, window, cx);
            }
            AppPickerEvent::Open(PickerTarget::Module(PickerModule::Macro)) => {
                self.open_module_tab(service_pages::ModulePage::Macro, window, cx);
            }
            AppPickerEvent::Open(PickerTarget::Module(PickerModule::Armory)) => {
                self.open_module_tab(service_pages::ModulePage::Armory, window, cx);
            }
            AppPickerEvent::Open(PickerTarget::Module(PickerModule::Feedback)) => {
                self.open_module_tab(service_pages::ModulePage::Feedback, window, cx);
            }
            // The Hue module's page is the 769 product workspace. Open that
            // device directly when it is present locally; otherwise report the
            // missing device instead of an installer gate.
            AppPickerEvent::Open(PickerTarget::Module(PickerModule::PhilipsHue)) => {
                let key = self.devices.iter().find_map(|workspace| {
                    let workspace = workspace.read(cx);
                    (workspace.device(cx).product_id == 769).then(|| workspace.identity(cx))
                });
                match key {
                    Some(key) => self.navigate(Location::Device(key), window, cx),
                    None => {
                        self.status = "本地没有 Philips Hue 设备，模块页无法打开。".into();
                        cx.notify();
                    }
                }
            }
            AppPickerEvent::AddWifiDevice
            | AppPickerEvent::Open(PickerTarget::Module(PickerModule::AddWifi)) => {
                self.iot_popup = Some(iot_popup::open(iot_popup::DeviceKind::General, window, cx));
                cx.notify();
            }
            AppPickerEvent::Open(PickerTarget::App(PickerApp::Synapse)) => {
                self.navigate(Location::Main(Tab::Home), window, cx);
            }
            AppPickerEvent::Open(PickerTarget::App(PickerApp::Chroma)) => {
                self.open_chroma_window(cx);
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

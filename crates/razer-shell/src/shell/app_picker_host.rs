//! Adapter from local workspace snapshots to the independent app picker.
use super::{AppShell, Location, Tab, app_picker::*, iot_popup, service_pages};
use gpui_kit::*;
use razer_model::model::{Device, SetupStatus};

fn picker_device(device: &Device, has_local_page: bool, position: usize) -> PickerDevice {
    let icon = device_icon(
        device.product_id,
        device.dashboard.source_category.as_deref(),
        device.dashboard.sub_category.as_deref(),
    )
    .unwrap_or("");
    // Root te passes productName to c; the Dashboard's name can differ.
    let names = if device.product_name.values.is_empty() {
        &device.name
    } else {
        &device.product_name
    };
    let mut item = PickerDevice::new(
        device.product_id,
        device.device_container_id.clone(),
        names.get("en"),
        icon,
    )
    .ready(device.setup_status == SetupStatus::Ready)
    .mixer_failed(
        device.dashboard.device_init_status_fail.as_deref() == Some("mixer_system_check_failed"),
    )
    .powered_off(
        device
            .current_power_status()
            .is_some_and(|power| power.charging_status.eq_ignore_ascii_case("off")),
    )
    .section_position(position)
    .launchable(has_local_page)
    .open_without_installation(razer_dashboard::local_installation_entry(
        device,
        has_local_page,
    ));
    for (locale, name) in &names.values {
        item = item.localized_name(locale.clone(), name.clone());
    }
    item
}

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
                let has_local_page = workspace.has_local_page(cx);
                let card_id = format!("open-{}", workspace.identity(cx));
                let position = order
                    .and_then(|order| order.iter().position(|id| *id == card_id))
                    .unwrap_or_else(|| order.map_or(0, Vec::len) + index);
                picker_device(device, has_local_page, position)
            })
            .collect();
        // Page availability is a capability. No installed/native module results
        // have been read, so those catalog fields deliberately remain Unknown.
        // Modules whose page is implemented in this host open directly instead of
        // showing the installer gate. Chroma Studio is a separate source
        // application (/synapse/chroma-studio/), not the Chroma Dashboard.
        let mut bundled_modules = vec![
            PickerModule::AddWifi,
            PickerModule::Macro,
            PickerModule::ChromaStudio,
            PickerModule::LinkedGames,
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
            // picker as a launchable app so its entry opens the
            // policy=5 `chroma-app` window without an installer service.
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
                        && workspace.has_local_page(cx))
                    .then(|| workspace.identity(cx))
                });
                if let Some(key) = key {
                    self.navigate(Location::Device(key), window, cx);
                }
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
            AppPickerEvent::Open(PickerTarget::Module(PickerModule::ChromaStudio)) => {
                self.open_chroma_studio(cx);
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
            // The picker retains its local read state. Writing the real host's
            // readFeatures store requires a connected service.
            AppPickerEvent::ReadFeature(_) => {}
            AppPickerEvent::UnreadCleared => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::picker_device;
    use gpui_kit::AssetSource;
    use razer_model::{model::Device, product_preview::registered_preview};

    #[test]
    fn persisted_picker_devices_reach_current_category_vectors_through_the_shell_adapter() {
        for (pid, category) in [
            (579, "keyboard"),
            (689, "keyboard"),
            (720, "keyboard"),
            (122, "mouse"),
            (229, "mouse"),
            (179, "accessory"),
            (182, "mouse"),
        ] {
            let original = registered_preview(pid).unwrap();
            let mut restored: Device =
                serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
            restored.begin_local_session();
            if pid == 182 {
                restored.real_product_id = 183;
            }
            let item = picker_device(&restored, true, 0);
            assert_eq!(
                item.icon(),
                format!("synapse/app-picker-category/{category}.svg")
            );
            assert!(
                razer_assets::SynapseAssets
                    .load(item.icon())
                    .unwrap()
                    .is_some()
            );
            assert_eq!(restored.layout_id, 0);
            assert!(restored.dashboard.connection_observation.is_none());
        }
        let mut physical = registered_preview(179).unwrap();
        physical.serial_number = "physical-receiver".into();
        physical.dashboard.source_category = Some("ACCESSORY".into());
        assert_eq!(
            picker_device(&physical, true, 0).icon(),
            "synapse/app-picker-category/accessory.svg"
        );
        physical.dashboard.source_category = Some("UNKNOWN".into());
        assert!(picker_device(&physical, true, 0).icon().is_empty());
    }
}

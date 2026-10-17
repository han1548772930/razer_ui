//! Current lighting host actions. Unrecovered middleware must not acknowledge writes.
use super::*;
use razer_pages::features::keyboard_products::{
    KeyboardLightingCommand, KeyboardLightingRequested,
};
use razer_pages::features::mouse_products::MouseLightingRequested;

impl AppShell {
    pub(super) fn request_keyboard_lighting(
        &mut self,
        entity: Entity<ProductWorkspace>,
        request: &KeyboardLightingRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !entity.read(cx).accepts_keyboard_lighting(request, cx) {
            return;
        }
        let result = match &request.command {
            KeyboardLightingCommand::LaunchStudio => self.try_open_chroma_studio(cx).map(|_| None),
            KeyboardLightingCommand::OpenMappingChromaStudio => {
                self.open_mapping_chroma_studio(cx).map(|_| None)
            }
            KeyboardLightingCommand::OpenMappingMacro => {
                let existed = self.host_tabs.is_open(&host_tabs::HostTab::Macro);
                self.navigate(Location::Macro, window, cx);
                if !existed {
                    let handle = self.main_window;
                    cx.spawn(async move |owner, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(100))
                            .await;
                        let _ = handle.update(cx, |_, window, cx| {
                            let _ = owner.update(cx, |shell, cx| {
                                if shell.host_tabs.is_open(&host_tabs::HostTab::Macro) {
                                    shell.navigate(Location::Macro, window, cx);
                                    window.activate_window();
                                }
                            });
                        });
                    })
                    .detach();
                }
                Ok(None)
            }
            KeyboardLightingCommand::ManageChromaApps { install_required } => self
                .manage_product_chroma_apps(*install_required, cx)
                .map(|_| None),
            KeyboardLightingCommand::OpenSettings { chroma } => self
                .open_product_settings(*chroma, window, cx)
                .map(|_| None),
            KeyboardLightingCommand::Observe => {
                Err("Current lighting service observations are not connected".into())
            }
            KeyboardLightingCommand::InstallChroma => {
                Err("Current local Chroma installation service is not connected".into())
            }
            KeyboardLightingCommand::Middleware(_) => {
                Err("Current lighting middleware/device adapter is not implemented".into())
            }
        };
        entity.update(cx, |workspace, cx| {
            workspace.complete_keyboard_lighting(request, result, window, cx);
        });
    }
    pub(super) fn request_mouse_lighting(
        &mut self,
        entity: Entity<ProductWorkspace>,
        request: &MouseLightingRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !entity.update(cx, |workspace, cx| {
            workspace.accepts_mouse_lighting(request, cx)
        }) {
            return;
        }
        let result = match request.command() {
            "launchChromaStudio" => self.try_open_chroma_studio(cx).map(|_| None),
            "openChromaStudio" => self.open_mapping_chroma_studio(cx).map(|_| None),
            "openDevicesModules" => self.open_mapping_chroma_installer(cx).map(|_| None),
            "focusChromaAppsBabel" => self.manage_product_chroma_apps(false, cx).map(|_| None),
            "openLightingSettings" => match request.payload()["chroma"].as_bool() {
                Some(chroma) => self.open_product_settings(chroma, window, cx).map(|_| None),
                None => Err("Missing current lighting settings host identity".into()),
            },
            "triggerInstall" => {
                Err("Current local Chroma installation service is not connected".into())
            }
            "focusChromaApps" => self
                .manage_product_chroma_apps(
                    request.payload()["installRequired"]
                        .as_bool()
                        .unwrap_or(true),
                    cx,
                )
                .map(|_| None),
            "checkIsFullyInstall" | "getMemoryStorageItem" => {
                Err("Current lighting service observations are not connected".into())
            }
            _ => Err("Current mouse lighting middleware/device adapter is not implemented".into()),
        };
        entity.update(cx, |workspace, cx| {
            workspace.complete_mouse_lighting(request, result, window, cx);
        });
    }
    pub(super) fn manage_product_chroma_apps(
        &mut self,
        install_required: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let existing = display_window::is_open(&"chroma-app".into(), cx);
        // Source does not request installation when the named Chroma host is
        // already open. Otherwise it resolves the installer before window.open.
        if !existing && install_required {
            return Err("Current local Chroma installation service is not connected".into());
        }
        self.try_open_chroma_window(cx)?;
        if existing {
            let host = self
                .chroma_host
                .as_ref()
                .ok_or_else(|| "Chroma host was not created".to_owned())?;
            host.update(cx, |host, cx| host.activate_apps(cx));
        }
        // A newly created host follows the original /chroma-app/dashboard URL.
        Ok(())
    }

    fn open_mapping_chroma_studio(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let existing = display_window::is_open(&"chroma-app".into(), cx);
        self.try_open_chroma_window(cx)?;
        if existing {
            // The current MapLighting caller focuses an existing Studio tab,
            // or activates the host's CHROMA_STUDIO service client immediately.
            self.try_open_chroma_studio(cx)
        } else {
            // Current URL action focus-module-installer schedules userOpen of
            // Chroma Studio in the Dashboard root after 3500 ms.
            let host = self
                .chroma_host
                .as_ref()
                .ok_or_else(|| "Chroma host was not created".to_owned())?;
            host.update(cx, |host, cx| host.schedule_mapping_studio(cx));
            Ok(())
        }
    }

    fn open_mapping_chroma_installer(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let existing = display_window::is_open(&"chroma-app".into(), cx);
        self.try_open_chroma_window(cx)?;
        // Current /162 _J.openDevicesModules only focuses an existing host.
        // Its new-window URL supplies the same delayed Dashboard action.
        if !existing {
            let host = self
                .chroma_host
                .as_ref()
                .ok_or_else(|| "Chroma host was not created".to_owned())?;
            host.update(cx, |host, cx| host.schedule_mapping_studio(cx));
        }
        Ok(())
    }
}

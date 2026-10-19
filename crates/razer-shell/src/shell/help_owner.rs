//! Source owner tutorial persistence and observed host-tab/window focus.
use super::*;
use razer_pages::features::{HelpOwnerCommand, HelpOwnerRequested};

impl AppShell {
    pub(super) fn request_help_owner(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &HelpOwnerRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.devices.contains(workspace) || !workspace.read(cx).accepts_help_owner(request, cx)
        {
            return;
        }
        match request.command() {
            HelpOwnerCommand::Observe => {
                if let Some(product_id) = request.product_names_source_product_id() {
                    // Original loadDataToUI reads connectedDeviceInfo and finds
                    // the first DeviceInfo.productId match. This is our local
                    // reconstruction of that preference, not a vendor read.
                    match razer_storage::source_local::path().and_then(|path| {
                        razer_storage::source_local::get(&path, "connectedDeviceInfo")
                    }) {
                        Ok(Some(serde_json::Value::Array(records))) => {
                            if let Some(names) = records
                                .iter()
                                .find(|row| {
                                    row.get("productId").and_then(serde_json::Value::as_u64)
                                        == Some(product_id as u64)
                                })
                                .and_then(|row| row.get("productName"))
                            {
                                let device = workspace.read(cx).snapshot(cx);
                                workspace.update(cx, |workspace, cx| {
                                    workspace.observe_help_product_names(
                                        &device,
                                        request,
                                        names.clone(),
                                        cx,
                                    )
                                });
                            }
                        }
                        Ok(None) => {}
                        Ok(Some(_)) => {
                            eprintln!("[help-title] original connectedDeviceInfo array unavailable")
                        }
                        Err(error) => {
                            eprintln!("[help-title] local source preference read failed: {error:#}")
                        }
                    }
                }
                if let Some(key) = request.tutorial_storage_key() {
                    match razer_storage::source_local::path().and_then(|path| {
                        razer_storage::source_local::get(&path, &key)
                    }) {
                        Ok(value) => {
                            workspace.update(cx, |workspace, cx| {
                                workspace.observe_help_tutorial_dismissal(request, value, cx)
                            });
                        }
                        Err(error) => eprintln!(
                            "[help-tutorial] source local preference read failed: {error:#}"
                        ),
                    }
                    if request.host_focus_actions().is_some() {
                        self.initialize_help_owner_focus(workspace, request, window, cx);
                    }
                }
                if let Some(key) = request.whats_new_storage_key() {
                    match razer_storage::source_local::path().and_then(|path| {
                        razer_storage::source_local::get(&path, key)
                    }) {
                        Ok(value) => {
                            workspace.update(cx, |workspace, cx| {
                                workspace.observe_help_whats_new_dismissal(request, value, cx)
                            });
                        }
                        Err(error) => {
                            eprintln!("[owner-whats-new] local preference read failed: {error:#}")
                        }
                    }
                }
                // PID 1398's renderDeviceName starts at "" in the source.
                // Its native THX endpoint name producer remains an explicit
                // gap; only observe_help_render_device_name may update it.
            }
            HelpOwnerCommand::DismissTutorial => {
                let mut persisted = true;
                for (key, value) in request.tutorial_storage_writes() {
                    if let Err(error) = razer_storage::source_local::path().and_then(|path| {
                        razer_storage::source_local::set(&path, &key, &value)
                    }) {
                        eprintln!(
                            "[help-tutorial] source local preference write failed: {error:#}"
                        );
                        persisted = false;
                        break;
                    }
                }
                if let Some((key, _)) = request.tutorial_storage_write().filter(|_| persisted) {
                    let result = razer_storage::source_local::path().and_then(|path| {
                        razer_storage::source_local::get(&path, &key)
                    });
                    match result {
                        Ok(value) => {
                            workspace.update(cx, |workspace, cx| {
                                workspace.observe_help_tutorial_dismissal(request, value, cx)
                            });
                        }
                        Err(error) => eprintln!(
                            "[help-tutorial] source local preference write/readback failed: {error:#}"
                        ),
                    }
                }
            }
            HelpOwnerCommand::DismissWhatsNew => {
                if let Some(key) = request.whats_new_storage_key() {
                    let result = razer_storage::source_local::path().and_then(|path| {
                        razer_storage::source_local::set(
                            &path,
                            key,
                            &serde_json::Value::Bool(true),
                        )?;
                        razer_storage::source_local::get(&path, key)
                    });
                    match result {
                        Ok(value) => {
                            workspace.update(cx, |workspace, cx| {
                                workspace.observe_help_whats_new_dismissal(request, value, cx)
                            });
                        }
                        Err(error) => eprintln!(
                            "[owner-whats-new] dismissal write/readback failed: {error:#}"
                        ),
                    }
                }
            }
            HelpOwnerCommand::SetSimultaneousAudio => {
                if let Some(message) = request.simultaneous_audio_message() {
                    // Preserve the exact source request. Neither local Redux
                    // state nor sending this request is a successful device write.
                    eprintln!(
                        "[smart-switch] native ON_SET_SIMULTANEOUS_AUDIO adapter is unavailable: {message}"
                    );
                }
            }
            HelpOwnerCommand::SetAudioEnhancements => {
                for message in request.audio_enhancement_messages() {
                    eprintln!(
                        "[audio-enhancements] original native adapter is unavailable: {message}"
                    );
                }
            }
            HelpOwnerCommand::SetAudioSound => {
                self.request_thx_sound(workspace.clone(), request.clone(), window, cx);
                for message in request.audio_sound_messages() {
                    if message.get("transport").is_some_and(|transport| {
                        !matches!(
                            transport["type"].as_str(),
                            Some("ON_SET_SPATIAL_ENABLE" | "ON_SET_KEYMAPPING")
                        )
                    }) {
                        eprintln!(
                            "[audio-sound] original source adapter is unavailable: {message}"
                        );
                    }
                }
            }
            HelpOwnerCommand::OpenSoundVolume | HelpOwnerCommand::OpenAudioProperties => {
                let property = if request.command() == HelpOwnerCommand::OpenSoundVolume {
                    razer_platform::system::Properties::Volume
                } else {
                    razer_platform::system::Properties::Sound
                };
                if let Err(error) = razer_platform::system::open(property) {
                    eprintln!("[audio-sound] original Windows sound command failed: {error:#}");
                }
            }
            HelpOwnerCommand::OpenWindowsSoundSettings => {
                if request.ms_settings_section() == Some("sound") {
                    if let Err(error) = razer_platform::system::open_sound_settings() {
                        // The original helper is a void native dispatch and
                        // mounts no extra result/success/error dialog.
                        eprintln!("[help-thx] source msSettings(sound) failed: {error:#}");
                    }
                }
            }
        }
    }
    fn initialize_help_owner_focus(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        request: &HelpOwnerRequested,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let frame =
            host_tabs::HostTabs::frame_name(&Location::Device(workspace.read(cx).identity(cx)));
        let selected = host_tabs::HostTabs::frame_name(&self.location);
        match host_tabs::host_window::status(window) {
            Ok(status) => {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_help_host_focus(
                        request,
                        &selected,
                        &frame,
                        status.and_then(|s| s.is_visible),
                        status.and_then(|s| s.is_minimized),
                        cx,
                    )
                });
            }
            Err(error) => {
                eprintln!("[help-tutorial] initial host status unavailable: {error:#}");
                // Current1453 chunk543 module6092 emitInitial catch consults
                // document.hidden instead of retrying BrowserWindow status.
                // The retained product frame and GPUI presentation visibility
                // supply that local view observation, not a native status ACK.
                let document_hidden = selected != frame || !window.is_visible();
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_help_initial_host_failure(request, document_hidden, cx)
                });
            }
        }
    }
    pub(super) fn observe_help_owner_tab(&mut self, next: &Location, cx: &mut Context<Self>) {
        let selected = host_tabs::HostTabs::frame_name(next);
        for workspace in &self.devices {
            let Some(request) = workspace
                .read(cx)
                .help_owner_observation_request(cx)
                .filter(|r| r.product_id() == 1453)
            else {
                continue;
            };
            let frame =
                host_tabs::HostTabs::frame_name(&Location::Device(workspace.read(cx).identity(cx)));
            workspace.update(cx, |workspace, cx| {
                workspace.observe_help_active_tab(&request, &selected, &frame, cx)
            });
        }
    }
    pub(super) fn observe_help_owner_window_focus(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let selected = host_tabs::HostTabs::frame_name(&self.location);
        let focused = window.is_window_active();
        let status = if focused {
            None
        } else {
            match host_tabs::host_window::status(window) {
                Ok(status) => status,
                Err(error) => {
                    eprintln!("[help-tutorial] host status unavailable: {error:#}");
                    return;
                }
            }
        };
        for workspace in &self.devices {
            let Some(request) = workspace
                .read(cx)
                .help_owner_observation_request(cx)
                .filter(|r| r.product_id() == 1453)
            else {
                continue;
            };
            let frame =
                host_tabs::HostTabs::frame_name(&Location::Device(workspace.read(cx).identity(cx)));
            workspace.update(cx, |workspace, cx| {
                if focused {
                    workspace.observe_help_active_tab(&request, &selected, &frame, cx)
                } else {
                    workspace.observe_help_host_focus(
                        &request,
                        &selected,
                        &frame,
                        status.and_then(|s| s.is_visible),
                        status.and_then(|s| s.is_minimized),
                        cx,
                    )
                }
            });
        }
    }
}

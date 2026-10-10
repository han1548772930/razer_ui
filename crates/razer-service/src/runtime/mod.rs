//! Worker dispatch only. Wire contract and client are owned by razer-ipc.
use razer_ipc::{FRAME_PREFIX, RequestEnvelope, ResponseEnvelope, ServiceRequest, read_frame};
use std::{io::Write, time::Duration};
#[cfg(windows)]
#[path = "windows/native.rs"]
mod native;
#[path = "portable.rs"]
mod portable;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(4);

fn write_response(response: &ResponseEnvelope) -> anyhow::Result<()> {
    let json = serde_json::to_string(response)?;
    let output = std::io::stdout();
    let mut output = output.lock();
    writeln!(output, "{FRAME_PREFIX}{json}")?;
    output.flush()?;
    Ok(())
}

/// `main` must dispatch `--service-worker` here before initializing GPUI, then
/// exit with this status. Never invoke this function from a UI callback.
pub fn run_worker() -> i32 {
    #[cfg(windows)]
    let mut runtime = native::NativeRuntime::new();
    let mut portable = portable::PortableRuntime::default();
    let mut host_storage = razer_storage::host::HostStorage::default();
    let input = std::io::stdin();
    let mut input = input.lock();
    loop {
        let envelope = match read_frame(&mut input) {
            Ok(Some(line)) => match serde_json::from_slice::<RequestEnvelope>(&line) {
                Ok(envelope) => envelope,
                Err(_) => return 2,
            },
            Ok(None) => return 0,
            Err(_) => return 2,
        };
        let shutdown = matches!(envelope.request, ServiceRequest::Shutdown);
        let result = match envelope.request {
            ServiceRequest::KeyboardLayoutRead => razer_platform::keyboard_layout::get()
                .map(|layout| serde_json::json!({"layout":layout})),
            ServiceRequest::WindowsServiceStart { name } => {
                razer_platform::windows_service_status::start(&name)
                    .map(|exit_code| serde_json::json!({"name":name,"exit_code":exit_code}))
            }
            ServiceRequest::WindowsServiceStop { name } => {
                razer_platform::windows_service_status::stop(&name)
                    .map(|exit_code| serde_json::json!({"name":name,"exit_code":exit_code}))
            }
            ServiceRequest::WindowsServiceStatus { name } => {
                razer_platform::windows_service_status::query(&name)
                    .map(|status| serde_json::json!({"name":name,"status":status}))
            }
            ServiceRequest::WheelScrollLinesRead => {
                razer_platform::wheel_scroll::get().map(|lines| serde_json::json!({"lines":lines}))
            }
            ServiceRequest::WheelScrollLinesWrite { lines } => razer_platform::wheel_scroll::set(
                lines,
            )
            .map(|accepted| serde_json::json!({"requested_lines":lines,"accepted":accepted})),
            ServiceRequest::HostStorageView { view } => {
                host_storage.register_view(view);
                Ok(serde_json::json!({"registered": true}))
            }
            ServiceRequest::HostStorageClose { view_id } => {
                host_storage.close_view(view_id);
                Ok(serde_json::json!({"closed": true}))
            }
            ServiceRequest::HostStorageCall { call } => host_storage
                .call(call)
                .and_then(|result| serde_json::to_value(result).map_err(Into::into)),
            ServiceRequest::HostStorageEvents { view_id } => host_storage
                .drain_events(view_id)
                .and_then(|events| serde_json::to_value(events).map_err(Into::into)),
            request @ (ServiceRequest::AudioDevices
            | ServiceRequest::GlobalShortcuts
            | ServiceRequest::RegisterShortcut { .. }
            | ServiceRequest::UnregisterShortcut { .. }
            | ServiceRequest::EnableGlobalShortcuts { .. }
            | ServiceRequest::ShortcutEvents
            | ServiceRequest::AudioVolumeRead { .. }
            | ServiceRequest::AudioVolumeWrite { .. }
            | ServiceRequest::AudioEndpoints { .. }
            | ServiceRequest::AudioNotificationsEnable { .. }
            | ServiceRequest::AudioNotificationsDrain
            | ServiceRequest::AudioRoutingEnable { .. }
            | ServiceRequest::AudioRouteDevice { .. }
            | ServiceRequest::AudioRouterEvents
            | ServiceRequest::ForegroundMonitorStart { .. }
            | ServiceRequest::ForegroundMonitorStop { .. }
            | ServiceRequest::ForegroundMonitorEvents { .. }
            | ServiceRequest::HidNodes
            | ServiceRequest::HidNodeReports { .. }
            | ServiceRequest::HidNodeDpiStagesRead { .. }
            | ServiceRequest::HidNodeDpiStagesWrite { .. }
            | ServiceRequest::HidNodeRead { .. }
            | ServiceRequest::HidNodeWrite { .. }
            | ServiceRequest::HidNodeKeyboardBrightnessRead { .. }
            | ServiceRequest::HidNodeKeyboardBrightnessWrite { .. }
            | ServiceRequest::HidNodeMixerRead { .. }
            | ServiceRequest::HidNodeMixerWrite { .. }
            | ServiceRequest::HidNodeMixerEqWrite { .. }
            | ServiceRequest::HidNodeMixerRouteRead { .. }
            | ServiceRequest::HidNodeMixerRouteWrite { .. }
            | ServiceRequest::HidNodeMixerRestartStreams { .. }
            | ServiceRequest::HidNodeReceiverStatus { .. }) => portable.request(request),
            request => {
                #[cfg(windows)]
                {
                    runtime.request(request)
                }
                #[cfg(not(windows))]
                {
                    match request {
                        ServiceRequest::Shutdown => Ok(serde_json::json!({"stopped":true})),
                        _ => Err(anyhow::anyhow!(
                            "此操作需要尚未移植的平台服务或设备身份适配"
                        )),
                    }
                }
            }
        };
        #[cfg(windows)]
        let fatal = runtime.is_poisoned();
        #[cfg(not(windows))]
        let fatal = false;
        let response = match result {
            Ok(data) => ResponseEnvelope {
                id: envelope.id,
                data: Some(data),
                error: None,
                fatal,
            },
            Err(error) => ResponseEnvelope {
                id: envelope.id,
                data: None,
                error: Some(format!("{error:#}")),
                fatal,
            },
        };
        if shutdown {
            // ServiceClient terminates its child after receiving this reply.
            // Finish owned WASAPI pumps, COM notifications and receiver input
            // readers first, so a successful frame cannot race their teardown.
            // This ordering is an owned Rust worker guarantee, not a claim
            // that all original host/plugin lifecycle chains are implemented.
            drop(portable);
            #[cfg(windows)]
            drop(runtime);
            return if write_response(&response).is_err() {
                2
            } else if fatal {
                3
            } else {
                0
            };
        }
        if write_response(&response).is_err() {
            return 2;
        }
        if fatal {
            return 3;
        }
    }
}

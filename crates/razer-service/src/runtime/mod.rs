//! Worker dispatch only. Wire contract and client are owned by razer-ipc.
use razer_ipc::{FRAME_PREFIX, RequestEnvelope, ResponseEnvelope, ServiceRequest, read_frame};
use std::io::Write;
#[path = "camera_controller.rs"]
mod camera_controller;
#[path = "controller_calibration.rs"]
mod controller_calibration;
#[path = "controller_lighting.rs"]
mod controller_lighting;
#[path = "controller_triggers.rs"]
mod controller_triggers;
mod feature_submission;
#[path = "keyboard_submission_controller.rs"]
mod keyboard_submission_controller;
#[path = "keyboard_wristrest_controller.rs"]
mod keyboard_wristrest_controller;
mod leviathan_events;
#[cfg(windows)]
mod lighting_engine;
mod lighting_engine_session;
#[path = "mouse_indicator_led_controller.rs"]
mod mouse_indicator_led_controller;
mod mouse_philips_controller;
#[path = "mouse_runtime_artwork_controller.rs"]
mod mouse_runtime_artwork_controller;
mod mouse_tracking_controller;
#[cfg(windows)]
#[path = "windows/native.rs"]
mod native;
#[path = "portable.rs"]
mod portable;
mod receiver_settings;
#[path = "seiren_audio_capture_controller.rs"]
mod seiren_audio_capture_controller;
mod shutdown;
#[path = "thx_controller.rs"]
mod thx_controller;

pub(crate) fn mapping_client() -> anyhow::Result<crate::mapping::owner::Client> {
    #[cfg(windows)]
    { native::mapping_client() }
    #[cfg(not(windows))]
    { anyhow::bail!("Native mapping execution is unavailable on this platform") }
}

fn write_response(response: &ResponseEnvelope) -> anyhow::Result<()> {
    let json = serde_json::to_string(response)?;
    let output = std::io::stdout();
    let mut output = output.lock();
    writeln!(output, "{FRAME_PREFIX}{json}")?;
    output.flush()?;
    Ok(())
}

fn cleanup_owners(
    portable: &mut portable::PortableRuntime,
    #[cfg(windows)] runtime: &mut native::NativeRuntime,
) -> shutdown::Cleanup {
    let mut cleanup = shutdown::Cleanup::default();
    #[cfg(windows)]
    cleanup.record("native", runtime.request(ServiceRequest::Shutdown));
    let mut portable_response = ResponseEnvelope {
        id: 0, data: Some(serde_json::json!({"stopped":true})), error: None, fatal: false,
    };
    portable.shutdown().apply(&mut portable_response);
    if let Some(error) = portable_response.error { cleanup.fail("portable", error); }
    else if let Some(data) = portable_response.data { cleanup.record("portable", Ok(data)); }
    cleanup
}

fn cleanup_without_reply(
    portable: &mut portable::PortableRuntime,
    #[cfg(windows)] runtime: &mut native::NativeRuntime,
) -> bool {
    let cleanup = cleanup_owners(portable, #[cfg(windows)] runtime);
    cleanup.log_failures();
    cleanup.succeeded()
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
                Err(_) => {
                    cleanup_without_reply(&mut portable, #[cfg(windows)] &mut runtime);
                    return 2;
                }
            },
            Ok(None) => return if cleanup_without_reply(&mut portable, #[cfg(windows)] &mut runtime) { 0 } else { 2 },
            Err(_) => {
                cleanup_without_reply(&mut portable, #[cfg(windows)] &mut runtime);
                return 2;
            }
        };
        let shutdown = matches!(envelope.request, ServiceRequest::Shutdown);
        let result = match envelope.request {
            ServiceRequest::HelpSystemInfo { product_id, query } => {
                crate::help_system_info::query(product_id, query)
                    .and_then(|response| serde_json::to_value(response).map_err(Into::into))
            }
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
            | ServiceRequest::MouseInputMonitoringStart { .. }
            | ServiceRequest::MouseInputMonitoringPoll { .. }
            | ServiceRequest::MouseInputMonitoringStop { .. }
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
            | ServiceRequest::CameraDiscover { .. }
            | ServiceRequest::CameraStart { .. }
            | ServiceRequest::CameraTaskStart { .. }
            | ServiceRequest::CameraPoll { .. }
            | ServiceRequest::CameraCancel { .. }
            | ServiceRequest::ThxStart { .. }
            | ServiceRequest::ThxOwnerStart { .. }
            | ServiceRequest::ThxOwnerSubmit { .. }
            | ServiceRequest::ThxOwnerRelease { .. }
            | ServiceRequest::ThxPoll { .. }
            | ServiceRequest::ThxCancel { .. }
            | ServiceRequest::HidNodeReports { .. }
            | ServiceRequest::ControllerTriggers { .. }
            | ServiceRequest::ControllerLighting { .. }
            | ServiceRequest::ControllerTriggerCalibrationStart { .. }
            | ServiceRequest::ControllerTriggerCalibrationPoll { .. }
            | ServiceRequest::ControllerTriggerCalibrationCancel { .. }
            | ServiceRequest::HidNodeDpiStagesRead { .. }
            | ServiceRequest::HidNodeDpiStagesWrite { .. }
            | ServiceRequest::HidNodeMouseCurrentDpi { .. }
            | ServiceRequest::HidNodeMouseBrightness { .. }
            | ServiceRequest::HidNodeMouseDynamic { .. }
            | ServiceRequest::HidNodeMouseCurrentPolling { .. }
            | ServiceRequest::HidNodeMouseCurrentPower { .. }
            | ServiceRequest::HidNodeMouseScroll { .. }
            | ServiceRequest::MouseTrackingStart { .. }
            | ServiceRequest::MouseTrackingPoll { .. }
            | ServiceRequest::MouseTrackingCancel { .. }
            | ServiceRequest::MousePhilipsStart { .. }
            | ServiceRequest::MousePhilipsPoll { .. }
            | ServiceRequest::MousePhilipsCancel { .. }
            | ServiceRequest::HidNodeLeviathan { .. }
            | ServiceRequest::LeviathanEventsStart { .. }
            | ServiceRequest::LeviathanEventsPoll { .. }
            | ServiceRequest::LeviathanEventsStop { .. }
            | ServiceRequest::HidNodeMouseIndicatorLed { .. }
            | ServiceRequest::MouseIndicatorLedStart { .. }
            | ServiceRequest::MouseIndicatorLedPoll { .. }
            | ServiceRequest::MouseIndicatorLedStop { .. }
            | ServiceRequest::HidNodeSeirenEq { .. }
            | ServiceRequest::KeyboardIndicator { .. }
            | ServiceRequest::KeyboardLatestRestore { .. }
            | ServiceRequest::KeyboardLegacyCalibration { .. }
            | ServiceRequest::MouseReceiverLegacy { .. }
            | ServiceRequest::HyperpollingReceiver { .. }
            | ServiceRequest::KeyboardAnalogStart { .. }
            | ServiceRequest::KeyboardAnalogPoll { .. }
            | ServiceRequest::KeyboardAnalogCancel { .. }
            | ServiceRequest::KeyboardWristrestStart { .. }
            | ServiceRequest::KeyboardWristrestPoll { .. }
            | ServiceRequest::KeyboardWristrestStop { .. }
            | ServiceRequest::MouseRuntimeArtworkStart { .. }
            | ServiceRequest::MouseRuntimeArtworkPoll { .. }
            | ServiceRequest::MouseRuntimeArtworkStop { .. }
            | ServiceRequest::SeirenAudioCaptureStart { .. }
            | ServiceRequest::SeirenAudioCapturePoll { .. }
            | ServiceRequest::SeirenAudioCaptureStop { .. }
            | ServiceRequest::SeirenAudioPlaybackStart { .. }
            | ServiceRequest::SeirenAudioPlaybackPoll { .. }
            | ServiceRequest::SeirenAudioPlaybackStop { .. }
            | ServiceRequest::HidNodeRead { .. }
            | ServiceRequest::HidNodeMouseReceiverRead { .. }
            | ServiceRequest::HidNodeMouseReceiverDpiStages { .. }
            | ServiceRequest::HidNodeMouseReceiverPolling { .. }
            | ServiceRequest::HidNodeMouseReceiverIdle { .. }
            | ServiceRequest::HidNodeWrite { .. }
            | ServiceRequest::HidNodeHanboCooler { .. }
            | ServiceRequest::HidNodeHanboCoolerRead { .. }
            | ServiceRequest::HidNodeHanboStartupRead { .. }
            | ServiceRequest::HidNodeKeyboardBrightnessRead { .. }
            | ServiceRequest::HidNodeKeyboardReceiverBrightness { .. }
            | ServiceRequest::HidNodeKeyboardMetadataRead { .. }
            | ServiceRequest::HidNodeKeyboardSerialRead { .. }
            | ServiceRequest::HidNodeKeyboardRelayMetadataRead { .. }
            | ServiceRequest::HidNodeKeyboardRelaySerialRead { .. }
            | ServiceRequest::HidNodeKeyboardBrightnessWrite { .. }
            | ServiceRequest::HidNodeMixerRead { .. }
            | ServiceRequest::HidNodeMixerLocalWrite { .. }
            | ServiceRequest::HidNodeMixerWrite { .. }
            | ServiceRequest::HidNodeMixerEqWrite { .. }
            | ServiceRequest::HidNodeMixerRouteRead { .. }
            | ServiceRequest::HidNodeMixerRouteWrite { .. }
            | ServiceRequest::HidNodeMixerRestartStreams { .. }
            | ServiceRequest::HidNodeReceiverStatus { .. }
            | ServiceRequest::HidNodeReceiverStatusForSource { .. }
            | ServiceRequest::HidNodeReceiverDockMode { .. }) => portable.request(request),
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
        let mut response = match result {
            Ok(data) => ResponseEnvelope {
                id: envelope.id,
                data: Some(data),
                error: None,
                fatal: false,
            },
            Err(error) => ResponseEnvelope {
                id: envelope.id,
                data: None,
                error: Some(format!("{error:#}")),
                fatal: false,
            },
        };
        if shutdown {
            portable.shutdown().apply(&mut response);
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
            } else {
                0
            };
        }
        if write_response(&response).is_err() {
            cleanup_without_reply(&mut portable, #[cfg(windows)] &mut runtime);
            return 2;
        }
    }
}

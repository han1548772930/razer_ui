//! Independent Windows service owners and device transport dispatch.
use super::ServiceRequest;
use anyhow::{Context as _, bail};
use razer_device::backend::HidNode;
use serde_json::{Value, json};
pub(super) fn broker_collection_metadata(node: &HidNode) -> anyhow::Result<Value> {
    let path = std::str::from_utf8(&node.path)?;
    hid::enumerate_collection(node.vendor_id, node.product_id, path)
}

pub(super) fn shutdown_ble_cleanup() -> anyhow::Result<()> {
    device_reads::shutdown_ble_cleanup()
}

pub(super) fn ble_cleanup_pending_count() -> usize {
    device_reads::ble_cleanup_pending_count()
}

pub(super) fn leviathan_event_source(
    target: &razer_device::leviathan::EventTarget,
) -> anyhow::Result<Box<dyn razer_device::receiver_pairing::HardwareEvents>> {
    device_reads::leviathan_event_source(target)
}
pub(super) fn keyboard_latest_restore(
    target: &razer_device::keyboard_latest_protocol::Target,
    operation: &razer_device::keyboard_latest_protocol::Operation,
) -> anyhow::Result<Value> {
    device_reads::keyboard_latest_restore(target, operation)
}
pub(super) fn keyboard_latest_event_source(
    target: &razer_device::keyboard_latest_protocol::Target,
) -> anyhow::Result<Box<dyn razer_device::receiver_pairing::HardwareEvents>> {
    device_reads::keyboard_latest_event_source(target)
}
pub(super) fn keyboard_submission(
    target: &razer_device::device_reads::DeviceReadTarget,
    profile_guid: &str,
    intent: razer_device::keyboard_submission::Intent,
    canceled: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<Value> {
    device_reads::keyboard_submission(target, profile_guid, intent, canceled)
}
pub(super) fn keyboard_indicator(
    target: &razer_device::device_reads::DeviceReadTarget,
    change: &razer_device::keyboard_indicator::Change,
) -> anyhow::Result<Value> {
    device_reads::keyboard_indicator(target, change)
}
pub(super) fn keyboard_profile_inventory(
    target: &razer_device::device_reads::DeviceReadTarget,
) -> anyhow::Result<Value> {
    device_reads::keyboard_metadata(target, super::KeyboardRead::ProfileInventory)
}
pub(super) fn keyboard_onboard_profiles(
    target: &razer_device::device_reads::DeviceReadTarget,
    options: &razer_device::keyboard_onboard_profiles::ReadOptions,
) -> anyhow::Result<Value> {
    device_reads::keyboard_metadata(target, super::KeyboardRead::OnboardProfiles(options))
}
pub(super) fn keyboard_legacy_calibration(
    target: &razer_device::device_reads::DeviceReadTarget,
    operation: &razer_device::keyboard_legacy_calibration::Operation,
) -> anyhow::Result<Value> {
    device_reads::keyboard_legacy_calibration(target, operation)
}
pub(super) fn mouse_receiver_legacy(
    target: &razer_device::device_reads::DeviceReadTarget,
    change: &razer_device::mouse_receiver_legacy::Change,
) -> anyhow::Result<Value> {
    device_reads::mouse_receiver_legacy(target, change)
}
pub(super) fn hyperpolling_receiver(
    path: &str,
    container: &str,
    command: razer_device::hyperpolling_receiver::Command,
    transaction: u8,
) -> anyhow::Result<Value> {
    hyperpolling_receiver::request(path, container, command, transaction)
}
pub(super) fn keyboard_wristrest(
    target: &razer_device::keyboard_wristrest::Target,
    canceled: &std::sync::atomic::AtomicBool,
    publish: &mut impl FnMut(razer_device::keyboard_wristrest::Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    device_reads::keyboard_wristrest(target, canceled, publish)
}
pub(super) fn mouse_runtime_artwork(
    target: &razer_device::mouse_runtime_artwork::Target,
    canceled: &std::sync::atomic::AtomicBool,
    publish: &mut impl FnMut(razer_device::mouse_runtime_artwork::Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    device_reads::mouse_runtime_artwork(target, canceled, publish)
}
pub(super) fn mouse_indicator_led_monitor(
    target: &razer_device::mouse_indicator_led::Target,
    canceled: &std::sync::atomic::AtomicBool,
    publish: &mut impl FnMut(razer_device::mouse_indicator_led::Notification) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    device_reads::mouse_indicator_led_monitor(target, canceled, publish)
}
pub(super) fn mouse_tracking(
    target: &razer_device::device_reads::DeviceReadTarget,
    setting: razer_device::mouse_tracking::TrackingSetting,
    canceled: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<razer_device::mouse_tracking::SubmissionReport> {
    device_reads::mouse_tracking(target, setting, canceled)
}
pub(super) fn mouse_philips(
    target: &razer_device::device_reads::DeviceReadTarget,
    setting: razer_device::mouse_philips::Setting,
    canceled: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<razer_device::mouse_philips::SubmissionReport> {
    device_reads::mouse_philips(target, setting, canceled)
}
pub(super) fn seiren_audio_capture(
    canceled: &std::sync::atomic::AtomicBool,
    options: &razer_device::seiren_microphone::CaptureOptions,
    publish: &mut dyn FnMut(razer_device::seiren_audio_capture::CaptureFrame) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    seiren_audio_capture_adapter::run(canceled, options, publish)
}
pub(super) fn seiren_audio_playback(
    canceled: &std::sync::atomic::AtomicBool,
    clip: &razer_device::seiren_microphone::PcmPlaybackFile,
    started: &mut dyn FnMut() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    seiren_audio_capture_adapter::playback(canceled, clip, started)
}
#[path = "controller_calibration_identity.rs"]
mod controller_calibration_identity;
#[path = "marci_identity.rs"]
mod marci_identity;
pub(in crate::runtime) fn marci_channels(
    container: &str,
) -> anyhow::Result<marci_identity::Channels> {
    marci_identity::resolve(container)
}
#[path = "controller_input.rs"]
mod controller_input;
#[path = "device_reads.rs"]
mod device_reads;
#[path = "hid/hid.rs"]
mod hid;
#[path = "hid/hid_transport.rs"]
mod hid_transport;
#[path = "mapping_runtime.rs"]
mod mapping_runtime;
pub(super) fn mapping_client() -> anyhow::Result<crate::mapping::owner::Client> {
    mapping_runtime::client()
}
#[path = "mapping_native_input.rs"]
pub mod mapping_native_input;
#[path = "mapping_native_session.rs"]
pub mod mapping_native_session;
#[path = "mixer_driver.rs"]
mod mixer_driver;
#[path = "mixer_endpoint.rs"]
mod mixer_endpoint;
#[path = "mixer_hardware.rs"]
mod mixer_hardware;
#[path = "mixer_lighting.rs"]
mod mixer_lighting;
#[path = "mixer_meter.rs"]
mod mixer_meter;
#[path = "mouse/input_monitoring.rs"]
mod mouse_input_monitoring;
#[path = "audio/seiren_audio_capture.rs"]
mod seiren_audio_capture_adapter;
#[path = "simple_version.rs"]
mod simple_version;
#[path = "audio/surround.rs"]
mod surround_adapter;
pub(super) fn mixer_lighting_sample(node: &HidNode, sources: [u8; 4]) -> anyhow::Result<Value> {
    mixer_lighting::sample(node, sources)
}
#[path = "receiver/hyperpolling_receiver.rs"]
mod hyperpolling_receiver;
#[path = "receiver/receiver.rs"]
mod receiver;
#[path = "receiver/receiver_brightness.rs"]
mod receiver_brightness;
#[path = "receiver/receiver_events.rs"]
mod receiver_events;
#[path = "receiver/receiver_identity.rs"]
mod receiver_identity;
#[path = "receiver/receiver_lighting.rs"]
mod receiver_lighting;
#[path = "receiver/receiver_pairing.rs"]
mod receiver_pairing;
#[path = "receiver/receiver_reconnect.rs"]
mod receiver_reconnect;
#[path = "system_monitor.rs"]
mod system_monitor;
#[path = "hid/usb.rs"]
mod usb;
#[path = "user_app_files.rs"]
mod user_app_files;

pub(super) fn controller_calibration_node_proof(node: &HidNode) -> anyhow::Result<Value> {
    controller_feature_node_proof(node, 1)
}

pub(super) fn controller_feature_node_proof(
    node: &HidNode,
    claim_interface: u8,
) -> anyhow::Result<Value> {
    controller_calibration_identity::proof(node, claim_interface)
}
pub(super) struct NativeRuntime {
    cleanup: NativeCleanup,
    mapping_scope: Option<crate::mapping::Client>,
    app_files: user_app_files::Events,
    receiver_pairing: receiver_pairing::Controller,
    receiver_lighting: receiver_lighting::Controller,
    receiver_reconnect: receiver_reconnect::Controller,
    controller_input: controller_input::State,
    mouse_input_monitoring: mouse_input_monitoring::State,
    mixer_meter: Option<mixer_meter::Meter>,
    mixer_hardware: Option<mixer_hardware::Hardware>,
    seiren_audio: device_reads::SeirenAudio,
    system_monitor: system_monitor::State,
    surround: surround_adapter::State,
}

#[derive(Default)]
struct NativeCleanup {
    joined: crate::runtime::shutdown::OwnerCleanup,
    receiver_lighting: crate::runtime::shutdown::OwnerCleanup,
    mouse_input_monitoring: crate::runtime::shutdown::OwnerCleanup,
    mixer_meter: crate::runtime::shutdown::OwnerCleanup,
    system_monitor: crate::runtime::shutdown::OwnerCleanup,
    controller_input: crate::runtime::shutdown::OwnerCleanup,
    mapping_scope: crate::runtime::shutdown::OwnerCleanup,
}

impl NativeRuntime {
    pub(super) fn new() -> Self {
        Self {
            cleanup: NativeCleanup::default(),
            mapping_scope: None,
            app_files: user_app_files::Events::default(),
            receiver_pairing: receiver_pairing::Controller::default(),
            receiver_lighting: receiver_lighting::Controller::default(),
            receiver_reconnect: receiver_reconnect::Controller::default(),
            controller_input: controller_input::State::new(),
            mouse_input_monitoring: mouse_input_monitoring::State::default(),
            mixer_meter: None,
            mixer_hardware: None,
            seiren_audio: device_reads::SeirenAudio::default(),
            system_monitor: system_monitor::State::default(),
            surround: surround_adapter::State::default(),
        }
    }

    pub(super) fn shutdown_cleanup(&mut self) -> crate::runtime::shutdown::Cleanup {
        use crate::runtime::shutdown::Cleanup;
        let mut cleanup = self.cleanup.joined.run(|| {
            let mut cleanup = Cleanup::default();
            cleanup.record("receiver_pairing", self.receiver_pairing.shutdown());
            cleanup.record("receiver_reconnect", self.receiver_reconnect.shutdown());
            cleanup.record("seiren_audio", self.seiren_audio.shutdown());
            cleanup.record("surround", self.surround.shutdown());
            cleanup
        });
        cleanup.merge(self.cleanup.receiver_lighting.run(|| {
            let mut cleanup = Cleanup::default();
            let result = self.receiver_lighting.shutdown();
            if let Ok(Some(status)) = &result {
                if !status.restoration_complete() || !status.software.cleanup_errors.is_empty() {
                    cleanup.fail(
                        "receiver_lighting",
                        format!(
                            "Receiver lighting restoration or cleanup was not confirmed: {:?}",
                            status.software
                        ),
                    );
                }
            }
            cleanup.record("receiver_lighting", result);
            if self.receiver_lighting.cleanup_pending() {
                cleanup.retain_resources();
            }
            cleanup
        }));
        cleanup.merge(self.cleanup.mouse_input_monitoring.run_result(
            "mouse_input_monitoring",
            || {
                let result = self.mouse_input_monitoring.shutdown();
                (result, self.mouse_input_monitoring.cleanup_pending())
            },
        ));
        self.mixer_hardware = None;
        cleanup.merge(self.cleanup.mixer_meter.run_result("mixer_meter", || {
            let result = match self.mixer_meter.as_mut() {
                Some(meter) => meter.stop(),
                None => Ok(()),
            };
            if result.is_ok() {
                self.mixer_meter = None;
            }
            (result, self.mixer_meter.is_some())
        }));
        cleanup.merge(
            self.cleanup
                .system_monitor
                .run_result("system_monitor", || {
                    let result = self.system_monitor.shutdown();
                    (result, self.system_monitor.cleanup_pending())
                }),
        );
        cleanup.merge(
            self.cleanup
                .controller_input
                .run_result("controller_input", || {
                    let result = self.controller_input.shutdown();
                    (result, self.controller_input.cleanup_pending())
                }),
        );
        cleanup.merge(self.cleanup.mapping_scope.run_result("mapping_scope", || {
            let result = match self.mapping_scope.as_mut() {
                Some(scope) => scope.request(ServiceRequest::Shutdown),
                None => Ok(json!({"stopped":true})),
            };
            if result.is_ok()
                || self
                    .mapping_scope
                    .as_ref()
                    .is_some_and(|scope| scope.is_stopped())
            {
                self.mapping_scope = None;
            }
            (result, self.mapping_scope.is_some())
        }));
        self.app_files.clear();
        cleanup
    }

    fn mapping_request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        if self.mapping_scope.is_none() {
            self.mapping_scope = Some(mapping_client()?);
        }
        self.mapping_scope.as_mut().unwrap().request(request)
    }

    pub(super) fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        match request {
            request @ (ServiceRequest::MousePhilipsMeasurementStart { .. }
            | ServiceRequest::MousePhilipsMeasurementPoll { .. }
            | ServiceRequest::MousePhilipsMeasurementCancel { .. }
            | ServiceRequest::StartMappingKeyCapture
            | ServiceRequest::StartGlobalShortcutCapture
            | ServiceRequest::StartKeyboardCalibrationCapture { .. }
            | ServiceRequest::MappingKeyCaptureEvents
            | ServiceRequest::GlobalShortcutCaptureEvents
            | ServiceRequest::StopMappingKeyCapture
            | ServiceRequest::StartShortcutCapture
            | ServiceRequest::ShortcutCaptureEvents
            | ServiceRequest::StopShortcutCapture
            | ServiceRequest::MappingRotationStart { .. }
            | ServiceRequest::MappingRotationRequest { .. }
            | ServiceRequest::MappingRotationPoll { .. }
            | ServiceRequest::MappingRotationStop { .. }
            | ServiceRequest::MappingDeviceStart { .. }
            | ServiceRequest::MappingDevicePublish { .. }
            | ServiceRequest::MappingDeviceEvents { .. }
            | ServiceRequest::MappingGamepadObservationStart { .. }
            | ServiceRequest::MappingGamepadObservationPoll { .. }
            | ServiceRequest::MappingGamepadObservationStop { .. }
            | ServiceRequest::MappingKeypadObservationStart { .. }
            | ServiceRequest::MappingKeypadObservationPoll { .. }
            | ServiceRequest::MappingKeypadObservationStop { .. }
            | ServiceRequest::MappingAnalogObservationStart { .. }
            | ServiceRequest::MappingAnalogObservationPoll { .. }
            | ServiceRequest::MappingAnalogObservationStop { .. }
            | ServiceRequest::MappingObservationStart { .. }
            | ServiceRequest::MappingInputObservationStart { .. }
            | ServiceRequest::MappingInputObservationPoll { .. }
            | ServiceRequest::MappingInputObservationStop { .. }
            | ServiceRequest::MappingObservationPoll { .. }
            | ServiceRequest::MappingObservationStop { .. }
            | ServiceRequest::MappingDeviceHypershiftActive { .. }
            | ServiceRequest::MappingDeviceStop { .. }
            | ServiceRequest::MappingKeyboardGamingIndicator { .. }
            | ServiceRequest::StartMacroRecording
            | ServiceRequest::PrepareOtfmRecorder
            | ServiceRequest::StartOtfmRecording
            | ServiceRequest::QueryMacroRecordingStarted
            | ServiceRequest::MacroRecordingMode
            | ServiceRequest::MacroRecorderGlobalState
            | ServiceRequest::SetOtfmMappingsSuspended { .. }
            | ServiceRequest::MappingOtfmInputStart { .. }
            | ServiceRequest::MacroRecorderStorageWrite { .. }
            | ServiceRequest::StopMacroRecording
            | ServiceRequest::MacroRecordingEvents
            | ServiceRequest::SuspendMacroMappings
            | ServiceRequest::ResumeMacroMappings
            | ServiceRequest::GlobalMode
            | ServiceRequest::GlobalShortcuts
            | ServiceRequest::RegisterShortcut { .. }
            | ServiceRequest::UnregisterShortcut { .. }
            | ServiceRequest::EnableGlobalShortcuts { .. }
            | ServiceRequest::ShortcutEvents
            | ServiceRequest::GlobalShortcutStart { .. }
            | ServiceRequest::GlobalShortcutPublish { .. }
            | ServiceRequest::GlobalShortcutPoll { .. }
            | ServiceRequest::GlobalShortcutStop { .. }
            | ServiceRequest::MappingExecution { .. }) => self.mapping_request(request),
            ServiceRequest::MouseInputMonitoringStart { owner_guid } => {
                self.start_mouse_input_monitoring(owner_guid)
            }
            ServiceRequest::MouseInputMonitoringPoll { owner_guid } => {
                self.poll_mouse_input_monitoring(&owner_guid)
            }
            ServiceRequest::MouseInputMonitoringStop { owner_guid } => {
                self.stop_mouse_input_monitoring(&owner_guid)
            }
            ServiceRequest::SystemMonitorStart {
                owner_guid,
                container_id,
                product_id,
            } => self.start_system_monitor(owner_guid, container_id, product_id),
            ServiceRequest::SystemMonitorPoll { owner_guid } => {
                serde_json::to_value(self.system_monitor.poll(&owner_guid)?).map_err(Into::into)
            }
            ServiceRequest::SystemMonitorStop { owner_guid } => {
                serde_json::to_value(self.system_monitor.stop(&owner_guid)?).map_err(Into::into)
            }
            ServiceRequest::ControllerInputStart {
                product_id,
                physical_product_id,
                device_container_id,
                owner_guid,
            } => self.start_controller_input(
                product_id,
                physical_product_id,
                &device_container_id,
                &owner_guid,
            ),
            ServiceRequest::ControllerInputPoll { owner_guid } => {
                self.controller_input_events(&owner_guid)
            }
            ServiceRequest::ControllerInputStop { owner_guid } => {
                self.stop_controller_input(&owner_guid)
            }
            ServiceRequest::HostStorageView { .. }
            | ServiceRequest::KeyboardLayoutRead
            | ServiceRequest::AudioVolumeRead { .. }
            | ServiceRequest::AudioVolumeWrite { .. }
            | ServiceRequest::WheelScrollLinesRead
            | ServiceRequest::WheelScrollLinesWrite { .. }
            | ServiceRequest::WindowsServiceStatus { .. }
            | ServiceRequest::WindowsServiceStart { .. }
            | ServiceRequest::WindowsServiceStop { .. }
            | ServiceRequest::HostStorageClose { .. }
            | ServiceRequest::HidNodeMouseBrightness { .. }
            | ServiceRequest::HostStorageCall { .. }
            | ServiceRequest::HostStorageEvents { .. }
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
            | ServiceRequest::HidNodeDpiStagesRead { .. }
            | ServiceRequest::HidNodeDpiStagesWrite { .. }
            | ServiceRequest::HidNodeMouseCurrentDpi { .. }
            | ServiceRequest::HidNodeMouseDynamic { .. }
            | ServiceRequest::HidNodeMouseRotation { .. }
            | ServiceRequest::HidNodeMouseCurrentPolling { .. }
            | ServiceRequest::HidNodeMouseCurrentPower { .. }
            | ServiceRequest::HidNodeMouseScroll { .. }
            | ServiceRequest::HidNodeLeviathan { .. }
            | ServiceRequest::LeviathanEventsStart { .. }
            | ServiceRequest::LeviathanEventsPoll { .. }
            | ServiceRequest::LeviathanEventsStop { .. }
            | ServiceRequest::HidNodeMouseIndicatorLed { .. }
            | ServiceRequest::MouseIndicatorLedStart { .. }
            | ServiceRequest::MouseTrackingStart { .. }
            | ServiceRequest::MouseTrackingPoll { .. }
            | ServiceRequest::MouseTrackingCancel { .. }
            | ServiceRequest::MousePhilipsStart { .. }
            | ServiceRequest::MousePhilipsPoll { .. }
            | ServiceRequest::MousePhilipsCancel { .. }
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
            | ServiceRequest::HidNodeKeyboardBrightnessRead { .. }
            | ServiceRequest::HidNodeKeyboardReceiverBrightness { .. }
            | ServiceRequest::HidNodeKeyboardMetadataRead { .. }
            | ServiceRequest::KeyboardProfileInventoryRead { .. }
            | ServiceRequest::KeyboardOnboardProfilesRead { .. }
            | ServiceRequest::HidNodeKeyboardSerialRead { .. }
            | ServiceRequest::HidNodeKeyboardRelayMetadataRead { .. }
            | ServiceRequest::HidNodeKeyboardRelaySerialRead { .. }
            | ServiceRequest::HidNodeKeyboardBrightnessWrite { .. }
            | ServiceRequest::HidNodeRead { .. }
            | ServiceRequest::HidNodeMouseReceiverRead { .. }
            | ServiceRequest::HidNodeMouseReceiverDpiStages { .. }
            | ServiceRequest::HidNodeMouseReceiverPolling { .. }
            | ServiceRequest::HidNodeMouseReceiverIdle { .. }
            | ServiceRequest::HidNodeWrite { .. }
            | ServiceRequest::HidNodeHanboCooler { .. }
            | ServiceRequest::HidNodeHanboCoolerRead { .. }
            | ServiceRequest::HidNodeHanboStartupRead { .. }
            | ServiceRequest::HidNodeMixerRead { .. }
            | ServiceRequest::HidNodeMixerLocalWrite { .. }
            | ServiceRequest::HidNodeMixerWrite { .. }
            | ServiceRequest::HidNodeMixerEqWrite { .. }
            | ServiceRequest::HidNodeMixerRouteRead { .. }
            | ServiceRequest::HidNodeMixerRouteWrite { .. }
            | ServiceRequest::HidNodeMixerRestartStreams { .. }
            | ServiceRequest::HidNodeReceiverStatus { .. }
            | ServiceRequest::HidNodeReceiverStatusForSource { .. }
            | ServiceRequest::HidNodeReceiverDockMode { .. }
            | ServiceRequest::ControllerTriggers { .. }
            | ServiceRequest::ControllerLighting { .. }
            | ServiceRequest::MarciLighting { .. }
            | ServiceRequest::ControllerSleep { .. }
            | ServiceRequest::ControllerSocd { .. }
            | ServiceRequest::ControllerInitialization { .. }
            | ServiceRequest::ControllerTriggerCalibrationStart { .. }
            | ServiceRequest::ControllerTriggerCalibrationPoll { .. }
            | ServiceRequest::ControllerTriggerCalibrationCancel { .. } => {
                bail!("便携查询必须由设备会话入口分派")
            }
            ServiceRequest::HidDevices => hid::enumerate(),
            ServiceRequest::HelpSystemInfo { .. } => {
                bail!("Help reports must be dispatched through their dedicated worker query")
            }
            ServiceRequest::UsbDevices => usb::enumerate(),
            ServiceRequest::DeviceRead { target, kind } => device_reads::query(&target, kind),
            ServiceRequest::DeviceWrite { target, setting } => {
                device_reads::write(&target, &setting)
            }
            ServiceRequest::DeviceKeyboardBrightnessRead { target } => {
                device_reads::keyboard_brightness(&target, None)
            }
            ServiceRequest::DeviceKeyboardMetadataRead { target } => {
                device_reads::keyboard_metadata(&target, super::KeyboardRead::Metadata)
            }
            ServiceRequest::DeviceKeyboardSerialRead { target } => {
                device_reads::keyboard_metadata(&target, super::KeyboardRead::Serial)
            }
            ServiceRequest::BleKeyboardMetadataRead { target } => {
                device_reads::keyboard_metadata_ble(&target, false)
            }
            ServiceRequest::BleKeyboardSerialRead { target } => {
                device_reads::keyboard_metadata_ble(&target, true)
            }
            ServiceRequest::DeviceKeyboardBrightnessWrite { target, percent } => {
                device_reads::keyboard_brightness(&target, Some(percent))
            }
            ServiceRequest::DeviceDpiStagesRead { target } => {
                device_reads::dpi_stages(&target, None)
            }
            ServiceRequest::DeviceDpiStagesWrite { target, draft } => {
                device_reads::dpi_stages(&target, Some(&draft))
            }
            ServiceRequest::DeviceMouseCurrentDpi { target, change } => {
                device_reads::mouse_current_dpi(&target, change.as_ref())
            }
            ServiceRequest::DeviceMouseBrightness { target, submission } => {
                device_reads::mouse_brightness(&target, &submission)
            }
            ServiceRequest::DeviceMouseDynamic { target, change } => {
                device_reads::mouse_dynamic(&target, change.as_ref())
            }
            ServiceRequest::DeviceMouseRotation { target, change } => {
                device_reads::mouse_rotation(&target, change.as_ref())
            }
            ServiceRequest::DeviceMouseCurrentPolling { target, change } => {
                device_reads::mouse_current_polling(&target, change.as_ref())
            }
            ServiceRequest::DeviceMouseCurrentPower { target, change } => {
                device_reads::mouse_current_power(&target, &change)
            }
            ServiceRequest::DeviceMouseScroll { target, submission } => {
                device_reads::mouse_scroll(&target, &submission)
            }
            ServiceRequest::DeviceLeviathan { target, command } => {
                device_reads::leviathan(&target, command)
            }
            ServiceRequest::DeviceMouseIndicatorLed { target, change } => {
                device_reads::mouse_indicator_led(&target, &change)
            }
            ServiceRequest::DeviceSeirenEq { target, change } => {
                self.seiren_audio.request(&target, &change)
            }
            ServiceRequest::SeirenNativeOpen { target } => self.seiren_audio.open(&target),
            ServiceRequest::SeirenNativeCall {
                target,
                native_operation,
            } => self.seiren_audio.call(&target, &native_operation),
            ServiceRequest::SeirenNativeEvents { target } => self.seiren_audio.events(&target),
            ServiceRequest::SeirenNativeClose { target } => self.seiren_audio.close(&target),
            ServiceRequest::SurroundOpen { identity, settings } => {
                let validate = surround_identity(&identity);
                self.surround.open(&identity, &settings, validate)
            }
            ServiceRequest::SurroundChange { identity, change } => {
                let validate = surround_identity(&identity);
                self.surround.change(&identity, &change, validate)
            }
            ServiceRequest::SurroundObserve { identity } => {
                let validate = surround_identity(&identity);
                self.surround.observe(&identity, validate)
            }
            ServiceRequest::SurroundClose { identity } => self.surround.close(&identity),
            ServiceRequest::BleMouseCurrentPower { target, change } => {
                device_reads::mouse_current_power_ble(&target, &change)
            }
            ServiceRequest::BleMouseCurrentDpi {
                target,
                change,
                context,
                cancellation,
            } => device_reads::mouse_current_dpi_ble(
                &target,
                change.as_ref(),
                context.as_ref(),
                cancellation.as_ref(),
            ),
            ServiceRequest::BleMouseRotation { target, change } => {
                device_reads::mouse_rotation_ble(&target, change.as_ref())
            }
            ServiceRequest::ReceiverWirelessStatus {
                path,
                device_container_id,
                source_product_id,
            } => receiver::query(&path, &device_container_id, source_product_id),
            ServiceRequest::ReceiverDockModeRead {
                path,
                device_container_id,
            } => receiver::dock_mode(&path, &device_container_id),
            ServiceRequest::ReceiverLightingApply {
                operation_id,
                path,
                device_container_id,
                configuration,
            } => serde_json::to_value(self.receiver_lighting.apply(
                operation_id,
                path,
                device_container_id,
                configuration,
            )?)
            .map_err(Into::into),
            ServiceRequest::ReceiverLightingPoll { operation_id } => {
                serde_json::to_value(self.receiver_lighting.poll(&operation_id)?)
                    .map_err(Into::into)
            }
            ServiceRequest::ReceiverLightingChromaAttributeResult {
                operation_id,
                worker_id,
                request_id,
                receipt,
            } => serde_json::to_value(self.receiver_lighting.chroma_attribute_result(
                &operation_id,
                worker_id,
                request_id,
                receipt,
            )?)
            .map_err(Into::into),
            ServiceRequest::ReceiverLightingStop { operation_id } => {
                serde_json::to_value(self.receiver_lighting.stop(&operation_id)?)
                    .map_err(Into::into)
            }
            ServiceRequest::ReceiverBrightnessRead {
                path,
                device_container_id,
            } => receiver_brightness::request(&path, &device_container_id, None),
            ServiceRequest::ReceiverBrightnessWrite {
                path,
                device_container_id,
                percent,
            } => receiver_brightness::request(&path, &device_container_id, Some(percent)),
            ServiceRequest::ReceiverIdentityRead {
                path,
                device_container_id,
            } => receiver_identity::read(&path, &device_container_id),
            ServiceRequest::ReceiverPairingStart {
                operation_id,
                path,
                device_container_id,
                category,
                action,
                source_product_id,
            } => self.receiver_pairing.start(
                operation_id,
                path,
                device_container_id,
                category,
                action,
                source_product_id,
            ),
            ServiceRequest::ReceiverPairingPoll { operation_id } => {
                self.receiver_pairing.poll(&operation_id)
            }
            ServiceRequest::ReceiverPairingCancel { operation_id } => {
                self.receiver_pairing.cancel(&operation_id)
            }
            ServiceRequest::ReceiverReconnectStart {
                operation_id,
                product_id,
                device_container_id,
                category,
                next_transaction_sequence,
                remaining_delay_ms,
                source_product_id,
            } => self.receiver_reconnect.start(
                operation_id,
                product_id,
                device_container_id,
                category,
                source_product_id,
                next_transaction_sequence,
                remaining_delay_ms,
            ),
            ServiceRequest::ReceiverReconnectPoll { operation_id } => {
                self.receiver_reconnect.poll(&operation_id)
            }
            ServiceRequest::ReceiverReconnectCancel { operation_id } => {
                self.receiver_reconnect.cancel(&operation_id)
            }
            ServiceRequest::AudioDevices => {
                Ok(serde_json::to_value(crate::simple_audio::enumerate()?)?)
            }
            ServiceRequest::UserAppFiles { command } => self.user_app_files(command),
            ServiceRequest::SimpleVersion => Ok(serde_json::to_value(simple_version::query()?)?),
            ServiceRequest::AudioMixerHardwareStart { node } => {
                anyhow::ensure!(
                    self.mixer_hardware.is_none(),
                    "Mixer hardware owner already active"
                );
                let (owner, response) = mixer_hardware::Hardware::start(node)?;
                self.mixer_hardware = Some(owner);
                Ok(response)
            }
            ServiceRequest::AudioMixerHardwareRead { node } => self
                .mixer_hardware
                .as_mut()
                .context("Mixer hardware not started")?
                .read(&node),
            ServiceRequest::AudioMixerHardwareStop => {
                self.mixer_hardware = None;
                Ok(json!({"stopped":true}))
            }
            ServiceRequest::AudioMixerHardwareInfo { node } => {
                anyhow::ensure!(
                    razer_device::audio_mixer::accepts(1342, node.vendor_id, node.product_id),
                    "Mixer hardware collection identity invalid"
                );
                let before = mixer_driver::observe_node(&node, 1342)?;
                let validate = || {
                    let current = mixer_driver::observe_node(&node, 1342)?;
                    anyhow::ensure!(
                        current["device_instance_id"] == before["device_instance_id"]
                            && current["device_container_id"] == before["device_container_id"],
                        "Mixer hardware information owner changed"
                    );
                    Ok(())
                };
                let snapshot = razer_hid::with_backend(|backend| {
                    let device = backend.open_validated(&node, std::rc::Rc::new(&validate))?;
                    razer_device::audio_mixer_hardware::read_initial(device.as_ref())
                })?;
                Ok(json!({"node":node,"product_id":1342,"initial":snapshot}))
            }
            ServiceRequest::AudioMixerMeterStart { node, mixer } => {
                anyhow::ensure!(
                    self.mixer_meter.is_none(),
                    "Mixer meter already has an owner"
                );
                let meter = mixer_meter::Meter::start(node, mixer)?;
                let result = meter.identity();
                self.mixer_meter = Some(meter);
                Ok(result)
            }
            ServiceRequest::AudioMixerMeterRead { node } => self
                .mixer_meter
                .as_ref()
                .context("Mixer meter not started")?
                .read(&node),
            ServiceRequest::AudioMixerMeterStop => {
                if let Some(meter) = self.mixer_meter.as_mut() {
                    meter.stop()?;
                }
                self.mixer_meter = None;
                Ok(json!({"capture_stopped":true}))
            }
            ServiceRequest::Shutdown => {
                let response = self.shutdown_cleanup().progress(0).into_response();
                match (response.data, response.error) {
                    (Some(data), None) => Ok(data),
                    (_, Some(error)) => Err(anyhow::anyhow!("{error}")),
                    _ => Err(anyhow::anyhow!("Native cleanup did not return an outcome")),
                }
            }
        }
    }
}

pub(super) fn mixer_driver_route(
    node: razer_device::backend::HidNode,
    product_id: u32,
    route: razer_device::audio_mixer::MixerRoute,
    enabled: Option<bool>,
) -> anyhow::Result<Value> {
    mixer_driver::route(node, product_id, route, enabled)
}

pub(super) fn mixer_control_output(
    node: &razer_device::backend::HidNode,
    report: &[u8],
) -> anyhow::Result<()> {
    hid::mixer_control_output(node, report)
}

pub(super) fn mixer_endpoint_write(
    node: &HidNode,
    jack: u8,
    flow: u8,
    value: &razer_device::audio_mixer::MixerValue,
) -> anyhow::Result<Value> {
    mixer_endpoint::write(node, jack, flow, value)
}

pub(super) fn mixer_restart_streams(
    node: razer_device::backend::HidNode,
    product_id: u32,
) -> anyhow::Result<Value> {
    mixer_driver::restart(node, product_id)
}

/// Current Carol owners address the native route by the device ContainerId;
/// each native task revalidates that the same physical product is present.
fn surround_identity(
    identity: &razer_device::audio_surround::Identity,
) -> impl Fn() -> anyhow::Result<()> + use<> {
    let container = identity.container_id().to_owned();
    let product = u32::from(identity.product());
    move || {
        native_target_with_scope(&container, Some(product), NativeTargetScope::Surround).map(|_| ())
    }
}
#[derive(Clone, Copy)]
enum NativeTargetScope {
    Direct,
    Surround,
}
/// Native container getters require a real, uniquely observed physical device.
/// This uses the same current USB enumeration and product identity projection
/// as discovery; arbitrary CLI strings and local profiles are not observations.
fn native_target(container: &str, expected_product: Option<u32>) -> anyhow::Result<(u32, Value)> {
    native_target_with_scope(container, expected_product, NativeTargetScope::Direct)
}
fn native_target_with_scope(
    container: &str,
    expected_product: Option<u32>,
    scope: NativeTargetScope,
) -> anyhow::Result<(u32, Value)> {
    use razer_device::device_identity;
    use razer_device::device_identity::IdentityLookup;
    anyhow::ensure!(
        receiver::valid_container(container),
        "查询须使用非零真实 ContainerId"
    );
    let observation = usb::enumerate()?;
    anyhow::ensure!(
        observation["complete"] == Value::Bool(true)
            && observation["failures"]
                .as_array()
                .is_some_and(Vec::is_empty),
        "物理 USB 枚举不完整，暂不查询 DLL"
    );
    let candidates: Vec<_> = observation["devices"]
        .as_array()
        .context("USB 设备数组缺失")?
        .iter()
        .filter(|device| {
            device["vendor_id"].as_u64() == Some(0x1532)
                && device["device_container_id"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(container))
        })
        .collect();
    anyhow::ensure!(
        candidates.len() == 1,
        "ContainerId 对应的真实 Razer USB 设备不唯一或已离线"
    );
    let device = candidates[0];
    for key in ["path", "device_instance_id"] {
        anyhow::ensure!(
            device[key].as_str().is_some_and(|value| !value.is_empty()),
            "物理 USB 身份缺少 {key}"
        );
    }
    let physical_pid = u32::try_from(device["product_id"].as_u64().context("物理 USB PID 缺失")?)?;
    let IdentityLookup::Unique(identity) = device_identity::lookup(physical_pid) else {
        bail!("物理 PID {physical_pid} 没有唯一的当前源码产品归属");
    };
    anyhow::ensure!(
        !identity.is_ble
            && (!identity.is_dongle
                || matches!(scope, NativeTargetScope::Surround)
                    && matches!(identity.product_id, 1392 | 1442)
                    && physical_pid == identity.product_id),
        "接收器或 BLE PID 不能冒充直接原生设备"
    );
    anyhow::ensure!(
        expected_product.is_none_or(|product| product == identity.product_id),
        "DLL 查询产品与真实 USB 身份不符"
    );
    Ok((identity.product_id, device.clone()))
}

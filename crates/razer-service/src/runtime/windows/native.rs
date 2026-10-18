//! Worker-only vendor bindings. Each signature is paired with the original
//! `apiObj`, callback declaration and invocation in the Electron wrapper.
use super::{CALLBACK_TIMEOUT, ServiceRequest};
use crate::dll::EngineLibrary;
use anyhow::{Context as _, bail};
use razer_device::backend::HidNode;
use razer_platform::native_paths::EnginePaths;
use serde_json::{Value, json};
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
use std::{
    ffi::{CStr, c_char},
    mem::ManuallyDrop,
    sync::{
        Mutex,
        mpsc::{self, Sender},
    },
};

#[path = "controller_calibration_identity.rs"]
mod controller_calibration_identity;
#[path = "controller_input.rs"]
mod controller_input;
#[path = "device_reads.rs"]
mod device_reads;
#[path = "global_shortcut_capture.rs"]
mod global_shortcut_capture;
#[path = "global_shortcuts.rs"]
mod global_shortcuts;
#[path = "hid/hid.rs"]
mod hid;
#[path = "hid/hid_transport.rs"]
mod hid_transport;
#[path = "macro_recording.rs"]
mod macro_recorder;
#[path = "mapping_device.rs"]
mod mapping_device;
#[path = "mapping_execution.rs"]
mod mapping_execution;
#[path = "mapping_key_capture.rs"]
mod mapping_key_capture;
#[path = "mapping_native_input.rs"]
pub mod mapping_native_input;
#[path = "mapping_native_output.rs"]
pub mod mapping_native_output;
#[path = "mapping_native_rotation.rs"]
pub mod mapping_native_rotation;
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
#[path = "mouse/philips_measurement.rs"]
mod philips_measurement;
#[path = "audio/seiren_audio_capture.rs"]
mod seiren_audio_capture_adapter;
#[path = "shortcut_capture.rs"]
mod shortcut_capture;
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

// No userdata argument exists in these APIs. The worker permits one outstanding
// call and terminates after a timeout, so a late callback cannot satisfy a later
// request. Function pointers are static for the entire worker lifetime.
static PENDING: Mutex<Option<Sender<CallbackReply>>> = Mutex::new(None);

type Callback0 = unsafe extern "C" fn();
type Callback2 = unsafe extern "C" fn(bool, *const c_char);
type Callback3 = unsafe extern "C" fn(bool, *const c_char, *const c_char);
type Initialize = unsafe extern "C" fn(Callback0);
type Query = unsafe extern "C" fn(Callback3);

pub(super) fn controller_calibration_node_proof(node: &HidNode) -> anyhow::Result<Value> {
    controller_feature_node_proof(node, 1)
}

pub(super) fn controller_feature_node_proof(
    node: &HidNode,
    claim_interface: u8,
) -> anyhow::Result<Value> {
    controller_calibration_identity::proof(node, claim_interface)
}
type StorageSetItem = unsafe extern "C" fn(*const c_char, *const c_char, Callback2);

struct CallbackReply {
    success: bool,
    reason: String,
    value: Option<String>,
}

unsafe fn copy_string(value: *const c_char) -> String {
    if value.is_null() {
        String::new()
    } else {
        // SAFETY: the original API callback supplies a borrowed NUL-terminated
        // string. Copy it during this invocation; the DLL retains ownership.
        unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned()
    }
}

fn complete(reply: CallbackReply) {
    if let Ok(mut slot) = PENDING.lock() {
        if let Some(sender) = slot.take() {
            let _ = sender.send(reply);
        }
    }
}

unsafe extern "C" fn callback0() {
    complete(CallbackReply {
        success: true,
        reason: String::new(),
        value: None,
    });
}

unsafe extern "C" fn callback2(success: bool, reason: *const c_char) {
    complete(CallbackReply {
        success,
        reason: unsafe { copy_string(reason) },
        value: None,
    });
}

unsafe extern "C" fn callback3(success: bool, reason: *const c_char, value: *const c_char) {
    complete(CallbackReply {
        success,
        reason: unsafe { copy_string(reason) },
        value: (!value.is_null()).then(|| unsafe { copy_string(value) }),
    });
}

pub(super) struct NativeRuntime {
    paths: EnginePaths,
    // Never unload a library while its background callbacks may still be active.
    // Shutdown is explicit; process exit reclaims the module after this returns.
    mapping: Option<ManuallyDrop<EngineLibrary>>,
    simple: Option<ManuallyDrop<EngineLibrary>>,
    mapping_initialized: bool,
    simple_initialized: bool,
    app_file_owners: Vec<String>,
    macro_recorder: macro_recorder::Recorder,
    global_shortcuts: global_shortcuts::State,
    receiver_pairing: receiver_pairing::Controller,
    receiver_lighting: receiver_lighting::Controller,
    receiver_reconnect: receiver_reconnect::Controller,
    mapping_device: mapping_device::State,
    mapping_execution: mapping_execution::State,
    controller_input: controller_input::State,
    philips_measurement: philips_measurement::State,
    mouse_input_monitoring: mouse_input_monitoring::State,
    shortcut_capture: shortcut_capture::State,
    mapping_key_capture: mapping_key_capture::State,
    mixer_meter: Option<mixer_meter::Meter>,
    mixer_hardware: Option<mixer_hardware::Hardware>,
    seiren_audio: device_reads::SeirenAudio,
    system_monitor: system_monitor::State,
    surround: surround_adapter::State,
    poisoned: bool,
}

impl NativeRuntime {
    pub(super) fn new() -> Self {
        Self {
            paths: EnginePaths::discover(),
            mapping: None,
            simple: None,
            mapping_initialized: false,
            simple_initialized: false,
            app_file_owners: Vec::new(),
            macro_recorder: macro_recorder::Recorder::new(),
            global_shortcuts: global_shortcuts::State::new(),
            receiver_pairing: receiver_pairing::Controller::default(),
            receiver_lighting: receiver_lighting::Controller::default(),
            receiver_reconnect: receiver_reconnect::Controller::default(),
            mapping_device: mapping_device::State::new(),
            mapping_execution: mapping_execution::State::default(),
            controller_input: controller_input::State::new(),
            philips_measurement: philips_measurement::State::new(),
            mouse_input_monitoring: mouse_input_monitoring::State::default(),
            shortcut_capture: shortcut_capture::State::new(),
            mapping_key_capture: mapping_key_capture::State::new(),
            mixer_meter: None,
            mixer_hardware: None,
            seiren_audio: device_reads::SeirenAudio::default(),
            system_monitor: system_monitor::State::default(),
            surround: surround_adapter::State::default(),
            poisoned: false,
        }
    }

    pub(super) fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    fn call(&mut self, invoke: impl FnOnce()) -> anyhow::Result<CallbackReply> {
        let reply = self.call_source(invoke)?;
        if !reply.success {
            bail!("原生服务拒绝请求：{}", reply.reason);
        }
        Ok(reply)
    }

    // Source callback failures are data; transport failure/timeout still poison
    // this owner so a late callback cannot complete a later request.
    fn call_source(&mut self, invoke: impl FnOnce()) -> anyhow::Result<CallbackReply> {
        if self.poisoned {
            bail!("设备服务回调状态已失效，请重新连接");
        }
        let (sender, receiver) = mpsc::channel();
        match PENDING.lock() {
            Ok(mut pending) if pending.is_none() => *pending = Some(sender),
            _ => {
                self.poisoned = true;
                bail!("设备服务回调状态不可用，请重新连接");
            }
        }
        invoke();
        let reply = match receiver.recv_timeout(CALLBACK_TIMEOUT) {
            Ok(reply) => reply,
            Err(error) => {
                self.poisoned = true;
                if let Ok(mut pending) = PENDING.lock() {
                    pending.take();
                }
                return Err(error).context("原生服务未在时限内返回回调");
            }
        };
        Ok(reply)
    }

    fn mapping(&mut self) -> anyhow::Result<()> {
        if self.mapping_initialized {
            return Ok(());
        }
        if self.mapping.is_none() {
            let path = self.paths.resolve_host_service("mapping_engine")?;
            self.mapping = Some(ManuallyDrop::new(EngineLibrary::load(&path)?));
        }
        // mapping_engine/win/index.js: ["void",["pointer"]], Callback("void",[]).
        let initialize: Initialize = unsafe { self.mapping_symbol("mappingEngineInitialize")? };
        if let Err(error) = self.call(|| unsafe { initialize(callback0) }) {
            self.poisoned = true;
            return Err(error);
        }
        self.mapping_initialized = true;
        Ok(())
    }

    fn simple(&mut self) -> anyhow::Result<()> {
        self.complete_system_monitor_simple_init(std::time::Duration::ZERO)?;
        if self.simple_initialized {
            return Ok(());
        }
        if self.simple.is_none() {
            let path = self.paths.resolve_host_service("simple_service")?;
            self.simple = Some(ManuallyDrop::new(EngineLibrary::load(&path)?));
        }
        // simple_service/win/index.js: identical void(callback0) lifecycle.
        let initialize: Initialize = unsafe { self.simple_symbol("simpleServiceInitialize")? };
        if let Err(error) = self.call(|| unsafe { initialize(callback0) }) {
            self.poisoned = true;
            return Err(error);
        }
        self.simple_initialized = true;
        Ok(())
    }

    unsafe fn mapping_symbol<T: Copy>(&self, name: &str) -> anyhow::Result<T> {
        let library = self.mapping.as_ref().context("映射引擎未加载")?;
        unsafe { library.func(name) }
            .with_context(|| format!("{} 缺少当前封装所需导出 {name}", library.path().display()))
    }

    unsafe fn simple_symbol<T: Copy>(&self, name: &str) -> anyhow::Result<T> {
        let library = self.simple.as_ref().context("音频服务未加载")?;
        unsafe { library.func(name) }
            .with_context(|| format!("{} 缺少当前封装所需导出 {name}", library.path().display()))
    }

    pub(super) fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        if self.poisoned
            && !matches!(
                request,
                ServiceRequest::Shutdown
                    | ServiceRequest::SystemMonitorPoll { .. }
                    | ServiceRequest::SystemMonitorStop { .. }
            )
        {
            bail!("设备服务回调状态已失效，请重新连接");
        }
        match request {
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
            ServiceRequest::MousePhilipsMeasurementStart {
                operation_id,
                target,
                kind,
            } => self.start_philips_measurement(operation_id, target, kind),
            ServiceRequest::MousePhilipsMeasurementPoll { operation_id } => {
                self.poll_philips_measurement(&operation_id)
            }
            ServiceRequest::MousePhilipsMeasurementCancel { operation_id } => {
                self.cancel_philips_measurement(&operation_id)
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
            ServiceRequest::StartMappingKeyCapture => self.start_mapping_key_capture(),
            ServiceRequest::StartGlobalShortcutCapture => self.start_global_shortcut_capture(),
            ServiceRequest::StartKeyboardCalibrationCapture {
                product_id,
                container_id,
            } => self.start_keyboard_calibration_capture(product_id, &container_id),
            ServiceRequest::MappingKeyCaptureEvents => self.mapping_key_capture_events(),
            ServiceRequest::GlobalShortcutCaptureEvents => self.global_shortcut_capture_events(),
            ServiceRequest::StopMappingKeyCapture => self.stop_mapping_key_capture(),
            ServiceRequest::StartShortcutCapture => self.start_shortcut_capture(),
            ServiceRequest::ShortcutCaptureEvents => self.shortcut_capture_events(),
            ServiceRequest::StopShortcutCapture => self.stop_shortcut_capture(),
            ServiceRequest::MappingDeviceStart {
                product_id,
                device_container_id,
                owner_guid,
            } => self.start_mapping_device(product_id, &device_container_id, &owner_guid),
            ServiceRequest::MappingDevicePublish {
                owner_guid,
                document,
            } => self.publish_mapping_device(&owner_guid, document),
            ServiceRequest::MappingDeviceEvents { owner_guid } => {
                self.mapping_device_events(&owner_guid)
            }
            ServiceRequest::MappingDeviceMacroRecordingMode { owner_guid } => {
                self.mapping_device_macro_recording_mode(&owner_guid)
            }
            ServiceRequest::MappingDeviceHypershiftActive { owner_guid } => {
                self.mapping_device_hypershift_active(&owner_guid)
            }
            ServiceRequest::MappingDeviceStop { owner_guid } => {
                self.stop_mapping_device(&owner_guid)
            }
            ServiceRequest::MappingKeyboardGamingIndicator {
                owner_guid,
                enabled,
            } => self.mapping_keyboard_indicator(&owner_guid, enabled),
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
            | ServiceRequest::ControllerTriggerCalibrationStart { .. }
            | ServiceRequest::ControllerTriggerCalibrationPoll { .. }
            | ServiceRequest::ControllerTriggerCalibrationCancel { .. } => {
                bail!("便携查询必须由设备会话入口分派")
            }
            ServiceRequest::HidDevices => hid::enumerate(),
            ServiceRequest::UsbDevices => usb::enumerate(),
            ServiceRequest::NativeLibraryVersion {
                library,
                product_id,
            } => crate::native_query::version(&library, product_id),
            ServiceRequest::NativeLibraryGetter {
                library,
                export,
                device_id,
                product_id,
            } => {
                let (observed_product, before) = native_target(&device_id, product_id)?;
                crate::native_read::getter(
                    &library,
                    &export,
                    &device_id,
                    Some(observed_product),
                    || {
                        let (_, current) = native_target(&device_id, Some(observed_product))?;
                        for key in ["path", "device_instance_id", "device_container_id"] {
                            anyhow::ensure!(
                                before[key]
                                    .as_str()
                                    .zip(current[key].as_str())
                                    .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b)),
                                "DLL 查询期间设备身份发生变化"
                            );
                        }
                        anyhow::ensure!(
                            before["product_id"] == current["product_id"],
                            "DLL 查询期间物理 PID 发生变化"
                        );
                        Ok(())
                    },
                )
            }
            ServiceRequest::DeviceRead { target, kind } => device_reads::query(&target, kind),
            ServiceRequest::DeviceWrite { target, setting } => {
                device_reads::write(&target, &setting)
            }
            ServiceRequest::DeviceKeyboardBrightnessRead { target } => {
                device_reads::keyboard_brightness(&target, None)
            }
            ServiceRequest::DeviceKeyboardMetadataRead { target } => {
                device_reads::keyboard_metadata(&target, false)
            }
            ServiceRequest::DeviceKeyboardSerialRead { target } => {
                device_reads::keyboard_metadata(&target, true)
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
            ServiceRequest::DeviceMouseCurrentPolling { target, change } => {
                device_reads::mouse_current_polling(&target, &change)
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
            ServiceRequest::SurroundClose { identity } => self.surround.close(&identity),
            ServiceRequest::BleMouseCurrentPower { target, change } => {
                device_reads::mouse_current_power_ble(&target, &change)
            }
            ServiceRequest::NativeLibrarySnapshot {
                library,
                device_container_id,
                product_id,
            } => {
                let (_, before) = native_target(&device_container_id, Some(product_id))?;
                crate::native_read::snapshot(&library, &device_container_id, product_id, || {
                    let (_, current) = native_target(&device_container_id, Some(product_id))?;
                    for key in ["path", "device_instance_id", "device_container_id"] {
                        anyhow::ensure!(
                            before[key]
                                .as_str()
                                .zip(current[key].as_str())
                                .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b)),
                            "DLL 查询期间设备身份发生变化"
                        );
                    }
                    anyhow::ensure!(
                        before["product_id"] == current["product_id"],
                        "DLL 查询期间物理 PID 发生变化"
                    );
                    Ok(())
                })
            }
            ServiceRequest::StartMacroRecording => self.start_macro_recording(),
            ServiceRequest::PrepareOtfmRecorder => self.prepare_otfm_recorder(),
            ServiceRequest::StartOtfmRecording => self.start_otfm_recording(),
            ServiceRequest::QueryMacroRecordingStarted => self.query_macro_recording_started(),
            ServiceRequest::MacroRecorderGlobalState => self.macro_recorder_global_state(),
            ServiceRequest::SetOtfmMappingsSuspended { suspended } => {
                self.set_otfm_mappings_suspended(suspended)
            }
            ServiceRequest::MappingOtfmInputStart { owner_guid } => {
                self.start_otfm_input(&owner_guid)
            }
            ServiceRequest::MacroRecorderStorageWrite { key, value } => {
                self.macro_recorder_storage_write(&key, &value)
            }
            ServiceRequest::StopMacroRecording => self.stop_macro_recording(),
            ServiceRequest::MacroRecordingEvents => Ok(self.macro_recorder.events()),
            ServiceRequest::SuspendMacroMappings => self.suspend_macro_mappings(),
            ServiceRequest::ResumeMacroMappings => self.resume_macro_mappings(),
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
            ServiceRequest::SimpleVersion => {
                self.simple()?;
                let symbol = "simpleGetVersionInfo";
                // Source declares void(callback(bool,string,string)).
                let query: Query = unsafe { self.simple_symbol(symbol)? };
                let value = self
                    .call(|| unsafe { query(callback3) })?
                    .value
                    .context("原生服务未返回结果")?;
                anyhow::ensure!(!value.trim().is_empty(), "{symbol} 返回空结果");
                Ok(serde_json::from_str(&value).unwrap_or(Value::String(value)))
            }
            ServiceRequest::GlobalMode => {
                self.mapping()?;
                let query: Query = unsafe { self.mapping_symbol("getGlobalMode")? };
                let value = self
                    .call(|| unsafe { query(callback3) })?
                    .value
                    .context("Mapping engine did not return global mode")?;
                anyhow::ensure!(
                    !value.trim().is_empty(),
                    "getGlobalMode returned empty data"
                );
                Ok(serde_json::from_str(&value).unwrap_or(Value::String(value)))
            }
            ServiceRequest::GlobalShortcuts
            | ServiceRequest::RegisterShortcut { .. }
            | ServiceRequest::UnregisterShortcut { .. }
            | ServiceRequest::EnableGlobalShortcuts { .. }
            | ServiceRequest::ShortcutEvents => {
                bail!("Global shortcuts must use the direct Rust platform owner")
            }
            ServiceRequest::GlobalShortcutStart { owner_guid } => {
                self.start_global_shortcut_owner(&owner_guid)
            }
            ServiceRequest::GlobalShortcutPublish {
                owner_guid,
                revision,
                document,
            } => self.publish_global_shortcuts(&owner_guid, revision, &document),
            ServiceRequest::GlobalShortcutPoll { owner_guid } => {
                self.global_shortcut_events(&owner_guid)
            }
            ServiceRequest::GlobalShortcutStop { owner_guid } => {
                self.stop_global_shortcut_owner(&owner_guid)
            }
            ServiceRequest::MappingExecution {
                owner_guid,
                revision,
                target,
            } => self.set_mapping_execution(&owner_guid, revision, target),
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
                use razer_device::backend::HidBackend;
                anyhow::ensure!(
                    razer_device::audio_mixer::accepts(1342, node.vendor_id, node.product_id),
                    "Mixer hardware collection identity invalid"
                );
                let snapshot = razer_hid::with_backend(|backend| {
                    let device = backend.open(&node)?;
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
                let mut errors = Vec::new();
                match self.receiver_lighting.shutdown() {
                    Ok(Some(status)) => {
                        if !status.restoration_complete()
                            || !status.software.cleanup_errors.is_empty()
                        {
                            errors.push(format!(
                                "Receiver lighting restoration or cleanup was not confirmed: {:?}",
                                status.software
                            ));
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        errors.push(format!("Receiver lighting cleanup failed: {error:#}"));
                    }
                }
                if let Some(owner) = self.mouse_input_monitoring.owner() {
                    if let Err(error) = self.stop_mouse_input_monitoring(&owner) {
                        errors.push(format!("Mouse input monitor cleanup failed: {error:#}"));
                    }
                }
                self.mixer_hardware = None;
                if let Some(meter) = self.mixer_meter.as_mut() {
                    match meter.stop() {
                        Ok(()) => self.mixer_meter = None,
                        Err(error) => errors.push(format!("Mixer meter cleanup failed: {error:#}")),
                    }
                }
                let system_monitor_cleanup = match self.system_monitor.shutdown() {
                    Ok(observation) => observation,
                    Err(error) => {
                        errors.push(format!("System monitor cleanup failed: {error:#}"));
                        None
                    }
                };
                let philips_cleanup = match self.shutdown_philips_measurement() {
                    Ok(progress) => progress,
                    Err(error) => {
                        errors.push(format!("Philips calibration cleanup failed: {error:#}"));
                        None
                    }
                };
                if let Some(owner) = self.controller_input.owner() {
                    if let Err(error) = self.stop_controller_input(&owner) {
                        errors.push(format!("Controller input cleanup failed: {error:#}"));
                    }
                }
                self.shutdown_macro_recording(&mut errors);
                if let Some(owner) = self.global_shortcuts.owner() {
                    if let Err(error) = self.stop_global_shortcut_owner(&owner) {
                        errors.push(format!("Global shortcut cleanup failed: {error:#}"));
                    }
                }
                if let Err(error) = self.stop_mapping_key_capture() {
                    errors.push(format!("Mapping key capture cleanup failed: {error:#}"));
                }
                if let Err(error) = self.stop_shortcut_capture() {
                    errors.push(format!("Shortcut capture cleanup failed: {error:#}"));
                }
                if let Some(owner) = self.mapping_device.owner_guid() {
                    if let Err(error) = self.stop_mapping_device(&owner) {
                        errors.push(format!("Mapping device cleanup failed: {error:#}"));
                    }
                }
                if self.mapping_initialized && !self.poisoned {
                    let result = (|| {
                        let shutdown: Initialize =
                            unsafe { self.mapping_symbol("mappingEngineShutdown")? };
                        self.call(|| unsafe { shutdown(callback0) })
                    })();
                    match result {
                        Ok(_) => self.mapping_initialized = false,
                        Err(error) => errors.push(format!("映射引擎关闭失败：{error:#}")),
                    }
                }
                // A missing export or rejected mapping shutdown must not skip
                // the independent audio service. A timed-out callback must not
                // be allowed to satisfy any later native call.
                if let Err(error) = self.complete_system_monitor_simple_init(CALLBACK_TIMEOUT) {
                    errors.push(format!(
                        "System monitor launcher cleanup remains pending: {error:#}"
                    ));
                }
                let monitor_launch_stopped = match self
                    .complete_system_monitor_launch(CALLBACK_TIMEOUT)
                {
                    Ok(()) => true,
                    Err(error) => {
                        errors.push(format!("System monitor launcher cleanup failed: {error:#}"));
                        !self.system_monitor_launch_pending()
                    }
                };
                if self.simple_initialized && !self.poisoned && monitor_launch_stopped {
                    let result = (|| {
                        let shutdown: Initialize =
                            unsafe { self.simple_symbol("simpleServiceShutdown")? };
                        self.call(|| unsafe { shutdown(callback0) })
                    })();
                    match result {
                        Ok(_) => {
                            self.simple_initialized = false;
                            self.app_file_owners.clear();
                        }
                        Err(error) => errors.push(format!("音频服务关闭失败：{error:#}")),
                    }
                }
                if !errors.is_empty() {
                    bail!("{}", errors.join("；"));
                }
                Ok(
                    json!({"stopped": true, "mouse_philips_measurement_cleanup": philips_cleanup, "system_monitor_cleanup": system_monitor_cleanup}),
                )
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
    move || native_target(&container, Some(product)).map(|_| ())
}
/// Native container getters require a real, uniquely observed physical device.
/// This uses the same current USB enumeration and product identity projection
/// as discovery; arbitrary CLI strings and local profiles are not observations.
fn native_target(container: &str, expected_product: Option<u32>) -> anyhow::Result<(u32, Value)> {
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
        !identity.is_dongle && !identity.is_ble,
        "接收器或 BLE PID 不能冒充直接原生设备"
    );
    anyhow::ensure!(
        expected_product.is_none_or(|product| product == identity.product_id),
        "DLL 查询产品与真实 USB 身份不符"
    );
    Ok((identity.product_id, device.clone()))
}

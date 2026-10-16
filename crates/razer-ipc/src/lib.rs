//! Shared worker wire contract and owned child-process client. No DLL or HID implementation.
use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

#[cfg(windows)]
#[path = "runtime_job.rs"]
mod job;

pub const FRAME_PREFIX: &str = "RAZER_UI_SERVICE ";
pub const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
/// Serializes owners of the process-wide vendor input redirection lifecycle.
/// A previous owner's stop/restore must finish before the next owner starts.
pub static KEY_CAPTURE_LIFECYCLE: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod camera_tests;
#[cfg(test)]
mod client_tests;
#[cfg(test)]
mod controller_trigger_tests;
#[cfg(test)]
mod mouse_scroll_tests;
#[cfg(test)]
mod thx_tests;

/// `operation` is the internal wire tag, so no variant payload may declare a
/// field named `operation`; payloads carry their own operation instead.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum ServiceRequest {
    SeirenAudioCaptureStart {
        session_id: u64,
        product_id: u32,
        #[serde(default)]
        options: razer_device::seiren_microphone::CaptureOptions,
    },
    SeirenAudioCapturePoll {
        session_id: u64,
    },
    SeirenAudioCaptureStop {
        session_id: u64,
    },
    SeirenAudioPlaybackStart {
        session_id: u64,
        product_id: u32,
        clip: razer_device::seiren_microphone::PcmPlaybackFile,
    },
    SeirenAudioPlaybackPoll {
        session_id: u64,
    },
    SeirenAudioPlaybackStop {
        session_id: u64,
    },
    HidNodeSeirenEq {
        node: razer_device::backend::HidNode,
        product_id: u32,
        change: razer_device::seiren_eq::Change,
    },
    DeviceSeirenEq {
        target: razer_device::device_reads::DeviceReadTarget,
        change: razer_device::seiren_eq::Change,
    },
    SeirenNativeOpen {
        target: razer_device::device_reads::DeviceReadTarget,
    },
    SeirenNativeCall {
        target: razer_device::device_reads::DeviceReadTarget,
        native_operation: razer_device::seiren_eq::NativeOperation,
    },
    SeirenNativeEvents {
        target: razer_device::device_reads::DeviceReadTarget,
    },
    SeirenNativeClose {
        target: razer_device::device_reads::DeviceReadTarget,
    },
    HidNodeMouseCurrentPower {
        node: razer_device::backend::HidNode,
        product_id: u32,
        change: razer_device::mouse_current_power::Change,
    },
    DeviceMouseCurrentPower {
        target: razer_device::device_reads::DeviceReadTarget,
        change: razer_device::mouse_current_power::Change,
    },
    HidNodeMouseScroll {
        node: razer_device::backend::HidNode,
        product_id: u32,
        #[serde(default)]
        peer_product_id: Option<u32>,
        submission: razer_device::mouse_scroll::Submission,
    },
    DeviceMouseScroll {
        target: razer_device::device_reads::DeviceReadTarget,
        submission: razer_device::mouse_scroll::Submission,
    },
    HidNodeLeviathan {
        node: razer_device::backend::HidNode,
        command: razer_device::leviathan::Operation,
    },
    DeviceLeviathan {
        target: razer_device::device_reads::DeviceReadTarget,
        command: razer_device::leviathan::Operation,
    },
    LeviathanEventsStart {
        session_id: String,
        target: razer_device::leviathan::EventTarget,
    },
    LeviathanEventsPoll {
        session_id: String,
    },
    LeviathanEventsStop {
        session_id: String,
    },
    BleMouseCurrentPower {
        target: razer_device::mouse_runtime_artwork::WindowsBleTarget,
        change: razer_device::mouse_current_power::Change,
    },
    HidNodeMouseIndicatorLed {
        node: razer_device::backend::HidNode,
        product_id: u32,
        change: razer_device::mouse_indicator_led::Change,
    },
    DeviceMouseIndicatorLed {
        target: razer_device::device_reads::DeviceReadTarget,
        change: razer_device::mouse_indicator_led::Change,
    },
    MouseIndicatorLedStart {
        operation_id: String,
        target: razer_device::mouse_indicator_led::Target,
    },
    MouseIndicatorLedPoll {
        operation_id: String,
    },
    MouseIndicatorLedStop {
        operation_id: String,
    },
    /// Original host process-local Map stores; never device write-back or local
    /// draft persistence. The owning host registers real view metadata first.
    HostStorageView {
        view: razer_storage::host::HostStorageView,
    },
    HostStorageClose {
        view_id: u64,
    },
    HostStorageCall {
        call: razer_storage::host::HostStorageCall,
    },
    HostStorageEvents {
        view_id: u64,
    },
    /// Local installed Apps Service file operations. Negative source callbacks
    /// remain result/reason data; transport failure is a separate error.
    UserAppFiles {
        command: razer_device::user_app_files::UserAppFilesOperation,
    },
    SimpleVersion,
    AudioDevices,
    /// Core Audio endpoint ID from real enumeration, never USB or product ID.
    AudioVolumeRead {
        device_id: String,
    },
    /// Original volume then mute ordering; response retains partial failure
    /// and a separate actual observation rather than fabricating atomic save.
    AudioVolumeWrite {
        device_id: String,
        mute: bool,
        volume: u8,
    },
    /// Source-recovered RzAudioUtil enumeration through the OS adapter;
    /// distinct from simple_service's AudioDevices response schema.
    AudioEndpoints {
        flow: razer_device::audio_util::AudioFlow,
    },
    /// Worker-owned RzAudioUtil local OS listener. No DLL callback is loaded.
    AudioNotificationsEnable {
        enable: bool,
    },
    /// Actual queued source events, including repeated-enable multiplicity.
    AudioNotificationsDrain,
    /// Source AudioRouter enabled value is a signed JSON integer; no DLL call.
    AudioRoutingEnable {
        enable: i32,
    },
    /// Empty routed name removes the old stream; identity is the primary name.
    AudioRouteDevice {
        primary_device: String,
        routed_device: String,
        #[serde(default)]
        primary_device_id: u32,
    },
    AudioRouterEvents,
    ForegroundMonitorStart {
        view_url: String,
    },
    KeyboardLayoutRead,
    ForegroundMonitorStop {
        view_url: String,
    },
    ForegroundMonitorEvents {
        view_url: String,
    },
    HidDevices,
    /// Local THX RPC + retained Output/Interrupt HID. No DLL/export dispatch.
    ThxStart {
        operation_id: String,
        scope: razer_device::thx_submission::ThxSubmissionScope,
        plan: razer_device::thx_submission::ThxSubmissionPlan,
    },
    /// Retain a render owner and its exact HID collection across operations.
    ThxOwnerStart {
        owner_id: String,
        operation_id: String,
        scope: razer_device::thx_submission::ThxSubmissionScope,
        plan: razer_device::thx_submission::ThxSubmissionPlan,
    },
    ThxOwnerSubmit {
        owner_id: String,
        operation_id: String,
        scope: razer_device::thx_submission::ThxSubmissionScope,
        plan: razer_device::thx_submission::ThxSubmissionPlan,
    },
    /// Stop operations and release Rust resources without device setters.
    /// Source render shutdown is a separate submitted plan.
    ThxOwnerRelease {
        owner_id: String,
    },
    ThxPoll {
        operation_id: String,
    },
    ThxCancel {
        operation_id: String,
    },
    CameraDiscover {
        scope: razer_device::camera_submission::CameraScope,
    },
    CameraStart {
        operation_id: String,
        scope: razer_device::camera_submission::CameraScope,
        plan: razer_device::camera_submission::CameraSubmissionPlan,
    },
    /// Current MW task-name Web Lock, scoped to the physical container.
    CameraTaskStart {
        operation_id: String,
        scope: razer_device::camera_submission::CameraScope,
        task_name: String,
        plan: razer_device::camera_submission::CameraSubmissionPlan,
    },
    CameraPoll {
        operation_id: String,
    },
    CameraCancel {
        operation_id: String,
    },
    ReceiverBrightnessRead {
        path: String,
        device_container_id: String,
    },
    ReceiverBrightnessWrite {
        path: String,
        device_container_id: String,
        percent: u8,
    },
    ReceiverIdentityRead {
        path: String,
        device_container_id: String,
    },
    /// Portable OS collections. Paths are opaque bytes; no Windows GUID or
    /// logical receiver peer is fabricated from these nodes.
    HidNodes,
    /// Observe the retained collection's actual descriptor, without sending a
    /// Razer command. Used to disambiguate source-selected transport routes.
    HidNodeReports {
        node: razer_device::backend::HidNode,
    },
    /// Current V3 trigger calibration owns its exact collection until cleanup.
    /// Acceptance and task completion are distinct from verified device writes.
    ControllerTriggers {
        node: razer_device::backend::HidNode,
        product_id: u32,
        trigger_operation: razer_device::controller_triggers::TriggerOperation,
    },
    ControllerInputStart {
        product_id: u32,
        physical_product_id: u32,
        device_container_id: String,
        owner_guid: String,
    },
    ControllerInputPoll {
        owner_guid: String,
    },
    ControllerInputStop {
        owner_guid: String,
    },
    ControllerTriggerCalibrationStart {
        operation_id: String,
        node: razer_device::backend::HidNode,
        product_id: u32,
        part_id: u8,
    },
    ControllerTriggerCalibrationPoll {
        operation_id: String,
    },
    ControllerTriggerCalibrationCancel {
        operation_id: String,
    },
    HidNodeRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
        kind: razer_device::device_reads::DeviceReadKind,
    },
    /// A retained mouse peer on an observed physical receiver collection.
    /// The worker must re-query the association around the mouse getter.
    HidNodeMouseReceiverRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: u32,
        kind: razer_device::device_reads::DeviceReadKind,
    },
    HidNodeMouseReceiverDpiStages {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: u32,
        draft: Option<razer_device::mouse_dpi_stages::DpiStagesDraft>,
    },
    HidNodeMouseReceiverPolling {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: u32,
        hz: u32,
    },
    HidNodeMouseReceiverIdle {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: u32,
        raw_time: u16,
    },
    /// Typed direct-device write; source capability, live identity, reply and
    /// readback are checked by the agent. No arbitrary reports or exports.
    HidNodeWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        setting: razer_device::device_writes::DeviceWriteSetting,
    },
    HidNodeReceiverStatus {
        node: razer_device::backend::HidNode,
    },
    HidNodeReceiverStatusForSource {
        node: razer_device::backend::HidNode,
        source_product_id: u16,
    },
    HidNodeReceiverDockMode {
        node: razer_device::backend::HidNode,
    },
    HidNodeDpiStagesRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
    },
    HidNodeDpiStagesWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        draft: razer_device::mouse_dpi_stages::DpiStagesDraft,
    },
    HidNodeMouseCurrentDpi {
        node: razer_device::backend::HidNode,
        product_id: u32,
        #[serde(default)]
        peer_product_id: Option<u32>,
        change: Option<razer_device::mouse_current_dpi::DpiChange>,
    },
    KeyboardIndicator {
        target: razer_device::keyboard_indicator::Target,
        change: razer_device::keyboard_indicator::Change,
    },
    KeyboardLatestRestore {
        target: razer_device::keyboard_latest_protocol::Target,
        protocol_operation: razer_device::keyboard_latest_protocol::Operation,
    },
    KeyboardLegacyCalibration {
        target: razer_device::keyboard_legacy_calibration::Target,
        calibration_operation: razer_device::keyboard_legacy_calibration::Operation,
    },
    MouseReceiverLegacy {
        target: razer_device::mouse_receiver_legacy::Target,
        change: razer_device::mouse_receiver_legacy::Change,
    },
    KeyboardAnalogStart {
        operation_id: String,
        target: razer_device::keyboard_submission::Target,
        profile_guid: String,
        intent: razer_device::keyboard_submission::Intent,
    },
    KeyboardAnalogPoll {
        operation_id: String,
    },
    KeyboardAnalogCancel {
        operation_id: String,
    },
    /// Source initial HW-module getter plus retained Protocol25 input events.
    /// Starting a reader is not a successful device observation.
    KeyboardWristrestStart {
        operation_id: String,
        target: razer_device::keyboard_wristrest::Target,
    },
    KeyboardWristrestPoll {
        operation_id: String,
    },
    KeyboardWristrestStop {
        operation_id: String,
    },
    MouseRuntimeArtworkStart {
        operation_id: String,
        target: razer_device::mouse_runtime_artwork::Target,
    },
    MouseRuntimeArtworkPoll {
        operation_id: String,
    },
    MouseRuntimeArtworkStop {
        operation_id: String,
    },
    HidNodeKeyboardBrightnessRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
    },
    HidNodeKeyboardReceiverBrightness {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: u32,
        percent: Option<u8>,
    },
    HidNodeKeyboardMetadataRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
    },
    HidNodeKeyboardSerialRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
    },
    HidNodeKeyboardRelayMetadataRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: u32,
    },
    HidNodeKeyboardRelaySerialRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: u32,
    },
    HidNodeKeyboardBrightnessWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        percent: u8,
    },
    /// Current Audio Mixer DSP reports; some queries use a hardware mailbox
    /// selector, so this is not a side-effect-free register snapshot.
    HidNodeMixerRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
        target: razer_device::audio_mixer::MixerTarget,
    },
    HidNodeMixerLocalWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        command: razer_device::audio_mixer_local::Command,
    },
    AudioMixerMeterStart {
        node: razer_device::backend::HidNode,
        mixer: bool,
    },
    AudioMixerMeterRead {
        node: razer_device::backend::HidNode,
    },
    AudioMixerMeterStop,
    AudioMixerHardwareStart {
        node: razer_device::backend::HidNode,
    },
    AudioMixerHardwareRead {
        node: razer_device::backend::HidNode,
    },
    AudioMixerHardwareStop,
    AudioMixerHardwareInfo {
        node: razer_device::backend::HidNode,
    },
    HidNodeMixerWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        target: razer_device::audio_mixer::MixerTarget,
        value: razer_device::audio_mixer::MixerValue,
    },
    /// Original microphone EQ caller enables and submits all ten bands.
    HidNodeMixerEqWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        bands: [i32; 10],
    },
    HidNodeMixerRouteRead {
        node: razer_device::backend::HidNode,
        product_id: u32,
        route: razer_device::audio_mixer::MixerRoute,
    },
    HidNodeMixerRouteWrite {
        node: razer_device::backend::HidNode,
        product_id: u32,
        route: razer_device::audio_mixer::MixerRoute,
        enabled: bool,
    },
    /// Original restartAudioDriver sequence; reports IOCTL completion, not
    /// a fabricated observation of audio playback or stream state.
    HidNodeMixerRestartStreams {
        node: razer_device::backend::HidNode,
        product_id: u32,
    },
    /// Physical USB devices, including products without a HID collection.
    UsbDevices,
    /// Read-only version query on any library the generated native inventory
    /// declares. Only a no-argument string-returning `GetDLLVersion`/`GetDllVersion`
    /// prefix is accepted, so no mutation can be requested through it.
    NativeLibraryVersion {
        library: String,
        #[serde(default)]
        product_id: Option<u32>,
    },
    /// Read-only getter on a declared device library
    /// (`Get*`/`Is*`/`Has*` with one `string` argument). Mutating exports are
    /// refused in the worker; see `backend::native_read`.
    NativeLibraryGetter {
        library: String,
        export: String,
        device_id: String,
        product_id: Option<u32>,
    },
    NativeLibrarySnapshot {
        library: String,
        device_container_id: String,
        product_id: u32,
    },
    /// A source-described query, scoped to a currently observed physical path.
    DeviceRead {
        target: razer_device::device_reads::DeviceReadTarget,
        kind: razer_device::device_reads::DeviceReadKind,
    },
    DeviceWrite {
        target: razer_device::device_reads::DeviceReadTarget,
        setting: razer_device::device_writes::DeviceWriteSetting,
    },
    DeviceKeyboardBrightnessRead {
        target: razer_device::device_reads::DeviceReadTarget,
    },
    DeviceKeyboardMetadataRead {
        target: razer_device::device_reads::DeviceReadTarget,
    },
    DeviceKeyboardSerialRead {
        target: razer_device::device_reads::DeviceReadTarget,
    },
    BleKeyboardMetadataRead {
        target: razer_device::mouse_runtime_artwork::WindowsBleTarget,
    },
    BleKeyboardSerialRead {
        target: razer_device::mouse_runtime_artwork::WindowsBleTarget,
    },
    DeviceDpiStagesRead {
        target: razer_device::device_reads::DeviceReadTarget,
    },
    DeviceDpiStagesWrite {
        target: razer_device::device_reads::DeviceReadTarget,
        draft: razer_device::mouse_dpi_stages::DpiStagesDraft,
    },
    DeviceMouseCurrentDpi {
        target: razer_device::device_reads::DeviceReadTarget,
        change: Option<razer_device::mouse_current_dpi::DpiChange>,
    },
    DeviceKeyboardBrightnessWrite {
        target: razer_device::device_reads::DeviceReadTarget,
        percent: u8,
    },
    /// Retained current-host USB mapping engine owner. Completion requires the
    /// native callback and the correlated `driver ready` device event.
    MappingDeviceStart {
        product_id: u32,
        device_container_id: String,
        owner_guid: String,
    },
    MappingDevicePublish {
        owner_guid: String,
        document: Value,
    },
    MappingDeviceEvents {
        owner_guid: String,
    },
    MappingDeviceMacroRecordingMode {
        owner_guid: String,
    },
    MappingDeviceHypershiftActive {
        owner_guid: String,
    },
    MappingDeviceStop {
        owner_guid: String,
    },
    MappingKeyboardGamingIndicator {
        owner_guid: String,
        enabled: bool,
    },
    /// Observe global input through the current mapping-engine recorder.
    /// These requests do not submit macros or mappings to a device.
    StartMacroRecording,
    /// Current background OTFM registers listeners before resolving its input
    /// device and suppresses mapping before starting the kOTFM recorder.
    PrepareOtfmRecorder,
    StartOtfmRecording,
    QueryMacroRecordingStarted,
    MacroRecorderGlobalState,
    SetOtfmMappingsSuspended {
        suspended: bool,
    },
    MappingOtfmInputStart {
        owner_guid: String,
    },
    MacroRecorderStorageWrite {
        key: String,
        value: Value,
    },
    StartShortcutCapture,
    StartMappingKeyCapture,
    StartKeyboardCalibrationCapture {
        product_id: u32,
        container_id: String,
    },
    MappingKeyCaptureEvents,
    StopMappingKeyCapture,
    ShortcutCaptureEvents,
    StopShortcutCapture,
    StopMacroRecording,
    MacroRecordingEvents,
    SuspendMacroMappings,
    ResumeMacroMappings,
    ReceiverWirelessStatus {
        path: String,
        device_container_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_product_id: Option<u16>,
    },
    ReceiverDockModeRead {
        path: String,
        device_container_id: String,
    },
    ReceiverPairingStart {
        operation_id: String,
        path: String,
        device_container_id: String,
        category: String,
        action: razer_device::receiver_pairing::PairingAction,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_product_id: Option<u16>,
    },
    ReceiverPairingPoll {
        operation_id: String,
    },
    ReceiverPairingCancel {
        operation_id: String,
    },
    ReceiverReconnectStart {
        operation_id: String,
        product_id: u16,
        device_container_id: String,
        category: String,
        next_transaction_sequence: u8,
        remaining_delay_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source_product_id: Option<u16>,
    },
    ReceiverReconnectPoll {
        operation_id: String,
    },
    /// Resource-owner disposal, independent of source DUALLINK_CANCEL.
    ReceiverReconnectCancel {
        operation_id: String,
    },
    GlobalMode,
    WheelScrollLinesRead,
    WheelScrollLinesWrite {
        lines: i32,
    },
    WindowsServiceStatus {
        name: String,
    },
    WindowsServiceStart {
        name: String,
    },
    WindowsServiceStop {
        name: String,
    },
    GlobalShortcuts,
    EnableGlobalShortcuts {
        enable: bool,
    },
    RegisterShortcut {
        vkey_code: u32,
        modifiers: u32,
        argument: String,
    },
    UnregisterShortcut {
        vkey_code: u32,
        modifiers: u32,
    },
    /// Caller supplies the original generateAppEngineMappings result and hash.
    /// This does not accept an arbitrary UI mapping object.
    SubmitGlobalShortcutMappings {
        app_engine: Value,
    },
    ShortcutEvents,
    Shutdown,
}

/// Diagnostic CLI requests use the same bounded worker and cleanup as the UI.
/// This function is a runtime entrypoint, never a static verification command.
pub fn isolated_request(request: ServiceRequest) -> anyhow::Result<Value> {
    let mut client = ServiceClient::spawn()?;
    let result = client.request(request);
    let shutdown = if client.is_stopped() {
        Ok(Value::Null)
    } else {
        client.request(ServiceRequest::Shutdown)
    };
    match (result, shutdown) {
        (Ok(value), Ok(_)) => Ok(value),
        (Err(error), Ok(_)) => Err(error),
        (Ok(_), Err(error)) => Err(error.context("查询后的 worker 关闭失败")),
        (Err(error), Err(shutdown)) => {
            Err(error.context(format!("worker 关闭同时失败：{shutdown:#}")))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub id: u64,
    pub request: ServiceRequest,
}

#[derive(Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub id: u64,
    pub data: Option<Value>,
    pub error: Option<String>,
    pub fatal: bool,
}

/// A single long-lived worker. Drop terminates only this owned child process.
pub struct ServiceClient {
    child: Option<Child>,
    #[cfg(windows)]
    job: Option<job::WorkerJob>,
    requests: Option<Sender<Vec<u8>>>,
    responses: Receiver<anyhow::Result<ResponseEnvelope>>,
    timeout: Duration,
    next_id: u64,
    stopped: bool,
    request_guard: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

impl ServiceClient {
    pub fn spawn() -> anyhow::Result<Self> {
        let current = std::env::current_exe()?;
        let sibling = current.with_file_name(if cfg!(windows) {
            "razer_agent.exe"
        } else {
            "razer_agent"
        });
        // Distributions include the independent headless binary. Existing
        // root-only development commands retain their pre-GPUI worker route.
        let executable = if sibling.is_file() { sibling } else { current };
        Self::spawn_with_program(&executable)
    }

    /// Explicit child executable for hosts that package the agent separately.
    /// No DLL/backend is initialized in the caller process.
    pub fn spawn_with_program(executable: &std::path::Path) -> anyhow::Result<Self> {
        #[cfg(windows)]
        let job = job::WorkerJob::new()?;
        let mut command = Command::new(executable);
        command
            .arg("--service-worker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW: do not expose a console for a background service.
            command.creation_flags(0x0800_0000);
        }
        let mut child = command.spawn().context("无法启动设备服务子进程")?;
        #[cfg(windows)]
        if let Err(error) = job.assign(&child) {
            return Err(with_cleanup_error(error, terminate_child(child)));
        }
        let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(with_cleanup_error(
                anyhow::anyhow!("设备服务通信管道不可用"),
                terminate_child(child),
            ));
        };
        let (requests, pending) = mpsc::channel::<Vec<u8>>();
        // Writing a pipe can block if startup hangs before the worker reads it.
        // Keep it off the request thread so recv_timeout covers the full request.
        if let Err(error) = std::thread::Builder::new()
            .name("razer-service-input".into())
            .spawn(move || {
                while let Ok(frame) = pending.recv() {
                    if stdin.write_all(&frame).and_then(|_| stdin.flush()).is_err() {
                        break;
                    }
                }
            })
        {
            return Err(with_cleanup_error(
                anyhow::Error::new(error).context("无法创建设备服务输入线程"),
                terminate_child(child),
            ));
        }
        let (sender, responses) = mpsc::channel();
        if let Err(error) = std::thread::Builder::new()
            .name("razer-service-output".into())
            .spawn(move || {
                let mut reader = BufReader::new(stdout);
                forward_responses(&mut reader, sender);
            })
        {
            drop(requests);
            return Err(with_cleanup_error(
                anyhow::Error::new(error).context("无法创建设备服务输出线程"),
                terminate_child(child),
            ));
        }
        Ok(Self {
            child: Some(child),
            #[cfg(windows)]
            job: Some(job),
            requests: Some(requests),
            responses,
            // First registration can perform initialization, event registration,
            // and registration itself (three bounded native callbacks).
            timeout: Duration::from_secs(15),
            next_id: 1,
            stopped: false,
            request_guard: None,
        })
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// An operation owns this worker exclusively. Invalidating its scope stops
    /// retries and later setters by terminating only this owned process. An
    /// already submitted device command cannot be rolled back by cancellation.
    pub fn set_request_guard(&mut self, guard: std::sync::Arc<std::sync::atomic::AtomicBool>) {
        self.request_guard = Some(guard);
    }

    pub fn request(&mut self, request: ServiceRequest) -> anyhow::Result<Value> {
        if self.stopped {
            bail!("设备服务已停止，请重新连接");
        }
        let shutting_down = matches!(request, ServiceRequest::Shutdown);
        if !shutting_down
            && self
                .request_guard
                .as_ref()
                .is_some_and(|guard| !guard.load(std::sync::atomic::Ordering::Acquire))
        {
            return Err(
                self.stop_after_error(anyhow::anyhow!("设备操作范围已取消，独占服务已关闭"))
            );
        }
        let starting_recorder = matches!(
            request,
            ServiceRequest::StartMacroRecording
                | ServiceRequest::PrepareOtfmRecorder
                | ServiceRequest::StartOtfmRecording
        );
        let controller_triggers = matches!(&request, ServiceRequest::ControllerTriggers { .. });
        let starting_mapping = matches!(
            request,
            ServiceRequest::MappingDeviceStart { .. }
                | ServiceRequest::ControllerInputStart { .. }
                | ServiceRequest::MappingOtfmInputStart { .. }
        );
        let starting_key_capture = matches!(
            request,
            ServiceRequest::StartMappingKeyCapture
                | ServiceRequest::StartKeyboardCalibrationCapture { .. }
        );
        let writing_app_files =
            matches!(&request, ServiceRequest::UserAppFiles { command } if command.mutates_files());
        let writing_device = matches!(
            request,
            ServiceRequest::DeviceWrite { .. }
                | ServiceRequest::HidNodeMouseScroll { .. }
                | ServiceRequest::DeviceMouseScroll { .. }
                | ServiceRequest::HidNodeSeirenEq { .. }
                | ServiceRequest::DeviceSeirenEq { .. }
                | ServiceRequest::SeirenNativeOpen { .. }
                | ServiceRequest::SeirenNativeCall { .. }
                | ServiceRequest::SeirenNativeEvents { .. }
                | ServiceRequest::SeirenNativeClose { .. }
                | ServiceRequest::ReceiverBrightnessWrite { .. }
                | ServiceRequest::AudioRouteDevice { .. }
                | ServiceRequest::AudioRoutingEnable { .. }
                | ServiceRequest::AudioVolumeWrite { .. }
                | ServiceRequest::HidNodeDpiStagesWrite { .. }
                | ServiceRequest::HidNodeMouseReceiverDpiStages { draft: Some(_), .. }
                | ServiceRequest::HidNodeMouseReceiverPolling { .. }
                | ServiceRequest::HidNodeMouseReceiverIdle { .. }
                | ServiceRequest::DeviceDpiStagesWrite { .. }
                | ServiceRequest::HidNodeKeyboardBrightnessWrite { .. }
                | ServiceRequest::HidNodeKeyboardReceiverBrightness {
                    percent: Some(_),
                    ..
                }
                | ServiceRequest::HidNodeMouseCurrentDpi {
                    change: Some(_),
                    ..
                }
                | ServiceRequest::DeviceMouseCurrentDpi {
                    change: Some(_),
                    ..
                }
                | ServiceRequest::DeviceKeyboardBrightnessWrite { .. }
                | ServiceRequest::MappingKeyboardGamingIndicator { .. }
                | ServiceRequest::KeyboardIndicator { .. }
                | ServiceRequest::KeyboardLatestRestore { .. }
                | ServiceRequest::KeyboardLegacyCalibration { .. }
                | ServiceRequest::MouseReceiverLegacy { .. }
                | ServiceRequest::MappingDeviceStart { .. }
                | ServiceRequest::MappingDevicePublish { .. }
                | ServiceRequest::SetOtfmMappingsSuspended { .. }
                | ServiceRequest::MacroRecorderGlobalState
                | ServiceRequest::MacroRecorderStorageWrite { .. }
                | ServiceRequest::StartMappingKeyCapture
                | ServiceRequest::StartKeyboardCalibrationCapture { .. }
                | ServiceRequest::MappingKeyCaptureEvents
                | ServiceRequest::StopMappingKeyCapture
                | ServiceRequest::StartShortcutCapture
                | ServiceRequest::HidNodeWrite { .. }
                | ServiceRequest::HidNodeMixerWrite { .. }
                | ServiceRequest::HidNodeMixerLocalWrite { .. }
                | ServiceRequest::AudioMixerMeterStart { .. }
                | ServiceRequest::AudioMixerMeterRead { .. }
                | ServiceRequest::AudioMixerHardwareStart { .. }
                | ServiceRequest::AudioMixerHardwareRead { .. }
                | ServiceRequest::AudioMixerHardwareStop
                | ServiceRequest::AudioMixerHardwareInfo { .. }
                | ServiceRequest::AudioMixerMeterStop
                | ServiceRequest::HidNodeMixerEqWrite { .. }
                | ServiceRequest::HidNodeMixerRouteWrite { .. }
                | ServiceRequest::HidNodeMixerRestartStreams { .. }
        ) || matches!(
            &request,
            ServiceRequest::HidNodeMouseCurrentPower { change, .. }
                | ServiceRequest::DeviceMouseCurrentPower { change, .. }
                | ServiceRequest::BleMouseCurrentPower { change, .. }
                if !matches!(change, razer_device::mouse_current_power::Change::ReadPowerSaving)
        ) || matches!(
            &request,
            ServiceRequest::HidNodeMouseIndicatorLed { change, .. }
                | ServiceRequest::DeviceMouseIndicatorLed { change, .. } if change.writes()
        ) || matches!(
            &request,
            ServiceRequest::HidNodeLeviathan { command, .. } | ServiceRequest::DeviceLeviathan { command, .. } if !command.is_read()
        );
        let reading_device = matches!(
            request,
            ServiceRequest::DeviceRead { .. }
                | ServiceRequest::HidNodeLeviathan {
                    command: razer_device::leviathan::Operation::ReadIo,
                    ..
                }
                | ServiceRequest::DeviceLeviathan {
                    command: razer_device::leviathan::Operation::ReadIo,
                    ..
                }
                | ServiceRequest::LeviathanEventsStart { .. }
                | ServiceRequest::LeviathanEventsPoll { .. }
                | ServiceRequest::LeviathanEventsStop { .. }
                | ServiceRequest::MouseIndicatorLedStart { .. }
                | ServiceRequest::MouseIndicatorLedPoll { .. }
                | ServiceRequest::MouseIndicatorLedStop { .. }
                | ServiceRequest::KeyboardWristrestStart { .. }
                | ServiceRequest::KeyboardWristrestPoll { .. }
                | ServiceRequest::KeyboardWristrestStop { .. }
                | ServiceRequest::MouseRuntimeArtworkStart { .. }
                | ServiceRequest::MouseRuntimeArtworkPoll { .. }
                | ServiceRequest::MouseRuntimeArtworkStop { .. }
                | ServiceRequest::ReceiverDockModeRead { .. }
                | ServiceRequest::HidNodeReceiverDockMode { .. }
                | ServiceRequest::HidNodeReceiverStatus { .. }
                | ServiceRequest::HidNodeReceiverStatusForSource { .. }
                | ServiceRequest::ReceiverBrightnessRead { .. }
                | ServiceRequest::AudioRouterEvents
                | ServiceRequest::AudioVolumeRead { .. }
                | ServiceRequest::HidNodeDpiStagesRead { .. }
                | ServiceRequest::HidNodeMouseReceiverDpiStages { draft: None, .. }
                | ServiceRequest::DeviceDpiStagesRead { .. }
                | ServiceRequest::HidNodeKeyboardBrightnessRead { .. }
                | ServiceRequest::HidNodeKeyboardReceiverBrightness { percent: None, .. }
                | ServiceRequest::HidNodeKeyboardMetadataRead { .. }
                | ServiceRequest::HidNodeKeyboardSerialRead { .. }
                | ServiceRequest::HidNodeKeyboardRelayMetadataRead { .. }
                | ServiceRequest::HidNodeKeyboardRelaySerialRead { .. }
                | ServiceRequest::HidNodeMouseCurrentDpi { change: None, .. }
                | ServiceRequest::DeviceMouseCurrentDpi { change: None, .. }
                | ServiceRequest::HidNodeMouseCurrentPower {
                    change: razer_device::mouse_current_power::Change::ReadPowerSaving,
                    ..
                }
                | ServiceRequest::DeviceMouseCurrentPower {
                    change: razer_device::mouse_current_power::Change::ReadPowerSaving,
                    ..
                }
                | ServiceRequest::BleMouseCurrentPower {
                    change: razer_device::mouse_current_power::Change::ReadPowerSaving,
                    ..
                }
                | ServiceRequest::DeviceKeyboardBrightnessRead { .. }
                | ServiceRequest::DeviceKeyboardMetadataRead { .. }
                | ServiceRequest::DeviceKeyboardSerialRead { .. }
                | ServiceRequest::BleKeyboardMetadataRead { .. }
                | ServiceRequest::BleKeyboardSerialRead { .. }
                | ServiceRequest::NativeLibrarySnapshot { .. }
                | ServiceRequest::AudioEndpoints { .. }
                | ServiceRequest::AudioNotificationsEnable { .. }
                | ServiceRequest::AudioNotificationsDrain
                | ServiceRequest::ForegroundMonitorStart { .. }
                | ServiceRequest::ForegroundMonitorStop { .. }
                | ServiceRequest::ForegroundMonitorEvents { .. }
                | ServiceRequest::HidNodeRead { .. }
                | ServiceRequest::HidNodeMouseReceiverRead { .. }
                | ServiceRequest::HidNodeMouseIndicatorLed {
                    change: razer_device::mouse_indicator_led::Change::ReadConfiguration
                        | razer_device::mouse_indicator_led::Change::ReadConnectionQuality
                        | razer_device::mouse_indicator_led::Change::ReadSerialNumber
                        | razer_device::mouse_indicator_led::Change::ReadBatteryLevel
                        | razer_device::mouse_indicator_led::Change::ReadChargingStatus,
                    ..
                }
                | ServiceRequest::DeviceMouseIndicatorLed {
                    change: razer_device::mouse_indicator_led::Change::ReadConfiguration
                        | razer_device::mouse_indicator_led::Change::ReadConnectionQuality
                        | razer_device::mouse_indicator_led::Change::ReadSerialNumber
                        | razer_device::mouse_indicator_led::Change::ReadBatteryLevel
                        | razer_device::mouse_indicator_led::Change::ReadChargingStatus,
                    ..
                }
                | ServiceRequest::HidNodeMixerRead { .. }
                | ServiceRequest::HidNodeMixerRouteRead { .. }
        );
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let mut frame = serde_json::to_vec(&RequestEnvelope { id, request })?;
        if frame.len() >= MAX_FRAME_BYTES {
            bail!("设备服务请求过大");
        }
        frame.push(b'\n');
        let write = self
            .requests
            .as_ref()
            .context("设备服务已关闭")?
            .send(frame);
        if let Err(error) = write {
            let error = anyhow::Error::new(error).context("设备服务连接中断");
            return Err(self.stop_after_error(error));
        }
        // Recorder setup can require initialize/register/callback/start, each
        // with its own bounded native completion. Other queries retain their
        // existing timeout; a timeout still terminates this owned worker.
        let timeout = if shutting_down {
            // Recorder stop, mapping restore, callback unregister and two
            // independent engine shutdowns can each consume CALLBACK_TIMEOUT.
            Duration::from_secs(25)
        } else if controller_triggers {
            // Eight current getters or four ordered setters retain their source
            // retry budgets. Scope cancellation still interrupts every 50ms.
            Duration::from_secs(95)
        } else if starting_key_capture {
            // One initialization plus bounded registration/readiness/redirect
            // callbacks for the attached input endpoints.
            Duration::from_secs(60)
        } else if starting_mapping {
            // Initialization, USB registration/readiness and two pairs of
            // event-registration callbacks each have a bounded native wait.
            Duration::from_secs(40)
        } else if starting_recorder {
            Duration::from_secs(20)
        } else if writing_device || writing_app_files {
            // Retained route: pre-read, setter acknowledgement, then readback.
            // Keyboard brightness has a 20-second service confirmation deadline;
            // the ordinary 15-second timeout must not kill its worker first.
            Duration::from_secs(30)
        } else if reading_device {
            // Relay reads bracket the device query with two bounded peer
            // observations, so the ordinary single-query limit is insufficient.
            Duration::from_secs(35)
        } else {
            self.timeout
        };
        let received = if shutting_down || self.request_guard.is_none() {
            self.responses
                .recv_timeout(timeout)
                .map_err(anyhow::Error::new)
        } else {
            let started = std::time::Instant::now();
            loop {
                if self
                    .request_guard
                    .as_ref()
                    .is_some_and(|guard| !guard.load(std::sync::atomic::Ordering::Acquire))
                {
                    break Err(anyhow::anyhow!(
                        "设备操作范围已取消；已发送的设置需重新读取确认"
                    ));
                }
                let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                    break Err(anyhow::anyhow!("设备服务确认超时"));
                };
                match self
                    .responses
                    .recv_timeout(remaining.min(Duration::from_millis(50)))
                {
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    response => break response.map_err(anyhow::Error::new),
                }
            }
        };
        let response = match received {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => return Err(self.stop_after_error(error)),
            Err(error) => {
                let error = error.context("设备服务超时或已退出；连接已关闭");
                return Err(self.stop_after_error(error));
            }
        };
        self.complete_response(id, shutting_down, response)
    }

    fn complete_response(
        &mut self,
        id: u64,
        shutting_down: bool,
        response: ResponseEnvelope,
    ) -> anyhow::Result<Value> {
        if response.id != id {
            return Err(
                self.stop_after_error(anyhow::anyhow!("设备服务响应序号不匹配，连接已关闭"))
            );
        }
        // A reply can wake recv_timeout after the owner invalidated its scope.
        // Recheck at delivery so that receipt cannot become a current result.
        // Shutdown still runs for an expired scope; cancellation cannot undo an
        // already submitted device command or establish its present state.
        if !shutting_down
            && self
                .request_guard
                .as_ref()
                .is_some_and(|guard| !guard.load(std::sync::atomic::Ordering::Acquire))
        {
            return Err(self.stop_after_error(anyhow::anyhow!(
                "设备操作范围已取消；旧响应未提交，已发送的设置仍可能生效"
            )));
        }
        let stop = if response.fatal || shutting_down {
            self.stop()
        } else {
            Ok(())
        };
        if let Some(error) = response.error {
            if let Err(cleanup) = stop {
                bail!("{error}；关闭设备服务失败：{cleanup:#}");
            }
            bail!("{error}");
        }
        stop?;
        response.data.context("设备服务返回了空响应")
    }

    fn stop_after_error(&mut self, error: anyhow::Error) -> anyhow::Error {
        with_cleanup_error(error, self.stop())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        self.requests.take();
        self.stopped = true;
        let result = self.child.take().map(terminate_child).unwrap_or(Ok(()));
        // Job closure also terminates the worker if explicit kill failed. OS
        // process exit closes this handle even if Rust Drop never gets to run.
        #[cfg(windows)]
        self.job.take();
        result
    }
}

/// Unprefixed vendor diagnostics may use a legacy code page. Once the worker
/// emits our prefix, however, every byte belongs to the owned IPC protocol:
/// decoding or framing failure is terminal, never a log line to skip.
fn forward_responses(reader: &mut impl BufRead, sender: Sender<anyhow::Result<ResponseEnvelope>>) {
    loop {
        let response = (|| {
            let Some(line) = read_frame(reader).context("读取设备服务响应失败，连接已关闭")?
            else {
                bail!("设备服务输出已关闭");
            };
            let Some(json) = line.strip_prefix(FRAME_PREFIX.as_bytes()) else {
                return Ok(None);
            };
            let response = serde_json::from_slice::<ResponseEnvelope>(json)
                .context("设备服务响应格式无效，连接已关闭")?;
            Ok(Some(response))
        })();
        match response {
            Ok(None) => continue,
            Ok(Some(response)) => {
                if sender.send(Ok(response)).is_err() {
                    return;
                }
            }
            Err(error) => {
                let _ = sender.send(Err(error));
                return;
            }
        }
    }
}

fn with_cleanup_error(error: anyhow::Error, cleanup: anyhow::Result<()>) -> anyhow::Error {
    match cleanup {
        Ok(()) => error,
        Err(cleanup) => anyhow::anyhow!("{error:#}；关闭设备服务失败：{cleanup:#}"),
    }
}

impl Drop for ServiceClient {
    fn drop(&mut self) {
        if !self.stopped {
            let _ = self.stop();
        }
    }
}

// Terminate only our owned child. `wait` can block in the OS, so even after a
// successful kill it belongs to a detached reaper, never the request thread.
fn terminate_child(mut child: Child) -> anyhow::Result<()> {
    if child.try_wait().ok().flatten().is_some() {
        return Ok(());
    }
    if let Err(error) = child.kill() {
        // Exiting between try_wait and kill is an ordinary race.
        if child.try_wait().ok().flatten().is_none() {
            return Err(error).context("无法结束设备服务子进程");
        }
        return Ok(());
    }
    std::thread::Builder::new()
        .name("razer-service-reaper".into())
        .spawn(move || {
            let _ = child.wait();
        })
        .context("无法创建设备服务回收线程")?;
    Ok(())
}

pub fn read_frame(reader: &mut impl BufRead) -> anyhow::Result<Option<Vec<u8>>> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if bytes.is_empty() {
                Ok(None)
            } else {
                bail!("不完整的设备服务数据帧")
            };
        }
        let end = available.iter().position(|byte| *byte == b'\n');
        let length = end.map_or(available.len(), |index| index + 1);
        if bytes.len() + length > MAX_FRAME_BYTES {
            bail!("设备服务数据帧过大");
        }
        bytes.extend_from_slice(&available[..length]);
        reader.consume(length);
        if end.is_some() {
            // Vendor diagnostics can use a legacy Windows code page. Only a
            // prefixed JSON response needs UTF-8; unrelated bytes are ignored.
            return Ok(Some(bytes));
        }
    }
}
